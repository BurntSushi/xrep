use std::collections::HashMap;
use std::cell::RefCell;
use std::sync::Arc;

use grep_matcher::{Match, Matcher, NoError};
use regex::bytes::{CaptureLocations, Regex};
use thread_local::CachedThreadLocal;

use config::ConfiguredHIR;
use error::Error;
use matcher::RegexCaptures;

/// A matcher for implementing "word match" semantics.
#[derive(Debug)]
pub struct WordMatcher {
    /// The regex which is roughly `(?:^|\W)(<original pattern>)(?:$|\W)`.
    regex: Regex,
    /// A map from capture group name to capture group index.
    names: HashMap<String, usize>,
    /// A reusable buffer for finding the match location of the inner group.
    locs: Arc<CachedThreadLocal<RefCell<CaptureLocations>>>,
}

impl Clone for WordMatcher {
    fn clone(&self) -> WordMatcher {
        // We implement Clone manually so that we get a fresh CachedThreadLocal
        // such that it can set its own thread owner. This permits each thread
        // usings `locs` to hit the fast path.
        WordMatcher {
            regex: self.regex.clone(),
            names: self.names.clone(),
            locs: Arc::new(CachedThreadLocal::new()),
        }
    }
}

impl WordMatcher {
    /// Create a new matcher from the given pattern that only produces matches
    /// that are considered "words."
    ///
    /// The given options are used to construct the regular expression
    /// internally.
    pub fn new(expr: &ConfiguredHIR) -> Result<WordMatcher, Error> {
        let word_expr = expr.with_pattern(|pat| {
            format!(r"(?:(?m:^)|\W)({})(?:(?m:$)|\W)", pat)
        })?;
        let regex = word_expr.regex()?;
        let locs = Arc::new(CachedThreadLocal::new());

        let mut names = HashMap::new();
        for (i, optional_name) in regex.capture_names().enumerate() {
            if let Some(name) = optional_name {
                names.insert(name.to_string(), i.checked_sub(1).unwrap());
            }
        }
        Ok(WordMatcher { regex, names, locs })
    }
}

impl Matcher for WordMatcher {
    type Captures = RegexCaptures;
    type Error = NoError;

    fn find_at(
        &self,
        haystack: &[u8],
        at: usize,
    ) -> Result<Option<Match>, NoError> {
        // To make this easy to get right, we extract captures here instead of
        // calling `find_at`. The actual match is at capture group `1` instead
        // of `0`. We *could* use `find_at` here and then trim the match after
        // the fact, but that's a bit harder to get right, and it's not clear
        // if it's worth it.

        let cell = self.locs.get_or(|| {
            Box::new(RefCell::new(self.regex.capture_locations()))
        });
        let mut caps = cell.borrow_mut();
        self.regex.captures_read_at(&mut caps, haystack, at);
        Ok(caps.get(1).map(|m| Match::new(m.0, m.1)))
    }

    fn new_captures(&self) -> Result<RegexCaptures, NoError> {
        Ok(RegexCaptures::with_offset(self.regex.capture_locations(), 1))
    }

    fn capture_count(&self) -> usize {
        self.regex.captures_len().checked_sub(1).unwrap()
    }

    fn capture_index(&self, name: &str) -> Option<usize> {
        self.names.get(name).map(|i| *i)
    }

    fn captures_at(
        &self,
        haystack: &[u8],
        at: usize,
        caps: &mut RegexCaptures,
    ) -> Result<bool, NoError> {
        let r = self.regex.captures_read_at(caps.locations(), haystack, at);
        Ok(r.is_some())
    }

    // We specifically do not implement other methods like find_iter or
    // captures_iter. Namely, the iter methods are guaranteed to be correct
    // by virtue of implementing find_at and captures_at above.
}

#[cfg(test)]
mod tests {
    use grep_matcher::{Captures, Match, Matcher};
    use config::Config;
    use super::WordMatcher;

    fn matcher(pattern: &str) -> WordMatcher {
        let chir = Config::default().hir(pattern).unwrap();
        WordMatcher::new(&chir).unwrap()
    }

    fn find(pattern: &str, haystack: &str) -> Option<(usize, usize)> {
        matcher(pattern)
            .find(haystack.as_bytes())
            .unwrap()
            .map(|m| (m.start(), m.end()))
    }

    fn find_by_caps(pattern: &str, haystack: &str) -> Option<(usize, usize)> {
        let m = matcher(pattern);
        let mut caps = m.new_captures().unwrap();
        if !m.captures(haystack.as_bytes(), &mut caps).unwrap() {
            None
        } else {
            caps.get(0).map(|m| (m.start(), m.end()))
        }
    }

    // Test that the standard `find` API reports offsets correctly.
    #[test]
    fn various_find() {
        assert_eq!(Some((0, 3)), find(r"foo", "foo"));
        assert_eq!(Some((0, 3)), find(r"foo", "foo("));
        assert_eq!(Some((1, 4)), find(r"foo", "!foo("));
        assert_eq!(None, find(r"foo", "!afoo("));

        assert_eq!(Some((0, 3)), find(r"foo", "foo☃"));
        assert_eq!(None, find(r"foo", "fooб"));
        // assert_eq!(Some((0, 3)), find(r"foo", "fooб"));

        // See: https://github.com/BurntSushi/ripgrep/issues/389
        assert_eq!(Some((0, 2)), find(r"-2", "-2"));
    }

    // Test that the captures API also reports offsets correctly, just as
    // find does. This exercises a different path in the code since captures
    // are handled differently.
    #[test]
    fn various_captures() {
        assert_eq!(Some((0, 3)), find_by_caps(r"foo", "foo"));
        assert_eq!(Some((0, 3)), find_by_caps(r"foo", "foo("));
        assert_eq!(Some((1, 4)), find_by_caps(r"foo", "!foo("));
        assert_eq!(None, find_by_caps(r"foo", "!afoo("));

        assert_eq!(Some((0, 3)), find_by_caps(r"foo", "foo☃"));
        assert_eq!(None, find_by_caps(r"foo", "fooб"));
        // assert_eq!(Some((0, 3)), find_by_caps(r"foo", "fooб"));

        // See: https://github.com/BurntSushi/ripgrep/issues/389
        assert_eq!(Some((0, 2)), find_by_caps(r"-2", "-2"));
    }

    // Test that the capture reporting methods work as advertised.
    #[test]
    fn capture_indexing() {
        let m = matcher(r"(a)(?P<foo>b)(c)");
        assert_eq!(4, m.capture_count());
        assert_eq!(Some(2), m.capture_index("foo"));

        let mut caps = m.new_captures().unwrap();
        assert_eq!(4, caps.len());

        assert!(m.captures(b"abc", &mut caps).unwrap());
        assert_eq!(caps.get(0), Some(Match::new(0, 3)));
        assert_eq!(caps.get(1), Some(Match::new(0, 1)));
        assert_eq!(caps.get(2), Some(Match::new(1, 2)));
        assert_eq!(caps.get(3), Some(Match::new(2, 3)));
        assert_eq!(caps.get(4), None);

        assert!(m.captures(b"#abc#", &mut caps).unwrap());
        assert_eq!(caps.get(0), Some(Match::new(1, 4)));
        assert_eq!(caps.get(1), Some(Match::new(1, 2)));
        assert_eq!(caps.get(2), Some(Match::new(2, 3)));
        assert_eq!(caps.get(3), Some(Match::new(3, 4)));
        assert_eq!(caps.get(4), None);
    }
}
