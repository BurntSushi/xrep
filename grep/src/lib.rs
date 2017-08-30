#![deny(missing_docs)]

/*!
A fast line oriented regex searcher.
*/

#[macro_use]
extern crate log;
extern crate memchr;
extern crate regex;
extern crate regex_syntax as syntax;

use std::error;
use std::fmt;
use std::result;

pub use search::{Grep, GrepBuilder, Iter, Match};

mod literals;
mod nonl;
mod search;
mod word_boundary;

/// Result is a convenient type alias that fixes the type of the error to
/// the `Error` type defined in this crate.
pub type Result<T> = result::Result<T, Error>;

/// Error enumerates the list of possible error conditions when building or
/// using a `Grep` line searcher.
#[derive(Debug)]
pub enum Error {
    /// An error from parsing or compiling a regex.
    Regex(GrepError),
    /// This error occurs when an illegal literal was found in the regex
    /// pattern. For example, if the line terminator is `\n` and the regex
    /// pattern is `\w+\n\w+`, then the presence of `\n` will cause this error.
    LiteralNotAllowed(char),
    /// An unused enum variant that indicates this enum may be expanded in
    /// the future and therefore should not be exhaustively matched.
    #[doc(hidden)]
    __Nonexhaustive,
}

impl error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::Regex(ref err) => err.description(),
            Error::LiteralNotAllowed(_) => "use of forbidden literal",
            Error::__Nonexhaustive => unreachable!(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::Regex(ref err) => err.cause(),
            _ => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Regex(ref err) => err.fmt(f),
            Error::LiteralNotAllowed(chr) => {
                write!(f, "Literal {:?} not allowed.", chr)
            }
            Error::__Nonexhaustive => unreachable!(),
        }
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Error {
        let message = GrepError::format_regex_error(String::from("Regex error"), err);
        Error::Regex(GrepError { description: message })
    }
}

impl From<syntax::Error> for Error {
    fn from(err: syntax::Error) -> Error {
        let message = GrepError::format_syntax_error(String::from("Regex syntax error"), err);
        Error::Regex(GrepError { description: message })
    }
}

/// GrepError wraps an error from parsing or compiling a regex
/// to provide a custom error message.
#[derive(Debug)]
pub struct GrepError {
    description: String
}

impl GrepError {
    fn truncate_regex_error(err: String) -> String {
        let mut error_message = err;
        let offset = error_message.find("at character").unwrap_or(error_message.len());
        return error_message.drain(..offset).collect();
    }

    fn format_regex_error(err_type: String, err: regex::Error) -> String {
        format!("{} caused by: {}", err_type, GrepError::truncate_regex_error(err.to_string()))
    }

    fn format_syntax_error(err_type: String, err: syntax::Error) -> String {
        format!("{} caused by: {}", err_type, GrepError::truncate_regex_error(err.to_string()))
    }
}

impl fmt::Display for GrepError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description)
    }
}

impl error::Error for GrepError {
    fn description(&self) -> &str {
        &self.description
    }
}
