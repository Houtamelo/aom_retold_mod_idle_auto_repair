use crate::ast::cst_helpers::{child_by_rule, is_skip_token};
use crate::ast::declaration::{Declaration, Declarator, UnparsedExpr};
use crate::ast::expr::Expr;
use crate::ast::preproc::{PreprocDef, PreprocElif, PreprocElse, PreprocEndif, PreprocIf};
use crate::ast::spanned::{Braced, Spanned};
use crate::ast::statement::{
    BlockItem, BlockItemList, BreakStatement, CompoundStatement, ContinueStatement,
    ExpressionStatement, ReturnStatement,
};
use crate::ast::type_system::{DeclarationSpecifiers, Identifier};
use crate::parser::{Cst, Node, NodeRef, Rule, Span};
use crate::lexer::Token;

/// The root of an XS source file: a sequence of top-level items.
///
/// Wraps `Rule::TranslationUnit` (the grammar's `translation_unit`
/// rule). Direct children are `top_level_item*` — each item dispatches
/// into a `TopLevelItem` variant. Skip tokens are silently skipped.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TranslationUnit {
    pub items: Vec<TopLevelItem>,
    pub span: Span,
}

impl TranslationUnit {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::TranslationUnit) {
            return None;
        }
        let mut items = Vec::new();
        for child in cst.children(node) {
            if is_skip_token(cst, child) {
                continue;
            }
            if let Some(item) = TopLevelItem::from_cst(cst, child) {
                items.push(item);
            }
        }
        Some(Self {
            items,
            span: cst.span(node),
        })
    }
}

/// One top-level item.
///
/// Dispatches in priority order: preprocessor first (most distinctive
/// starting token `#`), then include/class/rule/function/forward/
/// declaration. `None` is returned for any node that doesn't match a
/// recognized shape — including `ERROR` subtrees and bare tokens that
/// the parser couldn't reduce.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TopLevelItem {
    PreprocIf(PreprocIf),
    PreprocElif(PreprocElif),
    PreprocElse(PreprocElse),
    PreprocEndif(PreprocEndif),
    PreprocDef(PreprocDef),
    IncludeDirective(IncludeDirective),
    FunctionDefinition(FunctionDefinition),
    ForwardDeclaration(ForwardDeclaration),
    ClassDefinition(ClassDefinition),
    RuleDefinition(RuleDefinition),
    Declaration(Declaration),
}

impl TopLevelItem {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if let Some(x) = PreprocIf::from_cst(cst, node) {
            return Some(Self::PreprocIf(x));
        }
        if let Some(x) = PreprocElif::from_cst(cst, node) {
            return Some(Self::PreprocElif(x));
        }
        if let Some(x) = PreprocElse::from_cst(cst, node) {
            return Some(Self::PreprocElse(x));
        }
        if let Some(x) = PreprocEndif::from_cst(cst, node) {
            return Some(Self::PreprocEndif(x));
        }
        if let Some(x) = PreprocDef::from_cst(cst, node) {
            return Some(Self::PreprocDef(x));
        }
        if let Some(x) = IncludeDirective::from_cst(cst, node) {
            return Some(Self::IncludeDirective(x));
        }
        if let Some(x) = FunctionDefinition::from_cst(cst, node) {
            return Some(Self::FunctionDefinition(x));
        }
        if let Some(x) = ForwardDeclaration::from_cst(cst, node) {
            return Some(Self::ForwardDeclaration(x));
        }
        if let Some(x) = ClassDefinition::from_cst(cst, node) {
            return Some(Self::ClassDefinition(x));
        }
        if let Some(x) = RuleDefinition::from_cst(cst, node) {
            return Some(Self::RuleDefinition(x));
        }
        if let Some(x) = Declaration::from_cst(cst, node) {
            return Some(Self::Declaration(x));
        }
        None
    }
}

/// A class specifier: `class Name { member* };`.
///
/// Members are extracted INLINE because the grammar's `class_member^`
/// collapses the wrapper — direct children of `class_specifier` are
/// `field_declaration`, `forward_declaration`, or `function_definition`
/// rule nodes (per Pass 1 discovery). The trailing `;` is required by
/// the grammar; the parser always emits 1 diagnostic for class
/// specifiers (a known PoC limitation that does not block extraction).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ClassDefinition {
    pub name: Identifier,
    pub members: Braced<Vec<ClassMember>>,
    pub semi: Span,
    pub span: Span,
}

impl ClassDefinition {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ClassSpecifier) {
            return None;
        }

        let name_node = cst
            .children(node)
            .find(|c| cst.match_token(*c, Token::Identifier).is_some())?;
        let (text, name_span) = cst.match_token(name_node, Token::Identifier)?;
        let name = Identifier::new(text.to_string(), name_span);

        let open = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::LBrace).map(|(_, s)| s))?;
        let close = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::RBrace).map(|(_, s)| s))?;
        let semi = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Semi).map(|(_, s)| s));
        let close_end = close.end;

        let members = extract_class_members(cst, node);

        Some(Self {
            name,
            members: Braced::new(open, members, close),
            semi: semi.unwrap_or_else(|| close_end..close_end),
            span: cst.span(node),
        })
    }
}

/// Walks `class_specifier`'s children between LBrace and RBrace,
/// dispatching each into a `ClassMember` variant.
fn extract_class_members(cst: &Cst, class_node: NodeRef) -> Vec<ClassMember> {
    let children: Vec<_> = cst.children(class_node).collect();
    let lbrace_pos = children.iter().position(|c| cst.match_token(*c, Token::LBrace).is_some());
    let rbrace_pos = children.iter().position(|c| cst.match_token(*c, Token::RBrace).is_some());
    let (Some(lo), Some(hi)) = (lbrace_pos, rbrace_pos) else {
        return Vec::new();
    };

    children[lo + 1..hi]
        .iter()
        .filter(|c| !is_skip_token(cst, **c))
        .filter_map(|c| ClassMember::from_cst(cst, *c))
        .collect()
}

/// One class member: a field, a method, or a method forward declaration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ClassMember {
    FunctionDefinition(FunctionDefinition),
    ForwardDeclaration(ForwardDeclaration),
    FieldDeclaration(FieldDeclaration),
}

impl ClassMember {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if let Some(x) = FunctionDefinition::from_cst(cst, node) {
            return Some(Self::FunctionDefinition(x));
        }
        if let Some(x) = ForwardDeclaration::from_cst(cst, node) {
            return Some(Self::ForwardDeclaration(x));
        }
        if let Some(x) = FieldDeclaration::from_cst(cst, node) {
            return Some(Self::FieldDeclaration(x));
        }
        None
    }
}

/// A class field declaration: `int x = 5;` (initializer optional in
/// class bodies, unlike top-level declarations which require `=`).
///
/// Wraps `Rule::FieldDeclaration`. The declarator content is inline
/// (Identifier or Identifier(params)); there is no `Declarator` wrapper
/// node in the actual CST.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldDeclaration {
    pub decl_specs: DeclarationSpecifiers,
    pub declarator: Declarator,
    pub initializer: Option<(Span, UnparsedExpr)>,
    pub semi: Span,
    pub span: Span,
}

impl FieldDeclaration {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::FieldDeclaration) {
            return None;
        }
        let specs_node = child_by_rule(cst, node, Rule::DeclarationSpecifiers)?;
        let decl_specs = DeclarationSpecifiers::from_cst(cst, specs_node)?;

        let declarator = Declarator::from_inline(cst, node)?;

        let mut initializer: Option<(Span, UnparsedExpr)> = None;
        let mut semi: Option<Span> = None;
        let mut saw_assign = false;
        for child in cst.children(node) {
            if let Some((_, span)) = cst.match_token(child, Token::Assign) {
                if !saw_assign {
                    let expr_node = cst
                        .children(node)
                        .find(|c| {
                            matches!(cst.get(*c), Node::Rule(_, _))
                                && !cst.match_rule(*c, Rule::DeclarationSpecifiers)
                        })?;
                    initializer = Some((span, UnparsedExpr(expr_node)));
                    saw_assign = true;
                }
            }
            if let Some((_, span)) = cst.match_token(child, Token::Semi) {
                semi = Some(span);
            }
        }

        Some(Self {
            decl_specs,
            declarator,
            initializer,
            semi: semi?,
            span: cst.span(node),
        })
    }

    /// Re-extract the initializer as a typed `Expr` (T12 helper).
    pub fn expr(&self, cst: &Cst) -> Option<Expr> {
        let (_, unparsed) = self.initializer.as_ref()?;
        Expr::from_cst(cst, unparsed.0)
    }
}

/// A function or method definition: `void f() { body }`.
///
/// Wraps `Rule::FunctionDefinition` (the `@function_definition`
/// marker replaces the parent `function_definition_rule`). The
/// declarator content (Identifier + optional `(params)`) is inline.
/// The body is the brace-delimited sequence of block items between
/// LBrace and RBrace; bare tokens that aren't wrapped in
/// `compound_statement`/`expression_statement` are classified by
/// `BlockItem::from_cst` (which handles bare `Return`/`Break`/
/// `Continue` sequences too).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionDefinition {
    pub decl_specs: DeclarationSpecifiers,
    pub declarator: Declarator,
    pub body: Braced<BlockItemList>,
    pub span: Span,
}

impl FunctionDefinition {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::FunctionDefinition) {
            return None;
        }
        let specs_node = child_by_rule(cst, node, Rule::DeclarationSpecifiers)?;
        let decl_specs = DeclarationSpecifiers::from_cst(cst, specs_node)?;

        let declarator = Declarator::from_inline(cst, node)?;

        let open = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::LBrace).map(|(_, s)| s))?;
        let close = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::RBrace).map(|(_, s)| s))?;

        let items = extract_block_items(cst, node, open.clone(), close.clone());

        Some(Self {
            decl_specs,
            declarator,
            body: Braced::new(open, items, close),
            span: cst.span(node),
        })
    }
}

/// A function forward declaration: `void f();` or `int x;`.
///
/// Wraps `Rule::ForwardDeclaration` (the `@forward_declaration`
/// marker replaces the parent `forward_declaration_rule`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ForwardDeclaration {
    pub decl_specs: DeclarationSpecifiers,
    pub declarator: Declarator,
    pub semi: Span,
    pub span: Span,
}

impl ForwardDeclaration {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::ForwardDeclaration) {
            return None;
        }
        let specs_node = child_by_rule(cst, node, Rule::DeclarationSpecifiers)?;
        let decl_specs = DeclarationSpecifiers::from_cst(cst, specs_node)?;

        let declarator = Declarator::from_inline(cst, node)?;

        let semi = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Semi).map(|(_, s)| s))?;

        Some(Self {
            decl_specs,
            declarator,
            semi,
            span: cst.span(node),
        })
    }
}

/// An include directive: `include "path.xs";`.
///
/// Wraps `Rule::IncludeDirective`. The path stores the raw lexeme
/// (with surrounding quotes) per the spec's storage strategy.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IncludeDirective {
    pub path: Spanned<String>,
    pub semi: Span,
    pub span: Span,
}

impl IncludeDirective {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::IncludeDirective) {
            return None;
        }
        let path_node = cst
            .children(node)
            .find(|c| cst.match_token(*c, Token::StringLiteral).is_some())?;
        let (text, path_span) = cst.match_token(path_node, Token::StringLiteral)?;
        let semi = cst
            .children(node)
            .find_map(|c| cst.match_token(c, Token::Semi).map(|(_, s)| s))?;
        Some(Self {
            path: Spanned::new(text.to_string(), path_span),
            semi,
            span: cst.span(node),
        })
    }
}

/// A rule block: `rule Name modifier* { body }`.
///
/// Wraps `Rule::RuleDefinition`. **Note**: in the current PoC grammar
/// the `rule_definition` rule never parses cleanly (always emits
/// `ERROR` subtrees — see Pass 1 grammar notes). `from_cst` therefore
/// always returns `None` against real sources today; the code path is
/// kept correct for a grammar that fixes the issue.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RuleDefinition {
    pub name: Identifier,
    pub modifiers: Vec<RuleModifier>,
    pub body: Braced<BlockItemList>,
    pub span: Span,
}

impl RuleDefinition {
    pub fn from_cst(cst: &Cst, node: NodeRef) -> Option<Self> {
        if !cst.match_rule(node, Rule::RuleDefinition) {
            return None;
        }
        let name_node = cst
            .children(node)
            .find(|c| cst.match_token(*c, Token::Identifier).is_some())?;
        let (text, name_span) = cst.match_token(name_node, Token::Identifier)?;
        let name = Identifier::new(text.to_string(), name_span);

        let body_node = child_by_rule(cst, node, Rule::CompoundStatement)?;
        let compound = CompoundStatement::from_cst(cst, body_node)?;

        let modifiers = extract_rule_modifiers(cst, node);

        Some(Self {
            name,
            modifiers,
            body: compound.items,
            span: cst.span(node),
        })
    }
}

/// One rule-block modifier.
///
/// The grammar's `rule_modifier^` collapses the wrapper, so modifiers
/// appear INLINE inside `RuleDefinition`'s children. This enum
/// classifies each modifier token (or token pair) into one of 11
/// variants per the spec.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RuleModifier {
    MinInterval(Spanned<String>),
    MaxInterval(Spanned<String>),
    MinIntervalMS(Spanned<String>),
    MaxIntervalMS(Spanned<String>),
    Priority(Spanned<String>),
    HighFrequency(Span),
    Active(Span),
    Inactive(Span),
    RunImmediately(Span),
    Group(Identifier),
    Literal(Spanned<String>),
}

impl RuleModifier {
    /// Extract a single modifier from a window of `children` starting
    /// at `start`. Returns the classified modifier and the index past
    /// the last consumed child on success.
    fn classify(cst: &Cst, children: &[NodeRef], start: usize) -> Option<(Self, usize)> {
        let node = *children.get(start)?;
        let next_int = children
            .get(start + 1)
            .and_then(|n| cst.match_token(*n, Token::IntConst));
        let next_ident = children
            .get(start + 1)
            .and_then(|n| cst.match_token(*n, Token::Identifier));

        if cst.match_token(node, Token::MinInterval).is_some() {
            if let Some((text, value_span)) = next_int {
                return Some((
                    Self::MinInterval(Spanned::new(text.to_string(), value_span)),
                    start + 2,
                ));
            }
            return None;
        }
        if cst.match_token(node, Token::MaxInterval).is_some() {
            if let Some((text, value_span)) = next_int {
                return Some((
                    Self::MaxInterval(Spanned::new(text.to_string(), value_span)),
                    start + 2,
                ));
            }
            return None;
        }
        if cst.match_token(node, Token::MinIntervalMS).is_some() {
            if let Some((text, value_span)) = next_int {
                return Some((
                    Self::MinIntervalMS(Spanned::new(text.to_string(), value_span)),
                    start + 2,
                ));
            }
            return None;
        }
        if cst.match_token(node, Token::MaxIntervalMS).is_some() {
            if let Some((text, value_span)) = next_int {
                return Some((
                    Self::MaxIntervalMS(Spanned::new(text.to_string(), value_span)),
                    start + 2,
                ));
            }
            return None;
        }
        if cst.match_token(node, Token::Priority).is_some() {
            if let Some((text, value_span)) = next_int {
                return Some((
                    Self::Priority(Spanned::new(text.to_string(), value_span)),
                    start + 2,
                ));
            }
            return None;
        }
        if let Some((_, span)) = cst.match_token(node, Token::HighFrequency) {
            return Some((Self::HighFrequency(span), start + 1));
        }
        if let Some((_, span)) = cst.match_token(node, Token::Active) {
            return Some((Self::Active(span), start + 1));
        }
        if let Some((_, span)) = cst.match_token(node, Token::Inactive) {
            return Some((Self::Inactive(span), start + 1));
        }
        if let Some((_, span)) = cst.match_token(node, Token::RunImmediately) {
            return Some((Self::RunImmediately(span), start + 1));
        }
        if cst.match_token(node, Token::Group).is_some() {
            if let Some((text, id_span)) = next_ident {
                return Some((
                    Self::Group(Identifier::new(text.to_string(), id_span)),
                    start + 2,
                ));
            }
            return None;
        }
        if let Some((text, span)) = cst.match_token(node, Token::IntConst) {
            return Some((
                Self::Literal(Spanned::new(text.to_string(), span)),
                start + 1,
            ));
        }
        None
    }
}

/// Walks `rule_definition`'s children between the rule-name Identifier
/// and the `CompoundStatement` body, classifying each as a `RuleModifier`.
fn extract_rule_modifiers(cst: &Cst, rule_node: NodeRef) -> Vec<RuleModifier> {
    let children: Vec<_> = cst.children(rule_node).collect();
    let name_pos = children
        .iter()
        .position(|c| cst.match_token(*c, Token::Identifier).is_some());
    let body_pos = children
        .iter()
        .position(|c| cst.match_rule(*c, Rule::CompoundStatement));
    let (Some(lo), Some(hi)) = (name_pos, body_pos) else {
        return Vec::new();
    };

    let mut modifiers = Vec::new();
    let mut i = lo + 1;
    while i < hi {
        if is_skip_token(cst, children[i]) {
            i += 1;
            continue;
        }
        if let Some((m, next)) = RuleModifier::classify(cst, &children, i) {
            modifiers.push(m);
            i = next;
        } else {
            // Unknown modifier — bail to avoid an infinite loop.
            break;
        }
    }
    modifiers
}

/// Walks `function_definition`'s children between LBrace and RBrace,
/// dispatching each into a `BlockItem` variant. Handles both
/// rule-wrapped children (compound_statement, expression_statement,
/// declaration, etc.) and bare token sequences (Return + Semi, Break +
/// Semi, Continue + Semi) that appear directly in the function body.
fn extract_block_items(
    cst: &Cst,
    fn_node: NodeRef,
    open: Span,
    close: Span,
) -> BlockItemList {
    let children: Vec<_> = cst.children(fn_node).collect();
    let lo = children
        .iter()
        .position(|c| cst.match_token(*c, Token::LBrace).map(|(_, s)| s) == Some(open.clone()));
    let hi = children
        .iter()
        .position(|c| cst.match_token(*c, Token::RBrace).map(|(_, s)| s) == Some(close.clone()));
    let (Some(lo), Some(hi)) = (lo, hi) else {
        return Vec::new();
    };

    let mut items = Vec::new();
    let mut i = lo + 1;
    while i < hi {
        let child = children[i];
        if is_skip_token(cst, child) {
            i += 1;
            continue;
        }
        // Try rule-wrapped forms first.
        if let Some(item) = BlockItem::from_cst(cst, child) {
            items.push(item);
            i += 1;
            continue;
        }
        // Fall back to bare-token sequences inside the body.
        if let Some((item, next)) = classify_bare_body_item(cst, &children, i) {
            items.push(item);
            i = next;
            continue;
        }
        // Unknown — skip to avoid infinite loop.
        i += 1;
    }
    items
}

/// Classify a bare-token sequence starting at `start` inside a
/// function body. Handles the cases where the parser fails to wrap
/// `return x;`, `break;`, `continue;`, and empty `;` in a
/// `Rule::ReturnStatement`/`BreakStatement`/etc. node (current PoC
/// state).
fn classify_bare_body_item(
    cst: &Cst,
    children: &[NodeRef],
    start: usize,
) -> Option<(BlockItem, usize)> {
    let node = *children.get(start)?;
    if let Some((_, _)) = cst.match_token(node, Token::Return) {
        return Some(bare_return(cst, children, start));
    }
    if let Some((_, _)) = cst.match_token(node, Token::Break) {
        return Some(bare_break(cst, children, start));
    }
    if let Some((_, _)) = cst.match_token(node, Token::Continue) {
        return Some(bare_continue(cst, children, start));
    }
    if let Some((_, semi)) = cst.match_token(node, Token::Semi) {
        let semi_clone = semi.clone();
        return Some((
            BlockItem::Statement(crate::ast::statement::Statement::Expression(
                ExpressionStatement {
                    expr: None,
                    semi,
                    span: semi_clone,
                },
            )),
            start + 1,
        ));
    }
    None
}

fn bare_return(cst: &Cst, children: &[NodeRef], start: usize) -> (BlockItem, usize) {
    let return_node = children[start];
    let (_, return_span) = cst.match_token(return_node, Token::Return).unwrap();
    // Look for an optional expression (first Rule child after Return) and the Semi.
    let mut value: Option<UnparsedExpr> = None;
    let mut semi: Option<Span> = None;
    let mut consumed = start + 1;
    for j in start + 1..children.len() {
        let child = children[j];
        if is_skip_token(cst, child) {
            consumed = j + 1;
            continue;
        }
        if let Some((_, span)) = cst.match_token(child, Token::Semi) {
            semi = Some(span);
            consumed = j + 1;
            break;
        }
        if value.is_none() && matches!(cst.get(child), Node::Rule(_, _)) {
            value = Some(UnparsedExpr(child));
            consumed = j + 1;
        } else {
            break;
        }
    }
    let return_end = return_span.end;
    let (semi_span, span) = match semi {
        Some(s) => {
            let s_end = s.end;
            (s, return_span.start..s_end)
        }
        None => (return_end..return_end, return_span.start..return_end),
    };
    (
        BlockItem::Statement(crate::ast::statement::Statement::Return(ReturnStatement {
            value,
            semi: semi_span,
            span,
        })),
        consumed,
    )
}

fn bare_break(cst: &Cst, children: &[NodeRef], start: usize) -> (BlockItem, usize) {
    let break_node = children[start];
    let (_, break_span) = cst.match_token(break_node, Token::Break).unwrap();
    let mut semi: Option<Span> = None;
    let mut consumed = start + 1;
    for j in start + 1..children.len() {
        let child = children[j];
        if is_skip_token(cst, child) {
            consumed = j + 1;
            continue;
        }
        if let Some((_, span)) = cst.match_token(child, Token::Semi) {
            semi = Some(span);
            consumed = j + 1;
            break;
        }
        break;
    }
    let break_end = break_span.end;
    let (semi_span, span) = match semi {
        Some(s) => {
            let s_end = s.end;
            (s, break_span.start..s_end)
        }
        None => (break_end..break_end, break_span.start..break_end),
    };
    (
        BlockItem::Statement(crate::ast::statement::Statement::Break(BreakStatement {
            semi: semi_span,
            span,
        })),
        consumed,
    )
}

fn bare_continue(cst: &Cst, children: &[NodeRef], start: usize) -> (BlockItem, usize) {
    let cont_node = children[start];
    let (_, cont_span) = cst.match_token(cont_node, Token::Continue).unwrap();
    let mut semi: Option<Span> = None;
    let mut consumed = start + 1;
    for j in start + 1..children.len() {
        let child = children[j];
        if is_skip_token(cst, child) {
            consumed = j + 1;
            continue;
        }
        if let Some((_, span)) = cst.match_token(child, Token::Semi) {
            semi = Some(span);
            consumed = j + 1;
            break;
        }
        break;
    }
    let cont_end = cont_span.end;
    let (semi_span, span) = match semi {
        Some(s) => {
            let s_end = s.end;
            (s, cont_span.start..s_end)
        }
        None => (cont_end..cont_end, cont_span.start..cont_end),
    };
    (
        BlockItem::Statement(crate::ast::statement::Statement::Continue(ContinueStatement {
            semi: semi_span,
            span,
        })),
        consumed,
    )
}

// Helper: expose `children_by_rule` for callers that may need it in
// future extensions (kept in scope to silence dead-import warnings).

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::cst_helpers::{child_by_rule, is_skip_token};
    use crate::ast::type_table::TypeTable;
    use crate::parser::Parser;

    fn parse(source: &str) -> Cst<'_> {
        let mut diags = vec![];
        Parser::new(source, &mut diags).parse(&mut diags)
    }

    fn parse_with_types(source: &str, ctx: TypeTable) -> Cst<'_> {
        let mut diags = vec![];
        Parser::new_with_context(source, &mut diags, ctx).parse(&mut diags)
    }

    fn first_non_skip(cst: &Cst) -> NodeRef {
        cst.children(crate::parser::NodeRef::ROOT)
            .find(|c| !is_skip_token(cst, *c))
            .expect("at least one non-skip child")
    }

    fn has_error_descendant(cst: &Cst, node: NodeRef) -> bool {
        for child in cst.children(node) {
            if let Node::Rule(Rule::Error, _) = cst.get(child) {
                return true;
            }
            if has_error_descendant(cst, child) {
                return true;
            }
        }
        false
    }

    fn has_rule_descendant(cst: &Cst, node: NodeRef, rule: Rule) -> bool {
        for child in cst.children(node) {
            if let Node::Rule(r, _) = cst.get(child) {
                if r == rule {
                    return true;
                }
            }
            if has_rule_descendant(cst, child, rule) {
                return true;
            }
        }
        false
    }

    #[test]
    fn translation_unit_forwards_declaration_dispatches() {
        let cst = parse("int x;");
        let tu = TranslationUnit::from_cst(&cst, crate::parser::NodeRef::ROOT).expect("TU");
        assert_eq!(tu.items.len(), 1);
        assert!(matches!(tu.items[0], TopLevelItem::ForwardDeclaration(_)));
    }

    #[test]
    fn translation_unit_class_dispatch() {
        let cst = parse("class MyClass { int x = 5; void foo() {} }");
        let tu = TranslationUnit::from_cst(&cst, crate::parser::NodeRef::ROOT).expect("TU");
        assert_eq!(tu.items.len(), 1);
        assert!(matches!(tu.items[0], TopLevelItem::ClassDefinition(_)));
    }

    #[test]
    fn class_definition_extracts_name_and_empty_body() {
        let cst = parse("class Empty { }");
        let cd = ClassDefinition::from_cst(&cst, first_non_skip(&cst)).expect("class def");
        assert_eq!(cd.name.node, "Empty");
        assert_eq!(cd.members.inner.len(), 0);
    }

    #[test]
    fn class_definition_extracts_field_and_method() {
        let cst = parse("class MyClass { int x = 5; void foo() {} }");
        let cd = ClassDefinition::from_cst(&cst, first_non_skip(&cst)).expect("class def");
        assert_eq!(cd.name.node, "MyClass");
        assert_eq!(cd.members.inner.len(), 2);
        assert!(matches!(cd.members.inner[0], ClassMember::FieldDeclaration(_)));
        assert!(matches!(cd.members.inner[1], ClassMember::FunctionDefinition(_)));
        let semi = cd.semi.clone();
        // No trailing `;` in the source — extractor fills a zero-width span
        // past the closing brace.
        assert_eq!(semi.start, semi.end);
    }

    // ------------------------------------------------------------------
    // Regression tests for openspec/changes/2026-07-03-fix-class-specifier-
    // no-trailing-semi. The grammar fix dropped the `;` suffix from
    // `class_specifier` so real XS (which never uses `;` after class
    // definitions) parses without spurious diagnostics.
    // ------------------------------------------------------------------

    /// Helper: parse a source and return the Cst together with the
    /// diagnostic count. Lets the regression tests assert the *parser* is
    /// happy (0 diags), not just that AST extraction succeeds.
    fn parse_with_diags(source: &str) -> (Cst, Vec<crate::parser::Diagnostic>) {
        let mut diags = vec![];
        let cst = crate::parser::Parser::new(source, &mut diags).parse(&mut diags);
        (cst, diags)
    }

    #[test]
    fn class_definition_parses_without_trailing_semi_no_diagnostics() {
        // Real XS convention: `class Name { ... }` with no `;` after `}`.
        let source = "class MyClass { int x = 5; }";
        let (cst, diags) = parse_with_diags(source);
        assert!(
            diags.is_empty(),
            "expected 0 diagnostics for class without trailing `;`, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let cd = ClassDefinition::from_cst(&cst, first_non_skip(&cst)).expect("class def");
        assert_eq!(cd.name.node, "MyClass");
        assert_eq!(cd.members.inner.len(), 1);
    }

    #[test]
    fn class_definition_parses_with_trailing_semi_no_diagnostics() {
        // The rare form: `class Name { ... };` with `;` after `}`.
        // After the grammar fix, the `;` is NOT part of `class_specifier`
        // anymore — it's a sibling at the translation_unit level. The
        // parser still produces 0 diagnostics (the stray `;` doesn't
        // fail any rule), and `ClassDefinition::from_cst` returns
        // `Some(cd)` with a zero-width semi span (no `;` inside the
        // class_specifier node to extract).
        let source = "class MyClass { int x = 5; };";
        let (cst, diags) = parse_with_diags(source);
        assert!(
            diags.is_empty(),
            "expected 0 diagnostics for class with trailing `;`, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
        let cd = ClassDefinition::from_cst(&cst, first_non_skip(&cst)).expect("class def");
        assert_eq!(cd.name.node, "MyClass");
        // The trailing `;` is outside class_specifier, so cd.semi is
        // zero-width (the defensive fallback fires).
        assert_eq!(cd.semi.start, cd.semi.end);
    }

    #[test]
    fn class_definition_no_trailing_semi_is_default() {
        // Empty body + no `;`: the simplest form.
        let source = "class Empty { }";
        let (cst, diags) = parse_with_diags(source);
        assert!(diags.is_empty(), "expected 0 diagnostics, got: {:?}", diags);
        let cd = ClassDefinition::from_cst(&cst, first_non_skip(&cst)).expect("class def");
        assert_eq!(cd.name.node, "Empty");
        assert_eq!(cd.members.inner.len(), 0);
        // No `;` in source → semi is zero-width (defensive fallback).
        assert_eq!(cd.semi.start, cd.semi.end);
    }

    #[test]
    fn field_declaration_inside_class_unchanged() {
        // Sanity check: the inner `field_declaration` rule still
        // requires `;` (only `class_specifier`'s trailing `;` was
        // removed). Two class members: a field (with `;`) and a method
        // (with `}` body, no `;` after `}`).
        let source = "class MyClass { int x = 5; void foo() {} }";
        let (cst, diags) = parse_with_diags(source);
        assert!(diags.is_empty(), "expected 0 diagnostics, got: {:?}", diags);
        let cd = ClassDefinition::from_cst(&cst, first_non_skip(&cst)).expect("class def");
        assert_eq!(cd.members.inner.len(), 2);
        assert!(matches!(cd.members.inner[0], ClassMember::FieldDeclaration(_)));
        assert!(matches!(cd.members.inner[1], ClassMember::FunctionDefinition(_)));
    }

    #[test]
    fn include_directive_extracts_path() {
        let cst = parse("include \"path.xs\";");
        let inc = IncludeDirective::from_cst(&cst, first_non_skip(&cst)).expect("include");
        assert_eq!(inc.path.node, "\"path.xs\"");
        assert_eq!(inc.semi, 17..18);
    }

    #[test]
    fn function_definition_extracts_body_with_return() {
        let cst = parse("void myFunc(int a) { return; }");
        let fd = FunctionDefinition::from_cst(&cst, first_non_skip(&cst)).expect("fn def");
        assert_eq!(fd.decl_specs.ty.ty, crate::ast::type_system::Type::Void);
        assert_eq!(fd.body.inner.len(), 1);
        match &fd.body.inner[0] {
            BlockItem::Statement(crate::ast::statement::Statement::Return(_)) => {}
            other => panic!("expected Return statement, got {:?}", other),
        }
    }

    #[test]
    fn function_definition_empty_body() {
        let cst = parse("void f() { }");
        let fd = FunctionDefinition::from_cst(&cst, first_non_skip(&cst)).expect("fn def");
        assert_eq!(fd.body.inner.len(), 0);
    }

    #[test]
    fn function_definition_with_parameter_list() {
        let cst = parse("void myFunc(int a) { return; }");
        let fd = FunctionDefinition::from_cst(&cst, first_non_skip(&cst)).expect("fn def");
        match &fd.declarator.direct {
            crate::ast::declaration::DirectDeclarator::FunctionDeclarator(func_decl) => {
                let params = func_decl.params.as_ref().expect("params");
                assert_eq!(params.inner.items.len(), 1);
            }
            other => panic!("expected FunctionDeclarator, got {:?}", other),
        }
    }

    #[test]
    fn forward_declaration_extracts_int_x() {
        let cst = parse("int x;");
        let fd = ForwardDeclaration::from_cst(&cst, first_non_skip(&cst)).expect("fwd decl");
        assert_eq!(fd.decl_specs.ty.ty, crate::ast::type_system::Type::Int);
        assert_eq!(fd.semi, 5..6);
        match &fd.declarator.direct {
            crate::ast::declaration::DirectDeclarator::IdentDeclarator(id) => {
                assert_eq!(id.name.node, "x");
            }
            other => panic!("expected IdentDeclarator, got {:?}", other),
        }
    }

    #[test]
    fn forward_declaration_with_function_declarator() {
        let cst = parse("void foo();");
        let fd = ForwardDeclaration::from_cst(&cst, first_non_skip(&cst)).expect("fwd decl");
        match &fd.declarator.direct {
            crate::ast::declaration::DirectDeclarator::FunctionDeclarator(func_decl) => {
                assert_eq!(func_decl.base.node, "foo");
                let params = func_decl.params.as_ref().expect("params");
                assert!(params.inner.is_empty());
            }
            other => panic!("expected FunctionDeclarator, got {:?}", other),
        }
    }

    #[test]
    fn field_declaration_extracts_init_and_no_init() {
        let cst = parse("class MyClass { int x = 5; }");
        let cd = ClassDefinition::from_cst(&cst, first_non_skip(&cst)).expect("class");
        let members = &cd.members.inner;
        assert_eq!(members.len(), 1);
        match &members[0] {
            ClassMember::FieldDeclaration(fd) => {
                assert_eq!(fd.decl_specs.ty.ty, crate::ast::type_system::Type::Int);
                assert!(fd.initializer.is_some());
                assert_eq!(fd.semi, 25..26);
            }
            other => panic!("expected FieldDeclaration, got {:?}", other),
        }
    }

    #[test]
    fn rule_definition_extracts_empty_body() {
        // `rule r { }` parses cleanly (the grammar bug only affects
        // non-empty bodies). Use this to exercise the RuleDefinition
        // extraction path.
        let cst = parse("rule r { }");
        let first = first_non_skip(&cst);
        let rd = RuleDefinition::from_cst(&cst, first).expect("rule_definition");
        assert_eq!(rd.name.node, "r");
        assert!(rd.modifiers.is_empty());
        assert_eq!(rd.body.inner.len(), 0);
    }

    // ------------------------------------------------------------------
    // Rule-body extraction scenarios: S-RBE-01..S-RBE-08
    // ------------------------------------------------------------------

    #[test]
    fn rule_definition_extracts_empty_body_s_rbe_01() {
        let cst = parse("rule r { }");
        let first = first_non_skip(&cst);
        let rd = RuleDefinition::from_cst(&cst, first).expect("rule_definition");
        assert_eq!(rd.name.node, "r");
        assert_eq!(rd.body.inner.len(), 0);
        assert!(!has_error_descendant(&cst, first));
    }

    #[test]
    fn rule_definition_extracts_statement_body_s_rbe_02() {
        let cst = parse("rule r { x; }");
        let first = first_non_skip(&cst);
        let rd = RuleDefinition::from_cst(&cst, first).expect("rule_definition");
        assert_eq!(rd.body.inner.len(), 1);
        assert!(matches!(rd.body.inner[0], BlockItem::Statement(_)));
        assert!(!has_error_descendant(&cst, first));
    }

    #[test]
    fn rule_definition_extracts_declaration_body_s_rbe_03() {
        let cst = parse("rule r { int x = 1; }");
        let first = first_non_skip(&cst);
        let rd = RuleDefinition::from_cst(&cst, first).expect("rule_definition");
        assert_eq!(rd.body.inner.len(), 1);
        assert!(matches!(rd.body.inner[0], BlockItem::Declaration(_)));
        assert!(!has_error_descendant(&cst, first));
    }

    #[test]
    fn rule_definition_extracts_class_typed_declaration_body_s_rbe_04() {
        let mut types = TypeTable::with_primitives();
        types.insert_class("MyClass");
        let cst = parse_with_types("rule r { MyClass x; }", types);
        let first = first_non_skip(&cst);
        let rd = RuleDefinition::from_cst(&cst, first).expect("rule_definition");
        assert_eq!(rd.body.inner.len(), 1);
        assert!(matches!(rd.body.inner[0], BlockItem::Declaration(_)));
        assert!(!has_error_descendant(&cst, first));
    }

    #[test]
    fn rule_definition_extracts_mixed_body_s_rbe_05() {
        let cst = parse("rule r { int x = 1; x; }");
        let first = first_non_skip(&cst);
        let rd = RuleDefinition::from_cst(&cst, first).expect("rule_definition");
        assert_eq!(rd.body.inner.len(), 2);
        assert!(matches!(rd.body.inner[0], BlockItem::Declaration(_)));
        assert!(matches!(rd.body.inner[1], BlockItem::Statement(_)));
        assert!(!has_error_descendant(&cst, first));
    }

    #[test]
    fn rule_definition_extracts_retail_snippet_s_rbe_06() {
        // Excerpt from game/ai/core/economy/economic_units.xs::deleteExcessGatherers,
        // stopping before the `for`-loop line (Limitation 2).
        let snippet = r#"rule deleteExcessGatherers
inactive
{
   if (gUnassignedGatherers == 0)
   {
      debugEconomicUnits("We have no idle gatherers currently, nothing to delete.");
      return;
   }
   int queryID = useSimpleUnitQuery(cUnitTypeAbstractVillager);
   int numResults = kbUnitQueryExecute(queryID);
   int numAlreadyDeleted = 0;
}"#;
        let cst = parse(snippet);
        let first = first_non_skip(&cst);
        let rd = RuleDefinition::from_cst(&cst, first).expect("rule_definition");
        assert_eq!(rd.name.node, "deleteExcessGatherers");
        assert!(!has_error_descendant(&cst, first));
    }

    #[test]
    fn rule_definition_empty_table_int_is_not_declaration_s_rbe_07() {
        // With an empty TypeTable, `int` is not treated as a declaration
        // start. The body does not produce a clean RuleDefinition (the
        // statement fallback for `int x;` is not valid XS), but the
        // predicate prevents it from being classified as a Declaration.
        let table = TypeTable {
            primitives: std::collections::HashSet::new(),
            classes: std::collections::HashSet::new(),
        };
        let cst = parse_with_types("rule r { int x; }", table);
        let first = first_non_skip(&cst);
        assert!(!has_rule_descendant(&cst, first, Rule::Declaration));
    }

    #[test]
    fn rule_definition_unknown_class_is_not_declaration_s_rbe_08() {
        // A class that has not been inserted into the table is not
        // treated as a declaration start. As with S-RBE-07, the body is
        // not a valid statement, but the grammar must not produce a
        // clean Declaration node for it.
        let cst = parse_with_types("rule r { MyClass x; }", TypeTable::with_primitives());
        let first = first_non_skip(&cst);
        assert!(!has_rule_descendant(&cst, first, Rule::Declaration));
    }

    #[test]
    fn top_level_item_dispatches_preproc() {
        let cst = parse("#define X");
        let first = first_non_skip(&cst);
        match TopLevelItem::from_cst(&cst, first) {
            Some(TopLevelItem::PreprocDef(_)) => {}
            other => panic!("expected PreprocDef, got {:?}", other),
        }
    }

    #[test]
    fn top_level_item_returns_none_for_error_node() {
        // A malformed class specifier produces a CST node that the
        // typed-AST layer refuses to dispatch.
        let cst = parse("class { }");
        let first = first_non_skip(&cst);
        assert!(TopLevelItem::from_cst(&cst, first).is_none());
    }

    #[test]
    fn top_level_item_extracts_rule_definition() {
        let cst = parse("rule r { }");
        let first = first_non_skip(&cst);
        match TopLevelItem::from_cst(&cst, first) {
            Some(TopLevelItem::RuleDefinition(_)) => {}
            other => panic!("expected RuleDefinition, got {:?}", other),
        }
    }

    #[test]
    fn child_by_rule_helper_used_for_extraction() {
        // Regression: ensure our use of child_by_rule against
        // `Rule::DeclarationSpecifiers` works on a forward declaration.
        let cst = parse("int x;");
        let node = first_non_skip(&cst);
        let specs = child_by_rule(&cst, node, Rule::DeclarationSpecifiers).expect("specs");
        assert!(cst.match_rule(specs, Rule::DeclarationSpecifiers));
    }
}
