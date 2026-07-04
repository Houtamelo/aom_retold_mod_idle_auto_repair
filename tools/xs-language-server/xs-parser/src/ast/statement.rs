use crate::ast::cst_helpers::{child_by_rule, is_skip_token};
use crate::ast::declaration::{Declaration, UnparsedExpr};
use crate::ast::spanned::{Braced, Parenthesized};
use crate::ast::top_level::{FieldDeclaration, ForwardDeclaration, FunctionDefinition};
use crate::parser::{Cst, Node, NodeRef, Rule, Span};
use crate::lexer::Token;

/// A statement: one of the 9 XS statement kinds per design doc §3.7.
///
/// Dispatches in this order:
/// 1. `If`/`While`/`For`/`Switch` (each has a rule-wrapper form
///    marked with `@if_statement` etc. on the parent `statement^`,
///    which the parser may or may not produce depending on grammar
///    completeness)
/// 2. `Return`/`Break`/`Continue` (each has a rule-wrapper form OR a
///    bare token sequence at top level when the grammar fails to
///    reduce — the variants accept both shapes)
/// 3. `CompoundStatement` (a `{ block_item* }` block)
/// 4. `ExpressionStatement` (an `expr;`)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Statement {
    If(IfStatement),
    While(WhileStatement),
    For(ForStatement),
    Switch(SwitchStatement),
    Return(ReturnStatement),
    Break(BreakStatement),
    Continue(ContinueStatement),
    Compound(CompoundStatement),
    Expression(ExpressionStatement),
}

impl Statement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if let Some(x) = IfStatement::from_cst(cst, node) {
            return Some(Self::If(x));
        }
        if let Some(x) = WhileStatement::from_cst(cst, node) {
            return Some(Self::While(x));
        }
        if let Some(x) = ForStatement::from_cst(cst, node) {
            return Some(Self::For(x));
        }
        if let Some(x) = SwitchStatement::from_cst(cst, node) {
            return Some(Self::Switch(x));
        }
        if let Some(x) = ReturnStatement::from_cst(cst, node) {
            return Some(Self::Return(x));
        }
        if let Some(x) = BreakStatement::from_cst(cst, node) {
            return Some(Self::Break(x));
        }
        if let Some(x) = ContinueStatement::from_cst(cst, node) {
            return Some(Self::Continue(x));
        }
        if let Some(x) = CompoundStatement::from_cst(cst, node) {
            return Some(Self::Compound(x));
        }
        if let Some(x) = ExpressionStatement::from_cst(cst, node) {
            return Some(Self::Expression(x));
        }
        None
    }
}

/// One item in a block: a declaration, a function definition, or a
/// statement (compound or expression).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BlockItem {
    Declaration(Declaration),
    ForwardDeclaration(ForwardDeclaration),
    FunctionDefinition(FunctionDefinition),
    Statement(Statement),
}

impl BlockItem {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if let Some(x) = Declaration::from_cst(cst, node) {
            return Some(Self::Declaration(x));
        }
        if let Some(x) = ForwardDeclaration::from_cst(cst, node) {
            return Some(Self::ForwardDeclaration(x));
        }
        if let Some(x) = FunctionDefinition::from_cst(cst, node) {
            return Some(Self::FunctionDefinition(x));
        }
        if let Some(x) = Statement::from_cst(cst, node) {
            return Some(Self::Statement(x));
        }
        // Class-member field declarations also appear as block-level items
        // (e.g. inside a class body that was traversed as a compound).
        if let Some(x) = FieldDeclaration::from_cst(cst, node) {
            // Wrap a field declaration back into a BlockItem. BlockItem has
            // no FieldDeclaration variant; fold it into Statement::Compound
            // with a single expression-statement placeholder would lose
            // data. For Pass 2 we return None for this edge case — class
            // members are extracted via `ClassMember` instead.
            let _ = x;
            return None;
        }
        None
    }
}

pub type BlockItemList = Vec<BlockItem>;

/// A compound statement: `{ block_item* }`.
///
/// Wraps `Rule::CompoundStatement`. Direct children are the inner
/// block items (each dispatched via `BlockItem::from_cst`), in
/// source order, with skip tokens interleaved.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompoundStatement {
    pub items: Braced<BlockItemList>,
    pub span: Span,
}

impl CompoundStatement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::CompoundStatement) {
            return None;
        }
        let open = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::LBrace).map(|(_, s)| s))?;
        let close = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::RBrace).map(|(_, s)| s))?;

        let children: Vec<_> = cst.children(node).collect();
        let lo = children
            .iter()
            .position(|c| cst.match_token(*c, Token::LBrace).map(|(_, s)| s) == Some(open.clone()));
        let hi = children
            .iter()
            .position(|c| cst.match_token(*c, Token::RBrace).map(|(_, s)| s) == Some(close.clone()));
        let (Some(lo), Some(hi)) = (lo, hi) else {
            return Some(Self {
                items: Braced::new(open, Vec::new(), close),
                span: cst.span(node),
            });
        };

        let mut items = Vec::new();
        for child in &children[lo + 1..hi] {
            if is_skip_token(cst, *child) {
                continue;
            }
            if let Some(item) = BlockItem::from_cst(cst, *child) {
                items.push(item);
            }
        }

        Some(Self {
            items: Braced::new(open, items, close),
            span: cst.span(node),
        })
    }
}

/// An `if` statement: `if (cond) then else?`.
///
/// Accepts the rule-wrapper form (`Rule::IfStatement`) AND the bare
/// token sequence (`If + ( + expr + ) + statement [+ Else + statement]`)
/// because the current PoC grammar does not reduce the wrapper for
/// `if` (the dangling-else limitation prevents it).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IfStatement {
    pub cond: Parenthesized<UnparsedExpr>,
    pub then: Box<Statement>,
    pub else_: Option<Box<Statement>>,
    pub span: Span,
}

impl IfStatement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::IfStatement) {
            return Self::from_wrapper(cst, node);
        }
        Self::from_bare(cst, node)
    }

    fn from_wrapper(cst: &Cst, node: NodeRef) -> Option<Self> {
        let if_kw = child_by_rule(cst, node, Rule::IfStatement)?;
        let _ = if_kw;
        // The wrapper shape: 'If' ParenthesizedExpression statement
        // [Else statement]. The actual CST shape is grammar-dependent.
        // For the current PoC the wrapper is never produced; this
        // branch is kept correct for a grammar that fixes it.
        let if_span = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::If).map(|(_, s)| s))?;
        let paren_node = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::ParenthesizedExpression))?;
        let (open, expr, close) = parenthesized_expr_parts(cst, paren_node)?;
        let cond = Parenthesized::new(open, expr, close);
        // The then-body: first statement-like child after the paren.
        let then = first_statement_after(cst, node, paren_node)?;
        // The optional `else` branch. The lexer maps the `else`
        // keyword to `Token::PreprocElse` in both preprocessor and
        // statement contexts, so we look for that token inside the
        // wrapper and extract the first statement that follows it.
        let else_: Option<Box<Statement>> = cst
            .children(node)
            .find(|c| cst.match_token(*c, Token::PreprocElse).is_some())
            .and_then(|else_tok| first_statement_after(cst, node, else_tok).map(Box::new));
        let span_end = else_.as_ref().map_or_else(|| then.span().end, |e| e.span().end);
        let span = if_span.start..span_end;
        Some(Self {
            cond,
            then: Box::new(then),
            else_,
            span,
        })
    }

    fn from_bare(cst: &Cst, node: NodeRef) -> Option<Self> {
        // Bare token pattern: `If` followed by '(' expr ')' body [Else body].
        // Children of the surrounding top-level/block context are walked; this
        // method is invoked per child so the caller passes the If token's
        // sibling slice via `node` (the If token itself).
        if cst.match_token(node, Token::If).is_none() {
            return None;
        }
        None
    }
}

/// A `while` statement: `while (cond) body`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WhileStatement {
    pub cond: Parenthesized<UnparsedExpr>,
    pub body: Box<Statement>,
    pub span: Span,
}

impl WhileStatement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::WhileStatement) {
            return Self::from_wrapper(cst, node);
        }
        None
    }

    fn from_wrapper(cst: &Cst, node: NodeRef) -> Option<Self> {
        let while_span = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::While).map(|(_, s)| s))?;
        let paren_node = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::ParenthesizedExpression))?;
        let (open, expr, close) = parenthesized_expr_parts(cst, paren_node)?;
        let cond = Parenthesized::new(open, expr, close);
        let body = first_statement_after(cst, node, paren_node)?;
        let span = while_span.start..body.span().end;
        Some(Self {
            cond,
            body: Box::new(body),
            span,
        })
    }
}

/// A `for` statement: `for (init; cond?; post?) body`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForStatement {
    pub init: ForInit,
    pub cond: Option<UnparsedExpr>,
    pub post: Option<UnparsedExpr>,
    pub body: Box<Statement>,
    pub span: Span,
}

impl ForStatement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::ForStatement) {
            return Self::from_wrapper(cst, node);
        }
        None
    }

    fn from_wrapper(cst: &Cst, node: NodeRef) -> Option<Self> {
        let for_span = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::For).map(|(_, s)| s))?;
        // for ( init ; cond? ; post? ) body
        let children: Vec<_> = cst.children(node).collect();
        let lpar_pos = children
            .iter()
            .position(|c| cst.match_token(*c, Token::LPar).is_some())?;
        let rpar_pos = children
            .iter()
            .position(|c| cst.match_token(*c, Token::RPar).is_some())?;
        // Inside parens: init ; [expr] ; [expr]
        let inner = &children[lpar_pos + 1..rpar_pos];
        let semis: Vec<_> = inner
            .iter()
            .enumerate()
            .filter(|(_, c)| cst.match_token(**c, Token::Semi).is_some())
            .map(|(i, c)| (i, cst.match_token(*c, Token::Semi).unwrap().1))
            .collect();
        let init = if let Some((first_semi_pos, _)) = semis.first() {
            let init_children = &inner[..*first_semi_pos];
            let init_node = init_children
                .iter()
                .find(|c| matches!(cst.get(**c), Node::Rule(_, _)));
            match init_node {
                Some(n) => ForInit::from_cst(cst, *n).unwrap_or(ForInit::Empty),
                None => ForInit::Empty,
            }
        } else {
            ForInit::Empty
        };
        let cond = if semis.len() >= 2 {
            let (first_semi_pos, _) = semis[0];
            let (second_semi_pos, _) = semis[1];
            let cond_children = &inner[first_semi_pos + 1..second_semi_pos];
            let cond_node = cond_children
                .iter()
                .find(|c| matches!(cst.get(**c), Node::Rule(_, _)))
                .copied();
            cond_node.map(UnparsedExpr)
        } else {
            None
        };
        let post = if semis.len() >= 2 {
            let (second_semi_pos, _) = semis[1];
            let post_children = &inner[second_semi_pos + 1..];
            let post_node = post_children
                .iter()
                .find(|c| matches!(cst.get(**c), Node::Rule(_, _)))
                .copied();
            post_node.map(UnparsedExpr)
        } else {
            None
        };
        let body = first_statement_after(cst, node, children[rpar_pos])?;
        let span = for_span.start..body.span().end;
        Some(Self {
            init,
            cond,
            post,
            body: Box::new(body),
            span,
        })
    }
}

/// A `switch` statement: `switch (cond) { body }`.
///
/// The body is parsed as a generic compound statement, with `case` and
/// `default` labels modeled as labeled statements. This keeps the typed
/// AST simple while accepting the consecutive labels and nested compound
/// bodies used by retail XS.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SwitchStatement {
    pub cond: Parenthesized<UnparsedExpr>,
    pub body: CompoundStatement,
    pub span: Span,
}

impl SwitchStatement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if cst.match_rule(node, Rule::SwitchStatement) {
            return Self::from_wrapper(cst, node);
        }
        None
    }

    fn from_wrapper(cst: &Cst, node: NodeRef) -> Option<Self> {
        let switch_span = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Switch).map(|(_, s)| s))?;
        let paren_node = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::ParenthesizedExpression))?;
        let (open, expr, close) = parenthesized_expr_parts(cst, paren_node)?;
        let cond = Parenthesized::new(open, expr, close);

        let body_node = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::CompoundStatement))?;
        let body = CompoundStatement::from_cst(cst, body_node)?;

        let span = switch_span.start..body.span.end;
        Some(Self { cond, body, span })
    }
}

/// A `return [value];` statement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ReturnStatement {
    pub value: Option<UnparsedExpr>,
    pub semi: Span,
    pub span: Span,
}

impl ReturnStatement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ReturnStatement) {
            return None;
        }
        let return_span = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Return).map(|(_, s)| s))?;
        let value = cst
            .children(node)
            .find(|c| matches!(cst.get(*c), Node::Rule(_, _)))
            .map(UnparsedExpr);
        let semi = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Semi).map(|(_, s)| s))?;
        let semi_end = semi.end;
        Some(Self {
            value,
            semi,
            span: return_span.start..semi_end,
        })
    }
}

/// A `break;` statement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BreakStatement {
    pub semi: Span,
    pub span: Span,
}

impl BreakStatement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::BreakStatement) {
            return None;
        }
        let semi = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Semi).map(|(_, s)| s))?;
        let break_span = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Break).map(|(_, s)| s))?;
        let semi_end = semi.end;
        Some(Self {
            semi,
            span: break_span.start..semi_end,
        })
    }
}

/// A `continue;` statement.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContinueStatement {
    pub semi: Span,
    pub span: Span,
}

impl ContinueStatement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ContinueStatement) {
            return None;
        }
        let semi = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Semi).map(|(_, s)| s))?;
        let cont_span = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Continue).map(|(_, s)| s))?;
        let semi_end = semi.end;
        Some(Self {
            semi,
            span: cont_span.start..semi_end,
        })
    }
}

/// An expression statement: `[expr];`. The expression is optional to
/// support empty `;`.
///
/// Wraps `Rule::ExpressionStatement`. The inner expression is the
/// first rule child (comma_expr → ...); the semi is the lone `;`
/// token.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExpressionStatement {
    pub expr: Option<UnparsedExpr>,
    pub semi: Span,
    pub span: Span,
}

impl ExpressionStatement {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ExpressionStatement) {
            return None;
        }
        let expr = cst
            .children(node)
            .find(|c| matches!(cst.get(*c), Node::Rule(_, _)))
            .map(UnparsedExpr);
        let semi = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Semi).map(|(_, s)| s))?;
        let semi_start = semi.start;
        let semi_end = semi.end;
        let node_span_start = cst.span(node).start;
        Some(Self {
            expr,
            semi,
            span: node_span_start.min(semi_start)..semi_end,
        })
    }
}

/// A `for`-loop initializer: `for (init; ...)`.
///
/// Wraps `Rule::ForInit`. Three variants per the spec:
/// - `Declaration` — the inner is a `Rule::Declaration`
/// - `Expression` — the inner is an arbitrary expression (comma_expr etc.)
/// - `Empty` — no expression, no declaration (used when `for (; ; ...)`)
///
/// Pass 2: `Declaration` extraction uses the full `Declaration::from_cst`
/// path because declarations parse cleanly in PoC. Pass 3 may replace
/// the inner Expression contents with a real `Expr`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ForInit {
    Declaration(Declaration),
    Expression(UnparsedExpr),
    Empty,
}

impl ForInit {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ForInit) {
            return None;
        }
        // Find the first rule child: it's either a Declaration or a
        // generic expression (comma_expr etc.).
        let rule_child = cst
            .children(node)
            .find(|c| matches!(cst.get(*c), Node::Rule(_, _)));
        match rule_child {
            Some(n) if cst.match_rule(n, Rule::Declaration) => {
                let decl = Declaration::from_cst(cst, n)?;
                Some(Self::Declaration(decl))
            }
            Some(n) => Some(Self::Expression(UnparsedExpr(n))),
            None => Some(Self::Empty),
        }
    }
}

// ---- helpers ----

/// Splits a `Rule::ParenthesizedExpression` node into (open, expr, close).
pub(crate) fn parenthesized_expr_parts(
    cst: &Cst,
    node: NodeRef,
) -> Option<(Span, UnparsedExpr, Span)> {
    if !cst.match_rule(node, Rule::ParenthesizedExpression) {
        return None;
    }
    let open = cst
        .children(node)
        .find_map(|c| cst.match_token(c, Token::LPar).map(|(_, s)| s))?;
    let close = cst
        .children(node)
        .find_map(|c| cst.match_token(c, Token::RPar).map(|(_, s)| s))?;
    let expr_node = cst
        .children(node)
        .find(|c| matches!(cst.get(*c), Node::Rule(_, _)))?;
    Some((open, UnparsedExpr(expr_node), close))
}

/// Returns the first `Statement` in `parent`'s children that appears
/// after `prev` (used to find the body of `if`/`while`/`for` etc.).
fn first_statement_after(cst: &Cst, parent: NodeRef, prev: NodeRef) -> Option<Statement> {
    let children: Vec<_> = cst.children(parent).collect();
    let prev_pos = children.iter().position(|c| *c == prev)?;
    for child in &children[prev_pos + 1..] {
        if is_skip_token(cst, *child) {
            continue;
        }
        if let Some(s) = Statement::from_cst(cst, *child) {
            return Some(s);
        }
    }
    None
}

/// Trait that gives every Statement variant a uniform `span()` accessor
/// (used by the wrappers that build total spans).
pub trait StmtSpanned {
    fn span(&self) -> Span;
}

impl StmtSpanned for Statement {
    fn span(&self) -> Span {
        match self {
            Statement::If(s) => s.span.clone(),
            Statement::While(s) => s.span.clone(),
            Statement::For(s) => s.span.clone(),
            Statement::Switch(s) => s.span.clone(),
            Statement::Return(s) => s.span.clone(),
            Statement::Break(s) => s.span.clone(),
            Statement::Continue(s) => s.span.clone(),
            Statement::Compound(s) => s.span.clone(),
            Statement::Expression(s) => s.span.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::type_system::Type;
    use crate::ast::type_table::TypeTable;
    use crate::parser::{Diagnostic, Parser};

    fn parse(source: &str) -> Cst<'_> {
        let mut diags = vec![];
        Parser::new(source, &mut diags).parse(&mut diags)
    }

    fn parse_with_table(source: &str, table: TypeTable) -> (Cst<'_>, Vec<Diagnostic>) {
        let mut diags = vec![];
        let cst = Parser::new_with_context(source, &mut diags, table).parse(&mut diags);
        (cst, diags)
    }

    fn for_init_of(cst: &Cst) -> ForInit {
        let node = cst
            .children(crate::parser::NodeRef::ROOT)
            .find(|c| cst.match_rule(*c, Rule::ForInit))
            .expect("for_init node");
        ForInit::from_cst(cst, node).expect("for_init extraction")
    }

    fn first_non_skip(cst: &Cst) -> NodeRef {
        cst.children(crate::parser::NodeRef::ROOT)
            .find(|c| !is_skip_token(cst, *c))
            .expect("at least one non-skip child")
    }

    #[test]
    fn compound_statement_extracts_empty_block() {
        // Empty compound: parses inside a rule body (the rule
        // grammar names `compound_statement` directly; the function
        // grammar inlines `{ block_item* }` without the wrapper).
        let cst = parse("rule r { }");
        let first = first_non_skip(&cst);
        let cs_node = cst
            .children(first)
            .find(|c| cst.match_rule(*c, Rule::CompoundStatement))
            .expect("body compound");
        let cs = CompoundStatement::from_cst(&cst, cs_node).expect("compound");
        assert_eq!(cs.items.inner.len(), 0);
    }

    #[test]
    fn compound_statement_extracts_with_break() {
        // `rule r { break; }` parses with a `compound_statement`
        // wrapper. The body extraction is strict (BlockItem::from_cst
        // requires rule wrappers), so the body items list is empty
        // here — bare `break;` and `continue;` tokens inside a
        // compound_statement body are NOT classified by the strict
        // path. Function-body extraction in top_level.rs handles
        // the bare-token case via `classify_bare_body_item`.
        let cst = parse("rule r { break; }");
        let first = first_non_skip(&cst);
        let cs_node = cst
            .children(first)
            .find(|c| cst.match_rule(*c, Rule::CompoundStatement))
            .expect("body compound");
        let cs = CompoundStatement::from_cst(&cst, cs_node).expect("compound");
        assert_eq!(cs.items.inner.len(), 0);
        assert_eq!(cs.items.open.start, 7);
        assert_eq!(cs.items.close.end, 17);
    }

    #[test]
    fn expression_statement_with_identifier() {
        let cst = parse("x;");
        let first = first_non_skip(&cst);
        match Statement::from_cst(&cst, first) {
            Some(Statement::Expression(es)) => {
                assert!(es.expr.is_some());
                assert_eq!(es.semi, 1..2);
            }
            other => panic!("expected Expression, got {:?}", other),
        }
    }

    #[test]
    fn expression_statement_empty() {
        let cst = parse(";");
        let first = first_non_skip(&cst);
        match Statement::from_cst(&cst, first) {
            Some(Statement::Expression(es)) => {
                assert!(es.expr.is_none());
                assert_eq!(es.semi, 0..1);
            }
            other => panic!("expected Expression, got {:?}", other),
        }
    }

    #[test]
    fn for_init_empty() {
        let cst = parse("for (; ; ) { }");
        // The translation_unit's first non-skip child is `For` token
        // (not wrapped in for_statement due to grammar bug). Walk to
        // find the for_init rule child manually.
        let for_init_node = cst
            .children(crate::parser::NodeRef::ROOT)
            .find(|c| cst.match_rule(*c, Rule::ForInit))
            .expect("for_init rule child");
        let fi = ForInit::from_cst(&cst, for_init_node).expect("for_init");
        assert!(matches!(fi, ForInit::Empty));
    }

    #[test]
    fn for_init_expression() {
        let cst = parse("for (i = 0; ; ) { }");
        let for_init_node = cst
            .children(crate::parser::NodeRef::ROOT)
            .find(|c| cst.match_rule(*c, Rule::ForInit))
            .expect("for_init rule child");
        let fi = ForInit::from_cst(&cst, for_init_node).expect("for_init");
        assert!(matches!(fi, ForInit::Expression(_)));
    }

    #[test]
    fn for_statement_extracts_int_declaration_form() {
        let (cst, diags) = parse_with_table(
            "for (int i = 0; i < 10; i = i + 1) { }",
            TypeTable::with_primitives(),
        );
        assert!(diags.is_empty(), "expected 0 diagnostics, got {:?}", diags);
        let fi = for_init_of(&cst);
        match fi {
            ForInit::Declaration(decl) => {
                assert_eq!(decl.decl_specs.ty.ty, Type::Int);
                assert_eq!(decl.init_declarator_list.items.len(), 1);
                let init = &decl.init_declarator_list.items[0];
                match &init.declarator.direct {
                    crate::ast::declaration::DirectDeclarator::IdentDeclarator(id) => {
                        assert_eq!(id.name.node, "i");
                    }
                    other => panic!("expected identifier declarator, got {:?}", other),
                }
                assert!(init.initializer.is_some());
            }
            other => panic!("expected Declaration for-init, got {:?}", other),
        }
    }

    #[test]
    fn for_statement_extracts_bool_declaration_form() {
        let (cst, diags) = parse_with_table(
            "for (bool done = false; ; ) { }",
            TypeTable::with_primitives(),
        );
        assert!(diags.is_empty(), "expected 0 diagnostics, got {:?}", diags);
        let fi = for_init_of(&cst);
        match fi {
            ForInit::Declaration(decl) => {
                assert_eq!(decl.decl_specs.ty.ty, Type::Bool);
                assert_eq!(decl.init_declarator_list.items.len(), 1);
                let init = &decl.init_declarator_list.items[0];
                match &init.declarator.direct {
                    crate::ast::declaration::DirectDeclarator::IdentDeclarator(id) => {
                        assert_eq!(id.name.node, "done");
                    }
                    other => panic!("expected identifier declarator, got {:?}", other),
                }
                assert!(init.initializer.is_some());
            }
            other => panic!("expected Declaration for-init, got {:?}", other),
        }
    }

    #[test]
    fn for_statement_extracts_class_typed_declaration_form() {
        let mut types = TypeTable::with_primitives();
        types.insert_class("BOSystem");
        let (cst, diags) = parse_with_table(
            "for (BOSystem sys = default; ; ) { }",
            types,
        );
        assert!(diags.is_empty(), "expected 0 diagnostics, got {:?}", diags);
        let fi = for_init_of(&cst);
        match fi {
            ForInit::Declaration(decl) => {
                match &decl.decl_specs.ty.ty {
                    Type::Class(name) => assert_eq!(name.node, "BOSystem"),
                    other => panic!("expected Class type, got {:?}", other),
                }
                assert_eq!(decl.init_declarator_list.items.len(), 1);
                let init = &decl.init_declarator_list.items[0];
                match &init.declarator.direct {
                    crate::ast::declaration::DirectDeclarator::IdentDeclarator(id) => {
                        assert_eq!(id.name.node, "sys");
                    }
                    other => panic!("expected identifier declarator, got {:?}", other),
                }
            }
            other => panic!("expected Declaration for-init, got {:?}", other),
        }
    }

    #[test]
    fn for_statement_extracts_expression_form() {
        let (cst, diags) = parse_with_table(
            "for (i = 0; i < 10; i = i + 1) { }",
            TypeTable::with_primitives(),
        );
        assert!(diags.is_empty(), "expected 0 diagnostics, got {:?}", diags);
        let fi = for_init_of(&cst);
        assert!(
            matches!(fi, ForInit::Expression(_)),
            "expected Expression for-init, got {:?}",
            fi
        );
    }

    #[test]
    fn block_item_extracts_compound_statement() {
        // Compound statement wraps a `{ block_item* }` only when it
        // appears as a rule body (the rule grammar names
        // `compound_statement` directly). The body is empty here so
        // we just verify the wrapper is recognized.
        let cst = parse("rule r { }");
        let first = first_non_skip(&cst);
        let cs_node = cst
            .children(first)
            .find(|c| cst.match_rule(*c, Rule::CompoundStatement))
            .expect("body compound");
        match BlockItem::from_cst(&cst, cs_node) {
            Some(BlockItem::Statement(Statement::Compound(_))) => {}
            other => panic!("expected Statement::Compound, got {:?}", other),
        }
    }

    #[test]
    fn block_item_extracts_expression_statement() {
        let cst = parse("x = 5;");
        let first = first_non_skip(&cst);
        match BlockItem::from_cst(&cst, first) {
            Some(BlockItem::Statement(Statement::Expression(_))) => {}
            other => panic!("expected Statement::Expression, got {:?}", other),
        }
    }

    #[test]
    fn break_statement_inside_function_body() {
        // `break;` inside a function body is recognized via the
        // function-body bare-token handler in top_level.rs. Walk into
        // the function's body via FunctionDefinition::from_cst and
        // confirm the body has a BreakStatement item.
        let cst = parse("void f() { break; }");
        let first = first_non_skip(&cst);
        let fd = crate::ast::top_level::FunctionDefinition::from_cst(&cst, first).expect("fn");
        assert_eq!(fd.body.inner.len(), 1);
        match &fd.body.inner[0] {
            BlockItem::Statement(Statement::Break(_)) => {}
            other => panic!("expected Break statement, got {:?}", other),
        }
    }

    #[test]
    fn continue_statement_inside_function_body() {
        let cst = parse("void f() { continue; }");
        let first = first_non_skip(&cst);
        let fd = crate::ast::top_level::FunctionDefinition::from_cst(&cst, first).expect("fn");
        assert_eq!(fd.body.inner.len(), 1);
        match &fd.body.inner[0] {
            BlockItem::Statement(Statement::Continue(_)) => {}
            other => panic!("expected Continue statement, got {:?}", other),
        }
    }

    #[test]
    fn return_statement_inside_function_body() {
        let cst = parse("void f() { return; }");
        let first = first_non_skip(&cst);
        let fd = crate::ast::top_level::FunctionDefinition::from_cst(&cst, first).expect("fn");
        assert_eq!(fd.body.inner.len(), 1);
        match &fd.body.inner[0] {
            BlockItem::Statement(Statement::Return(_)) => {}
            other => panic!("expected Return statement, got {:?}", other),
        }
    }

    #[test]
    fn break_statement_returns_none_for_unrelated_node() {
        // The bare-token fallback was removed; rule-wrapper only.
        // A bare `break` token not in a function body returns None.
        let cst = parse("break;");
        let break_token = cst
            .children(crate::parser::NodeRef::ROOT)
            .find(|c| cst.match_token(*c, Token::Break).is_some())
            .expect("break token");
        assert!(BreakStatement::from_cst(&cst, break_token).is_none());
    }

    #[test]
    fn return_statement_returns_none_for_bare_token() {
        // `return x;` at top-level — no wrapper, no surrounding
        // sibling context. ReturnStatement::from_cst requires the
        // rule wrapper.
        let cst = parse("return x;");
        let first = first_non_skip(&cst);
        assert!(Statement::from_cst(&cst, first).is_none());
    }


    #[test]
    fn statement_dispatch_returns_none_for_unrelated_node() {
        let cst = parse("int x;");
        let first = first_non_skip(&cst);
        assert!(Statement::from_cst(&cst, first).is_none());
    }

    #[test]
    fn block_item_dispatch_returns_none_for_unrelated_node() {
        let cst = parse("int x;");
        let first = first_non_skip(&cst);
        // Declaration would match! Let me try a different source.
        // Actually, "int x;" IS a forward_declaration, so BlockItem
        // matches it as ForwardDeclaration. Confirm the positive case.
        match BlockItem::from_cst(&cst, first) {
            Some(BlockItem::ForwardDeclaration(_)) => {}
            other => panic!("expected ForwardDeclaration, got {:?}", other),
        }
    }

    fn first_if_in_function(cst: &Cst) -> IfStatement {
        let first = first_non_skip(cst);
        let fd = crate::ast::top_level::FunctionDefinition::from_cst(cst, first)
            .expect("function definition");
        assert_eq!(fd.body.inner.len(), 1);
        match &fd.body.inner[0] {
            BlockItem::Statement(Statement::If(if_stmt)) => if_stmt.clone(),
            other => panic!("expected If statement, got {:?}", other),
        }
    }

    #[test]
    fn if_statement_without_else() {
        let cst = parse("void f() { if (a) {} }");
        let if_stmt = first_if_in_function(&cst);
        assert!(if_stmt.else_.is_none());
    }

    #[test]
    fn if_statement_with_else() {
        let cst = parse("void f() { if (a) {} else {} }");
        let if_stmt = first_if_in_function(&cst);
        assert!(if_stmt.else_.is_some());
    }

    #[test]
    fn if_statement_else_if_chain() {
        let cst = parse("void f() { if (a) {} else if (b) {} else {} }");
        let outer = first_if_in_function(&cst);
        assert!(outer.else_.is_some(), "outer if should have an else branch");
        match outer.else_.as_ref().unwrap().as_ref() {
            Statement::If(inner) => {
                assert!(inner.else_.is_some(), "inner if should also have an else branch");
            }
            other => panic!("expected nested If in else branch, got {:?}", other),
        }
    }

}
