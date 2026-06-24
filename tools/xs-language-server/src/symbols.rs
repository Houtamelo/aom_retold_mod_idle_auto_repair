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

/// A function parameter — `int x` or `string s = "default"`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Param {
    pub ty: String,
    pub name: String,
}

impl Param {
    /// `int x` or `int x = 5` — used in hover signatures.
    pub fn render(&self, source: &str) -> String {
        // Find the parameter declaration text in the source so we preserve
        // default values verbatim.
        // The caller already knows the byte range; we accept the source and
        // let render just print type + name (defaults are appended separately).
        let _ = source;
        format!("{} {}", self.ty, self.name)
    }
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
    pub is_extern: bool,
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
            "function_definition" => {
                extract_function(child, source, &mut table.symbols)
            }
            "declaration" => extract_declaration(child, source, &mut table.symbols),
            _ => {}
        }
    }
    table
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

    // The `mutable` / `extern` storage class.
    let is_extern = find_named_child(node, "storage_class_specifier")
        .map(|n| {
            let mut sub = n.walk();
            n.children(&mut sub)
                .any(|c| matches!(c.kind(), "extern" | "static" | "mutable"))
        })
        .unwrap_or(false);

    // Parameters from the `parameter_list` child.
    let params = find_named_child(node, "parameter_list")
        .map(|n| extract_params(n, source))
        .unwrap_or_default();

    let full_range = node_range(node);
    let selection_range = node_range(name_node);
    let detail = if params.is_empty() {
        format!("{} {}()", ty, name)
    } else {
        format!(
            "{} {}({})",
            ty,
            name,
            params
                .iter()
                .map(|p| format!("{} {}", p.ty, p.name))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };

    out.push(Symbol {
        name,
        kind: SymbolKind::Function,
        ty,
        params,
        is_extern,
        full_range,
        selection_range,
        detail,
    });
}

fn extract_declaration(node: tree_sitter::Node<'_>, source: &str, out: &mut Vec<Symbol>) {
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

    let is_const = find_named_child(node, "type_qualifier")
        .map(|n| node_text(n, source).trim() == "const")
        .unwrap_or(false);

    let full_range = node_range(node);
    let selection_range = node_range(name_node);
    // Preserve the RHS for the detail line so the user can see `int x = 5`
    // not just `int x`.
    let init_text = node_text(init, source).to_string();
    let detail = format!("{} {}", ty, init_text);

    out.push(Symbol {
        name,
        kind: if is_const {
            SymbolKind::Constant
        } else {
            SymbolKind::Variable
        },
        ty,
        params: Vec::new(),
        is_extern: false,
        full_range,
        selection_range,
        detail,
    });
}

fn extract_params(node: tree_sitter::Node<'_>, source: &str) -> Vec<Param> {
    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "parameter_declaration" {
            continue;
        }
        let ty = find_named_child(child, "primitive_type")
            .or_else(|| find_named_child(child, "array_type"))
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_default();
        let name = find_named_child(child, "identifier")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_default();
        params.push(Param { ty, name });
    }
    params
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
fn find_named_child<'a>(
    node: tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
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
    }

    #[test]
    fn extracts_function_with_params() {
        let src = "mutable void setFoo(int x, string s) {}\n";
        let t = table_for(src);
        let s = t.find("setFoo").expect("setFoo symbol");
        assert_eq!(s.kind, SymbolKind::Function);
        assert_eq!(s.ty, "void");
        assert_eq!(s.params.len(), 2);
        assert_eq!(s.params[0].ty, "int");
        assert_eq!(s.params[0].name, "x");
        assert!(s.detail.contains("int x"));
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
}