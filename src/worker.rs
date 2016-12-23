use std::fs::File;
use std::io;
use std::path::Path;

use grep::Grep;
use ignore::DirEntry;
use memmap::{Mmap, Protection};
use termcolor::WriteColor;

use pathutil::strip_prefix;
use printer::Printer;
use search_buffer::BufferSearcher;
use search_stream::{InputBuffer, Searcher};

use Result;

pub enum Work {
    Stdin,
    DirEntry(DirEntry),
}

pub struct WorkerBuilder {
    grep: Grep,
    opts: Options,
}

#[derive(Clone, Debug)]
struct Options {
    mmap: bool,
    after_context: usize,
    before_context: usize,
    count: bool,
    files_with_matches: bool,
    files_without_matches: bool,
    eol: u8,
    invert_match: bool,
    line_number: bool,
    max_count: Option<u64>,
    no_messages: bool,
    quiet: bool,
    text: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            mmap: false,
            after_context: 0,
            before_context: 0,
            count: false,
            files_with_matches: false,
            files_without_matches: false,
            eol: b'\n',
            invert_match: false,
            line_number: false,
            max_count: None,
            no_messages: false,
            quiet: false,
            text: false,
        }
    }
}

impl WorkerBuilder {
    /// Create a new builder for a worker.
    ///
    /// A reusable input buffer and a grep matcher are required, but there
    /// are numerous additional options that can be configured on this builder.
    pub fn new(grep: Grep) -> WorkerBuilder {
        WorkerBuilder {
            grep: grep,
            opts: Options::default(),
        }
    }

    /// Create the worker from this builder.
    pub fn build(self) -> Worker {
        let mut inpbuf = InputBuffer::new();
        inpbuf.eol(self.opts.eol);
        Worker {
            grep: self.grep,
            inpbuf: inpbuf,
            opts: self.opts,
        }
    }

    /// The number of contextual lines to show after each match. The default
    /// is zero.
    pub fn after_context(mut self, count: usize) -> Self {
        self.opts.after_context = count;
        self
    }

    /// The number of contextual lines to show before each match. The default
    /// is zero.
    pub fn before_context(mut self, count: usize) -> Self {
        self.opts.before_context = count;
        self
    }

    /// If enabled, searching will print a count instead of each match.
    ///
    /// Disabled by default.
    pub fn count(mut self, yes: bool) -> Self {
        self.opts.count = yes;
        self
    }

    /// If enabled, searching will print the path instead of each match.
    ///
    /// Disabled by default.
    pub fn files_with_matches(mut self, yes: bool) -> Self {
        self.opts.files_with_matches = yes;
        self
    }

    /// If enabled, searching will print the path of files without any matches.
    ///
    /// Disabled by default.
    pub fn files_without_matches(mut self, yes: bool) -> Self {
        self.opts.files_without_matches = yes;
        self
    }

    /// Set the end-of-line byte used by this searcher.
    pub fn eol(mut self, eol: u8) -> Self {
        self.opts.eol = eol;
        self
    }

    /// If enabled, matching is inverted so that lines that *don't* match the
    /// given pattern are treated as matches.
    pub fn invert_match(mut self, yes: bool) -> Self {
        self.opts.invert_match = yes;
        self
    }

    /// If enabled, compute line numbers and prefix each line of output with
    /// them.
    pub fn line_number(mut self, yes: bool) -> Self {
        self.opts.line_number = yes;
        self
    }

    /// Limit the number of matches to the given count.
    ///
    /// The default is None, which corresponds to no limit.
    pub fn max_count(mut self, count: Option<u64>) -> Self {
        self.opts.max_count = count;
        self
    }

    /// If enabled, try to use memory maps for searching if possible.
    pub fn mmap(mut self, yes: bool) -> Self {
        self.opts.mmap = yes;
        self
    }

    /// If enabled, error messages are suppressed.
    ///
    /// This is disabled by default.
    pub fn no_messages(mut self, yes: bool) -> Self {
        self.opts.no_messages = yes;
        self
    }

    /// If enabled, don't show any output and quit searching after the first
    /// match is found.
    pub fn quiet(mut self, yes: bool) -> Self {
        self.opts.quiet = yes;
        self
    }

    /// If enabled, search binary files as if they were text.
    pub fn text(mut self, yes: bool) -> Self {
        self.opts.text = yes;
        self
    }
}

/// Worker is responsible for executing searches on file paths, while choosing
/// streaming search or memory map search as appropriate.
pub struct Worker {
    inpbuf: InputBuffer,
    grep: Grep,
    opts: Options,
}

impl Worker {
    /// Execute the worker with the given printer and work item.
    ///
    /// A work item can either be stdin or a file path.
    pub fn run<W: WriteColor>(
        &mut self,
        printer: &mut Printer<W>,
        work: Work,
    ) -> u64 {
        let result = match work {
            Work::Stdin => {
                let stdin = io::stdin();
                let stdin = stdin.lock();
                self.search(printer, Path::new("<stdin>"), stdin)
            }
            Work::DirEntry(dent) => {
                let mut path = dent.path();
                let file = match File::open(path) {
                    Ok(file) => file,
                    Err(err) => {
                        if !self.opts.no_messages {
                            eprintln!("{}: {}", path.display(), err);
                        }
                        return 0;
                    }
                };
                if let Some(p) = strip_prefix("./", path) {
                    path = p;
                }
                if self.opts.mmap {
                    self.search_mmap(printer, path, &file)
                } else {
                    self.search(printer, path, file)
                }
            }
        };
        match result {
            Ok(count) => {
                count
            }
            Err(err) => {
                if !self.opts.no_messages {
                    eprintln!("{}", err);
                }
                0
            }
        }
    }

    fn search<R: io::Read, W: WriteColor>(
        &mut self,
        printer: &mut Printer<W>,
        path: &Path,
        rdr: R,
    ) -> Result<u64> {
        let searcher = Searcher::new(
            &mut self.inpbuf, printer, &self.grep, path, rdr);
        searcher
            .after_context(self.opts.after_context)
            .before_context(self.opts.before_context)
            .count(self.opts.count)
            .files_with_matches(self.opts.files_with_matches)
            .files_without_matches(self.opts.files_without_matches)
            .eol(self.opts.eol)
            .line_number(self.opts.line_number)
            .invert_match(self.opts.invert_match)
            .max_count(self.opts.max_count)
            .quiet(self.opts.quiet)
            .text(self.opts.text)
            .run()
            .map_err(From::from)
    }

    fn search_mmap<W: WriteColor>(
        &mut self,
        printer: &mut Printer<W>,
        path: &Path,
        file: &File,
    ) -> Result<u64> {
        if try!(file.metadata()).len() == 0 {
            // Opening a memory map with an empty file results in an error.
            // However, this may not actually be an empty file! For example,
            // /proc/cpuinfo reports itself as an empty file, but it can
            // produce data when it's read from. Therefore, we fall back to
            // regular read calls.
            return self.search(printer, path, file);
        }
        let mmap = try!(Mmap::open(file, Protection::Read));
        let searcher = BufferSearcher::new(
            printer, &self.grep, path, unsafe { mmap.as_slice() });
        Ok(searcher
            .count(self.opts.count)
            .files_with_matches(self.opts.files_with_matches)
            .files_without_matches(self.opts.files_without_matches)
            .eol(self.opts.eol)
            .line_number(self.opts.line_number)
            .invert_match(self.opts.invert_match)
            .max_count(self.opts.max_count)
            .quiet(self.opts.quiet)
            .text(self.opts.text)
            .run())
    }
}
