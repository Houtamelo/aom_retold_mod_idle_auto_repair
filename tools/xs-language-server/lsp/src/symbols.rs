//! Per-file symbol table: extracts rule/function/variable declarations from
//! the typed AST so we can resolve hover, go-to-definition, and provide a
//! document outline.
//!
//! Extraction walks `TranslationUnit::items` and dispatches into:
//!   * `TopLevelItem::RuleDefinition`     -> Rule
//!   * `TopLevelItem::FunctionDefinition` -> Function
//!   * `TopLevelItem::ClassDefinition`    -> Class + members
//!   * `TopLevelItem::Declaration`        -> Variable or Constant
//!   * `TopLevelItem::ForwardDeclaration` -> Function (forward-only)
//!
//! Note: `#define` is NOT supported (XS doesn't use it; only `#if`/`#ifdef`
//! appear in real code, and those files are already failing to parse
//! anyway). When preproc support is added, add a `Macro` variant here.

use std::collections::HashSet;

use tower_lsp_server::ls_types::Range;
use xs_parser::ast::expr::{CallExpr, Expr, PostfixExpr, PostfixInner, StringLiteral};
use xs_parser::ast::statement::{BlockItem, ForInit, Statement};
use xs_parser::ast::type_table::TypeTable;
use xs_parser::ast::{
    ClassDefinition, ClassMember, Declaration, DeclarationSpecifiers, DirectDeclarator,
    ForwardDeclaration, FunctionDefinition, FunctionDeclarator, Identifier, InitDeclarator,
    ParameterDeclaration, ParameterInner, ParameterList, RegularParam, RuleDefinition,
    StorageClassSpecifier, TopLevelItem, TranslationUnit, Type, TypeQualifier,
};
use xs_parser::parser::{Cst, NodeRef, Parser, Span};

use crate::range::span_to_range;

/// What kind of XS construct a symbol represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SymbolKind {
    Rule,
    Function,
    Variable,
    Constant,
    Class,
    ClassField,
    ClassMethod,
}

impl SymbolKind {
    /// One-line detail string for hover / outline (e.g. "int gReservePlan").
    pub fn label(&self) -> &'static str {
        match self {
            SymbolKind::Rule => "rule",
            SymbolKind::Function => "function",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
            SymbolKind::Class => "class",
            SymbolKind::ClassField => "field",
            SymbolKind::ClassMethod => "method",
        }
    }
}

/// Visibility of a top-level symbol for cross-file lookup.
///
/// * `Local`  — file-local (no `extern`, no `const`).
/// * `Const`  — `const` declaration; file-local but read-only.
/// * `Extern` — `extern` declaration; visible to other files without `include`.
/// * `Public` — global function/variable not marked `extern` or `static`.
///   Functions default to public; non-`extern` variables are `Local`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    #[default]
    Local,
    Const,
    Extern,
    Public,
}

/// A function parameter — `int x` or `string s = "default"`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Param {
    pub ty: String,
    pub name: String,
    /// Raw default-value expression text, e.g. `"-1"` or `"\"hi\""`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// `ref` modifier — parameter passed by reference.
    /// The XS compiler rejects defaults on ref params.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_ref: bool,
}

/// A single named XS construct visible at the top level of a file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    /// Type string: `int`, `void`, `bool`, `int[]`, etc. Empty for rules.
    pub ty: String,
    /// Function params; empty for non-functions.
    pub params: Vec<Param>,
    /// For class fields and methods, the name of the enclosing class.
    #[serde(default)]
    pub class_owner: Option<String>,
    /// `extern` storage class on a function/variable declaration.
    #[serde(default)]
    pub is_extern: bool,
    /// `mutable` modifier on a function.
    #[serde(default)]
    pub is_mutable: bool,
    /// `static` storage-class specifier.
    #[serde(default)]
    pub is_static: bool,
    /// Forward-only declaration (function header without body).
    #[serde(default)]
    pub is_forward: bool,
    /// Cross-file visibility.
    #[serde(default)]
    pub visibility: Visibility,
    /// Full range of the declaration (for hover).
    pub full_range: Range,
    /// Range of just the identifier (for outline / go-to-selection).
    pub selection_range: Range,
    /// One-line summary for hover and outline (e.g.
    /// `void setDistributionNumbers(int food, int wood, int gold)` or
    /// `int gReservePlan = -1`).
    pub detail: String,
}

/// The full per-file symbol table.
#[derive(Debug, Default, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymbolTable {
    pub symbols: Vec<Symbol>,
}

impl SymbolTable {
    /// Find a symbol by exact name. Returns the LAST match (later
    /// declarations shadow earlier ones — XS doesn't formally define
    /// shadowing, but the convention is last-write-wins).
    pub fn find(&self, name: &str) -> Option<&Symbol> { self.symbols.iter().rev().find(|s| s.name == name) }

    /// All symbols whose name starts with `prefix`.
    pub fn matching(&self, prefix: &str) -> impl Iterator<Item = &Symbol> {
        self.symbols.iter().filter(move |s| s.name.starts_with(prefix))
    }
}

/// Parse `source` into a typed AST and build its symbol table.
pub fn build_symbol_table(source: &str) -> SymbolTable {
    let (cst, tu) = parse_tu(source);
    build_symbol_table_from_tu(&cst, &tu, source)
}

/// Extract user-defined class names from source via a lightweight line scan.
///
/// Walking the typed AST for class definitions can miss classes that follow a
/// class body with unsupported constructs, because parser recovery may resync
/// past the remaining top-level items. A `class <Identifier>` declaration is
/// always a top-level keyword followed by an identifier on the same line, so a
/// regex-free scan is robust and cheap.
pub fn extract_class_names(source: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    for line in source.lines() {
        let after_leading_ws = line.trim_start();
        // Match `class` as a standalone keyword at the start of a logical line.
        if let Some(after_class) = after_leading_ws.strip_prefix("class") {
            if after_class.starts_with(|c: char| c.is_alphanumeric() || c == '_') {
                continue;
            }
            let rest = after_class.trim_start();
            if rest.is_empty() {
                continue;
            }
            let name = rest
                .split(|c: char| !c.is_alphanumeric() && c != '_')
                .next()
                .unwrap_or("");
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
    }
    names
}

/// Build a symbol table that includes local variable declarations inside
/// function and block bodies, in addition to top-level symbols.
pub fn build_full_symbol_table(source: &str) -> SymbolTable {
    let (cst, tu) = parse_tu(source);
    let mut table = build_symbol_table_from_tu(&cst, &tu, source);
    extract_all_local_declarations(&cst, &tu, source, &mut table.symbols);
    table
}

fn parse_tu(source: &str) -> (Cst<'_>, TranslationUnit) {
    let mut diags = Vec::new();
    let cst = Parser::new_with_context(source, &mut diags, TypeTable::with_primitives()).parse(&mut diags);
    let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT).unwrap_or_else(|| TranslationUnit {
        items: Vec::new(),
        span: 0..0,
    });
    (cst, tu)
}

fn build_symbol_table_from_tu(cst: &Cst<'_>, tu: &TranslationUnit, source: &str) -> SymbolTable {
    let mut table = SymbolTable::default();
    for item in &tu.items {
        match item {
            TopLevelItem::RuleDefinition(r) => {
                if let Some(s) = symbol_from_rule(r, source) {
                    table.symbols.push(s);
                }
            }
            TopLevelItem::FunctionDefinition(f) => {
                if let Some(s) = symbol_from_function(f, cst, source) {
                    table.symbols.push(s);
                }
            }
            TopLevelItem::ClassDefinition(c) => {
                if let Some(s) = symbol_from_class(c, source) {
                    table.symbols.push(s);
                }
                for member in &c.members.inner {
                    if let Some(s) = symbol_from_class_member(member, &c.name.node, cst, source) {
                        table.symbols.push(s);
                    }
                }
            }
            TopLevelItem::ForwardDeclaration(f) => {
                if let Some(s) = symbol_from_forward_declaration(f, cst, source) {
                    table.symbols.push(s);
                }
            }
            TopLevelItem::Declaration(d) => {
                if let Some(s) = symbol_from_declaration(d, cst, source) {
                    table.symbols.push(s);
                }
            }
            _ => {}
        }
    }
    table
}

fn symbol_from_rule(rule: &RuleDefinition, source: &str) -> Option<Symbol> {
    let name = rule.name.node.clone();
    Some(Symbol {
        name: name.clone(),
        kind: SymbolKind::Rule,
        ty: String::new(),
        params: Vec::new(),
        class_owner: None,
        is_extern: false,
        is_mutable: false,
        is_static: false,
        is_forward: false,
        visibility: Visibility::Public,
        full_range: span_to_range(source, rule.span.clone()),
        selection_range: span_to_range(source, rule.name.span.clone()),
        detail: format!("rule {}", name),
    })
}

fn symbol_from_function(func: &FunctionDefinition, cst: &Cst<'_>, source: &str) -> Option<Symbol> {
    let name_id = name_from_declarator(&func.declarator.direct)?;
    let name = name_id.node.clone();
    let modifiers = modifiers_from_specs(&func.decl_specs);
    let params = func
        .declarator
        .direct
        .as_function_declarator()
        .and_then(|fd| fd.params.as_ref())
        .map(|p| params_from_list(&p.inner, cst, source))
        .unwrap_or_default();
    let detail = format_function_detail(&format_type(&func.decl_specs), &name_id.node, &params);
    Some(Symbol {
        name,
        kind: SymbolKind::Function,
        ty: format_type(&func.decl_specs),
        params,
        class_owner: None,
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: false,
        visibility: function_visibility(&modifiers),
        full_range: span_to_range(source, func.span.clone()),
        selection_range: span_to_range(source, name_id.span.clone()),
        detail,
    })
}

fn symbol_from_forward_declaration(fwd: &ForwardDeclaration, cst: &Cst<'_>, source: &str) -> Option<Symbol> {
    let name_id = name_from_declarator(&fwd.declarator.direct)?;
    let name = name_id.node.clone();
    let modifiers = modifiers_from_specs(&fwd.decl_specs);
    let params = fwd
        .declarator
        .direct
        .as_function_declarator()
        .and_then(|fd| fd.params.as_ref())
        .map(|p| params_from_list(&p.inner, cst, source))
        .unwrap_or_default();
    let detail = format_function_detail(&format_type(&fwd.decl_specs), &name_id.node, &params);
    Some(Symbol {
        name,
        kind: SymbolKind::Function,
        ty: format_type(&fwd.decl_specs),
        params,
        class_owner: None,
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: true,
        visibility: function_visibility(&modifiers),
        full_range: span_to_range(source, fwd.span.clone()),
        selection_range: span_to_range(source, name_id.span.clone()),
        detail,
    })
}

fn symbol_from_declaration(decl: &Declaration, _cst: &Cst<'_>, source: &str) -> Option<Symbol> {
    // Parity with the legacy parser path: one symbol per declaration,
    // using the first init_declarator. Multi-variable declarations are
    // rare in real XS and changing the count would break the retail
    // parity test.
    let init = decl.init_declarator_list.items.iter().next()?;
    symbol_from_init_declarator(init, decl, source)
}

fn symbol_from_init_declarator(
    init: &InitDeclarator,
    decl: &Declaration,
    source: &str,
) -> Option<Symbol> {
    let name_id = name_from_declarator(&init.declarator.direct)?;
    let name = name_id.node.clone();
    let modifiers = modifiers_from_specs(&decl.decl_specs);
    let visibility = variable_visibility(&modifiers);
    let kind = if modifiers.is_const {
        SymbolKind::Constant
    } else {
        SymbolKind::Variable
    };
    let full_range = span_to_range(source, decl.span.clone());
    let selection_range = span_to_range(source, name_id.span.clone());
    let init_text = source[init.span.clone()].trim().to_string();
    let detail = format!("{} {}", format_type(&decl.decl_specs), init_text);
    Some(Symbol {
        name,
        kind,
        ty: format_type(&decl.decl_specs),
        params: Vec::new(),
        class_owner: None,
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: false,
        visibility,
        full_range,
        selection_range,
        detail,
    })
}

fn symbol_from_class(class: &ClassDefinition, source: &str) -> Option<Symbol> {
    Some(Symbol {
        name: class.name.node.clone(),
        kind: SymbolKind::Class,
        ty: String::new(),
        params: Vec::new(),
        class_owner: None,
        is_extern: false,
        is_mutable: false,
        is_static: false,
        is_forward: false,
        visibility: Visibility::Public,
        full_range: span_to_range(source, class.span.clone()),
        selection_range: span_to_range(source, class.name.span.clone()),
        detail: format!("class {}", class.name.node),
    })
}

fn symbol_from_class_member(
    member: &ClassMember,
    class_name: &str,
    cst: &Cst<'_>,
    source: &str,
) -> Option<Symbol> {
    match member {
        ClassMember::FunctionDefinition(f) => {
            let mut sym = symbol_from_function(f, cst, source)?;
            sym.kind = SymbolKind::ClassMethod;
            sym.class_owner = Some(class_name.to_string());
            sym.visibility = Visibility::Local;
            Some(sym)
        }
        ClassMember::ForwardDeclaration(f) => {
            let mut sym = symbol_from_forward_declaration(f, cst, source)?;
            sym.kind = SymbolKind::ClassMethod;
            sym.class_owner = Some(class_name.to_string());
            sym.visibility = Visibility::Local;
            Some(sym)
        }
        ClassMember::FieldDeclaration(f) => {
            let name_id = name_from_declarator(&f.declarator.direct)?;
            let modifiers = modifiers_from_specs(&f.decl_specs);
            let (kind, params) = if let Some(fd) = f.declarator.direct.as_function_declarator() {
                (
                    SymbolKind::ClassMethod,
                    fd.params
                        .as_ref()
                        .map(|p| params_from_list(&p.inner, cst, source))
                        .unwrap_or_default(),
                )
            } else {
                (SymbolKind::ClassField, Vec::new())
            };
            let name = name_id.node.clone();
            let ty = format_type(&f.decl_specs);
            let detail = if kind == SymbolKind::ClassMethod {
                format_function_detail(&ty, &name, &params)
            } else {
                format!("{} {}", ty, name)
            };
            Some(Symbol {
                name,
                kind,
                ty,
                params,
                class_owner: Some(class_name.to_string()),
                is_extern: modifiers.is_extern,
                is_mutable: modifiers.is_mutable,
                is_static: modifiers.is_static,
                is_forward: false,
                visibility: Visibility::Local,
                full_range: span_to_range(source, f.span.clone()),
                selection_range: span_to_range(source, name_id.span.clone()),
                detail,
            })
        }
    }
}

// ------------------------------------------------------------------
// Local (block-level) declarations
// ------------------------------------------------------------------

fn extract_all_local_declarations(
    cst: &Cst<'_>,
    tu: &TranslationUnit,
    source: &str,
    out: &mut Vec<Symbol>,
) {
    for item in &tu.items {
        match item {
            TopLevelItem::FunctionDefinition(f) => {
                extract_locals_from_block(&f.body.inner, cst, source, out);
            }
            TopLevelItem::ClassDefinition(c) => {
                for member in &c.members.inner {
                    if let ClassMember::FunctionDefinition(f) = member {
                        extract_locals_from_block(&f.body.inner, cst, source, out);
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_locals_from_block(
    items: &[BlockItem],
    cst: &Cst<'_>,
    source: &str,
    out: &mut Vec<Symbol>,
) {
    for item in items {
        match item {
            BlockItem::Declaration(d) => {
                if let Some(s) = local_symbol_from_declaration(d, cst, source) {
                    out.push(s);
                }
            }
            BlockItem::FunctionDefinition(f) => {
                extract_locals_from_block(&f.body.inner, cst, source, out);
            }
            BlockItem::Statement(s) => {
                visit_statement_for_locals(s, cst, source, out);
            }
            _ => {}
        }
    }
}

fn visit_statement_for_locals(
    stmt: &Statement,
    cst: &Cst<'_>,
    source: &str,
    out: &mut Vec<Symbol>,
) {
    match stmt {
        Statement::Compound(comp) => {
            extract_locals_from_block(&comp.items.inner, cst, source, out);
        }
        Statement::If(ifst) => {
            visit_statement_for_locals(&ifst.then, cst, source, out);
            if let Some(else_) = &ifst.else_ {
                visit_statement_for_locals(else_, cst, source, out);
            }
        }
        Statement::While(while_) => {
            visit_statement_for_locals(&while_.body, cst, source, out);
        }
        Statement::For(for_) => {
            if let ForInit::Declaration(d) = &for_.init {
                if let Some(s) = local_symbol_from_declaration(d, cst, source) {
                    out.push(s);
                }
            }
            visit_statement_for_locals(&for_.body, cst, source, out);
        }
        Statement::Switch(switch) => {
            for case in &switch.cases.inner {
                extract_locals_from_block(&case.body.items.inner, cst, source, out);
            }
        }
        _ => {}
    }
}

fn local_symbol_from_declaration(
    decl: &Declaration,
    _cst: &Cst<'_>,
    source: &str,
) -> Option<Symbol> {
    let init = decl.init_declarator_list.items.iter().next()?;
    let name_id = name_from_declarator(&init.declarator.direct)?;
    let name = name_id.node.clone();
    let modifiers = modifiers_from_specs(&decl.decl_specs);
    let init_text = source[init.span.clone()].trim().to_string();
    let detail = format!("{} {}", format_type(&decl.decl_specs), init_text);
    Some(Symbol {
        name,
        kind: SymbolKind::Variable,
        ty: format_type(&decl.decl_specs),
        params: Vec::new(),
        class_owner: None,
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: false,
        visibility: if modifiers.is_extern {
            Visibility::Extern
        } else {
            Visibility::Local
        },
        full_range: span_to_range(source, decl.span.clone()),
        selection_range: span_to_range(source, name_id.span.clone()),
        detail,
    })
}

// ------------------------------------------------------------------
// Rule registration extraction
// ------------------------------------------------------------------

/// Functions that register a rule by name at runtime.
const RULE_REGISTRATION_FUNCTIONS: &[&str] = &[
    "xsEnableRule",
    "xsDisableRule",
    "xsSetRuleMinInterval",
    "xsSetRuleMaxInterval",
    "xsRuleIgnoreIntervalOnce",
    "trDelayedRuleActivation",
    "trRuleAdd",
    "trRuleAddActive",
];

/// Extract the names of rules that are registered through runtime helpers.
///
/// Rules in XS are first-class callbacks registered with helpers like
/// `xsEnableRule("myRule")` or `trRuleAdd("myRule")`. They may not have a
/// matching `rule myRule {}` definition in the same translation unit, so
/// capturing the registration call lets semantic analysis resolve calls to
/// those rules.
pub fn extract_rule_registrations(source: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    let (cst, tu) = parse_tu(source);
    collect_rule_registrations_in_tu(&cst, &tu, source, &mut out);
    out
}

fn collect_rule_registrations_in_tu(
    cst: &Cst<'_>,
    tu: &TranslationUnit,
    _source: &str,
    out: &mut HashSet<String>,
) {
    for item in &tu.items {
        match item {
            TopLevelItem::FunctionDefinition(f) => {
                collect_rule_registrations_in_block(&cst, &f.body.inner, out);
            }
            TopLevelItem::ClassDefinition(c) => {
                for member in &c.members.inner {
                    if let ClassMember::FunctionDefinition(f) = member {
                        collect_rule_registrations_in_block(&cst, &f.body.inner, out);
                    }
                }
            }
            TopLevelItem::Declaration(d) => {
                for init in &d.init_declarator_list.items {
                    if let Some(expr) = init.expr(cst) {
                        collect_rule_registrations_in_expr(&cst, &expr, out);
                    }
                }
            }
            _ => {}
        }
    }
}

fn collect_rule_registrations_in_block(
    cst: &Cst<'_>,
    items: &[BlockItem],
    out: &mut HashSet<String>,
) {
    for item in items {
        match item {
            BlockItem::Declaration(d) => {
                for init in &d.init_declarator_list.items {
                    if let Some(expr) = init.expr(cst) {
                        collect_rule_registrations_in_expr(cst, &expr, out);
                    }
                }
            }
            BlockItem::FunctionDefinition(f) => {
                collect_rule_registrations_in_block(cst, &f.body.inner, out);
            }
            BlockItem::Statement(s) => {
                collect_rule_registrations_in_statement(cst, s, out);
            }
            _ => {}
        }
    }
}

fn collect_rule_registrations_in_statement(
    cst: &Cst<'_>,
    stmt: &Statement,
    out: &mut HashSet<String>,
) {
    match stmt {
        Statement::Compound(comp) => {
            collect_rule_registrations_in_block(cst, &comp.items.inner, out);
        }
        Statement::If(ifst) => {
            collect_rule_registrations_in_statement(cst, &ifst.then, out);
            if let Some(else_) = &ifst.else_ {
                collect_rule_registrations_in_statement(cst, else_, out);
            }
        }
        Statement::While(while_) => {
            collect_rule_registrations_in_statement(cst, &while_.body, out);
        }
        Statement::For(for_) => {
            if let ForInit::Declaration(d) = &for_.init {
                for init in &d.init_declarator_list.items {
                    if let Some(expr) = init.expr(cst) {
                        collect_rule_registrations_in_expr(cst, &expr, out);
                    }
                }
            }
            collect_rule_registrations_in_statement(cst, &for_.body, out);
        }
        Statement::Switch(switch) => {
            for case in &switch.cases.inner {
                collect_rule_registrations_in_block(cst, &case.body.items.inner, out);
            }
        }
        Statement::Expression(es) => {
            if let Some(e) = es.expr.as_ref().and_then(|u| Expr::from_cst(cst, u.0)) {
                collect_rule_registrations_in_expr(cst, &e, out);
            }
        }
        Statement::Return(ret) => {
            if let Some(v) = &ret.value {
                if let Some(e) = Expr::from_cst(cst, v.0) {
                    collect_rule_registrations_in_expr(cst, &e, out);
                }
            }
        }
        _ => {}
    }
}

fn collect_rule_registrations_in_expr(cst: &Cst<'_>, expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Postfix(pfe) => {
            if let PostfixInner::Call(call) = &pfe.inner {
                try_register_rule(cst, pfe, call, out);
                collect_rule_registrations_in_expr(cst, &pfe.target, out);
                if let Some(args) = &call.args {
                    for (arg, _) in &args.items.items {
                        if let Some(e) = arg.expr(cst) {
                            collect_rule_registrations_in_expr(cst, &e, out);
                        }
                    }
                }
            } else {
                collect_rule_registrations_in_expr(cst, &pfe.target, out);
            }
        }
        Expr::Unary(u) => collect_rule_registrations_in_expr(cst, &u.operand, out),
        Expr::Binary(b) => {
            collect_rule_registrations_in_expr(cst, &b.lhs, out);
            collect_rule_registrations_in_expr(cst, &b.rhs, out);
        }
        Expr::Conditional(c) => {
            collect_rule_registrations_in_expr(cst, &c.cond, out);
            if let Some(t) = &c.then {
                collect_rule_registrations_in_expr(cst, t, out);
            }
            collect_rule_registrations_in_expr(cst, &c.else_, out);
        }
        Expr::Assignment(a) => {
            collect_rule_registrations_in_expr(cst, &a.lhs, out);
            collect_rule_registrations_in_expr(cst, &a.rhs, out);
        }
        Expr::Comma(c) => {
            for e in &c.exprs {
                collect_rule_registrations_in_expr(cst, e, out);
            }
        }
        Expr::Paren(p) => collect_rule_registrations_in_expr(cst, &p.inner, out),
        _ => {}
    }
}

fn try_register_rule(
    cst: &Cst<'_>,
    pfe: &PostfixExpr,
    call: &CallExpr,
    out: &mut HashSet<String>,
) {
    if let Expr::Identifier(id) = pfe.target.as_ref() {
        if !RULE_REGISTRATION_FUNCTIONS.contains(&id.name.node.as_str()) {
            return;
        }
        if let Some((arg, _)) = call.args.as_ref().and_then(|a| a.items.items.iter().next()) {
            if let Some(Expr::StringLiteral(StringLiteral { value, .. })) = arg.expr(cst) {
                let name = value.trim_matches('"').trim_matches('\'').to_string();
                if !name.is_empty() {
                    out.insert(name);
                }
            }
        }
    }
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

/// Modifiers parsed from a declaration/function header.
#[derive(Debug, Default)]
struct Modifiers {
    is_extern: bool,
    is_static: bool,
    is_mutable: bool,
    is_const: bool,
}

fn modifiers_from_specs(specs: &DeclarationSpecifiers) -> Modifiers {
    let mut m = Modifiers::default();
    for (s, _) in &specs.storage {
        match s {
            StorageClassSpecifier::Extern => m.is_extern = true,
            StorageClassSpecifier::Static => m.is_static = true,
            StorageClassSpecifier::Mutable => m.is_mutable = true,
        }
    }
    m.is_const = specs.type_quals.iter().any(|(q, _)| *q == TypeQualifier::Const);
    m
}

fn function_visibility(m: &Modifiers) -> Visibility {
    if m.is_extern {
        Visibility::Extern
    } else if m.is_static {
        Visibility::Local
    } else {
        Visibility::Public
    }
}

fn variable_visibility(m: &Modifiers) -> Visibility {
    if m.is_extern {
        Visibility::Extern
    } else if m.is_const {
        Visibility::Const
    } else {
        Visibility::Local
    }
}

fn format_type(specs: &DeclarationSpecifiers) -> String {
    let mut s = format_primitive_or_class(&specs.ty.ty);
    if specs.ty.is_array {
        s.push_str("[]");
    }
    s
}

fn format_primitive_or_class(ty: &Type) -> String {
    match ty {
        Type::Void => "void".to_string(),
        Type::Int => "int".to_string(),
        Type::Bool => "bool".to_string(),
        Type::Float => "float".to_string(),
        Type::String => "string".to_string(),
        Type::Vector => "vector".to_string(),
        Type::Class(id) => id.node.clone(),
    }
}

fn name_from_declarator(direct: &DirectDeclarator) -> Option<&Identifier> {
    match direct {
        DirectDeclarator::IdentDeclarator(id) => Some(&id.name),
        DirectDeclarator::FunctionDeclarator(fd) => Some(&fd.base),
        DirectDeclarator::ParenDeclarator(pd) => name_from_declarator(&pd.inner.direct),
    }
}

fn format_function_detail(ty: &str, name: &str, params: &[Param]) -> String {
    if params.is_empty() {
        format!("{} {}()", ty, name)
    } else {
        let rendered: Vec<String> = params
            .iter()
            .map(|p| match &p.default {
                Some(d) => format!("{} {} = {}", p.ty, p.name, d),
                None => format!("{} {}", p.ty, p.name),
            })
            .collect();
        format!("{} {}({})", ty, name, rendered.join(", "))
    }
}

fn params_from_list(list: &ParameterList, cst: &Cst<'_>, source: &str) -> Vec<Param> {
    list.items.iter().map(|param| param_from_declaration(param, cst, source)).collect()
}

fn param_from_declaration(param: &ParameterDeclaration, cst: &Cst<'_>, source: &str) -> Param {
    match &param.inner {
        ParameterInner::RegularParam(RegularParam { name, default, .. }) => Param {
            ty: format_type(&param.decl_specs),
            name: name.as_ref().map_or_else(String::new, |n| n.node.clone()),
            default: default_expr_text(default.as_ref(), cst, source),
            is_ref: param.decl_specs.is_ref(),
        },
        ParameterInner::FunctionPointerParam(fp) => Param {
            ty: format_function_pointer_type(&fp.fn_type, cst, source),
            name: fp.name.node.clone(),
            default: default_expr_text(fp.default.as_ref(), cst, source),
            is_ref: false,
        },
    }
}

fn format_function_pointer_type(
    fn_type: &(xs_parser::ast::TypeSpecifier, xs_parser::ast::Parenthesized<ParameterList>),
    cst: &Cst<'_>,
    source: &str,
) -> String {
    let ret = format_type(&DeclarationSpecifiers {
        storage: vec![],
        ty: fn_type.0.clone(),
        type_quals: vec![],
        span: fn_type.0.span.clone(),
    });
    let params = params_from_list(&fn_type.1.inner, cst, source);
    let rendered: Vec<String> = params
        .iter()
        .map(|p| match &p.default {
            Some(d) => format!("{} {} = {}", p.ty, p.name, d),
            None => format!("{} {}", p.ty, p.name),
        })
        .collect();
    format!("{}({})", ret, rendered.join(", "))
}

fn default_expr_text(
    default: Option<&(Span, xs_parser::ast::UnparsedExpr)>,
    cst: &Cst<'_>,
    source: &str,
) -> Option<String> {
    let (_, unparsed) = default?;
    let span = cst.span(unparsed.0);
    Some(source[span].to_string())
}

// ------------------------------------------------------------------
// Trait extensions for cleaner typed-AST access
// ------------------------------------------------------------------

trait DirectDeclaratorExt {
    fn as_function_declarator(&self) -> Option<&FunctionDeclarator>;
}

impl DirectDeclaratorExt for DirectDeclarator {
    fn as_function_declarator(&self) -> Option<&FunctionDeclarator> {
        match self {
            DirectDeclarator::FunctionDeclarator(fd) => Some(fd),
            _ => None,
        }
    }
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn table_for(source: &str) -> SymbolTable { build_symbol_table(source) }

    #[test]
    fn extracts_extern_variable() {
        let src = "extern int gReservePlan = -1;\n";
        let t = table_for(src);
        assert_eq!(t.symbols.len(), 1);
        let s = &t.symbols[0];
        assert_eq!(s.name, "gReservePlan");
        assert_eq!(s.kind, SymbolKind::Variable);
        assert_eq!(s.ty, "int");
        assert!(s.is_extern);
        assert_eq!(s.visibility, Visibility::Extern);
        assert!(s.detail.contains("gReservePlan"));
    }

    #[test]
    fn extracts_const_as_constant() {
        let src = "const bool cAllowFoo = true;\n";
        let t = table_for(src);
        let s = &t.symbols[0];
        assert_eq!(s.name, "cAllowFoo");
        assert_eq!(s.kind, SymbolKind::Constant);
        assert_eq!(s.ty, "bool");
        assert_eq!(s.visibility, Visibility::Const);
    }

    #[test]
    fn extracts_function_with_params() {
        let src = "mutable void setFoo(int x, string s) {}\n";
        let t = table_for(src);
        let s = t.find("setFoo").expect("setFoo symbol");
        assert_eq!(s.kind, SymbolKind::Function);
        assert_eq!(s.ty, "void");
        assert!(s.is_mutable);
        assert_eq!(s.visibility, Visibility::Public);
        assert_eq!(s.params.len(), 2);
        assert_eq!(s.params[0].ty, "int");
        assert_eq!(s.params[0].name, "x");
        assert!(s.detail.contains("int x"));
    }

    #[test]
    fn extracts_forward_declaration() {
        let src = "void bar(int x = -1);\n";
        let t = table_for(src);
        assert_eq!(t.symbols.len(), 1);
        let s = &t.symbols[0];
        assert_eq!(s.name, "bar");
        assert_eq!(s.kind, SymbolKind::Function);
        assert!(s.is_forward);
        assert_eq!(s.params.len(), 1);
        assert_eq!(s.params[0].default.as_deref(), Some("-1"));
    }

    #[test]
    fn extracts_extern_function() {
        let src = "extern void shared(int a) {}\n";
        let t = table_for(src);
        let s = t.find("shared").expect("shared symbol");
        assert!(s.is_extern);
        assert_eq!(s.visibility, Visibility::Extern);
    }

    #[test]
    fn extracts_rule_with_modifiers() {
        let src = "rule cleanupLingering\nminInterval 15\nactive\n{}\n";
        let t = table_for(src);
        let s = t.find("cleanupLingering").expect("rule symbol");
        assert_eq!(s.kind, SymbolKind::Rule);
        assert!(s.ty.is_empty());
    }

    #[test]
    fn find_takes_last_match() {
        let src = "int x = 1;\nint x = 2;\n";
        let t = table_for(src);
        let s = t.find("x").unwrap();
        // Last match wins
        assert!(s.detail.contains("= 2"));
    }

    #[test]
    fn empty_source_yields_empty_table() {
        let t = table_for("");
        assert!(t.symbols.is_empty());
    }

    #[test]
    fn extracts_simple_default_parameter_still() {
        let src = "void bar(int x = -1) { }\n";
        let t = table_for(src);
        let s = t.find("bar").expect("bar symbol");
        assert_eq!(s.kind, SymbolKind::Function);
        assert_eq!(s.params.len(), 1);
        assert_eq!(s.params[0].ty, "int");
        assert_eq!(s.params[0].default.as_deref(), Some("-1"));
    }

    #[test]
    fn test_symbol_kind_rule_variant_exists() {
        // Verify the enum variant exists and is distinct from Function.
        assert_ne!(SymbolKind::Rule, SymbolKind::Function);
    }

    #[test]
    fn test_extract_rule_from_xs_enable_rule() {
        let src = r#"void init() { xsEnableRule("updateBreakdown"); }"#;
        let regs = extract_rule_registrations(src);
        assert!(regs.contains("updateBreakdown"));
    }

    #[test]
    fn test_extract_rule_from_tr_rule_add() {
        let src = r#"void init() { trRuleAdd("updateBreakdown", 1); }"#;
        let regs = extract_rule_registrations(src);
        assert!(regs.contains("updateBreakdown"));
    }

    #[test]
    fn test_extract_rule_from_tr_rule_add_active() {
        let src = r#"void init() { trRuleAddActive("updateBreakdown", 1); }"#;
        let regs = extract_rule_registrations(src);
        assert!(regs.contains("updateBreakdown"));
    }

    #[test]
    fn test_extract_multiple_rules() {
        let src = r#"
            void init() {
                xsEnableRule("ruleA");
                xsDisableRule("ruleB");
                trRuleAdd("ruleC", 5);
            }
        "#;
        let regs = extract_rule_registrations(src);
        assert_eq!(regs.len(), 3);
        assert!(regs.contains("ruleA"));
        assert!(regs.contains("ruleB"));
        assert!(regs.contains("ruleC"));
    }

    #[test]
    fn test_extract_no_rules_from_empty_file() {
        let regs = extract_rule_registrations("");
        assert!(regs.is_empty());
    }

    #[test]
    fn from_cst_correctly_extracts_rule_class_function() {
        let src = r#"
            rule myRule active { }
            class MyClass { int field = 0; void method() {} }
            void globalFn(int x = -1) {}
        "#;
        let t = table_for(src);
        assert!(t.find("myRule").is_some());
        assert!(t.find("MyClass").is_some());
        assert!(t.find("field").is_some());
        assert!(t.find("method").is_some());
        assert!(t.find("globalFn").is_some());
    }

    #[test]
    fn build_symbol_table_yields_symbol_entry_per_item() {
        let src = "extern int a = 1;\nvoid f() {}\nclass C { int x = 0; }\n";
        let t = table_for(src);
        let kinds: Vec<_> = t.symbols.iter().map(|s| s.kind).collect();
        assert!(kinds.contains(&SymbolKind::Variable));
        assert!(kinds.contains(&SymbolKind::Function));
        assert!(kinds.contains(&SymbolKind::Class));
        assert!(kinds.contains(&SymbolKind::ClassField));
    }
}
