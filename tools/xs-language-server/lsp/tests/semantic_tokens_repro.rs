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

fn engine_only_api() -> engine_api::SharedEngineApi {
    Arc::new(engine_api::EngineApi {
        syscalls: vec![engine_api::Syscall {
            name: "engineFn".to_string(),
            help: "engine function".to_string(),
            return_type: "void".to_string(),
            params: vec![],
            filename: None,
        }],
        aiplans: vec![],
    })
}

fn compute_for(
    source: &str,
    current_file: Option<&Path>,
    engine: &engine_api::SharedEngineApi,
    ws: &workspace::Workspace,
    project: &workspace::VirtualProject,
) -> Vec<semantic_tokens::Token> {
    let (_cst, _diags) = parser::parse(source);
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

#[test]
fn test_semantic_token_legend_advertised() {
    let caps = semantic_tokens::server_capabilities();
    let opts = match caps {
        tower_lsp_server::ls_types::SemanticTokensServerCapabilities::SemanticTokensOptions(o) => o,
        _ => panic!("expected SemanticTokensOptions"),
    };
    let legend = opts.legend;
    let types: Vec<String> = legend
        .token_types
        .iter()
        .map(|t| t.as_str().to_string())
        .collect();
    let modifiers: Vec<String> = legend
        .token_modifiers
        .iter()
        .map(|m| m.as_str().to_string())
        .collect();

    assert!(
        types.contains(&"function".to_string()),
        "legend must contain function"
    );
    assert!(
        types.contains(&"variable".to_string()),
        "legend must contain variable"
    );
    assert!(
        types.contains(&"type".to_string()),
        "legend must contain type"
    );

    assert!(
        modifiers.contains(&"engine".to_string()),
        "legend must contain engine modifier"
    );
    assert!(
        modifiers.contains(&"modded".to_string()),
        "legend must contain modded modifier"
    );
    assert!(
        modifiers.contains(&"unmodded".to_string()),
        "legend must contain unmodded modifier"
    );
    assert!(
        modifiers.contains(&"local".to_string()),
        "legend must contain local modifier"
    );
    assert!(
        modifiers.contains(&"static".to_string()),
        "legend must contain static modifier"
    );
}

#[test]
fn test_engine_function_emits_engine_modifier() {
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = engine_only_api();
    let source = "void f() { engineFn(); }";

    let tokens = compute_for(source, None, &engine, &ws, &project);
    let engine_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "engineFn")
        .expect("engineFn token missing");
    assert_eq!(engine_token.token_type, SemanticTokenType::FUNCTION);
    assert!(
        engine_token
            .modifiers
            .iter()
            .any(|m| m.as_str() == "engine"),
        "engineFn should have engine modifier"
    );
}

#[test]
fn test_modded_function_emits_modded_modifier() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let mod_root = tmp.path().join("mod");
    let overlay_file = mod_root.join("game").join("ai").join("modded.xs");
    std::fs::create_dir_all(overlay_file.parent().unwrap()).unwrap();
    std::fs::write(&overlay_file, "void modFn() {}").unwrap();

    let mut ws = workspace::Workspace::new(game_root.parent().unwrap().to_path_buf());
    let mod_uri = tower_lsp_server::ls_types::Uri::from_file_path(&mod_root).unwrap();
    ws.register_mod(mod_uri).unwrap();

    let entry = ws.mods().first().unwrap();
    let project = ws.build_virtual_project(entry);

    let engine = Arc::new(engine_api::EngineApi {
        syscalls: vec![],
        aiplans: vec![],
    });
    let source = "void modFn() {} void caller() { modFn(); }";
    let tokens = compute_for(source, Some(&overlay_file), &engine, &ws, &project);
    let modded_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "modFn")
        .expect("modFn token missing");
    assert_eq!(modded_token.token_type, SemanticTokenType::FUNCTION);
    assert!(
        modded_token
            .modifiers
            .iter()
            .any(|m| m.as_str() == "modded"),
        "modFn should have modded modifier"
    );
}

#[test]
fn test_unmodded_function_emits_unmodded_modifier() {
    let tmp = TempDir::new().unwrap();
    let game_root = tmp.path().join("game");
    let vanilla_dir = game_root.join("ai");
    std::fs::create_dir_all(&vanilla_dir).unwrap();
    let vanilla_file = vanilla_dir.join("vanilla.xs");
    std::fs::write(&vanilla_file, "void vanillaFn() {}").unwrap();

    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = Arc::new(engine_api::EngineApi {
        syscalls: vec![],
        aiplans: vec![],
    });
    let source = "void vanillaFn() {} void caller() { vanillaFn(); }";
    let tokens = compute_for(source, Some(&vanilla_file), &engine, &ws, &project);
    let unmodded_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "vanillaFn")
        .expect("vanillaFn token missing");
    assert_eq!(unmodded_token.token_type, SemanticTokenType::FUNCTION);
    assert!(
        unmodded_token
            .modifiers
            .iter()
            .any(|m| m.as_str() == "unmodded"),
        "vanillaFn should have unmodded modifier"
    );
}

#[test]
fn test_local_variable_emits_local_modifier() {
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = Arc::new(engine_api::EngineApi {
        syscalls: vec![],
        aiplans: vec![],
    });
    let source = "void foo() { int localVar = 1; localVar = 2; }";
    let tokens = compute_for(source, None, &engine, &ws, &project);
    let local_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "localVar")
        .expect("localVar token missing");
    assert_eq!(local_token.token_type, SemanticTokenType::VARIABLE);
    assert!(
        local_token.modifiers.iter().any(|m| m.as_str() == "local"),
        "localVar should have local modifier"
    );
}

#[test]
fn test_builtin_type_emits_engine_modifier() {
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = Arc::new(engine_api::EngineApi {
        syscalls: vec![],
        aiplans: vec![],
    });
    let source = "void foo() { int x = 1; }";
    let tokens = compute_for(source, None, &engine, &ws, &project);
    let type_token = tokens
        .iter()
        .find(|t| token_text(source, t) == "int")
        .expect("int token missing");
    assert_eq!(type_token.token_type, SemanticTokenType::TYPE);
    assert!(
        type_token.modifiers.iter().any(|m| m.as_str() == "engine"),
        "int should have engine modifier"
    );
}
