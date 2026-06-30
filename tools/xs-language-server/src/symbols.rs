//! Per-file symbol table: extracts rule/function/variable declarations from
//! the AST so we can resolve hover, go-to-definition, and provide a
//! document outline (week 3 of the post-spike roadmap).
//!
//! For week 3 we build a flat per-file table — no cross-file resolution.
//! Cross-file resolution lands in week 4 (or later) when we have proper
//! workspace indexing that handles `include "foo.xs"`.
//!
//! Extraction walks `translation_unit` children and pulls names out of:
//!   * `rule_definition`     -> Rule
//!   * `function_definition` -> Function
//!   * `declaration`         -> Variable or Constant (depending on `const`
//!                              type-qualifier)
//!
//! Note: `#define` is NOT supported (XS doesn't use it; only `#if`/`#ifdef`
//! appear in real code, and those files are already failing to parse
//! anyway). When preproc support is added, add a `Macro` variant here.

use std::collections::HashSet;

use tower_lsp::lsp_types::{Position, Range};

/// What kind of XS construct a symbol represents.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SymbolKind {
    Rule,
    Function,
    Variable,
    Constant,
}

impl SymbolKind {
    /// One-line detail string for hover / outline (e.g. "int gReservePlan").
    pub fn label(&self) -> &'static str {
        match self {
            SymbolKind::Rule => "rule",
            SymbolKind::Function => "function",
            SymbolKind::Variable => "variable",
            SymbolKind::Constant => "constant",
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Local,
    Const,
    Extern,
    Public,
}

impl Default for Visibility {
    fn default() -> Self {
        Visibility::Local
    }
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
    pub fn find(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().rev().find(|s| s.name == name)
    }

    /// All symbols whose name starts with `prefix`.
    pub fn matching(&self, prefix: &str) -> impl Iterator<Item = &Symbol> {
        self.symbols
            .iter()
            .filter(move |s| s.name.starts_with(prefix))
    }
}

/// Build a symbol table for a parsed XS file.
pub fn build_symbol_table(tree: &tree_sitter::Tree, source: &str) -> SymbolTable {
    let mut table = SymbolTable::default();
    let root = tree.root_node();
    if root.kind() != "translation_unit" {
        return table;
    }
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "rule_definition" => extract_rule(child, source, &mut table.symbols),
            "function_definition" => extract_function(child, source, &mut table.symbols),
            "declaration" => extract_declaration(child, source, &mut table.symbols),
            // The XS grammar currently parses function forward declarations
            // (`void bar(int x = -1);`) as an ERROR node containing the type,
            // identifier and parameter list. Extract them so callers get
            // forward-declaration symbols anyway.  ERROR nodes that contain a
            // body are recovered as full function definitions.
            "ERROR" => {
                extract_error_forward_declaration(child, source, &mut table.symbols);
                extract_error_function_definition(child, source, &mut table.symbols);
            }
            _ => {}
        }
    }
    table
}

/// Build a symbol table that includes local variable declarations inside
/// function and block bodies, in addition to top-level symbols.
pub fn build_full_symbol_table(tree: &tree_sitter::Tree, source: &str) -> SymbolTable {
    let mut table = build_symbol_table(tree, source);
    extract_local_declarations(tree.root_node(), source, &mut table.symbols);
    table
}

fn extract_local_declarations(
    node: tree_sitter::Node<'_>,
    source: &str,
    out: &mut Vec<Symbol>,
) {
    if node.kind() == "compound_statement" {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "declaration" {
                extract_local_declaration(child, source, out);
            }
        }
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        extract_local_declarations(child, source, out);
    }
}

fn extract_local_declaration(node: tree_sitter::Node<'_>, source: &str, out: &mut Vec<Symbol>) {
    let init = match find_named_child(node, "init_declarator") {
        Some(n) => n,
        None => return,
    };
    let name_node = match find_named_child(init, "identifier") {
        Some(n) => n,
        None => return,
    };

    let name = node_text(name_node, source).to_string();
    let ty = find_named_child(node, "primitive_type")
        .or_else(|| find_named_child(node, "array_type"))
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();
    let modifiers = extract_modifiers(node);
    let visibility = if modifiers.is_extern {
        Visibility::Extern
    } else {
        Visibility::Local
    };
    let full_range = node_range(node);
    let selection_range = node_range(name_node);
    let init_text = node_text(init, source).to_string();
    let detail = format!("{} {}", ty, init_text);

    out.push(Symbol {
        name,
        kind: SymbolKind::Variable,
        ty,
        params: Vec::new(),
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: false,
        visibility,
        full_range,
        selection_range,
        detail,
    });
}

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
pub fn extract_rule_registrations(tree: &tree_sitter::Tree, source: &str) -> HashSet<String> {
    let mut out = HashSet::new();
    collect_rule_registrations(tree.root_node(), source, &mut out);
    out
}

fn collect_rule_registrations(
    node: tree_sitter::Node<'_>,
    source: &str,
    out: &mut HashSet<String>,
) {
    if node.kind() == "call_expression" {
        try_register_rule(node, source, out);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_rule_registrations(child, source, out);
    }
}

fn try_register_rule(call: tree_sitter::Node<'_>, source: &str, out: &mut HashSet<String>) {
    let Some(callee_node) = find_named_child(call, "identifier") else {
        return;
    };
    let callee = node_text(callee_node, source);
    if !RULE_REGISTRATION_FUNCTIONS.contains(&callee) {
        return;
    }

    let Some(arg_list) = find_named_child(call, "argument_list") else {
        return;
    };
    let mut args_cursor = arg_list.walk();
    let first_arg = arg_list
        .named_children(&mut args_cursor)
        .find(|c| c.kind() != "comment");

    if let Some(arg) = first_arg {
        if arg.kind() == "string_literal" {
            let text = node_text(arg, source);
            // Strip surrounding quotes; XS only uses double quotes for
            // string literals, but be defensive against single quotes.
            let name = text.trim_matches('"').trim_matches('\'').to_string();
            if !name.is_empty() {
                out.insert(name);
            }
        }
    }
}

fn extract_rule(node: tree_sitter::Node<'_>, _source: &str, out: &mut Vec<Symbol>) {
    let name_node = match find_named_child(node, "identifier") {
        Some(n) => n,
        None => return,
    };
    let name = node_text(name_node, _source).to_string();
    let full_range = node_range(node);
    let selection_range = node_range(name_node);
    out.push(Symbol {
        name,
        kind: SymbolKind::Rule,
        ty: String::new(),
        params: Vec::new(),
        is_extern: false,
        is_mutable: false,
        is_static: false,
        is_forward: false,
        visibility: Visibility::Public,
        full_range,
        selection_range,
        detail: format!("rule {}", node_text(name_node, _source)),
    });
}

fn extract_function(node: tree_sitter::Node<'_>, source: &str, out: &mut Vec<Symbol>) {
    // The function name is the `identifier` child of `function_definition`.
    let name_node = match find_named_child(node, "identifier") {
        Some(n) => n,
        None => return,
    };
    let name = node_text(name_node, source).to_string();

    // The return type is the `primitive_type` or `array_type` child.
    let ty = find_named_child(node, "primitive_type")
        .or_else(|| find_named_child(node, "array_type"))
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();

    // Storage-class / qualifier modifiers on the declaration.
    let modifiers = extract_modifiers(node);

    // Parameters from the `parameter_list` child.
    let params = find_named_child(node, "parameter_list")
        .map(|n| extract_params(n, source))
        .unwrap_or_default();

    let full_range = node_range(node);
    let selection_range = node_range(name_node);
    let detail = format_function_detail(&ty, &name, &params);

    out.push(Symbol {
        name,
        kind: SymbolKind::Function,
        ty,
        params,
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: false,
        visibility: function_visibility(&modifiers),
        full_range,
        selection_range,
        detail,
    });
}

fn extract_declaration(node: tree_sitter::Node<'_>, source: &str, out: &mut Vec<Symbol>) {
    // Forward function declaration: `void bar();` parses as a `declaration`
    // whose declarator is a `function_declarator` with no body.
    if let Some(declarator) = find_named_child(node, "function_declarator") {
        extract_forward_declaration(node, declarator, source, out);
        return;
    }

    // The declared name lives inside `init_declarator` (since XS doesn't have
    // bare `declarator`s — every variable is initialized at the point of
    // declaration).
    let init = match find_named_child(node, "init_declarator") {
        Some(n) => n,
        None => return,
    };
    let name_node = match find_named_child(init, "identifier") {
        Some(n) => n,
        None => return,
    };
    let name = node_text(name_node, source).to_string();

    let ty = find_named_child(node, "primitive_type")
        .or_else(|| find_named_child(node, "array_type"))
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();

    let modifiers = extract_modifiers(node);
    let visibility = variable_visibility(&modifiers);

    let full_range = node_range(node);
    let selection_range = node_range(name_node);
    // Preserve the RHS for the detail line so the user can see `int x = 5`
    // not just `int x`.
    let init_text = node_text(init, source).to_string();
    let detail = format!("{} {}", ty, init_text);

    out.push(Symbol {
        name,
        kind: if modifiers.is_const {
            SymbolKind::Constant
        } else {
            SymbolKind::Variable
        },
        ty,
        params: Vec::new(),
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: false,
        visibility,
        full_range,
        selection_range,
        detail,
    });
}

fn extract_forward_declaration(
    decl: tree_sitter::Node<'_>,
    declarator: tree_sitter::Node<'_>,
    source: &str,
    out: &mut Vec<Symbol>,
) {
    let name_node = match find_named_child(declarator, "identifier") {
        Some(n) => n,
        // Nested function pointers are not valid XS; ignore them.
        None => return,
    };
    let name = node_text(name_node, source).to_string();

    let ty = find_named_child(decl, "primitive_type")
        .or_else(|| find_named_child(decl, "array_type"))
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();

    let modifiers = extract_modifiers(decl);
    let params = find_named_child(declarator, "parameter_list")
        .map(|n| extract_params(n, source))
        .unwrap_or_default();

    let full_range = node_range(decl);
    let selection_range = node_range(name_node);
    let detail = format_function_detail(&ty, &name, &params);

    out.push(Symbol {
        name,
        kind: SymbolKind::Function,
        ty,
        params,
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: true,
        visibility: function_visibility(&modifiers),
        full_range,
        selection_range,
        detail,
    });
}

/// Try to recover a full function definition from a grammar ERROR node that
/// contains a body. This is a safety net for lambda variants or other
/// function-header syntax that the main `function_definition` rule does not
/// yet accept.
fn extract_error_function_definition(
    node: tree_sitter::Node<'_>,
    source: &str,
    out: &mut Vec<Symbol>,
) {
    if node.kind() != "ERROR" {
        return;
    }
    if find_named_child(node, "compound_statement").is_none() {
        return;
    }
    let ty = find_named_child(node, "primitive_type")
        .or_else(|| find_named_child(node, "array_type"))
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();
    let name_node = match find_named_child(node, "identifier") {
        Some(n) => n,
        None => return,
    };
    let params = match find_named_child(node, "parameter_list") {
        Some(n) => extract_params(n, source),
        None => return,
    };

    let name = node_text(name_node, source).to_string();
    let modifiers = extract_modifiers(node);
    let full_range = node_range(node);
    let selection_range = node_range(name_node);
    let detail = format_function_detail(&ty, &name, &params);
    out.push(Symbol {
        name,
        kind: SymbolKind::Function,
        ty,
        params,
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: false,
        visibility: function_visibility(&modifiers),
        full_range,
        selection_range,
        detail,
    });
}

/// Try to recover a forward function declaration from a grammar ERROR node.
///
/// The current tree-sitter XS grammar does not have a dedicated rule for
/// function declarations without bodies, so signatures like
/// `void bar(int x = -1);` surface as an `ERROR` node containing the type,
/// identifier and parameter list. We intentionally keep the condition narrow:
/// it must have a type, identifier and parameters, and no body.
fn extract_error_forward_declaration(
    node: tree_sitter::Node<'_>,
    source: &str,
    out: &mut Vec<Symbol>,
) {
    if node.kind() != "ERROR" {
        return;
    }
    // If the ERROR has a function body, it's a malformed definition, not a
    // forward declaration.
    if find_named_child(node, "compound_statement").is_some() {
        return;
    }
    let ty = find_named_child(node, "primitive_type")
        .or_else(|| find_named_child(node, "array_type"))
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();
    let name_node = match find_named_child(node, "identifier") {
        Some(n) => n,
        None => return,
    };
    let params = match find_named_child(node, "parameter_list") {
        Some(n) => extract_params(n, source),
        None => return,
    };

    let name = node_text(name_node, source).to_string();
    let modifiers = extract_modifiers(node);
    let full_range = node_range(node);
    let selection_range = node_range(name_node);
    let detail = format_function_detail(&ty, &name, &params);

    out.push(Symbol {
        name,
        kind: SymbolKind::Function,
        ty,
        params,
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_static: modifiers.is_static,
        is_forward: true,
        visibility: function_visibility(&modifiers),
        full_range,
        selection_range,
        detail,
    });
}

/// Modifiers parsed from a declaration/function header.
#[derive(Debug, Default)]
struct Modifiers {
    is_extern: bool,
    is_static: bool,
    is_mutable: bool,
    is_const: bool,
}

fn extract_modifiers(node: tree_sitter::Node<'_>) -> Modifiers {
    let mut m = Modifiers::default();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "storage_class_specifier" && child.kind() != "type_qualifier" {
            continue;
        }
        let mut sub = child.walk();
        for token in child.children(&mut sub) {
            match token.kind() {
                "extern" => m.is_extern = true,
                "static" => m.is_static = true,
                "mutable" => m.is_mutable = true,
                "const" => m.is_const = true,
                _ => {}
            }
        }
    }
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

fn extract_params(node: tree_sitter::Node<'_>, source: &str) -> Vec<Param> {
    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "parameter_declaration" {
            continue;
        }
        let ty = if let Some(fp) = find_named_child(child, "function_pointer_type") {
            format_function_pointer_type(fp, source)
        } else {
            find_named_child(child, "primitive_type")
                .or_else(|| find_named_child(child, "array_type"))
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default()
        };
        let name = find_named_child(child, "identifier")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_default();
        let default = extract_param_default(child, source);
        let is_ref = has_ref_qualifier(child);
        params.push(Param {
            ty,
            name,
            default,
            is_ref,
        });
    }
    params
}

/// True if the parameter declaration carries a `ref` type qualifier.
fn has_ref_qualifier(node: tree_sitter::Node<'_>) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "type_qualifier" {
            let mut inner = child.walk();
            for token in child.children(&mut inner) {
                if token.kind() == "ref" {
                    return true;
                }
            }
        }
    }
    false
}

fn format_function_pointer_type(node: tree_sitter::Node<'_>, source: &str) -> String {
    let ret = node
        .child_by_field_name("return")
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();
    let params = node
        .child_by_field_name("parameters")
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_else(|| "()".to_string());
    format!("{}{}", ret, params)
}

/// Extract the raw default-value expression following `=` in a parameter
/// declaration, if any.
fn extract_param_default(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    let mut saw_eq = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "=" {
            saw_eq = true;
        } else if saw_eq {
            return Some(node_text(child, source).trim().to_string());
        }
    }
    None
}

// --- node helpers ---

fn node_range(node: tree_sitter::Node<'_>) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range::new(
        Position::new(start.row as u32, start.column as u32),
        Position::new(end.row as u32, end.column as u32),
    )
}

fn node_text<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

/// First direct named child of `node` whose `kind()` matches `kind`.
fn find_named_child<'a>(node: tree_sitter::Node<'a>, kind: &str) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|c| c.kind() == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn table_for(source: &str) -> SymbolTable {
        let tree = parser::parse(source).expect("parse");
        build_symbol_table(&tree, source)
    }

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
    fn extracts_function_pointer_parameter_type_and_lambda_default() {
        let src = "void foo(int x = -1, void(int) cb = [](int id = -1) {}) { }\n";
        let t = table_for(src);
        let s = t.find("foo").expect("foo symbol");
        assert_eq!(s.kind, SymbolKind::Function);
        assert_eq!(s.ty, "void");
        assert_eq!(s.params.len(), 2);
        assert_eq!(s.params[0].name, "x");
        assert_eq!(s.params[0].default.as_deref(), Some("-1"));
        assert_eq!(s.params[1].ty, "void(int)");
        assert_eq!(s.params[1].name, "cb");
        assert_eq!(s.params[1].default.as_deref(), Some("[](int id = -1) {}"));
        assert!(s.detail.contains("void(int) cb = [](int id = -1) {}"));
    }

    #[test]
    fn extracts_lambda_default_with_return_type() {
        let src = "void boConditionalWait(int planID = -1, bool() condition = []() -> bool { return(true); }) { }\n";
        let t = table_for(src);
        let s = t
            .find("boConditionalWait")
            .expect("boConditionalWait symbol");
        assert_eq!(s.params.len(), 2);
        assert_eq!(s.params[1].ty, "bool()");
        assert_eq!(
            s.params[1].default.as_deref(),
            Some("[]() -> bool { return(true); }")
        );
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
    fn function_definition_with_inner_error_in_default_value_still_extracts() {
        // The regular `function_definition` rule matches this header despite
        // an ERROR in the default-value subtree. The outer `{ }` body lets
        // `extract_function` produce a usable Function symbol.
        let src = "void broken(int x = [ ] { }) { }\n";
        let t = table_for(src);
        let s = t
            .find("broken")
            .expect("broken symbol from function_definition");
        assert_eq!(s.kind, SymbolKind::Function);
        assert!(!s.is_forward);
        assert_eq!(s.params.len(), 1);
        assert_eq!(s.params[0].name, "x");
    }

    #[test]
    fn test_symbol_kind_rule_variant_exists() {
        // Verify the enum variant exists and is distinct from Function.
        assert_ne!(SymbolKind::Rule, SymbolKind::Function);
    }

    #[test]
    fn test_extract_rule_from_xs_enable_rule() {
        let src = r#"void init() { xsEnableRule("updateBreakdown"); }"#;
        let tree = parser::parse(src).expect("parse");
        let regs = extract_rule_registrations(&tree, src);
        assert!(regs.contains("updateBreakdown"));
    }

    #[test]
    fn test_extract_rule_from_tr_rule_add() {
        let src = r#"void init() { trRuleAdd("updateBreakdown", 1); }"#;
        let tree = parser::parse(src).expect("parse");
        let regs = extract_rule_registrations(&tree, src);
        assert!(regs.contains("updateBreakdown"));
    }

    #[test]
    fn test_extract_rule_from_tr_rule_add_active() {
        let src = r#"void init() { trRuleAddActive("updateBreakdown", 1); }"#;
        let tree = parser::parse(src).expect("parse");
        let regs = extract_rule_registrations(&tree, src);
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
        let tree = parser::parse(src).expect("parse");
        let regs = extract_rule_registrations(&tree, src);
        assert_eq!(regs.len(), 3);
        assert!(regs.contains("ruleA"));
        assert!(regs.contains("ruleB"));
        assert!(regs.contains("ruleC"));
    }

    #[test]
    fn test_extract_no_rules_from_empty_file() {
        let tree = parser::parse("").expect("parse");
        let regs = extract_rule_registrations(&tree, "");
        assert!(regs.is_empty());
    }
}
