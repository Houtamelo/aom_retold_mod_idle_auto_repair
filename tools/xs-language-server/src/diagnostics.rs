//! Convert a tree-sitter parse tree into LSP `Diagnostic`s.
//!
//! Day 4-5 of the spike only reports syntax-level errors — `ERROR` and
//! `MISSING` nodes from the parser. Phase 3 adds cross-file semantic
//! diagnostics (extern collisions, forward declarations, mutable redefinition,
//! user-function type checks) on top of the parse diagnostics.

use std::path::Path;

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use tree_sitter::{Node, Tree};

use crate::engine_api::EngineApi;
use crate::semantic::VirtualProject;
use crate::symbols::SymbolTable;
use crate::{semantic, typecheck};

/// Walk the tree and collect one `Diagnostic` per error site.
pub fn collect_diagnostics(tree: &Tree, _source: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    walk(tree.root_node(), &mut out);
    out
}

fn walk(node: Node, out: &mut Vec<Diagnostic>) {
    if node.is_error() || node.is_missing() {
        out.push(to_diagnostic(node));
    }
    for child in node.children(&mut node.walk()) {
        walk(child, out);
    }
}

/// Full diagnostic pass for a file: parse errors + type-check + semantic.
///
/// `project` and `current_file` are `None` for unowned files; in that case
/// only engine-API-based checks run.
pub fn collect_all(
    tree: &Tree,
    source: &str,
    engine: &EngineApi,
    table: &SymbolTable,
    project: Option<&VirtualProject>,
    current_file: Option<&Path>,
) -> Vec<Diagnostic> {
    let mut diags = collect_diagnostics(tree, source);
    diags.extend(typecheck::check_calls(tree, source, engine, table, project));
    if let (Some(p), Some(cf)) = (project, current_file) {
        diags.extend(semantic::check_all(p, cf));
    }
    diags
}

fn to_diagnostic(node: Node) -> Diagnostic {
    let start = node.start_position();
    let end = node.end_position();
    let kind = if node.is_missing() { "MISSING" } else { "ERROR" };
    let snippet_len = (end.column - start.column).max(1);
    let message = format!(
        "Parse error: unexpected or invalid XS syntax ({kind} node, ~{snippet_len} column(s))"
    );
    Diagnostic {
        range: Range {
            start: Position::new(start.row as u32, start.column as u32),
            end: Position::new(end.row as u32, end.column as u32),
        },
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("xs-language-server".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}