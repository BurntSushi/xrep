/*!
An implementation of `grep-matcher`'s `Matcher` trait for Rust's regex engine.
*/

#![deny(missing_docs)]

extern crate grep_matcher;
#[macro_use]
extern crate log;
extern crate regex;
extern crate regex_syntax;
extern crate thread_local;
extern crate utf8_ranges;

pub use error::{Error, ErrorKind};
pub use matcher::{RegexCaptures, RegexMatcher, RegexMatcherBuilder};

mod ast;
mod config;
mod crlf;
mod error;
mod literal;
mod matcher;
mod non_matching;
mod strip;
mod util;
mod word;
