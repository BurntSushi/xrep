/*!
An implementation of `grep-matcher`'s `Matcher` trait for
[PCRE2](https://www.pcre.org/).
*/

#![deny(missing_docs)]

extern crate grep_matcher;
extern crate pcre2;

pub use error::{Error, ErrorKind};
pub use matcher::{RegexCaptures, RegexMatcher, RegexMatcherBuilder};

mod error;
mod matcher;
