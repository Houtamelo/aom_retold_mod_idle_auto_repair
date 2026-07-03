use std::path::Path;
use std::sync::Arc;

use tempfile::TempDir;
use tower_lsp_server::ls_types::SemanticTokenType;

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
    let own_table = symbols::build_full_symbol_table(source);
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

fn has_modifier(token: &semantic_tokens::Token, name: &str) -> bool {
    token.modifiers.iter().any(|m| m.as_str() == name)
}

#[test]
fn instance_field_reference_emits_variable_member_unmodded() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let vanilla_dir = game_root.join("ai");
    std::fs::create_dir_all(&vanilla_dir).unwrap();
    let vanilla_file = vanilla_dir.join("unit.xs");
    std::fs::write(&vanilla_file, "class Unit { int health = -1; }").unwrap();

    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = no_engine_api();
    let source = "void f() { obj.health = 10; }";
    let tokens = compute_for(source, None, &engine, &ws, &project);

    let field_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "health" && t.token_type == SemanticTokenType::VARIABLE)
        .expect("health field token missing");
    assert!(
        has_modifier(field_token, "member"),
        "field reference should have member modifier"
    );
    assert!(
        has_modifier(field_token, "unmodded"),
        "vanilla field reference should have unmodded modifier"
    );
}

#[test]
fn static_method_call_emits_function_member_unmodded() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let vanilla_dir = game_root.join("ai");
    std::fs::create_dir_all(&vanilla_dir).unwrap();
    let vanilla_file = vanilla_dir.join("unit.xs");
    std::fs::write(&vanilla_file, "class Unit { void takeDamage() {} }").unwrap();

    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = no_engine_api();
    let source = "void f() { Unit.takeDamage(); }";
    let tokens = compute_for(source, None, &engine, &ws, &project);

    let method_token = tokens
        .iter()
        .find(|t| {
            token_text(source, t) == "takeDamage" && t.token_type == SemanticTokenType::FUNCTION
        })
        .expect("takeDamage method token missing");
    assert!(
        has_modifier(method_token, "member"),
        "method reference should have member modifier"
    );
    assert!(
        has_modifier(method_token, "unmodded"),
        "vanilla method reference should have unmodded modifier"
    );
}

#[test]
fn modded_instance_field_reference_emits_variable_member_modded() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let mod_root = tmp.path().join("mod");
    let overlay_file = mod_root.join("game").join("ai").join("unit.xs");
    std::fs::create_dir_all(overlay_file.parent().unwrap()).unwrap();
    std::fs::write(&overlay_file, "class Unit { int health = -1; }").unwrap();

    let mut ws = workspace::Workspace::new(game_root.parent().unwrap().to_path_buf());
    let mod_uri = tower_lsp_server::ls_types::Uri::from_file_path(&mod_root).unwrap();
    ws.register_mod(mod_uri).unwrap();

    let entry = ws.mods().first().unwrap();
    let project = ws.build_virtual_project(entry);

    let engine = no_engine_api();
    let source = "class Unit { int health = -1; }\nvoid f() { obj.health = 10; }";
    let tokens = compute_for(source, Some(&overlay_file), &engine, &ws, &project);

    let field_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "health" && t.token_type == SemanticTokenType::VARIABLE)
        .expect("health field token missing");
    assert!(
        has_modifier(field_token, "member"),
        "field reference should have member modifier"
    );
    assert!(
        has_modifier(field_token, "modded"),
        "modded field reference should have modded modifier"
    );
}

#[test]
fn ambiguous_member_name_prefers_modded_origin() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path();
    let game_root = root.join("game");
    let mod_root = root.join("mod");
    let vanilla = game_root.join("ai").join("vanilla.xs");
    let overlay = mod_root.join("game").join("ai").join("modded.xs");
    std::fs::create_dir_all(vanilla.parent().unwrap()).unwrap();
    std::fs::create_dir_all(overlay.parent().unwrap()).unwrap();
    std::fs::write(&vanilla, "class Vanilla { int value = -1; }").unwrap();
    std::fs::write(&overlay, "class Modded { int value = -1; }").unwrap();

    let mut ws = workspace::Workspace::new(root.to_path_buf());
    let mod_uri = tower_lsp_server::ls_types::Uri::from_file_path(&mod_root).unwrap();
    ws.register_mod(mod_uri).unwrap();

    let entry = ws.mods().first().unwrap();
    let project = ws.build_virtual_project(entry);

    let engine = no_engine_api();
    let source = "void f() { obj.value = 10; }";
    let tokens = compute_for(source, None, &engine, &ws, &project);

    let field_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "value" && t.token_type == SemanticTokenType::VARIABLE)
        .expect("value field token missing");
    assert!(
        has_modifier(field_token, "member"),
        "field reference should have member modifier"
    );
    assert!(
        has_modifier(field_token, "modded"),
        "ambiguous member should prefer modded origin"
    );
}

#[test]
fn engine_class_member_reference_emits_variable_member_engine() {
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = no_engine_api();

    let current = tmp.path().join("outside_game.xs");
    let source = "class Unit { int health = -1; }\nvoid f() { obj.health = 10; }";
    let tokens = compute_for(source, Some(&current), &engine, &ws, &project);

    let field_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "health" && t.token_type == SemanticTokenType::VARIABLE)
        .expect("health field token missing");
    assert!(
        has_modifier(field_token, "member"),
        "field reference should have member modifier"
    );
    assert!(
        has_modifier(field_token, "engine"),
        "engine class member should have engine modifier"
    );
}

#[test]
fn member_declaration_emits_member_modifier() {
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = no_engine_api();

    let current = tmp.path().join("outside_game.xs");
    let source = "class Unit { int health = -1; void heal() {} }";
    let tokens = compute_for(source, Some(&current), &engine, &ws, &project);

    let field_decl = tokens
        .iter()
        .find(|t| token_text(source, t) == "health" && t.token_type == SemanticTokenType::VARIABLE)
        .expect("field declaration token missing");
    assert!(
        has_modifier(field_decl, "member"),
        "field declaration inside class should carry member modifier"
    );

    let method_decl = tokens
        .iter()
        .find(|t| token_text(source, t) == "heal" && t.token_type == SemanticTokenType::FUNCTION)
        .expect("method declaration token missing");
    assert!(
        has_modifier(method_decl, "member"),
        "method declaration inside class should carry member modifier"
    );
}
