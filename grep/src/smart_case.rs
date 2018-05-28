use syntax::ast::{self, Ast};
use syntax::ast::parse::Parser;

/// The results of analyzing a regex for cased literals.
#[derive(Clone, Debug, Default)]
pub struct Cased {
    /// True if and only if a literal uppercase character occurs in the regex.
    ///
    /// A regex like `\pL` contains no uppercase literals, even though `L`
    /// is uppercase and the `\pL` class contains uppercase characters.
    pub any_uppercase: bool,
    /// True if and only if the regex contains any literal at all. A regex like
    /// `\pL` has this set to false.
    pub any_literal: bool,
}

impl Cased {
    /// Returns a `Cased` value by doing analysis on the AST of `pattern`.
    ///
    /// If `pattern` is not a valid regular expression, then `None` is
    /// returned.
    pub fn from_pattern(pattern: &str) -> Option<Cased> {
        Parser::new()
            .parse(pattern)
            .map(|ast| Cased::from_ast(&ast))
            .ok()
    }

    fn from_ast(ast: &Ast) -> Cased {
        let mut cased = Cased::default();
        cased.from_ast_impl(ast);
        cased
    }

    fn from_ast_impl(&mut self, ast: &Ast) {
        if self.done() {
            return;
        }
        match *ast {
            Ast::Empty(_)
            | Ast::Flags(_)
            | Ast::Dot(_)
            | Ast::Assertion(_)
            | Ast::Class(ast::Class::Unicode(_))
            | Ast::Class(ast::Class::Perl(_)) => {}
            Ast::Literal(ref x) => {
                self.from_ast_literal(x);
            }
            Ast::Class(ast::Class::Bracketed(ref x)) => {
                self.from_ast_class_set(&x.kind);
            }
            Ast::Repetition(ref x) => {
                self.from_ast_impl(&x.ast);
            }
            Ast::Group(ref x) => {
                self.from_ast_impl(&x.ast);
            }
            Ast::Alternation(ref alt) => {
                for x in &alt.asts {
                    self.from_ast_impl(x);
                }
            }
            Ast::Concat(ref alt) => {
                for x in &alt.asts {
                    self.from_ast_impl(x);
                }
            }
        }
    }

    fn from_ast_class_set(&mut self, ast: &ast::ClassSet) {
        if self.done() {
            return;
        }
        match *ast {
            ast::ClassSet::Item(ref item) => {
                self.from_ast_class_set_item(item);
            }
            ast::ClassSet::BinaryOp(ref x) => {
                self.from_ast_class_set(&x.lhs);
                self.from_ast_class_set(&x.rhs);
            }
        }
    }

    fn from_ast_class_set_item(&mut self, ast: &ast::ClassSetItem) {
        if self.done() {
            return;
        }
        match *ast {
            ast::ClassSetItem::Empty(_)
            | ast::ClassSetItem::Ascii(_)
            | ast::ClassSetItem::Unicode(_)
            | ast::ClassSetItem::Perl(_) => {}
            ast::ClassSetItem::Literal(ref x) => {
                self.from_ast_literal(x);
            }
            ast::ClassSetItem::Range(ref x) => {
                self.from_ast_literal(&x.start);
                self.from_ast_literal(&x.end);
            }
            ast::ClassSetItem::Bracketed(ref x) => {
                self.from_ast_class_set(&x.kind);
            }
            ast::ClassSetItem::Union(ref union) => {
                for x in &union.items {
                    self.from_ast_class_set_item(x);
                }
            }
        }
    }

    fn from_ast_literal(&mut self, ast: &ast::Literal) {
        self.any_literal = true;
        self.any_uppercase = self.any_uppercase || ast.c.is_uppercase();
    }

    /// Returns true if and only if the attributes can never change no matter
    /// what other AST it might see.
    fn done(&self) -> bool {
        self.any_uppercase && self.any_literal
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cased(pattern: &str) -> Cased {
        Cased::from_pattern(pattern).unwrap()
    }

    #[test]
    fn various() {
        let x = cased("");
        assert!(!x.any_uppercase);
        assert!(!x.any_literal);

        let x = cased("foo");
        assert!(!x.any_uppercase);
        assert!(x.any_literal);

        let x = cased("Foo");
        assert!(x.any_uppercase);
        assert!(x.any_literal);

        let x = cased("foO");
        assert!(x.any_uppercase);
        assert!(x.any_literal);

        let x = cased(r"foo\\");
        assert!(!x.any_uppercase);
        assert!(x.any_literal);

        let x = cased(r"foo\w");
        assert!(!x.any_uppercase);
        assert!(x.any_literal);

        let x = cased(r"foo\S");
        assert!(!x.any_uppercase);
        assert!(x.any_literal);

        let x = cased(r"foo\p{Ll}");
        assert!(!x.any_uppercase);
        assert!(x.any_literal);

        let x = cased(r"foo[a-z]");
        assert!(!x.any_uppercase);
        assert!(x.any_literal);

        let x = cased(r"foo[A-Z]");
        assert!(x.any_uppercase);
        assert!(x.any_literal);

        let x = cased(r"foo[\S\t]");
        assert!(!x.any_uppercase);
        assert!(x.any_literal);

        let x = cased(r"foo\\S");
        assert!(x.any_uppercase);
        assert!(x.any_literal);

        let x = cased(r"\p{Ll}");
        assert!(!x.any_uppercase);
        assert!(!x.any_literal);

        let x = cased(r"aBc\w");
        assert!(x.any_uppercase);
        assert!(x.any_literal);
    }
}
