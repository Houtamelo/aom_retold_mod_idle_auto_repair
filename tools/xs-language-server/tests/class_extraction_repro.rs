use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use tempfile::TempDir;
use tower_lsp::lsp_types::SemanticTokenType;

use xs_language_server::{engine_api, parser, semantic_tokens, symbols, workspace};

fn token_text(source: &str, token: &semantic_tokens::Token) -> String {
    let line = source.lines().nth(token.line as usize).unwrap_or("");
    line.chars()
        .skip(token.char as usize)
        .take(token.len as usize)
        .collect()
}

fn compute_for(
    source: &str,
    current_file: Option<&Path>,
    engine: &engine_api::SharedEngineApi,
    ws: &workspace::Workspace,
    project: &workspace::VirtualProject,
) -> Vec<semantic_tokens::Token> {
    let tree = parser::parse(source).expect("parse");
    let own_table = symbols::build_full_symbol_table(&tree, source);
    let cache_dir = TempDir::new().unwrap();
    let member_index = semantic_tokens::MemberIndex::build(
        &own_table,
        ws,
        project,
        cache_dir.path(),
        current_file,
    );
    semantic_tokens::compute_tokens(
        source,
        current_file,
        &own_table,
        None,
        engine,
        ws,
        project,
        &member_index,
    )
}

fn no_engine_api() -> engine_api::SharedEngineApi {
    Arc::new(engine_api::EngineApi {
        syscalls: vec![],
        aiplans: vec![],
    })
}

#[test]
fn vanilla_class_reference_emits_type_unmodded() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let vanilla_dir = game_root.join("ai");
    std::fs::create_dir_all(&vanilla_dir).unwrap();
    let vanilla_file = vanilla_dir.join("vanilla.xs");
    std::fs::write(&vanilla_file, "class VanillaClass {}").unwrap();

    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = no_engine_api();
    let source = "class VanillaClass {}\nVanillaClass x;";
    let tokens = compute_for(source, Some(&vanilla_file), &engine, &ws, &project);

    let class_token = tokens
        .iter()
        .find(|t| {
            token_text(source, t) == "VanillaClass" && t.token_type == SemanticTokenType::TYPE
        })
        .expect("VanillaClass type token missing");
    assert!(
        class_token
            .modifiers
            .iter()
            .any(|m| m.as_str() == "unmodded"),
        "vanilla class reference should have unmodded modifier"
    );
}

#[test]
fn modded_class_reference_emits_type_modded() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let mod_root = tmp.path().join("mod");
    let overlay_file = mod_root.join("game").join("ai").join("modded.xs");
    std::fs::create_dir_all(overlay_file.parent().unwrap()).unwrap();
    std::fs::write(&overlay_file, "class ModdedClass {}").unwrap();

    let mut ws = workspace::Workspace::new(game_root.parent().unwrap().to_path_buf());
    let mod_uri = tower_lsp::lsp_types::Url::from_file_path(&mod_root).unwrap();
    ws.register_mod(mod_uri).unwrap();

    let entry = ws.mods().first().unwrap();
    let project = ws.build_virtual_project(entry);

    let engine = no_engine_api();
    let source = "class ModdedClass {}\nModdedClass x;";
    let tokens = compute_for(source, Some(&overlay_file), &engine, &ws, &project);

    let class_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "ModdedClass" && t.token_type == SemanticTokenType::TYPE)
        .expect("ModdedClass type token missing");
    assert!(
        class_token.modifiers.iter().any(|m| m.as_str() == "modded"),
        "modded class reference should have modded modifier"
    );
}

#[test]
fn unknown_class_reference_falls_back_to_engine() {
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = no_engine_api();
    let source = "UnknownClass x;";
    let tokens = compute_for(source, None, &engine, &ws, &project);

    let class_token = tokens
        .iter()
        .find(|t| {
            token_text(source, t) == "UnknownClass" && t.token_type == SemanticTokenType::TYPE
        })
        .expect("UnknownClass type token missing");
    assert!(
        class_token.modifiers.iter().any(|m| m.as_str() == "engine"),
        "unknown class reference should fall back to engine modifier"
    );
}

#[test]
fn class_declaration_does_not_crash_walker() {
    let source = "class MyClass { int field = 0; void method() {} }";
    let tree = parser::parse(source).expect("parse");
    let table = symbols::build_symbol_table(&tree, source);
    let class_syms: Vec<_> = table
        .symbols
        .iter()
        .filter(|s| s.kind == symbols::SymbolKind::Class)
        .collect();
    assert_eq!(
        class_syms.len(),
        1,
        "exactly one Class symbol should be extracted"
    );
    assert_eq!(class_syms[0].name, "MyClass");
    // The full range should be non-empty (start before end).
    let range = class_syms[0].full_range;
    assert!(
        range.start.line < range.end.line
            || (range.start.line == range.end.line && range.start.character < range.end.character),
        "class full range should be non-empty"
    );
}

#[test]
fn many_class_declarations_build_quickly() {
    let mut source = String::new();
    for i in 0..60 {
        source.push_str(&format!("class Class{} {{}}\n", i));
    }
    let tree = parser::parse(&source).expect("parse");

    let start = Instant::now();
    let table = symbols::build_symbol_table(&tree, &source);
    let elapsed = start.elapsed();

    assert_eq!(
        table
            .symbols
            .iter()
            .filter(|s| s.kind == symbols::SymbolKind::Class)
            .count(),
        60,
        "all 60 class declarations should be extracted"
    );
    assert!(
        elapsed.as_millis() < 100,
        "symbol table build should complete in < 100 ms, took {:?}",
        elapsed
    );
}
