//! Convert a tree-sitter parse tree into LSP `Diagnostic`s.
//!
//! Day 4-5 of the spike only reports syntax-level errors — `ERROR` and
//! `MISSING` nodes from the parser. Richer diagnostics (wrong arg count,
//! undefined identifier, etc.) come later in the post-spike roadmap.

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use tree_sitter::{Node, Tree};

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