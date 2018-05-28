use syntax::hir::{self, Hir, HirKind};

/// Strips Unicode word boundaries from the given expression.
///
/// The key invariant this maintains is that the expression returned will match
/// *at least* every where the expression given will match. Namely, a match of
/// the returned expression can report false positives but it will never report
/// false negatives.
///
/// If no word boundaries could be stripped, then None is returned.
pub fn strip_unicode_word_boundaries(expr: &Hir) -> Option<Hir> {
    // The real reason we do this is because Unicode word boundaries are the
    // one thing that Rust's regex DFA engine can't handle. When it sees a
    // Unicode word boundary among non-ASCII text, it falls back to one of the
    // slower engines. We work around this limitation by attempting to use
    // a regex to find candidate matches without a Unicode word boundary. We'll
    // only then use the full (and slower) regex to confirm a candidate as a
    // match or not during search.
    //
    // It looks like we only check the outer edges for `\b`? I guess this is
    // an attempt to optimize for the `-w/--word-regexp` flag? ---AG
    match *expr.kind() {
        HirKind::Concat(ref es) if !es.is_empty() => {
            let first = is_unicode_word_boundary(&es[0]);
            let last = is_unicode_word_boundary(es.last().unwrap());
            // Be careful not to strip word boundaries if there are no other
            // expressions to match.
            match (first, last) {
                (true, false) if es.len() > 1 => {
                    Some(Hir::concat(es[1..].to_vec()))
                }
                (false, true) if es.len() > 1 => {
                    Some(Hir::concat(es[..es.len() - 1].to_vec()))
                }
                (true, true) if es.len() > 2 => {
                    Some(Hir::concat(es[1..es.len() - 1].to_vec()))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

/// Returns true if the given expression is a Unicode word boundary.
fn is_unicode_word_boundary(expr: &Hir) -> bool {
    match *expr.kind() {
        HirKind::WordBoundary(hir::WordBoundary::Unicode) => true,
        HirKind::WordBoundary(hir::WordBoundary::UnicodeNegate) => true,
        HirKind::Group(ref x) => is_unicode_word_boundary(&x.hir),
        _ => false,
    }
}
