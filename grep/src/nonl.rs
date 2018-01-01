use syntax::Expr;

use {Error, Result};

/// Returns a new expression that is guaranteed to never match the given
/// ASCII character.
///
/// If the expression contains the literal byte, then an error is returned.
///
/// If `byte` is not an ASCII character (i.e., greater than `0x7F`), then this
/// function panics.
pub fn remove(expr: Expr, byte: u8) -> Result<Expr> {
    // TODO(burntsushi): There is a bug in this routine where only `\n` is
    // handled correctly. Namely, `AnyChar` and `AnyByte` need to be translated
    // to proper character classes instead of the special `AnyCharNoNL` and
    // `AnyByteNoNL` classes.
    use syntax::Expr::*;
    assert!(byte <= 0x7F);
    let chr = byte as char;
    assert!(chr.len_utf8() == 1);

    Ok(match expr {
        Literal { chars, casei } => {
            if chars.iter().position(|&c| c == chr).is_some() {
                return Err(Error::LiteralNotAllowed(chr));
            }
            Literal { chars: chars, casei: casei }
        }
        LiteralBytes { bytes, casei } => {
            if bytes.iter().position(|&b| b == byte).is_some() {
                return Err(Error::LiteralNotAllowed(chr));
            }
            LiteralBytes { bytes: bytes, casei: casei }
        }
        AnyChar => AnyCharNoNL,
        AnyByte => AnyByteNoNL,
        Class(mut cls) => {
            cls.remove(chr);
            Class(cls)
        }
        ClassBytes(mut cls) => {
            cls.remove(byte);
            ClassBytes(cls)
        }
        Group { e, i, name } => {
            Group {
                e: Box::new(remove(*e, byte)?),
                i: i,
                name: name,
            }
        }
        Repeat { e, r, greedy } => {
            Repeat {
                e: Box::new(remove(*e, byte)?),
                r: r,
                greedy: greedy,
            }
        }
        Concat(exprs) => {
            Concat(exprs.into_iter().map(|e| remove(e, byte)).collect::<Result<Vec<Expr>>>()?)
        }
        Alternate(exprs) => {
            Alternate(exprs.into_iter().map(|e| remove(e, byte)).collect::<Result<Vec<Expr>>>()?)
        }
        e => e,
    })
}
