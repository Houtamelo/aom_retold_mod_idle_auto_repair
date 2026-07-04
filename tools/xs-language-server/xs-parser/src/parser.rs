use super::lexer::{Token, tokenize};
use crate::ast::type_table::TypeTable;

// TODO: change if codespan_reporting is not used
use codespan_reporting::diagnostic::Label;
pub type Diagnostic = codespan_reporting::diagnostic::Diagnostic<()>;

//==============================================================================
// TypeTable-backed declaration disambiguation (2026-07-03):
//
// `TypeTable` provides the set of known type names (primitive
// keywords + caller-supplied class names). The generated parser
// consults it via `predicate_block_item_1` to decide whether a token
// sequence starting with an identifier should be tried as a
// declaration/function-definition inside a rule or function body
// before falling through to `statement`.
//==============================================================================

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

impl<'a> Parser<'a> {
    /// Returns `true` when the current token is a primitive return type
    /// immediately followed by `(`, indicating a function-pointer
    /// variable declaration.
    fn function_pointer_branch_applies(&self) -> bool {
        matches!(
            self.current,
            Token::Void | Token::Int | Token::Bool | Token::Float | Token::StringKw | Token::Vector
        ) && self.peek(1) == Token::LPar
    }

    /// Returns `true` when the current token starts a declaration or
    /// nested function definition. Used by both `block_item` and
    /// `for_init` predicates to disambiguate declaration forms from
    /// expression-statement/expression forms using the parser's
    /// `TypeTable`.
    fn current_starts_declaration(&self) -> bool {
        let text = match self.current {
            Token::Identifier => &self.cst.source()[self.span()],
            Token::Void => "void",
            Token::Int => "int",
            Token::Bool => "bool",
            Token::Float => "float",
            Token::StringKw => "string",
            Token::Vector => "vector",
            // Qualifiers never start an expression statement in XS.
            Token::Extern | Token::Static | Token::Const
            | Token::Mutable | Token::Ref => return true,
            _ => return false,
        };
        self.context.is_type(text)
    }
}

impl<'a> ParserCallbacks<'a> for Parser<'a> {
    type Diagnostic = Diagnostic;
    type Context = TypeTable;

    fn create_tokens(_context: &mut Self::Context, source: &'a str, diags: &mut Vec<Self::Diagnostic>) -> (Vec<Token>, Vec<Span>) {
        tokenize(source, diags)
    }
    fn create_diagnostic(&self, span: Span, message: String) -> Self::Diagnostic {
        Self::Diagnostic::error()
            .with_message(message)
            .with_label(Label::primary((), span))
    }

    /// Semantic predicate for the declaration branch of `block_item^`.
    fn predicate_block_item_1(&self) -> bool {
        self.current_starts_declaration()
    }

    /// Semantic predicate for the declaration branch of `for_init`.
    ///
    /// Distinguishes `for (int i = 0; ...)` (declaration) from
    /// `for (foo = 0; ...)` (expression) the same way `block_item`
    /// resolves the type-vs-expression ambiguity.
    fn predicate_for_init_1(&self) -> bool {
        self.current_starts_declaration()
    }

    fn predicate_declaration_1(&self) -> bool {
        self.function_pointer_branch_applies()
    }

    fn predicate_block_declaration_or_definition_1(&self) -> bool {
        self.function_pointer_branch_applies()
    }

    /// Semantic predicate for the `?1 'default' ':' statement` branch of `statement^`.
    fn predicate_statement_1(&self) -> bool {
        self.current == Token::DefaultKw && self.peek(1) == Token::Colon
    }

    /// Semantic predicate for the function-pointer-field branch of
    /// `class_member^`. Returns `true` when the current token is a
    /// primitive type and the next token is `(` — i.e. the start of
    /// `void(int) field = ...;` or `bool(ref X) field = ...;`.
    ///
    /// Without this, the parser falls through to `field_declaration`,
    /// which parses `void(int) field` as `void` (type) + `(int)`
    /// (parenthesized declarator) + `field` (stray identifier) and
    /// then chokes on the missing `=` or `;`.
    fn predicate_class_member_1(&self) -> bool {
        if !matches!(
            self.current,
            Token::Void | Token::Int | Token::Bool | Token::Float | Token::StringKw | Token::Vector
        ) {
            return false;
        }
        self.peek(1) == Token::LPar
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser_at(source: &str, ctx: TypeTable) -> Parser<'_> {
        let mut diags = Vec::new();
        let mut parser = Parser::new_with_context(source, &mut diags, ctx);
        // `new_with_context` leaves `current` at EOF until parsing begins.
        // Initialize the token stream so the predicate sees the first token.
        parser.init_skip();
        parser
    }

    #[test]
    fn predicate_primitive_int_is_declaration_start() {
        let parser = parser_at("int", TypeTable::with_primitives());
        assert!(parser.predicate_block_item_1());
    }

    #[test]
    fn predicate_known_class_is_declaration_start() {
        let mut types = TypeTable::with_primitives();
        types.insert_class("BOSystem");
        let parser = parser_at("BOSystem", types);
        assert!(parser.predicate_block_item_1());
    }

    #[test]
    fn predicate_unknown_identifier_is_not_declaration_start() {
        let parser = parser_at("foo", TypeTable::with_primitives());
        assert!(!parser.predicate_block_item_1());
    }

    #[test]
    fn predicate_qualifier_is_declaration_start() {
        let parser = parser_at("extern", TypeTable::default());
        assert!(parser.predicate_block_item_1());
    }

    #[test]
    fn predicate_empty_table_primitive_is_not_declaration_start() {
        let table = TypeTable {
            primitives: std::collections::HashSet::new(),
            classes: std::collections::HashSet::new(),
        };
        let parser = parser_at("int", table);
        assert!(!parser.predicate_block_item_1());
    }
}
