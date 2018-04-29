use std::borrow::Cow;
use std::fmt;
use std::io;
use std::path::Path;
use std::time;

use grep_matcher::{Captures, LineTerminator, Match, Matcher};
use grep_searcher::{
    LineIter,
    SinkError, SinkContext, SinkContextKind, SinkMatch,
};
#[cfg(feature = "serde1")]
use serde::{Serialize, Serializer};

/// A type for handling replacements while amortizing allocation.
pub struct Replacer<M: Matcher> {
    space: Option<Space<M>>,
}

struct Space<M: Matcher> {
    /// The place to store capture locations.
    caps: M::Captures,
    /// The place to write a replacement to.
    dst: Vec<u8>,
    /// The place to store match offsets in terms of `dst`.
    matches: Vec<Match>,
}

impl<M: Matcher> fmt::Debug for Replacer<M> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (dst, matches) = self.replacement().unwrap_or((&[], &[]));
        f.debug_struct("Replacer")
            .field("dst", &dst)
            .field("matches", &matches)
            .finish()
    }
}

impl<M: Matcher> Replacer<M> {
    /// Create a new replacer for use with a particular matcher.
    ///
    /// This constructor does not allocate. Instead, space for dealing with
    /// replacements is allocated lazily only when needed.
    pub fn new() -> Replacer<M> {
        Replacer { space: None }
    }

    /// Executes a replacement on the given subject string by replacing all
    /// matches with the given replacement. To access the result of the
    /// replacement, use the `replacement` method.
    ///
    /// This can fail if the underlying matcher reports an error.
    pub fn replace_all<'a>(
        &'a mut self,
        matcher: &M,
        subject: &[u8],
        replacement: &[u8],
    ) -> io::Result<()> {
        {
            let &mut Space {
                ref mut dst,
                ref mut caps,
                ref mut matches,
            } = self.allocate(matcher)?;
            dst.clear();
            matches.clear();

            matcher.replace_with_captures(
                subject,
                caps,
                dst,
                |caps, dst| {
                    let start = dst.len();
                    caps.interpolate(
                        |name| matcher.capture_index(name),
                        subject,
                        replacement,
                        dst,
                    );
                    let end = dst.len();
                    matches.push(Match::new(start, end));
                    true
                },
            ).map_err(io::Error::error_message)?;
        }
        Ok(())
    }

    /// Return the result of the prior replacement and the match offsets for
    /// all replacement occurrences within the returned replacement buffer.
    ///
    /// If no replacement has occurred then `None` is returned.
    pub fn replacement<'a>(&'a self) -> Option<(&'a [u8], &'a [Match])> {
        match self.space {
            None => None,
            Some(ref space) => {
                if space.matches.is_empty() {
                    None
                } else {
                    Some((&space.dst, &space.matches))
                }
            }
        }
    }

    /// Clear space used for performing a replacement.
    ///
    /// Subsequent calls to `replacement` after calling `clear` (but before
    /// executing another replacement) will always return `None`.
    pub fn clear(&mut self) {
        if let Some(ref mut space) = self.space {
            space.dst.clear();
            space.matches.clear();
        }
    }

    /// Allocate space for replacements when used with the given matcher and
    /// return a mutable reference to that space.
    ///
    /// This can fail if allocating space for capture locations from the given
    /// matcher fails.
    fn allocate(&mut self, matcher: &M) -> io::Result<&mut Space<M>> {
        if self.space.is_none() {
            let caps = matcher
                .new_captures()
                .map_err(io::Error::error_message)?;
            self.space = Some(Space {
                caps: caps,
                dst: vec![],
                matches: vec![],
            });
        }
        Ok(self.space.as_mut().unwrap())
    }
}

/// A simple layer of abstraction over either a match or a contextual line
/// reported by the searcher.
///
/// In particular, this provides an API that unions the `SinkMatch` and
/// `SinkContext` types while also exposing a list of all individual match
/// locations.
///
/// While this serves as a convenient mechanism to abstract over `SinkMatch`
/// and `SinkContext`, this also provides a way to abstract over replacements.
/// Namely, after a replacement, a `Sunk` value can be constructed using the
/// results of the replacement instead of the bytes reported directly by the
/// searcher.
#[derive(Debug)]
pub struct Sunk<'a> {
    bytes: &'a [u8],
    absolute_byte_offset: u64,
    line_number: Option<u64>,
    context_kind: Option<&'a SinkContextKind>,
    matches: &'a [Match],
    original_matches: &'a [Match],
}

impl<'a> Sunk<'a> {
    #[inline]
    pub fn empty() -> Sunk<'static> {
        Sunk {
            bytes: &[],
            absolute_byte_offset: 0,
            line_number: None,
            context_kind: None,
            matches: &[],
            original_matches: &[],
        }
    }

    #[inline]
    pub fn from_sink_match(
        sunk: &'a SinkMatch<'a>,
        original_matches: &'a [Match],
        replacement: Option<(&'a [u8], &'a [Match])>,
    ) -> Sunk<'a> {
        let (bytes, matches) = replacement.unwrap_or_else(|| {
            (sunk.bytes(), original_matches)
        });
        Sunk {
            bytes: bytes,
            absolute_byte_offset: sunk.absolute_byte_offset(),
            line_number: sunk.line_number(),
            context_kind: None,
            matches: matches,
            original_matches: original_matches,
        }
    }

    #[inline]
    pub fn from_sink_context(
        sunk: &'a SinkContext<'a>,
        original_matches: &'a [Match],
        replacement: Option<(&'a [u8], &'a [Match])>,
    ) -> Sunk<'a> {
        let (bytes, matches) = replacement.unwrap_or_else(|| {
            (sunk.bytes(), original_matches)
        });
        Sunk {
            bytes: bytes,
            absolute_byte_offset: sunk.absolute_byte_offset(),
            line_number: sunk.line_number(),
            context_kind: Some(sunk.kind()),
            matches: matches,
            original_matches: original_matches,
        }
    }

    #[inline]
    pub fn context_kind(&self) -> Option<&'a SinkContextKind> {
        self.context_kind
    }

    #[inline]
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    #[inline]
    pub fn matches(&self) -> &'a [Match] {
        self.matches
    }

    #[inline]
    pub fn original_matches(&self) -> &'a [Match] {
        self.original_matches
    }

    #[inline]
    pub fn lines(&self, line_term: u8) -> LineIter<'a> {
        LineIter::new(line_term, self.bytes())
    }

    #[inline]
    pub fn absolute_byte_offset(&self) -> u64 {
        self.absolute_byte_offset
    }

    #[inline]
    pub fn line_number(&self) -> Option<u64> {
        self.line_number
    }
}

/// A simple encapsulation of a file path used by a printer.
///
/// This represents any transforms that we might want to perform on the path,
/// such as converting it to valid UTF-8 and/or replacing its separator with
/// something else. This allows us to amortize work if we are printing the
/// file path for every match.
///
/// In the common case, no transformation is needed, which lets us avoid the
/// allocation. Typically, only Windows requires a transform, since we can't
/// access the raw bytes of a path directly and first need to lossily convert
/// to UTF-8. Windows is also typically where the path separator replacement
/// is used, e.g., in cygwin environments to use `/` instead of `\`.
///
/// Users of this type are expected to construct it from a normal `Path`
/// found in the standard library. It can then be written to any `io::Write`
/// implementation using the `as_bytes` method. This achieves platform
/// portability with a small cost: on Windows, paths that are not valid UTF-16
/// will not roundtrip correctly.
#[derive(Clone, Debug)]
pub struct PrinterPath<'a>(Cow<'a, [u8]>);

impl<'a> PrinterPath<'a> {
    /// Create a new path suitable for printing.
    pub fn new(path: &'a Path) -> PrinterPath<'a> {
        PrinterPath::new_impl(path)
    }

    #[cfg(unix)]
    fn new_impl(path: &'a Path) -> PrinterPath<'a> {
        use std::os::unix::ffi::OsStrExt;
        PrinterPath(Cow::Borrowed(path.as_os_str().as_bytes()))
    }

    #[cfg(not(unix))]
    fn new_impl(path: &'a Path) -> PrinterPath<'a> {
        PrinterPath(match path.to_string_lossy() {
            Cow::Owned(path) => Cow::Owned(path.into_bytes()),
            Cow::Borrowed(path) => Cow::Borrowed(path.as_bytes()),
        })
    }

    /// Create a new printer path from the given path which can be efficiently
    /// written to a writer without allocation.
    ///
    /// If the given separator is present, then any separators in `path` are
    /// replaced with it.
    pub fn with_separator(path: &'a Path, sep: Option<u8>) -> PrinterPath<'a> {
        let mut ppath = PrinterPath::new(path);
        if let Some(sep) = sep {
            ppath.replace_separator(sep);
        }
        ppath
    }

    /// Replace the path separator in this path with the given separator
    /// and do it in place. On Windows, both `/` and `\` are treated as
    /// path separators that are both replaced by `new_sep`. In all other
    /// environments, only `/` is treated as a path separator.
    fn replace_separator(&mut self, new_sep: u8) {
        let transformed_path: Vec<_> = self.as_bytes().iter().map(|&b| {
            if b == b'/' || (cfg!(windows) && b == b'\\') {
                new_sep
            } else {
                b
            }
        }).collect();
        self.0 = Cow::Owned(transformed_path);
    }

    /// Return the raw bytes for this path.
    pub fn as_bytes(&self) -> &[u8] {
        &*self.0
    }
}

/// A type that provides "nicer" Display and Serialize impls for
/// std::time::Duration. The serialization format should actually be compatible
/// with the Deserialize impl for std::time::Duration, since this type only
/// adds new fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NiceDuration(pub time::Duration);

impl fmt::Display for NiceDuration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:0.6}s", self.fractional_seconds())
    }
}

impl NiceDuration {
    /// Returns the number of seconds in this duration in fraction form.
    /// The number to the left of the decimal point is the number of seconds,
    /// and the number to the right is the number of milliseconds.
    fn fractional_seconds(&self) -> f64 {
        let fractional = (self.0.subsec_nanos() as f64) / 1_000_000_000.0;
        self.0.as_secs() as f64 + fractional
    }
}

#[cfg(feature = "serde1")]
impl Serialize for NiceDuration {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        let mut state = ser.serialize_struct("Duration", 2)?;
        state.serialize_field("secs", &self.0.as_secs())?;
        state.serialize_field("nanos", &self.0.subsec_nanos())?;
        state.serialize_field("human", &format!("{}", self))?;
        state.end()
    }
}

/// Trim prefix ASCII spaces from the given slice and return the corresponding
/// range.
///
/// This stops trimming a prefix as soon as it sees non-whitespace or a line
/// terminator.
pub fn trim_ascii_prefix_range(
    line_term: LineTerminator,
    slice: &[u8],
    range: Match,
) -> Match {
    fn is_space(b: u8) -> bool {
        match b {
            b'\t' | b'\n' | b'\x0B' | b'\x0C' | b'\r' | b' ' => true,
            _ => false,
        }
    }

    let count = slice[range]
        .iter()
        .take_while(|&&b| -> bool {
            is_space(b) && !line_term.as_bytes().contains(&b)
        })
        .count();
    range.with_start(range.start() + count)
}

/// Trim prefix ASCII spaces from the given slice and return the corresponding
/// sub-slice.
pub fn trim_ascii_prefix(line_term: LineTerminator, slice: &[u8]) -> &[u8] {
    let range = trim_ascii_prefix_range(
        line_term,
        slice,
        Match::new(0, slice.len()),
    );
    &slice[range]
}
