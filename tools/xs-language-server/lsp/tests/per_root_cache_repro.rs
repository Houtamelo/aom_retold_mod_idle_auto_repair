//! TDD repro tests for the per-root `MergedView` cache.
//!
//! The cache is session-only and in-memory. It is keyed by `(root_path,
//! closure_content_hash)` so that unrelated roots stay warm while any root whose
//! include closure changed is rebuilt on the next access.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tempfile::TempDir;

use xs_language_server::cache::{CacheError, CachedRootView, PerRootMergedViewCache};
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

/// Build a fresh cache instance pointing at a fresh per-file parse cache dir.
fn new_cache() -> PerRootMergedViewCache {
    let dir = TempDir::new().unwrap();
    PerRootMergedViewCache::new(dir.path())
}

/// Build a workspace over a temporary `game/` tree with the given files.
fn workspace_for(files: &[(&str, &str)]) -> (TempDir, Workspace, VirtualProject) {
    let id = next_fixture_id();
    let tmp = TempDir::with_prefix(format!("aomr_per_root_cache_{id}_")).unwrap();
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
fn test_cache_returns_same_arc_on_identical_closure_hash() -> Result<(), CacheError> {
    let (_tmp, ws, project) = workspace_for(&[
        ("ai/root.xs", "include \"helper.xs\";\nvoid root() {}\n"),
        ("ai/helper.xs", "void helper() {}\n"),
    ]);
    let root_path = abs_path(_tmp.path(), "ai/root.xs");

    let cache = new_cache();
    let v1 = cache.get_or_build(&root_path, &ws, &project)?;
    let v2 = cache.get_or_build(&root_path, &ws, &project)?;

    assert!(
        Arc::ptr_eq(&v1, &v2),
        "same root and unchanged closure must return the same Arc<MergedView>"
    );
    Ok(())
}

#[test]
fn test_cache_rebuilds_after_closure_change() -> Result<(), CacheError> {
    let (_tmp, ws, project) = workspace_for(&[
        ("ai/root.xs", "include \"helper.xs\";\nvoid root() {}\n"),
        ("ai/helper.xs", "void helper() {}\n"),
    ]);
    let root_path = abs_path(_tmp.path(), "ai/root.xs");
    let helper_path = abs_path(_tmp.path(), "ai/helper.xs");

    let cache = new_cache();
    let v1 = cache.get_or_build(&root_path, &ws, &project)?;

    // Mutate a file in the root's closure; this changes the closure content hash.
    std::fs::write(&helper_path, "void helper_changed() {}\n").unwrap();

    let v2 = cache.get_or_build(&root_path, &ws, &project)?;

    assert!(
        !Arc::ptr_eq(&v1, &v2),
        "mutating a closure file must produce a new Arc<MergedView>"
    );
    // Sanity: the new view reflects the changed symbol.
    assert!(v2.find("helper_changed").is_some());
    Ok(())
}

#[test]
fn test_cache_invalidates_entry_containing_changed_path() -> Result<(), CacheError> {
    let (_tmp, ws, project) = workspace_for(&[
        ("ai/r1.xs", "include \"r1_helper.xs\";\nvoid r1() {}\n"),
        ("ai/r1_helper.xs", "void r1_helper() {}\n"),
        ("ai/r2.xs", "include \"r2_helper.xs\";\nvoid r2() {}\n"),
        ("ai/r2_helper.xs", "void r2_helper() {}\n"),
        ("ai/r3.xs", "include \"r3_helper.xs\";\nvoid r3() {}\n"),
        ("ai/r3_helper.xs", "void r3_helper() {}\n"),
    ]);
    let r1 = abs_path(_tmp.path(), "ai/r1.xs");
    let r2 = abs_path(_tmp.path(), "ai/r2.xs");
    let r3 = abs_path(_tmp.path(), "ai/r3.xs");
    let r2_helper = abs_path(_tmp.path(), "ai/r2_helper.xs");

    let cache = new_cache();
    let ar1 = cache.get_or_build(&r1, &ws, &project)?;
    let ar2 = cache.get_or_build(&r2, &ws, &project)?;
    let ar3 = cache.get_or_build(&r3, &ws, &project)?;

    cache.invalidate_for_paths(&[r2_helper.clone()]);

    let br1 = cache.get_or_build(&r1, &ws, &project)?;
    let br2 = cache.get_or_build(&r2, &ws, &project)?;
    let br3 = cache.get_or_build(&r3, &ws, &project)?;

    assert!(
        Arc::ptr_eq(&ar1, &br1),
        "r1's closure did not change; the cached Arc must still be returned"
    );
    assert!(
        !Arc::ptr_eq(&ar2, &br2),
        "r2's closure contained the changed path; its entry must have been dropped"
    );
    assert!(
        Arc::ptr_eq(&ar3, &br3),
        "r3's closure did not change; the cached Arc must still be returned"
    );
    Ok(())
}

#[test]
fn test_unchanged_root_view_reused_while_changed_root_rebuilds() -> Result<(), CacheError> {
    let (_tmp, ws, project) = workspace_for(&[
        ("ai/r1.xs", "include \"r1_helper.xs\";\nvoid r1() {}\n"),
        ("ai/r1_helper.xs", "void r1_helper() {}\n"),
        ("ai/r2.xs", "include \"r2_helper.xs\";\nvoid r2() {}\n"),
        ("ai/r2_helper.xs", "void r2_helper() {}\n"),
        ("ai/r3.xs", "include \"r3_helper.xs\";\nvoid r3() {}\n"),
        ("ai/r3_helper.xs", "void r3_helper() {}\n"),
    ]);
    let r1 = abs_path(_tmp.path(), "ai/r1.xs");
    let r2 = abs_path(_tmp.path(), "ai/r2.xs");
    let r3 = abs_path(_tmp.path(), "ai/r3.xs");
    let r2_helper = abs_path(_tmp.path(), "ai/r2_helper.xs");

    let cache = new_cache();
    let ar1 = cache.get_or_build(&r1, &ws, &project)?;
    let ar2 = cache.get_or_build(&r2, &ws, &project)?;
    let ar3 = cache.get_or_build(&r3, &ws, &project)?;

    // Change a file only in r2's closure, then invalidate by that path.
    std::fs::write(&r2_helper, "void r2_helper_changed() {}\n").unwrap();
    cache.invalidate_for_paths(&[r2_helper.clone()]);

    let br1 = cache.get_or_build(&r1, &ws, &project)?;
    let br2 = cache.get_or_build(&r2, &ws, &project)?;
    let br3 = cache.get_or_build(&r3, &ws, &project)?;

    assert!(
        Arc::ptr_eq(&ar1, &br1),
        "unchanged r1 must reuse its cached view"
    );
    assert!(
        !Arc::ptr_eq(&ar2, &br2),
        "changed r2 must rebuild and return a new view"
    );
    assert!(
        Arc::ptr_eq(&ar3, &br3),
        "unchanged r3 must reuse its cached view"
    );
    Ok(())
}

#[test]
fn test_cache_stores_closure_file_list() -> Result<(), CacheError> {
    let (_tmp, ws, project) = workspace_for(&[
        (
            "ai/root.xs",
            "include \"a.xs\";\ninclude \"b.xs\";\nvoid root() {}\n",
        ),
        ("ai/a.xs", "void a() {}\n"),
        ("ai/b.xs", "void b() {}\n"),
    ]);
    let root_path = abs_path(_tmp.path(), "ai/root.xs");
    let a = abs_path(_tmp.path(), "ai/a.xs");
    let b = abs_path(_tmp.path(), "ai/b.xs");

    let cache = new_cache();
    let _view = cache.get_or_build(&root_path, &ws, &project)?;

    let entry: CachedRootView = cache
        .entry(&root_path)
        .expect("cache must hold an entry for the built root");

    let mut stored: Vec<PathBuf> = entry.closure_files().iter().cloned().collect();
    stored.sort();

    let expected = vec![a, b, root_path];
    assert_eq!(
        stored, expected,
        "closure_files must contain the root and every resolved include target, sorted"
    );
    Ok(())
}
