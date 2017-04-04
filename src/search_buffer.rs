/*!
The `search_buffer` module is responsible for searching a single file all in a
single buffer. Typically, the source of the buffer is a memory map. This can
be useful for when memory maps are faster than streaming search.

Note that this module doesn't quite support everything that `search_stream`
does. Notably, showing contexts.
*/
use std::cmp;
use std::path::Path;

use grep::Grep;
use termcolor::WriteColor;

use printer::Printer;
use search_stream::{IterLines, Options, count_lines, is_binary};

pub struct BufferSearcher<'a, W: 'a> {
    opts: Options,
    printer: &'a mut Printer<W>,
    grep: &'a Grep,
    path: &'a Path,
    buf: &'a [u8],
    match_count: u64,
    line_count: Option<u64>,
    last_line: usize,
}

impl<'a, W: WriteColor> BufferSearcher<'a, W> {
    pub fn new(
        printer: &'a mut Printer<W>,
        grep: &'a Grep,
        path: &'a Path,
        buf: &'a [u8],
    ) -> BufferSearcher<'a, W> {
        BufferSearcher {
            opts: Options::default(),
            printer: printer,
            grep: grep,
            path: path,
            buf: buf,
            match_count: 0,
            line_count: None,
            last_line: 0,
        }
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

    /// If enabled, searching will print the path of files that *don't* match
    /// the given pattern.
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
        self
    }

    #[inline(never)]
    pub fn run(mut self) -> u64 {
        let binary_upto = cmp::min(10240, self.buf.len());
        if !self.opts.text && is_binary(&self.buf[..binary_upto], true) {
            return 0;
        }

        self.match_count = 0;
        self.line_count = if self.opts.line_number { Some(0) } else { None };
        let mut last_end = 0;
        for m in self.grep.iter(self.buf) {
            if self.opts.invert_match {
                self.print_inverted_matches(last_end, m.start());
            } else {
                self.print_match(m.start(), m.end());
            }
            last_end = m.end();
            if self.opts.terminate(self.match_count) {
                break;
            }
        }
        if self.opts.invert_match && !self.opts.terminate(self.match_count) {
            let upto = self.buf.len();
            self.print_inverted_matches(last_end, upto);
        }
        if self.opts.count && self.match_count > 0 {
            self.printer.path_count(self.path, self.match_count);
        }
        if self.opts.files_with_matches && self.match_count > 0 {
            self.printer.path(self.path);
        }
        if self.opts.files_without_matches && self.match_count == 0 {
            self.printer.path(self.path);
        }
        self.match_count
    }

    #[inline(always)]
    pub fn print_match(&mut self, start: usize, end: usize) {
        self.match_count += 1;
        if self.opts.skip_matches() {
            return;
        }
        self.count_lines(start);
        self.add_line(end);
        self.printer.matched(
            self.grep.regex(), self.path, self.buf,
            start, end, self.line_count);
    }

    #[inline(always)]
    fn print_inverted_matches(&mut self, start: usize, end: usize) {
        debug_assert!(self.opts.invert_match);
        let mut it = IterLines::new(self.opts.eol, start);
        while let Some((s, e)) = it.next(&self.buf[..end]) {
            if self.opts.terminate(self.match_count) {
                return;
            }
            self.print_match(s, e);
        }
    }

    #[inline(always)]
    fn count_lines(&mut self, upto: usize) {
        if let Some(ref mut line_count) = self.line_count {
            *line_count += count_lines(
                &self.buf[self.last_line..upto], self.opts.eol);
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use grep::GrepBuilder;

    use printer::Printer;
    use termcolor;

    use super::BufferSearcher;

    const SHERLOCK: &'static str = "\
For the Doctor Watsons of this world, as opposed to the Sherlock
Holmeses, success in the province of detective work must always
be, to a very large extent, the result of luck. Sherlock Holmes
can extract a clew from a wisp of straw or a flake of cigar ash;
but Doctor Watson has to have it taken out for him and dusted,
and exhibited clearly, with a label attached.\
";

    fn test_path() -> &'static Path {
        &Path::new("/baz.rs")
    }

    type TestSearcher<'a> = BufferSearcher<'a, termcolor::NoColor<Vec<u8>>>;

    fn search<F: FnMut(TestSearcher) -> TestSearcher>(
        pat: &str,
        haystack: &str,
        mut map: F,
    ) -> (u64, String) {
        let outbuf = termcolor::NoColor::new(vec![]);
        let mut pp = Printer::new(outbuf).with_filename(true);
        let grep = GrepBuilder::new(pat).build().unwrap();
        let count = {
            let searcher = BufferSearcher::new(
                &mut pp, &grep, test_path(), haystack.as_bytes());
            map(searcher).run()
        };
        (count, String::from_utf8(pp.into_inner().into_inner()).unwrap())
    }

    #[test]
    fn basic_search() {
        let (count, out) = search("Sherlock", SHERLOCK, |s|s);
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
        let (count, out) = search(
            "Sherlock", SHERLOCK, |s| s.line_number(true));
        assert_eq!(2, count);
        assert_eq!(out, "\
/baz.rs:1:For the Doctor Watsons of this world, as opposed to the Sherlock
/baz.rs:3:be, to a very large extent, the result of luck. Sherlock Holmes
");
    }

    #[test]
    fn count() {
        let (count, out) = search(
            "Sherlock", SHERLOCK, |s| s.count(true));
        assert_eq!(2, count);
        assert_eq!(out, "/baz.rs:2\n");
    }

    #[test]
    fn files_with_matches() {
        let (count, out) = search(
            "Sherlock", SHERLOCK, |s| s.files_with_matches(true));
        assert_eq!(1, count);
        assert_eq!(out, "/baz.rs\n");
    }

    #[test]
    fn files_without_matches() {
        let (count, out) = search(
            "zzzz", SHERLOCK, |s| s.files_without_matches(true));
        assert_eq!(0, count);
        assert_eq!(out, "/baz.rs\n");
    }

    #[test]
    fn max_count() {
        let (count, out) = search(
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
        let (count, out) = search(
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
        let (count, out) = search("Sherlock", SHERLOCK, |s| {
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
        let (count, out) = search("Sherlock", SHERLOCK, |s| {
            s.invert_match(true).count(true)
        });
        assert_eq!(4, count);
        assert_eq!(out, "/baz.rs:4\n");
    }
}
