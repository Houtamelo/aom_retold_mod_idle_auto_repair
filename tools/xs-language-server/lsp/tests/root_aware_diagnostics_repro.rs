//! TDD repro tests for PR-5: multi-root diagnostic aggregation.
//!
//! These tests drive the pure aggregation helpers directly rather than the full
//! LSP server. The messaging format (universal / partial / truncated) and the
//! producing-roots priority order are the behaviours under test.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Position, Range, Uri};

use xs_language_server::diagnostics::{
    aggregate_results, collect_all, should_skip_multi_root_pass, DiagnosticCategory,
    DiagnosticContext, DiagnosticsByUri, PerRootDiagnostic,
};
use xs_language_server::engine_api::EngineApi;
use xs_language_server::include_graph::ReverseIncludeGraph;
use xs_language_server::merged_view::{IncludeDiagnosticKind, MergedView};
use xs_language_server::semantic::VirtualProject as SemanticProject;
use xs_language_server::workspace::{VirtualProject as WorkspaceProject, Workspace};

fn file_uri(name: &str) -> Uri {
    Uri::from_file_path(format!("/tmp/{name}")).unwrap()
}

fn root_path(name: &str) -> PathBuf {
    if name.starts_with('/') {
        PathBuf::from(name)
    } else {
        PathBuf::from(format!("/tmp/{name}"))
    }
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

    // Three total roots, but only two produced this diagnostic -> partial coverage.
    let agg = aggregate_results(inputs, 3, &[], &[open_root.clone()]);
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

    // Three total roots, but only two produced this diagnostic -> partial coverage.
    let agg = aggregate_results(inputs, 3, &[mod_dir], &[]);
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

// -----------------------------------------------------------------------------
// PR-6: remaining spec coverage — edge cases in orphans, forward declarations,
// mod overlays, binary .xs files, include cycles, and missing includes.
// -----------------------------------------------------------------------------

/// Write fixture files under a temp root. `rel` paths are relative to the root
/// (e.g. `game/ai/a.xs`); binary content can be passed as a byte slice.
fn write_game_files(root: &Path, files: &[(&str, &[u8])]) {
    for (rel, content) in files {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }
}

/// Build a fixture workspace and semantic project from the listed files.
///
/// If `mod_dir` is given, a mod workspace folder is registered at `<root>/<modDir>`,
/// which activates mod-overlay resolution for files under that directory.
fn build_fixture(
    root: &Path,
    files: &[(&str, &[u8])],
    mod_dir: Option<&str>,
) -> (Workspace, WorkspaceProject, SemanticProject) {
    write_game_files(root, files);

    let mut ws = Workspace::new(root.to_path_buf());
    let mut sources: HashMap<PathBuf, String> = HashMap::new();
    for (rel, content) in files {
        let path = root.join(rel);
        if path.extension().and_then(|e| e.to_str()) != Some("xs") {
            continue;
        }
        if let Ok(text) = std::str::from_utf8(content) {
            sources.insert(path, text.to_string());
        }
    }

    let wp = if let Some(m) = mod_dir {
        let mod_path = root.join(m);
        ws.register_mod(Uri::from_file_path(&mod_path).unwrap()).unwrap();
        let entry = ws.mods().first().unwrap().clone();
        ws.build_virtual_project(&entry)
    } else {
        WorkspaceProject::default()
    };

    (ws, wp, SemanticProject::from_files(sources))
}

/// Run the full diagnostic pass for `path` using the same helpers that
/// `publish_diagnostics` calls in the LSP server.
fn diagnose_file(
    path: &Path,
    ws: &Workspace,
    wp: &WorkspaceProject,
    sem: &SemanticProject,
) -> xs_language_server::diagnostics::DiagnosticsByUri {
    let source = std::fs::read_to_string(path).unwrap();
    let table = xs_language_server::symbols::build_symbol_table(&source);
    let cache_dir = xs_language_server::cache::state_cache_dir();
    let file_view = MergedView::build(path, &source, &table, ws, wp, &cache_dir);
    let ctx = DiagnosticContext {
        source: &source,
        engine: &EngineApi::default(),
        table: &table,
        project: Some(sem),
        file: Some(path),
        file_view: &file_view,
        root_view: Some(&file_view),
    };
    collect_all(&ctx)
}

fn error_messages_for(
    uri: &Uri,
    diags: &xs_language_server::diagnostics::DiagnosticsByUri,
) -> Vec<String> {
    diags
        .get(uri)
        .map(|v| v.iter().map(|d| d.message.clone()).collect())
        .unwrap_or_default()
}

/// V1 over-classification: a file that is not included by any other workspace
/// file is treated as its own root. This is deliberately the accepted V1
/// behavior even though a future PR may distinguish "dead code" from active
/// roots. The orphan pass therefore publishes local diagnostics with no
/// `as seen from:` suffix.
#[test]
fn test_dead_code_orphan_documents_over_classification() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let (ws, wp, sem) = build_fixture(
        root,
        &[("game/ai/dead.xs", b"void deadFn() {}\n")],
        None,
    );
    let path = root.join("game/ai/dead.xs");

    let source = std::fs::read_to_string(&path).unwrap();
    let table = xs_language_server::symbols::build_symbol_table(&source);
    let cache_dir = xs_language_server::cache::state_cache_dir();
    let file_view = MergedView::build(&path, &source, &table, &ws, &wp, &cache_dir);
    let graph = ReverseIncludeGraph::build(file_view.graph());
    assert!(
        graph.roots_that_include(&path).is_empty(),
        "dead-code file must have no includers and be treated as its own root"
    );

    let diags = diagnose_file(&path, &ws, &wp, &sem);
    let uri = Uri::from_file_path(&path).unwrap();
    let file_diags = diags.get(&uri).cloned().unwrap_or_default();
    assert!(
        file_diags.is_empty(),
        "a complete, valid orphan file should produce no diagnostics: {:?}",
        file_diags
    );
    // Vacuously true for a clean file, but documents the orphan-root contract.
    for d in &file_diags {
        assert!(
            !d.message.contains("as seen from"),
            "orphan (self-root) diagnostics must not show a root suffix: {}",
            d.message
        );
    }
}

/// A symbol defined in A, included transitively through B into C, must be
/// callable from C without an unresolved-symbol or use-before-declaration
/// diagnostic.
#[test]
fn test_forward_declaration_across_chain_no_unresolved() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let (ws, wp, sem) = build_fixture(
        root,
        &[
            ("game/ai/a.xs", b"void gizmo() {}\n"),
            ("game/ai/b.xs", b"include \"a.xs\";\nvoid caller() { gizmo(); }\n"),
            ("game/ai/c.xs", b"include \"b.xs\";\nvoid main() { gizmo(); }\n"),
        ],
        None,
    );
    let path = root.join("game/ai/c.xs");

    let diags = diagnose_file(&path, &ws, &wp, &sem);
    let uri = Uri::from_file_path(&path).unwrap();
    let msgs = error_messages_for(&uri, &diags);
    assert!(
        !msgs.iter().any(|m| m.contains("gizmo") && is_unresolved_or_use_before_decl(m)),
        "gizmo should resolve through the transitive include chain; got: {:?}",
        msgs
    );
}

fn is_unresolved_or_use_before_decl(message: &str) -> bool {
    message.contains("unresolved")
        || message.contains("Error 0310")
        || message.contains("before declaration")
}

/// Mod overlays hide vanilla files at the same relative path. The merged
/// include-paste scope for a mod file must see the mod's symbol, not the
/// vanilla one.
///
/// V1 caveat: `forward_callable_merged` still falls back to scanning
/// `semantic::VirtualProject.files`, so the diagnostic pass may *accept*
/// a call to `vanillaFn` even though it is not in the merged scope. The
/// contract asserted here is the include-graph / merged-view contract: only
/// the overlay file participates.
#[test]
fn test_mod_overlay_root_chain_respects_override() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let (ws, wp, sem) = build_fixture(
        root,
        &[
            ("game/ai/core/core.xs", b"void vanillaFn() {}\n"),
            (
                "mod/game/ai/main.xs",
                b"include \"core/core.xs\";\nvoid test() { modFn(); vanillaFn(); }\n",
            ),
            ("mod/game/ai/core/core.xs", b"void modFn() {}\n"),
        ],
        Some("mod"),
    );
    let path = root.join("mod/game/ai/main.xs");
    let mod_core = root.join("mod/game/ai/core/core.xs");

    let source = std::fs::read_to_string(&path).unwrap();
    let table = xs_language_server::symbols::build_symbol_table(&source);
    let cache_dir = xs_language_server::cache::state_cache_dir();
    let file_view = MergedView::build(&path, &source, &table, &ws, &wp, &cache_dir);

    assert!(
        file_view.graph().edges().iter().all(|e| e.to == mod_core),
        "include edge from main.xs must resolve to the mod overlay, got {:?}",
        file_view.graph().edges()
    );
    assert!(
        file_view.find("modFn").is_some(),
        "modFn must be visible in the merged scope"
    );
    assert!(
        file_view.find("vanillaFn").is_none(),
        "vanillaFn must be hidden by the mod overlay"
    );

    let diags = diagnose_file(&path, &ws, &wp, &sem);
    let uri = Uri::from_file_path(&path).unwrap();
    let msgs = error_messages_for(&uri, &diags);
    assert!(
        !msgs.iter().any(|m| m.contains("modFn") && is_unresolved_or_use_before_decl(m)),
        "modFn should resolve through the overlay; got: {:?}",
        msgs
    );
}

/// A binary `.xs` file (the AoM:R random-map serialised-data case) must not
/// abort the merge, must not appear as an include-graph edge, and must be
/// reported as an `Unreadable` include diagnostic.
#[test]
fn test_binary_xs_excluded_from_include_graph() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let binary = root.join("game/ai/binary.xs");
    let (ws, wp, _sem) = build_fixture(
        root,
        &[
            ("game/ai/main.xs", b"include \"binary.xs\";\nvoid ownFn() {}\n"),
            ("game/ai/binary.xs", &[0xff, 0xfe, 0x00, 0xab, 0xcd, 0xef, 0x01]),
        ],
        None,
    );
    let path = root.join("game/ai/main.xs");

    let source = std::fs::read_to_string(&path).unwrap();
    let table = xs_language_server::symbols::build_symbol_table(&source);
    let cache_dir = xs_language_server::cache::state_cache_dir();
    let file_view = MergedView::build(&path, &source, &table, &ws, &wp, &cache_dir);

    assert!(
        file_view.graph().edges().iter().all(|e| e.to != binary),
        "binary .xs target must not appear in the include graph; got {:?}",
        file_view.graph().edges()
    );

    let unreadable: Vec<&str> = file_view.unreadable_includes().map(|d| d.target.as_str()).collect();
    assert!(
        unreadable.contains(&"binary.xs"),
        "binary include must produce an Unreadable diagnostic: {:?}",
        unreadable
    );

    // The includer itself must parse cleanly; the only user-visible issue
    // is the Unreadable include diagnostic for the binary target.
    let parse_diags = xs_language_server::diagnostics::collect_diagnostics(&source);
    assert!(
        parse_diags.is_empty(),
        "the includer should have no parse diagnostics: {:?}",
        parse_diags
    );
}

/// A cycle in the include graph must terminate cleanly and fall back to the
/// orphan rule for the opened file.
#[test]
fn test_include_cycle_falls_back_to_orphan() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let (ws, wp, sem) = build_fixture(
        root,
        &[
            ("game/ai/a.xs", b"include \"b.xs\";\nvoid aFn() { bFn(); }\n"),
            ("game/ai/b.xs", b"include \"a.xs\";\nvoid bFn() {}\n"),
        ],
        None,
    );
    let path = root.join("game/ai/a.xs");

    let source = std::fs::read_to_string(&path).unwrap();
    let table = xs_language_server::symbols::build_symbol_table(&source);
    let cache_dir = xs_language_server::cache::state_cache_dir();
    let file_view = MergedView::build(&path, &source, &table, &ws, &wp, &cache_dir);

    assert!(
        file_view.graph().is_cyclic(),
        "merged view should detect the include cycle"
    );
    let graph = ReverseIncludeGraph::build(file_view.graph());
    assert!(
        graph.roots_that_include(&path).is_empty(),
        "cycle must fall back to treating the opened file as its own root"
    );

    let diags = diagnose_file(&path, &ws, &wp, &sem);
    let uri = Uri::from_file_path(&path).unwrap();
    let msgs = error_messages_for(&uri, &diags);
    assert!(
        !msgs.iter().any(|m| m.contains("bFn") && is_unresolved_or_use_before_decl(m)),
        "bFn is included (despite the cycle) and must resolve; got: {:?}",
        msgs
    );
}

/// A missing include target must be reported through the missing-include path
/// and must not be represented as an edge in either direction of the graph.
#[test]
fn test_unreadable_include_produces_include_diagnostic_not_graph_edge() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let missing = root.join("game/ai/missing.xs");
    let (ws, wp, sem) = build_fixture(
        root,
        &[("game/ai/a.xs", b"include \"missing.xs\";\nvoid aFn() {}\n")],
        None,
    );
    let path = root.join("game/ai/a.xs");

    let source = std::fs::read_to_string(&path).unwrap();
    let table = xs_language_server::symbols::build_symbol_table(&source);
    let cache_dir = xs_language_server::cache::state_cache_dir();
    let file_view = MergedView::build(&path, &source, &table, &ws, &wp, &cache_dir);

    assert!(
        file_view.graph().edges().is_empty(),
        "missing include must not create a graph edge"
    );
    assert!(
        file_view
            .unresolved_includes()
            .any(|d| d.kind == IncludeDiagnosticKind::Missing && d.target == "missing.xs"),
        "missing include must surface as IncludeDiagnostic::Missing"
    );

    let graph = ReverseIncludeGraph::build(file_view.graph());
    assert!(
        graph.direct_includers(&missing).is_empty(),
        "missing file must not appear in the reverse graph"
    );

    let diags = diagnose_file(&path, &ws, &wp, &sem);
    let uri = Uri::from_file_path(&path).unwrap();
    let msgs = error_messages_for(&uri, &diags);
    assert!(
        msgs.iter().any(|m| m.contains("include not found: missing.xs")),
        "missing include must be published as a user-visible diagnostic; got: {:?}",
        msgs
    );
}
