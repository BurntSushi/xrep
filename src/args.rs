use std::cmp;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, BufRead};
use std::ops;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap;
use encoding_rs::Encoding;
use env_logger;
use grep::{Grep, GrepBuilder};
use log;
use num_cpus;
use regex;
use same_file;
use termcolor;

use app;
use atty;
use ignore::overrides::{Override, OverrideBuilder};
use ignore::types::{FileTypeDef, Types, TypesBuilder};
use ignore;
use printer::{ColorSpecs, Printer};
use unescape::unescape;
use worker::{Worker, WorkerBuilder};

use Result;

/// `Args` are transformed/normalized from `ArgMatches`.
#[derive(Debug)]
pub struct Args {
    paths: Vec<PathBuf>,
    after_context: usize,
    before_context: usize,
    color_choice: termcolor::ColorChoice,
    colors: ColorSpecs,
    column: bool,
    context_separator: Vec<u8>,
    count: bool,
    encoding: Option<&'static Encoding>,
    files_with_matches: bool,
    files_without_matches: bool,
    eol: u8,
    files: bool,
    follow: bool,
    glob_overrides: Override,
    grep: Grep,
    heading: bool,
    hidden: bool,
    ignore_files: Vec<PathBuf>,
    invert_match: bool,
    line_number: bool,
    line_per_match: bool,
    max_columns: Option<usize>,
    max_count: Option<u64>,
    max_filesize: Option<u64>,
    maxdepth: Option<usize>,
    mmap: bool,
    no_ignore: bool,
    no_ignore_parent: bool,
    no_ignore_vcs: bool,
    no_messages: bool,
    null: bool,
    only_matching: bool,
    path_separator: Option<u8>,
    quiet: bool,
    quiet_matched: QuietMatched,
    replace: Option<Vec<u8>>,
    sort_files: bool,
    stdout_handle: Option<same_file::Handle>,
    text: bool,
    threads: usize,
    type_list: bool,
    types: Types,
    with_filename: bool,
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
        let matches = app::app().get_matches();

        let mut logb = env_logger::LogBuilder::new();
        if matches.is_present("debug") {
            logb.filter(None, log::LogLevelFilter::Debug);
        } else {
            logb.filter(None, log::LogLevelFilter::Warn);
        }
        if let Err(err) = logb.init() {
            errored!("failed to initialize logger: {}", err);
        }
        ArgMatches(matches).to_args()
    }

    /// Returns true if ripgrep should print the files it will search and exit
    /// (but not do any actual searching).
    pub fn files(&self) -> bool {
        self.files
    }

    /// Create a new line based matcher. The matcher returned can be used
    /// across multiple threads simultaneously. This matcher only supports
    /// basic searching of regular expressions in a single buffer.
    ///
    /// The pattern and other flags are taken from the command line.
    pub fn grep(&self) -> Grep {
        self.grep.clone()
    }

    /// Whether ripgrep should be quiet or not.
    pub fn quiet(&self) -> bool {
        self.quiet
    }

    /// Returns a thread safe boolean for determining whether to quit a search
    /// early when quiet mode is enabled.
    ///
    /// If quiet mode is disabled, then QuietMatched.has_match always returns
    /// false.
    pub fn quiet_matched(&self) -> QuietMatched {
        self.quiet_matched.clone()
    }

    /// Create a new printer of individual search results that writes to the
    /// writer given.
    pub fn printer<W: termcolor::WriteColor>(&self, wtr: W) -> Printer<W> {
        let mut p = Printer::new(wtr)
            .colors(self.colors.clone())
            .column(self.column)
            .context_separator(self.context_separator.clone())
            .eol(self.eol)
            .heading(self.heading)
            .line_per_match(self.line_per_match)
            .null(self.null)
            .only_matching(self.only_matching)
            .path_separator(self.path_separator)
            .with_filename(self.with_filename)
            .max_columns(self.max_columns);
        if let Some(ref rep) = self.replace {
            p = p.replace(rep.clone());
        }
        p
    }

    /// Retrieve the configured file separator.
    pub fn file_separator(&self) -> Option<Vec<u8>> {
        let use_heading_sep =
            self.heading
            && !self.count
            && !self.files_with_matches
            && !self.files_without_matches;
        if use_heading_sep {
            Some(b"".to_vec())
        } else if self.before_context > 0 || self.after_context > 0 {
            Some(self.context_separator.clone())
        } else {
            None
        }
    }

    /// Returns true if the given arguments are known to never produce a match.
    pub fn never_match(&self) -> bool {
        self.max_count == Some(0)
    }

    /// Create a new writer for single-threaded searching with color support.
    pub fn stdout(&self) -> termcolor::StandardStream {
        termcolor::StandardStream::stdout(self.color_choice)
    }

    /// Returns a handle to stdout for filtering search.
    ///
    /// A handle is returned if and only if ripgrep's stdout is being
    /// redirected to a file. The handle returned corresponds to that file.
    ///
    /// This can be used to ensure that we do not attempt to search a file
    /// that ripgrep is writing to.
    pub fn stdout_handle(&self) -> Option<&same_file::Handle> {
        self.stdout_handle.as_ref()
    }

    /// Create a new buffer writer for multi-threaded searching with color
    /// support.
    pub fn buffer_writer(&self) -> termcolor::BufferWriter {
        let mut wtr = termcolor::BufferWriter::stdout(self.color_choice);
        wtr.separator(self.file_separator());
        wtr
    }

    /// Return the paths that should be searched.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Returns true if there is exactly one file path given to search.
    pub fn is_one_path(&self) -> bool {
        self.paths.len() == 1
        && (self.paths[0] == Path::new("-") || self.paths[0].is_file())
    }

    /// Create a worker whose configuration is taken from the
    /// command line.
    pub fn worker(&self) -> Worker {
        WorkerBuilder::new(self.grep())
            .after_context(self.after_context)
            .before_context(self.before_context)
            .count(self.count)
            .encoding(self.encoding)
            .files_with_matches(self.files_with_matches)
            .files_without_matches(self.files_without_matches)
            .eol(self.eol)
            .line_number(self.line_number)
            .invert_match(self.invert_match)
            .max_count(self.max_count)
            .mmap(self.mmap)
            .no_messages(self.no_messages)
            .quiet(self.quiet)
            .text(self.text)
            .build()
    }

    /// Returns the number of worker search threads that should be used.
    pub fn threads(&self) -> usize {
        self.threads
    }

    /// Returns a list of type definitions currently loaded.
    pub fn type_defs(&self) -> &[FileTypeDef] {
        self.types.definitions()
    }

    /// Returns true if ripgrep should print the type definitions currently
    /// loaded and then exit.
    pub fn type_list(&self) -> bool {
        self.type_list
    }

    /// Returns true if error messages should be suppressed.
    pub fn no_messages(&self) -> bool {
        self.no_messages
    }

    /// Create a new recursive directory iterator over the paths in argv.
    pub fn walker(&self) -> ignore::Walk {
        self.walker_builder().build()
    }

    /// Create a new parallel recursive directory iterator over the paths
    /// in argv.
    pub fn walker_parallel(&self) -> ignore::WalkParallel {
        self.walker_builder().build_parallel()
    }

    fn walker_builder(&self) -> ignore::WalkBuilder {
        let paths = self.paths();
        let mut wd = ignore::WalkBuilder::new(&paths[0]);
        for path in &paths[1..] {
            wd.add(path);
        }
        for path in &self.ignore_files {
            if let Some(err) = wd.add_ignore(path) {
                if !self.no_messages {
                    eprintln!("{}", err);
                }
            }
        }

        wd.follow_links(self.follow);
        wd.hidden(!self.hidden);
        wd.max_depth(self.maxdepth);
        wd.max_filesize(self.max_filesize);
        wd.overrides(self.glob_overrides.clone());
        wd.types(self.types.clone());
        wd.git_global(!self.no_ignore && !self.no_ignore_vcs);
        wd.git_ignore(!self.no_ignore && !self.no_ignore_vcs);
        wd.git_exclude(!self.no_ignore && !self.no_ignore_vcs);
        wd.ignore(!self.no_ignore);
        wd.parents(!self.no_ignore_parent);
        wd.threads(self.threads());
        if self.sort_files {
            wd.sort_by(|a, b| a.cmp(b));
        }
        wd
    }
}

/// `ArgMatches` wraps `clap::ArgMatches` and provides semantic meaning to
/// several options/flags.
struct ArgMatches<'a>(clap::ArgMatches<'a>);

impl<'a> ops::Deref for ArgMatches<'a> {
    type Target = clap::ArgMatches<'a>;
    fn deref(&self) -> &clap::ArgMatches<'a> { &self.0 }
}

impl<'a> ArgMatches<'a> {
    /// Convert the result of parsing CLI arguments into ripgrep's
    /// configuration.
    fn to_args(&self) -> Result<Args> {
        let paths = self.paths();
        let line_number = self.line_number(&paths);
        let mmap = try!(self.mmap(&paths));
        let with_filename = self.with_filename(&paths);
        let (before_context, after_context) = try!(self.contexts());
        let quiet = self.is_present("quiet");
        let args = Args {
            paths: paths,
            after_context: after_context,
            before_context: before_context,
            color_choice: self.color_choice(),
            colors: try!(self.color_specs()),
            column: self.column(),
            context_separator: self.context_separator(),
            count: self.is_present("count"),
            encoding: try!(self.encoding()),
            files_with_matches: self.is_present("files-with-matches"),
            files_without_matches: self.is_present("files-without-match"),
            eol: b'\n',
            files: self.is_present("files"),
            follow: self.is_present("follow"),
            glob_overrides: try!(self.overrides()),
            grep: try!(self.grep()),
            heading: self.heading(),
            hidden: self.hidden(),
            ignore_files: self.ignore_files(),
            invert_match: self.is_present("invert-match"),
            line_number: line_number,
            line_per_match: self.is_present("vimgrep"),
            max_columns: try!(self.usize_of("max-columns")),
            max_count: try!(self.usize_of("max-count")).map(|max| max as u64),
            max_filesize: try!(self.max_filesize()),
            maxdepth: try!(self.usize_of("maxdepth")),
            mmap: mmap,
            no_ignore: self.no_ignore(),
            no_ignore_parent: self.no_ignore_parent(),
            no_ignore_vcs: self.no_ignore_vcs(),
            no_messages: self.is_present("no-messages"),
            null: self.is_present("null"),
            only_matching: self.is_present("only-matching"),
            path_separator: try!(self.path_separator()),
            quiet: quiet,
            quiet_matched: QuietMatched::new(quiet),
            replace: self.replace(),
            sort_files: self.is_present("sort-files"),
            stdout_handle: self.stdout_handle(),
            text: self.text(),
            threads: try!(self.threads()),
            type_list: self.is_present("type-list"),
            types: try!(self.types()),
            with_filename: with_filename,
        };
        if args.mmap {
            debug!("will try to use memory maps");
        }
        Ok(args)
    }

    /// Return all file paths that ripgrep should search.
    fn paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = match self.values_of_os("path") {
            None => vec![],
            Some(vals) => vals.map(|p| Path::new(p).to_path_buf()).collect(),
        };
        // If --file, --files or --regexp is given, then the first path is
        // always in `pattern`.
        if self.is_present("file")
            || self.is_present("files")
            || self.is_present("regexp") {
            if let Some(path) = self.value_of_os("PATTERN") {
                paths.insert(0, Path::new(path).to_path_buf());
            }
        }
        if paths.is_empty() {
            paths.push(self.default_path());
        }
        paths
    }

    /// Return the default path that ripgrep should search.
    fn default_path(&self) -> PathBuf {
        let file_is_stdin =
            self.values_of_os("file").map_or(false, |mut files| {
                files.any(|f| f == "-")
            });
        let search_cwd = atty::is(atty::Stream::Stdin)
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

    /// Return all of the ignore files given on the command line.
    fn ignore_files(&self) -> Vec<PathBuf> {
        match self.values_of_os("ignore-file") {
            None => return vec![],
            Some(vals) => vals.map(|p| Path::new(p).to_path_buf()).collect(),
        }
    }

    /// Return the pattern that should be used for searching.
    ///
    /// If multiple -e/--regexp flags are given, then they are all collapsed
    /// into one pattern.
    ///
    /// If any part of the pattern isn't valid UTF-8, then an error is
    /// returned.
    fn pattern(&self) -> Result<String> {
        Ok(try!(self.patterns()).join("|"))
    }

    /// Get a sequence of all available patterns from the command line.
    /// This includes reading the -e/--regexp and -f/--file flags.
    ///
    /// Note that if -F/--fixed-strings is set, then all patterns will be
    /// escaped. Similarly, if -w/--word-regexp is set, then all patterns
    /// are surrounded by `\b`.
    ///
    /// If any pattern is invalid UTF-8, then an error is returned.
    fn patterns(&self) -> Result<Vec<String>> {
        if self.is_present("files") || self.is_present("type-list") {
            return Ok(vec![self.empty_pattern()]);
        }
        let mut pats = vec![];
        match self.values_of_os("regexp") {
            None => {
                if self.values_of_os("file").is_none() {
                    if let Some(os_pat) = self.value_of_os("PATTERN") {
                        pats.push(try!(self.os_str_pattern(os_pat)));
                    }
                }
            }
            Some(os_pats) => {
                for os_pat in os_pats {
                    pats.push(try!(self.os_str_pattern(os_pat)));
                }
            }
        }
        if let Some(files) = self.values_of_os("file") {
            for file in files {
                if file == "-" {
                    let stdin = io::stdin();
                    for line in stdin.lock().lines() {
                        pats.push(self.str_pattern(&try!(line)));
                    }
                } else {
                    let f = try!(fs::File::open(file));
                    for line in io::BufReader::new(f).lines() {
                        pats.push(self.str_pattern(&try!(line)));
                    }
                }
            }
        }
        if pats.is_empty() {
            pats.push(self.empty_pattern())
        }
        Ok(pats)
    }

    /// Converts an OsStr pattern to a String pattern, including word
    /// boundaries or escapes if applicable.
    ///
    /// If the pattern is not valid UTF-8, then an error is returned.
    fn os_str_pattern(&self, pat: &OsStr) -> Result<String> {
        let s = try!(pattern_to_str(pat));
        Ok(self.str_pattern(s))
    }

    /// Converts a &str pattern to a String pattern, including word
    /// boundaries or escapes if applicable.
    fn str_pattern(&self, pat: &str) -> String {
        let s = self.word_pattern(self.literal_pattern(pat.to_string()));
        if s.is_empty() {
            self.empty_pattern()
        } else {
            s
        }
    }

    /// Returns the given pattern as a literal pattern if the
    /// -F/--fixed-strings flag is set. Otherwise, the pattern is returned
    /// unchanged.
    fn literal_pattern(&self, pat: String) -> String {
        if self.is_present("fixed-strings") {
            regex::escape(&pat)
        } else {
            pat
        }
    }

    /// Returns the given pattern as a word pattern if the -w/--word-regexp
    /// flag is set. Otherwise, the pattern is returned unchanged.
    fn word_pattern(&self, pat: String) -> String {
        if self.is_present("word-regexp") {
            format!(r"\b(?:{})\b", pat)
        } else {
            pat
        }
    }

    /// Empty pattern returns a pattern that is guaranteed to produce an empty
    /// regular expression that is valid in any position.
    fn empty_pattern(&self) -> String {
        // This would normally just be an empty string, which works on its
        // own, but if the patterns are joined in a set of alternations, then
        // you wind up with `foo|`, which is invalid.
        self.word_pattern("z{0}".to_string())
    }

    /// Returns true if and only if file names containing each match should
    /// be emitted.
    ///
    /// `paths` should be a slice of all top-level file paths that ripgrep
    /// will need to search.
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

    /// Returns a handle to stdout for filtering search.
    ///
    /// A handle is returned if and only if ripgrep's stdout is being
    /// redirected to a file. The handle returned corresponds to that file.
    ///
    /// This can be used to ensure that we do not attempt to search a file
    /// that ripgrep is writing to.
    fn stdout_handle(&self) -> Option<same_file::Handle> {
        let h = match same_file::Handle::stdout() {
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

    /// Returns true if and only if memory map searching should be tried.
    ///
    /// `paths` should be a slice of all top-level file paths that ripgrep
    /// will need to search.
    fn mmap(&self, paths: &[PathBuf]) -> Result<bool> {
        let (before, after) = try!(self.contexts());
        let enc = try!(self.encoding());
        Ok(if before > 0 || after > 0 || self.is_present("no-mmap") {
            false
        } else if self.is_present("mmap") {
            true
        } else if cfg!(target_os = "macos") {
            // On Mac, memory maps appear to suck. Neat.
            false
        } else if enc.is_some() {
            // There's no practical way to transcode a memory map that isn't
            // isomorphic to searching over io::Read.
            false
        } else {
            // If we're only searching a few paths and all of them are
            // files, then memory maps are probably faster.
            paths.len() <= 10 && paths.iter().all(|p| p.is_file())
        })
    }

    /// Returns true if and only if line numbers should be shown.
    fn line_number(&self, paths: &[PathBuf]) -> bool {
        if self.is_present("no-line-number") || self.is_present("count") {
            false
        } else {
            let only_stdin = paths == &[Path::new("-")];
            (atty::is(atty::Stream::Stdout) && !only_stdin)
            || self.is_present("line-number")
            || self.is_present("column")
            || self.is_present("pretty")
            || self.is_present("vimgrep")
        }
    }

    /// Returns true if and only if column numbers should be shown.
    fn column(&self) -> bool {
        self.is_present("column") || self.is_present("vimgrep")
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

    /// Returns the replacement string as UTF-8 bytes if it exists.
    fn replace(&self) -> Option<Vec<u8>> {
        self.value_of_lossy("replace").map(|s| s.into_owned().into_bytes())
    }

    /// Returns the unescaped context separator in UTF-8 bytes.
    fn context_separator(&self) -> Vec<u8> {
        match self.value_of_lossy("context-separator") {
            None => b"--".to_vec(),
            Some(sep) => unescape(&sep),
        }
    }

    /// Returns the unescaped path separator in UTF-8 bytes.
    fn path_separator(&self) -> Result<Option<u8>> {
        match self.value_of_lossy("path-separator") {
            None => Ok(None),
            Some(sep) => {
                let sep = unescape(&sep);
                if sep.is_empty() {
                    Ok(None)
                } else if sep.len() > 1 {
                    Err(From::from(format!(
                        "A path separator must be exactly one byte, but \
                         the given separator is {} bytes.", sep.len())))
                } else {
                    Ok(Some(sep[0]))
                }
            }
        }
    }

    /// Returns the before and after contexts from the command line.
    ///
    /// If a context setting was absent, then `0` is returned.
    ///
    /// If there was a problem parsing the values from the user as an integer,
    /// then an error is returned.
    fn contexts(&self) -> Result<(usize, usize)> {
        let after = try!(self.usize_of("after-context")).unwrap_or(0);
        let before = try!(self.usize_of("before-context")).unwrap_or(0);
        let both = try!(self.usize_of("context")).unwrap_or(0);
        Ok(if both > 0 {
            (both, both)
        } else {
            (before, after)
        })
    }

    /// Returns the user's color choice based on command line parameters and
    /// environment.
    fn color_choice(&self) -> termcolor::ColorChoice {
        let preference = match self.0.value_of_lossy("color") {
            None => "auto".to_string(),
            Some(v) => v.into_owned(),
        };
        if preference == "always" {
            termcolor::ColorChoice::Always
        } else if preference == "ansi" {
            termcolor::ColorChoice::AlwaysAnsi
        } else if preference == "auto" {
            if atty::is(atty::Stream::Stdout) || self.is_present("pretty") {
                termcolor::ColorChoice::Auto
            } else {
                termcolor::ColorChoice::Never
            }
        } else {
            termcolor::ColorChoice::Never
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
            specs.push(try!(spec_str.parse()));
        }
        Ok(ColorSpecs::new(&specs))
    }

    /// Return the text encoding specified.
    ///
    /// If the label given by the caller doesn't correspond to a valid
    /// supported encoding (and isn't `auto`), then return an error.
    ///
    /// A `None` encoding implies that the encoding should be automatically
    /// detected on a per-file basis.
    fn encoding(&self) -> Result<Option<&'static Encoding>> {
        match self.0.value_of_lossy("encoding") {
            None => Ok(None),
            Some(label) => {
                if label == "auto" {
                    return Ok(None);
                }
                match Encoding::for_label_no_replacement(label.as_bytes()) {
                    Some(enc) => Ok(Some(enc)),
                    None => Err(From::from(
                        format!("unsupported encoding: {}", label))),
                }
            }
        }
    }

    /// Returns the approximate number of threads that ripgrep should use.
    fn threads(&self) -> Result<usize> {
        if self.is_present("sort-files") {
            return Ok(1);
        }
        let threads = try!(self.usize_of("threads")).unwrap_or(0);
        Ok(if threads == 0 {
            cmp::min(12, num_cpus::get())
        } else {
            threads
        })
    }

    /// Builds a grep matcher from the command line flags.
    ///
    /// If there was a problem extracting the pattern from the command line
    /// flags, then an error is returned.
    fn grep(&self) -> Result<Grep> {
        let smart =
            self.is_present("smart-case")
            && !self.is_present("ignore-case")
            && !self.is_present("case-sensitive");
        let casei =
            self.is_present("ignore-case")
            && !self.is_present("case-sensitive");
        let mut gb = GrepBuilder::new(&try!(self.pattern()))
            .case_smart(smart)
            .case_insensitive(casei)
            .line_terminator(b'\n');

        if let Some(limit) = try!(self.dfa_size_limit()) {
            gb = gb.dfa_size_limit(limit);
        }
        if let Some(limit) = try!(self.regex_size_limit()) {
            gb = gb.size_limit(limit);
        }
        gb.build().map_err(From::from)
    }

    /// Builds the set of glob overrides from the command line flags.
    fn overrides(&self) -> Result<Override> {
        let mut ovr = OverrideBuilder::new(try!(env::current_dir()));
        for glob in self.values_of_lossy_vec("glob") {
            try!(ovr.add(&glob));
        }
        // this is smelly. In the long run it might make sense
        // to change overridebuilder to be like globsetbuilder
        // but this would be a breaking change to the ignore crate
        // so it is being shelved for now...
        try!(ovr.case_insensitive(true));
        for glob in self.values_of_lossy_vec("iglob") {
            try!(ovr.add(&glob));
        }
        ovr.build().map_err(From::from)
    }

    /// Builds a file type matcher from the command line flags.
    fn types(&self) -> Result<Types> {
        let mut btypes = TypesBuilder::new();
        btypes.add_defaults();
        for ty in self.values_of_lossy_vec("type-clear") {
            btypes.clear(&ty);
        }
        for def in self.values_of_lossy_vec("type-add") {
            try!(btypes.add_def(&def));
        }
        for ty in self.values_of_lossy_vec("type") {
            btypes.select(&ty);
        }
        for ty in self.values_of_lossy_vec("type-not") {
            btypes.negate(&ty);
        }
        btypes.build().map_err(From::from)
    }

    /// Parses an argument of the form `[0-9]+(KMG)?`.
    ///
    /// This always returns the result as a type `u64`. This must be converted
    /// to the appropriate type by the caller.
    fn parse_human_readable_size_arg(
        &self,
        arg_name: &str,
    ) -> Result<Option<u64>> {
        let arg_value = match self.value_of_lossy(arg_name) {
            Some(x) => x,
            None => return Ok(None)
        };
        let re = regex::Regex::new("^([0-9]+)([KMG])?$").unwrap();
        let caps = try!(
            re.captures(&arg_value).ok_or_else(|| {
                format!("invalid format for {}", arg_name)
            }));

        let value = try!(caps[1].parse::<u64>());
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
            None      => Ok(Some(value)),
            Some("K") => try_suffix(v_10),
            Some("M") => try_suffix(v_20),
            Some("G") => try_suffix(v_30),
            _ => Err(From::from(format!("invalid suffix for {}", arg_name)))
        }
    }

    /// Parse the dfa-size-limit argument option into a byte count.
    fn dfa_size_limit(&self) -> Result<Option<usize>> {
        let r = try!(self.parse_human_readable_size_arg("dfa-size-limit"));
        human_readable_to_usize("dfa-size-limit", r)
    }

    /// Parse the regex-size-limit argument option into a byte count.
    fn regex_size_limit(&self) -> Result<Option<usize>> {
        let r = try!(self.parse_human_readable_size_arg("regex-size-limit"));
        human_readable_to_usize("regex-size-limit", r)
    }

    /// Parses the max-filesize argument option into a byte count.
    fn max_filesize(&self) -> Result<Option<u64>> {
        self.parse_human_readable_size_arg("max-filesize")
    }

    /// Returns true if ignore files should be ignored.
    fn no_ignore(&self) -> bool {
        self.is_present("no-ignore")
        || self.occurrences_of("unrestricted") >= 1
    }

    /// Returns true if parent ignore files should be ignored.
    fn no_ignore_parent(&self) -> bool {
        self.is_present("no-ignore-parent") || self.no_ignore()
    }

    /// Returns true if VCS ignore files should be ignored.
    fn no_ignore_vcs(&self) -> bool {
        self.is_present("no-ignore-vcs") || self.no_ignore()
    }

    /// Returns true if and only if hidden files/directories should be
    /// searched.
    fn hidden(&self) -> bool {
        self.is_present("hidden") || self.occurrences_of("unrestricted") >= 2
    }

    /// Returns true if and only if all files should be treated as if they
    /// were text, even if ripgrep would detect it as a binary file.
    fn text(&self) -> bool {
        self.is_present("text") || self.occurrences_of("unrestricted") >= 3
    }

    /// Like values_of_lossy, but returns an empty vec if the flag is not
    /// present.
    fn values_of_lossy_vec(&self, name: &str) -> Vec<String> {
        self.values_of_lossy(name).unwrap_or_else(Vec::new)
    }

    /// Safely reads an arg value with the given name, and if it's present,
    /// tries to parse it as a usize value.
    fn usize_of(&self, name: &str) -> Result<Option<usize>> {
        match self.value_of_lossy(name) {
            None => Ok(None),
            Some(v) => v.parse().map(Some).map_err(From::from),
        }
    }
}

fn pattern_to_str(s: &OsStr) -> Result<&str> {
    match s.to_str() {
        Some(s) => Ok(s),
        None => Err(From::from(format!(
            "Argument '{}' is not valid UTF-8. \
             Use hex escape sequences to match arbitrary \
             bytes in a pattern (e.g., \\xFF).",
             s.to_string_lossy()))),
    }
}

/// A simple thread safe abstraction for determining whether a search should
/// stop if the user has requested quiet mode.
#[derive(Clone, Debug)]
pub struct QuietMatched(Arc<Option<AtomicBool>>);

impl QuietMatched {
    /// Create a new QuietMatched value.
    ///
    /// If quiet is true, then set_match and has_match will reflect whether
    /// a search should quit or not because it found a match.
    ///
    /// If quiet is false, then set_match is always a no-op and has_match
    /// always returns false.
    fn new(quiet: bool) -> QuietMatched {
        let atomic = if quiet { Some(AtomicBool::new(false)) } else { None };
        QuietMatched(Arc::new(atomic))
    }

    /// Returns true if and only if quiet mode is enabled and a match has
    /// occurred.
    pub fn has_match(&self) -> bool {
        match *self.0 {
            None => false,
            Some(ref matched) => matched.load(Ordering::SeqCst),
        }
    }

    /// Sets whether a match has occurred or not.
    ///
    /// If quiet mode is disabled, then this is a no-op.
    pub fn set_match(&self, yes: bool) -> bool {
        match *self.0 {
            None => false,
            Some(_) if !yes => false,
            Some(ref m) => { m.store(true, Ordering::SeqCst); true }
        }
    }
}

/// Convert the result of a `parse_human_readable_size_arg` call into
/// a `usize`, failing if the type does not fit.
fn human_readable_to_usize(
    arg_name: &str,
    value: Option<u64>,
) -> Result<Option<usize>> {
    use std::usize;

    match value {
        None => Ok(None),
        Some(v) => {
            if v <= usize::MAX as u64 {
                Ok(Some(v as usize))
            } else {
                let msg = format!("number too large for {}", arg_name);
                Err(From::from(msg))
            }
        }
    }
}

/// Returns true if and only if stdin is deemed searchable.
#[cfg(unix)]
fn stdin_is_readable() -> bool {
    use std::os::unix::fs::FileTypeExt;
    use same_file::Handle;

    let ft = match Handle::stdin().and_then(|h| h.as_file().metadata()) {
        Err(_) => return false,
        Ok(md) => md.file_type(),
    };
    ft.is_file() || ft.is_fifo()
}

/// Returns true if and only if stdin is deemed searchable.
#[cfg(windows)]
fn stdin_is_readable() -> bool {
    // On Windows, it's not clear what the possibilities are to me, so just
    // always return true.
    true
}
