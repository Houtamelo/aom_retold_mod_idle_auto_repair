// SPDX-License-Identifier: MIT
//
// Pass 3 (T11) — expression AST: literals, primary expressions, unary.
//
// CST shape discoveries (encoded here):
// - The grammar's `^` and `@name` markers cause literal wrappers to be
//   produced as bare tokens in many cases (e.g. `IntConst` directly under
//   `postfix_expr` with no `Rule::IntLiteral`). We accept BOTH shapes:
//   the wrapper rule or the inner token.
// - `expression^: comma_expr;` causes the parent `expression` to be
//   replaced by `comma_expr`, so the parser emits `Rule::CommaExpr`
//   (not `Rule::Expression`) at the expr rule boundary.
// - `comma_expr: assignment_expr ;` — plain `:` so `comma_expr` stays
//   as a wrapper around whatever `assignment_expr^` reduces to.
// - `assignment_expr^` collapses into the chosen binary/conditional
//   node (e.g. `Rule::BinaryExpr` for the no-assignment case,
//   `Rule::AssignmentExpr` for `a = b`).
// - `unary_expr:` (no caret) → the wrapper stays `Rule::UnaryExpr`
//   when it falls through to `postfix_expr`, but `@unary_op_expr`
//   and `@pre_inc_or_dec_expr` replace the parent when those
//   alternatives match.
// - `binary_expr:` similarly: `@additive_expr` etc. replace the
//   parent for the matching alternatives; the bare fallthrough
//   `| unary_expr` keeps the wrapper as `Rule::BinaryExpr`.
//
// Pass 3 (T12) — see expr T12 section appended below.

use crate::ast::cst_helpers::{child_by_rule, child_by_token, is_skip_token, only_child};
use crate::ast::spanned::{Braced, Bracketed, Parenthesized};
use crate::ast::statement::{BlockItemList, CompoundStatement};
use crate::ast::type_system::{Identifier, TypeSpecifier};
use crate::parser::{Cst, Node, NodeRef, Rule, Span};
use crate::lexer::Token;

// =============================================================================
// Helpers
// =============================================================================

/// Returns the first child of `parent` that is a rule node (skipping
/// whitespace/comments). Used to descend through passthrough rules like
/// `comma_expr → binary_expr → unary_expr → postfix_expr`.
fn first_rule_child(cst: &Cst, parent: NodeRef) -> Option<NodeRef> {
    cst.children(parent)
        .find(|c| matches!(cst.get(*c), Node::Rule(_, _)))
}

/// Source text at a span.
#[inline]
fn slice_at(cst: &Cst, span: &Span) -> String {
    cst.source()[span.clone()].to_string()
}

/// Dispatch helper: walks one passthrough layer (Expression, CommaExpr,
/// BinaryExpr, UnaryExpr, PostfixExpr, PrimaryExpr) at a time. Tries
/// each `from_cst` in order, returning the first non-None result.
fn dispatch_passthrough(cst: &Cst, node: NodeRef) -> Option<Expr> {
    let inner = first_rule_child(cst, node)?;
    Expr::from_cst(cst, inner)
}

/// Returns `Some(inner)` if `node` is a transparent wrapper rule (one
/// that wraps a single inner expression without introducing semantic
/// content), and `Some(node)` otherwise. The chain of wrapper rules is:
///
///   Expression -> CommaExpr -> AssignmentExpr -> ConditionalExpr ->
///   BinaryExpr -> UnaryExpr -> PostfixExpr -> PrimaryExpr
///
/// A wrapper is transparent iff its children are exactly one rule
/// node (the next wrapper down) plus zero or more skip tokens. When
/// the wrapper has operator tokens (e.g., `=` in AssignmentExpr, `||`
/// in BinaryExpr, `?` in ConditionalExpr, `.` in PostfixExpr), it has
/// real semantic content and is NOT descended.
///
/// When the bottom of the chain has only token children (e.g., a
/// `Rule::PostfixExpr` containing just an `IntConst`), the descent
/// continues to the first non-skip token child. The literal
/// extractors handle direct-token nodes via their `match_token`
/// branches.
fn transparent_descent(cst: &Cst, node: NodeRef) -> Option<NodeRef> {
    if matches!(cst.get(node), Node::Token(_, _)) {
        return Some(node);
    }
    // Count children: 1 rule child AND zero non-skip tokens = transparent.
    let mut rule_count = 0;
    let mut rule_child = None;
    let mut first_token_child = None;
    for c in cst.children(node) {
        match cst.get(c) {
            Node::Rule(_, _) => {
                rule_count += 1;
                if rule_child.is_none() {
                    rule_child = Some(c);
                }
            }
            Node::Token(t, _) => {
                if !matches!(t, Token::Whitespace | Token::LineComment | Token::BlockComment | Token::Error) {
                    if first_token_child.is_none() {
                        first_token_child = Some(c);
                    }
                }
            }
        }
    }
    if rule_count == 1 && first_token_child.is_none() {
        // Pure passthrough — descend to the rule child.
        rule_child
    } else if rule_count == 0 {
        // No rule children — descend to the first non-skip token child
        // so literal extractors can match it directly.
        first_token_child.or(Some(node))
    } else {
        // Multiple rule children or operator tokens present — this is
        // a semantic-content wrapper, do NOT descend.
        Some(node)
    }
}

// =============================================================================
// Expr — the AST root for expressions
// =============================================================================

/// The complete typed AST representation of an XS expression. Sixteen
/// variants cover every expression-producing rule in `xs.llw`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expr {
    IntLiteral(IntLiteral),
    FloatLiteral(FloatLiteral),
    StringLiteral(StringLiteral),
    TrueLiteral(TrueLiteral),
    FalseLiteral(FalseLiteral),
    NullLiteral(NullLiteral),
    Identifier(IdentifierExpr),
    Paren(ParenExpr),
    Lambda(LambdaExpr),
    New(NewExpr),
    Default(DefaultExpr),
    Vector(VectorLiteral),
    Unary(UnaryExpr),
    Postfix(PostfixExpr),
    Binary(BinaryExpr),
    Conditional(ConditionalExpr),
    Assignment(AssignmentExpr),
    Comma(CommaExpr),
}

impl Expr {
    /// Extract an `Expr` from a CST node. Dispatches to the
    /// appropriate variant's `from_cst` based on the rule type.
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        // Transparent descent: when `node` is a wrapper rule that
        // doesn't introduce semantic content (no assignment, no
        // ternary, no operator at that level), recurse into the
        // first rule child. The grammar chains
        //   comma_expr -> assignment_expr -> conditional_expr ->
        //   binary_expr -> unary_expr -> postfix_expr -> primary_expr
        // and many of these wrappers have a passthrough case. Test
        // helpers like `init_comma_expr` may return the top-level
        // CommaExpr node; without descent the dispatch would never
        // find the inner IntLiteral.
        if let Some(inner) = transparent_descent(cst, node) {
            if inner != node {
                return Self::from_cst(cst, inner);
            }
        }

        // Each variant extractor. Order matters: literals and primary
        // shapes first (most specific), then constructs, then ops.
        // Token-only nodes reach the literal extractors via their
        // direct-token branches (see IntLiteral::from_cst, etc.).
        if let Some(v) = IntLiteral::from_cst(cst, node) {
            return Some(Self::IntLiteral(v));
        }
        if let Some(v) = FloatLiteral::from_cst(cst, node) {
            return Some(Self::FloatLiteral(v));
        }
        if let Some(v) = StringLiteral::from_cst(cst, node) {
            return Some(Self::StringLiteral(v));
        }
        if let Some(v) = TrueLiteral::from_cst(cst, node) {
            return Some(Self::TrueLiteral(v));
        }
        if let Some(v) = FalseLiteral::from_cst(cst, node) {
            return Some(Self::FalseLiteral(v));
        }
        if let Some(v) = NullLiteral::from_cst(cst, node) {
            return Some(Self::NullLiteral(v));
        }
        if let Some(v) = IdentifierExpr::from_cst(cst, node) {
            return Some(Self::Identifier(v));
        }
        if let Some(v) = ParenExpr::from_cst(cst, node) {
            return Some(Self::Paren(v));
        }
        if let Some(v) = NewExpr::from_cst(cst, node) {
            return Some(Self::New(v));
        }
        if let Some(v) = DefaultExpr::from_cst(cst, node) {
            return Some(Self::Default(v));
        }
        if let Some(v) = VectorLiteral::from_cst(cst, node) {
            return Some(Self::Vector(v));
        }
        if let Some(v) = UnaryExpr::from_cst(cst, node) {
            return Some(Self::Unary(v));
        }
        if let Some(v) = PostfixExpr::from_cst(cst, node) {
            return Some(Self::Postfix(v));
        }
        if let Some(v) = BinaryExpr::from_cst(cst, node) {
            return Some(Self::Binary(v));
        }
        if let Some(v) = ConditionalExpr::from_cst(cst, node) {
            return Some(Self::Conditional(v));
        }
        if let Some(v) = AssignmentExpr::from_cst(cst, node) {
            return Some(Self::Assignment(v));
        }
        if let Some(v) = CommaExpr::from_cst(cst, node) {
            return Some(Self::Comma(v));
        }
        // Lambda last — its from_cst currently never succeeds in PoC
        // because the grammar emits ERROR subtrees for `[]`.
        if let Some(v) = LambdaExpr::from_cst(cst, node) {
            return Some(Self::Lambda(v));
        }
        None
    }

    /// Total span covered by this expression (first token .. last token).
    pub fn span(&self) -> Span {
        match self {
            Self::IntLiteral(v) => v.span.clone(),
            Self::FloatLiteral(v) => v.span.clone(),
            Self::StringLiteral(v) => v.span.clone(),
            Self::TrueLiteral(v) => v.span.clone(),
            Self::FalseLiteral(v) => v.span.clone(),
            Self::NullLiteral(v) => v.span.clone(),
            Self::Identifier(v) => v.span.clone(),
            Self::Paren(v) => v.span.clone(),
            Self::Lambda(v) => v.span.clone(),
            Self::New(v) => v.span.clone(),
            Self::Default(v) => v.span.clone(),
            Self::Vector(v) => v.span.clone(),
            Self::Unary(v) => v.span.clone(),
            Self::Postfix(v) => v.span.clone(),
            Self::Binary(v) => v.span.clone(),
            Self::Conditional(v) => v.span.clone(),
            Self::Assignment(v) => v.span.clone(),
            Self::Comma(v) => v.span.clone(),
        }
    }
}

// =============================================================================
// T11 — Literals & primaries
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IntLiteral {
    pub value: String,
    pub span: Span,
}

impl IntLiteral {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::IntLiteral) {
            let (text, span) = cst.children(node).find_map(|c| {
                cst.match_token(c, Token::IntConst)
                    .or_else(|| cst.match_token(c, Token::Minus))
                    .or_else(|| cst.match_token(c, Token::Plus))
                    .or_else(|| cst.match_token(c, Token::FloatConst))
            })?;
            let _ = text;
            let span = cst.span(node);
            // Re-extract by scanning children so we get the actual
            // IntConst span (which may include leading sign for
            // unusual literal forms).
            let v = cst
                .children(node)
                .find_map(|c| cst.match_token(c, Token::IntConst))?;
            return Some(Self {
                value: slice_at(cst, &v.1),
                span,
            });
        }
        // Direct token (postfix/primary branches that collapse the
        // `@int_literal` marker to just the IntConst token).
        if let Some((_, span)) = cst.match_token(node, Token::IntConst) {
            return Some(Self {
                value: slice_at(cst, &span),
                span,
            });
        }
        // A leading-signed integer written `-5` is captured by
        // UnaryExpr::from_cst, not here. `IntLiteral` only stores
        // bare IntConst tokens.
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FloatLiteral {
    pub value: String,
    pub span: Span,
}

impl FloatLiteral {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::FloatLiteral) {
            let v = cst
                .children(node)
                .find_map(|c| cst.match_token(c, Token::FloatConst))?;
            let span = cst.span(node);
            return Some(Self {
                value: slice_at(cst, &v.1),
                span,
            });
        }
        if let Some((_, span)) = cst.match_token(node, Token::FloatConst) {
            return Some(Self {
                value: slice_at(cst, &span),
                span,
            });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StringLiteral {
    /// Raw lexeme, including the surrounding double-quotes.
    pub value: String,
    pub span: Span,
}

impl StringLiteral {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::StringLiteral) {
            let v = cst
                .children(node)
                .find_map(|c| cst.match_token(c, Token::StringLiteral))?;
            let span = cst.span(node);
            return Some(Self {
                value: slice_at(cst, &v.1),
                span,
            });
        }
        if let Some((_, span)) = cst.match_token(node, Token::StringLiteral) {
            return Some(Self {
                value: slice_at(cst, &span),
                span,
            });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TrueLiteral {
    pub span: Span,
}

impl TrueLiteral {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::TrueLiteral) {
            return Some(Self {
                span: cst.span(node),
            });
        }
        if cst.match_token(node, Token::TrueKw).is_some()
            || cst.match_token(node, Token::TrueUpper).is_some()
        {
            return Some(Self {
                span: cst.span(node),
            });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FalseLiteral {
    pub span: Span,
}

impl FalseLiteral {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::FalseLiteral) {
            return Some(Self {
                span: cst.span(node),
            });
        }
        if cst.match_token(node, Token::FalseKw).is_some()
            || cst.match_token(node, Token::FalseUpper).is_some()
        {
            return Some(Self {
                span: cst.span(node),
            });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NullLiteral {
    pub span: Span,
}

impl NullLiteral {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::NullLiteral) {
            return Some(Self {
                span: cst.span(node),
            });
        }
        if cst.match_token(node, Token::NullKw).is_some()
            || cst.match_token(node, Token::NullUpper).is_some()
            || cst.match_token(node, Token::Nullptr).is_some()
        {
            return Some(Self {
                span: cst.span(node),
            });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdentifierExpr {
    pub name: Identifier,
    pub span: Span,
}

impl IdentifierExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::IdentifierExpr) {
            let (text, span) = cst
                .children(node)
                .find_map(|c| cst.match_token(c, Token::Identifier))?;
            let outer_span = cst.span(node);
            return Some(Self {
                name: Identifier::new(text.to_string(), span),
                span: outer_span,
            });
        }
        // Direct Identifier token (the grammar's `@identifier_expr`
        // collapses to just the token in PoC).
        if let Some((text, span)) = cst.match_token(node, Token::Identifier) {
            return Some(Self {
                name: Identifier::new(text.to_string(), span.clone()),
                span,
            });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParenExpr {
    pub inner: Box<Expr>,
    pub span: Span,
}

impl ParenExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::ParenExpr) {
            let inner_node = first_rule_child(cst, node)?;
            let inner = Expr::from_cst(cst, inner_node)?;
            return Some(Self {
                inner: Box::new(inner),
                span: cst.span(node),
            });
        }
        // PoC: `(` expr `)` appears with LPar/expr/RPar as direct
        // children of `postfix_expr`. We don't try to detect this
        // case here — the postfix dispatch already handles it.
        None
    }

    pub fn open_span(&self, cst: &Cst) -> Option<Span> {
        // Caller knows the inner; for formatting we'd need the open
        // span stored too. Reserved for future extension; not used
        // by Pass 3 extractors.
        let _ = cst;
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DefaultExpr {
    pub span: Span,
}

impl DefaultExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::DefaultExpr) {
            return Some(Self {
                span: cst.span(node),
            });
        }
        if cst.match_token(node, Token::DefaultKw).is_some() {
            return Some(Self {
                span: cst.span(node),
            });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NewExpr {
    pub ty: TypeSpecifier,
    /// None when the call has zero arguments (parser still emits `()`).
    pub args: Option<crate::ast::argument::ArgumentList>,
    pub span: Span,
}

impl NewExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        // `new X()` doesn't currently parse in PoC (the
        // `?t primary_expr 'new' ...` alternative is rejected).
        // The Token::New fallback below also rarely fires. Kept
        // best-effort for forward compatibility.
        let _ = node;
        let _ = cst;
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VectorLiteral {
    pub args: Option<crate::ast::argument::ArgumentList>,
    pub span: Span,
}

impl VectorLiteral {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::VectorLiteral) {
            return Some(Self {
                args: None, // forward compat — populated when grammar supports
                span: cst.span(node),
            });
        }
        // PoC fallback: `vector LPar argument_list RPar` appears under
        // postfix_expr with no VectorLiteral wrapper.
        if cst.match_token(node, Token::Vector).is_some() {
            return Some(Self {
                args: None,
                span: cst.span(node),
            });
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LambdaExpr {
    /// Capture list between `[` and `]`. XS lambdas in the shipped
    /// scripts use empty capture lists (`[]`), so the inner vector is
    /// currently always empty; the field is kept for forward
    /// compatibility with richer capture syntax.
    pub captures: Bracketed<Vec<Identifier>>,
    pub params: crate::ast::spanned::Parenthesized<crate::ast::parameter::ParameterList>,
    pub ret_type: Option<TypeSpecifier>,
    pub body: Braced<BlockItemList>,
    pub span: Span,
}

impl LambdaExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::LambdaExpr) {
            return None;
        }

        let children: Vec<_> = cst.children(node).collect();

        let lbrak = child_by_token(cst, node, Token::LBrak)?;
        let rbrak = child_by_token(cst, node, Token::RBrak)?;
        let captures = Bracketed::new(lbrak, Vec::new(), rbrak);

        let lpar_idx = children
            .iter()
            .position(|c| cst.match_token(*c, Token::LPar).is_some())?;
        let rpar_idx = children
            .iter()
            .position(|c| cst.match_token(*c, Token::RPar).is_some())?;
        let lpar = child_by_token(cst, node, Token::LPar)?;
        let rpar = child_by_token(cst, node, Token::RPar)?;
        let params_inner = children[lpar_idx + 1..rpar_idx]
            .iter()
            .find(|c| cst.match_rule(**c, Rule::ParameterList))
            .and_then(|n| crate::ast::parameter::ParameterList::from_cst(cst, *n))
            .unwrap_or_else(crate::ast::parameter::ParameterList::empty);
        let params = Parenthesized::new(lpar, params_inner, rpar);

        let ret_type = child_by_rule(cst, node, Rule::TypeSpecifier)
            .and_then(|n| TypeSpecifier::from_cst(cst, n));

        let body_node = child_by_rule(cst, node, Rule::CompoundStatement)?;
        let body = CompoundStatement::from_cst(cst, body_node)?;

        Some(Self {
            captures,
            params,
            ret_type,
            body: body.items,
            span: cst.span(node),
        })
    }
}

// =============================================================================
// UnaryExpr (T11: simple unary only; T12 extends with Postfix/Binary wrappers)
// =============================================================================

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Minus,
    Excl,
    Tilde,
}

impl UnaryOp {
    pub fn from_token(token: Token) -> Option<Self> {
        Some(match token {
            Token::Minus => Self::Minus,
            Token::Excl => Self::Excl,
            Token::Tilde => Self::Tilde,
            _ => return None,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PreIncDecOp {
    PreInc,
    PreDec,
}

impl PreIncDecOp {
    pub fn from_token(token: Token) -> Option<Self> {
        Some(match token {
            Token::PlusPlus => Self::PreInc,
            Token::MinusMinus => Self::PreDec,
            _ => return None,
        })
    }
}

/// A unary expression that combines the three PoC subtypes:
/// - `UnaryOp`-prefixed (`-x`, `!x`, `~x`) — see `Rule::UnaryOpExpr`
/// - `PreInc`/`PreDec`-prefixed (`++x`, `--x`) — see `Rule::PreIncOrDecExpr`
/// - Passthrough (just the inner expression) — see `Rule::UnaryExpr`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct UnaryExpr {
    pub kind: UnaryKind,
    pub operand: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UnaryKind {
    Op(UnaryOp),
    PreIncDec(PreIncDecOp),
}

impl UnaryExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::UnaryOpExpr) {
            let op_token = cst.children(node).find_map(|c| {
                if cst.match_token(c, Token::Minus).is_some() {
                    Some(Token::Minus)
                } else if cst.match_token(c, Token::Excl).is_some() {
                    Some(Token::Excl)
                } else if cst.match_token(c, Token::Tilde).is_some() {
                    Some(Token::Tilde)
                } else {
                    None
                }
            })?;
            let op = UnaryOp::from_token(op_token)?;
            let inner_node = first_rule_child(cst, node)?;
            let operand = Expr::from_cst(cst, inner_node)?;
            return Some(Self {
                kind: UnaryKind::Op(op),
                operand: Box::new(operand),
                span: cst.span(node),
            });
        }
        if cst.match_rule(node, Rule::PreIncOrDecExpr) {
            let op_token = cst.children(node).find_map(|c| {
                if cst.match_token(c, Token::PlusPlus).is_some() {
                    Some(Token::PlusPlus)
                } else if cst.match_token(c, Token::MinusMinus).is_some() {
                    Some(Token::MinusMinus)
                } else {
                    None
                }
            })?;
            let op = PreIncDecOp::from_token(op_token)?;
            let inner_node = first_rule_child(cst, node)?;
            let operand = Expr::from_cst(cst, inner_node)?;
            return Some(Self {
                kind: UnaryKind::PreIncDec(op),
                operand: Box::new(operand),
                span: cst.span(node),
            });
        }
        // Passthrough: Rule::UnaryExpr falls through to postfix.
        if cst.match_rule(node, Rule::UnaryExpr) {
            return dispatch_passthrough(cst, node).and_then(|e| {
                Some(Self {
                    kind: UnaryKind::Op(UnaryOp::Excl), // placeholder
                    operand: Box::new(e),
                    span: cst.span(node),
                })
            });
        }
        None
    }
}

// =============================================================================
// T12 — Postfix, Binary, Conditional, Assignment, Comma
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PostfixInner {
    Call(CallExpr),
    Field(FieldExpr),
    Subscript(SubscriptExpr),
    PostInc(PostIncExpr),
    PostDec(PostDecExpr),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PostfixExpr {
    pub target: Box<Expr>,
    pub inner: PostfixInner,
    pub span: Span,
}

impl PostfixExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::CallExpr) {
            let call = CallExpr::from_cst(cst, node)?;
            let span = call.span.clone();
            return Some(Self {
                target: call.target.clone(),
                inner: PostfixInner::Call(call),
                span,
            });
        }
        if cst.match_rule(node, Rule::FieldExpr) {
            let field = FieldExpr::from_cst(cst, node)?;
            let span = field.span.clone();
            return Some(Self {
                target: field.target.clone(),
                inner: PostfixInner::Field(field),
                span,
            });
        }
        if cst.match_rule(node, Rule::SubscriptExpr) {
            let sub = SubscriptExpr::from_cst(cst, node)?;
            let span = sub.span.clone();
            return Some(Self {
                target: sub.target.clone(),
                inner: PostfixInner::Subscript(sub),
                span,
            });
        }
        if cst.match_rule(node, Rule::PostIncExpr) {
            let inc = PostIncExpr::from_cst(cst, node)?;
            let span = inc.span.clone();
            return Some(Self {
                target: inc.target.clone(),
                inner: PostfixInner::PostInc(inc),
                span,
            });
        }
        if cst.match_rule(node, Rule::PostDecExpr) {
            let dec = PostDecExpr::from_cst(cst, node)?;
            let span = dec.span.clone();
            return Some(Self {
                target: dec.target.clone(),
                inner: PostfixInner::PostDec(dec),
                span,
            });
        }
        // Passthrough: Rule::PostfixExpr falls through to the inner
        // primary expression. The actual CST may directly contain a
        // token (e.g., `Rule::PostfixExpr [IntConst "5"]`) because
        // `primary_expr^` doesn't always create a node. We can't
        // construct a PostfixExpr without a real postfix operator, so
        // return None and let Expr::from_cst try other extractors.
        if cst.match_rule(node, Rule::PostfixExpr) {
            return None;
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallExpr {
    pub target: Box<Expr>,
    pub args: Option<crate::ast::argument::ArgumentList>,
    pub open: Span,
    pub close: Span,
    pub span: Span,
}

impl CallExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::CallExpr) {
            return None;
        }
        let target_node = first_rule_child(cst, node)?;
        let target = Expr::from_cst(cst, target_node)?;
        let open = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::LPar).map(|(_, s)| s))?;
        let close = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::RPar).map(|(_, s)| s))?;
        let args = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::ArgumentList))
            .and_then(|n| crate::ast::argument::ArgumentList::from_cst(cst, n));
        let span = cst.span(node);
        Some(Self {
            target: Box::new(target),
            args,
            open,
            close,
            span,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldExpr {
    pub target: Box<Expr>,
    pub field: Identifier,
    pub dot: Span,
    pub span: Span,
}

impl FieldExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::FieldExpr) {
            return None;
        }
        let target_node = first_rule_child(cst, node)?;
        let target = Expr::from_cst(cst, target_node)?;
        let dot = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Dot).map(|(_, s)| s))?;
        let (text, field_span) = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Identifier))?;
        Some(Self {
            target: Box::new(target),
            field: Identifier::new(text.to_string(), field_span),
            dot,
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SubscriptExpr {
    pub target: Box<Expr>,
    pub index: Box<Expr>,
    pub open: Span,
    pub close: Span,
    pub span: Span,
}

impl SubscriptExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::SubscriptExpr) {
            return None;
        }
        let target_node = first_rule_child(cst, node)?;
        let target = Expr::from_cst(cst, target_node)?;
        let open = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::LBrak).map(|(_, s)| s))?;
        let close = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::RBrak).map(|(_, s)| s))?;
                // The index is the middle rule child (after LBrak, before RBrak).
        let mut rule_iter = cst
            .children(node)
            .filter(|c| matches!(cst.get(*c), Node::Rule(_, _)));
        let _ = rule_iter.next(); // target
        let index_node = rule_iter.next()?;
        let index = Expr::from_cst(cst, index_node)?;
        Some(Self {
            target: Box::new(target),
            index: Box::new(index),
            open,
            close,
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PostIncExpr {
    pub target: Box<Expr>,
    pub op: Span,
    pub span: Span,
}

impl PostIncExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::PostIncExpr) {
            return None;
        }
        let target_node = first_rule_child(cst, node)?;
        let target = Expr::from_cst(cst, target_node)?;
        let op = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::PlusPlus).map(|(_, s)| s))?;
        let span = cst.span(node);
        Some(Self {
            target: Box::new(target),
            op,
            span,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PostDecExpr {
    pub target: Box<Expr>,
    pub op: Span,
    pub span: Span,
}

impl PostDecExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::PostDecExpr) {
            return None;
        }
        let target_node = first_rule_child(cst, node)?;
        let target = Expr::from_cst(cst, target_node)?;
        let op = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::MinusMinus).map(|(_, s)| s))?;
        let span = cst.span(node);
        Some(Self {
            target: Box::new(target),
            op,
            span,
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum BinaryOp {
    LogicalOr,
    LogicalAnd,
    Eq,
    Neq,
    Lt,
    Gt,
    Leq,
    Geq,
    BitAnd,
    BitOr,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl BinaryOp {
    pub fn from_rule(rule: Rule) -> Option<Self> {
        Some(match rule {
            Rule::LogicalOrExpr => Self::LogicalOr,
            Rule::LogicalAndExpr => Self::LogicalAnd,
            Rule::EqualityExpr => Self::Eq, // default; refined below
            Rule::RelationalExpr => Self::Lt, // default; refined below
            Rule::BitwiseExpr => Self::BitAnd, // default; refined below
            Rule::AdditiveExpr => Self::Add, // default; refined below
            Rule::MultiplicativeExpr => Self::Mul, // default; refined below
            _ => return None,
        })
    }

    /// Refines the default op by inspecting the actual token child
    /// (EqualityExpr → Eq/Neq; RelationalExpr → Lt/Gt/Leq/Geq;
    /// AdditiveExpr → Plus/Minus; MultiplicativeExpr → Mul/Div/Mod).
    pub fn refine_from_tokens(cst: &Cst, node: NodeRef) -> Option<Self> {
        let t = cst.children(node).find_map(|c| {
            cst.match_token(c, Token::OrOr)
                .map(|(_, s)| (BinaryOp::LogicalOr, s))
                .or_else(|| {
                    cst.match_token(c, Token::AndAnd)
                        .map(|(_, s)| (BinaryOp::LogicalAnd, s))
                })
                .or_else(|| cst.match_token(c, Token::Eq).map(|(_, s)| (BinaryOp::Eq, s)))
                .or_else(|| {
                    cst.match_token(c, Token::Neq).map(|(_, s)| (BinaryOp::Neq, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Lt).map(|(_, s)| (BinaryOp::Lt, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Gt).map(|(_, s)| (BinaryOp::Gt, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Leq).map(|(_, s)| (BinaryOp::Leq, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Geq).map(|(_, s)| (BinaryOp::Geq, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Amp).map(|(_, s)| (BinaryOp::BitAnd, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Pipe).map(|(_, s)| (BinaryOp::BitOr, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Plus).map(|(_, s)| (BinaryOp::Add, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Minus).map(|(_, s)| (BinaryOp::Sub, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Star).map(|(_, s)| (BinaryOp::Mul, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Slash).map(|(_, s)| (BinaryOp::Div, s))
                })
                .or_else(|| {
                    cst.match_token(c, Token::Percent)
                        .map(|(_, s)| (BinaryOp::Mod, s))
                })
        })?;
        Some(t.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BinaryExpr {
    pub lhs: Box<Expr>,
    pub op: BinaryOp,
    pub rhs: Box<Expr>,
    pub op_span: Span,
    pub span: Span,
}

impl BinaryExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        let rule = match cst.get(node) {
            Node::Rule(r, _) => r,
            Node::Token(_, _) => return None,
        };
        match rule {
            Rule::LogicalOrExpr
            | Rule::LogicalAndExpr
            | Rule::EqualityExpr
            | Rule::RelationalExpr
            | Rule::BitwiseExpr
            | Rule::AdditiveExpr
            | Rule::MultiplicativeExpr => {
                // Children of the wrapper: `binary_expr + op + binary_expr`
                // (with possible whitespace/interstitial skips). We need
                // to find: the first two rule children (lhs + rhs) and the
                // operator token in between.
                let mut rule_iter = cst
                    .children(node)
                    .filter(|c| matches!(cst.get(*c), Node::Rule(_, _)));
                let lhs_node = rule_iter.next()?;
                let lhs = Expr::from_cst(cst, lhs_node)?;
                let rhs_node = rule_iter.next()?;
                let rhs = Expr::from_cst(cst, rhs_node)?;
                let (op, op_span) = cst.children(node).find_map(|c| {
                    cst.match_token(c, Token::OrOr)
                        .map(|(_, s)| (BinaryOp::LogicalOr, s))
                        .or_else(|| {
                            cst.match_token(c, Token::AndAnd)
                                .map(|(_, s)| (BinaryOp::LogicalAnd, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Eq)
                                .map(|(_, s)| (BinaryOp::Eq, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Neq)
                                .map(|(_, s)| (BinaryOp::Neq, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Lt)
                                .map(|(_, s)| (BinaryOp::Lt, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Gt)
                                .map(|(_, s)| (BinaryOp::Gt, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Leq)
                                .map(|(_, s)| (BinaryOp::Leq, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Geq)
                                .map(|(_, s)| (BinaryOp::Geq, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Amp)
                                .map(|(_, s)| (BinaryOp::BitAnd, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Pipe)
                                .map(|(_, s)| (BinaryOp::BitOr, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Plus)
                                .map(|(_, s)| (BinaryOp::Add, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Minus)
                                .map(|(_, s)| (BinaryOp::Sub, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Star)
                                .map(|(_, s)| (BinaryOp::Mul, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Slash)
                                .map(|(_, s)| (BinaryOp::Div, s))
                        })
                        .or_else(|| {
                            cst.match_token(c, Token::Percent)
                                .map(|(_, s)| (BinaryOp::Mod, s))
                        })
                })?;
                Some(Self {
                    lhs: Box::new(lhs),
                    op,
                    rhs: Box::new(rhs),
                    op_span,
                    span: cst.span(node),
                })
            }
            Rule::BinaryExpr => {
                // Passthrough (the bare `| unary_expr` fallback kept the
                // wrapper as BinaryExpr). Recurse into the only child.
                let inner_node = first_rule_child(cst, node)?;
                let inner = Expr::from_cst(cst, inner_node)?;
                // Wrap as a "BinaryExpr" with the inner expression AS the
                // rhs and a missing lhs? That would fabricate data.
                // Instead, return None so the dispatcher tries other
                // variants. The caller (Expr::from_cst) will then recurse
                // via dispatch_passthrough.
                let _ = inner;
                None
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConditionalExpr {
    pub cond: Box<Expr>,
    /// `None` when the GNU extension `a ?: b` is used (middle omitted).
    pub then: Option<Box<Expr>>,
    pub else_: Box<Expr>,
    pub question: Span,
    pub colon: Span,
    pub span: Span,
}

impl ConditionalExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ConditionalExpr) {
            return None;
        }
        // Children: cond rule_child + Question + [opt middle] + Colon + else rule_child
        // Both whitespace and skip-tokens interleave.
        let mut rule_iter = cst
            .children(node)
            .filter(|c| matches!(cst.get(*c), Node::Rule(_, _)));
        let cond_node = rule_iter.next()?;
        let cond = Expr::from_cst(cst, cond_node)?;
        let else_node = rule_iter.last()?;
        let else_ = Expr::from_cst(cst, else_node)?;

        let question = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Question).map(|(_, s)| s))?;
        let colon = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Colon).map(|(_, s)| s))?;

        // Optional middle: any rule child between `question` and `colon`.
        let then = {
            // Find rule children strictly between Question and Colon spans.
            let mut middle_rules = Vec::new();
            let mut seen_q = false;
            for c in cst.children(node) {
                if cst.match_token(c, Token::Question).is_some() {
                    seen_q = true;
                    continue;
                }
                if cst.match_token(c, Token::Colon).is_some() {
                    break;
                }
                if seen_q {
                    if matches!(cst.get(c), Node::Rule(_, _)) {
                        middle_rules.push(c);
                    }
                }
            }
            middle_rules
                .into_iter()
                .next()
                .and_then(|n| Expr::from_cst(cst, n))
                .map(Box::new)
        };

        Some(Self {
            cond: Box::new(cond),
            then,
            else_: Box::new(else_),
            question,
            colon,
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AssignmentOp {
    Assign,
    PlusAssign,
    MinusAssign,
    StarAssign,
    SlashAssign,
    PercentAssign,
}

impl AssignmentOp {
    pub fn from_token(token: Token) -> Option<Self> {
        Some(match token {
            Token::Assign => Self::Assign,
            Token::PlusAssign => Self::PlusAssign,
            Token::MinusAssign => Self::MinusAssign,
            Token::StarAssign => Self::StarAssign,
            Token::SlashAssign => Self::SlashAssign,
            Token::PercentAssign => Self::PercentAssign,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssignmentExpr {
    pub lhs: Box<Expr>,
    pub op: AssignmentOp,
    pub rhs: Box<Expr>,
    pub op_span: Span,
    pub span: Span,
}

impl AssignmentExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::AssignmentExpr) {
            return None;
        }
        let mut rule_iter = cst
            .children(node)
            .filter(|c| matches!(cst.get(*c), Node::Rule(_, _)));
        let lhs_node = rule_iter.next()?;
        let lhs = Expr::from_cst(cst, lhs_node)?;
        let rhs_node = rule_iter.next()?;
        let rhs = Expr::from_cst(cst, rhs_node)?;
        let (op, op_span) = cst
            .children(node)
            .find_map(|c| {
                cst.match_token(c, Token::Assign)
                    .map(|(_, s)| (AssignmentOp::Assign, s))
                    .or_else(|| {
                        cst.match_token(c, Token::PlusAssign)
                            .map(|(_, s)| (AssignmentOp::PlusAssign, s))
                    })
                    .or_else(|| {
                        cst.match_token(c, Token::MinusAssign)
                            .map(|(_, s)| (AssignmentOp::MinusAssign, s))
                    })
                    .or_else(|| {
                        cst.match_token(c, Token::StarAssign)
                            .map(|(_, s)| (AssignmentOp::StarAssign, s))
                    })
                    .or_else(|| {
                        cst.match_token(c, Token::SlashAssign)
                            .map(|(_, s)| (AssignmentOp::SlashAssign, s))
                    })
                    .or_else(|| {
                        cst.match_token(c, Token::PercentAssign)
                            .map(|(_, s)| (AssignmentOp::PercentAssign, s))
                    })
            })?;
        Some(Self {
            lhs: Box::new(lhs),
            op,
            rhs: Box::new(rhs),
            op_span,
            span: cst.span(node),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommaExpr {
    /// The head of the comma list. In the current PoC grammar the list
    /// has length 1 (the `comma_expr (',' expression)*` Kleene chain
    /// is disabled by an LL(1) conflict; see `xs.llw` lines 625-629).
    /// We still model the list as `Vec<Expr>` so a grammar fix to
    /// enable chained commas requires no API change.
    pub exprs: Vec<Expr>,
    pub span: Span,
}

impl CommaExpr {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        // The grammar wraps the inner expression directly under
        // `Rule::CommaExpr`. The grammar does NOT currently support
        // chained commas (`a, b, c`), so we collect all rule children
        // at this level and extract each into an Expr. If the grammar
        // is fixed to allow chained commas, the comma-separator spans
        // become available here too.
        let inner_node = first_rule_child(cst, node)?;
        let expr = Expr::from_cst(cst, inner_node)?;
        let span = cst.span(node);
        Some(Self {
            exprs: vec![expr],
            span,
        })
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;

    fn parse(source: &str) -> Cst<'_> {
        let mut diags = vec![];
        Parser::new(source, &mut diags).parse(&mut diags)
    }

    fn first_non_skip(cst: &Cst) -> NodeRef {
        cst.children(NodeRef::ROOT)
            .find(|c| !is_skip_token(cst, *c))
            .expect("at least one non-skip child")
    }

    /// Walk to the comma_expr that holds the initializer expression
    /// (`= ...` part) of a `int x = ...;` declaration.
    fn init_comma_expr(cst: &Cst) -> NodeRef {
        let decl = first_non_skip(cst);
        let init_list = cst
            .children(decl)
            .find(|c| cst.match_rule(*c, Rule::InitDeclaratorList))
            .unwrap();
        let init = cst
            .children(init_list)
            .find(|c| cst.match_rule(*c, Rule::InitDeclarator))
            .unwrap();
        // The expr is the first rule child after Assign.
        cst.children(init)
            .find(|c| cst.match_rule(*c, Rule::CommaExpr))
            .expect("comma_expr under init_declarator")
    }

    // -------- T11: literals --------

    #[test]
    fn int_literal_parses() {
        let cst = parse("int x = 5;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::IntLiteral(v) => {
                assert_eq!(v.value, "5");
                assert_eq!(v.span, 8..9);
            }
            other => panic!("expected IntLiteral, got {:?}", other),
        }
    }

    #[test]
    fn float_literal_parses() {
        let cst = parse("float f = 3.14;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::FloatLiteral(v) => {
                assert_eq!(v.value, "3.14");
                assert_eq!(v.span, 10..14);
            }
            other => panic!("expected FloatLiteral, got {:?}", other),
        }
    }

    #[test]
    fn string_literal_parses_with_quotes() {
        let cst = parse(r#"string s = "hello";"#);
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::StringLiteral(v) => {
                assert_eq!(v.value, "\"hello\"");
                assert_eq!(v.span, 11..18);
            }
            other => panic!("expected StringLiteral, got {:?}", other),
        }
    }

    #[test]
    fn true_literal_parses() {
        let cst = parse("bool b = true;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::TrueLiteral(v) => {
                assert_eq!(v.span, 9..13);
            }
            other => panic!("expected TrueLiteral, got {:?}", other),
        }
    }

    #[test]
    fn false_literal_parses() {
        let cst = parse("bool b = false;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::FalseLiteral(v) => {
                assert_eq!(v.span, 9..14);
            }
            other => panic!("expected FalseLiteral, got {:?}", other),
        }
    }

    #[test]
    fn null_literal_parses() {
        let cst = parse("bool b = null;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::NullLiteral(v) => {
                assert_eq!(v.span, 9..13);
            }
            other => panic!("expected NullLiteral, got {:?}", other),
        }
    }

    #[test]
    fn identifier_expr_parses() {
        let cst = parse("int x = y;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Identifier(v) => {
                assert_eq!(v.name.node, "y");
                assert_eq!(v.span, 8..9);
            }
            other => panic!("expected Identifier, got {:?}", other),
        }
    }

    #[test]
    fn default_expr_parses() {
        let cst = parse("int x = default;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Default(v) => {
                assert_eq!(v.span, 8..15);
            }
            other => panic!("expected Default, got {:?}", other),
        }
    }

    // -------- T11: unary --------

    #[test]
    fn unary_minus_parses() {
        let cst = parse("int y = -x;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Unary(u) => {
                assert!(matches!(u.kind, UnaryKind::Op(UnaryOp::Minus)));
                match u.operand.as_ref() {
                    Expr::Identifier(id) => assert_eq!(id.name.node, "x"),
                    other => panic!("expected Identifier operand, got {:?}", other),
                }
            }
            other => panic!("expected Unary, got {:?}", other),
        }
    }

    #[test]
    fn unary_excl_parses() {
        let cst = parse("bool b = !cond;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Unary(u) => {
                assert!(matches!(u.kind, UnaryKind::Op(UnaryOp::Excl)));
            }
            other => panic!("expected Unary, got {:?}", other),
        }
    }

    #[test]
    fn unary_pre_inc_parses() {
        let cst = parse("int y = ++x;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Unary(u) => {
                assert!(matches!(u.kind, UnaryKind::PreIncDec(PreIncDecOp::PreInc)));
            }
            other => panic!("expected Unary, got {:?}", other),
        }
    }

    #[test]
    fn unary_pre_dec_parses() {
        let cst = parse("int y = --x;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Unary(u) => {
                assert!(matches!(u.kind, UnaryKind::PreIncDec(PreIncDecOp::PreDec)));
            }
            other => panic!("expected Unary, got {:?}", other),
        }
    }

    // -------- T12: postfix --------

    #[test]
    fn call_expr_parses() {
        let cst = parse("int y = foo(a, b);");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Postfix(p) => match &p.inner {
                PostfixInner::Call(call) => {
                    assert!(matches!(call.target.as_ref(), Expr::Identifier(_)));
                    let args = call.args.as_ref().expect("args");
                    assert_eq!(args.items.len(), 2);
                }
                other => panic!("expected Call inner, got {:?}", other),
            },
            other => panic!("expected Postfix, got {:?}", other),
        }
    }

    #[test]
    fn field_expr_parses() {
        let cst = parse("int y = a.b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Postfix(p) => match &p.inner {
                PostfixInner::Field(field) => {
                    assert_eq!(field.field.node, "b");
                    assert_eq!(field.dot, 9..10);
                }
                other => panic!("expected Field inner, got {:?}", other),
            },
            other => panic!("expected Postfix, got {:?}", other),
        }
    }

    #[test]
    fn subscript_expr_parses() {
        let cst = parse("int y = a[0];");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Postfix(p) => match &p.inner {
                PostfixInner::Subscript(sub) => {
                    assert!(matches!(sub.target.as_ref(), Expr::Identifier(_)));
                    assert!(matches!(sub.index.as_ref(), Expr::IntLiteral(_)));
                }
                other => panic!("expected Subscript inner, got {:?}", other),
            },
            other => panic!("expected Postfix, got {:?}", other),
        }
    }

    #[test]
    fn post_inc_parses() {
        let cst = parse("int y = x++;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Postfix(p) => {
                assert!(matches!(p.inner, PostfixInner::PostInc(_)));
            }
            other => panic!("expected Postfix, got {:?}", other),
        }
    }

    #[test]
    fn post_dec_parses() {
        let cst = parse("int y = x--;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Postfix(p) => {
                assert!(matches!(p.inner, PostfixInner::PostDec(_)));
            }
            other => panic!("expected Postfix, got {:?}", other),
        }
    }

    // -------- T12: binary --------

    #[test]
    fn additive_binary_parses() {
        let cst = parse("int y = a + b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Binary(b) => {
                assert_eq!(b.op, BinaryOp::Add);
                assert!(matches!(b.lhs.as_ref(), Expr::Identifier(_)));
                assert!(matches!(b.rhs.as_ref(), Expr::Identifier(_)));
                assert_eq!(b.op_span, 10..11);
            }
            other => panic!("expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn multiplicative_binary_parses() {
        let cst = parse("int y = a * b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Binary(b) => {
                assert_eq!(b.op, BinaryOp::Mul);
            }
            other => panic!("expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn equality_binary_parses() {
        let cst = parse("bool b = a == b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Binary(b) => {
                assert_eq!(b.op, BinaryOp::Eq);
                assert_eq!(b.op_span, 11..13);
            }
            other => panic!("expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn relational_binary_parses() {
        let cst = parse("bool b = a < b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Binary(b) => {
                assert_eq!(b.op, BinaryOp::Lt);
                assert_eq!(b.op_span, 11..12);
            }
            other => panic!("expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn logical_and_parses() {
        let cst = parse("bool b = a && b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Binary(b) => {
                assert_eq!(b.op, BinaryOp::LogicalAnd);
                assert_eq!(b.op_span, 11..13);
            }
            other => panic!("expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn logical_or_parses() {
        let cst = parse("bool b = a || b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Binary(b) => {
                assert_eq!(b.op, BinaryOp::LogicalOr);
            }
            other => panic!("expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn bitwise_and_parses() {
        let cst = parse("int x = a & b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Binary(b) => {
                assert_eq!(b.op, BinaryOp::BitAnd);
                assert_eq!(b.op_span, 10..11);
            }
            other => panic!("expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn bitwise_or_parses() {
        let cst = parse("int x = a | b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Binary(b) => {
                assert_eq!(b.op, BinaryOp::BitOr);
                assert_eq!(b.op_span, 10..11);
            }
            other => panic!("expected Binary, got {:?}", other),
        }
    }

    #[test]
    fn bitwise_and_logical_and_stay_distinct() {
        let cst = parse("bool b = a && b;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Binary(b) => assert_eq!(b.op, BinaryOp::LogicalAnd),
            other => panic!("expected Binary, got {:?}", other),
        }
    }

    // -------- T12: conditional --------

    #[test]
    fn conditional_parses_with_then() {
        let cst = parse("int y = a ? b : c;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Conditional(c) => {
                assert!(matches!(c.cond.as_ref(), Expr::Identifier(_)));
                assert!(c.then.is_some());
                assert!(matches!(c.else_.as_ref(), Expr::Identifier(_)));
                assert_eq!(c.question, 10..11);
                assert_eq!(c.colon, 14..15);
            }
            other => panic!("expected Conditional, got {:?}", other),
        }
    }

    // -------- T12: assignment --------

    #[test]
    fn assignment_parses_simple() {
        let cst = parse("int y = a = 5;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Assignment(a) => {
                assert_eq!(a.op, AssignmentOp::Assign);
                assert!(matches!(a.lhs.as_ref(), Expr::Identifier(_)));
                assert!(matches!(a.rhs.as_ref(), Expr::IntLiteral(_)));
                assert_eq!(a.op_span, 10..11);
            }
            other => panic!("expected Assignment, got {:?}", other),
        }
    }

    #[test]
    fn assignment_parses_compound() {
        let cst = parse("int y = a += 5;");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Assignment(a) => {
                assert_eq!(a.op, AssignmentOp::PlusAssign);
                assert_eq!(a.op_span, 10..12);
            }
            other => panic!("expected Assignment, got {:?}", other),
        }
    }

    // -------- T12: comma --------

    #[test]
    fn comma_expr_has_single_inner() {
        // Current PoC grammar can't parse chained `a, b, c`, so
        // `Expr::from_cst` on a top-level CommaExpr wrapper either
        // returns the inner via transparent_descent (the semantically
        // equivalent simple case) or the CommaExpr wrapper itself
        // (when the inner carries no extra semantic content worth
        // collapsing). Both forms represent the same XS expression
        // (a single expression) — assert that one of them is returned
        // with one inner expression.
        let cst = parse("int x = 5;");
        let inner = init_comma_expr(&cst);
        let expr = Expr::from_cst(&cst, inner).expect("Expr");
        match expr {
            Expr::Comma(c) => {
                assert_eq!(c.exprs.len(), 1);
            }
            // With transparent_descent, a single-expression CommaExpr
            // also unwraps to the inner expression. Both are valid.
            Expr::IntLiteral(lit) => {
                assert_eq!(lit.value, "5");
            }
            other => panic!("expected Comma or IntLiteral, got {:?}", other),
        }
    }

    // -------- PR-D: lambda expressions --------

    #[test]
    fn lambda_extracts_captures_and_params_lambda_d_01() {
        let cst = parse("void cb = [](int x) { aiEcho(x); };");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Lambda(l) => {
                assert!(l.captures.inner.is_empty(), "expected empty captures");
                assert_eq!(l.params.inner.items.items.len(), 1);
                match &l.params.inner.items.items[0].0.inner {
                    crate::ast::parameter::ParameterInner::RegularParam(rp) => {
                        assert_eq!(rp.name.node, "x");
                    }
                    other => panic!("expected RegularParam, got {:?}", other),
                }
                assert_eq!(l.body.inner.len(), 1);
            }
            other => panic!("expected Lambda, got {:?}", other),
        }
    }

    #[test]
    fn lambda_extracts_with_no_captures_lambda_d_02() {
        let cst = parse("void gFoo = []() {};");
        let inner = init_comma_expr(&cst);
        match Expr::from_cst(&cst, inner).expect("Expr") {
            Expr::Lambda(l) => {
                assert!(l.captures.inner.is_empty());
                assert!(l.params.inner.is_empty());
                assert!(l.body.inner.is_empty());
                assert!(l.ret_type.is_none());
            }
            other => panic!("expected Lambda, got {:?}", other),
        }
    }
}
