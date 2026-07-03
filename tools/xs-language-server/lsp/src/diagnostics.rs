//! Convert parser diagnostics and semantic checks into LSP `Diagnostic`s.
//!
//! `collect_diagnostics` maps the `lelwel` parser's `Diagnostic` values to
//! LSP severity/ranges. `collect_all` layers definition-time and type-check
//! diagnostics on top.

use std::{collections::HashMap, path::Path};

use codespan_reporting::diagnostic::Severity;
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Uri};

use crate::{
    definition_check,
    engine_api::EngineApi,
    merged_view::{IncludeDiagnostic, MergedView},
    range::span_to_range,
    semantic,
    semantic::VirtualProject,
    symbols::SymbolTable,
    typecheck,
};

/// Consume `lelwel` parser diagnostics and convert them to LSP diagnostics.
pub fn collect_diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut parse_diags = Vec::new();
    let _cst = xs_parser::parser::Parser::new_with_context(
        source,
        &mut parse_diags,
        xs_parser::ast::TypeTable::with_primitives(),
    )
    .parse(&mut parse_diags);
    parse_diags
        .into_iter()
        .map(|d| xs_diagnostic_to_lsp(source, &d))
        .collect()
}

fn xs_diagnostic_to_lsp(source: &str, d: &xs_parser::parser::Diagnostic) -> Diagnostic {
    let range = d
        .labels
        .first()
        .map(|label| span_to_range(source, label.range.clone()))
        .unwrap_or_default();
    let severity = Some(match d.severity {
        Severity::Bug | Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Note => DiagnosticSeverity::INFORMATION,
        Severity::Help => DiagnosticSeverity::HINT,
    });
    Diagnostic {
        range,
        severity,
        code: None,
        code_description: None,
        source: Some("xs-language-server".to_string()),
        message: d.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Full diagnostic pass for a file: parse errors + type-check + semantic.
///
/// `project` and `current_file` are `None` for unowned files; in that case
/// only engine-API-based checks run. When `merged` is provided, the
/// include-paste scope drives cross-file resolution.
pub fn collect_all(
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
        let parse_diags = collect_diagnostics(source);
        if !parse_diags.is_empty() {
            diags.entry(uri.clone()).or_default().extend(parse_diags);
        }

        let definition_diags = definition_check::validate_definitions(source);
        if !definition_diags.is_empty() {
            diags.entry(uri.clone()).or_default().extend(definition_diags);
        }

        let typecheck_diags = typecheck::check_calls_with_merged(source, engine, table, merged, project);
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
    use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Range};

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

    #[test]
    fn collect_diagnostics_maps_parse_severity() {
        // `void f(` is missing the parameter list / closing brace, so the
        // typed parser emits at least one ERROR-severity diagnostic.
        let src = "void f(\n";
        let diags = super::collect_diagnostics(src);
        assert!(
            diags.iter().any(|d| matches!(d.severity, Some(DiagnosticSeverity::ERROR))),
            "expected an ERROR diagnostic for malformed source, got: {:?}",
            diags
        );
    }

    #[test]
    fn collect_diagnostics_is_empty_for_valid_source() {
        let src = "void f() {}\n";
        let diags = super::collect_diagnostics(src);
        assert!(diags.is_empty(), "expected no diagnostics for valid source, got: {:?}", diags);
    }

    #[test]
    fn collect_diagnostics_does_not_leak_internal_marker() {
        let src = "void f(\n";
        let diags = super::collect_diagnostics(src);
        for d in &diags {
            assert!(
                !d.message.contains("MISSING") && !d.message.contains("ERROR") && !d.message.contains("node"),
                "message {:?} leaks parser internals",
                d.message
            );
        }
    }
}
