use std::ffi::OsStr;
use std::str;

use bstr::{ByteSlice, ByteVec};

/// A single state in the state machine used by `unescape`.
#[derive(Clone, Copy, Eq, PartialEq)]
enum State {
    /// The state after seeing a `\`.
    Escape,
    /// The state after seeing a `\x`.
    HexFirst,
    /// The state after seeing a `\x[0-9A-Fa-f]`.
    HexSecond(char),
    /// Default state.
    Literal,
}

/// Escapes arbitrary bytes into a human readable string.
///
/// This converts `\t`, `\r` and `\n` into their escaped forms. It also
/// converts the non-printable subset of ASCII in addition to invalid UTF-8
/// bytes to hexadecimal escape sequences. Everything else is left as is.
///
/// The dual of this routine is [`unescape`](fn.unescape.html).
///
/// # Example
///
/// This example shows how to convert a byte string that contains a `\n` and
/// invalid UTF-8 bytes into a `String`.
///
/// Pay special attention to the use of raw strings. That is, `r"\n"` is
/// equivalent to `"\\n"`.
///
/// ```
/// use grep_cli::escape;
///
/// assert_eq!(r"foo\nbar\xFFbaz", escape(b"foo\nbar\xFFbaz"));
/// ```
pub fn escape(bytes: &[u8]) -> String {
    let mut escaped = String::new();
    for (s, e, ch) in bytes.char_indices() {
        if ch == '\u{FFFD}' {
            for b in bytes[s..e].bytes() {
                escape_byte(b, &mut escaped);
            }
        } else {
            escape_char(ch, &mut escaped);
        }
    }
    escaped
}

/// Escapes an OS string into a human readable string.
///
/// This is like [`escape`](fn.escape.html), but accepts an OS string.
pub fn escape_os(string: &OsStr) -> String {
    escape(Vec::from_os_str_lossy(string).as_bytes())
}

/// Unescapes a string.
///
/// It supports a limited set of escape sequences:
///
/// * `\t`, `\r` and `\n` are mapped to their corresponding ASCII bytes.
/// * `\xZZ` hexadecimal escapes are mapped to their byte.
///
/// Everything else is left as is, including non-hexadecimal escapes like
/// `\xGG`.
///
/// This is useful when it is desirable for a command line argument to be
/// capable of specifying arbitrary bytes or otherwise make it easier to
/// specify non-printable characters.
///
/// The dual of this routine is [`escape`](fn.escape.html).
///
/// # Example
///
/// This example shows how to convert an escaped string (which is valid UTF-8)
/// into a corresponding sequence of bytes. Each escape sequence is mapped to
/// its bytes, which may include invalid UTF-8.
///
/// Pay special attention to the use of raw strings. That is, `r"\n"` is
/// equivalent to `"\\n"`.
///
/// ```
/// use grep_cli::unescape;
///
/// assert_eq!(&b"foo\nbar\xFFbaz"[..], &*unescape(r"foo\nbar\xFFbaz"));
/// ```
pub fn unescape(s: &str) -> Vec<u8> {
    use self::State::*;

    let mut bytes = vec![];
    let mut state = Literal;
    for c in s.chars() {
        match state {
            Escape => match c {
                '\\' => {
                    bytes.push(b'\\');
                    state = Literal;
                }
                'n' => {
                    bytes.push(b'\n');
                    state = Literal;
                }
                'r' => {
                    bytes.push(b'\r');
                    state = Literal;
                }
                't' => {
                    bytes.push(b'\t');
                    state = Literal;
                }
                'x' => {
                    state = HexFirst;
                }
                c => {
                    bytes.extend(format!(r"\{}", c).into_bytes());
                    state = Literal;
                }
            },
            HexFirst => match c {
                '0'..='9' | 'A'..='F' | 'a'..='f' => {
                    state = HexSecond(c);
                }
                c => {
                    bytes.extend(format!(r"\x{}", c).into_bytes());
                    state = Literal;
                }
            },
            HexSecond(first) => match c {
                '0'..='9' | 'A'..='F' | 'a'..='f' => {
                    let ordinal = format!("{}{}", first, c);
                    let byte = u8::from_str_radix(&ordinal, 16).unwrap();
                    bytes.push(byte);
                    state = Literal;
                }
                c => {
                    let original = format!(r"\x{}{}", first, c);
                    bytes.extend(original.into_bytes());
                    state = Literal;
                }
            },
            Literal => match c {
                '\\' => {
                    state = Escape;
                }
                c => {
                    bytes.extend(c.to_string().as_bytes());
                }
            },
        }
    }
    match state {
        Escape => bytes.push(b'\\'),
        HexFirst => bytes.extend(b"\\x"),
        HexSecond(c) => bytes.extend(format!("\\x{}", c).into_bytes()),
        Literal => {}
    }
    bytes
}

/// Unescapes an OS string.
///
/// This is like [`unescape`](fn.unescape.html), but accepts an OS string.
///
/// Note that this first lossily decodes the given OS string as UTF-8. That
/// is, an escaped string (the thing given) should be valid UTF-8.
pub fn unescape_os(string: &OsStr) -> Vec<u8> {
    unescape(&string.to_string_lossy())
}

/// Adds the given codepoint to the given string, escaping it if necessary.
fn escape_char(cp: char, into: &mut String) {
    if cp.is_ascii() {
        escape_byte(cp as u8, into);
    } else {
        into.push(cp);
    }
}

/// Adds the given byte to the given string, escaping it if necessary.
fn escape_byte(byte: u8, into: &mut String) {
    match byte {
        0x21..=0x5B | 0x5D..=0x7D => into.push(byte as char),
        b'\n' => into.push_str(r"\n"),
        b'\r' => into.push_str(r"\r"),
        b'\t' => into.push_str(r"\t"),
        b'\\' => into.push_str(r"\\"),
        _ => into.push_str(&format!(r"\x{:02X}", byte)),
    }
}

#[cfg(test)]
mod tests {
    use super::{escape, unescape};

    fn b(bytes: &'static [u8]) -> Vec<u8> {
        bytes.to_vec()
    }

    #[test]
    fn empty() {
        assert_eq!(b(b""), unescape(r""));
        assert_eq!(r"", escape(b""));
    }

    #[test]
    fn backslash() {
        assert_eq!(b(b"\\"), unescape(r"\\"));
        assert_eq!(r"\\", escape(b"\\"));
    }

    #[test]
    fn nul() {
        assert_eq!(b(b"\x00"), unescape(r"\x00"));
        assert_eq!(r"\x00", escape(b"\x00"));
    }

    #[test]
    fn nl() {
        assert_eq!(b(b"\n"), unescape(r"\n"));
        assert_eq!(r"\n", escape(b"\n"));
    }

    #[test]
    fn tab() {
        assert_eq!(b(b"\t"), unescape(r"\t"));
        assert_eq!(r"\t", escape(b"\t"));
    }

    #[test]
    fn carriage() {
        assert_eq!(b(b"\r"), unescape(r"\r"));
        assert_eq!(r"\r", escape(b"\r"));
    }

    #[test]
    fn nothing_simple() {
        assert_eq!(b(b"\\a"), unescape(r"\a"));
        assert_eq!(b(b"\\a"), unescape(r"\\a"));
        assert_eq!(r"\\a", escape(b"\\a"));
    }

    #[test]
    fn nothing_hex0() {
        assert_eq!(b(b"\\x"), unescape(r"\x"));
        assert_eq!(b(b"\\x"), unescape(r"\\x"));
        assert_eq!(r"\\x", escape(b"\\x"));
    }

    #[test]
    fn nothing_hex1() {
        assert_eq!(b(b"\\xz"), unescape(r"\xz"));
        assert_eq!(b(b"\\xz"), unescape(r"\\xz"));
        assert_eq!(r"\\xz", escape(b"\\xz"));
    }

    #[test]
    fn nothing_hex2() {
        assert_eq!(b(b"\\xzz"), unescape(r"\xzz"));
        assert_eq!(b(b"\\xzz"), unescape(r"\\xzz"));
        assert_eq!(r"\\xzz", escape(b"\\xzz"));
    }

    #[test]
    fn invalid_utf8() {
        assert_eq!(r"\xFF", escape(b"\xFF"));
        assert_eq!(r"a\xFFb", escape(b"a\xFFb"));
    }
}
