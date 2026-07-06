//! Regression tests for the indirect-include false-positive diagnostic
//! scenario.
//!
//! Three-file layout mimicking an entry-point file that includes B first and
//! then includes A, where A references B's symbols without including B
//! itself:
//!
//! ```text
//! A.xs
//!   void use_indirectly_imported_symbol() {
//!      int result = imported_symbol + 5;  // symbol defined in B.xs
//!   }
//!
//! B.xs
//!   int imported_symbol = 8;
//!
//! C.xs (entry point)
//!   include "B.xs";
//!   include "A.xs";
//! ```
//!
//! The XS engine accepts this at load time because it doesn't care about
//! individual-file order — it concatenates the entry point's include chain
//! and resolves every symbol against the union of the included files. From
//! A.xs's perspective the merged-include graph lists only A (A has no
//! `#include` of its own), but `imported_symbol` is genuinely visible once
//! C pastes the chain together.
//!
//! This file hosts TWO sub-tests. See the results summary below for which
//! one fails today.
//!
//! ## test_indirect_include_does_not_emit_unresolved_symbol_on_a (GREEN)
//!
//! The user's literal bug hypothesis. A.xs references `imported_symbol` as a
//! value in an arithmetic expression — no `CallExpr` uses the name. The test
//! asserts that the LSP does NOT flag it as unresolved. Today's typed-AST
//! behaviour: `check_calls_with_merged` only inspects `CallExpr` callees
//! (`typecheck::extract_callee_name` only fires on `PostfixInner::Call`),
//! and `check_forward_declarations_for_merged_view` only walks `CallExpr`s
//! (`semantic::collect_calls` filters by `Rule::CallExpr`). Bare identifier
//! references inside non-call expressions are not flagged anywhere in the
//! current pipeline. Therefore: **no false-positive, not a bug**.
//!
//! ## test_indirect_include_does_not_emit_unresolved_when_used_as_callee (RED)
//!
//! The same three-file layout, but A.xs CAL `imported_symbol()` — turning
//! the symbol into a `CallExpr` callee. This sub-test is what the user could
//! have meant by "can't be resolved". It currently FAILS with the exact
//! diagnostic:
//!
//! ```text
//! Error 0310: invalid symbol lookup 'imported_symbol' at line 2
//! ```
//!
//! Root cause: `forward_callable_merged` (`semantic.rs::forward_callable_merged`)
//! inspects `merged.symbols()` and `project.registered_rules` but does NOT
//! include the project-fallback that `forward_callable` (the non-merged
//! counterpart at `semantic.rs::forward_callable`) has — namely, "defined in
//! another file of the project is always callable". Opening A.xs in
//! isolation gives a merged view containing only A, so `imported_symbol`
//! (defined in B) does not appear; the merged-view resolver then concludes
//! the symbol is missing and emits the E0310 diagnostic. The non-merged
//! variant, if it were used here, would have found the symbol in the
//! workspace-wide project and emitted no diagnostic.
//!
//! One-line fix sketch (NOT applied, per the user's "do not propose a fix
//! beyond a one-line mention" instruction): mirror the
//! `defined_elsewhere`-style `project.files` block from
//! `forward_callable` (lines 814-824) into `forward_callable_merged`.
//! Today's typed-AST path passes the merged view into the check, which is
//! why this regression slips through.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use tempfile::TempDir;
use tower_lsp_server::ls_types::Diagnostic;

use xs_language_server::cache::PerRootMergedViewCache;
use xs_language_server::diagnostics;
use xs_language_server::engine_api::EngineApi;
use xs_language_server::include_graph::ReverseIncludeGraph;
use xs_language_server::merged_view::MergedView;
use xs_language_server::semantic::VirtualProject as SemanticProject;
use xs_language_server::symbols::SymbolTable;
use xs_language_server::workspace::{VirtualProject as WorkspaceVirtualProject, Workspace};

/// Write fixtures under a temporary `game/` root, build an in-memory semantic
/// project covering every file, and build a merged view rooted at
/// `current_rel`.
fn indirect_include_fixture(
    files: &[(&str, &str)],
    current_rel: &str,
) -> (TempDir, SemanticProject, MergedView, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let mut map = HashMap::new();
    for (rel, src) in files {
        let path = root.join("game").join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(src.as_bytes()).unwrap();
        map.insert(path.clone(), src.to_string());
    }
    let prj = SemanticProject::from_files(map);
    let current = root.join("game").join(current_rel);
    let source = prj.files.get(&current).unwrap().source.clone();
    let own = prj.files.get(&current).unwrap().table.clone();
    let ws = Workspace::new(root.to_path_buf());
    let project = WorkspaceVirtualProject::default();
    let cache_dir = TempDir::new().unwrap();
    let merged = MergedView::build(&current, &source, &own, &ws, &project, cache_dir.path());
    (tmp, prj, merged, current)
}

/// Infer the `game/` install root for a fixture path (`.../game/ai/A.xs`).
fn infer_game_root(current_file: &std::path::Path) -> Option<std::path::PathBuf> {
    current_file.ancestors().find(|p| p.file_name() == Some(std::ffi::OsStr::new("game"))).and_then(|p| p.parent()).map(|p| p.to_path_buf())
}

/// Mirror the exact entry point the LSP server runs on `didOpen` (see
/// `server::publish_diagnostics` → `diagnostics::collect_all`).
fn publish_diagnostics_for(
    project: &SemanticProject,
    engine: &EngineApi,
    merged: &MergedView,
    current_file: &std::path::Path,
    source: &str,
) -> Vec<Diagnostic> {
    let current_uri = tower_lsp_server::ls_types::Uri::from_file_path(current_file)
        .expect("current file must be a file:// URI");
    let own_table: SymbolTable = project
        .files
        .get(current_file)
        .map(|f| f.table.clone())
        .unwrap_or_default();

    // Build the root-chain view just like server::publish_diagnostics does.
    let root_view: Option<MergedView> = infer_game_root(current_file).and_then(|game_root| {
        let graph = ReverseIncludeGraph::build(merged.graph());
        let roots = graph.roots_that_include(current_file);
        roots.into_iter().next().and_then(|root| {
            let ws = Workspace::new(game_root);
            let project = WorkspaceVirtualProject::default();
            let cache_dir = tempfile::TempDir::new().unwrap();
            let cache = PerRootMergedViewCache::new(cache_dir.path());
            MergedView::build_from_root(&root, &ws, &project, &cache).ok().map(|arc| (*arc).clone())
        })
    });

    let ctx = diagnostics::DiagnosticContext {
        source,
        engine,
        table: &own_table,
        project: Some(project),
        file: Some(current_file),
        file_view: merged,
        root_view: root_view.as_ref(),
    };
    let by_uri = diagnostics::collect_all(&ctx);
    by_uri.get(&current_uri).cloned().unwrap_or_default()
}

/// The user's three-file scenario, opened from A.xs.
///
/// A.xs has no `#include` of its own, so the per-file merged view for A
/// contains only A. The semantic `VirtualProject` covers the full set of
/// workspace files (A, B, C). The XS engine's textual-paste semantics
/// resolve `imported_symbol` at link time via C, so the LSP must not flag
/// it as unresolved when A is opened in isolation.
#[test]
fn test_indirect_include_does_not_emit_unresolved_symbol_on_a() {
    let (_tmp, prj, merged, current) = indirect_include_fixture(
        &[
            (
                // Line numbers below are 0-indexed; "// line 0" lives at index 0
                // so line 3 in the user's example is the `int result = ...` line.
                "ai/A.xs",
                "void use_indirectly_imported_symbol() {\n\
                 // line 1\n\
                 // line 2\n\
                 int result = imported_symbol + 5;\n\
                 }\n",
            ),
            ("ai/B.xs", "int imported_symbol = 8;\n"),
            (
                "ai/C.xs",
                "include \"B.xs\";\n\
                 include \"A.xs\";\n",
            ),
        ],
        "ai/A.xs",
    );

    // Sanity-check the fixture: A.xs has no `#include` directives, so its
    // merged view does not contain B's symbols. If this ever stops being true
    // the test scenario no longer exercises the indirect-include path.
    assert!(
        merged.find("imported_symbol").is_none(),
        "test fixture is wrong: A.xs's merged view must NOT include B's `imported_symbol` for this regression to be \
         meaningful (got Some, meaning A is reaching B through its own include chain, which would be the direct case)"
    );

    // The semantic project DOES see every file in the workspace. The user's
    // pre-Phase-1 hypothesis was that the LSP resolves symbols against the
    // union, which is correct. Asserting it directly keeps the test honest if
    // the project model ever narrows visibility to the merged view alone.
    assert!(
        prj.files.values().any(|f| f.table.find("imported_symbol").is_some()),
        "semantic project must contain B.xs's `imported_symbol`; otherwise we are not actually exercising the \
         indirect-include path"
    );

    let engine = EngineApi::default();
    let source = prj.files.get(&current).unwrap().source.clone();
    let diags = publish_diagnostics_for(&prj, &engine, &merged, &current, &source);

    // The user's bug hypothesis: a "symbol `imported_symbol` cannot be
    // resolved" diagnostic on line 3 of A.xs. We test against both the
    // exact phrasing the legacy LSP used and the E0310 wording the typed
    // AST emits today.
    let offenders: Vec<&Diagnostic> = diags
        .iter()
        .filter(|d| {
            let m = d.message.as_str();
            m.contains("imported_symbol")
                && (m.contains("cannot be resolved")
                    || m.contains("cannot be found")
                    || m.contains("Error 0310")
                    || m.contains("invalid symbol lookup")
                    || m.contains("unresolved"))
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "indirectly-included symbol `imported_symbol` must not be flagged as unresolved when A.xs is opened in isolation. \
         Got diagnostics on line 3 of A.xs: {:?}",
        offenders.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    // Belt and braces: the line-3 range of A.xs (4 lines, so line 3 of a
    // 0-indexed file is the `int result = ...` line) must produce no
    // diagnostic whatsoever, even unrelated ones.
    let line_3_diags: Vec<&Diagnostic> = diags
        .iter()
        .filter(|d| d.range.start.line == 3)
        .collect();
    assert!(
        line_3_diags.is_empty(),
        "no diagnostic of any kind should land on line 3 of A.xs (`int result = imported_symbol + 5;`); got {:?}",
        line_3_diags.iter().map(|d| &d.message).collect::<Vec<_>>()
    );

    // Sanity log: dump every emitted diagnostic so a future regressor sees
    // what the LSP actually said for this fixture instead of puzzling over
    // an empty offenders list.
    if !diags.is_empty() {
        eprintln!(
            "[indirect_include_symbol_repro] A.xs emitted {} diagnostic(s) for the fixture: {:?}",
            diags.len(),
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
}

/// Same three-file layout, but `imported_symbol` is called as if it were a
/// function inside A.xs. This is the only way the typed AST would actually
/// produce an E0310-style diagnostic for an unknown callee today, because
/// `check_forward_declarations_for_merged_view` only inspects `CallExpr`
/// targets — bare identifier references inside expressions are not flagged.
///
/// This sub-test pins down that even the more aggressive shape of the user's
/// bug hypothesis does not regress: when the LSP opens A.xs in isolation,
/// `imported_symbol` is NOT in A's include chain and IS in the semantic
/// project's workspace-wide file set, so the `forward_callable_merged`
/// helper is satisfied via the project fallback and emits no diagnostic.
#[test]
fn test_indirect_include_does_not_emit_unresolved_when_used_as_callee() {
    let (_tmp, prj, merged, current) = indirect_include_fixture(
        &[
            (
                "ai/A.xs",
                "void use_indirectly_imported_symbol() {\n\
                 imported_symbol();\n\
                 }\n",
            ),
            // Defining `imported_symbol` as a *function* (not a variable) is
            // the realistic case for this sub-test: the LSP checks callable
            // callees via `check_forward_declarations_for_merged_view`.
            ("ai/B.xs", "void imported_symbol() {}\n"),
            ("ai/C.xs", "include \"B.xs\";\ninclude \"A.xs\";\n"),
        ],
        "ai/A.xs",
    );

    assert!(
        merged.find("imported_symbol").is_none(),
        "test fixture invariant: A.xs's merged view must not contain B's symbol (A doesn't include B directly)"
    );

    let engine = EngineApi::default();
    let source = prj.files.get(&current).unwrap().source.clone();
    let diags = publish_diagnostics_for(&prj, &engine, &merged, &current, &source);

    let offenders: Vec<&Diagnostic> = diags
        .iter()
        .filter(|d| {
            let m = d.message.as_str();
            m.contains("imported_symbol")
                && (m.contains("Error 0310")
                    || m.contains("invalid symbol lookup")
                    || m.contains("before declaration")
                    || m.contains("unresolved"))
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "LSP must not flag `imported_symbol()` in A.xs as unresolved even when A doesn't include B directly — the \
         workspace project contains B so the symbol is link-time reachable through C. \
         Diagnostics: {:?}",
        offenders.iter().map(|d| &d.message).collect::<Vec<_>>()
    );
}
