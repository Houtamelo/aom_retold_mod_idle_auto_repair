use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use tempfile::TempDir;
use tower_lsp_server::ls_types::Uri;

use xs_language_server::{engine_api, parser, semantic_tokens, symbols, workspace};

// Convenience helper for tests that only need workspace symbols.
#[allow(dead_code)]
fn no_engine_api() -> engine_api::SharedEngineApi {
    Arc::new(engine_api::EngineApi {
        syscalls: vec![],
        aiplans: vec![],
    })
}

fn table_for(source: &str) -> symbols::SymbolTable {
    symbols::build_symbol_table(source)
}

fn game_workspace(game_root: &Path) -> (workspace::Workspace, workspace::VirtualProject) {
    // Workspace::new expects the directory that contains `game/`, not the game folder itself.
    let ws = workspace::Workspace::new(game_root.parent().unwrap().to_path_buf());
    let project = workspace::VirtualProject::default();
    (ws, project)
}

fn mod_workspace(
    root: &Path,
    mod_path: &Path,
) -> (workspace::Workspace, workspace::VirtualProject) {
    let mut ws = workspace::Workspace::new(root.to_path_buf());
    let mod_uri = Uri::from_file_path(mod_path).unwrap();
    ws.register_mod(mod_uri).unwrap();
    let entry = ws.mods().first().unwrap();
    let project = ws.build_virtual_project(entry);
    (ws, project)
}

#[test]
fn class_field_is_extracted() {
    let source = "class Foo { int value = 0; }";
    let table = table_for(source);
    let field = table
        .symbols
        .iter()
        .find(|s| s.kind == symbols::SymbolKind::ClassField && s.name == "value")
        .expect("ClassField 'value' should be extracted");
    assert_eq!(field.class_owner.as_deref(), Some("Foo"));
}

#[test]
fn class_method_is_extracted() {
    let source = "class Foo { void bar() {} }";
    let table = table_for(source);
    let method = table
        .symbols
        .iter()
        .find(|s| s.kind == symbols::SymbolKind::ClassMethod && s.name == "bar")
        .expect("ClassMethod 'bar' should be extracted");
    assert_eq!(method.class_owner.as_deref(), Some("Foo"));
}

#[test]
fn cross_file_field_origin_is_unmodded_when_vanilla() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let file = game_root.join("ai").join("foo.xs");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "class Foo { int value = -1; }").unwrap();

    let (ws, project) = game_workspace(&game_root);
    let cache_dir = TempDir::new().unwrap();
    let member_index = semantic_tokens::MemberIndex::build(
        &symbols::SymbolTable::default(),
        &ws,
        &project,
        cache_dir.path(),
        None,
    );
    assert_eq!(
        member_index.origin("value"),
        Some(semantic_tokens::Origin::Unmodded)
    );
}

#[test]
fn cross_file_method_origin_is_unmodded_when_vanilla() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let file = game_root.join("ai").join("foo.xs");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "class Foo { void work() {} }").unwrap();

    let (ws, project) = game_workspace(&game_root);
    let cache_dir = TempDir::new().unwrap();
    let member_index = semantic_tokens::MemberIndex::build(
        &symbols::SymbolTable::default(),
        &ws,
        &project,
        cache_dir.path(),
        None,
    );
    assert_eq!(
        member_index.origin("work"),
        Some(semantic_tokens::Origin::Unmodded)
    );
}

#[test]
fn ambiguous_member_prefers_modded_origin() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let game_root = root.join("game");
    let mod_root = root.join("mod");
    let vanilla = game_root.join("ai").join("foo.xs");
    let overlay = mod_root.join("game").join("ai").join("foo.xs");
    std::fs::create_dir_all(vanilla.parent().unwrap()).unwrap();
    std::fs::create_dir_all(overlay.parent().unwrap()).unwrap();
    std::fs::write(&vanilla, "class Vanilla { int value = -1; }").unwrap();
    std::fs::write(&overlay, "class Modded { int value = -1; }").unwrap();

    let (ws, project) = mod_workspace(root, &mod_root);
    let cache_dir = TempDir::new().unwrap();
    let member_index = semantic_tokens::MemberIndex::build(
        &symbols::SymbolTable::default(),
        &ws,
        &project,
        cache_dir.path(),
        None,
    );
    assert_eq!(
        member_index.origin("value"),
        Some(semantic_tokens::Origin::Modded)
    );
}

#[test]
fn many_classes_with_members_build_quickly() {
    let mut source = String::new();
    for i in 0..60 {
        source.push_str(&format!(
            "class Class{} {{ int field{} = -1; void method{}() {{}} }}\n",
            i, i, i
        ));
    }
    let (_cst, _diags) = parser::parse(&source);

    let start = Instant::now();
    let table = symbols::build_symbol_table(&source);
    let elapsed = start.elapsed();

    assert_eq!(
        table
            .symbols
            .iter()
            .filter(|s| s.kind == symbols::SymbolKind::ClassField)
            .count(),
        60,
        "all 60 class fields should be extracted"
    );
    assert_eq!(
        table
            .symbols
            .iter()
            .filter(|s| s.kind == symbols::SymbolKind::ClassMethod)
            .count(),
        60,
        "all 60 class methods should be extracted"
    );
    assert!(
        elapsed.as_millis() < 200,
        "symbol table build should complete in < 200 ms, took {:?}",
        elapsed
    );
}

#[test]
fn malformed_class_body_does_not_panic() {
    // Missing semicolon and an invalid nested class-like token:
    // the parser may produce ERROR nodes inside the body.
    let source = "class Bad { int x int y = 0; }";
    let (_cst, _diags) = parser::parse(source);
    // parsing itself must not panic; build_symbol_table must not panic even if parse produces diagnostics.
    let _table = symbols::build_symbol_table(source);
}
