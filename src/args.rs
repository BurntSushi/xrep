use std::cmp;
use std::env;
use std::io;
use std::path::{Path, PathBuf};

use docopt::Docopt;
use env_logger;
use grep::{Grep, GrepBuilder};
use log;
use num_cpus;
use regex;
use term::Terminal;
#[cfg(not(windows))]
use term;
#[cfg(windows)]
use term::WinConsole;
use walkdir::WalkDir;

use atty;
use gitignore::{Gitignore, GitignoreBuilder};
use ignore::Ignore;
use out::{Out, ColoredTerminal};
use printer::Printer;
use search_buffer::BufferSearcher;
use search_stream::{InputBuffer, Searcher};
#[cfg(windows)]
use terminal_win::WindowsBuffer;
use types::{FileTypeDef, Types, TypesBuilder};
use walk;

use Result;

/// The Docopt usage string.
///
/// If you've never heard of Docopt before, see: http://docopt.org
/// (TL;DR: The CLI parser is generated from the usage string below.)
const USAGE: &'static str = "
Usage: rg [options] -e PATTERN ... [<path> ...]
       rg [options] <pattern> [<path> ...]
       rg [options] --files [<path> ...]
       rg [options] --type-list
       rg [options] --help
       rg [options] --version

rg recursively searches your current directory for a regex pattern.

Common options:
    -a, --text                 Search binary files as if they were text.
    -c, --count                Only show count of line matches for each file.
    --color WHEN               Whether to use coloring in match.
                               Valid values are never, always or auto.
                               [default: auto]
    -e, --regexp PATTERN ...   Use PATTERN to search. This option can be
                               provided multiple times, where all patterns
                               given are searched.
    -F, --fixed-strings        Treat the pattern as a literal string instead of
                               a regular expression.
    -g, --glob GLOB ...        Include or exclude files for searching that
                               match the given glob. This always overrides any
                               other ignore logic. Multiple glob flags may be
                               used. Globbing rules match .gitignore globs.
                               Precede a glob with a '!' to exclude it.
    -h, --help                 Show this usage message.
    -i, --ignore-case          Case insensitive search.
    -n, --line-number          Show line numbers (1-based). This is enabled
                               by default at a tty.
    -N, --no-line-number       Suppress line numbers.
    -q, --quiet                Do not print anything to stdout. If a match is
                               found in a file, stop searching that file.
    -r, --replace ARG          Replace every match with the string given.
                               Capture group indices (e.g., $5) and names
                               (e.g., $foo) are supported.
    -t, --type TYPE ...        Only search files matching TYPE. Multiple type
                               flags may be provided. Use the --type-list flag
                               to list all available types.
    -T, --type-not TYPE ...    Do not search files matching TYPE. Multiple
                               not-type flags may be provided.
    -u, --unrestricted ...     Reduce the level of 'smart' searching. A
                               single -u doesn't respect .gitignore (etc.)
                               files. Two -u flags will search hidden files
                               and directories. Three -u flags will search
                               binary files. -uu is equivalent to grep -r,
                               and -uuu is equivalent to grep -a -r.
    -v, --invert-match         Invert matching.
    -w, --word-regexp          Only show matches surrounded by word boundaries.
                               This is equivalent to putting \\b before and
                               after the search pattern.

Less common options:
    -A, --after-context NUM
        Show NUM lines after each match.

    -B, --before-context NUM
        Show NUM lines before each match.

    -C, --context NUM
        Show NUM lines before and after each match.

    --column
        Show column numbers (1 based) in output. This only shows the column
        numbers for the first match on each line. Note that this doesn't try
        to account for Unicode. One byte is equal to one column.

    --context-separator ARG
        The string to use when separating non-continuous context lines. Escape
        sequences may be used. [default: --]

    --debug
        Show debug messages.

    --files
        Print each file that would be searched (but don't search).

    -l, --files-with-matches
        Only show path of each file with matches.

    -H, --with-filename
        Prefix each match with the file name that contains it. This is the
        default when more than one file is searched.

    --no-filename
        Never show the filename for a match. This is the default when
        one file is searched.

    --heading
        Show the file name above clusters of matches from each file.
        This is the default mode at a tty.

    --no-heading
        Don't show any file name heading.

    --hidden
        Search hidden directories and files. (Hidden directories and files are
        skipped by default.)

    -L, --follow
        Follow symlinks.

    --maxdepth NUM
        Descend at most NUM directories below the command line arguments.
        A value of zero only searches the starting-points themselves.

    --mmap
        Search using memory maps when possible. This is enabled by default
        when ripgrep thinks it will be faster. (Note that mmap searching
        doesn't currently support the various context related options.)

    --no-mmap
        Never use memory maps, even when they might be faster.

    --no-ignore
        Don't respect ignore files (.gitignore, .ignore, etc.)
        This implies --no-ignore-parent.

    --no-ignore-parent
        Don't respect ignore files in parent directories.

    --no-ignore-vcs
        Don't respect version control ignore files (e.g., .gitignore).
        Note that .ignore files will continue to be respected.

    --null
        Whenever a file name is printed, follow it with a NUL byte.
        This includes printing filenames before matches, and when printing
        a list of matching files such as with --count, --files-with-matches
        and --files.

    -o, --only-matching
        Only print the text that matches the regex, with each match on a
        separate line.

    -p, --pretty
        Alias for --color=always --heading -n.

    -S, --smart-case
        Search case insensitively if the pattern is all lowercase.
        Search case sensitively otherwise.

    -j, --threads ARG
        The number of threads to use. Defaults to the number of logical CPUs
        (capped at 6). [default: 0]

    --version
        Show the version number of ripgrep and exit.

    --vimgrep
        Show results with every match on its own line, including line
        numbers and column numbers. (With this option, a line with more
        than one match of the regex will be printed more than once.)

File type management options:
    --type-list
        Show all supported file types and their associated globs.

    --type-add ARG ...
        Add a new glob for a particular file type. Only one glob can be added
        at a time. Multiple type-add flags can be provided. Unless type-clear
        is used, globs are added to any existing globs inside of ripgrep. Note
        that this must be passed to every invocation of rg.

        Example: `--type-add html:*.html`

    --type-clear TYPE ...
        Clear the file type globs previously defined for TYPE. This only clears
        the default type definitions that are found inside of ripgrep. Note
        that this must be passed to every invocation of rg.
";

/// RawArgs are the args as they are parsed from Docopt. They aren't used
/// directly by the rest of ripgrep.
#[derive(Debug, RustcDecodable)]
pub struct RawArgs {
    arg_pattern: String,
    arg_path: Vec<String>,
    flag_after_context: usize,
    flag_before_context: usize,
    flag_color: String,
    flag_column: bool,
    flag_context: usize,
    flag_context_separator: String,
    flag_count: bool,
    flag_files_with_matches: bool,
    flag_debug: bool,
    flag_files: bool,
    flag_follow: bool,
    flag_glob: Vec<String>,
    flag_heading: bool,
    flag_hidden: bool,
    flag_ignore_case: bool,
    flag_invert_match: bool,
    flag_line_number: bool,
    flag_fixed_strings: bool,
    flag_maxdepth: Option<usize>,
    flag_mmap: bool,
    flag_no_heading: bool,
    flag_no_ignore: bool,
    flag_no_ignore_parent: bool,
    flag_no_ignore_vcs: bool,
    flag_no_line_number: bool,
    flag_no_mmap: bool,
    flag_no_filename: bool,
    flag_null: bool,
    flag_only_matching: bool,
    flag_pretty: bool,
    flag_quiet: bool,
    flag_regexp: Vec<String>,
    flag_replace: Option<String>,
    flag_smart_case: bool,
    flag_text: bool,
    flag_threads: usize,
    flag_type: Vec<String>,
    flag_type_not: Vec<String>,
    flag_type_list: bool,
    flag_type_add: Vec<String>,
    flag_type_clear: Vec<String>,
    flag_unrestricted: u32,
    flag_vimgrep: bool,
    flag_with_filename: bool,
    flag_word_regexp: bool,
}

/// Args are transformed/normalized from RawArgs.
#[derive(Debug)]
pub struct Args {
    pattern: String,
    paths: Vec<PathBuf>,
    after_context: usize,
    before_context: usize,
    color: bool,
    column: bool,
    context_separator: Vec<u8>,
    count: bool,
    files_with_matches: bool,
    eol: u8,
    files: bool,
    follow: bool,
    glob_overrides: Option<Gitignore>,
    grep: Grep,
    heading: bool,
    hidden: bool,
    ignore_case: bool,
    invert_match: bool,
    line_number: bool,
    line_per_match: bool,
    maxdepth: Option<usize>,
    mmap: bool,
    no_ignore: bool,
    no_ignore_parent: bool,
    no_ignore_vcs: bool,
    null: bool,
    only_matching: bool,
    quiet: bool,
    replace: Option<Vec<u8>>,
    text: bool,
    threads: usize,
    type_defs: Vec<FileTypeDef>,
    type_list: bool,
    types: Types,
    with_filename: bool,
}

impl RawArgs {
    /// Convert arguments parsed into a configuration used by ripgrep.
    fn to_args(&self) -> Result<Args> {
        let pattern = self.pattern();
        let paths =
            if self.arg_path.is_empty() {
                if atty::on_stdin()
                    || self.flag_files
                    || self.flag_type_list
                    || !atty::stdin_is_readable() {
                    vec![Path::new("./").to_path_buf()]
                } else {
                    vec![Path::new("-").to_path_buf()]
                }
            } else {
                self.arg_path.iter().map(|p| {
                    Path::new(p).to_path_buf()
                }).collect()
            };
        let (after_context, before_context) =
            if self.flag_context > 0 {
                (self.flag_context, self.flag_context)
            } else {
                (self.flag_after_context, self.flag_before_context)
            };
        let mmap =
            if before_context > 0 || after_context > 0 || self.flag_no_mmap {
                false
            } else if self.flag_mmap {
                true
            } else if cfg!(windows) {
                // On Windows, memory maps appear faster than read calls. Neat.
                true
            } else if cfg!(darwin) {
                // On Mac, memory maps appear to suck. Neat.
                false
            } else {
                // If we're only searching a few paths and all of them are
                // files, then memory maps are probably faster.
                paths.len() <= 10 && paths.iter().all(|p| p.is_file())
            };
        if mmap {
            debug!("will try to use memory maps");
        }
        let glob_overrides =
            if self.flag_glob.is_empty() {
                None
            } else {
                let cwd = try!(env::current_dir());
                let mut bgi = GitignoreBuilder::new(cwd);
                for pat in &self.flag_glob {
                    try!(bgi.add("<argv>", pat));
                }
                Some(try!(bgi.build()))
            };
        let threads =
            if self.flag_threads == 0 {
                cmp::min(8, num_cpus::get())
            } else {
                self.flag_threads
            };
        let color =
            if self.flag_vimgrep {
                false
            } else if self.flag_color == "auto" {
                atty::on_stdout() || self.flag_pretty
            } else {
                self.flag_color == "always"
            };
        let eol = b'\n';

        let mut with_filename = self.flag_with_filename;
        if !with_filename {
            with_filename = paths.len() > 1 || paths[0].is_dir();
        }
        with_filename = with_filename && !self.flag_no_filename;

        let mut btypes = TypesBuilder::new();
        btypes.add_defaults();
        try!(self.add_types(&mut btypes));
        let types = try!(btypes.build());
        let grep = try!(
            GrepBuilder::new(&pattern)
                .case_smart(self.flag_smart_case)
                .case_insensitive(self.flag_ignore_case)
                .line_terminator(eol)
                .build()
        );
        let no_ignore = self.flag_no_ignore || self.flag_unrestricted >= 1;
        let hidden = self.flag_hidden || self.flag_unrestricted >= 2;
        let text = self.flag_text || self.flag_unrestricted >= 3;
        let mut args = Args {
            pattern: pattern,
            paths: paths,
            after_context: after_context,
            before_context: before_context,
            color: color,
            column: self.flag_column,
            context_separator: unescape(&self.flag_context_separator),
            count: self.flag_count,
            files_with_matches: self.flag_files_with_matches,
            eol: eol,
            files: self.flag_files,
            follow: self.flag_follow,
            glob_overrides: glob_overrides,
            grep: grep,
            heading: !self.flag_no_heading && self.flag_heading,
            hidden: hidden,
            ignore_case: self.flag_ignore_case,
            invert_match: self.flag_invert_match,
            line_number: !self.flag_no_line_number && self.flag_line_number,
            line_per_match: self.flag_vimgrep,
            maxdepth: self.flag_maxdepth,
            mmap: mmap,
            no_ignore: no_ignore,
            no_ignore_parent:
                // --no-ignore implies --no-ignore-parent
                self.flag_no_ignore_parent || no_ignore,
            no_ignore_vcs:
                // --no-ignore implies --no-ignore-vcs
                self.flag_no_ignore_vcs || no_ignore,
            null: self.flag_null,
            only_matching: self.flag_only_matching,
            quiet: self.flag_quiet,
            replace: self.flag_replace.clone().map(|s| s.into_bytes()),
            text: text,
            threads: threads,
            type_defs: btypes.definitions(),
            type_list: self.flag_type_list,
            types: types,
            with_filename: with_filename,
        };
        // If stdout is a tty, then apply some special default options.
        if atty::on_stdout() || self.flag_pretty {
            if !self.flag_no_line_number && !args.count {
                args.line_number = true;
            }
            if !self.flag_no_heading {
                args.heading = true;
            }
        }
        if self.flag_vimgrep {
            args.column = true;
            args.line_number = true;
        }
        Ok(args)
    }

    fn add_types(&self, types: &mut TypesBuilder) -> Result<()> {
        for ty in &self.flag_type_clear {
            types.clear(ty);
        }
        for def in &self.flag_type_add {
            try!(types.add_def(def));
        }
        for ty in &self.flag_type {
            types.select(ty);
        }
        for ty in &self.flag_type_not {
            types.negate(ty);
        }
        Ok(())
    }

    fn pattern(&self) -> String {
        if !self.flag_regexp.is_empty() {
            if self.flag_fixed_strings {
                self.flag_regexp.iter().cloned().map(|lit| {
                    self.word_pattern(regex::quote(&lit))
                }).collect::<Vec<String>>().join("|")
            } else {
                self.flag_regexp.iter().cloned().map(|pat| {
                    self.word_pattern(pat)
                }).collect::<Vec<String>>().join("|")
            }
        } else {
            if self.flag_fixed_strings {
                self.word_pattern(regex::quote(&self.arg_pattern))
            } else {
                self.word_pattern(self.arg_pattern.clone())
            }
        }
    }

    fn word_pattern(&self, s: String) -> String {
        if self.flag_word_regexp {
            format!(r"\b{}\b", s)
        } else {
            s
        }
    }
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
        // Get all of the arguments, being careful to require valid UTF-8.
        let mut argv = vec![];
        for arg in env::args_os() {
            match arg.into_string() {
                Ok(s) => argv.push(s),
                Err(s) => {
                    errored!("Argument '{}' is not valid UTF-8. \
                              Use hex escape sequences to match arbitrary \
                              bytes in a pattern (e.g., \\xFF).",
                              s.to_string_lossy());
                }
            }
        }
        let mut raw: RawArgs =
            Docopt::new(USAGE)
                .and_then(|d| d.argv(argv).version(Some(version())).decode())
                .unwrap_or_else(|e| e.exit());

        let mut logb = env_logger::LogBuilder::new();
        if raw.flag_debug {
            logb.filter(None, log::LogLevelFilter::Debug);
        } else {
            logb.filter(None, log::LogLevelFilter::Warn);
        }
        if let Err(err) = logb.init() {
            errored!("failed to initialize logger: {}", err);
        }

        // *sigh*... If --files is given, then the first path ends up in
        // pattern.
        if raw.flag_files {
            if !raw.arg_pattern.is_empty() {
                raw.arg_path.insert(0, raw.arg_pattern.clone());
            }
        }
        raw.to_args().map_err(From::from)
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

    /// Creates a new input buffer that is used in searching.
    pub fn input_buffer(&self) -> InputBuffer {
        let mut inp = InputBuffer::new();
        inp.eol(self.eol);
        inp
    }

    /// Whether we should prefer memory maps for searching or not.
    pub fn mmap(&self) -> bool {
        self.mmap
    }

    /// Create a new printer of individual search results that writes to the
    /// writer given.
    pub fn printer<W: Terminal + Send>(&self, wtr: W) -> Printer<W> {
        let mut p = Printer::new(wtr)
            .column(self.column)
            .context_separator(self.context_separator.clone())
            .eol(self.eol)
            .heading(self.heading)
            .line_per_match(self.line_per_match)
            .quiet(self.quiet)
            .null(self.null)
            .with_filename(self.with_filename)
            .only_matching(self.only_matching);
        if let Some(ref rep) = self.replace {
            p = p.replace(rep.clone());
        }
        p
    }

    /// Create a new printer of search results for an entire file that writes
    /// to the writer given.
    pub fn out(&self) -> Out {
        let mut out = Out::new(self.color);
        if let Some(filesep) = self.file_separator() {
            out = out.file_separator(filesep);
        }
        out
    }

    /// Retrieve the configured file separator.
    pub fn file_separator(&self) -> Option<Vec<u8>> {
        if self.heading && !self.count && !self.files_with_matches {
            Some(b"".to_vec())
        } else if self.before_context > 0 || self.after_context > 0 {
            Some(self.context_separator.clone())
        } else {
            None
        }
    }

    /// Create a new buffer for use with searching.
    #[cfg(not(windows))]
    pub fn outbuf(&self) -> ColoredTerminal<term::TerminfoTerminal<Vec<u8>>> {
        ColoredTerminal::new(vec![], self.color)
    }

    /// Create a new buffer for use with searching.
    #[cfg(windows)]
    pub fn outbuf(&self) -> ColoredTerminal<WindowsBuffer> {
        ColoredTerminal::new_buffer(self.color)
    }

    /// Create a new buffer for use with searching.
    #[cfg(not(windows))]
    pub fn stdout(
        &self,
    ) -> ColoredTerminal<term::TerminfoTerminal<io::BufWriter<io::Stdout>>> {
        ColoredTerminal::new(io::BufWriter::new(io::stdout()), self.color)
    }

    /// Create a new buffer for use with searching.
    #[cfg(windows)]
    pub fn stdout(&self) -> ColoredTerminal<WinConsole<io::Stdout>> {
        ColoredTerminal::new_stdout(self.color)
    }

    /// Return the paths that should be searched.
    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    /// Create a new line based searcher whose configuration is taken from the
    /// command line. This searcher supports a dizzying array of features:
    /// inverted matching, line counting, context control and more.
    pub fn searcher<'a, R: io::Read, W: Send + Terminal>(
        &self,
        inp: &'a mut InputBuffer,
        printer: &'a mut Printer<W>,
        grep: &'a Grep,
        path: &'a Path,
        rdr: R,
    ) -> Searcher<'a, R, W> {
        Searcher::new(inp, printer, grep, path, rdr)
            .after_context(self.after_context)
            .before_context(self.before_context)
            .count(self.count)
            .files_with_matches(self.files_with_matches)
            .eol(self.eol)
            .line_number(self.line_number)
            .invert_match(self.invert_match)
            .text(self.text)
    }

    /// Create a new line based searcher whose configuration is taken from the
    /// command line. This search operates on an entire file all once (which
    /// may have been memory mapped).
    pub fn searcher_buffer<'a, W: Send + Terminal>(
        &self,
        printer: &'a mut Printer<W>,
        grep: &'a Grep,
        path: &'a Path,
        buf: &'a [u8],
    ) -> BufferSearcher<'a, W> {
        BufferSearcher::new(printer, grep, path, buf)
            .count(self.count)
            .files_with_matches(self.files_with_matches)
            .eol(self.eol)
            .line_number(self.line_number)
            .invert_match(self.invert_match)
            .text(self.text)
    }

    /// Returns the number of worker search threads that should be used.
    pub fn threads(&self) -> usize {
        self.threads
    }

    /// Returns a list of type definitions currently loaded.
    pub fn type_defs(&self) -> &[FileTypeDef] {
        &self.type_defs
    }

    /// Returns true if ripgrep should print the type definitions currently
    /// loaded and then exit.
    pub fn type_list(&self) -> bool {
        self.type_list
    }

    /// Create a new recursive directory iterator at the path given.
    pub fn walker(&self, path: &Path) -> Result<walk::Iter> {
        let mut wd = WalkDir::new(path).follow_links(self.follow);
        if let Some(maxdepth) = self.maxdepth {
            wd = wd.max_depth(maxdepth);
        }
        let mut ig = Ignore::new();
        // Only register ignore rules if this is a directory. If it's a file,
        // then it was explicitly given by the end user, so we always search
        // it.
        if path.is_dir() {
            ig.ignore_hidden(!self.hidden);
            ig.no_ignore(self.no_ignore);
            ig.no_ignore_vcs(self.no_ignore_vcs);
            ig.add_types(self.types.clone());
            if !self.no_ignore_parent {
                try!(ig.push_parents(path));
            }
            if let Some(ref overrides) = self.glob_overrides {
                ig.add_override(overrides.clone());
            }
        }
        Ok(walk::Iter::new(ig, wd))
    }
}

fn version() -> String {
    let (maj, min, pat) = (
        option_env!("CARGO_PKG_VERSION_MAJOR"),
        option_env!("CARGO_PKG_VERSION_MINOR"),
        option_env!("CARGO_PKG_VERSION_PATCH"),
    );
    match (maj, min, pat) {
        (Some(maj), Some(min), Some(pat)) =>
            format!("{}.{}.{}", maj, min, pat),
        _ => "".to_owned(),
    }
}

/// A single state in the state machine used by `unescape`.
#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    Escape,
    HexFirst,
    HexSecond(char),
    Literal,
}

/// Unescapes a string given on the command line. It supports a limit set of
/// escape sequences:
///
/// * \t, \r and \n are mapped to their corresponding ASCII bytes.
/// * \xZZ hexadecimal escapes are mapped to their byte.
fn unescape(s: &str) -> Vec<u8> {
    use self::State::*;

    let mut bytes = vec![];
    let mut state = Literal;
    for c in s.chars() {
        match state {
            Escape => {
                match c {
                    'n' => { bytes.push(b'\n'); state = Literal; }
                    'r' => { bytes.push(b'\r'); state = Literal; }
                    't' => { bytes.push(b'\t'); state = Literal; }
                    'x' => { state = HexFirst; }
                    c => {
                        bytes.extend(&format!(r"\{}", c).into_bytes());
                        state = Literal;
                    }
                }
            }
            HexFirst => {
                match c {
                    '0'...'9' | 'A'...'F' | 'a'...'f' => {
                        state = HexSecond(c);
                    }
                    c => {
                        bytes.extend(&format!(r"\x{}", c).into_bytes());
                        state = Literal;
                    }
                }
            }
            HexSecond(first) => {
                match c {
                    '0'...'9' | 'A'...'F' | 'a'...'f' => {
                        let ordinal = format!("{}{}", first, c);
                        let byte = u8::from_str_radix(&ordinal, 16).unwrap();
                        bytes.push(byte);
                        state = Literal;
                    }
                    c => {
                        let original = format!(r"\x{}{}", first, c);
                        bytes.extend(&original.into_bytes());
                        state = Literal;
                    }
                }
            }
            Literal => {
                match c {
                    '\\' => { state = Escape; }
                    c => { bytes.extend(c.to_string().as_bytes()); }
                }
            }
        }
    }
    match state {
        Escape => bytes.push(b'\\'),
        HexFirst => bytes.extend(b"\\x"),
        HexSecond(c) => bytes.extend(&format!("\\x{}", c).into_bytes()),
        Literal => {}
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::unescape;

    fn b(bytes: &'static [u8]) -> Vec<u8> {
        bytes.to_vec()
    }

    #[test]
    fn unescape_nul() {
        assert_eq!(b(b"\x00"), unescape(r"\x00"));
    }

    #[test]
    fn unescape_nl() {
        assert_eq!(b(b"\n"), unescape(r"\n"));
    }

    #[test]
    fn unescape_tab() {
        assert_eq!(b(b"\t"), unescape(r"\t"));
    }

    #[test]
    fn unescape_carriage() {
        assert_eq!(b(b"\r"), unescape(r"\r"));
    }

    #[test]
    fn unescape_nothing_simple() {
        assert_eq!(b(b"\\a"), unescape(r"\a"));
    }

    #[test]
    fn unescape_nothing_hex0() {
        assert_eq!(b(b"\\x"), unescape(r"\x"));
    }

    #[test]
    fn unescape_nothing_hex1() {
        assert_eq!(b(b"\\xz"), unescape(r"\xz"));
    }

    #[test]
    fn unescape_nothing_hex2() {
        assert_eq!(b(b"\\xzz"), unescape(r"\xzz"));
    }
}
