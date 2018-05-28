use syntax::hir::{self, Hir, HirKind};

use {Error, Result};

/// Returns a new expression that is guaranteed to never match the given
/// ASCII character.
///
/// If the expression contains the literal byte, then an error is returned.
///
/// If `byte` is not an ASCII character (i.e., greater than `0x7F`), then this
/// function panics.
pub fn remove(expr: Hir, byte: u8) -> Result<Hir> {
    assert!(byte <= 0x7F);
    let chr = byte as char;
    assert!(chr.len_utf8() == 1);

    Ok(match expr.into_kind() {
        HirKind::Empty => Hir::empty(),
        HirKind::Literal(hir::Literal::Unicode(c)) => {
            if c == chr {
                return Err(Error::LiteralNotAllowed(chr));
            }
            Hir::literal(hir::Literal::Unicode(c))
        }
        HirKind::Literal(hir::Literal::Byte(b)) => {
            if b as char == chr {
                return Err(Error::LiteralNotAllowed(chr));
            }
            Hir::literal(hir::Literal::Byte(b))
        }
        HirKind::Class(hir::Class::Unicode(mut cls)) => {
            let remove = hir::ClassUnicode::new(Some(
                hir::ClassUnicodeRange::new(chr, chr),
            ));
            cls.difference(&remove);
            if cls.iter().next().is_none() {
                return Err(Error::LiteralNotAllowed(chr));
            }
            Hir::class(hir::Class::Unicode(cls))
        }
        HirKind::Class(hir::Class::Bytes(mut cls)) => {
            let remove = hir::ClassBytes::new(Some(
                hir::ClassBytesRange::new(byte, byte),
            ));
            cls.difference(&remove);
            if cls.iter().next().is_none() {
                return Err(Error::LiteralNotAllowed(chr));
            }
            Hir::class(hir::Class::Bytes(cls))
        }
        HirKind::Anchor(x) => Hir::anchor(x),
        HirKind::WordBoundary(x) => Hir::word_boundary(x),
        HirKind::Repetition(mut x) => {
            x.hir = Box::new(remove(*x.hir, byte)?);
            Hir::repetition(x)
        }
        HirKind::Group(mut x) => {
            x.hir = Box::new(remove(*x.hir, byte)?);
            Hir::group(x)
        }
        HirKind::Concat(xs) => {
            let xs = xs.into_iter()
                .map(|e| remove(e, byte))
                .collect::<Result<Vec<Hir>>>()?;
            Hir::concat(xs)
        }
        HirKind::Alternation(xs) => {
            let xs = xs.into_iter()
                .map(|e| remove(e, byte))
                .collect::<Result<Vec<Hir>>>()?;
            Hir::alternation(xs)
        }
    })
}
