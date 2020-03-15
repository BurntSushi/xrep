/*!
An implementation of `grep-matcher`'s `Matcher` trait for Rust's regex engine.
*/

#![deny(missing_docs)]

extern crate aho_corasick;
extern crate bstr;
extern crate grep_matcher;
#[macro_use]
extern crate log;
extern crate regex;
extern crate regex_syntax;
extern crate thread_local;

pub use error::{Error, ErrorKind};
pub use matcher::{RegexCaptures, RegexMatcher, RegexMatcherBuilder};

mod ast;
mod config;
mod crlf;
mod error;
mod literal;
mod matcher;
mod multi;
mod non_matching;
mod strip;
mod util;
mod word;
