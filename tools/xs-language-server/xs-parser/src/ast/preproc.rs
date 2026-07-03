use crate::ast::cst_helpers::is_skip_token;
use crate::ast::spanned::Spanned;
use crate::ast::type_system::Identifier;
use crate::parser::{Cst, Node, NodeRef, Rule, Span};
use crate::lexer::Token;

/// `#define NAME` — a preprocessor name definition.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreprocDef {
    pub name: Identifier,
    pub span: Span,
}

impl PreprocDef {
    /// Extract from a `Rule::PreprocDef` node. The CST shape is:
    /// `preproc_def → Hash, PreprocDefine, [skip], Identifier` (in order,
    /// with skip tokens interleaved).
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::PreprocDef) {
            return None;
        }
        let (text, span) = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Identifier))?;
        Some(Self {
            name: Identifier::new(text.to_string(), span),
            span: cst.span(node),
        })
    }
}

/// `#if EXPR` — the head of a preprocessor conditional.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreprocIf {
    pub cond: PreprocExpr,
    pub span: Span,
}

impl PreprocIf {
    /// Extract from a `Rule::PreprocIf` node. The direct children are:
    /// `Token::Hash`, `Token::If`, possibly skip tokens, then
    /// `Rule::PreprocExpr`.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::PreprocIf) {
            return None;
        }
        let expr_node = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::PreprocExpr))?;
        let cond = PreprocExpr::from_cst(cst, expr_node)?;
        Some(Self {
            cond,
            span: cst.span(node),
        })
    }
}

/// `#elif EXPR` — a continuation branch of a preprocessor conditional.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreprocElif {
    pub cond: PreprocExpr,
    pub span: Span,
}

impl PreprocElif {
    /// Extract from a `Rule::PreprocElif` node. Children:
    /// `Token::Hash`, `Token::PreprocElif`, skip tokens, `Rule::PreprocExpr`.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::PreprocElif) {
            return None;
        }
        let expr_node = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::PreprocExpr))?;
        let cond = PreprocExpr::from_cst(cst, expr_node)?;
        Some(Self {
            cond,
            span: cst.span(node),
        })
    }
}

/// `#else` — the terminal fallback branch of a preprocessor conditional.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreprocElse {
    pub span: Span,
}

impl PreprocElse {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::PreprocElse) {
            return None;
        }
        Some(Self { span: cst.span(node) })
    }
}

/// `#endif` — closes a preprocessor conditional.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PreprocEndif {
    pub span: Span,
}

impl PreprocEndif {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::PreprocEndif) {
            return None;
        }
        Some(Self { span: cst.span(node) })
    }
}

/// Equality operator at the preproc_eq level.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum EqOp {
    Eq,
    Neq,
}

impl EqOp {
    /// Classifies the token `node` as an equality operator. Returns `None`
    /// if the node is not an `Eq` or `Neq` token.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_token(node, Token::Eq).is_some() {
            return Some(Self::Eq);
        }
        if cst.match_token(node, Token::Neq).is_some() {
            return Some(Self::Neq);
        }
        None
    }
}

/// Relational operator at the preproc_rel level.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum RelOp {
    Lt,
    Gt,
    Leq,
    Geq,
}

impl RelOp {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_token(node, Token::Lt).is_some() {
            return Some(Self::Lt);
        }
        if cst.match_token(node, Token::Gt).is_some() {
            return Some(Self::Gt);
        }
        if cst.match_token(node, Token::Leq).is_some() {
            return Some(Self::Leq);
        }
        if cst.match_token(node, Token::Geq).is_some() {
            return Some(Self::Geq);
        }
        None
    }
}

/// Additive operator at the preproc_add level.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AddOp {
    Plus,
    Minus,
}

impl AddOp {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_token(node, Token::Plus).is_some() {
            return Some(Self::Plus);
        }
        if cst.match_token(node, Token::Minus).is_some() {
            return Some(Self::Minus);
        }
        None
    }
}

/// Multiplicative operator at the preproc_mul level.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum MulOp {
    Times,
    Div,
    Mod,
}

impl MulOp {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_token(node, Token::Star).is_some() {
            return Some(Self::Times);
        }
        if cst.match_token(node, Token::Slash).is_some() {
            return Some(Self::Div);
        }
        if cst.match_token(node, Token::Percent).is_some() {
            return Some(Self::Mod);
        }
        None
    }
}

/// A classified preprocessor expression.
///
/// The grammar produces a deep chain of single-child rule nodes
/// (`preproc_or_expr → preproc_and_expr → preproc_eq_expr → ...`). The
/// `from_cst` descent identifies which level the expression operates at
/// (e.g. `Eq(EqOp::Neq)` for a `defined(X) != 0` chain) and returns the
/// variant without folding the operands onto the variant itself (per the
/// T8 spec — the 12 variants carry only the listed data).
///
/// To reach the operands after classification, callers re-walk the
/// children of the relevant preproc level.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PreprocExpr {
    /// OR-chain (operands are preproc_and_expr children of a
    /// preproc_or_expr parent).
    Or,
    /// AND-chain (operands are preproc_eq_expr children of a
    /// preproc_and_expr parent).
    And,
    /// Equality comparison (operator identifies `==` or `!=`).
    Eq(EqOp),
    /// Relational comparison (operator identifies `<`, `>`, `<=`, `>=`).
    Relational(RelOp),
    /// Additive expression (operator identifies `+` or `-`).
    Additive(AddOp),
    /// Multiplicative expression (operator identifies `*`, `/`, `%`).
    Multiplicative(MulOp),
    /// Logical NOT (`!EXPR`).
    Not,
    /// A `defined(IDENT)` predicate.
    Defined(Identifier),
    /// A parenthesized sub-expression (`(EXPR)`).
    Paren,
    /// An integer literal.
    IntConst(Spanned<String>),
    /// A floating-point literal.
    FloatConst(Spanned<String>),
    /// A bare identifier used as a preprocessor expression.
    Identifier(Identifier),
}

impl PreprocExpr {
    /// Classify the preprocessor expression rooted at `node`.
    ///
    /// The grammar always wraps expressions through every level
    /// (`preproc_or_expr → preproc_and_expr → ... → preproc_primary`)
    /// even when a level has only one operand and no operator of its
    /// kind. To avoid misclassifying single-operand expressions as
    /// upper-level binary ops, this descent only claims a level when
    /// the corresponding operator token is present; otherwise it
    /// transparently descends to the unique child.
    ///
    /// Accepts any of the rule nodes produced by the grammar's
    /// preprocessor sub-hierarchy: `PreprocOrExpr`, `PreprocAndExpr`,
    /// `PreprocEqExpr`, `PreprocRelExpr`, `PreprocAddExpr`,
    /// `PreprocMulExpr`, `PreprocUnaryExpr`, `PreprocPrimary`,
    /// `PreprocDefined`, as well as the outermost `PreprocExpr`
    /// wrapper. Returns `None` if the input shape does not match any
    /// recognized form (for example, an `ERROR` or skip subtree).
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        let mut current = node;
        loop {
            if is_skip_token(cst, current) {
                return None;
            }
            match cst.get(current) {
                Node::Token(_, _) => {
                    // Bare token — meaningful only for IntConst / FloatConst /
                    // Identifier leaves, which always live under
                    // preproc_primary in the actual grammar.
                    if let Some((text, span)) = cst.match_token(current, Token::IntConst) {
                        return Some(Self::IntConst(Spanned::new(text.to_string(), span)));
                    }
                    if let Some((text, span)) = cst.match_token(current, Token::FloatConst) {
                        return Some(Self::FloatConst(Spanned::new(text.to_string(), span)));
                    }
                    if let Some((text, span)) = cst.match_token(current, Token::Identifier) {
                        return Some(Self::Identifier(Identifier::new(text.to_string(), span)));
                    }
                    return None;
                }
                Node::Rule(Rule::PreprocOrExpr, _) => {
                    if Self::has_op(cst, current, Token::OrOr) {
                        return Some(Self::Or);
                    }
                    current = Self::only_rule_child(cst, current)?;
                }
                Node::Rule(Rule::PreprocAndExpr, _) => {
                    if Self::has_op(cst, current, Token::AndAnd) {
                        return Some(Self::And);
                    }
                    current = Self::only_rule_child(cst, current)?;
                }
                Node::Rule(Rule::PreprocEqExpr, _) => {
                    if let Some(op) = Self::first_eq_op(cst, current) {
                        return Some(Self::Eq(op));
                    }
                    current = Self::only_rule_child(cst, current)?;
                }
                Node::Rule(Rule::PreprocRelExpr, _) => {
                    if let Some(op) = Self::first_rel_op(cst, current) {
                        return Some(Self::Relational(op));
                    }
                    current = Self::only_rule_child(cst, current)?;
                }
                Node::Rule(Rule::PreprocAddExpr, _) => {
                    if let Some(op) = Self::first_add_op(cst, current) {
                        return Some(Self::Additive(op));
                    }
                    current = Self::only_rule_child(cst, current)?;
                }
                Node::Rule(Rule::PreprocMulExpr, _) => {
                    if let Some(op) = Self::first_mul_op(cst, current) {
                        return Some(Self::Multiplicative(op));
                    }
                    current = Self::only_rule_child(cst, current)?;
                }
                Node::Rule(Rule::PreprocUnaryExpr, _) => {
                    // Two arms: '!' preproc_unary_expr | preproc_primary.
                    if Self::has_op(cst, current, Token::Excl) {
                        return Some(Self::Not);
                    }
                    current = Self::only_rule_child(cst, current)?;
                }
                Node::Rule(Rule::PreprocPrimary, _) => {
                    return Self::from_primary(cst, current);
                }
                Node::Rule(Rule::PreprocDefined, _) => {
                    let (text, span) = cst
                        .children(current)
                        .find_map(|c| cst.match_token(c, Token::Identifier))?;
                    return Some(Self::Defined(Identifier::new(text.to_string(), span)));
                }
                // Any other rule (e.g. the outermost preproc_expr wrapping
                // everything) — descend to the first rule child.
                Node::Rule(_, _) => {
                    current = Self::only_rule_child(cst, current)?;
                }
            }
        }
    }

    /// Extract from a `Rule::PreprocPrimary` node. Children are one of:
    /// a single `Rule::PreprocDefined` (already handled at the caller
    /// level), a single `Token::IntConst`, `Token::FloatConst`,
    /// `Token::Identifier`, or `Token::LPar`+`Rule::PreprocExpr`+`Token::RPar`.
    fn from_primary(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::PreprocPrimary) {
            return None;
        }
        for child in cst.children(node) {
            if cst.match_rule(child, Rule::PreprocDefined) {
                let (text, span) = cst
                    .children(child)
                    .find_map(|c| cst.match_token(c, Token::Identifier))?;
                return Some(Self::Defined(Identifier::new(text.to_string(), span)));
            }
            if let Some((text, span)) = cst.match_token(child, Token::IntConst) {
                return Some(Self::IntConst(Spanned::new(text.to_string(), span)));
            }
            if let Some((text, span)) = cst.match_token(child, Token::FloatConst) {
                return Some(Self::FloatConst(Spanned::new(text.to_string(), span)));
            }
            if let Some((text, span)) = cst.match_token(child, Token::Identifier) {
                return Some(Self::Identifier(Identifier::new(text.to_string(), span)));
            }
            if cst.match_token(child, Token::LPar).is_some() {
                return Some(Self::Paren);
            }
        }
        None
    }

    fn has_op(cst: &Cst, node: NodeRef, token: Token) -> bool {
        cst.children(node)
            .any(|c| cst.match_token(c, token).is_some())
    }

    fn only_rule_child(cst: &Cst, node: NodeRef) -> Option<NodeRef> {
        cst.children(node)
            .find(|c| matches!(cst.get(*c), Node::Rule(_, _)))
    }

    fn first_eq_op(cst: &Cst, node: NodeRef) -> Option<EqOp> {
        for child in cst.children(node) {
            if let Some(op) = EqOp::from_cst(cst, child) {
                return Some(op);
            }
        }
        None
    }

    fn first_rel_op(cst: &Cst, node: NodeRef) -> Option<RelOp> {
        for child in cst.children(node) {
            if let Some(op) = RelOp::from_cst(cst, child) {
                return Some(op);
            }
        }
        None
    }

    fn first_add_op(cst: &Cst, node: NodeRef) -> Option<AddOp> {
        for child in cst.children(node) {
            if let Some(op) = AddOp::from_cst(cst, child) {
                return Some(op);
            }
        }
        None
    }

    fn first_mul_op(cst: &Cst, node: NodeRef) -> Option<MulOp> {
        for child in cst.children(node) {
            if let Some(op) = MulOp::from_cst(cst, child) {
                return Some(op);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::cst_helpers::child_by_rule;
    use crate::parser::{NodeRef, Parser};

    fn parse<'a>(source: &'a str) -> Cst<'a> {
        let mut diags = vec![];
        Parser::new(source, &mut diags).parse(&mut diags)
    }

    /// Returns the first non-skip rule child of the translation unit.
    fn first_rule(cst: &Cst) -> NodeRef {
        cst.children(NodeRef::ROOT)
            .find(|c| !is_skip_token(cst, *c))
            .expect("at least one non-skip child")
    }

    /// Returns the inner `PreprocExpr` node under the first top-level item.
    fn inner_expr_node(cst: &Cst) -> NodeRef {
        let outer = first_rule(cst);
        child_by_rule(cst, outer, Rule::PreprocExpr).expect("outer preproc_expr child")
    }

    /// Classify the preprocessor expression for `source` and return it.
    fn classify(source: &str) -> PreprocExpr {
        let cst = parse(source);
        let expr_node = inner_expr_node(&cst);
        PreprocExpr::from_cst(&cst, expr_node).expect("classifies cleanly")
    }

    /// Walks descendants of `node` looking for one whose `match_token`
    /// returns `Some` for the given token. Useful when the operator
    /// appears deep in the CST chain.
    fn find_token(cst: &Cst, node: NodeRef, token: Token) -> Option<NodeRef> {
        for child in cst.children(node) {
            if cst.match_token(child, token).is_some() {
                return Some(child);
            }
            if matches!(cst.get(child), Node::Rule(_, _)) {
                if let Some(found) = find_token(cst, child, token) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Descends through transparent wrappers (PreprocOrExpr,
    /// PreprocAndExpr, ..., PreprocUnaryExpr — those without an
    /// operator at their level) and returns the first
    /// `PreprocXExpr` rule child of a given kind that does carry that
    /// operator. Falls back to the first rule child. Used by tests
    /// that need to land at a specific level.
    fn descend_to(cst: &Cst, node: NodeRef, target: Rule) -> NodeRef {
        if cst.match_rule(node, target) {
            return node;
        }
        for child in cst.children(node) {
            if matches!(cst.get(child), Node::Rule(_, _)) {
                let found = descend_to(cst, child, target);
                if cst.match_rule(found, target) {
                    return found;
                }
            }
        }
        node
    }

    #[test]
    fn preproc_def_extracts_identifier() {
        let cst = parse("#define X");
        let def = PreprocDef::from_cst(&cst, first_rule(&cst)).expect("parses #define");
        assert_eq!(def.name.node, "X");
        assert_eq!(def.span, 0..9);
    }

    #[test]
    fn preproc_def_returns_none_on_non_def() {
        let cst = parse("#if X");
        let outer = first_rule(&cst);
        assert!(PreprocDef::from_cst(&cst, outer).is_none());
    }

    #[test]
    fn preproc_if_with_defined_parses_to_defined_variant() {
        let expr = classify("#if defined(X)");
        match expr {
            PreprocExpr::Defined(id) => assert_eq!(id.node, "X"),
            other => panic!("expected Defined, got {:?}", other),
        }
    }

    #[test]
    fn preproc_if_with_defined_neq_int_parses_to_eq_neq() {
        // The grammar does not model a `false` keyword, but
        // `defined(X) != 0` is an Eq(Neq) whose operands are one
        // `Defined` and one `IntConst`.
        let expr = classify("#if defined(X) != 0");
        assert_eq!(expr, PreprocExpr::Eq(EqOp::Neq));
    }

    #[test]
    fn preproc_if_with_int_constant_parses_to_int_const() {
        let expr = classify("#if 1");
        match expr {
            PreprocExpr::IntConst(s) => assert_eq!(s.node, "1"),
            other => panic!("expected IntConst, got {:?}", other),
        }
    }

    #[test]
    fn preproc_if_with_float_constant_parses_to_float_const() {
        let expr = classify("#if 1.0");
        match expr {
            PreprocExpr::FloatConst(s) => assert_eq!(s.node, "1.0"),
            other => panic!("expected FloatConst, got {:?}", other),
        }
    }

    #[test]
    fn preproc_or_and_chain_root_is_or() {
        // `A && B || C` parses to outermost `Or`. The inner AND chain
        // (`A && B`) is a `preproc_and_expr` rule sibling of the second
        // operand (`C`) — verified via direct CST inspection.
        let cst = parse("#if A && B || C");
        let or_node = inner_expr_node(&cst);
        // The outer PreprocExpr node wraps a single PreprocOrExpr child.
        let root_or = child_by_rule(&cst, or_node, Rule::PreprocOrExpr).unwrap();
        let expr = PreprocExpr::from_cst(&cst, or_node).unwrap();
        assert_eq!(expr, PreprocExpr::Or);

        let kids: Vec<_> = cst
            .children(root_or)
            .filter(|c| !is_skip_token(&cst, *c))
            .collect();
        // 2 rule children (the AND chain and the bare `C` operand) — OrOr
        // token is in between but is a Token, not a Rule.
        let rule_kids: Vec<_> = kids
            .iter()
            .copied()
            .filter(|c| matches!(cst.get(*c), Node::Rule(_, _)))
            .collect();
        assert_eq!(rule_kids.len(), 2);
        // First operand is a PreprocAndExpr → both `A` and `B` are inside.
        let first_and_node = rule_kids[0];
        assert!(cst.match_rule(first_and_node, Rule::PreprocAndExpr));
        let and_kids: Vec<_> = cst
            .children(first_and_node)
            .filter(|c| matches!(cst.get(*c), Node::Rule(_, _)))
            .collect();
        assert_eq!(and_kids.len(), 2);
        // Last operand is the bare `C` identifier.
        let last = rule_kids[1];
        let last_expr = PreprocExpr::from_cst(&cst, last).unwrap();
        assert_eq!(
            last_expr,
            PreprocExpr::Identifier(Identifier::new("C".to_string(), 14..15))
        );
    }

    #[test]
    fn nested_paren_under_and_chain() {
        // Outer root: AND (between defined(X) and the parenthesized OR chain).
        let cst = parse("#if defined(X) && (defined(Y) || defined(Z))");
        let and_node = inner_expr_node(&cst);
        let expr = PreprocExpr::from_cst(&cst, and_node).unwrap();
        assert_eq!(expr, PreprocExpr::And);

        // Walk: preproc_expr → preproc_or_expr (transparent) →
        // preproc_and_expr (carries AndAnd).
        let root_and = descend_to(&cst, and_node, Rule::PreprocAndExpr);
        let operands: Vec<_> = cst
            .children(root_and)
            .filter(|c| cst.match_rule(*c, Rule::PreprocEqExpr))
            .collect();
        assert_eq!(operands.len(), 2);

        // First operand: defined(X).
        let first = PreprocExpr::from_cst(&cst, operands[0]).unwrap();
        assert_eq!(
            first,
            PreprocExpr::Defined(Identifier::new("X".to_string(), 12..13))
        );

        // Second operand: a parenthesized OR chain.
        let second = PreprocExpr::from_cst(&cst, operands[1]).unwrap();
        assert_eq!(second, PreprocExpr::Paren);

        // Inside the paren, the inner OR chain exercises Defined twice
        // plus the Or variant.
        let inner_or_node = descend_to(&cst, operands[1], Rule::PreprocPrimary);
        let inner_expr = cst
            .children(inner_or_node)
            .find(|c| cst.match_rule(*c, Rule::PreprocExpr))
            .expect("inner preproc_expr under preproc_primary");
        let inner_or = PreprocExpr::from_cst(&cst, inner_expr).unwrap();
        assert_eq!(inner_or, PreprocExpr::Or);
    }

    #[test]
    fn unary_not_classifies_as_not() {
        let expr = classify("#if !defined(X)");
        assert_eq!(expr, PreprocExpr::Not);
    }

    #[test]
    fn relational_lt_classifies_as_relational() {
        let expr = classify("#if A < B");
        assert_eq!(expr, PreprocExpr::Relational(RelOp::Lt));
    }

    #[test]
    fn additive_plus_classifies_as_additive() {
        let expr = classify("#if A + 1");
        assert_eq!(expr, PreprocExpr::Additive(AddOp::Plus));
    }

    #[test]
    fn multiplicative_modulo_classifies_as_multiplicative() {
        let expr = classify("#if A % 2");
        assert_eq!(expr, PreprocExpr::Multiplicative(MulOp::Mod));
    }

    #[test]
    fn eq_eq_op_classifies_as_eq_eq() {
        let expr = classify("#if A == B");
        assert_eq!(expr, PreprocExpr::Eq(EqOp::Eq));
    }

    #[test]
    fn eq_op_each_token_kind() {
        // The op enums are exercised indirectly through the integration
        // tests above (they classify the operator at the right level).
        // Here we additionally confirm that handing an unrelated token
        // back to EqOp::from_cst returns None — the contract.
        let cst = parse("#if A == B");
        let eq_token = find_token(&cst, inner_expr_node(&cst), Token::Eq).expect("Eq token");
        assert_eq!(EqOp::from_cst(&cst, eq_token), Some(EqOp::Eq));

        let cst = parse("#if A != B");
        let neq_token = find_token(&cst, inner_expr_node(&cst), Token::Neq).expect("Neq token");
        assert_eq!(EqOp::from_cst(&cst, neq_token), Some(EqOp::Neq));
    }

    #[test]
    fn rel_op_each_token_kind() {
        for (src, want, tok) in [
            ("#if A < B", RelOp::Lt, Token::Lt),
            ("#if A > B", RelOp::Gt, Token::Gt),
            ("#if A <= B", RelOp::Leq, Token::Leq),
            ("#if A >= B", RelOp::Geq, Token::Geq),
        ] {
            let cst = parse(src);
            let op_token =
                find_token(&cst, inner_expr_node(&cst), tok).expect("op token at any depth");
            assert_eq!(RelOp::from_cst(&cst, op_token), Some(want));
        }
    }

    #[test]
    fn add_op_each_token_kind() {
        let cst = parse("#if A + B");
        let plus =
            find_token(&cst, inner_expr_node(&cst), Token::Plus).expect("Plus token");
        assert_eq!(AddOp::from_cst(&cst, plus), Some(AddOp::Plus));

        let cst = parse("#if A - B");
        let minus =
            find_token(&cst, inner_expr_node(&cst), Token::Minus).expect("Minus token");
        assert_eq!(AddOp::from_cst(&cst, minus), Some(AddOp::Minus));
    }

    #[test]
    fn mul_op_each_token_kind() {
        for (src, want, tok) in [
            ("#if A * B", MulOp::Times, Token::Star),
            ("#if A / B", MulOp::Div, Token::Slash),
            ("#if A % B", MulOp::Mod, Token::Percent),
        ] {
            let cst = parse(src);
            let op_token =
                find_token(&cst, inner_expr_node(&cst), tok).expect("op token at any depth");
            assert_eq!(MulOp::from_cst(&cst, op_token), Some(want));
        }
    }

    #[test]
    fn op_enums_reject_unrelated_tokens() {
        // Confirm the negative contract for every op enum.
        let cst = parse("#if A");
        let id = find_token(&cst, inner_expr_node(&cst), Token::Identifier).expect("id");
        assert_eq!(EqOp::from_cst(&cst, id), None);
        assert_eq!(RelOp::from_cst(&cst, id), None);
        assert_eq!(AddOp::from_cst(&cst, id), None);
        assert_eq!(MulOp::from_cst(&cst, id), None);
    }

    #[test]
    fn from_cst_returns_none_on_unrelated_node() {
        // A bare `#define` node is not a PreprocExpr.
        let cst = parse("#define X");
        let def = first_rule(&cst);
        assert!(PreprocExpr::from_cst(&cst, def).is_none());
    }

    #[test]
    fn preproc_if_struct_carries_classified_cond() {
        let cst = parse("#if defined(BAR)");
        let outer = first_rule(&cst);
        let pi = PreprocIf::from_cst(&cst, outer).expect("preproc_if");
        assert_eq!(
            pi.cond,
            PreprocExpr::Defined(Identifier::new("BAR".to_string(), 12..15))
        );
        assert_eq!(pi.span, 0..16);
    }

    #[test]
    fn preproc_elif_struct_carries_classified_cond() {
        // The grammar accepts `#elif EXPR`; exercise the Identifier variant
        // through the elif wrapper.
        let cst = parse("#elif FOO");
        let outer = first_rule(&cst);
        let el = PreprocElif::from_cst(&cst, outer).expect("preproc_elif");
        assert_eq!(
            el.cond,
            PreprocExpr::Identifier(Identifier::new("FOO".to_string(), 6..9))
        );
    }

    #[test]
    fn preproc_else_and_endif_have_spans() {
        let cst_e = parse("#else");
        let else_node = first_rule(&cst_e);
        let el = PreprocElse::from_cst(&cst_e, else_node).unwrap();
        assert_eq!(el.span, 0..5);

        let cst_n = parse("#endif");
        let end_node = first_rule(&cst_n);
        let en = PreprocEndif::from_cst(&cst_n, end_node).unwrap();
        assert_eq!(en.span, 0..6);
    }
}
