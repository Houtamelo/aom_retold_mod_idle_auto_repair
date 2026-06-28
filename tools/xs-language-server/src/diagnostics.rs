//! Convert a tree-sitter parse tree into LSP `Diagnostic`s.
//!
//! Day 4-5 of the spike only reports syntax-level errors — `ERROR` and
//! `MISSING` nodes from the parser. Phase 3 adds cross-file semantic
//! diagnostics (extern collisions, forward declarations, mutable redefinition,
//! user-function type checks) on top of the parse diagnostics.

use std::collections::HashMap;
use std::path::Path;

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range, Url};
use tree_sitter::{Node, Tree};

use crate::engine_api::EngineApi;
use crate::merged_view::{IncludeDiagnostic, MergedView};
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
/// only engine-API-based checks run. When `merged` is provided, the
/// include-paste scope drives cross-file resolution.
pub fn collect_all(
    tree: &Tree,
    source: &str,
    engine: &EngineApi,
    table: &SymbolTable,
    project: Option<&VirtualProject>,
    current_file: Option<&Path>,
    merged: Option<&MergedView>,
) -> DiagnosticsByUri {
    let mut diags: DiagnosticsByUri = HashMap::new();
    let current_uri = current_file.and_then(|p| Url::from_file_path(p).ok());

    if let Some(uri) = &current_uri {
        let parse_diags = collect_diagnostics(tree, source);
        if !parse_diags.is_empty() {
            diags.entry(uri.clone()).or_default().extend(parse_diags);
        }

        let typecheck_diags =
            typecheck::check_calls_with_merged(tree, source, engine, table, merged, project);
        if !typecheck_diags.is_empty() {
            diags.entry(uri.clone()).or_default().extend(typecheck_diags);
        }
    }

    if let Some(p) = project {
        if let Some(cf) = current_file {
            let extern_diags = semantic::check_extern_collisions(p, cf, merged);
            merge_diagnostic_maps(&mut diags, extern_diags);

            if let Some(uri) = &current_uri {
                let fwd = if let Some(mv) = merged {
                    semantic::check_forward_declarations_for_merged_view(p, engine, cf, mv)
                } else {
                    semantic::check_forward_declarations(p, engine, cf)
                };
                if !fwd.is_empty() {
                    diags.entry(uri.clone()).or_default().extend(fwd);
                }

                let mut_diags = if let Some(mv) = merged {
                    semantic::check_mutable_redefinitions_for_merged_view(mv)
                } else {
                    semantic::check_mutable_redefinitions(p)
                };
                if !mut_diags.is_empty() {
                    diags.entry(uri.clone()).or_default().extend(mut_diags);
                }
            }
        }
    }

    if let Some(mv) = merged {
        if let Some(uri) = &current_uri {
            for inc in mv.missing_includes() {
                diags
                    .entry(uri.clone())
                    .or_default()
                    .push(include_diagnostic_to_lsp(inc));
            }
        }
    }

    diags
}

fn merge_diagnostic_maps(base: &mut DiagnosticsByUri, other: DiagnosticsByUri) {
    for (uri, ds) in other {
        base.entry(uri).or_default().extend(ds);
    }
}

fn include_diagnostic_to_lsp(inc: &IncludeDiagnostic) -> Diagnostic {
    Diagnostic {
        range: inc.range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(tower_lsp::lsp_types::NumberOrString::String(
            "E0310".to_string(),
        )),
        code_description: None,
        source: Some("xs-language-server".to_string()),
        message: format!("include not found: {}", inc.target),
        related_information: None,
        tags: None,
        data: None,
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

/// Diagnostics grouped by the URI to which they belong.
pub type DiagnosticsByUri = HashMap<Url, Vec<Diagnostic>>;

/// Stable classification bucket for a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCategory {
    ExternCollision,
    UnresolvedSymbol,
    WrongArgCount,
    WrongRangeUri,
    Other,
}

/// Classify a diagnostic by its message content.
pub fn categorize(d: &Diagnostic) -> DiagnosticCategory {
    let msg = d.message.to_ascii_lowercase();
    if msg.contains("duplicate extern") || msg.contains("extern collision") {
        DiagnosticCategory::ExternCollision
    } else if msg.contains("expected") && msg.contains("argument") {
        DiagnosticCategory::WrongArgCount
    } else if msg.contains("error 0310")
        || msg.contains("invalid symbol lookup")
        || msg.contains("unresolved")
    {
        DiagnosticCategory::UnresolvedSymbol
    } else {
        DiagnosticCategory::Other
    }
}

/// True if `d` is a duplicate-`extern` / `extern`-collision diagnostic.
pub fn is_duplicate_extern(d: &Diagnostic) -> bool {
    categorize(d) == DiagnosticCategory::ExternCollision
}

/// True if `d` is an unresolved-symbol/use-before-declaration diagnostic.
pub fn is_unresolved_symbol(d: &Diagnostic) -> bool {
    categorize(d) == DiagnosticCategory::UnresolvedSymbol
}

/// True if `d` is a wrong-argument-count diagnostic.
pub fn is_argument_mismatch(d: &Diagnostic) -> bool {
    categorize(d) == DiagnosticCategory::WrongArgCount
}

#[cfg(test)]
mod tests {
    use tower_lsp::lsp_types::{Diagnostic, Range};

    use super::{categorize, DiagnosticCategory};

    fn diag(message: &str) -> Diagnostic {
        Diagnostic {
            range: Range::default(),
            severity: None,
            code: None,
            code_description: None,
            source: None,
            message: message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    #[test]
    fn test_diagnostic_category_assigns_extern_collision() {
        assert_eq!(
            categorize(&diag("duplicate extern: 'gFoo' is declared extern in a.xs and b.xs")),
            DiagnosticCategory::ExternCollision
        );
        assert_eq!(
            categorize(&diag("extern collision: 'gFoo' is declared extern in a.xs, also defined in b.xs")),
            DiagnosticCategory::ExternCollision
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_unresolved_symbol() {
        assert_eq!(
            categorize(&diag("Error 0310: invalid symbol lookup 'foo' at line 1")),
            DiagnosticCategory::UnresolvedSymbol
        );
        assert_eq!(
            categorize(&diag("unresolved symbol 'bar' at line 2")),
            DiagnosticCategory::UnresolvedSymbol
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_wrong_arg_count() {
        assert_eq!(
            categorize(&diag("expected 4 argument(s) to `aiPlanCreate`, got 2")),
            DiagnosticCategory::WrongArgCount
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_other() {
        assert_eq!(
            categorize(&diag("mutable function 'foo' redefined with different signature")),
            DiagnosticCategory::Other
        );
    }
}
