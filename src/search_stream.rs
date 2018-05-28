/*!
The `search_stream` module is responsible for searching a single file and
printing matches. In particular, it searches the file in a streaming fashion
using `read` calls and a (roughly) fixed size buffer.
*/

use std::cmp;
use std::error::Error as StdError;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use bytecount;
use grep::{Grep, Match};
use memchr::{memchr, memrchr};
use termcolor::WriteColor;

use printer::Printer;

/// The default read size (capacity of input buffer).
const READ_SIZE: usize = 8 * (1<<10);

/// Error describes errors that can occur while searching.
#[derive(Debug)]
pub enum Error {
    /// A standard I/O error attached to a particular file path.
    Io {
        err: io::Error,
        path: PathBuf,
    }
}

impl Error {
    fn from_io<P: AsRef<Path>>(err: io::Error, path: P) -> Error {
        Error::Io { err: err, path: path.as_ref().to_path_buf() }
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Io { ref err, .. } => err.description(),
        }
    }

    fn cause(&self) -> Option<&StdError> {
        match *self {
            Error::Io { ref err, .. } => Some(err),
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Io { ref err, ref path } => {
                write!(f, "{}: {}", path.display(), err)
            }
        }
    }
}

pub struct Searcher<'a, R, W: 'a> {
    opts: Options,
    inp: &'a mut InputBuffer,
    printer: &'a mut Printer<W>,
    grep: &'a Grep,
    path: &'a Path,
    haystack: R,
    match_line_count: u64,
    match_count: Option<u64>,
    line_count: Option<u64>,
    byte_offset: Option<u64>,
    last_match: Match,
    last_printed: usize,
    last_line: usize,
    after_context_remaining: usize,
}

/// Options for configuring search.
#[derive(Clone)]
pub struct Options {
    pub after_context: usize,
    pub before_context: usize,
    pub byte_offset: bool,
    pub count: bool,
    pub count_matches: bool,
    pub files_with_matches: bool,
    pub files_without_matches: bool,
    pub eol: u8,
    pub invert_match: bool,
    pub line_number: bool,
    pub max_count: Option<u64>,
    pub quiet: bool,
    pub text: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            after_context: 0,
            before_context: 0,
            byte_offset: false,
            count: false,
            count_matches: false,
            files_with_matches: false,
            files_without_matches: false,
            eol: b'\n',
            invert_match: false,
            line_number: false,
            max_count: None,
            quiet: false,
            text: false,
        }
    }

}

impl Options {
    /// Several options (--quiet, --count, --count-matches, --files-with-matches,
    /// --files-without-match) imply that we shouldn't ever display matches.
    pub fn skip_matches(&self) -> bool {
        self.count || self.files_with_matches || self.files_without_matches
        || self.quiet || self.count_matches
    }

    /// Some options (--quiet, --files-with-matches, --files-without-match)
    /// imply that we can stop searching after the first match.
    pub fn stop_after_first_match(&self) -> bool {
        self.files_with_matches || self.files_without_matches || self.quiet
    }

    /// Returns true if the search should terminate based on the match line count.
    pub fn terminate(&self, match_line_count: u64) -> bool {
        if match_line_count > 0 && self.stop_after_first_match() {
            return true;
        }
        if self.max_count.map_or(false, |max| match_line_count >= max) {
            return true;
        }
        false
    }
}

impl<'a, R: io::Read, W: WriteColor> Searcher<'a, R, W> {
    /// Create a new searcher.
    ///
    /// `inp` is a reusable input buffer that is used as scratch space by this
    /// searcher.
    ///
    /// `printer` is used to output all results of searching.
    ///
    /// `grep` is the actual matcher.
    ///
    /// `path` is the file path being searched.
    ///
    /// `haystack` is a reader of text to search.
    pub fn new(
        inp: &'a mut InputBuffer,
        printer: &'a mut Printer<W>,
        grep: &'a Grep,
        path: &'a Path,
        haystack: R,
    ) -> Searcher<'a, R, W> {
        Searcher {
            opts: Options::default(),
            inp: inp,
            printer: printer,
            grep: grep,
            path: path,
            haystack: haystack,
            match_line_count: 0,
            match_count: None,
            line_count: None,
            byte_offset: None,
            last_match: Match::default(),
            last_printed: 0,
            last_line: 0,
            after_context_remaining: 0,
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

    /// If enabled, searching will print a 0-based offset of the
    /// matching line (or the actual match if -o is specified) before
    /// printing the line itself.
    ///
    /// Disabled by default.
    pub fn byte_offset(mut self, yes: bool) -> Self {
        self.opts.byte_offset = yes;
        self
    }

    /// If enabled, searching will print a count instead of each match.
    ///
    /// Disabled by default.
    pub fn count(mut self, yes: bool) -> Self {
        self.opts.count = yes;
        self
    }

    /// If enabled, searching will print the count of individual matches
    /// instead of each match.
    ///
    /// Disabled by default.
   pub fn count_matches(mut self, yes: bool) -> Self {
        self.opts.count_matches = yes;
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

    /// If enabled, don't show any output and quit searching after the first
    /// match is found.
    pub fn quiet(mut self, yes: bool) -> Self {
        self.opts.quiet = yes;
        self
    }

    /// If enabled, search binary files as if they were text.
    pub fn text(mut self, yes: bool) -> Self {
        self.opts.text = yes;
        self.inp.text(yes);
        self
    }

    /// Execute the search. Results are written to the printer and the total
    /// number of matches is returned.
    #[inline(never)]
    pub fn run(mut self) -> Result<u64, Error> {
        self.inp.reset();
        self.match_line_count = 0;
        self.line_count = if self.opts.line_number { Some(0) } else { None };
        self.byte_offset = if self.opts.byte_offset { Some(0) } else { None };
        self.match_count = if self.opts.count_matches { Some(0) } else { None };
        self.last_match = Match::default();
        self.after_context_remaining = 0;
        while !self.terminate() {
            let upto = self.inp.lastnl;
            self.print_after_context(upto);
            if !self.fill()? {
                break;
            }
            while !self.terminate() && self.inp.pos < self.inp.lastnl {
                let matched = self.grep.read_match(
                    &mut self.last_match,
                    &self.inp.buf[..self.inp.lastnl],
                    self.inp.pos);
                if self.opts.invert_match {
                    let upto =
                        if matched {
                            self.last_match.start()
                        } else {
                            self.inp.lastnl
                        };
                    if upto > self.inp.pos {
                        let upto_context = self.inp.pos;
                        self.print_after_context(upto_context);
                        self.print_before_context(upto_context);
                        self.print_inverted_matches(upto);
                    }
                } else if matched {
                    let start = self.last_match.start();
                    let end = self.last_match.end();
                    self.print_after_context(start);
                    self.print_before_context(start);
                    self.print_match(start, end);
                }
                if matched {
                    self.inp.pos = self.last_match.end();
                } else {
                    self.inp.pos = self.inp.lastnl;
                }
            }
        }
        if self.after_context_remaining > 0 {
            if self.last_printed == self.inp.lastnl {
                self.fill()?;
            }
            let upto = self.inp.lastnl;
            if upto > 0 {
                self.print_after_context(upto);
            }
        }
        if self.match_line_count > 0 {
            if self.opts.count {
                self.printer.path_count(self.path, self.match_line_count);
            } else if self.opts.count_matches {
                self.printer.path_count(self.path, self.match_count.unwrap());
            } else if self.opts.files_with_matches {
                self.printer.path(self.path);
            }
        } else if self.opts.files_without_matches {
            self.printer.path(self.path);
        }
        Ok(self.match_line_count)
    }

    #[inline(always)]
    fn terminate(&self) -> bool {
        self.opts.terminate(self.match_line_count)
    }

    #[inline(always)]
    fn fill(&mut self) -> Result<bool, Error> {
        let keep =
            if self.opts.before_context > 0 || self.opts.after_context > 0 {
                let lines = 1 + cmp::max(
                    self.opts.before_context, self.opts.after_context);
                start_of_previous_lines(
                    self.opts.eol,
                    &self.inp.buf,
                    self.inp.lastnl.saturating_sub(1),
                    lines)
            } else {
                self.inp.lastnl
            };
        if keep < self.last_printed {
            self.last_printed -= keep;
        } else {
            self.last_printed = 0;
        }
        if keep <= self.last_line {
            self.last_line -= keep;
        } else {
            self.count_lines(keep);
            self.last_line = 0;
        }
        self.count_byte_offset(keep);
        let ok = self.inp.fill(&mut self.haystack, keep).map_err(|err| {
            Error::from_io(err, &self.path)
        })?;
        Ok(ok)
    }

    #[inline(always)]
    fn print_inverted_matches(&mut self, upto: usize) {
        debug_assert!(self.opts.invert_match);
        let mut it = IterLines::new(self.opts.eol, self.inp.pos);
        while let Some((start, end)) = it.next(&self.inp.buf[..upto]) {
            if self.terminate() {
                return;
            }
            self.print_match(start, end);
            self.inp.pos = end;
        }
    }

    #[inline(always)]
    fn print_before_context(&mut self, upto: usize) {
        if self.opts.skip_matches() || self.opts.before_context == 0 {
            return;
        }
        let start = self.last_printed;
        let end = upto;
        if start >= end {
            return;
        }
        let before_context_start =
            start + start_of_previous_lines(
                self.opts.eol,
                &self.inp.buf[start..],
                end - start - 1,
                self.opts.before_context);
        let mut it = IterLines::new(self.opts.eol, before_context_start);
        while let Some((s, e)) = it.next(&self.inp.buf[..end]) {
            self.print_separator(s);
            self.print_context(s, e);
        }
    }

    #[inline(always)]
    fn print_after_context(&mut self, upto: usize) {
        if self.opts.skip_matches() || self.after_context_remaining == 0 {
            return;
        }
        let start = self.last_printed;
        let end = upto;
        let mut it = IterLines::new(self.opts.eol, start);
        while let Some((s, e)) = it.next(&self.inp.buf[..end]) {
            self.print_context(s, e);
            self.after_context_remaining -= 1;
            if self.after_context_remaining == 0 {
                break;
            }
        }
    }

    #[inline(always)]
    fn print_match(&mut self, start: usize, end: usize) {
        self.match_line_count += 1;
        self.count_individual_matches(start, end);
        if self.opts.skip_matches() {
            return;
        }
        self.print_separator(start);
        self.count_lines(start);
        self.add_line(end);
        self.printer.matched(
            self.grep.regex(), self.path,
            &self.inp.buf, start, end, self.line_count, self.byte_offset);
        self.last_printed = end;
        self.after_context_remaining = self.opts.after_context;
    }

    #[inline(always)]
    fn print_context(&mut self, start: usize, end: usize) {
        self.count_lines(start);
        self.add_line(end);
        self.printer.context(
            &self.path, &self.inp.buf, start, end,
            self.line_count, self.byte_offset);
        self.last_printed = end;
    }

    #[inline(always)]
    fn print_separator(&mut self, before: usize) {
        if self.opts.before_context == 0 && self.opts.after_context == 0 {
            return;
        }
        if !self.printer.has_printed() {
            return;
        }
        if (self.last_printed == 0 && before > 0)
            || self.last_printed < before {
            self.printer.context_separate();
        }
    }

    #[inline(always)]
    fn count_byte_offset(&mut self, buf_last_end: usize) {
        if let Some(ref mut byte_offset) = self.byte_offset {
            *byte_offset += buf_last_end as u64;
        }
    }

    #[inline(always)]
    fn count_individual_matches(&mut self, start: usize, end: usize) {
        if let Some(ref mut count) = self.match_count {
            for _ in self.grep.regex().find_iter(&self.inp.buf[start..end]) {
                *count += 1;
            }
        }
    }

    #[inline(always)]
    fn count_lines(&mut self, upto: usize) {
        if let Some(ref mut line_count) = self.line_count {
            *line_count += count_lines(
                &self.inp.buf[self.last_line..upto], self.opts.eol);
            self.last_line = upto;
        }
    }

    #[inline(always)]
    fn add_line(&mut self, line_end: usize) {
        if let Some(ref mut line_count) = self.line_count {
            *line_count += 1;
            self.last_line = line_end;
        }
    }
}

/// `InputBuffer` encapsulates the logic of maintaining a ~fixed sized buffer
/// on which to search. There are three key pieces of complexity:
///
/// 1. We must be able to handle lines that are longer than the size of the
///    buffer. For this reason, the buffer is allowed to expand (and is
///    therefore not technically fixed). Note that once a buffer expands, it
///    will never contract.
/// 2. The contents of the buffer may end with a partial line, so we must keep
///    track of where the last complete line ends. Namely, the partial line
///    is only completed on subsequent reads *after* searching up through
///    the last complete line is done.
/// 3. When printing the context of a match, the last N lines of the buffer
///    may need to be rolled over into the next buffer. For example, a match
///    may occur at the beginning of a buffer, in which case, lines at the end
///    of the previous contents of the buffer need to be printed.
///
/// An `InputBuffer` is designed to be reused and isn't tied to any particular
/// reader.
pub struct InputBuffer {
    /// The number of bytes to attempt to read at a time. Once set, this is
    /// never changed.
    read_size: usize,
    /// The end-of-line terminator used in this buffer.
    eol: u8,
    /// A scratch buffer.
    tmp: Vec<u8>,
    /// A buffer to read bytes into. All searches are executed directly against
    /// this buffer and pos/lastnl/end point into it.
    buf: Vec<u8>,
    /// The current position in buf. The current position represents where the
    /// next search should start.
    pos: usize,
    /// The position immediately following the last line terminator in buf.
    /// This may be equal to end.
    ///
    /// Searching should never cross this boundary. In particular, the contents
    /// of the buffer following this position may correspond to *partial* line.
    /// All contents before this position are complete lines.
    lastnl: usize,
    /// The end position of the buffer. Data after this position is not
    /// specified.
    end: usize,
    /// Set to true if and only if no reads have occurred yet.
    first: bool,
    /// Set to true if all binary data should be treated as if it were text.
    text: bool,
}

impl InputBuffer {
    /// Create a new buffer with a default capacity.
    pub fn new() -> InputBuffer {
        InputBuffer::with_capacity(READ_SIZE)
    }

    /// Create a new buffer with the capacity given.
    ///
    /// The capacity determines the size of each read from the underlying
    /// reader.
    ///
    /// `cap` must be a minimum of `1`.
    pub fn with_capacity(mut cap: usize) -> InputBuffer {
        if cap == 0 {
            cap = 1;
        }
        InputBuffer {
            read_size: cap,
            eol: b'\n',
            buf: vec![0; cap],
            tmp: vec![],
            pos: 0,
            lastnl: 0,
            end: 0,
            first: true,
            text: false,
        }
    }

    /// Set the end-of-line terminator used by this input buffer.
    pub fn eol(&mut self, eol: u8) -> &mut Self {
        self.eol = eol;
        self
    }

    /// If enabled, search binary files as if they were text.
    ///
    /// Note that this may cause the buffer to load the entire contents of a
    /// file into memory.
    pub fn text(&mut self, yes: bool) -> &mut Self {
        self.text = yes;
        self
    }

    /// Resets this buffer so that it may be reused with a new reader.
    fn reset(&mut self) {
        self.pos = 0;
        self.lastnl = 0;
        self.end = 0;
        self.first = true;
    }

    /// Fill the contents of this buffer with the reader given. The reader
    /// given should be the same in every call to fill unless reset has been
    /// called.
    ///
    /// The bytes in buf[keep_from..end] are rolled over into the beginning
    /// of the buffer.
    fn fill<R: io::Read>(
        &mut self,
        rdr: &mut R,
        keep_from: usize,
    ) -> Result<bool, io::Error> {
        // Rollover bytes from buf[keep_from..end] and update our various
        // pointers. N.B. This could be done with the ptr::copy, but I haven't
        // been able to produce a benchmark that notices a difference in
        // performance. (Invariably, ptr::copy is seems clearer IMO, but it is
        // not safe.)
        self.tmp.clear();
        self.tmp.extend_from_slice(&self.buf[keep_from..self.end]);
        self.buf[0..self.tmp.len()].copy_from_slice(&self.tmp);
        self.pos = self.lastnl - keep_from;
        self.lastnl = 0;
        self.end = self.tmp.len();
        while self.lastnl == 0 {
            // If our buffer isn't big enough to hold the contents of a full
            // read, expand it.
            if self.buf.len() - self.end < self.read_size {
                let min_len = self.read_size + self.buf.len() - self.end;
                let new_len = cmp::max(min_len, self.buf.len() * 2);
                self.buf.resize(new_len, 0);
            }
            let n = rdr.read(
                &mut self.buf[self.end..self.end + self.read_size])?;
            if !self.text {
                if is_binary(&self.buf[self.end..self.end + n], self.first) {
                    return Ok(false);
                }
            }
            self.first = false;
            // We assume that reading 0 bytes means we've hit EOF.
            if n == 0 {
                // If we've searched everything up to the end of the buffer,
                // then there's nothing left to do.
                if self.end - self.pos == 0 {
                    return Ok(false);
                }
                // Even if we hit EOF, we might still have to search the
                // last line if it didn't contain a trailing terminator.
                self.lastnl = self.end;
                break;
            }
            self.lastnl =
                memrchr(self.eol, &self.buf[self.end..self.end + n])
                .map(|i| self.end + i + 1)
                .unwrap_or(0);
            self.end += n;
        }
        Ok(true)
    }
}

/// Returns true if and only if the given buffer is determined to be "binary"
/// or otherwise not contain text data that is usefully searchable.
///
/// Note that this may return both false positives and false negatives.
#[inline(always)]
pub fn is_binary(buf: &[u8], first: bool) -> bool {
    if first && buf.len() >= 4 && &buf[0..4] == b"%PDF" {
        return true;
    }
    memchr(b'\x00', buf).is_some()
}

/// Count the number of lines in the given buffer.
#[inline(never)]
pub fn count_lines(buf: &[u8], eol: u8) -> u64 {
    bytecount::count(buf, eol) as u64
}

/// Replaces a with b in buf.
#[allow(dead_code)]
fn replace_buf(buf: &mut [u8], a: u8, b: u8) {
    if a == b {
        return;
    }
    let mut pos = 0;
    while let Some(i) = memchr(a, &buf[pos..]).map(|i| pos + i) {
        buf[i] = b;
        pos = i + 1;
        while buf.get(pos) == Some(&a) {
            buf[pos] = b;
            pos += 1;
        }
    }
}

/// An "iterator" over lines in a particular buffer.
///
/// Idiomatic Rust would borrow the buffer and use it as internal state to
/// advance over the positions of each line. We neglect that approach to avoid
/// the borrow in the search code. (Because the borrow prevents composition
/// through other mutable methods.)
pub struct IterLines {
    eol: u8,
    pos: usize,
}

impl IterLines {
    /// Creates a new iterator over lines starting at the position given.
    ///
    /// The buffer is passed to the `next` method.
    #[inline(always)]
    pub fn new(eol: u8, start: usize) -> IterLines {
        IterLines {
            eol: eol,
            pos: start,
        }
    }

    /// Return the start and end position of the next line in the buffer. The
    /// buffer given should be the same on every call.
    ///
    /// The range returned includes the new line.
    #[inline(always)]
    pub fn next(&mut self, buf: &[u8]) -> Option<(usize, usize)> {
        match memchr(self.eol, &buf[self.pos..]) {
            None => {
                if self.pos < buf.len() {
                    let start = self.pos;
                    self.pos = buf.len();
                    Some((start, buf.len()))
                } else {
                    None
                }
            }
            Some(end) => {
                let start = self.pos;
                let end = self.pos + end + 1;
                self.pos = end;
                Some((start, end))
            }
        }
    }
}

/// Returns the starting index of the Nth line preceding `end`.
///
/// If `buf` is empty, then `0` is returned. If `count` is `0`, then `end` is
/// returned.
///
/// If `end` points at a new line in `buf`, then searching starts as if `end`
/// pointed immediately before the new line.
///
/// The position returned corresponds to the first byte in the given line.
#[inline(always)]
fn start_of_previous_lines(
    eol: u8,
    buf: &[u8],
    mut end: usize,
    mut count: usize,
) -> usize {
    // TODO(burntsushi): This function needs to be badly simplified. The case
    // analysis is impossible to follow.
    if buf[..end].is_empty() {
        return 0;
    }
    if count == 0 {
        return end;
    }
    if end == buf.len() {
        end -= 1;
    }
    if buf[end] == eol {
        if end == 0 {
            return end + 1;
        }
        end -= 1;
    }
    while count > 0 {
        if buf[end] == eol {
            count -= 1;
            if count == 0 {
                return end + 1;
            }
            if end == 0 {
                return end;
            }
            end -= 1;
            continue;
        }
        match memrchr(eol, &buf[..end]) {
            None => {
                return 0;
            }
            Some(i) => {
                count -= 1;
                end = i;
                if end == 0 {
                    if buf[end] == eol && count == 0 {
                        end += 1;
                    }
                    return end;
                }
                end -= 1;
            }
        }
    }
    end + 2
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::path::Path;

    use grep::GrepBuilder;
    use printer::Printer;
    use termcolor;

    use super::{InputBuffer, Searcher, start_of_previous_lines};

    const SHERLOCK: &'static str = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
";

    const CODE: &'static str = "\
extern crate snap;

use std::io;

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();

    // Wrap the stdin reader in a Snappy reader.
    let mut rdr = snap::Reader::new(stdin.lock());
    let mut wtr = stdout.lock();
    io::copy(&mut rdr, &mut wtr).expect(\"I/O operation failed\");
}
";

    fn hay(s: &str) -> io::Cursor<Vec<u8>> {
        io::Cursor::new(s.to_string().into_bytes())
    }

    fn test_path() -> &'static Path {
        &Path::new("/baz.rs")
    }

    type TestSearcher<'a> = Searcher<
        'a,
        io::Cursor<Vec<u8>>,
        termcolor::NoColor<Vec<u8>>,
    >;

    fn search_smallcap<F: FnMut(TestSearcher) -> TestSearcher>(
        pat: &str,
        haystack: &str,
        mut map: F,
    ) -> (u64, String) {
        let mut inp = InputBuffer::with_capacity(1);
        let outbuf = termcolor::NoColor::new(vec![]);
        let mut pp = Printer::new(outbuf).with_filename(true);
        let grep = GrepBuilder::new(pat).build().unwrap();
        let count = {
            let searcher = Searcher::new(
                &mut inp, &mut pp, &grep, test_path(), hay(haystack));
            map(searcher).run().unwrap()
        };
        (count, String::from_utf8(pp.into_inner().into_inner()).unwrap())
    }

    fn search<F: FnMut(TestSearcher) -> TestSearcher>(
        pat: &str,
        haystack: &str,
        mut map: F,
    ) -> (u64, String) {
        let mut inp = InputBuffer::with_capacity(4096);
        let outbuf = termcolor::NoColor::new(vec![]);
        let mut pp = Printer::new(outbuf).with_filename(true);
        let grep = GrepBuilder::new(pat).build().unwrap();
        let count = {
            let searcher = Searcher::new(
                &mut inp, &mut pp, &grep, test_path(), hay(haystack));
            map(searcher).run().unwrap()
        };
        (count, String::from_utf8(pp.into_inner().into_inner()).unwrap())
    }

    #[test]
    fn previous_lines() {
        let eol = b'\n';
        let text = SHERLOCK.as_bytes();
        assert_eq!(366, text.len());

        assert_eq!(0, start_of_previous_lines(eol, text, 366, 100));
        assert_eq!(366, start_of_previous_lines(eol, text, 366, 0));

        assert_eq!(321, start_of_previous_lines(eol, text, 366, 1));
        assert_eq!(321, start_of_previous_lines(eol, text, 365, 1));
        assert_eq!(321, start_of_previous_lines(eol, text, 364, 1));
        assert_eq!(321, start_of_previous_lines(eol, text, 322, 1));
        assert_eq!(321, start_of_previous_lines(eol, text, 321, 1));
        assert_eq!(258, start_of_previous_lines(eol, text, 320, 1));

        assert_eq!(258, start_of_previous_lines(eol, text, 366, 2));
        assert_eq!(258, start_of_previous_lines(eol, text, 365, 2));
        assert_eq!(258, start_of_previous_lines(eol, text, 364, 2));
        assert_eq!(258, start_of_previous_lines(eol, text, 322, 2));
        assert_eq!(258, start_of_previous_lines(eol, text, 321, 2));
        assert_eq!(193, start_of_previous_lines(eol, text, 320, 2));

        assert_eq!(65, start_of_previous_lines(eol, text, 66, 1));
        assert_eq!(0, start_of_previous_lines(eol, text, 66, 2));
        assert_eq!(64, start_of_previous_lines(eol, text, 64, 0));
        assert_eq!(0, start_of_previous_lines(eol, text, 64, 1));
        assert_eq!(0, start_of_previous_lines(eol, text, 64, 2));

        assert_eq!(0, start_of_previous_lines(eol, text, 0, 2));
        assert_eq!(0, start_of_previous_lines(eol, text, 0, 1));
    }

    #[test]
    fn previous_lines_short() {
        let eol = b'\n';
        let text = &b"a\nb\nc\nd\ne\nf\n"[..];
        assert_eq!(12, text.len());

        assert_eq!(10, start_of_previous_lines(eol, text, 12, 1));
        assert_eq!(8, start_of_previous_lines(eol, text, 12, 2));
        assert_eq!(6, start_of_previous_lines(eol, text, 12, 3));
        assert_eq!(4, start_of_previous_lines(eol, text, 12, 4));
        assert_eq!(2, start_of_previous_lines(eol, text, 12, 5));
        assert_eq!(0, start_of_previous_lines(eol, text, 12, 6));
        assert_eq!(0, start_of_previous_lines(eol, text, 12, 7));
        assert_eq!(10, start_of_previous_lines(eol, text, 11, 1));
        assert_eq!(8, start_of_previous_lines(eol, text, 11, 2));
        assert_eq!(6, start_of_previous_lines(eol, text, 11, 3));
        assert_eq!(4, start_of_previous_lines(eol, text, 11, 4));
        assert_eq!(2, start_of_previous_lines(eol, text, 11, 5));
        assert_eq!(0, start_of_previous_lines(eol, text, 11, 6));
        assert_eq!(0, start_of_previous_lines(eol, text, 11, 7));
        assert_eq!(10, start_of_previous_lines(eol, text, 10, 1));
        assert_eq!(8, start_of_previous_lines(eol, text, 10, 2));
        assert_eq!(6, start_of_previous_lines(eol, text, 10, 3));
        assert_eq!(4, start_of_previous_lines(eol, text, 10, 4));
        assert_eq!(2, start_of_previous_lines(eol, text, 10, 5));
        assert_eq!(0, start_of_previous_lines(eol, text, 10, 6));
        assert_eq!(0, start_of_previous_lines(eol, text, 10, 7));

        assert_eq!(8, start_of_previous_lines(eol, text, 9, 1));
        assert_eq!(8, start_of_previous_lines(eol, text, 8, 1));

        assert_eq!(6, start_of_previous_lines(eol, text, 7, 1));
        assert_eq!(6, start_of_previous_lines(eol, text, 6, 1));

        assert_eq!(4, start_of_previous_lines(eol, text, 5, 1));
        assert_eq!(4, start_of_previous_lines(eol, text, 4, 1));

        assert_eq!(2, start_of_previous_lines(eol, text, 3, 1));
        assert_eq!(2, start_of_previous_lines(eol, text, 2, 1));

        assert_eq!(0, start_of_previous_lines(eol, text, 1, 1));
        assert_eq!(0, start_of_previous_lines(eol, text, 0, 1));
    }

    #[test]
    fn previous_lines_empty() {
        let eol = b'\n';
        let text = &b"\n\n\nd\ne\nf\n"[..];
        assert_eq!(9, text.len());

        assert_eq!(7, start_of_previous_lines(eol, text, 9, 1));
        assert_eq!(5, start_of_previous_lines(eol, text, 9, 2));
        assert_eq!(3, start_of_previous_lines(eol, text, 9, 3));
        assert_eq!(2, start_of_previous_lines(eol, text, 9, 4));
        assert_eq!(1, start_of_previous_lines(eol, text, 9, 5));
        assert_eq!(0, start_of_previous_lines(eol, text, 9, 6));
        assert_eq!(0, start_of_previous_lines(eol, text, 9, 7));

        let text = &b"a\n\n\nd\ne\nf\n"[..];
        assert_eq!(10, text.len());

        assert_eq!(8, start_of_previous_lines(eol, text, 10, 1));
        assert_eq!(6, start_of_previous_lines(eol, text, 10, 2));
        assert_eq!(4, start_of_previous_lines(eol, text, 10, 3));
        assert_eq!(3, start_of_previous_lines(eol, text, 10, 4));
        assert_eq!(2, start_of_previous_lines(eol, text, 10, 5));
        assert_eq!(0, start_of_previous_lines(eol, text, 10, 6));
        assert_eq!(0, start_of_previous_lines(eol, text, 10, 7));
    }

    #[test]
    fn basic_search1() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s|s);
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:be, to a very large extent, the result of luck. Sherlock Holmes
");
    }

    #[test]
    fn binary() {
        let text = "Sherlock\n\x00Holmes\n";
        let (count, out) = search("Sherlock|Holmes", text, |s|s);
        assert_eq!(0, count);
        assert_eq!(out, "");
    }

    #[test]
    fn binary_text() {
        let text = "Sherlock\n\x00Holmes\n";
        let (count, out) = search("Sherlock|Holmes", text, |s| s.text(true));
        assert_eq!(2, count);
        assert_eq!(out, "/baz.rs:Sherlock\n/baz.rs:\x00Holmes\n");
    }

    #[test]
    fn line_numbers() {
        let (count, out) = search_smallcap(
            "Sherlock", SHERLOCK, |s| s.line_number(true));
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
");
    }

    #[test]
    fn count() {
        let (count, out) = search_smallcap(
            "Sherlock", SHERLOCK, |s| s.count(true));
        assert_eq!(2, count);
        assert_eq!(out, "/baz.rs:2\n");
    }

    #[test]
    fn byte_offset() {
        let (_, out) = search_smallcap(
            "Sherlock", SHERLOCK, |s| s.byte_offset(true));
        assert_eq!(out, "\
/baz.rs:0:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:129:be, to a very large extent, the result of luck. Sherlock Holmes
");
    }

    #[test]
    fn byte_offset_with_before_context() {
        let (_, out) = search_smallcap("dusted", SHERLOCK, |s| {
            s.line_number(true).byte_offset(true).before_context(2)
        });
        assert_eq!(out, "\
/baz.rs-3-129-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-193-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:258:but Doctor Watson has to have it taken out for him and dusted,
");
    }

    #[test]
    fn byte_offset_inverted() {
        let (_, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.invert_match(true).byte_offset(true)
        });
        assert_eq!(out, "\
/baz.rs:65:Holmeses, success in the province of detective work must always
/baz.rs:193:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:258:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:321:and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn count_matches() {
        let (_, out) = search_smallcap(
            "the", SHERLOCK, |s| s.count_matches(true));
        assert_eq!(out, "/baz.rs:4\n");
    }

    #[test]
    fn files_with_matches() {
        let (count, out) = search_smallcap(
            "Sherlock", SHERLOCK, |s| s.files_with_matches(true));
        assert_eq!(1, count);
        assert_eq!(out, "/baz.rs\n");
    }

    #[test]
    fn files_without_matches() {
        let (count, out) = search_smallcap(
            "zzzz", SHERLOCK, |s| s.files_without_matches(true));
        assert_eq!(0, count);
        assert_eq!(out, "/baz.rs\n");
    }

    #[test]
    fn max_count() {
        let (count, out) = search_smallcap(
            "Sherlock", SHERLOCK, |s| s.max_count(Some(1)));
        assert_eq!(1, count);
        assert_eq!(out, "\
/baz.rs:For the Doctor Watsons of this world, as opposed to the Sherlock
");
    }

    #[test]
    fn invert_match_max_count() {
        let (count, out) = search(
            "zzzz", SHERLOCK, |s| s.invert_match(true).max_count(Some(1)));
        assert_eq!(1, count);
        assert_eq!(out, "\
/baz.rs:For the Doctor Watsons of this world, as opposed to the Sherlock
");
    }

    #[test]
    fn invert_match() {
        let (count, out) = search_smallcap(
            "Sherlock", SHERLOCK, |s| s.invert_match(true));
        assert_eq!(4, count);
        assert_eq!(out, "\
/baz.rs:Holmeses, success in the province of detective work must always
/baz.rs:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn invert_match_line_numbers() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.invert_match(true).line_number(true)
        });
        assert_eq!(4, count);
        assert_eq!(out, "\
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs:4:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:6:and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn invert_match_count() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.invert_match(true).count(true)
        });
        assert_eq!(4, count);
        assert_eq!(out, "/baz.rs:4\n");
    }

    #[test]
    fn before_context_one1() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.line_number(true).before_context(1)
        });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
");
    }

    #[test]
    fn before_context_invert_one1() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.line_number(true).before_context(1).invert_match(true)
        });
        assert_eq!(4, count);
        assert_eq!(out, "\
/baz.rs-1-For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs:4:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:6:and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn before_context_invert_one2() {
        let (count, out) = search_smallcap(" a ", SHERLOCK, |s| {
            s.line_number(true).before_context(1).invert_match(true)
        });
        assert_eq!(3, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:2:Holmeses, success in the province of detective work must always
--
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
");
    }

    #[test]
    fn before_context_two1() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.line_number(true).before_context(2)
        });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
");
    }

    #[test]
    fn before_context_two2() {
        let (count, out) = search_smallcap("dusted", SHERLOCK, |s| {
            s.line_number(true).before_context(2)
        });
        assert_eq!(1, count);
        assert_eq!(out, "\
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
");
    }

    #[test]
    fn before_context_two3() {
        let (count, out) = search_smallcap(
            "success|attached", SHERLOCK, |s| {
                s.line_number(true).before_context(2)
            });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs-1-For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:2:Holmeses, success in the province of detective work must always
--
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs-5-but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:6:and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn before_context_two4() {
        let (count, out) = search("stdin", CODE, |s| {
            s.line_number(true).before_context(2)
        });
        assert_eq!(3, count);
        assert_eq!(out, "\
/baz.rs-4-
/baz.rs-5-fn main() {
/baz.rs:6:    let stdin = io::stdin();
/baz.rs-7-    let stdout = io::stdout();
/baz.rs-8-
/baz.rs:9:    // Wrap the stdin reader in a Snappy reader.
/baz.rs:10:    let mut rdr = snap::Reader::new(stdin.lock());
");
    }

    #[test]
    fn before_context_two5() {
        let (count, out) = search("stdout", CODE, |s| {
            s.line_number(true).before_context(2)
        });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs-5-fn main() {
/baz.rs-6-    let stdin = io::stdin();
/baz.rs:7:    let stdout = io::stdout();
--
/baz.rs-9-    // Wrap the stdin reader in a Snappy reader.
/baz.rs-10-    let mut rdr = snap::Reader::new(stdin.lock());
/baz.rs:11:    let mut wtr = stdout.lock();
");
    }

    #[test]
    fn before_context_three1() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
                s.line_number(true).before_context(3)
            });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
");
    }

    #[test]
    fn after_context_one1() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.line_number(true).after_context(1)
        });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
");
    }

    #[test]
    fn after_context_invert_one1() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.line_number(true).after_context(1).invert_match(true)
        });
        assert_eq!(4, count);
        assert_eq!(out, "\
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs:4:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs:6:and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn after_context_invert_one2() {
        let (count, out) = search_smallcap(" a ", SHERLOCK, |s| {
            s.line_number(true).after_context(1).invert_match(true)
        });
        assert_eq!(3, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
--
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs-6-and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn after_context_invert_one_max_count_two() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.line_number(true)
             .invert_match(true)
             .after_context(1)
             .max_count(Some(2))
        });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs:4:can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs-5-but Doctor Watson has to have it taken out for him and dusted,
");
    }

    #[test]
    fn after_context_two1() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.line_number(true).after_context(2)
        });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs-5-but Doctor Watson has to have it taken out for him and dusted,
");
    }

    #[test]
    fn after_context_two2() {
        let (count, out) = search_smallcap("dusted", SHERLOCK, |s| {
            s.line_number(true).after_context(2)
        });
        assert_eq!(1, count);
        assert_eq!(out, "\
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs-6-and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn after_context_two3() {
        let (count, out) = search_smallcap(
            "success|attached", SHERLOCK, |s| {
                s.line_number(true).after_context(2)
            });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:2:Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
--
/baz.rs:6:and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn after_context_two_max_count_two() {
        let (count, out) = search_smallcap(
            "Doctor", SHERLOCK, |s| {
                s.line_number(true).after_context(2).max_count(Some(2))
            });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs-3-be, to a very large extent, the result of luck. Sherlock Holmes
--
/baz.rs:5:but Doctor Watson has to have it taken out for him and dusted,
/baz.rs-6-and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn after_context_three1() {
        let (count, out) = search_smallcap("Sherlock", SHERLOCK, |s| {
            s.line_number(true).after_context(3)
        });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs-2-Holmeses, success in the province of detective work must always
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
/baz.rs-4-can extract a clew from a wisp of straw or a flake of cigar ash;
/baz.rs-5-but Doctor Watson has to have it taken out for him and dusted,
/baz.rs-6-and exhibited clearly, with a label attached.
");
    }

    #[test]
    fn before_after_context_two1() {
        let (count, out) = search(
            r"fn main|let mut rdr", CODE, |s| {
                s.line_number(true).after_context(2).before_context(2)
            });
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs-3-use std::io;
/baz.rs-4-
/baz.rs:5:fn main() {
/baz.rs-6-    let stdin = io::stdin();
/baz.rs-7-    let stdout = io::stdout();
/baz.rs-8-
/baz.rs-9-    // Wrap the stdin reader in a Snappy reader.
/baz.rs:10:    let mut rdr = snap::Reader::new(stdin.lock());
/baz.rs-11-    let mut wtr = stdout.lock();
/baz.rs-12-    io::copy(&mut rdr, &mut wtr).expect(\"I/O operation failed\");
");
    }
}
