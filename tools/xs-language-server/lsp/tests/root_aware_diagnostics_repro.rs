//! TDD repro tests for PR-5: multi-root diagnostic aggregation.
//!
//! These tests drive the pure aggregation helpers directly rather than the full
//! LSP server. The messaging format (universal / partial / truncated) and the
//! producing-roots priority order are the behaviours under test.

use std::collections::HashMap;
use std::path::PathBuf;

use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Uri};

use xs_language_server::diagnostics::{
    aggregate_results, should_skip_multi_root_pass, DiagnosticCategory, DiagnosticsByUri,
    PerRootDiagnostic,
};

fn file_uri(name: &str) -> Uri {
    Uri::from_file_path(format!("/tmp/{name}")).unwrap()
}

fn root_path(name: &str) -> PathBuf {
    PathBuf::from(format!("/tmp/{name}"))
}

fn dummy_range() -> Range {
    Range::new(Position::new(0, 0), Position::new(0, 5))
}

fn diagnostic(message: &str) -> Diagnostic {
    Diagnostic {
        range: dummy_range(),
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(NumberOrString::String("E0001".to_string())),
        code_description: None,
        source: Some("xs-language-server".to_string()),
        message: message.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}

fn per_root(uri: Uri, message: &str, root: &str, category: DiagnosticCategory) -> PerRootDiagnostic {
    PerRootDiagnostic {
        uri,
        range: dummy_range(),
        severity: DiagnosticSeverity::ERROR,
        code: Some(NumberOrString::String("E0001".to_string())),
        category,
        base_message: message.to_string(),
        producing_root: root_path(root),
    }
}

#[test]
fn test_orphan_file_gets_full_diagnostics() {
    let uri = file_uri("A.xs");
    let inputs = vec![per_root(
        uri.clone(),
        "missing forward declaration for 'foo'",
        "A.xs",
        DiagnosticCategory::Other,
    )];

    let agg = aggregate_results(inputs, 1, &[], &[]);
    let diags = agg.get(&uri).expect("one diagnostic for the orphan file");
    assert_eq!(diags.len(), 1);
    let msg = &diags[0].message;
    assert!(
        msg.contains("missing forward declaration for 'foo'"),
        "base message was altered: {msg}"
    );
    assert!(
        !msg.contains("as seen from"),
        "orphan (single self-root) must not show a root suffix: {msg}"
    );
}

#[test]
fn test_single_root_reachable_file_emits_as_seen_from() {
    let uri = file_uri("A.xs");
    let inputs = vec![per_root(
        uri.clone(),
        "unresolved symbol 'foo'",
        "C.xs",
        DiagnosticCategory::UnresolvedSymbol,
    )];

    let agg = aggregate_results(inputs, 1, &[], &[]);
    let diags = agg.get(&uri).expect("one diagnostic for the reachable file");
    assert_eq!(diags.len(), 1);
    let msg = &diags[0].message;
    assert!(msg.contains("unresolved symbol 'foo'"), "base message was altered: {msg}");
    assert!(
        msg.contains("as seen from: C.xs"),
        "single reachable root must be listed: {msg}"
    );
}

#[test]
fn test_multi_root_partial_coverage_lists_trees() {
    let uri = file_uri("A.xs");
    let inputs = vec![per_root(
        uri.clone(),
        "unresolved symbol 'foo'",
        "R1.xs",
        DiagnosticCategory::UnresolvedSymbol,
    )];

    let agg = aggregate_results(inputs, 2, &[], &[]);
    let msg = &agg.get(&uri).unwrap()[0].message;
    assert!(msg.contains("as seen from: R1.xs"), "partial coverage must list producing root: {msg}");
}

#[test]
fn test_multi_root_universal_coverage_omits_tree_names() {
    let uri = file_uri("A.xs");
    let base = "unresolved symbol 'foo'";
    let inputs = vec![
        per_root(uri.clone(), base, "R1.xs", DiagnosticCategory::UnresolvedSymbol),
        per_root(uri.clone(), base, "R2.xs", DiagnosticCategory::UnresolvedSymbol),
        per_root(uri.clone(), base, "R3.xs", DiagnosticCategory::UnresolvedSymbol),
    ];

    let agg = aggregate_results(inputs, 3, &[], &[]);
    let msg = &agg.get(&uri).unwrap()[0].message;
    assert!(msg.contains(base), "base message was altered: {msg}");
    assert!(!msg.contains("as seen from"), "universal coverage must omit tree names: {msg}");
}

#[test]
fn test_multi_root_truncated_coverage_shows_first_three_and_more() {
    let uri = file_uri("A.xs");
    let base = "unresolved symbol 'foo'";
    let mut inputs = Vec::with_capacity(105);
    for i in 1..=105 {
        let name = format!("R{:03}.xs", i);
        inputs.push(per_root(uri.clone(), base, &name, DiagnosticCategory::UnresolvedSymbol));
    }

    let agg = aggregate_results(inputs, 105, &[], &[]);
    let msg = &agg.get(&uri).unwrap()[0].message;
    let suffix = "\n  as seen from: R001.xs, R002.xs, R003.xs ...and 102 more";
    assert!(
        msg.ends_with(suffix),
        "truncated suffix did not match; got: {msg}"
    );
}

#[test]
fn test_local_short_circuit_skips_multi_root_when_no_cross_file_issues() {
    let uri = file_uri("A.xs");

    let mut local_only: DiagnosticsByUri = HashMap::new();
    local_only.insert(
        uri.clone(),
        vec![Diagnostic {
            range: dummy_range(),
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("E0002".to_string())),
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message: "non-ref parameter `x` must have a default value".to_string(),
            related_information: None,
            tags: None,
            data: None,
        }],
    );
    assert!(
        should_skip_multi_root_pass(&local_only),
        "definition-only diagnostics should allow the local-pass short-circuit"
    );

    let mut cross_file: DiagnosticsByUri = HashMap::new();
    cross_file.insert(
        uri,
        vec![diagnostic("unresolved symbol 'foo'")],
    );
    assert!(
        !should_skip_multi_root_pass(&cross_file),
        "unresolved-symbol diagnostics must trigger the multi-root pass"
    );
}

#[test]
fn test_producing_roots_priority_order_currently_open_first() {
    let uri = file_uri("A.xs");
    let open_root = root_path("open.xs");
    let inputs = vec![
        per_root(uri.clone(), "msg", "alphabetical.xs", DiagnosticCategory::UnresolvedSymbol),
        per_root(uri.clone(), "msg", "open.xs", DiagnosticCategory::UnresolvedSymbol),
    ];

    let agg = aggregate_results(inputs, 2, &[], &[open_root.clone()]);
    let msg = &agg.get(&uri).unwrap()[0].message;
    assert!(
        msg.contains("as seen from: open.xs, alphabetical.xs"),
        "currently-open root must be listed first: {msg}"
    );
}

#[test]
fn test_producing_roots_priority_order_mod_overlay_second() {
    let uri = file_uri("A.xs");
    let mod_dir = PathBuf::from("/mod");
    let inputs = vec![
        per_root(uri.clone(), "msg", "alpha.xs", DiagnosticCategory::UnresolvedSymbol),
        per_root(uri.clone(), "msg", "/mod/mod.xs", DiagnosticCategory::UnresolvedSymbol),
    ];

    let agg = aggregate_results(inputs, 2, &[mod_dir], &[]);
    let msg = &agg.get(&uri).unwrap()[0].message;
    assert!(
        msg.contains("as seen from: mod.xs, alpha.xs"),
        "mod-overlay root must precede alphabetical root: {msg}"
    );
}

#[test]
fn test_universal_counts_match_roots_when_all_produce() {
    let uri = file_uri("A.xs");
    let inputs = vec![
        per_root(uri.clone(), "wrong arg count", "R1.xs", DiagnosticCategory::WrongArgCount),
        per_root(uri.clone(), "wrong arg count", "R2.xs", DiagnosticCategory::WrongArgCount),
    ];

    let agg = aggregate_results(inputs, 2, &[], &[]);
    let msg = &agg.get(&uri).unwrap()[0].message;
    assert!(!msg.contains("as seen from"), "all roots producing is universal: {msg}");
}
