//! TDD repro tests for root-based merged views and file rebasing.
//!
//! PR-3 adds `MergedView::build_from_root` and `MergedView::rebase_to_file` so
//! that a file diagnosed as part of a root's include chain can see the full
//! chain's symbols while keeping its own source and symbol table.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tempfile::TempDir;

use xs_language_server::cache::{CacheError, PerRootMergedViewCache};
use xs_language_server::merged_view::MergedView;
use xs_language_server::symbols;
use xs_language_server::workspace::{VirtualProject, Workspace};

/// Monotonically increasing fixture id so parallel tests never share temp dirs.
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

/// Build a workspace over a temporary `game/` tree with the given files.
fn workspace_for(files: &[(&str, &str)]) -> (TempDir, Workspace, VirtualProject) {
    let id = next_fixture_id();
    let tmp = TempDir::with_prefix(format!("aomr_merged_view_root_{id}_")).unwrap();
    let root = tmp.path().to_path_buf();
    write_game_files(&root, files);

    let ws = Workspace::new(root.clone());
    let project = VirtualProject::default();
    (tmp, ws, project)
}

fn abs_path(root: &Path, rel: &str) -> PathBuf {
    root.join("game").join(rel)
}

#[test]
fn test_build_from_root_loads_root_chain() -> Result<(), CacheError> {
    let (_tmp, ws, project) = workspace_for(&[
        (
            "ai/root.xs",
            "include \"middle.xs\";\ninclude \"sibling.xs\";\nvoid root() {}\n",
        ),
        ("ai/middle.xs", "void middle() {}\n"),
        ("ai/sibling.xs", "void sibling() {}\n"),
    ]);
    let root_path = abs_path(_tmp.path(), "ai/root.xs");
    let middle_path = abs_path(_tmp.path(), "ai/middle.xs");
    let sibling_path = abs_path(_tmp.path(), "ai/sibling.xs");

    let cache = PerRootMergedViewCache::new(TempDir::new().unwrap().path());
    let view = MergedView::build_from_root(&root_path, &ws, &project, &cache)?;

    let closure: Vec<_> = view.closure_files().cloned().collect();
    assert!(
        closure.contains(&root_path),
        "closure must contain root: {closure:?}"
    );
    assert!(
        closure.contains(&middle_path),
        "closure must contain middle: {closure:?}"
    );
    assert!(
        closure.contains(&sibling_path),
        "closure must contain sibling: {closure:?}"
    );

    let view2 = MergedView::build_from_root(&root_path, &ws, &project, &cache)?;
    assert!(
        Arc::ptr_eq(&view, &view2),
        "second build_from_root for unchanged closure must return the same Arc"
    );

    Ok(())
}

#[test]
fn test_rebase_to_file_preserves_root_chain() -> Result<(), CacheError> {
    let (_tmp, ws, project) = workspace_for(&[
        (
            "ai/root.xs",
            "include \"middle.xs\";\ninclude \"sibling.xs\";\nvoid root() {}\n",
        ),
        ("ai/middle.xs", "void middle() {}\n"),
        ("ai/sibling.xs", "void sibling() {}\n"),
    ]);
    let root_path = abs_path(_tmp.path(), "ai/root.xs");
    let middle_path = abs_path(_tmp.path(), "ai/middle.xs");
    let sibling_path = abs_path(_tmp.path(), "ai/sibling.xs");

    let cache = PerRootMergedViewCache::new(TempDir::new().unwrap().path());
    let root_view = MergedView::build_from_root(&root_path, &ws, &project, &cache)?;

    let middle_source = std::fs::read_to_string(&middle_path).unwrap();
    let middle_table = symbols::build_symbol_table(&middle_source);
    let rebased = root_view.rebase_to_file(&middle_path, middle_source.clone(), middle_table);

    assert_eq!(
        rebased.current_file(),
        middle_path,
        "rebased view's current_file must be the target file"
    );
    assert_eq!(
        rebased.closure_files().cloned().collect::<Vec<_>>(),
        root_view.closure_files().cloned().collect::<Vec<_>>(),
        "rebased view must carry the same closure as the root view"
    );
    let closure: Vec<_> = rebased.closure_files().cloned().collect();
    assert!(
        closure.contains(&root_path),
        "rebased closure must still contain root: {closure:?}"
    );
    assert!(
        closure.contains(&sibling_path),
        "rebased closure must still contain sibling: {closure:?}"
    );

    Ok(())
}

#[test]
fn test_rebased_view_exposes_sibling_symbol() -> Result<(), CacheError> {
    let (_tmp, ws, project) = workspace_for(&[
        (
            "ai/root.xs",
            "include \"middle.xs\";\ninclude \"sibling.xs\";\nvoid root() {}\n",
        ),
        ("ai/middle.xs", "void gizmo() {}\n"),
        ("ai/sibling.xs", "void sibling() {}\n"),
    ]);
    let root_path = abs_path(_tmp.path(), "ai/root.xs");
    let sibling_path = abs_path(_tmp.path(), "ai/sibling.xs");

    let cache = PerRootMergedViewCache::new(TempDir::new().unwrap().path());
    let root_view = MergedView::build_from_root(&root_path, &ws, &project, &cache)?;

    let sibling_source = std::fs::read_to_string(&sibling_path).unwrap();
    let sibling_table = symbols::build_symbol_table(&sibling_source);
    let rebased = root_view.rebase_to_file(&sibling_path, sibling_source, sibling_table);

    let gizmo = rebased
        .find("gizmo")
        .expect("sibling file must see middle symbol via root's include chain");
    assert_eq!(gizmo.symbol.name, "gizmo");
    assert!(
        !matches!(
            gizmo.provenance,
            xs_language_server::merged_view::VisibilityProvenance::OwnFile
        ),
        "gizmo is defined in middle.xs, not in sibling.xs"
    );

    Ok(())
}

#[test]
fn test_rebased_view_keeps_own_table_for_current_file() -> Result<(), CacheError> {
    let (_tmp, ws, project) = workspace_for(&[
        (
            "ai/root.xs",
            "include \"leaf.xs\";\nvoid root() {}\n",
        ),
        ("ai/leaf.xs", "void leaf_only() {}\n"),
    ]);
    let root_path = abs_path(_tmp.path(), "ai/root.xs");
    let leaf_path = abs_path(_tmp.path(), "ai/leaf.xs");

    let cache = PerRootMergedViewCache::new(TempDir::new().unwrap().path());
    let root_view = MergedView::build_from_root(&root_path, &ws, &project, &cache)?;

    let leaf_source = "void leaf_only() {}\n".to_string();
    let leaf_table = symbols::build_symbol_table(&leaf_source);
    let rebased = root_view.rebase_to_file(&leaf_path, leaf_source, leaf_table.clone());

    assert_eq!(
        rebased.current_file(),
        leaf_path,
        "rebased view's current_file must be the target file"
    );

    let own = rebased.own_table();
    assert!(
        own.find("leaf_only").is_some(),
        "rebased own_table must contain the target file's own symbol"
    );
    assert!(
        root_view.own_table().find("leaf_only").is_none(),
        "root view's own_table must NOT contain the leaf's own symbol"
    );

    // The rebased view still carries the root chain's symbols.
    assert!(rebased.find("root").is_some(), "rebased view must still see root's own symbols");

    Ok(())
}
