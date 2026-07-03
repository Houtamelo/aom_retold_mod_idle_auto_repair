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

#[test]
fn constant_reference_emits_constant_token_type() {
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = no_engine_api();
    let source = "const int MAX = 100;\nint v = MAX;";
    let tokens = compute_for(source, None, &engine, &ws, &project);

    let constant_tokens: Vec<_> = tokens
        .iter()
        .filter(|t| token_text(source, t) == "MAX")
        .collect();
    assert!(
        !constant_tokens.is_empty(),
        "MAX reference should produce at least one token"
    );
    assert!(
        constant_tokens
            .iter()
            .any(|t| t.token_type == SemanticTokenType::new("constant")),
        "constant reference should emit 'constant' token type"
    );
}

#[test]
fn rule_reference_emits_rule_token_type() {
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = no_engine_api();
    let source = "rule myRule() {}\nvoid caller() { myRule(); }";
    let tokens = compute_for(source, None, &engine, &ws, &project);

    let rule_tokens: Vec<_> = tokens
        .iter()
        .filter(|t| token_text(source, t) == "myRule")
        .collect();
    assert!(
        !rule_tokens.is_empty(),
        "myRule reference should produce at least one token"
    );
    assert!(
        rule_tokens
            .iter()
            .any(|t| t.token_type == SemanticTokenType::new("rule")),
        "rule reference should emit 'rule' token type"
    );
}

#[test]
fn extern_variable_emits_extern_and_origin_modifiers() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let vanilla_dir = game_root.join("ai");
    std::fs::create_dir_all(&vanilla_dir).unwrap();
    let vanilla_file = vanilla_dir.join("vanilla.xs");
    std::fs::write(&vanilla_file, "extern int gFoo = 0;").unwrap();

    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = no_engine_api();
    let source = "extern int gFoo = 0;\nint v = gFoo;";
    let tokens = compute_for(source, Some(&vanilla_file), &engine, &ws, &project);

    let extern_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "gFoo" && t.token_type == SemanticTokenType::VARIABLE)
        .expect("gFoo variable token missing");
    let modifiers: Vec<&str> = extern_token.modifiers.iter().map(|m| m.as_str()).collect();
    assert!(
        modifiers.contains(&"extern"),
        "extern variable should have extern modifier: {:?}",
        modifiers
    );
    assert!(
        modifiers.contains(&"unmodded"),
        "extern variable in vanilla should have unmodded origin: {:?}",
        modifiers
    );
}

#[test]
fn constant_preserves_origin_modifier_with_new_token_type() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let mod_root = tmp.path().join("mod");
    let overlay_file = mod_root.join("game").join("ai").join("modded.xs");
    std::fs::create_dir_all(overlay_file.parent().unwrap()).unwrap();
    std::fs::write(&overlay_file, "const int MOD_MAX = 200;").unwrap();

    let mut ws = workspace::Workspace::new(game_root.parent().unwrap().to_path_buf());
    let mod_uri = tower_lsp_server::ls_types::Uri::from_file_path(&mod_root).unwrap();
    ws.register_mod(mod_uri).unwrap();

    let entry = ws.mods().first().unwrap();
    let project = ws.build_virtual_project(entry);

    let engine = no_engine_api();
    let source = "const int MOD_MAX = 200;\nint v = MOD_MAX;";
    let tokens = compute_for(source, Some(&overlay_file), &engine, &ws, &project);

    let constant_tokens: Vec<_> = tokens
        .iter()
        .filter(|t| token_text(source, t) == "MOD_MAX")
        .collect();
    assert!(
        !constant_tokens.is_empty(),
        "MOD_MAX constant should produce at least one token"
    );
    assert!(
        constant_tokens
            .iter()
            .any(|t| t.token_type == SemanticTokenType::new("constant")),
        "MOD_MAX should emit 'constant' token type"
    );
    assert!(
        constant_tokens
            .iter()
            .any(|t| t.modifiers.iter().any(|m| m.as_str() == "modded")),
        "MOD_MAX in modded code should keep modded origin modifier"
    );
}
