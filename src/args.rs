use std::cmp;
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use atty;
use clap;
use grep::matcher::LineTerminator;
#[cfg(feature = "pcre2")]
use grep::pcre2::{
    RegexMatcher as PCRE2RegexMatcher,
    RegexMatcherBuilder as PCRE2RegexMatcherBuilder,
};
use grep::printer::{
    ColorSpecs, Stats,
    JSON, JSONBuilder,
    Standard, StandardBuilder,
    Summary, SummaryBuilder, SummaryKind,
};
use grep::regex::{
    RegexMatcher as RustRegexMatcher,
    RegexMatcherBuilder as RustRegexMatcherBuilder,
};
use grep::searcher::{
    BinaryDetection, Encoding, MmapChoice, Searcher, SearcherBuilder,
};
use ignore::overrides::{Override, OverrideBuilder};
use ignore::types::{FileTypeDef, Types, TypesBuilder};
use ignore::{Walk, WalkBuilder, WalkParallel};
use log;
use num_cpus;
use path_printer::{PathPrinter, PathPrinterBuilder};
use regex::{self, Regex};
use same_file::Handle;
use termcolor::{
    WriteColor,
    BufferedStandardStream, BufferWriter, ColorChoice, StandardStream,
};

use app;
use config;
use logger::Logger;
use messages::{set_messages, set_ignore_messages};
use search::{PatternMatcher, Printer, SearchWorker, SearchWorkerBuilder};
use subject::SubjectBuilder;
use unescape::{escape, unescape};
use Result;

/// The command that ripgrep should execute based on the command line
/// configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    /// Search using exactly one thread.
    Search,
    /// Search using possibly many threads.
    SearchParallel,
    /// The command line parameters suggest that a search should occur, but
    /// ripgrep knows that a match can never be found (e.g., no given patterns
    /// or --max-count=0).
    SearchNever,
    /// Show the files that would be searched, but don't actually search them,
    /// and use exactly one thread.
    Files,
    /// Show the files that would be searched, but don't actually search them,
    /// and perform directory traversal using possibly many threads.
    FilesParallel,
    /// List all file type definitions configured, including the default file
    /// types and any additional file types added to the command line.
    Types,
}

impl Command {
    /// Returns true if and only if this command requires executing a search.
    fn is_search(&self) -> bool {
        use self::Command::*;

        match *self {
            Search | SearchParallel => true,
            SearchNever | Files | FilesParallel | Types => false,
        }
    }
}

/// The primary configuration object used throughout ripgrep. It provides a
/// high-level convenient interface to the provided command line arguments.
///
/// An `Args` object is cheap to clone and can be used from multiple threads
/// simultaneously.
#[derive(Clone, Debug)]
pub struct Args(Arc<ArgsImp>);

#[derive(Clone, Debug)]
struct ArgsImp {
    /// Mid-to-low level routines for extracting CLI arguments.
    matches: ArgMatches,
    /// The patterns provided at the command line and/or via the -f/--file
    /// flag. This may be empty.
    patterns: Vec<String>,
    /// A matcher built from the patterns.
    ///
    /// It's important that this is only built once, since building this goes
    /// through regex compilation and various types of analyses. That is, if
    /// you need many of theses (one per thread, for example), it is better to
    /// build it once and then clone it.
    matcher: PatternMatcher,
    /// The paths provided at the command line. This is guaranteed to be
    /// non-empty. (If no paths are provided, then a default path is created.)
    paths: Vec<PathBuf>,
    /// Returns true if and only if `paths` had to be populated with a single
    /// default path.
    using_default_path: bool,
}

impl Args {
    /// Parse the command line arguments for this process.
    ///
    /// If a CLI usage error occurred, then exit the process and print a usage
    /// or error message. Similarly, if the user requested the version of
    /// ripgrep, then print the version and exit.
    ///
    /// Also, initialize a global logger.
    pub fn parse() -> Result<Args> {
        // We parse the args given on CLI. This does not include args from
        // the config. We use the CLI args as an initial configuration while
        // trying to parse config files. If a config file exists and has
        // arguments, then we re-parse argv, otherwise we just use the matches
        // we have here.
        let early_matches = ArgMatches::new(app::app().get_matches());
        set_messages(!early_matches.is_present("no-messages"));
        set_ignore_messages(!early_matches.is_present("no-ignore-messages"));

        if let Err(err) = Logger::init() {
            return Err(format!("failed to initialize logger: {}", err).into());
        }
        if early_matches.is_present("trace") {
            log::set_max_level(log::LevelFilter::Trace);
        } else if early_matches.is_present("debug") {
            log::set_max_level(log::LevelFilter::Debug);
        } else {
            log::set_max_level(log::LevelFilter::Warn);
        }

        let matches = early_matches.reconfigure();
        // The logging level may have changed if we brought in additional
        // arguments from a configuration file, so recheck it and set the log
        // level as appropriate.
        if matches.is_present("trace") {
            log::set_max_level(log::LevelFilter::Trace);
        } else if matches.is_present("debug") {
            log::set_max_level(log::LevelFilter::Debug);
        } else {
            log::set_max_level(log::LevelFilter::Warn);
        }
        set_messages(!matches.is_present("no-messages"));
        set_ignore_messages(!matches.is_present("no-ignore-messages"));
        matches.to_args()
    }

    /// Return direct access to command line arguments.
    fn matches(&self) -> &ArgMatches {
        &self.0.matches
    }

    /// Return the patterns found in the command line arguments. This includes
    /// patterns read via the -f/--file flags.
    fn patterns(&self) -> &[String] {
        &self.0.patterns
    }

    /// Return the matcher builder from the patterns.
    fn matcher(&self) -> &PatternMatcher {
        &self.0.matcher
    }

    /// Return the paths found in the command line arguments. This is
    /// guaranteed to be non-empty. In the case where no explicit arguments are
    /// provided, a single default path is provided automatically.
    fn paths(&self) -> &[PathBuf] {
        &self.0.paths
    }

    /// Returns true if and only if `paths` had to be populated with a default
    /// path, which occurs only when no paths were given as command line
    /// arguments.
    fn using_default_path(&self) -> bool {
        self.0.using_default_path
    }

    /// Return the printer that should be used for formatting the output of
    /// search results.
    ///
    /// The returned printer will write results to the given writer.
    fn printer<W: WriteColor>(&self, wtr: W) -> Result<Printer<W>> {
        match self.matches().output_kind() {
            OutputKind::Standard => {
                let separator_search = self.command()? == Command::Search;
                self.matches()
                    .printer_standard(self.paths(), wtr, separator_search)
                    .map(Printer::Standard)
            }
            OutputKind::Summary => {
                self.matches()
                    .printer_summary(self.paths(), wtr)
                    .map(Printer::Summary)
            }
            OutputKind::JSON => {
                self.matches()
                    .printer_json(wtr)
                    .map(Printer::JSON)
            }
        }
    }
}

/// High level public routines for building data structures used by ripgrep
/// from command line arguments.
impl Args {
    /// Create a new buffer writer for multi-threaded printing with color
    /// support.
    pub fn buffer_writer(&self) -> Result<BufferWriter> {
        let mut wtr = BufferWriter::stdout(self.matches().color_choice());
        wtr.separator(self.matches().file_separator()?);
        Ok(wtr)
    }

    /// Return the high-level command that ripgrep should run.
    pub fn command(&self) -> Result<Command> {
        let is_one_search = self.matches().is_one_search(self.paths());
        let threads = self.matches().threads()?;
        let one_thread = is_one_search || threads == 1;

        Ok(if self.matches().is_present("type-list") {
            Command::Types
        } else if self.matches().is_present("files") {
            if one_thread {
                Command::Files
            } else {
                Command::FilesParallel
            }
        } else if self.matches().can_never_match(self.patterns()) {
            Command::SearchNever
        } else if one_thread {
            Command::Search
        } else {
            Command::SearchParallel
        })
    }

    /// Builder a path printer that can be used for printing just file paths,
    /// with optional color support.
    ///
    /// The printer will print paths to the given writer.
    pub fn path_printer<W: WriteColor>(
        &self,
        wtr: W,
    ) -> Result<PathPrinter<W>> {
        let mut builder = PathPrinterBuilder::new();
        builder
            .color_specs(self.matches().color_specs()?)
            .separator(self.matches().path_separator()?)
            .terminator(self.matches().path_terminator().unwrap_or(b'\n'));
        Ok(builder.build(wtr))
    }

    /// Returns true if and only if the search should quit after finding the
    /// first match.
    pub fn quit_after_match(&self) -> Result<bool> {
        Ok(self.matches().is_present("quiet") && self.stats()?.is_none())
    }

    /// Build a worker for executing searches.
    ///
    /// Search results are written to the given writer.
    pub fn search_worker<W: WriteColor>(
        &self,
        wtr: W,
    ) -> Result<SearchWorker<W>> {
        let matcher = self.matcher().clone();
        let printer = self.printer(wtr)?;
        let searcher = self.matches().searcher(self.paths())?;
        let mut builder = SearchWorkerBuilder::new();
        builder
            .json_stats(self.matches().is_present("json"))
            .preprocessor(self.matches().preprocessor())
            .search_zip(self.matches().is_present("search-zip"));
        Ok(builder.build(matcher, searcher, printer))
    }

    /// Returns a zero value for tracking statistics if and only if it has been
    /// requested.
    ///
    /// When this returns a `Stats` value, then it is guaranteed that the
    /// search worker will be configured to track statistics as well.
    pub fn stats(&self) -> Result<Option<Stats>> {
        Ok(if self.command()?.is_search() && self.matches().stats() {
            Some(Stats::new())
        } else {
            None
        })
    }

    /// Return a builder for constructing subjects. A subject represents a
    /// single unit of something to search. Typically, this corresponds to a
    /// file or a stream such as stdin.
    pub fn subject_builder(&self) -> SubjectBuilder {
        let mut builder = SubjectBuilder::new();
        builder
            .strip_dot_prefix(self.using_default_path())
            .skip(self.matches().stdout_handle());
        builder
    }

    /// Execute the given function with a writer to stdout that enables color
    /// support based on the command line configuration.
    pub fn stdout(&self) -> Box<WriteColor + Send> {
        let color_choice = self.matches().color_choice();
        if atty::is(atty::Stream::Stdout) {
            Box::new(StandardStream::stdout(color_choice))
        } else {
            Box::new(BufferedStandardStream::stdout(color_choice))
        }
    }

    /// Return the type definitions compiled into ripgrep.
    ///
    /// If there was a problem reading and parsing the type definitions, then
    /// this returns an error.
    pub fn type_defs(&self) -> Result<Vec<FileTypeDef>> {
        Ok(self.matches().types()?.definitions().to_vec())
    }

    /// Return a walker that never uses additional threads.
    pub fn walker(&self) -> Result<Walk> {
        Ok(self.matches().walker_builder(self.paths())?.build())
    }

    /// Return a walker that never uses additional threads.
    pub fn walker_parallel(&self) -> Result<WalkParallel> {
        Ok(self.matches().walker_builder(self.paths())?.build_parallel())
    }
}

/// `ArgMatches` wraps `clap::ArgMatches` and provides semantic meaning to
/// the parsed arguments.
#[derive(Clone, Debug)]
struct ArgMatches(clap::ArgMatches<'static>);

/// The output format. Generally, this corresponds to the printer that ripgrep
/// uses to show search results.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputKind {
    /// Classic grep-like or ack-like format.
    Standard,
    /// Show matching files and possibly the number of matches in each file.
    Summary,
    /// Emit match information in the JSON Lines format.
    JSON,
}

impl ArgMatches {
    /// Create an ArgMatches from clap's parse result.
    fn new(clap_matches: clap::ArgMatches<'static>) -> ArgMatches {
        ArgMatches(clap_matches)
    }

    /// Run clap and return the matches using a config file if present. If clap
    /// determines a problem with the user provided arguments (or if --help or
    /// --version are given), then an error/usage/version will be printed and
    /// the process will exit.
    ///
    /// If there are no additional arguments from the environment (e.g., a
    /// config file), then the given matches are returned as is.
    fn reconfigure(self) -> ArgMatches {
        // If the end user says no config, then respect it.
        if self.is_present("no-config") {
            debug!("not reading config files because --no-config is present");
            return self;
        }
        // If the user wants ripgrep to use a config file, then parse args
        // from that first.
        let mut args = config::args();
        if args.is_empty() {
            return self;
        }
        let mut cliargs = env::args_os();
        if let Some(bin) = cliargs.next() {
            args.insert(0, bin);
        }
        args.extend(cliargs);
        debug!("final argv: {:?}", args);
        ArgMatches::new(app::app().get_matches_from(args))
    }

    /// Convert the result of parsing CLI arguments into ripgrep's higher level
    /// configuration structure.
    fn to_args(self) -> Result<Args> {
        // We compute these once since they could be large.
        let patterns = self.patterns()?;
        let matcher = self.matcher(&patterns)?;
        let mut paths = self.paths();
        let using_default_path =
            if paths.is_empty() {
                paths.push(self.path_default());
                true
            } else {
                false
            };
        Ok(Args(Arc::new(ArgsImp {
            matches: self,
            patterns: patterns,
            matcher: matcher,
            paths: paths,
            using_default_path: using_default_path,
        })))
    }
}

/// High level routines for converting command line arguments into various
/// data structures used by ripgrep.
///
/// Methods are sorted alphabetically.
impl ArgMatches {
    /// Return the matcher that should be used for searching.
    ///
    /// If there was a problem building the matcher (e.g., a syntax error),
    /// then this returns an error.
    #[cfg(feature = "pcre2")]
    fn matcher(&self, patterns: &[String]) -> Result<PatternMatcher> {
        if self.is_present("pcre2") {
            let matcher = self.matcher_pcre2(patterns)?;
            Ok(PatternMatcher::PCRE2(matcher))
        } else {
            let matcher = match self.matcher_rust(patterns) {
                Ok(matcher) => matcher,
                Err(err) => {
                    return Err(From::from(suggest_pcre2(err.to_string())));
                }
            };
            Ok(PatternMatcher::RustRegex(matcher))
        }
    }

    /// Return the matcher that should be used for searching.
    ///
    /// If there was a problem building the matcher (e.g., a syntax error),
    /// then this returns an error.
    #[cfg(not(feature = "pcre2"))]
    fn matcher(&self, patterns: &[String]) -> Result<PatternMatcher> {
        if self.is_present("pcre2") {
            return Err(From::from(
                "PCRE2 is not available in this build of ripgrep",
            ));
        }
        let matcher = self.matcher_rust(patterns)?;
        Ok(PatternMatcher::RustRegex(matcher))
    }

    /// Build a matcher using Rust's regex engine.
    ///
    /// If there was a problem building the matcher (such as a regex syntax
    /// error), then an error is returned.
    fn matcher_rust(&self, patterns: &[String]) -> Result<RustRegexMatcher> {
        let mut builder = RustRegexMatcherBuilder::new();
        builder
            .case_smart(self.case_smart())
            .case_insensitive(self.case_insensitive())
            .multi_line(true)
            .unicode(true)
            .octal(false)
            .word(self.is_present("word-regexp"));
        if self.is_present("multiline") {
            builder.dot_matches_new_line(self.is_present("multiline-dotall"));
            if self.is_present("crlf") {
                builder
                    .crlf(true)
                    .line_terminator(None);
            }
        } else {
            builder
                .line_terminator(Some(b'\n'))
                .dot_matches_new_line(false);
            if self.is_present("crlf") {
                builder.crlf(true);
            }
            // We don't need to set this in multiline mode since mulitline
            // matchers don't use optimizations related to line terminators.
            // Moreover, a mulitline regex used with --null-data should
            // be allowed to match NUL bytes explicitly, which this would
            // otherwise forbid.
            if self.is_present("null-data") {
                builder.line_terminator(Some(b'\x00'));
            }
        }
        if let Some(limit) = self.regex_size_limit()? {
            builder.size_limit(limit);
        }
        if let Some(limit) = self.dfa_size_limit()? {
            builder.dfa_size_limit(limit);
        }
        Ok(builder.build(&patterns.join("|"))?)
    }

    /// Build a matcher using PCRE2.
    ///
    /// If there was a problem building the matcher (such as a regex syntax
    /// error), then an error is returned.
    #[cfg(feature = "pcre2")]
    fn matcher_pcre2(&self, patterns: &[String]) -> Result<PCRE2RegexMatcher> {
        let mut builder = PCRE2RegexMatcherBuilder::new();
        builder
            .case_smart(self.case_smart())
            .caseless(self.case_insensitive())
            .multi_line(true)
            .word(self.is_present("word-regexp"));
        // For whatever reason, the JIT craps out during compilation with a
        // "no more memory" error on 32 bit systems. So don't use it there.
        if !cfg!(target_pointer_width = "32") {
            builder.jit(true);
        }
        if self.pcre2_unicode() {
            builder.utf(true).ucp(true);
            if self.encoding()?.is_some() {
                // SAFETY: If an encoding was specified, then we're guaranteed
                // to get valid UTF-8, so we can disable PCRE2's UTF checking.
                // (Feeding invalid UTF-8 to PCRE2 is UB.)
                unsafe {
                    builder.disable_utf_check();
                }
            }
        }
        if self.is_present("multiline") {
            builder.dotall(self.is_present("multiline-dotall"));
        }
        if self.is_present("crlf") {
            builder.crlf(true);
        }
        Ok(builder.build(&patterns.join("|"))?)
    }

    /// Build a JSON printer that writes results to the given writer.
    fn printer_json<W: io::Write>(&self, wtr: W) -> Result<JSON<W>> {
        let mut builder = JSONBuilder::new();
        builder
            .pretty(false)
            .max_matches(self.max_count()?)
            .always_begin_end(false);
        Ok(builder.build(wtr))
    }

    /// Build a Standard printer that writes results to the given writer.
    ///
    /// The given paths are used to configure aspects of the printer.
    ///
    /// If `separator_search` is true, then the returned printer will assume
    /// the responsibility of printing a separator between each set of
    /// search results, when appropriate (e.g., when contexts are enabled).
    /// When it's set to false, the caller is responsible for handling
    /// separators.
    ///
    /// In practice, we want the printer to handle it in the single threaded
    /// case but not in the multi-threaded case.
    fn printer_standard<W: WriteColor>(
        &self,
        paths: &[PathBuf],
        wtr: W,
        separator_search: bool,
    ) -> Result<Standard<W>> {
        let mut builder = StandardBuilder::new();
        builder
            .color_specs(self.color_specs()?)
            .stats(self.stats())
            .heading(self.heading())
            .path(self.with_filename(paths))
            .only_matching(self.is_present("only-matching"))
            .per_match(self.is_present("vimgrep"))
            .replacement(self.replacement())
            .max_columns(self.max_columns()?)
            .max_matches(self.max_count()?)
            .column(self.column())
            .byte_offset(self.is_present("byte-offset"))
            .trim_ascii(self.is_present("trim"))
            .separator_search(None)
            .separator_context(Some(self.context_separator()))
            .separator_field_match(b":".to_vec())
            .separator_field_context(b"-".to_vec())
            .separator_path(self.path_separator()?)
            .path_terminator(self.path_terminator());
        if separator_search {
            builder.separator_search(self.file_separator()?);
        }
        Ok(builder.build(wtr))
    }

    /// Build a Summary printer that writes results to the given writer.
    ///
    /// The given paths are used to configure aspects of the printer.
    ///
    /// This panics if the output format is not `OutputKind::Summary`.
    fn printer_summary<W: WriteColor>(
        &self,
        paths: &[PathBuf],
        wtr: W,
    ) -> Result<Summary<W>> {
        let mut builder = SummaryBuilder::new();
        builder
            .kind(self.summary_kind().expect("summary format"))
            .color_specs(self.color_specs()?)
            .stats(self.stats())
            .path(self.with_filename(paths))
            .max_matches(self.max_count()?)
            .separator_field(b":".to_vec())
            .separator_path(self.path_separator()?)
            .path_terminator(self.path_terminator());
        Ok(builder.build(wtr))
    }

    /// Build a searcher from the command line parameters.
    fn searcher(&self, paths: &[PathBuf]) -> Result<Searcher> {
        let (ctx_before, ctx_after) = self.contexts()?;
        let line_term =
            if self.is_present("crlf") {
                LineTerminator::crlf()
            } else if self.is_present("null-data") {
                LineTerminator::byte(b'\x00')
            } else {
                LineTerminator::byte(b'\n')
            };
        let mut builder = SearcherBuilder::new();
        builder
            .line_terminator(line_term)
            .invert_match(self.is_present("invert-match"))
            .line_number(self.line_number(paths))
            .multi_line(self.is_present("multiline"))
            .before_context(ctx_before)
            .after_context(ctx_after)
            .passthru(self.is_present("passthru"))
            .memory_map(self.mmap_choice(paths))
            .binary_detection(self.binary_detection())
            .encoding(self.encoding()?);
        Ok(builder.build())
    }

    /// Return a builder for recursively traversing a directory while
    /// respecting ignore rules.
    ///
    /// If there was a problem parsing the CLI arguments necessary for
    /// constructing the builder, then this returns an error.
    fn walker_builder(&self, paths: &[PathBuf]) -> Result<WalkBuilder> {
        let mut builder = WalkBuilder::new(&paths[0]);
        for path in &paths[1..] {
            builder.add(path);
        }
        for path in self.ignore_paths() {
            if let Some(err) = builder.add_ignore(path) {
                ignore_message!("{}", err);
            }
        }
        builder
            .max_depth(self.usize_of("max-depth")?)
            .follow_links(self.is_present("follow"))
            .max_filesize(self.max_file_size()?)
            .threads(self.threads()?)
            .overrides(self.overrides()?)
            .types(self.types()?)
            .hidden(!self.hidden())
            .parents(!self.no_ignore_parent())
            .ignore(!self.no_ignore())
            .git_global(
                !self.no_ignore()
                && !self.no_ignore_vcs()
                && !self.no_ignore_global())
            .git_ignore(!self.no_ignore() && !self.no_ignore_vcs())
            .git_exclude(!self.no_ignore() && !self.no_ignore_vcs());
        if !self.no_ignore() {
            builder.add_custom_ignore_filename(".rgignore");
        }
        if self.is_present("sort-files") {
            builder.sort_by_file_name(|a, b| a.cmp(b));
        }
        Ok(builder)
    }
}

/// Mid level routines for converting command line arguments into various types
/// of data structures.
///
/// Methods are sorted alphabetically.
impl ArgMatches {
    /// Returns the form of binary detection to perform.
    fn binary_detection(&self) -> BinaryDetection {
        let none =
            self.is_present("text")
            || self.unrestricted_count() >= 3
            || self.is_present("null-data");
        if none {
            BinaryDetection::none()
        } else {
            BinaryDetection::quit(b'\x00')
        }
    }

    /// Returns true if the command line configuration implies that a match
    /// can never be shown.
    fn can_never_match(&self, patterns: &[String]) -> bool {
        patterns.is_empty() || self.max_count().ok() == Some(Some(0))
    }

    /// Returns true if and only if case should be ignore.
    ///
    /// If --case-sensitive is present, then case is never ignored, even if
    /// --ignore-case is present.
    fn case_insensitive(&self) -> bool {
        self.is_present("ignore-case") && !self.is_present("case-sensitive")
    }

    /// Returns true if and only if smart case has been enabled.
    ///
    /// If either --ignore-case of --case-sensitive are present, then smart
    /// case is disabled.
    fn case_smart(&self) -> bool {
        self.is_present("smart-case")
        && !self.is_present("ignore-case")
        && !self.is_present("case-sensitive")
    }

    /// Returns the user's color choice based on command line parameters and
    /// environment.
    fn color_choice(&self) -> ColorChoice {
        let preference = match self.value_of_lossy("color") {
            None => "auto".to_string(),
            Some(v) => v,
        };
        if preference == "always" {
            ColorChoice::Always
        } else if preference == "ansi" {
            ColorChoice::AlwaysAnsi
        } else if preference == "auto" {
            if atty::is(atty::Stream::Stdout) || self.is_present("pretty") {
                ColorChoice::Auto
            } else {
                ColorChoice::Never
            }
        } else {
            ColorChoice::Never
        }
    }

    /// Returns the color specifications given by the user on the CLI.
    ///
    /// If the was a problem parsing any of the provided specs, then an error
    /// is returned.
    fn color_specs(&self) -> Result<ColorSpecs> {
        // Start with a default set of color specs.
        let mut specs = vec![
            #[cfg(unix)]
            "path:fg:magenta".parse().unwrap(),
            #[cfg(windows)]
            "path:fg:cyan".parse().unwrap(),
            "line:fg:green".parse().unwrap(),
            "match:fg:red".parse().unwrap(),
            "match:style:bold".parse().unwrap(),
        ];
        for spec_str in self.values_of_lossy_vec("colors") {
            specs.push(spec_str.parse()?);
        }
        Ok(ColorSpecs::new(&specs))
    }

    /// Returns true if and only if column numbers should be shown.
    fn column(&self) -> bool {
        if self.is_present("no-column") {
            return false;
        }
        self.is_present("column") || self.is_present("vimgrep")
    }

    /// Returns the before and after contexts from the command line.
    ///
    /// If a context setting was absent, then `0` is returned.
    ///
    /// If there was a problem parsing the values from the user as an integer,
    /// then an error is returned.
    fn contexts(&self) -> Result<(usize, usize)> {
        let after = self.usize_of("after-context")?.unwrap_or(0);
        let before = self.usize_of("before-context")?.unwrap_or(0);
        let both = self.usize_of("context")?.unwrap_or(0);
        Ok(if both > 0 {
            (both, both)
        } else {
            (before, after)
        })
    }

    /// Returns the unescaped context separator in UTF-8 bytes.
    ///
    /// If one was not provided, the default `--` is returned.
    fn context_separator(&self) -> Vec<u8> {
        match self.value_of_lossy("context-separator") {
            None => b"--".to_vec(),
            Some(sep) => unescape(&sep),
        }
    }

    /// Returns whether the -c/--count or the --count-matches flags were
    /// passed from the command line.
    ///
    /// If --count-matches and --invert-match were passed in, behave
    /// as if --count and --invert-match were passed in (i.e. rg will
    /// count inverted matches as per existing behavior).
    fn counts(&self) -> (bool, bool) {
        let count = self.is_present("count");
        let count_matches = self.is_present("count-matches");
        let invert_matches = self.is_present("invert-match");
        let only_matching = self.is_present("only-matching");
        if count_matches && invert_matches {
            // Treat `-v --count-matches` as `-v -c`.
            (true, false)
        } else if count && only_matching {
            // Treat `-c --only-matching` as `--count-matches`.
            (false, true)
        } else {
            (count, count_matches)
        }
    }

    /// Parse the dfa-size-limit argument option into a byte count.
    fn dfa_size_limit(&self) -> Result<Option<usize>> {
        let r = self.parse_human_readable_size("dfa-size-limit")?;
        u64_to_usize("dfa-size-limit", r)
    }

    /// Returns the type of encoding to use.
    ///
    /// This only returns an encoding if one is explicitly specified. When no
    /// encoding is present, the Searcher will still do BOM sniffing for UTF-16
    /// and transcode seamlessly.
    fn encoding(&self) -> Result<Option<Encoding>> {
        if self.is_present("no-encoding") {
            return Ok(None);
        }
        let label = match self.value_of_lossy("encoding") {
            None if self.pcre2_unicode() => "utf-8".to_string(),
            None => return Ok(None),
            Some(label) => label,
        };
        if label == "auto" {
            return Ok(None);
        }
        Ok(Some(Encoding::new(&label)?))
    }

    /// Return the file separator to use based on the CLI configuration.
    fn file_separator(&self) -> Result<Option<Vec<u8>>> {
        // File separators are only used for the standard grep-line format.
        if self.output_kind() != OutputKind::Standard {
            return Ok(None);
        }

        let (ctx_before, ctx_after) = self.contexts()?;
        Ok(if self.heading() {
            Some(b"".to_vec())
        } else if ctx_before > 0 || ctx_after > 0 {
            Some(self.context_separator().clone())
        } else {
            None
        })
    }

    /// Returns true if and only if matches should be grouped with file name
    /// headings.
    fn heading(&self) -> bool {
        if self.is_present("no-heading") || self.is_present("vimgrep") {
            false
        } else {
            atty::is(atty::Stream::Stdout)
            || self.is_present("heading")
            || self.is_present("pretty")
        }
    }

    /// Returns true if and only if hidden files/directories should be
    /// searched.
    fn hidden(&self) -> bool {
        self.is_present("hidden") || self.unrestricted_count() >= 2
    }

    /// Return all of the ignore file paths given on the command line.
    fn ignore_paths(&self) -> Vec<PathBuf> {
        let paths = match self.values_of_os("ignore-file") {
            None => return vec![],
            Some(paths) => paths,
        };
        paths.map(|p| Path::new(p).to_path_buf()).collect()
    }

    /// Returns true if and only if ripgrep is invoked in a way where it knows
    /// it search exactly one thing.
    fn is_one_search(&self, paths: &[PathBuf]) -> bool {
        if paths.len() != 1 {
            return false;
        }
        self.is_only_stdin(paths) || paths[0].is_file()
    }

    /// Returns true if and only if we're only searching a single thing and
    /// that thing is stdin.
    fn is_only_stdin(&self, paths: &[PathBuf]) -> bool {
        paths == [Path::new("-")]
    }

    /// Returns true if and only if we should show line numbers.
    fn line_number(&self, paths: &[PathBuf]) -> bool {
        if self.output_kind() == OutputKind::Summary {
            return false;
        }
        if self.is_present("no-line-number") {
            return false;
        }
        if self.output_kind() == OutputKind::JSON {
            return true;
        }

        // A few things can imply counting line numbers. In particular, we
        // generally want to show line numbers by default when printing to a
        // tty for human consumption, except for one interesting case: when
        // we're only searching stdin. This makes pipelines work as expected.
        (atty::is(atty::Stream::Stdout) && !self.is_only_stdin(paths))
        || self.is_present("line-number")
        || self.is_present("column")
        || self.is_present("pretty")
        || self.is_present("vimgrep")
    }

    /// The maximum number of columns allowed on each line.
    ///
    /// If `0` is provided, then this returns `None`.
    fn max_columns(&self) -> Result<Option<u64>> {
        Ok(self.usize_of_nonzero("max-columns")?.map(|n| n as u64))
    }

    /// The maximum number of matches permitted.
    fn max_count(&self) -> Result<Option<u64>> {
        Ok(self.usize_of("max-count")?.map(|n| n as u64))
    }

    /// Parses the max-filesize argument option into a byte count.
    fn max_file_size(&self) -> Result<Option<u64>> {
        self.parse_human_readable_size("max-filesize")
    }

    /// Returns whether we should attempt to use memory maps or not.
    fn mmap_choice(&self, paths: &[PathBuf]) -> MmapChoice {
        // SAFETY: Memory maps are difficult to impossible to encapsulate
        // safely in a portable way that doesn't simultaneously negate some of
        // the benfits of using memory maps. For ripgrep's use, we never mutate
        // a memory map and generally never store the contents of memory map
        // in a data structure that depends on immutability. Generally
        // speaking, the worst thing that can happen is a SIGBUS (if the
        // underlying file is truncated while reading it), which will cause
        // ripgrep to abort. This reasoning should be treated as suspect.
        let maybe = unsafe { MmapChoice::auto() };
        let never = MmapChoice::never();
        if self.is_present("no-mmap") {
            never
        } else if self.is_present("mmap") {
            maybe
        } else if paths.len() <= 10 && paths.iter().all(|p| p.is_file()) {
            // If we're only searching a few paths and all of them are
            // files, then memory maps are probably faster.
            maybe
        } else {
            never
        }
    }

    /// Returns true if ignore files should be ignored.
    fn no_ignore(&self) -> bool {
        self.is_present("no-ignore") || self.unrestricted_count() >= 1
    }

    /// Returns true if global ignore files should be ignored.
    fn no_ignore_global(&self) -> bool {
        self.is_present("no-ignore-global") || self.no_ignore()
    }

    /// Returns true if parent ignore files should be ignored.
    fn no_ignore_parent(&self) -> bool {
        self.is_present("no-ignore-parent") || self.no_ignore()
    }

    /// Returns true if VCS ignore files should be ignored.
    fn no_ignore_vcs(&self) -> bool {
        self.is_present("no-ignore-vcs") || self.no_ignore()
    }

    /// Determine the type of output we should produce.
    fn output_kind(&self) -> OutputKind {
        if self.is_present("quiet") {
            // While we don't technically print results (or aggregate results)
            // in quiet mode, we still support the --stats flag, and those
            // stats are computed by the Summary printer for now.
            return OutputKind::Summary;
        } else if self.is_present("json") {
            return OutputKind::JSON;
        }

        let (count, count_matches) = self.counts();
        let summary =
            count
            || count_matches
            || self.is_present("files-with-matches")
            || self.is_present("files-without-match");
        if summary {
            OutputKind::Summary
        } else {
            OutputKind::Standard
        }
    }

    /// Builds the set of glob overrides from the command line flags.
    fn overrides(&self) -> Result<Override> {
        let mut builder = OverrideBuilder::new(env::current_dir()?);
        for glob in self.values_of_lossy_vec("glob") {
            builder.add(&glob)?;
        }
        // This only enables case insensitivity for subsequent globs.
        builder.case_insensitive(true)?;
        for glob in self.values_of_lossy_vec("iglob") {
            builder.add(&glob)?;
        }
        Ok(builder.build()?)
    }

    /// Return all file paths that ripgrep should search.
    ///
    /// If no paths were given, then this returns an empty list.
    fn paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = match self.values_of_os("path") {
            None => vec![],
            Some(paths) => paths.map(|p| Path::new(p).to_path_buf()).collect(),
        };
        // If --file, --files or --regexp is given, then the first path is
        // always in `pattern`.
        if self.is_present("file")
            || self.is_present("files")
            || self.is_present("regexp")
        {
            if let Some(path) = self.value_of_os("pattern") {
                paths.insert(0, Path::new(path).to_path_buf());
            }
        }
        paths
    }

    /// Return the default path that ripgrep should search. This should only
    /// be used when ripgrep is not otherwise given at least one file path
    /// as a positional argument.
    fn path_default(&self) -> PathBuf {
        let file_is_stdin = self.values_of_os("file")
            .map_or(false, |mut files| files.any(|f| f == "-"));
        let search_cwd =
            atty::is(atty::Stream::Stdin)
            || !stdin_is_readable()
            || (self.is_present("file") && file_is_stdin)
            || self.is_present("files")
            || self.is_present("type-list");
        if search_cwd {
            Path::new("./").to_path_buf()
        } else {
            Path::new("-").to_path_buf()
        }
    }

    /// Returns the unescaped path separator as a single byte, if one exists.
    ///
    /// If the provided path separator is more than a single byte, then an
    /// error is returned.
    fn path_separator(&self) -> Result<Option<u8>> {
        let sep = match self.value_of_lossy("path-separator") {
            None => return Ok(None),
            Some(sep) => unescape(&sep),
        };
        if sep.is_empty() {
            Ok(None)
        } else if sep.len() > 1 {
            Err(From::from(format!(
                "A path separator must be exactly one byte, but \
                 the given separator is {} bytes: {}\n\
                 In some shells on Windows '/' is automatically \
                 expanded. Use '//' instead.",
                 sep.len(),
                 escape(&sep),
            )))
        } else {
            Ok(Some(sep[0]))
        }
    }

    /// Returns the byte that should be used to terminate paths.
    ///
    /// Typically, this is only set to `\x00` when the --null flag is provided,
    /// and `None` otherwise.
    fn path_terminator(&self) -> Option<u8> {
        if self.is_present("null") {
            Some(b'\x00')
        } else {
            None
        }
    }

    /// Get a sequence of all available patterns from the command line.
    /// This includes reading the -e/--regexp and -f/--file flags.
    ///
    /// Note that if -F/--fixed-strings is set, then all patterns will be
    /// escaped. If -x/--line-regexp is set, then all patterns are surrounded
    /// by `^...$`. Other things, such as --word-regexp, are handled by the
    /// regex matcher itself.
    ///
    /// If any pattern is invalid UTF-8, then an error is returned.
    fn patterns(&self) -> Result<Vec<String>> {
        if self.is_present("files") || self.is_present("type-list") {
            return Ok(vec![]);
        }
        let mut pats = vec![];
        match self.values_of_os("regexp") {
            None => {
                if self.values_of_os("file").is_none() {
                    if let Some(os_pat) = self.value_of_os("pattern") {
                        pats.push(self.pattern_from_os_str(os_pat)?);
                    }
                }
            }
            Some(os_pats) => {
                for os_pat in os_pats {
                    pats.push(self.pattern_from_os_str(os_pat)?);
                }
            }
        }
        if let Some(files) = self.values_of_os("file") {
            for file in files {
                if file == "-" {
                    let stdin = io::stdin();
                    for line in stdin.lock().lines() {
                        pats.push(self.pattern_from_str(&line?));
                    }
                } else {
                    let f = File::open(file)?;
                    for line in io::BufReader::new(f).lines() {
                        pats.push(self.pattern_from_str(&line?));
                    }
                }
            }
        }
        Ok(pats)
    }

    /// Returns a pattern that is guaranteed to produce an empty regular
    /// expression that is valid in any position.
    fn pattern_empty(&self) -> String {
        // This would normally just be an empty string, which works on its
        // own, but if the patterns are joined in a set of alternations, then
        // you wind up with `foo|`, which is currently invalid in Rust's regex
        // engine.
        "(?:z{0})*".to_string()
    }

    /// Converts an OsStr pattern to a String pattern. The pattern is escaped
    /// if -F/--fixed-strings is set.
    ///
    /// If the pattern is not valid UTF-8, then an error is returned.
    fn pattern_from_os_str(&self, pat: &OsStr) -> Result<String> {
        let s = pattern_to_str(pat)?;
        Ok(self.pattern_from_str(s))
    }

    /// Converts a &str pattern to a String pattern. The pattern is escaped
    /// if -F/--fixed-strings is set.
    fn pattern_from_str(&self, pat: &str) -> String {
        let litpat = self.pattern_literal(pat.to_string());
        let s = self.pattern_line(litpat);

        if s.is_empty() {
            self.pattern_empty()
        } else {
            s
        }
    }

    /// Returns the given pattern as a line pattern if the -x/--line-regexp
    /// flag is set. Otherwise, the pattern is returned unchanged.
    fn pattern_line(&self, pat: String) -> String {
        if self.is_present("line-regexp") {
            format!(r"^(?:{})$", pat)
        } else {
            pat
        }
    }

    /// Returns the given pattern as a literal pattern if the
    /// -F/--fixed-strings flag is set. Otherwise, the pattern is returned
    /// unchanged.
    fn pattern_literal(&self, pat: String) -> String {
        if self.is_present("fixed-strings") {
            regex::escape(&pat)
        } else {
            pat
        }
    }

    /// Returns the preprocessor command if one was specified.
    fn preprocessor(&self) -> Option<PathBuf> {
        let path = match self.value_of_os("pre") {
            None => return None,
            Some(path) => path,
        };
        if path.is_empty() {
            return None;
        }
        Some(Path::new(path).to_path_buf())
    }

    /// Parse the regex-size-limit argument option into a byte count.
    fn regex_size_limit(&self) -> Result<Option<usize>> {
        let r = self.parse_human_readable_size("regex-size-limit")?;
        u64_to_usize("regex-size-limit", r)
    }

    /// Returns the replacement string as UTF-8 bytes if it exists.
    fn replacement(&self) -> Option<Vec<u8>> {
        self.value_of_lossy("replace").map(|s| s.into_bytes())
    }

    /// Returns true if and only if aggregate statistics for a search should
    /// be tracked.
    ///
    /// Generally, this is only enabled when explicitly requested by in the
    /// command line arguments via the --stats flag, but this can also be
    /// enabled implicity via the output format, e.g., for JSON Lines.
    fn stats(&self) -> bool {
        self.output_kind() == OutputKind::JSON || self.is_present("stats")
    }

    /// Returns a handle to stdout for filtering search.
    ///
    /// A handle is returned if and only if ripgrep's stdout is being
    /// redirected to a file. The handle returned corresponds to that file.
    ///
    /// This can be used to ensure that we do not attempt to search a file
    /// that ripgrep is writing to.
    fn stdout_handle(&self) -> Option<Handle> {
        let h = match Handle::stdout() {
            Err(_) => return None,
            Ok(h) => h,
        };
        let md = match h.as_file().metadata() {
            Err(_) => return None,
            Ok(md) => md,
        };
        if !md.is_file() {
            return None;
        }
        Some(h)
    }

    /// When the output format is `Summary`, this returns the type of summary
    /// output to show.
    ///
    /// This returns `None` if the output format is not `Summary`.
    fn summary_kind(&self) -> Option<SummaryKind> {
        let (count, count_matches) = self.counts();
        if self.is_present("quiet") {
            Some(SummaryKind::Quiet)
        } else if count_matches {
            Some(SummaryKind::CountMatches)
        } else if count {
            Some(SummaryKind::Count)
        } else if self.is_present("files-with-matches") {
            Some(SummaryKind::PathWithMatch)
        } else if self.is_present("files-without-match") {
            Some(SummaryKind::PathWithoutMatch)
        } else {
            None
        }
    }

    /// Return the number of threads that should be used for parallelism.
    fn threads(&self) -> Result<usize> {
        if self.is_present("sort-files") {
            return Ok(1);
        }
        let threads = self.usize_of("threads")?.unwrap_or(0);
        Ok(if threads == 0 {
            cmp::min(12, num_cpus::get())
        } else {
            threads
        })
    }

    /// Builds a file type matcher from the command line flags.
    fn types(&self) -> Result<Types> {
        let mut builder = TypesBuilder::new();
        builder.add_defaults();
        for ty in self.values_of_lossy_vec("type-clear") {
            builder.clear(&ty);
        }
        for def in self.values_of_lossy_vec("type-add") {
            builder.add_def(&def)?;
        }
        for ty in self.values_of_lossy_vec("type") {
            builder.select(&ty);
        }
        for ty in self.values_of_lossy_vec("type-not") {
            builder.negate(&ty);
        }
        builder.build().map_err(From::from)
    }

    /// Returns the number of times the `unrestricted` flag is provided.
    fn unrestricted_count(&self) -> u64 {
        self.occurrences_of("unrestricted")
    }

    /// Returns true if and only if PCRE2's Unicode mode should be enabled.
    fn pcre2_unicode(&self) -> bool {
        // PCRE2 Unicode is enabled by default, so only disable it when told
        // to do so explicitly.
        self.is_present("pcre2") && !self.is_present("no-pcre2-unicode")
    }

    /// Returns true if and only if file names containing each match should
    /// be emitted.
    fn with_filename(&self, paths: &[PathBuf]) -> bool {
        if self.is_present("no-filename") {
            false
        } else {
            self.is_present("with-filename")
            || self.is_present("vimgrep")
            || paths.len() > 1
            || paths.get(0).map_or(false, |p| p.is_dir())
        }
    }
}

/// Lower level generic helper methods for teasing values out of clap.
impl ArgMatches {
    /// Like values_of_lossy, but returns an empty vec if the flag is not
    /// present.
    fn values_of_lossy_vec(&self, name: &str) -> Vec<String> {
        self.values_of_lossy(name).unwrap_or_else(Vec::new)
    }

    /// Safely reads an arg value with the given name, and if it's present,
    /// tries to parse it as a usize value.
    ///
    /// If the number is zero, then it is considered absent and `None` is
    /// returned.
    fn usize_of_nonzero(&self, name: &str) -> Result<Option<usize>> {
        let n = match self.usize_of(name)? {
            None => return Ok(None),
            Some(n) => n,
        };
        Ok(if n == 0 {
            None
        } else {
            Some(n)
        })
    }

    /// Safely reads an arg value with the given name, and if it's present,
    /// tries to parse it as a usize value.
    fn usize_of(&self, name: &str) -> Result<Option<usize>> {
        match self.value_of_lossy(name) {
            None => Ok(None),
            Some(v) => v.parse().map(Some).map_err(From::from),
        }
    }

    /// Parses an argument of the form `[0-9]+(KMG)?`.
    ///
    /// If the aforementioned format is not recognized, then this returns an
    /// error.
    fn parse_human_readable_size(
        &self,
        arg_name: &str,
    ) -> Result<Option<u64>> {
        lazy_static! {
            static ref RE: Regex = Regex::new(r"^([0-9]+)([KMG])?$").unwrap();
        }

        let arg_value = match self.value_of_lossy(arg_name) {
            Some(x) => x,
            None => return Ok(None)
        };
        let caps = RE
            .captures(&arg_value)
            .ok_or_else(|| {
                format!("invalid format for {}", arg_name)
            })?;

        let value = caps[1].parse::<u64>()?;
        let suffix = caps.get(2).map(|x| x.as_str());

        let v_10 = value.checked_mul(1024);
        let v_20 = v_10.and_then(|x| x.checked_mul(1024));
        let v_30 = v_20.and_then(|x| x.checked_mul(1024));
        let try_suffix = |x: Option<u64>| {
            if x.is_some() {
                Ok(x)
            } else {
                Err(From::from(format!("number too large for {}", arg_name)))
            }
        };
        match suffix {
            None => Ok(Some(value)),
            Some("K") => try_suffix(v_10),
            Some("M") => try_suffix(v_20),
            Some("G") => try_suffix(v_30),
            _ => Err(From::from(format!("invalid suffix for {}", arg_name)))
        }
    }
}

/// The following methods mostly dispatch to the underlying clap methods
/// directly. Methods that would otherwise get a single value will fetch all
/// values and return the last one. (Clap returns the first one.) We only
/// define the ones we need.
impl ArgMatches {
    fn is_present(&self, name: &str) -> bool {
        self.0.is_present(name)
    }

    fn occurrences_of(&self, name: &str) -> u64 {
        self.0.occurrences_of(name)
    }

    fn value_of_lossy(&self, name: &str) -> Option<String> {
        self.0.value_of_lossy(name).map(|s| s.into_owned())
    }

    fn values_of_lossy(&self, name: &str) -> Option<Vec<String>> {
        self.0.values_of_lossy(name)
    }

    fn value_of_os(&self, name: &str) -> Option<&OsStr> {
        self.0.value_of_os(name)
    }

    fn values_of_os(&self, name: &str) -> Option<clap::OsValues> {
        self.0.values_of_os(name)
    }
}

/// Convert an OsStr to a Unicode string.
///
/// Patterns _must_ be valid UTF-8, so if the given OsStr isn't valid UTF-8,
/// this returns an error.
fn pattern_to_str(s: &OsStr) -> Result<&str> {
    s.to_str().ok_or_else(|| {
        From::from(format!(
            "Argument '{}' is not valid UTF-8. \
             Use hex escape sequences to match arbitrary \
             bytes in a pattern (e.g., \\xFF).",
             s.to_string_lossy()
        ))
    })
}

/// Inspect an error resulting from building a Rust regex matcher, and if it's
/// believed to correspond to a syntax error that PCRE2 could handle, then
/// add a message to suggest the use of -P/--pcre2.
#[cfg(feature = "pcre2")]
fn suggest_pcre2(msg: String) -> String {
    if !msg.contains("backreferences") && !msg.contains("look-around") {
        msg
    } else {
        format!("{}

Consider enabling PCRE2 with the --pcre2 flag, which can handle backreferences
and look-around.", msg)
    }
}

/// Convert the result of parsing a human readable file size to a `usize`,
/// failing if the type does not fit.
fn u64_to_usize(
    arg_name: &str,
    value: Option<u64>,
) -> Result<Option<usize>> {
    use std::usize;

    let value = match value {
        None => return Ok(None),
        Some(value) => value,
    };
    if value <= usize::MAX as u64 {
        Ok(Some(value as usize))
    } else {
        Err(From::from(format!("number too large for {}", arg_name)))
    }
}

/// Returns true if and only if stdin is deemed searchable.
#[cfg(unix)]
fn stdin_is_readable() -> bool {
    use std::os::unix::fs::FileTypeExt;

    let ft = match Handle::stdin().and_then(|h| h.as_file().metadata()) {
        Err(_) => return false,
        Ok(md) => md.file_type(),
    };
    ft.is_file() || ft.is_fifo()
}

/// Returns true if and only if stdin is deemed searchable.
#[cfg(windows)]
fn stdin_is_readable() -> bool {
    use std::os::windows::io::AsRawHandle;
    use winapi::um::fileapi::GetFileType;
    use winapi::um::winbase::{FILE_TYPE_DISK, FILE_TYPE_PIPE};

    let handle = match Handle::stdin() {
        Err(_) => return false,
        Ok(handle) => handle,
    };
    let raw_handle = handle.as_raw_handle();
    // SAFETY: As far as I can tell, it's not possible to use GetFileType in
    // a way that violates safety. We give it a handle and we get an integer.
    let ft = unsafe { GetFileType(raw_handle) };
    ft == FILE_TYPE_DISK || ft == FILE_TYPE_PIPE
}
