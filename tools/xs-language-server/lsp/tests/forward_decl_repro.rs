//! Strict-TDD regression tests for Issue #2: false-positive "used before
//! declaration" diagnostics when a symbol is reached through a transitive
//! include.

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use tempfile::TempDir;

use xs_language_server::engine_api::EngineApi;
use xs_language_server::merged_view::{MergedView, VisibilityProvenance};
use xs_language_server::semantic::{
    VirtualProject as SemanticProject, check_forward_declarations_for_merged_view,
};
use xs_language_server::workspace::{VirtualProject as WorkspaceVirtualProject, Workspace};

/// Write fixtures under a temporary `game/` root, build an in-memory semantic
/// project, and build a merged view for `current_rel`.
fn merged_fixture(
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

fn before_declaration_count(diags: &[tower_lsp_server::ls_types::Diagnostic]) -> usize {
    diags
        .iter()
        .filter(|d| d.message.contains("before declaration"))
        .count()
}

#[test]
fn test_vanilla_main_xs_in_mod_overlay_produces_zero_false_positives() {
    // Mirrors the Issue #2 layout: main.xs includes core/core.xs early and
    // calls a function defined in a transitively-included file shortly after.
    // The transitive include directive lives at a *late* line in core.xs.
    let (_tmp, prj, merged, current) = merged_fixture(
        &[
            (
                "ai/core/main.xs",
                "include \"core/core.xs\";\n\nvoid setup() { setupDebugCategories(); }\n",
            ),
            (
                "ai/core/core.xs",
                "// lots of other code\n\n// more filler lines so the include line is late\ninclude \"core/utilities/debug.xs\";\n",
            ),
            (
                "ai/core/utilities/debug.xs",
                "void setupDebugCategories() {}\n",
            ),
        ],
        "ai/core/main.xs",
    );

    let ms = merged
        .find("setupDebugCategories")
        .expect("setupDebugCategories should be visible in merged view");
    assert!(
        matches!(
            ms.provenance,
            VisibilityProvenance::TransitiveInclude { .. }
        ),
        "expected transitive include provenance, got {:?}",
        ms.provenance
    );

    let diags =
        check_forward_declarations_for_merged_view(&prj, &EngineApi::default(), &current, &merged, None);
    assert_eq!(
        before_declaration_count(&diags),
        0,
        "expected zero 'before declaration' diagnostics for Issue #2 layout, got {diags:?}"
    );
}

#[test]
fn test_real_same_file_forward_decl_error_still_detected() {
    // A genuine use-before-definition inside the same file must still be
    // reported even when the merged view is available.
    let (_tmp, prj, merged, current) = merged_fixture(
        &[("ai/a.xs", "void foo() { bar(); }\nvoid bar() {}\n")],
        "ai/a.xs",
    );

    let diags =
        check_forward_declarations_for_merged_view(&prj, &EngineApi::default(), &current, &merged, None);
    assert_eq!(
        before_declaration_count(&diags),
        1,
        "expected exactly one 'before declaration' diagnostic for same-file forward use, got {diags:?}"
    );
    assert!(diags.iter().any(|d| d.message.contains("bar")));
}

#[test]
fn test_mutable_function_called_before_redefinition_does_not_emit() {
    // An included file declares a mutable helper and later redefines it. The
    // analysed file includes it and calls helper() after the include.
    let (_tmp, prj, merged, current) = merged_fixture(
        &[
            (
                "ai/main.xs",
                "include \"helper.xs\";\n\nvoid setup() { helper(); }\n",
            ),
            (
                "ai/helper.xs",
                "mutable void helper() {}\nvoid helper() {}\n",
            ),
        ],
        "ai/main.xs",
    );

    let diags =
        check_forward_declarations_for_merged_view(&prj, &EngineApi::default(), &current, &merged, None);
    assert_eq!(
        before_declaration_count(&diags),
        0,
        "mutable helper should be forward-callable through transitive include, got {diags:?}"
    );
}

#[test]
fn test_cyclic_includes_do_not_infinite_loop() {
    // a.xs <-> b.xs. Build the merged view inside a bounded wait; the walker
    // must terminate and report the cycle.
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let a = root.join("game").join("ai").join("a.xs");
    let b = root.join("game").join("ai").join("b.xs");
    std::fs::create_dir_all(a.parent().unwrap()).unwrap();
    std::fs::write(&a, "include \"b.xs\";\nvoid aFn() {}\n").unwrap();
    std::fs::write(&b, "include \"a.xs\";\nvoid bFn() {}\n").unwrap();

    let map: HashMap<PathBuf, String> = [
        (a.clone(), std::fs::read_to_string(&a).unwrap()),
        (b.clone(), std::fs::read_to_string(&b).unwrap()),
    ]
    .into_iter()
    .collect();
    let prj = SemanticProject::from_files(map);
    let source = prj.files.get(&a).unwrap().source.clone();
    let own = prj.files.get(&a).unwrap().table.clone();
    let ws = Workspace::new(root.to_path_buf());
    let project = WorkspaceVirtualProject::default();
    let cache_dir = TempDir::new().unwrap();

    let merged = std::thread::scope(|s| {
        let handle =
            s.spawn(|| MergedView::build(&a, &source, &own, &ws, &project, cache_dir.path()));
        handle
            .join()
            .expect("merged view build should terminate without panicking")
    });

    assert!(
        merged.graph().is_cyclic(),
        "cycle should be detected, got {:?}",
        merged.graph().edges()
    );
}

#[test]
fn test_multiple_root_includes_takes_minimum_effective_line() {
    // The same file is included on lines 5 and 10 of the analysed file. The
    // symbol's effective_line must be 5 (the earliest reaching edge).
    let (_tmp, _prj, merged, _current) = merged_fixture(
        &[
            (
                "ai/main.xs",
                "// line 0\n// line 1\n// line 2\n// line 3\n// line 4\ninclude \"helper.xs\";\n// line 6\n// line 7\n// line 8\n// line 9\ninclude \"helper.xs\";\n",
            ),
            ("ai/helper.xs", "void helperFn() {}\n"),
        ],
        "ai/main.xs",
    );

    let ms = merged
        .find("helperFn")
        .expect("helperFn should be visible in merged view");
    assert!(
        matches!(ms.provenance, VisibilityProvenance::DirectInclude { .. }),
        "expected direct include provenance, got {:?}",
        ms.provenance
    );
    assert_eq!(
        ms.effective_line(),
        5,
        "effective_line should be the earliest root include line"
    );
}

#[test]
fn test_definition_in_current_file_uses_definition_line() {
    // Own-file symbols should report their definition line as the effective
    // line.
    let src = "void ownFn() {}\n";
    let (_tmp, _prj, merged, _current) = merged_fixture(&[("ai/own.xs", src)], "ai/own.xs");

    let ms = merged.find("ownFn").expect("ownFn should be visible");
    assert!(
        matches!(ms.provenance, VisibilityProvenance::OwnFile),
        "expected own-file provenance, got {:?}",
        ms.provenance
    );
    assert_eq!(
        ms.effective_line(),
        ms.symbol.selection_range.start.line,
        "own-file effective_line should equal the symbol's definition line"
    );
}
