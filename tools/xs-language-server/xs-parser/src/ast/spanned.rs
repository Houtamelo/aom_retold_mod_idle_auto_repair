use crate::parser::{Cst, Node, NodeRef, Span};
use crate::lexer::Token;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Spanned<T> {
    pub node: T,
    pub span: Span,
}

impl<T> Spanned<T> {
    pub fn new(node: T, span: Span) -> Self {
        Self { node, span }
    }

    pub fn span(&self) -> Span {
        self.span.clone()
    }

    pub fn into_inner(self) -> T {
        self.node
    }

    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Spanned<U> {
        Spanned::new(f(self.node), self.span)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Parenthesized<T> {
    pub open: Span,
    pub inner: T,
    pub close: Span,
}

impl<T> Parenthesized<T> {
    pub fn new(open: Span, inner: T, close: Span) -> Self {
        Self { open, inner, close }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Braced<T> {
    pub open: Span,
    pub inner: T,
    pub close: Span,
}

impl<T> Braced<T> {
    pub fn new(open: Span, inner: T, close: Span) -> Self {
        Self { open, inner, close }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Bracketed<T> {
    pub open: Span,
    pub inner: T,
    pub close: Span,
}

impl<T> Bracketed<T> {
    pub fn new(open: Span, inner: T, close: Span) -> Self {
        Self { open, inner, close }
    }
}

/// A list of items separated by commas. Each item carries an optional span
/// for the trailing comma (None for the last item).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommaSeparatedList<T> {
    pub items: Vec<(T, Option<Span>)>,
}

impl<T> Default for CommaSeparatedList<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> CommaSeparatedList<T> {
    pub fn new(items: Vec<(T, Option<Span>)>) -> Self {
        Self { items }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|(t, _)| t)
    }

    pub fn iter_with_separators(&self) -> impl Iterator<Item = &(T, Option<Span>)> {
        self.items.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = T> {
        self.items.into_iter().map(|(t, _)| t)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SemiColonSeparatedList<T> {
    pub items: Vec<(T, Option<Span>)>,
}

impl<T> Default for SemiColonSeparatedList<T> {
    fn default() -> Self {
        Self { items: Vec::new() }
    }
}

impl<T> SemiColonSeparatedList<T> {
    pub fn new(items: Vec<(T, Option<Span>)>) -> Self {
        Self { items }
    }

    pub fn iter(&self) -> impl Iterator<Item = &T> {
        self.items.iter().map(|(t, _)| t)
    }

    pub fn into_iter(self) -> impl Iterator<Item = T> {
        self.items.into_iter().map(|(t, _)| t)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

/// Holds a `Token` and its source span. Cannot be `Copy`/`Eq`/`Hash`
/// because the `Token` enum is not (it carries a Logos-derived payload).
#[derive(Debug, Clone, PartialEq)]
pub struct TokenSpan {
    pub token: Token,
    pub span: Span,
}

impl TokenSpan {
    pub fn from_cst(cst: &Cst, parent: NodeRef, token: Token) -> Option<Self> {
        cst.children(parent)
            .find_map(|c| cst.match_token(c, token))
            .map(|(_, span)| Self { token, span })
    }

    pub fn from_node(node: NodeRef, cst: &Cst) -> Option<Self> {
        match cst.get(node) {
            Node::Token(token, _) => {
                let span = cst.span(node);
                Some(Self { token, span })
            }
            Node::Rule(_, _) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{NodeRef, Parser};

    fn parse<'a>(source: &'a str) -> Cst<'a> {
        let mut diags = vec![];
        Parser::new(source, &mut diags).parse(&mut diags)
    }

    #[test]
    fn spanned_new_and_accessors() {
        let span = 0..3;
        let s = Spanned::new("foo".to_string(), span.clone());
        assert_eq!(s.node, "foo");
        assert_eq!(s.span(), span);
        assert_eq!(s.into_inner(), "foo");
    }

    #[test]
    fn spanned_map_preserves_span() {
        let span = 5..10;
        let s = Spanned::new(3, span.clone()).map(|n| n * 2);
        assert_eq!(s.node, 6);
        assert_eq!(s.span(), span);
    }

    #[test]
    fn parenthesized_holds_brackets() {
        let open = 0..1;
        let close = 5..6;
        let p = Parenthesized::new(open.clone(), 42_i32, close.clone());
        assert_eq!(p.open, open);
        assert_eq!(p.inner, 42);
        assert_eq!(p.close, close);
    }

    #[test]
    fn comma_separated_list_iter_and_len() {
        let list = CommaSeparatedList::new(vec![
            (1, Some(1..2)),
            (2, Some(3..4)),
            (3, None),
        ]);
        assert_eq!(list.len(), 3);
        assert!(!list.is_empty());
        let collected: Vec<i32> = list.iter().copied().collect();
        assert_eq!(collected, vec![1, 2, 3]);
    }

    #[test]
    fn comma_separated_list_into_iter() {
        let list = CommaSeparatedList::new(vec![(10, None), (20, Some(2..3))]);
        let collected: Vec<i32> = list.into_iter().collect();
        assert_eq!(collected, vec![10, 20]);
    }

    #[test]
    fn comma_separated_list_empty() {
        let list: CommaSeparatedList<i32> = CommaSeparatedList::default();
        assert!(list.is_empty());
        assert_eq!(list.len(), 0);
    }

    #[test]
    fn semi_colon_separated_list_iter() {
        let list = SemiColonSeparatedList::new(vec![("a", None), ("b", Some(1..2))]);
        let collected: Vec<&str> = list.iter().copied().collect();
        assert_eq!(collected, vec!["a", "b"]);
    }

    #[test]
    fn token_span_from_cst_finds_matching_token() {
        let source = "int x;";
        let cst = parse(source);
        let root = cst.children(NodeRef::ROOT).next().unwrap();
        let ts = TokenSpan::from_cst(&cst, root, Token::Semi).expect("semi");
        assert_eq!(ts.token, Token::Semi);
        assert_eq!(ts.span, source.len() - 1..source.len());
    }

    #[test]
    fn token_span_from_cst_returns_none_when_missing() {
        let source = "int x";
        let cst = parse(source);
        let root = cst.children(NodeRef::ROOT).next().unwrap();
        assert!(TokenSpan::from_cst(&cst, root, Token::Semi).is_none());
    }
}