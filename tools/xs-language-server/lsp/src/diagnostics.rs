//! Convert a tree-sitter parse tree into LSP `Diagnostic`s.
//!
//! Day 4-5 of the spike only reports syntax-level errors — `ERROR` and
//! `MISSING` nodes from the parser. Phase 3 adds cross-file semantic
//! diagnostics (extern collisions, forward declarations, mutable redefinition,
//! user-function type checks) on top of the parse diagnostics.

use std::{collections::HashMap, path::Path};

use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Position, Range, Uri};
use tree_sitter::{Node, Tree};

use crate::{
    definition_check,
    engine_api::EngineApi,
    merged_view::{IncludeDiagnostic, MergedView},
    semantic,
    semantic::VirtualProject,
    symbols::SymbolTable,
    typecheck,
};

/// Walk the tree and collect one `Diagnostic` per error site.
pub fn collect_diagnostics(tree: &Tree, source: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    walk(tree.root_node(), source, &mut out);
    out
}

fn walk(node: Node, source: &str, out: &mut Vec<Diagnostic>) {
    if node.is_error() || node.is_missing() {
        out.push(to_diagnostic(node, source));
    }
    for child in node.children(&mut node.walk()) {
        walk(child, source, out);
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
    let current_uri = current_file.and_then(|p| Uri::from_file_path(p));

    if let Some(uri) = &current_uri {
        let parse_diags = collect_diagnostics(tree, source);
        if !parse_diags.is_empty() {
            diags.entry(uri.clone()).or_default().extend(parse_diags);
        }

        let definition_diags = definition_check::validate_definitions(tree, source);
        if !definition_diags.is_empty() {
            diags.entry(uri.clone()).or_default().extend(definition_diags);
        }

        let typecheck_diags = typecheck::check_calls_with_merged(tree, source, engine, table, merged, project);
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

    // Always emit an entry for the current file so clients receive an empty
    // publishDiagnostics notification when the last issue is resolved.
    if let Some(uri) = &current_uri {
        diags.entry(uri.clone()).or_default();
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
        code: Some(tower_lsp_server::ls_types::NumberOrString::String("E0310".to_string())),
        code_description: None,
        source: Some("xs-language-server".to_string()),
        message: format!("include not found: {}", inc.target),
        related_information: None,
        tags: None,
        data: None,
    }
}

fn to_diagnostic(node: Node, source: &str) -> Diagnostic {
    let start = node.start_position();
    let end = node.end_position();
    let message = if node.is_missing() {
        missing_token_message(node)
    } else if node.is_error() {
        unexpected_token_message(node, source)
    } else {
        // Defensive: should never be reached because callers only hand us
        // ERROR / MISSING nodes, but keeps the exhaustiveness checker happy.
        format!("Parse error near line {}, column {}", start.row + 1, start.column + 1)
    };
    Diagnostic {
        range: Range {
            start: Position::new(start.row as u32, start.column as u32),
            end:   Position::new(end.row as u32, end.column as u32),
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

/// Slice of `source` covered by `node`.
fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str { &source[node.byte_range()] }

/// Try to describe a MISSING node as `Missing '<token>'`.
///
/// A missing node's `kind()` is the expected symbol, which for anonymous
/// terminals (e.g. `;`, `}`) is the literal token text. We use that as the
/// display token; if it looks like a non-terminal or is empty, fall back to
/// a line/column message that avoids exposing grammar internals.
fn missing_token_message(node: Node) -> String {
    let token = node.kind();
    if token.is_empty() || token.chars().any(|c| c.is_alphabetic() && c.is_uppercase()) {
        let pos = node.start_position();
        return format!("Parse error near line {}, column {}", pos.row + 1, pos.column + 1);
    }
    format!("Missing '{}'", token)
}

/// Classify a tree-sitter node `kind` as something safe to put in a
/// user-facing message. Returns `Some(<friendly name>)` if the kind is a
/// concrete token (e.g. `identifier`, `string_literal`, punctuation), or
/// `None` if the kind leaks parser internals (tree-sitter's `"ERROR"` /
/// `"MISSING"` markers) or grammar non-terminals (snake_case symbols like
/// `primitive_type`, `expression`, `_statement`).
fn friendly_kind(kind: &str) -> Option<&str> {
    if kind.is_empty() {
        return None;
    }
    // Tree-sitter's internal error / missing markers MUST NOT be echoed.
    if kind == "ERROR" || kind == "MISSING" {
        return None;
    }
    // Grammar non-terminals are snake_case. We only want concrete tokens
    // (identifiers, string/number literals, punctuation).
    if kind.contains('_') {
        return None;
    }
    Some(kind)
}

/// Try to describe an ERROR node as `Unexpected <kind> '<text>'`.
///
/// Looks at the error node's first named child. If the child's `kind` is a
/// concrete token (per [`friendly_kind`]), the message names it directly.
/// Otherwise we describe the unexpected token generically as `token '<text>'`.
/// Falls back to line/column when the node contains no usable children.
fn unexpected_token_message(node: Node, source: &str) -> String {
    let pos = node.start_position();
    // Prefer the first named child because it usually points at the token
    // that the parser could not consume.
    let target = node.children(&mut node.walk()).find(|c| c.is_named()).unwrap_or(node);
    let kind = target.kind();
    let text = node_text(target, source);
    if text.is_empty() {
        return format!("Parse error near line {}, column {}", pos.row + 1, pos.column + 1);
    }
    match friendly_kind(kind) {
        Some(k) => format!("Unexpected {} '{}'", k, text),
        None => format!("Unexpected token '{}'", text),
    }
}

/// Diagnostics grouped by the URI to which they belong.
pub type DiagnosticsByUri = HashMap<tower_lsp_server::ls_types::Uri, Vec<Diagnostic>>;

/// Stable classification bucket for a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCategory {
    ExternCollision,
    UnresolvedSymbol,
    WrongArgCount,
    WrongArgType,
    DefinitionError,
    WrongRangeUri,
    Other,
}

/// Classify a diagnostic by its message content.
pub fn categorize(d: &Diagnostic) -> DiagnosticCategory {
    let msg = d.message.to_ascii_lowercase();
    if msg.contains("duplicate extern") || msg.contains("extern collision") {
        DiagnosticCategory::ExternCollision
    } else if msg.contains("must have a default value")
        || msg.contains("cannot have a default value")
        || msg.contains("must be initialized")
        || msg.contains("must be assigned a constant expression")
    {
        DiagnosticCategory::DefinitionError
    } else if msg.contains("expected") && msg.contains("argument") && msg.contains("of type") {
        // Type mismatch diagnostics mention the expected type (e.g.
        // "expected argument 1 of type `int`, got `float`").
        DiagnosticCategory::WrongArgType
    } else if msg.contains("expected") && msg.contains("argument") {
        DiagnosticCategory::WrongArgCount
    } else if msg.contains("error 0310") || msg.contains("invalid symbol lookup") || msg.contains("unresolved") {
        DiagnosticCategory::UnresolvedSymbol
    } else {
        DiagnosticCategory::Other
    }
}

/// True if `d` is a duplicate-`extern` / `extern`-collision diagnostic.
pub fn is_duplicate_extern(d: &Diagnostic) -> bool { categorize(d) == DiagnosticCategory::ExternCollision }

/// True if `d` is an unresolved-symbol/use-before-declaration diagnostic.
pub fn is_unresolved_symbol(d: &Diagnostic) -> bool { categorize(d) == DiagnosticCategory::UnresolvedSymbol }

/// True if `d` is a wrong-argument-count diagnostic.
pub fn is_argument_mismatch(d: &Diagnostic) -> bool { categorize(d) == DiagnosticCategory::WrongArgCount }

#[cfg(test)]
mod tests {
    use tower_lsp_server::ls_types::{Diagnostic, Range};

    use super::{DiagnosticCategory, categorize};

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
        assert_eq!(categorize(&diag("unresolved symbol 'bar' at line 2")), DiagnosticCategory::UnresolvedSymbol);
    }

    #[test]
    fn test_diagnostic_category_assigns_wrong_arg_count() {
        assert_eq!(
            categorize(&diag("expected 4 argument(s) to `aiPlanCreate`, got 2")),
            DiagnosticCategory::WrongArgCount
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_wrong_arg_type() {
        assert_eq!(
            categorize(&diag("expected argument 1 of type `int` for `aiPlanCreate`, got `float`")),
            DiagnosticCategory::WrongArgType
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_definition_error() {
        assert_eq!(
            categorize(&diag("non-ref parameter `x` must have a default value")),
            DiagnosticCategory::DefinitionError
        );
        assert_eq!(
            categorize(&diag("ref parameter `x` cannot have a default value")),
            DiagnosticCategory::DefinitionError
        );
        assert_eq!(categorize(&diag("variable `x` must be initialized")), DiagnosticCategory::DefinitionError);
        assert_eq!(
            categorize(&diag("constant `x` must be assigned a constant expression")),
            DiagnosticCategory::DefinitionError
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_other() {
        assert_eq!(
            categorize(&diag("mutable function 'foo' redefined with different signature")),
            DiagnosticCategory::Other
        );
    }

    fn parse_messages(src: &str) -> Vec<String> {
        let tree = crate::parser::parse(src).expect("parse should succeed");
        super::collect_diagnostics(&tree, src)
            .into_iter()
            .map(|d| d.message)
            .collect()
    }

    fn assert_no_internals(message: &str) {
        let banned = ["MISSING", "ERROR", "node", "column(s)"];
        for word in &banned {
            assert!(!message.contains(word), "message {:?} must not contain internal token {:?}", message, word);
        }
    }

    #[test]
    fn parse_message_for_missing_semicolon() {
        let msgs = parse_messages("void f() { int x = 1 }\n");
        let wanted = msgs.iter().find(|m| m == &&"Missing ';'".to_string());
        assert!(wanted.is_some(), "expected \"Missing ';'\" among {:?}", msgs);
        for m in &msgs {
            assert_no_internals(m);
        }
    }

    #[test]
    fn parse_message_for_missing_closing_brace() {
        let msgs = parse_messages("void f() { int x = 1;\n");
        let wanted = msgs.iter().find(|m| m == &&"Missing '}'".to_string());
        assert!(wanted.is_some(), "expected \"Missing '}}'\" among {:?}", msgs);
        for m in &msgs {
            assert_no_internals(m);
        }
    }

    #[test]
    fn parse_message_for_unexpected_identifier() {
        let msgs = parse_messages("void f() { int x = foo bar; }\n");
        let wanted = msgs.iter().find(|m| m.starts_with("Unexpected identifier "));
        assert!(wanted.is_some(), "expected an 'Unexpected identifier ...' message among {:?}", msgs);
        for m in &msgs {
            assert_no_internals(m);
        }
    }

    /// Edge case from `bad.xs`: a missing-expression-after-`=` produces an
    /// ERROR node whose first named child has kind `"ERROR"` (tree-sitter's
    /// internal error marker) or a grammar non-terminal like `"primitive_type"`.
    /// The formatter MUST NOT leak either into the user-facing message.
    #[test]
    fn parse_message_for_missing_rhs_expression_does_not_leak_grammar_internals() {
        // Match the exact context from the bad.xs fixture: a `;` immediately
        // after `=` followed by another statement on the next line.
        let src = "rule brokenRule\nminInterval 5\nactive\n{\nint x = ;\naiEcho(\"hello\");\n}\n";
        let msgs = parse_messages(src);
        for m in &msgs {
            assert_no_internals(m);
            // Grammar non-terminals are snake_case; they MUST NOT appear.
            assert!(!m.contains("primitive_type"), "message {:?} leaks grammar non-terminal", m);
        }
    }

    /// Edge case from `bad.xs`: a `"hello` (unterminated string literal)
    /// produces a MISSING node for the closing `"`. Verify the formatter
    /// describes it without exposing the source bytes or token kind.
    #[test]
    fn parse_message_for_unterminated_string_does_not_leak_grammar_internals() {
        let msgs = parse_messages("void f() { aiEcho(\"hello\n }\n");
        for m in &msgs {
            assert_no_internals(m);
        }
    }

    /// Edge case from `bad.xs`: a stray `;` between two statements produces
    /// an ERROR node whose first named child has kind `"ERROR"` itself
    /// (tree-sitter's internal error marker). The formatter MUST NOT echo
    /// `"ERROR"` back to the user.
    #[test]
    fn parse_message_for_stray_semicolon_does_not_echo_error_token() {
        let msgs = parse_messages("void f() { int x = 1;; int y = 2; }\n");
        for m in &msgs {
            assert_no_internals(m);
        }
    }
}
