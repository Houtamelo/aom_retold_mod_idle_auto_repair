//! Typed-AST extraction tests for the fixtures originally used in the
//! R1-F-01..R1-F-03 audit. These tests prove that `build_symbol_table`
//! (now backed by `xs_parser::ast`) recovers the same symbols without
//! relying on tree-sitter ERROR-node heuristics.

use xs_language_server::symbols::{SymbolKind, build_symbol_table};

#[test]
fn typed_ast_extracts_function_definition() {
    let src = "void helper(int x = -1) { }\n";
    let table = build_symbol_table(src);
    let s = table
        .find("helper")
        .expect("function definition must be extracted");
    assert_eq!(s.kind, SymbolKind::Function);
    assert!(!s.is_forward, "function with body is not a forward declaration");
    assert_eq!(s.name, "helper");
}

#[test]
fn typed_ast_extracts_forward_declaration() {
    let src = "void bar(int x = -1);\n";
    let table = build_symbol_table(src);
    let s = table
        .find("bar")
        .expect("forward declaration must be extracted");
    assert_eq!(s.kind, SymbolKind::Function);
    assert!(s.is_forward, "prototype is marked forward");
    assert_eq!(s.name, "bar");
}

#[test]
fn typed_ast_extracts_bare_forward_declaration() {
    let src = "void baz();\n";
    let table = build_symbol_table(src);
    let s = table
        .find("baz")
        .expect("bare prototype must be extracted");
    assert_eq!(s.kind, SymbolKind::Function);
    assert!(s.is_forward);
}

#[test]
fn typed_ast_forward_declaration_followed_by_definition_is_not_forward() {
    let src = "void bar(int x = -1);\nvoid bar(int x = -1) { }\n";
    let table = build_symbol_table(src);
    let s = table.find("bar").expect("bar must be extracted");
    assert_eq!(s.kind, SymbolKind::Function);
    assert!(!s.is_forward, "definition supersedes the prototype");
}

#[test]
fn typed_ast_extracts_rules_classes_and_globals() {
    let src = r#"
rule MyRule
    minInterval 1
    maxInterval 2
    active
{
    xsChatData("test");
}

class MyClass {
    int field = 0;
    void method() { }
}

int globalVar = 7;
"#;
    let table = build_symbol_table(src);

    let rule = table.find("MyRule").expect("rule extracted");
    assert_eq!(rule.kind, SymbolKind::Rule);

    let cls = table.find("MyClass").expect("class extracted");
    assert_eq!(cls.kind, SymbolKind::Class);

    let var = table.find("globalVar").expect("global variable extracted");
    assert_eq!(var.kind, SymbolKind::Variable);
}
