use crate::parser::{Cst, Node, NodeRef, Rule, Span};
use crate::lexer::Token;

/// Returns the first child node that matches `rule`, or `None` if there is
/// no such child.
pub fn child_by_rule(cst: &Cst, parent: NodeRef, rule: Rule) -> Option<NodeRef> {
    cst.children(parent).find(|c| cst.match_rule(*c, rule))
}

/// Returns all children that match `rule` in source order.
pub fn children_by_rule(cst: &Cst, parent: NodeRef, rule: Rule) -> Vec<NodeRef> {
    cst.children(parent)
        .filter(|c| cst.match_rule(*c, rule))
        .collect()
}

/// Returns the span of the first child that is a token matching `token`.
pub fn child_by_token(cst: &Cst, parent: NodeRef, token: Token) -> Option<Span> {
    cst.children(parent)
        .find_map(|c| cst.match_token(c, token).map(|(_, span)| span))
}

/// Returns the unique rule child of `parent`, or `None` if there isn't
/// exactly one. Tokens (whitespace, comments, etc.) are skipped.
pub fn only_child(cst: &Cst, parent: NodeRef) -> Option<NodeRef> {
    let mut rule_children = cst
        .children(parent)
        .filter(|c| matches!(cst.get(*c), Node::Rule(_, _)));
    let first = rule_children.next()?;
    if rule_children.next().is_some() {
        return None;
    }
    Some(first)
}

/// Returns true if the node is a skip token (whitespace, comments, lexer
/// errors). These are produced by the parser but never carry semantic
/// meaning.
pub fn is_skip_token(cst: &Cst, node: NodeRef) -> bool {
    match cst.get(node) {
        Node::Token(
            Token::Whitespace | Token::LineComment | Token::BlockComment | Token::Error,
            _,
        ) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse<'a>(source: &'a str) -> Cst<'a> {
        let mut diags = vec![];
        Parser::new(source, &mut diags).parse(&mut diags)
    }

    fn first_child(cst: &Cst) -> NodeRef {
        cst.children(NodeRef::ROOT).next().unwrap()
    }

    #[test]
    fn child_by_rule_finds_declaration_specifiers() {
        let cst = parse("int x = 5;");
        let decl = first_child(&cst);
        let specs = child_by_rule(&cst, decl, Rule::DeclarationSpecifiers)
            .expect("declaration_specifiers child");
        assert!(cst.match_rule(specs, Rule::DeclarationSpecifiers));
    }

    #[test]
    fn child_by_rule_returns_none_when_absent() {
        let cst = parse("int x = 5;");
        let decl = first_child(&cst);
        assert!(child_by_rule(&cst, decl, Rule::CompoundStatement).is_none());
    }

    #[test]
    fn children_by_rule_collects_all() {
        let cst = parse("extern const int x;");
        let specs = child_by_rule(&cst, first_child(&cst), Rule::DeclarationSpecifiers).unwrap();
        let modifiers = children_by_rule(&cst, specs, Rule::StorageClassSpecifier);
        let quals = children_by_rule(&cst, specs, Rule::TypeQualifier);
        assert_eq!(modifiers.len(), 1);
        assert_eq!(quals.len(), 1);
    }

    #[test]
    fn children_by_rule_empty_when_no_match() {
        let cst = parse("int x;");
        let specs = child_by_rule(&cst, first_child(&cst), Rule::DeclarationSpecifiers).unwrap();
        let modifiers = children_by_rule(&cst, specs, Rule::StorageClassSpecifier);
        assert!(modifiers.is_empty());
    }

    #[test]
    fn child_by_token_returns_semi_span() {
        let cst = parse("int x = 5;");
        let decl = first_child(&cst);
        let semi = child_by_token(&cst, decl, Token::Semi).expect("semi");
        assert_eq!(semi, 9..10);
    }

    #[test]
    fn child_by_token_returns_none_when_absent() {
        let cst = parse("int x = 5");
        let decl = first_child(&cst);
        assert!(child_by_token(&cst, decl, Token::Semi).is_none());
    }

    #[test]
    fn only_child_returns_singleton_rule_child() {
        let cst = parse("int x = 5;");
        let decl = first_child(&cst);
        let specs = child_by_rule(&cst, decl, Rule::DeclarationSpecifiers).unwrap();
        let only = only_child(&cst, specs).expect("only one rule child (PrimitiveType)");
        // The actual type is wrapped in a `Rule::PrimitiveType` node —
        // see `examples/diag_cst.rs` for the full shape.
        assert!(cst.match_rule(only, Rule::PrimitiveType));
    }

    #[test]
    fn only_child_returns_none_for_multi_rule_children() {
        let cst = parse("extern const int x;");
        let specs = child_by_rule(&cst, first_child(&cst), Rule::DeclarationSpecifiers).unwrap();
        assert!(only_child(&cst, specs).is_none());
    }

    #[test]
    fn is_skip_token_recognizes_whitespace_and_comments() {
        let cst = parse("/* hi */ int x; // trailing");
        let whitespace_child = cst
            .children(NodeRef::ROOT)
            .find(|c| is_skip_token(&cst, *c))
            .expect("at least one skip child");
        let _ = whitespace_child;
    }
}