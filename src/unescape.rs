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

/// Unescapes a string given on the command line. It supports a limited set of
/// escape sequences:
///
/// * `\t`, `\r` and `\n` are mapped to their corresponding ASCII bytes.
/// * `\xZZ` hexadecimal escapes are mapped to their byte.
pub fn unescape(s: &str) -> Vec<u8> {
    use self::State::*;

    let mut bytes = vec![];
    let mut state = Literal;
    for c in s.chars() {
        match state {
            Escape => {
                match c {
                    'n' => { bytes.push(b'\n'); state = Literal; }
                    'r' => { bytes.push(b'\r'); state = Literal; }
                    't' => { bytes.push(b'\t'); state = Literal; }
                    'x' => { state = HexFirst; }
                    c => {
                        bytes.extend(format!(r"\{}", c).into_bytes());
                        state = Literal;
                    }
                }
            }
            HexFirst => {
                match c {
                    '0'...'9' | 'A'...'F' | 'a'...'f' => {
                        state = HexSecond(c);
                    }
                    c => {
                        bytes.extend(format!(r"\x{}", c).into_bytes());
                        state = Literal;
                    }
                }
            }
            HexSecond(first) => {
                match c {
                    '0'...'9' | 'A'...'F' | 'a'...'f' => {
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
                }
            }
            Literal => {
                match c {
                    '\\' => { state = Escape; }
                    c => { bytes.extend(c.to_string().as_bytes()); }
                }
            }
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

#[cfg(test)]
mod tests {
    use super::unescape;

    fn b(bytes: &'static [u8]) -> Vec<u8> {
        bytes.to_vec()
    }

    #[test]
    fn unescape_nul() {
        assert_eq!(b(b"\x00"), unescape(r"\x00"));
    }

    #[test]
    fn unescape_nl() {
        assert_eq!(b(b"\n"), unescape(r"\n"));
    }

    #[test]
    fn unescape_tab() {
        assert_eq!(b(b"\t"), unescape(r"\t"));
    }

    #[test]
    fn unescape_carriage() {
        assert_eq!(b(b"\r"), unescape(r"\r"));
    }

    #[test]
    fn unescape_nothing_simple() {
        assert_eq!(b(b"\\a"), unescape(r"\a"));
    }

    #[test]
    fn unescape_nothing_hex0() {
        assert_eq!(b(b"\\x"), unescape(r"\x"));
    }

    #[test]
    fn unescape_nothing_hex1() {
        assert_eq!(b(b"\\xz"), unescape(r"\xz"));
    }

    #[test]
    fn unescape_nothing_hex2() {
        assert_eq!(b(b"\\xzz"), unescape(r"\xzz"));
    }
}
