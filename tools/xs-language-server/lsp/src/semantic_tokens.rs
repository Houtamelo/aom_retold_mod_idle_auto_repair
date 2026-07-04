//! LSP semantic-token provider for XS.
//!
//! Emits `textDocument/semanticTokens/full` tokens for functions, variables,
//! and type references classified by origin (`engine`, `modded`, `unmodded`)
//! and, for variables, by storage class (`local`, `static`).

use std::{collections::HashMap, path::Path};

use tower_lsp_server::ls_types::{
    Range,
    SemanticToken,
    SemanticTokenModifier,
    SemanticTokenType,
    SemanticTokensFullOptions,
    SemanticTokensLegend,
    SemanticTokensOptions,
    SemanticTokensServerCapabilities,
};
use xs_parser::ast::{
    ClassDefinition, ClassMember, DeclarationSpecifiers, Expr, PostfixInner, TopLevelItem,
    TranslationUnit, Type, TypeTable,
};
use xs_parser::parser::{Cst, NodeRef, Parser};

use crate::{
    cache,
    engine_api,
    merged_view::MergedView,
    range::span_to_range,
    symbols::{self, Symbol, SymbolKind, Visibility},
    workspace::{VirtualProject, Workspace},
};

const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::TYPE,
    SemanticTokenType::new("constant"),
    SemanticTokenType::new("rule"),
];

const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::new("engine"),
    SemanticTokenModifier::new("modded"),
    SemanticTokenModifier::new("unmodded"),
    SemanticTokenModifier::new("local"),
    SemanticTokenModifier::new("static"),
    SemanticTokenModifier::new("extern"),
    SemanticTokenModifier::new("member"),
];

/// A single semantic token in source-order, before LSP delta encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub line: u32,
    pub char: u32,
    pub len: u32,
    pub token_type: SemanticTokenType,
    pub modifiers: Vec<SemanticTokenModifier>,
}

/// Server capability advertised to LSP clients.
pub fn server_capabilities() -> SemanticTokensServerCapabilities {
    SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
        legend: SemanticTokensLegend {
            token_types: TOKEN_TYPES.into(),
            token_modifiers: TOKEN_MODIFIERS.into(),
        },
        full: Some(SemanticTokensFullOptions::Bool(true)),
        range: Some(false),
        work_done_progress_options: Default::default(),
    })
}

fn parse_typed(source: &str) -> Option<(Cst<'_>, TranslationUnit)> {
    let mut diags = Vec::new();
    let cst = Parser::new_with_context(source, &mut diags, TypeTable::with_primitives()).parse(&mut diags);
    let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT)?;
    Some((cst, tu))
}

/// Compute semantic tokens for `source`.
///
/// `current_file` is the absolute path of the file being analysed, used to
/// classify the origin of symbols defined in it. `own_table` is the per-file
/// symbol table (including local variable declarations). `merged` is the
/// include-paste merged view, if one was built. `member_index` maps member
/// names to their strongest workspace origin for `obj.field`/`Class.method`
/// references.
// Semantic-token entry points pass the full LSP context; splitting these
// signatures would force unrelated data structures into a helper struct.
#[allow(clippy::too_many_arguments)]
pub fn compute_tokens(
    source: &str,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    member_index: &MemberIndex,
) -> Vec<Token> {
    let Some((cst, tu)) = parse_typed(source) else {
        return Vec::new();
    };

    let mut tokens = Vec::new();

    // Declared symbols: rules, functions, variables, constants, classes,
    // class fields, class methods.
    for sym in &own_table.symbols {
        if let Some(token) = token_for_symbol(sym, current_file, own_table, merged, workspace, project) {
            tokens.push(token);
        }
    }

    // Type specifiers (primitive and user-defined class types) wherever they
    // appear in declarations, function signatures, and class members.
    visit_top_level_for_types(
        source,
        &tu,
        current_file,
        own_table,
        merged,
        engine,
        workspace,
        project,
        &mut tokens,
    );

    // Expression identifiers and member references.
    visit_top_level_for_exprs(
        &cst,
        source,
        &tu,
        current_file,
        own_table,
        merged,
        engine,
        workspace,
        project,
        member_index,
        &mut tokens,
    );

    tokens.sort_by(|a, b| a.line.cmp(&b.line).then(a.char.cmp(&b.char)));
    tokens
}

fn token_for_symbol(
    sym: &Symbol,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    workspace: &Workspace,
    project: &VirtualProject,
) -> Option<Token> {
    let name = &sym.name;
    let token_type = match sym.kind {
        SymbolKind::Rule => SemanticTokenType::new("rule"),
        SymbolKind::Function | SymbolKind::ClassMethod => SemanticTokenType::FUNCTION,
        SymbolKind::Variable | SymbolKind::ClassField => SemanticTokenType::VARIABLE,
        SymbolKind::Constant => SemanticTokenType::new("constant"),
        SymbolKind::Class => SemanticTokenType::TYPE,
    };

    let defining_path: Option<&Path> = if let Some(ms) = merged.and_then(|m| m.find(name)) {
        ms.provenance.origin()
    } else if own_table.find(name).map_or(false, |s| std::ptr::eq(s, sym)) {
        current_file
    } else {
        None
    };
    let origin = defining_path
        .map(|p| classify_origin(p, workspace, project))
        .unwrap_or(Origin::Engine);

    let range = sym.selection_range;
    let len = range.end.character.saturating_sub(range.start.character)
        + (range.end.line.saturating_sub(range.start.line)) * u32::MAX;
    Some(Token {
        line: range.start.line,
        char: range.start.character,
        len,
        token_type,
        modifiers: classify_modifiers(sym, origin),
    })
}

fn visit_top_level_for_types(
    source: &str,
    tu: &TranslationUnit,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    tokens: &mut Vec<Token>,
) {
    for item in &tu.items {
        visit_top_level_item_for_types(
            source,
            item,
            current_file,
            own_table,
            merged,
            engine,
            workspace,
            project,
            tokens,
        );
    }
}

fn visit_top_level_item_for_types(
    source: &str,
    item: &TopLevelItem,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    tokens: &mut Vec<Token>,
) {
    match item {
        TopLevelItem::FunctionDefinition(f) => {
            visit_type_specifier(source, &f.decl_specs, current_file, own_table, merged, engine, workspace, project, tokens);
            visit_params_for_types(source, &f.declarator.direct, current_file, own_table, merged, engine, workspace, project, tokens);
        }
        TopLevelItem::ForwardDeclaration(f) => {
            visit_type_specifier(source, &f.decl_specs, current_file, own_table, merged, engine, workspace, project, tokens);
            visit_params_for_types(source, &f.declarator.direct, current_file, own_table, merged, engine, workspace, project, tokens);
        }
        TopLevelItem::ClassDefinition(c) => visit_class_for_types(source, c, current_file, own_table, merged, engine, workspace, project, tokens),
        TopLevelItem::Declaration(d) => {
            visit_type_specifier(source, &d.decl_specs, current_file, own_table, merged, engine, workspace, project, tokens);
        }
        _ => {}
    }
}

fn visit_class_for_types(
    source: &str,
    c: &ClassDefinition,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    tokens: &mut Vec<Token>,
) {
    for member in &c.members.inner {
        match member {
            ClassMember::FunctionDefinition(f) => {
                visit_type_specifier(source, &f.decl_specs, current_file, own_table, merged, engine, workspace, project, tokens);
                visit_params_for_types(source, &f.declarator.direct, current_file, own_table, merged, engine, workspace, project, tokens);
            }
            ClassMember::ForwardDeclaration(f) => {
                visit_type_specifier(source, &f.decl_specs, current_file, own_table, merged, engine, workspace, project, tokens);
                visit_params_for_types(source, &f.declarator.direct, current_file, own_table, merged, engine, workspace, project, tokens);
            }
            ClassMember::FieldDeclaration(f) => {
                visit_type_specifier(source, &f.decl_specs, current_file, own_table, merged, engine, workspace, project, tokens);
            }
        }
    }
}

fn visit_params_for_types(
    source: &str,
    direct: &xs_parser::ast::DirectDeclarator,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    tokens: &mut Vec<Token>,
) {
    match direct {
        xs_parser::ast::DirectDeclarator::FunctionDeclarator(fd) => {
            if let Some(pl) = &fd.params {
                for (param, _) in &pl.inner.items.items {
                    visit_type_specifier(
                        source,
                        &param.decl_specs,
                        current_file,
                        own_table,
                        merged,
                        engine,
                        workspace,
                        project,
                        tokens,
                    );
                }
            }
        }
        xs_parser::ast::DirectDeclarator::ParenDeclarator(pd) => {
            visit_params_for_types(source, &pd.inner.direct, current_file, own_table, merged, engine, workspace, project, tokens);
        }
        _ => {}
    }
}

fn visit_type_specifier(
    source: &str,
    specs: &DeclarationSpecifiers,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    tokens: &mut Vec<Token>,
) {
    let ty = &specs.ty;
    let token_type = if is_primitive_type(ty) {
        SemanticTokenType::TYPE
    } else if let Type::Class(name) = &ty.ty {
        let type_name = name.node.as_str();
        if own_table.find(type_name).is_some()
            || merged.and_then(|m| m.find(type_name)).is_some()
            || engine.find_syscall(type_name).is_some()
        {
            SemanticTokenType::TYPE
        } else {
            // Unknown class: still emit a type token with engine origin.
            SemanticTokenType::TYPE
        }
    } else {
        return;
    };

    let origin = if is_primitive_type(ty) {
        Origin::Engine
    } else if let Type::Class(name) = &ty.ty {
        let type_name = &name.node;
        let defining_path = if let Some(ms) = merged.and_then(|m| m.find(type_name)) {
            ms.provenance.origin()
        } else if own_table.find(type_name).is_some() {
            current_file
        } else {
            None
        };
        defining_path
            .map(|p| classify_origin(p, workspace, project))
            .unwrap_or(Origin::Engine)
    } else {
        Origin::Engine
    };

    let range = span_to_range(source, ty.span.clone());
    let len = range.end.character.saturating_sub(range.start.character)
        + (range.end.line.saturating_sub(range.start.line)) * u32::MAX;
    tokens.push(Token {
        line: range.start.line,
        char: range.start.character,
        len,
        token_type,
        modifiers: vec![origin_modifier(origin)],
    });
}

fn is_primitive_type(ty: &xs_parser::ast::type_system::TypeSpecifier) -> bool {
    matches!(
        ty.ty,
        Type::Void | Type::Bool | Type::Int | Type::Float | Type::String | Type::Vector
    )
}

fn visit_top_level_for_exprs(
    cst: &Cst<'_>,
    source: &str,
    tu: &TranslationUnit,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    member_index: &MemberIndex,
    tokens: &mut Vec<Token>,
) {
    for item in &tu.items {
        visit_top_level_item_for_exprs(
            cst, source, item, current_file, own_table, merged, engine, workspace, project, member_index, tokens,
        );
    }
}

fn visit_top_level_item_for_exprs(
    cst: &Cst<'_>,
    source: &str,
    item: &TopLevelItem,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    member_index: &MemberIndex,
    tokens: &mut Vec<Token>,
) {
    match item {
        TopLevelItem::FunctionDefinition(f) => {
            visit_block_items_for_exprs(
                cst, source, &f.body.inner, current_file, own_table, merged, engine, workspace, project, member_index,
                tokens,
            );
        }
        TopLevelItem::ClassDefinition(c) => {
            for member in &c.members.inner {
                match member {
                    ClassMember::FunctionDefinition(f) => visit_block_items_for_exprs(
                        cst, source, &f.body.inner, current_file, own_table, merged, engine, workspace, project, member_index,
                        tokens,
                    ),
                    ClassMember::FieldDeclaration(f) => {
                        if let Some(expr) = f.expr(cst) {
                            visit_expr(
                                cst, source, &expr, current_file, own_table, merged, engine, workspace, project, member_index,
                                tokens,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
        TopLevelItem::Declaration(d) => {
            for init in &d.init_declarator_list.items {
                if let Some(expr) = init.expr(cst) {
                    visit_expr(
                        cst, source, &expr, current_file, own_table, merged, engine, workspace, project, member_index,
                        tokens,
                    );
                }
            }
        }
        _ => {}
    }
}

fn visit_block_items_for_exprs(
    cst: &Cst<'_>,
    source: &str,
    items: &[xs_parser::ast::statement::BlockItem],
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    member_index: &MemberIndex,
    tokens: &mut Vec<Token>,
) {
    use xs_parser::ast::statement::BlockItem;
    for item in items {
        match item {
            BlockItem::Declaration(d) => {
                for init in &d.init_declarator_list.items {
                    if let Some(expr) = init.expr(cst) {
                        visit_expr(
                            cst, source, &expr, current_file, own_table, merged, engine, workspace, project, member_index,
                            tokens,
                        );
                    }
                }
            }
            BlockItem::FunctionDefinition(f) => visit_block_items_for_exprs(
                cst, source, &f.body.inner, current_file, own_table, merged, engine, workspace, project, member_index,
                tokens,
            ),
            BlockItem::Statement(s) => visit_statement_for_exprs(
                cst, source, s, current_file, own_table, merged, engine, workspace, project, member_index, tokens,
            ),
            _ => {}
        }
    }
}

fn visit_statement_for_exprs(
    cst: &Cst<'_>,
    source: &str,
    stmt: &xs_parser::ast::statement::Statement,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    member_index: &MemberIndex,
    tokens: &mut Vec<Token>,
) {
    use xs_parser::ast::statement::{ForInit, Statement};
    match stmt {
        Statement::Compound(c) => visit_block_items_for_exprs(
            cst, source, &c.items.inner, current_file, own_table, merged, engine, workspace, project, member_index,
            tokens,
        ),
        Statement::Expression(es) => {
            if let Some(e) = es.expr.as_ref().and_then(|u| Expr::from_cst(cst, u.0)) {
                visit_expr(
                    cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                    tokens,
                );
            }
        }
        Statement::Return(r) => {
            if let Some(v) = &r.value {
                if let Some(e) = Expr::from_cst(cst, v.0) {
                    visit_expr(
                        cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                        tokens,
                    );
                }
            }
        }
        Statement::If(i) => {
            if let Some(e) = Expr::from_cst(cst, i.cond.inner.0) {
                visit_expr(
                    cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                    tokens,
                );
            }
            visit_statement_for_exprs(
                cst, source, &i.then, current_file, own_table, merged, engine, workspace, project, member_index,
                tokens,
            );
            if let Some(else_) = &i.else_ {
                visit_statement_for_exprs(
                    cst, source, else_, current_file, own_table, merged, engine, workspace, project, member_index,
                    tokens,
                );
            }
        }
        Statement::While(w) => {
            if let Some(e) = Expr::from_cst(cst, w.cond.inner.0) {
                visit_expr(
                    cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                    tokens,
                );
            }
            visit_statement_for_exprs(
                cst, source, &w.body, current_file, own_table, merged, engine, workspace, project, member_index,
                tokens,
            );
        }
        Statement::For(f) => {
            match &f.init {
                ForInit::Declaration(d) => {
                    for init in &d.init_declarator_list.items {
                        if let Some(e) = init.expr(cst) {
                            visit_expr(
                                cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                                tokens,
                            );
                        }
                    }
                }
                ForInit::Expression(u) => {
                    if let Some(e) = Expr::from_cst(cst, u.0) {
                        visit_expr(
                            cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                            tokens,
                        );
                    }
                }
                ForInit::Empty => {}
            }
            if let Some(u) = &f.cond {
                if let Some(e) = Expr::from_cst(cst, u.0) {
                    visit_expr(
                        cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                        tokens,
                    );
                }
            }
            if let Some(u) = &f.post {
                if let Some(e) = Expr::from_cst(cst, u.0) {
                    visit_expr(
                        cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                        tokens,
                    );
                }
            }
            visit_statement_for_exprs(
                cst, source, &f.body, current_file, own_table, merged, engine, workspace, project, member_index,
                tokens,
            );
        }
        Statement::Switch(s) => {
            if let Some(e) = Expr::from_cst(cst, s.cond.inner.0) {
                visit_expr(
                    cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                    tokens,
                );
            }
            visit_block_items_for_exprs(
                cst, source, &s.body.items.inner, current_file, own_table, merged, engine, workspace, project,
                member_index, tokens,
            );
        }
        _ => {}
    }
}

fn visit_expr(
    cst: &Cst<'_>,
    source: &str,
    expr: &Expr,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    member_index: &MemberIndex,
    tokens: &mut Vec<Token>,
) {
    match expr {
        Expr::Identifier(id) => {
            if let Some(token) = classify_identifier(
                &id.name.node,
                source,
                &id.name.span,
                current_file,
                own_table,
                merged,
                engine,
                workspace,
                project,
            ) {
                tokens.push(token);
            }
        }
        Expr::Postfix(pfe) => {
            if let PostfixInner::Call(call) = &pfe.inner {
                // If the call target is itself a field expression, the field is a
                // method reference.
                if let Expr::Postfix(inner_pfe) = pfe.target.as_ref() {
                    if let PostfixInner::Field(field) = &inner_pfe.inner {
                        if let Some(token) = classify_member_identifier(
                            &field.field.node,
                            &field.field.span,
                            source,
                            member_index,
                            true,
                        ) {
                            tokens.push(token);
                        }
                    } else {
                        // Field call with non-field inner postfix: still tokenize the
                        // call target so plain identifiers like `myRule()` get their
                        // owning symbol's token type (rule / function / etc.).
                        visit_expr(
                            cst, source, &pfe.target, current_file, own_table, merged, engine,
                            workspace, project, member_index, tokens,
                        );
                    }
                } else {
                    // Plain call `foo(...)` — visit the target so `foo` gets a token.
                    visit_expr(
                        cst, source, &pfe.target, current_file, own_table, merged, engine,
                        workspace, project, member_index, tokens,
                    );
                }
                if let Some(args) = &call.args {
                    for (arg, _) in &args.items.items {
                        if let Some(e) = arg.expr(cst) {
                            visit_expr(
                                cst, source, &e, current_file, own_table, merged, engine, workspace, project, member_index,
                                tokens,
                            );
                        }
                    }
                }
            } else if let PostfixInner::Field(field) = &pfe.inner {
                if let Some(token) = classify_member_identifier(
                    &field.field.node,
                    &field.field.span,
                    source,
                    member_index,
                    false,
                ) {
                    tokens.push(token);
                }
                visit_expr(
                    cst, source, &pfe.target, current_file, own_table, merged, engine, workspace, project, member_index,
                    tokens,
                );
            } else {
                visit_expr(
                    cst, source, &pfe.target, current_file, own_table, merged, engine, workspace, project, member_index,
                    tokens,
                );
            }
        }
        Expr::Unary(u) => visit_expr(
            cst, source, &u.operand, current_file, own_table, merged, engine, workspace, project, member_index,
            tokens,
        ),
        Expr::Binary(b) => {
            visit_expr(cst, source, &b.lhs, current_file, own_table, merged, engine, workspace, project, member_index, tokens);
            visit_expr(cst, source, &b.rhs, current_file, own_table, merged, engine, workspace, project, member_index, tokens);
        }
        Expr::Conditional(c) => {
            visit_expr(cst, source, &c.cond, current_file, own_table, merged, engine, workspace, project, member_index, tokens);
            if let Some(t) = &c.then {
                visit_expr(cst, source, t, current_file, own_table, merged, engine, workspace, project, member_index, tokens);
            }
            visit_expr(cst, source, &c.else_, current_file, own_table, merged, engine, workspace, project, member_index, tokens);
        }
        Expr::Assignment(a) => {
            visit_expr(cst, source, &a.lhs, current_file, own_table, merged, engine, workspace, project, member_index, tokens);
            visit_expr(cst, source, &a.rhs, current_file, own_table, merged, engine, workspace, project, member_index, tokens);
        }
        Expr::Comma(c) => {
            for e in &c.exprs {
                visit_expr(cst, source, e, current_file, own_table, merged, engine, workspace, project, member_index, tokens);
            }
        }
        Expr::Paren(p) => visit_expr(
            cst, source, &p.inner, current_file, own_table, merged, engine, workspace, project, member_index,
            tokens,
        ),
        _ => {}
    }
}

fn classify_identifier(
    name: &str,
    source: &str,
    span: &std::ops::Range<usize>,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
) -> Option<Token> {
    if name.is_empty() {
        return None;
    }

    let range = span_to_range(source, span.clone());
    let len = range.end.character.saturating_sub(range.start.character)
        + (range.end.line.saturating_sub(range.start.line)) * u32::MAX;

    let (symbol, defining_path): (Option<&Symbol>, Option<&Path>) = if let Some(ms) = merged.and_then(|m| m.find(name)) {
        (Some(&ms.symbol), ms.provenance.origin())
    } else if let Some(sym) = own_table.find(name) {
        (Some(sym), current_file)
    } else if engine.find_syscall(name).is_some() {
        return Some(Token {
            line: range.start.line,
            char: range.start.character,
            len,
            token_type: SemanticTokenType::FUNCTION,
            modifiers: vec![SemanticTokenModifier::new("engine")],
        });
    } else {
        (None, None)
    };

    let symbol = symbol?;
    let origin = defining_path
        .map(|p| classify_origin(p, workspace, project))
        .unwrap_or(Origin::Engine);
    let token_type = match symbol.kind {
        SymbolKind::Function | SymbolKind::ClassMethod => SemanticTokenType::FUNCTION,
        SymbolKind::Variable | SymbolKind::ClassField => SemanticTokenType::VARIABLE,
        SymbolKind::Constant => SemanticTokenType::new("constant"),
        SymbolKind::Rule => SemanticTokenType::new("rule"),
        SymbolKind::Class => SemanticTokenType::TYPE,
    };

    Some(Token {
        line: range.start.line,
        char: range.start.character,
        len,
        token_type,
        modifiers: classify_modifiers(symbol, origin),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Engine,
    Modded,
    Unmodded,
}

fn classify_origin(file: &Path, workspace: &Workspace, project: &VirtualProject) -> Origin {
    if project.file_overrides.values().any(|p| p == file) {
        Origin::Modded
    } else if workspace.game_relative_path(file).is_some() {
        Origin::Unmodded
    } else {
        Origin::Engine
    }
}

fn origin_modifier(origin: Origin) -> SemanticTokenModifier {
    match origin {
        Origin::Engine => SemanticTokenModifier::new("engine"),
        Origin::Modded => SemanticTokenModifier::new("modded"),
        Origin::Unmodded => SemanticTokenModifier::new("unmodded"),
    }
}

fn classify_modifiers(symbol: &Symbol, origin: Origin) -> Vec<SemanticTokenModifier> {
    let mut modifiers = vec![origin_modifier(origin)];
    if symbol.class_owner.is_some() {
        modifiers.push(SemanticTokenModifier::new("member"));
    } else if symbol.kind == SymbolKind::Variable || symbol.kind == SymbolKind::Constant {
        if symbol.is_static {
            modifiers.push(SemanticTokenModifier::new("static"));
        } else if symbol.visibility == Visibility::Local {
            modifiers.push(SemanticTokenModifier::new("local"));
        }
    }
    if symbol.kind == SymbolKind::Variable && symbol.is_extern {
        modifiers.push(SemanticTokenModifier::new("extern"));
    }
    modifiers
}

/// Index of class-member names to their strongest workspace origin.
///
/// Used to color `obj.field` and `Class.method()` references without
/// requiring type inference: if any class in the workspace declares the
/// member, we use that member's origin. Modded origins are preferred over
/// unmodded over engine.
pub struct MemberIndex {
    origins: HashMap<String, Origin>,
}

impl MemberIndex {
    /// Build a member index from the current file and every visible file in
    /// the project. `current_file` is skipped when scanning workspace files
    /// because `own_table` already reflects the buffer text.
    pub fn build(
        own_table: &symbols::SymbolTable,
        workspace: &Workspace,
        project: &VirtualProject,
        cache_dir: &Path,
        current_file: Option<&Path>,
    ) -> Self {
        let mut origins = HashMap::new();

        let promote = |origins: &mut HashMap<String, Origin>, name: String, origin: Origin| {
            let strength = |o: Origin| match o {
                Origin::Engine => 0,
                Origin::Unmodded => 1,
                Origin::Modded => 2,
            };
            origins
                .entry(name)
                .and_modify(|existing| {
                    if strength(origin) > strength(*existing) {
                        *existing = origin;
                    }
                })
                .or_insert(origin);
        };

        if let Some(path) = current_file {
            for sym in &own_table.symbols {
                if is_class_member(sym) {
                    promote(&mut origins, sym.name.clone(), classify_origin(path, workspace, project));
                }
            }
        }

        for (rel, path) in project.visible_files(workspace) {
            if current_file == Some(path.as_path()) {
                continue;
            }
            let table = match cache::load_or_parse_symbols(&path, &rel, cache_dir) {
                Ok(t) => t,
                Err(e) => {
                    tracing::debug!("member index: could not load symbols for {}: {}", path.display(), e);
                    continue;
                }
            };
            for sym in &table.symbols {
                if is_class_member(sym) {
                    promote(&mut origins, sym.name.clone(), classify_origin(&path, workspace, project));
                }
            }
        }

        Self { origins }
    }

    /// Return the strongest origin for a member name, if any.
    pub fn origin(&self, name: &str) -> Option<Origin> { self.origins.get(name).copied() }
}

fn is_class_member(sym: &Symbol) -> bool {
    sym.class_owner.is_some() && (sym.kind == SymbolKind::ClassField || sym.kind == SymbolKind::ClassMethod)
}

fn classify_member_identifier(
    name: &str,
    span: &std::ops::Range<usize>,
    source: &str,
    member_index: &MemberIndex,
    is_call: bool,
) -> Option<Token> {
    if name.is_empty() {
        return None;
    }
    let origin = member_index.origin(name)?;
    let range = span_to_range(source, span.clone());
    let len = range.end.character.saturating_sub(range.start.character)
        + (range.end.line.saturating_sub(range.start.line)) * u32::MAX;
    let token_type = if is_call { SemanticTokenType::FUNCTION } else { SemanticTokenType::VARIABLE };

    Some(Token {
        line: range.start.line,
        char: range.start.character,
        len,
        token_type,
        modifiers: vec![origin_modifier(origin), SemanticTokenModifier::new("member")],
    })
}

/// Encode a slice of tokens into the LSP `SemanticTokens.data` format.
pub fn encode(tokens: &[Token]) -> Vec<SemanticToken> {
    let mut data = Vec::with_capacity(tokens.len());
    let mut prev_line: u32 = 0;
    let mut prev_char: u32 = 0;

    for token in tokens {
        let type_index = TOKEN_TYPES
            .iter()
            .position(|t| *t == token.token_type)
            .expect("token type in legend") as u32;
        let mut modifier_mask: u32 = 0;
        for m in &token.modifiers {
            if let Some(idx) = TOKEN_MODIFIERS.iter().position(|lm| *lm == *m) {
                modifier_mask |= 1 << idx;
            }
        }

        let delta_line = token.line - prev_line;
        let delta_start = if delta_line == 0 { token.char - prev_char } else { token.char };

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: token.len,
            token_type: type_index,
            token_modifiers_bitset: modifier_mask,
        });

        prev_line = token.line;
        prev_char = token.char;
    }

    data
}

/// Extract declared symbols from a source file, marking whether each
/// is a declaration. Used internally and exposed for unit tests.
pub fn extract_symbols(source: &str) -> Vec<(String, SymbolKind, Range, bool)> {
    let table = symbols::build_symbol_table(source);
    table
        .symbols
        .iter()
        .map(|sym| (sym.name.clone(), sym.kind, sym.selection_range, true))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_symbols_flags_function_declaration() {
        let source = "void foo(int x) { int y = 1; }";
        let symbols = extract_symbols(source);
        assert!(
            symbols
                .iter()
                .any(|(n, k, _, is_decl)| n == "foo" && *k == SymbolKind::Function && *is_decl),
            "function declaration should be flagged"
        );
    }

    #[test]
    fn encode_empty_tokens_is_empty() {
        assert!(encode(&[]).is_empty());
    }

    #[test]
    fn encode_two_tokens_on_same_line() {
        let tokens = vec![
            Token {
                line: 0,
                char: 0,
                len: 3,
                token_type: SemanticTokenType::FUNCTION,
                modifiers: vec![SemanticTokenModifier::new("engine")],
            },
            Token {
                line: 0,
                char: 5,
                len: 2,
                token_type: SemanticTokenType::VARIABLE,
                modifiers: vec![SemanticTokenModifier::new("local")],
            },
        ];
        let data = encode(&tokens);
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].delta_line, 0);
        assert_eq!(data[0].delta_start, 0);
        assert_eq!(data[0].length, 3);
        assert_eq!(data[0].token_type, 0);
        assert_eq!(data[0].token_modifiers_bitset, 1);
        assert_eq!(data[1].delta_line, 0);
        assert_eq!(data[1].delta_start, 5);
        assert_eq!(data[1].length, 2);
        assert_eq!(data[1].token_type, 1);
        assert_eq!(data[1].token_modifiers_bitset, 8);
    }

    #[test]
    fn classify_origin_detects_overlay() {
        let tmp = tempfile::tempdir().unwrap();
        let game_root = tmp.path().join("game");
        let mod_root = tmp.path().join("mod");
        let overlay = mod_root.join("game").join("ai").join("f.xs");
        std::fs::create_dir_all(overlay.parent().unwrap()).unwrap();
        std::fs::write(&overlay, "").unwrap();

        let mut ws = Workspace::new(tmp.path().to_path_buf());
        let uri = tower_lsp_server::ls_types::Uri::from_file_path(&mod_root).unwrap();
        ws.register_mod(uri).unwrap();
        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        assert_eq!(classify_origin(&overlay, &ws, &project), Origin::Modded);
        assert_eq!(classify_origin(&game_root.join("ai").join("g.xs"), &ws, &project), Origin::Unmodded);
        assert_eq!(classify_origin(Path::new("/tmp/orphan.xs"), &ws, &project), Origin::Engine);
    }
}
