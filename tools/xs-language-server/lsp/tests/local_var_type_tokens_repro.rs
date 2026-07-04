//! TDD: semantic tokens for type specifiers inside function bodies.
//!
//! After the typed-AST refactor, `visit_top_level_item_for_types`
//! only visited the outer type of a function/class/forward-decl. It
//! did NOT recurse into function bodies, so a local `int x = 1;` did
//! not emit a TYPE token for `int`. The pre-existing
//! `test_builtin_type_emits_engine_modifier` test in
//! `semantic_tokens_repro.rs` was therefore failing — not a flake,
//! but a real feature gap.
//!
//! Each test parses a small source, calls `compute_tokens`, and
//! asserts that a token for the inner type specifier is present and
//! carries the `engine` modifier.

use std::path::Path;
use std::sync::Arc;
use tempfile::TempDir;
use tower_lsp_server::ls_types::SemanticTokenModifier;
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
    engine: &engine_api::SharedEngineApi,
    ws: &workspace::Workspace,
    project: &workspace::VirtualProject,
) -> Vec<semantic_tokens::Token> {
    let (_cst, _diags) = parser::parse(source);
    let own_table = symbols::build_full_symbol_table(source);
    let cache_dir = TempDir::new().unwrap();
    let member_index = semantic_tokens::MemberIndex::build(
        &own_table, ws, project, cache_dir.path(), None,
    );
    semantic_tokens::compute_tokens(
        source, None, &own_table, None, engine, ws, project, &member_index,
    )
}

fn find_type<'a>(tokens: &'a [semantic_tokens::Token], source: &str, name: &str) -> Option<&'a semantic_tokens::Token> {
    tokens.iter().find(|t| {
        let text = token_text(source, t);
        text == name
            && (t.token_type == tower_lsp_server::ls_types::SemanticTokenType::TYPE
                || format!("{:?}", t.token_type).contains("type"))
    })
}

#[test]
fn local_int_emits_type_token() {
    let source = "void foo() { int x = 1; }";
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = Arc::new(engine_api::EngineApi { syscalls: vec![], aiplans: vec![] });
    let tokens = compute_for(source, &engine, &ws, &project);
    let int_token = find_type(&tokens, source, "int");
    assert!(int_token.is_some(), "int type token missing; got tokens: {:?}", tokens);
}

#[test]
fn local_int_has_engine_modifier() {
    let source = "void foo() { int x = 1; }";
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = Arc::new(engine_api::EngineApi { syscalls: vec![], aiplans: vec![] });
    let tokens = compute_for(source, &engine, &ws, &project);
    let int_token = find_type(&tokens, source, "int").expect("int token missing");
    assert!(
        int_token.modifiers.iter().any(|m| m == &SemanticTokenModifier::new("engine")),
        "int should have engine modifier; got modifiers: {:?}",
        int_token.modifiers
    );
}

#[test]
fn local_string_emits_type_token() {
    let source = r#"void foo() { string s = "hi"; }"#;
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = Arc::new(engine_api::EngineApi { syscalls: vec![], aiplans: vec![] });
    let tokens = compute_for(source, &engine, &ws, &project);
    let t = find_type(&tokens, source, "string");
    assert!(t.is_some(), "string type token missing; got tokens: {:?}", tokens);
}

#[test]
fn top_level_type_still_works() {
    // Regression guard: top-level type emission must still work.
    let source = "int gVar = 0;";
    let tmp = TempDir::new().unwrap();
    let ws = workspace::Workspace::new(tmp.path().to_path_buf());
    let project = workspace::VirtualProject::default();
    let engine = Arc::new(engine_api::EngineApi { syscalls: vec![], aiplans: vec![] });
    let tokens = compute_for(source, &engine, &ws, &project);
    let t = find_type(&tokens, source, "int");
    assert!(t.is_some(), "int type token missing; got tokens: {:?}", tokens);
}
