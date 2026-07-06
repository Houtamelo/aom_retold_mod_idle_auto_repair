//! Integration tests for `textDocument/documentLink`.

use std::path::PathBuf;

use tempfile::TempDir;
use tower_lsp_server::ls_types::Uri;
use xs_language_server::document_link::document_links;
use xs_language_server::workspace::{VirtualProject, Workspace};

fn ai_fixture() -> (TempDir, PathBuf, Workspace, VirtualProject) {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().to_path_buf();

    let core = root.join("game").join("ai").join("core").join("core.xs");
    std::fs::create_dir_all(core.parent().unwrap()).unwrap();
    std::fs::write(&core, "void vanillaCore() {}\n").unwrap();

    let ws = Workspace::new(root.clone());
    let project = VirtualProject::default();
    (tmp, root, ws, project)
}

#[test]
fn single_resolvable_include_yields_one_link() {
    let (_tmp, root, ws, project) = ai_fixture();
    let main = root.join("game").join("ai").join("main.xs");
    std::fs::write(&main, "include \"core/core.xs\";\n").unwrap();

    let source = std::fs::read_to_string(&main).unwrap();
    let links = document_links(&source, &main, &ws, &project);
    assert_eq!(links.len(), 1, "expected exactly one document link, got {:?}", links);

    let link = &links[0];
    assert_eq!(link.range.start.line, 0);
    assert_eq!(link.range.start.character, 9);
    assert_eq!(link.range.end.line, 0);
    assert_eq!(link.range.end.character, 21);

    let target = link
        .target
        .as_ref()
        .expect("link should have a target URI")
        .to_file_path()
        .expect("target should be a file URI");
    assert!(
        target.to_string_lossy().ends_with("core/core.xs"),
        "expected target to end with core/core.xs, got {}",
        target.display()
    );
}

#[test]
fn unresolved_include_returns_empty() {
    let (_tmp, root, ws, project) = ai_fixture();
    let main = root.join("game").join("ai").join("main.xs");
    std::fs::write(&main, "include \"does/not/exist.xs\";\n").unwrap();

    let source = std::fs::read_to_string(&main).unwrap();
    let links = document_links(&source, &main, &ws, &project);
    assert!(links.is_empty(), "unresolved include should produce no links");
}

#[test]
fn three_includes_one_missing_returns_two_links() {
    let (_tmp, root, ws, project) = ai_fixture();
    let main = root.join("game").join("ai").join("main.xs");
    std::fs::write(
        &main,
        "include \"core/core.xs\";\ninclude \"missing.xs\";\ninclude \"core/core.xs\";\n",
    )
    .unwrap();

    let source = std::fs::read_to_string(&main).unwrap();
    let links = document_links(&source, &main, &ws, &project);
    assert_eq!(
        links.len(),
        2,
        "expected two links (one missing target excluded), got {:?}",
        links
    );
}

#[test]
fn file_with_no_includes_returns_empty() {
    let (_tmp, root, ws, project) = ai_fixture();
    let main = root.join("game").join("ai").join("main.xs");
    std::fs::write(&main, "void hello() {}\n").unwrap();

    let source = std::fs::read_to_string(&main).unwrap();
    let links = document_links(&source, &main, &ws, &project);
    assert!(links.is_empty(), "file with no includes should produce no links");
}

#[test]
fn mod_overlay_is_preferred_over_vanilla() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().to_path_buf();

    let vanilla = root.join("game").join("ai").join("core").join("core.xs");
    let overlay = root
        .join("mod_a")
        .join("game")
        .join("ai")
        .join("core")
        .join("core.xs");
    let main = root
        .join("mod_a")
        .join("game")
        .join("ai")
        .join("main.xs");

    std::fs::create_dir_all(vanilla.parent().unwrap()).unwrap();
    std::fs::create_dir_all(overlay.parent().unwrap()).unwrap();
    std::fs::write(&vanilla, "void vanillaCore() {}\n").unwrap();
    std::fs::write(&overlay, "void modCore() {}\n").unwrap();
    std::fs::write(&main, "include \"core/core.xs\";\n").unwrap();

    let mut ws = Workspace::new(root.clone());
    ws.register_mod(Uri::from_file_path(root.join("mod_a")).unwrap()).unwrap();
    let project = ws.build_virtual_project(ws.mods().first().unwrap());

    let source = std::fs::read_to_string(&main).unwrap();
    let links = document_links(&source, &main, &ws, &project);
    assert_eq!(links.len(), 1);
    let target = links[0]
        .target
        .as_ref()
        .expect("target URI")
        .to_file_path()
        .expect("file URI");
    assert_eq!(target, overlay, "document link should target the mod overlay copy");
    assert_ne!(target, vanilla, "document link should not target the vanilla file");
}
