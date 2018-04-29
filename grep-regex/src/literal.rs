/*
This module is responsible for extracting *inner* literals out of the AST of a
regular expression. Normally this is the job of the regex engine itself, but
the regex engine doesn't look for inner literals. Since we're doing line based
searching, we can use them, so we need to do it ourselves.
*/

use std::cmp;

use regex_syntax::hir::{self, Hir, HirKind};
use regex_syntax::hir::literal::{Literal, Literals};

use util;

/// Represents prefix, suffix and inner "required" literals for a regular
/// expression.
///
/// Prefixes and suffixes are detected using regex-syntax. The inner required
/// literals are detected using something custom (but based on the code in
/// regex-syntax).
#[derive(Clone, Debug)]
pub struct LiteralSets {
    /// A set of prefix literals.
    prefixes: Literals,
    /// A set of suffix literals.
    suffixes: Literals,
    /// A set of literals such that at least one of them must appear in every
    /// match. A literal in this set may be neither a prefix nor a suffix.
    required: Literals,
}

impl LiteralSets {
    /// Create a set of literals from the given HIR expression.
    pub fn new(expr: &Hir) -> LiteralSets {
        let mut required = Literals::empty();
        union_required(expr, &mut required);
        LiteralSets {
            prefixes: Literals::prefixes(expr),
            suffixes: Literals::suffixes(expr),
            required: required,
        }
    }

    /// If it is deemed advantageuous to do so (via various suspicious
    /// heuristics), this will return a single regular expression pattern that
    /// matches a subset of the language matched by the regular expression that
    /// generated these literal sets. The idea here is that the pattern
    /// returned by this method is much cheaper to search for. i.e., It is
    /// usually a single literal or an alternation of literals.
    pub fn one_regex(&self) -> Option<String> {
        // TODO: The logic in this function is basically inscrutable. It grew
        // organically in the old grep 0.1 crate. Ideally, it would be
        // re-worked. In fact, the entire inner literal extraction should be
        // re-worked. Actually, most of regex-syntax's literal extraction
        // should also be re-worked. Alas... only so much time in the day.

        if self.prefixes.all_complete() && !self.prefixes.is_empty() {
            debug!("literal prefixes detected: {:?}", self.prefixes);
            // When this is true, the regex engine will do a literal scan,
            // so we don't need to return anything.
            return None;
        }

        // Out of inner required literals, prefixes and suffixes, which one
        // is the longest? We pick the longest to do fast literal scan under
        // the assumption that a longer literal will have a lower false
        // positive rate.
        let pre_lcp = self.prefixes.longest_common_prefix();
        let pre_lcs = self.prefixes.longest_common_suffix();
        let suf_lcp = self.suffixes.longest_common_prefix();
        let suf_lcs = self.suffixes.longest_common_suffix();

        let req_lits = self.required.literals();
        let req = match req_lits.iter().max_by_key(|lit| lit.len()) {
            None => &[],
            Some(req) => &***req,
        };

        let mut lit = pre_lcp;
        if pre_lcs.len() > lit.len() {
            lit = pre_lcs;
        }
        if suf_lcp.len() > lit.len() {
            lit = suf_lcp;
        }
        if suf_lcs.len() > lit.len() {
            lit = suf_lcs;
        }
        if req_lits.len() == 1 && req.len() > lit.len() {
            lit = req;
        }

        // Special case: if we detected an alternation of inner required
        // literals and its longest literal is bigger than the longest
        // prefix/suffix, then choose the alternation. In practice, this
        // helps with case insensitive matching, which can generate lots of
        // inner required literals.
        let any_empty = req_lits.iter().any(|lit| lit.is_empty());
        if req.len() > lit.len() && req_lits.len() > 1 && !any_empty {
            debug!("required literals found: {:?}", req_lits);
            let alts: Vec<String> = req_lits
                .into_iter()
                .map(|x| util::bytes_to_regex(x))
                .collect();
            // We're matching raw bytes, so disable Unicode mode.
            Some(format!("(?-u:{})", alts.join("|")))
        } else if lit.is_empty() {
            None
        } else {
            debug!("required literal found: {:?}", util::show_bytes(lit));
            Some(format!("(?-u:{})", util::bytes_to_regex(&lit)))
        }
    }
}

fn union_required(expr: &Hir, lits: &mut Literals) {
    match *expr.kind() {
        HirKind::Literal(hir::Literal::Unicode(c)) => {
            let mut buf = [0u8; 4];
            lits.cross_add(c.encode_utf8(&mut buf).as_bytes());
        }
        HirKind::Literal(hir::Literal::Byte(b)) => {
            lits.cross_add(&[b]);
        }
        HirKind::Class(hir::Class::Unicode(ref cls)) => {
            if count_unicode_class(cls) >= 5 || !lits.add_char_class(cls) {
                lits.cut();
            }
        }
        HirKind::Class(hir::Class::Bytes(ref cls)) => {
            if count_byte_class(cls) >= 5 || !lits.add_byte_class(cls) {
                lits.cut();
            }
        }
        HirKind::Group(hir::Group { ref hir, .. }) => {
            union_required(&**hir, lits);
        }
        HirKind::Repetition(ref x) => {
            match x.kind {
                hir::RepetitionKind::ZeroOrOne => lits.cut(),
                hir::RepetitionKind::ZeroOrMore => lits.cut(),
                hir::RepetitionKind::OneOrMore => {
                    union_required(&x.hir, lits);
                    lits.cut();
                }
                hir::RepetitionKind::Range(ref rng) => {
                    let (min, max) = match *rng {
                        hir::RepetitionRange::Exactly(m) => (m, Some(m)),
                        hir::RepetitionRange::AtLeast(m) => (m, None),
                        hir::RepetitionRange::Bounded(m, n) => (m, Some(n)),
                    };
                    repeat_range_literals(
                        &x.hir, min, max, x.greedy, lits, union_required);
                }
            }
        }
        HirKind::Concat(ref es) if es.is_empty() => {}
        HirKind::Concat(ref es) if es.len() == 1 => {
            union_required(&es[0], lits)
        }
        HirKind::Concat(ref es) => {
            for e in es {
                let mut lits2 = lits.to_empty();
                union_required(e, &mut lits2);
                if lits2.is_empty() {
                    lits.cut();
                    continue;
                }
                if lits2.contains_empty() {
                    lits.cut();
                }
                if !lits.cross_product(&lits2) {
                    // If this expression couldn't yield any literal that
                    // could be extended, then we need to quit. Since we're
                    // short-circuiting, we also need to freeze every member.
                    lits.cut();
                    break;
                }
            }
        }
        HirKind::Alternation(ref es) => {
            alternate_literals(es, lits, union_required);
        }
        _ => lits.cut(),
    }
}

fn repeat_range_literals<F: FnMut(&Hir, &mut Literals)>(
    e: &Hir,
    min: u32,
    max: Option<u32>,
    _greedy: bool,
    lits: &mut Literals,
    mut f: F,
) {
    if min == 0 {
        // This is a bit conservative. If `max` is set, then we could
        // treat this as a finite set of alternations. For now, we
        // just treat it as `e*`.
        lits.cut();
    } else {
        let n = cmp::min(lits.limit_size(), min as usize);
        // We only extract literals from a single repetition, even though
        // we could do more. e.g., `a{3}` will have `a` extracted instead of
        // `aaa`. The reason is that inner literal extraction can't be unioned
        // across repetitions. e.g., extracting `foofoofoo` from `(\w+foo){3}`
        // is wrong.
        f(e, lits);
        if n < min as usize {
            lits.cut();
        }
        if max.map_or(true, |max| min < max) {
            lits.cut();
        }
    }
}

fn alternate_literals<F: FnMut(&Hir, &mut Literals)>(
    es: &[Hir],
    lits: &mut Literals,
    mut f: F,
) {
    let mut lits2 = lits.to_empty();
    for e in es {
        let mut lits3 = lits.to_empty();
        lits3.set_limit_size(lits.limit_size() / 5);
        f(e, &mut lits3);
        if lits3.is_empty() || !lits2.union(lits3) {
            // If we couldn't find suffixes for *any* of the
            // alternates, then the entire alternation has to be thrown
            // away and any existing members must be frozen. Similarly,
            // if the union couldn't complete, stop and freeze.
            lits.cut();
            return;
        }
    }
    // All we do at the moment is look for prefixes and suffixes. If both
    // are empty, then we report nothing. We should be able to do better than
    // this, but we'll need something more expressive than just a "set of
    // literals."
    let lcp = lits2.longest_common_prefix();
    let lcs = lits2.longest_common_suffix();
    if !lcp.is_empty() {
        lits.cross_add(lcp);
    }
    lits.cut();
    if !lcs.is_empty() {
        lits.add(Literal::empty());
        lits.add(Literal::new(lcs.to_vec()));
    }
}

/// Return the number of characters in the given class.
fn count_unicode_class(cls: &hir::ClassUnicode) -> u32 {
    cls.iter().map(|r| 1 + (r.end() as u32 - r.start() as u32)).sum()
}

/// Return the number of bytes in the given class.
fn count_byte_class(cls: &hir::ClassBytes) -> u32 {
    cls.iter().map(|r| 1 + (r.end() as u32 - r.start() as u32)).sum()
}

#[cfg(test)]
mod tests {
    use regex_syntax::Parser;
    use super::LiteralSets;

    fn sets(pattern: &str) -> LiteralSets {
        let hir = Parser::new().parse(pattern).unwrap();
        LiteralSets::new(&hir)
    }

    fn one_regex(pattern: &str) -> Option<String> {
        sets(pattern).one_regex()
    }

    // Put a pattern into the same format as the one returned by `one_regex`.
    fn pat(pattern: &str) -> Option<String> {
        Some(format!("(?-u:{})", pattern))
    }

    #[test]
    fn various() {
        // Obviously no literals.
        assert!(one_regex(r"\w").is_none());
        assert!(one_regex(r"\pL").is_none());

        // Tantalizingly close.
        assert!(one_regex(r"\w|foo").is_none());

        // There's a literal, but it's better if the regex engine handles it
        // internally.
        assert!(one_regex(r"abc").is_none());

        // Core use cases.
        assert_eq!(one_regex(r"\wabc\w"), pat("abc"));
        assert_eq!(one_regex(r"abc\w"), pat("abc"));

        // TODO: Make these pass. We're missing some potentially big wins
        // without these.
        // assert_eq!(one_regex(r"\w(foo|bar|baz)"), pat("foo|bar|baz"));
        // assert_eq!(one_regex(r"\w(foo|bar|baz)\w"), pat("foo|bar|baz"));
    }
}
