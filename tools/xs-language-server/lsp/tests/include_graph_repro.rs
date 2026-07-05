//! TDD repro tests for the project-wide reverse-include graph.
//!
//! These tests drive `ReverseIncludeGraph::build` from a single root's forward
//! `IncludeGraph` (produced by `MergedView::build`). PR-2 will later feed every
//! root in the workspace into the same structure to get a project-wide view.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use tempfile::TempDir;

use xs_language_server::include_graph::ReverseIncludeGraph;
use xs_language_server::merged_view::MergedView;
use xs_language_server::symbols;
use xs_language_server::workspace::{VirtualProject as WorkspaceVirtualProject, Workspace};

/// Monotonically increasing fixture id so parallel tests never share temp dirs.
/// Replaces the previous `std::process::id()` based scheme, which races when
/// cargo runs integration tests concurrently in the same process.
static FIXTURE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_fixture_id() -> u64 {
    FIXTURE_COUNTER.fetch_add(1, Ordering::SeqCst)
}

fn write_game_files(root: &Path, files: &[(&str, &str)]) {
    for (rel, content) in files {
        let path = root.join("game").join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
    }
}

/// Build a `ReverseIncludeGraph` from the forward include graph of `root_rel`.
fn build_graph_for_root(
    files: &[(&str, &str)],
    root_rel: &str,
) -> (TempDir, ReverseIncludeGraph, PathBuf) {
    let id = next_fixture_id();
    let tmp = TempDir::with_prefix(format!("aomr_include_graph_{id}_")).unwrap();
    let root = tmp.path().to_path_buf();
    write_game_files(&root, files);

    let ws = Workspace::new(root.clone());
    let project = WorkspaceVirtualProject::default();
    let cache_dir = TempDir::new().unwrap();

    let root_path = root.join("game").join(root_rel);
    let source = std::fs::read_to_string(&root_path).unwrap();
    let own = symbols::build_symbol_table(&source);
    let merged = MergedView::build(&root_path, &source, &own, &ws, &project, cache_dir.path());
    let graph = ReverseIncludeGraph::build(merged.graph());

    (tmp, graph, root_path)
}

fn abs_path(root: &Path, rel: &str) -> PathBuf {
    root.join("game").join(rel)
}

#[test]
fn test_roots_that_include_empty_for_orphan_file() {
    let (_tmp, graph, orphan) = build_graph_for_root(
        &[("ai/foo.xs", "void foo() {}\n")],
        "ai/foo.xs",
    );
    assert_eq!(graph.roots_that_include(&orphan), Vec::<PathBuf>::new());
}

#[test]
fn test_roots_that_include_finds_single_includer() {
    let (tmp, graph, _root) = build_graph_for_root(
        &[
            ("ai/root.xs", "include \"inner.xs\";\n"),
            ("ai/inner.xs", "void inner() {}\n"),
        ],
        "ai/root.xs",
    );
    let inner = abs_path(tmp.path(), "ai/inner.xs");
    let roots = graph.roots_that_include(&inner);
    assert_eq!(roots, vec![abs_path(tmp.path(), "ai/root.xs")]);
}

#[test]
fn test_roots_that_include_finds_transitive_chain() {
    let (tmp, graph, _root) = build_graph_for_root(
        &[
            ("ai/a.xs", "include \"b.xs\";\n"),
            ("ai/b.xs", "include \"c.xs\";\n"),
            ("ai/c.xs", "void c() {}\n"),
        ],
        "ai/a.xs",
    );
    let c = abs_path(tmp.path(), "ai/c.xs");
    let roots = graph.roots_that_include(&c);
    assert_eq!(roots, vec![abs_path(tmp.path(), "ai/a.xs")]);
}

#[test]
fn test_roots_that_include_handles_diamond() {
    let (tmp, graph, _root) = build_graph_for_root(
        &[
            (
                "ai/root.xs",
                "include \"a.xs\";\ninclude \"b.xs\";\n",
            ),
            ("ai/a.xs", "include \"c.xs\";\n"),
            ("ai/b.xs", "include \"c.xs\";\n"),
            ("ai/c.xs", "void c() {}\n"),
        ],
        "ai/root.xs",
    );
    let c = abs_path(tmp.path(), "ai/c.xs");
    let roots = graph.roots_that_include(&c);
    assert_eq!(roots, vec![abs_path(tmp.path(), "ai/root.xs")]);
}

#[test]
fn test_orphan_file_is_own_root_via_empty_return() {
    let (_tmp, graph, _orphan) = build_graph_for_root(
        &[("ai/existing.xs", "void existing() {}\n")],
        "ai/existing.xs",
    );
    let nonexistent = PathBuf::from("/does/not/exist.xs");
    assert_eq!(graph.roots_that_include(&nonexistent), Vec::<PathBuf>::new());
}

#[test]
fn test_direct_includers_returns_only_first_hop() {
    let (tmp, graph, _root) = build_graph_for_root(
        &[
            (
                "ai/root.xs",
                "include \"a.xs\";\ninclude \"b.xs\";\n",
            ),
            ("ai/a.xs", "include \"c.xs\";\n"),
            ("ai/b.xs", "include \"c.xs\";\n"),
            ("ai/c.xs", "void c() {}\n"),
        ],
        "ai/root.xs",
    );
    let c = abs_path(tmp.path(), "ai/c.xs");
    let direct: Vec<_> = graph.direct_includers(&c).to_vec();
    assert_eq!(
        direct,
        vec![
            abs_path(tmp.path(), "ai/a.xs"),
            abs_path(tmp.path(), "ai/b.xs"),
        ]
    );
}

#[test]
fn test_len_counts_distinct_files() {
    let (_tmp, graph, _root) = build_graph_for_root(
        &[
            (
                "ai/root.xs",
                "include \"a.xs\";\ninclude \"b.xs\";\n",
            ),
            ("ai/a.xs", "include \"c.xs\";\n"),
            ("ai/b.xs", "include \"c.xs\";\n"),
            ("ai/c.xs", "void c() {}\n"),
        ],
        "ai/root.xs",
    );
    assert_eq!(graph.len(), 4);
}
