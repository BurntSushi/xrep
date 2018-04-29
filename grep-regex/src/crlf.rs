use regex_syntax::hir::{self, Hir, HirKind};

/// Substitutes all occurrences of multi-line enabled `$` with `(?:\r?$)`.
///
/// This does not preserve the exact semantics of the given expression,
/// however, it does have the useful property that anything that matched the
/// given expression will also match the returned expression. The difference is
/// that the returned expression can match possibly other things as well.
///
/// The principle reason why we do this is because the underlying regex engine
/// doesn't support CRLF aware `$` look-around. It's planned to fix it at that
/// level, but we perform this kludge in the mean time.
///
/// Note that while the match preserving semantics are nice and neat, the
/// match position semantics are quite a bit messier. Namely, `$` only ever
/// matches the position between characters where as `\r??` can match a
/// character and change the offset. This is regretable, but works out pretty
/// nicely in most cases, especially when a match is limited to a single line.
pub fn crlfify(expr: Hir) -> Hir {
    match expr.into_kind() {
        HirKind::Anchor(hir::Anchor::EndLine) => {
            let concat = Hir::concat(vec![
                Hir::repetition(hir::Repetition {
                    kind: hir::RepetitionKind::ZeroOrOne,
                    greedy: false,
                    hir: Box::new(Hir::literal(hir::Literal::Unicode('\r'))),
                }),
                Hir::anchor(hir::Anchor::EndLine),
            ]);
            Hir::group(hir::Group {
                kind: hir::GroupKind::NonCapturing,
                hir: Box::new(concat),
            })
        }
        HirKind::Empty => Hir::empty(),
        HirKind::Literal(x) => Hir::literal(x),
        HirKind::Class(x) => Hir::class(x),
        HirKind::Anchor(x) => Hir::anchor(x),
        HirKind::WordBoundary(x) => Hir::word_boundary(x),
        HirKind::Repetition(mut x) => {
            x.hir = Box::new(crlfify(*x.hir));
            Hir::repetition(x)
        }
        HirKind::Group(mut x) => {
            x.hir = Box::new(crlfify(*x.hir));
            Hir::group(x)
        }
        HirKind::Concat(xs) => {
            Hir::concat(xs.into_iter().map(crlfify).collect())
        }
        HirKind::Alternation(xs) => {
            Hir::alternation(xs.into_iter().map(crlfify).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use regex_syntax::Parser;
    use super::crlfify;

    fn roundtrip(pattern: &str) -> String {
        let expr1 = Parser::new().parse(pattern).unwrap();
        let expr2 = crlfify(expr1);
        expr2.to_string()
    }

    #[test]
    fn various() {
        assert_eq!(roundtrip(r"(?m)$"), "(?:\r??(?m:$))");
        assert_eq!(roundtrip(r"(?m)$$"), "(?:\r??(?m:$))(?:\r??(?m:$))");
        assert_eq!(
            roundtrip(r"(?m)(?:foo$|bar$)"),
            "(?:foo(?:\r??(?m:$))|bar(?:\r??(?m:$)))"
        );
        assert_eq!(roundtrip(r"(?m)$a"), "(?:\r??(?m:$))a");

        // Not a multiline `$`, so no crlfifying occurs.
        assert_eq!(roundtrip(r"$"), "\\z");
        // It's a literal, derp.
        assert_eq!(roundtrip(r"\$"), "\\$");
    }
}
