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
    ///
    /// Returns `true` when the current token starts a declaration or
    /// nested function definition. For identifiers we look the name up
    /// in the parser's `TypeTable`; primitive type keywords are mapped
    /// to their lexeme and checked the same way. Declaration qualifiers
    /// always commit to the declaration branch.
    fn predicate_block_item_1(&self) -> bool {
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
