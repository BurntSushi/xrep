use memchr::{memchr, memrchr};
use regex::bytes::{Regex, RegexBuilder};
use syntax;

use literals::LiteralSets;
use nonl;
use syntax::Expr;
use word_boundary::strip_unicode_word_boundaries;
use Result;

/// A matched line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Match {
    start: usize,
    end: usize,
}

impl Match {
    /// Create a new empty match value.
    pub fn new() -> Match {
        Match::default()
    }

    /// Return the starting byte offset of the line that matched.
    #[inline]
    pub fn start(&self) -> usize {
        self.start
    }

    /// Return the ending byte offset of the line that matched.
    #[inline]
    pub fn end(&self) -> usize {
        self.end
    }
}

/// A fast line oriented regex searcher.
#[derive(Clone, Debug)]
pub struct Grep {
    re: Regex,
    required: Option<Regex>,
    opts: Options,
}

/// A builder for a grep searcher.
#[derive(Clone, Debug)]
pub struct GrepBuilder {
    pattern: String,
    opts: Options,
}

#[derive(Clone, Debug)]
struct Options {
    case_insensitive: bool,
    case_smart: bool,
    line_terminator: u8,
    size_limit: usize,
    dfa_size_limit: usize,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            case_insensitive: false,
            case_smart: false,
            line_terminator: b'\n',
            size_limit: 10 * (1 << 20),
            dfa_size_limit: 10 * (1 << 20),
        }
    }
}

impl GrepBuilder {
    /// Create a new builder for line searching.
    ///
    /// The pattern given should be a regular expression. The precise syntax
    /// supported is documented on the regex crate.
    pub fn new(pattern: &str) -> GrepBuilder {
        GrepBuilder {
            pattern: pattern.to_string(),
            opts: Options::default(),
        }
    }

    /// Set the line terminator.
    ///
    /// The line terminator can be any ASCII character and serves to delineate
    /// the match boundaries in the text searched.
    ///
    /// This panics if `ascii_byte` is greater than `0x7F` (i.e., not ASCII).
    pub fn line_terminator(mut self, ascii_byte: u8) -> GrepBuilder {
        assert!(ascii_byte <= 0x7F);
        self.opts.line_terminator = ascii_byte;
        self
    }

    /// Set the case sensitive flag (`i`) on the regex.
    pub fn case_insensitive(mut self, yes: bool) -> GrepBuilder {
        self.opts.case_insensitive = yes;
        self
    }

    /// Whether to enable smart case search or not (disabled by default).
    ///
    /// Smart case uses case insensitive search if the regex is contains all
    /// lowercase literal characters. Otherwise, a case sensitive search is
    /// used instead.
    ///
    /// Enabling the case_insensitive flag overrides this.
    pub fn case_smart(mut self, yes: bool) -> GrepBuilder {
        self.opts.case_smart = yes;
        self
    }

    /// Set the approximate size limit of the compiled regular expression.
    ///
    /// This roughly corresponds to the number of bytes occupied by a
    /// single compiled program. If the program exceeds this number, then a
    /// compilation error is returned.
    pub fn size_limit(mut self, limit: usize) -> GrepBuilder {
        self.opts.size_limit = limit;
        self
    }

    /// Set the approximate size of the cache used by the DFA.
    ///
    /// This roughly corresponds to the number of bytes that the DFA will use
    /// while searching.
    ///
    /// Note that this is a per thread limit. There is no way to set a global
    /// limit. In particular, if a regex is used from multiple threads
    /// simulanteously, then each thread may use up to the number of bytes
    /// specified here.
    pub fn dfa_size_limit(mut self, limit: usize) -> GrepBuilder {
        self.opts.dfa_size_limit = limit;
        self
    }

    /// Create a line searcher.
    ///
    /// If there was a problem parsing or compiling the regex with the given
    /// options, then an error is returned.
    pub fn build(self) -> Result<Grep> {
        let expr = try!(self.parse());
        let literals = LiteralSets::create(&expr);
        let re = try!(self.regex(&expr));
        let required = match literals.to_regex_builder() {
            Some(builder) => Some(try!(self.regex_build(builder))),
            None => {
                match strip_unicode_word_boundaries(&expr) {
                    None => None,
                    Some(expr) => {
                        debug!("Stripped Unicode word boundaries. \
                                New AST:\n{:?}", expr);
                        self.regex(&expr).ok()
                    }
                }
            }
        };
        Ok(Grep {
            re: re,
            required: required,
            opts: self.opts,
        })
    }

    /// Creates a new regex from the given expression with the current
    /// configuration.
    fn regex(&self, expr: &Expr) -> Result<Regex> {
        let mut builder = RegexBuilder::new(&expr.to_string());
        builder.unicode(true);
        self.regex_build(builder)
    }

    /// Builds a new regex from the given builder using the caller's settings.
    fn regex_build(&self, mut builder: RegexBuilder) -> Result<Regex> {
        builder
            .multi_line(true)
            .size_limit(self.opts.size_limit)
            .dfa_size_limit(self.opts.dfa_size_limit)
            .build()
            .map_err(From::from)
    }

    /// Parses the underlying pattern and ensures the pattern can never match
    /// the line terminator.
    fn parse(&self) -> Result<syntax::Expr> {
        let expr =
            try!(syntax::ExprBuilder::new()
                 .allow_bytes(true)
                 .unicode(true)
                 .case_insensitive(try!(self.is_case_insensitive()))
                 .parse(&self.pattern));
        let expr = try!(nonl::remove(expr, self.opts.line_terminator));
        debug!("regex ast:\n{:#?}", expr);
        Ok(expr)
    }

    /// Determines whether the case insensitive flag should be enabled or not.
    ///
    /// An error is returned if the regex could not be parsed.
    fn is_case_insensitive(&self) -> Result<bool> {
        if self.opts.case_insensitive {
            return Ok(true);
        }
        if !self.opts.case_smart {
            return Ok(false);
        }
        let expr =
            try!(syntax::ExprBuilder::new()
                 .allow_bytes(true)
                 .unicode(true)
                 .parse(&self.pattern));
        Ok(!has_uppercase_literal(&expr))
    }
}

impl Grep {
    /// Returns a reference to the underlying regex used by the searcher.
    pub fn regex(&self) -> &Regex {
        &self.re
    }

    /// Returns an iterator over all matches in the given buffer.
    pub fn iter<'b, 's>(&'s self, buf: &'b [u8]) -> Iter<'b, 's> {
        Iter {
            searcher: self,
            buf: buf,
            start: 0,
        }
    }

    /// Fills in the next line that matches in the given buffer starting at
    /// the position given.
    ///
    /// If no match could be found, `false` is returned, otherwise, `true` is
    /// returned.
    pub fn read_match(
        &self,
        mat: &mut Match,
        buf: &[u8],
        mut start: usize,
    ) -> bool {
        if start >= buf.len() {
            return false;
        }
        if let Some(ref req) = self.required {
            while start < buf.len() {
                let e = match req.shortest_match(&buf[start..]) {
                    None => return false,
                    Some(e) => start + e,
                };
                let (prevnl, nextnl) = self.find_line(buf, e, e);
                match self.re.shortest_match(&buf[prevnl..nextnl]) {
                    None => {
                        start = nextnl;
                        continue;
                    }
                    Some(_) => {
                        self.fill_match(mat, prevnl, nextnl);
                        return true;
                    }
                }
            }
            false
        } else {
            let e = match self.re.shortest_match(&buf[start..]) {
                None => return false,
                Some(e) => start + e,
            };
            let (s, e) = self.find_line(buf, e, e);
            self.fill_match(mat, s, e);
            true
        }
    }

    fn fill_match(&self, mat: &mut Match, start: usize, end: usize) {
        mat.start = start;
        mat.end = end;
    }

    fn find_line(&self, buf: &[u8], s: usize, e: usize) -> (usize, usize) {
        (self.find_line_start(buf, s), self.find_line_end(buf, e))
    }

    fn find_line_start(&self, buf: &[u8], pos: usize) -> usize {
        memrchr(self.opts.line_terminator, &buf[0..pos]).map_or(0, |i| i + 1)
    }

    fn find_line_end(&self, buf: &[u8], pos: usize) -> usize {
        memchr(self.opts.line_terminator, &buf[pos..])
            .map_or(buf.len(), |i| pos + i + 1)
    }
}

/// An iterator over all matches in a particular buffer.
///
/// `'b` refers to the lifetime of the buffer, and `'s` refers to the lifetime
/// of the searcher.
pub struct Iter<'b, 's> {
    searcher: &'s Grep,
    buf: &'b [u8],
    start: usize,
}

impl<'b, 's> Iterator for Iter<'b, 's> {
    type Item = Match;

    fn next(&mut self) -> Option<Match> {
        let mut mat = Match::default();
        if !self.searcher.read_match(&mut mat, self.buf, self.start) {
            self.start = self.buf.len();
            return None;
        }
        self.start = mat.end;
        Some(mat)
    }
}

fn has_uppercase_literal(expr: &Expr) -> bool {
    use syntax::Expr::*;
    fn byte_is_upper(b: u8) -> bool { b'A' <= b && b <= b'Z' }
    match *expr {
        Literal { ref chars, casei } => {
            casei || chars.iter().any(|c| c.is_uppercase())
        }
        LiteralBytes { ref bytes, casei } => {
            casei || bytes.iter().any(|&b| byte_is_upper(b))
        }
        Class(ref ranges) => {
            for r in ranges {
                if r.start.is_uppercase() || r.end.is_uppercase() {
                    return true;
                }
            }
            false
        }
        ClassBytes(ref ranges) => {
            for r in ranges {
                if byte_is_upper(r.start) || byte_is_upper(r.end) {
                    return true;
                }
            }
            false
        }
        Group { ref e, .. } => has_uppercase_literal(e),
        Repeat { ref e, .. } => has_uppercase_literal(e),
        Concat(ref es) => es.iter().any(has_uppercase_literal),
        Alternate(ref es) => es.iter().any(has_uppercase_literal),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused_imports)]

    use memchr::{memchr, memrchr};
    use regex::bytes::Regex;

    use super::{GrepBuilder, Match};

    static SHERLOCK: &'static [u8] = include_bytes!("./data/sherlock.txt");

    #[allow(dead_code)]
    fn s(bytes: &[u8]) -> String {
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn find_lines(pat: &str, haystack: &[u8]) -> Vec<Match> {
        let re = Regex::new(pat).unwrap();
        let mut lines = vec![];
        for m in re.find_iter(haystack) {
            let start = memrchr(b'\n', &haystack[..m.start()])
                        .map_or(0, |i| i + 1);
            let end = memchr(b'\n', &haystack[m.end()..])
                      .map_or(haystack.len(), |i| m.end() + i + 1);
            lines.push(Match {
                start: start,
                end: end,
            });
        }
        lines
    }

    fn grep_lines(pat: &str, haystack: &[u8]) -> Vec<Match> {
        let g = GrepBuilder::new(pat).build().unwrap();
        g.iter(haystack).collect()
    }

    #[test]
    fn buffered_literal() {
        let expected = find_lines("Sherlock Holmes", SHERLOCK);
        let got = grep_lines("Sherlock Holmes", SHERLOCK);
        assert_eq!(expected.len(), got.len());
        assert_eq!(expected, got);
    }
}
