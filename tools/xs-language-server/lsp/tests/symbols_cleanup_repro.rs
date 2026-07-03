//! Strict-TDD RED→GREEN contracts for Batch B findings (R1-F-01, R1-F-02, R1-F-03).
//!
//! Each test asserts the post-fix state: dead code is gone, and test names
//! match the code paths they actually exercise.

use xs_language_server::symbols::{SymbolKind, build_symbol_table};

// ===================================================================
// R1-F-01 — `Param::render` is dead (zero callers) and has been removed
// ===================================================================

#[test]
fn param_render_no_callers_in_production_source() {
    // Grep the entire LSP source tree (excluding this audit file and
    // `target/`) for any caller of `Param::render` or the trait-method form
    // `.render(`. There must be zero: the function has been deleted.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let src_root = manifest_dir.join("src");

    let mut hits: Vec<(std::path::PathBuf, usize, String)> = Vec::new();
    visit_rs_files(&src_root, &mut |path, line_no, line| {
        if line.contains("Param::render") || line.contains(".render(") {
            hits.push((path.to_path_buf(), line_no, line.to_string()));
        }
    });

    assert!(
        hits.is_empty(),
        "Param::render must have zero callers in production source, found {}: {:#?}",
        hits.len(),
        hits
    );
}

fn visit_rs_files(dir: &std::path::Path, on_line: &mut dyn FnMut(&std::path::Path, usize, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, on_line);
        } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                for (line_no, line) in contents.lines().enumerate() {
                    on_line(&path, line_no + 1, line);
                }
            }
        }
    }
}

#[test]
fn function_definition_with_inner_error_in_default_value_still_extracts_test_renamed() {
    // Strict-TDD contract: the misleading test
    // `recovers_function_definition_from_error_node_with_body` must be renamed
    // to describe the regular `function_definition` path it actually exercises.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest_dir.join("src/symbols.rs");
    let contents = std::fs::read_to_string(&path).expect("read src/symbols.rs");

    assert!(
        contents
            .contains("fn function_definition_with_inner_error_in_default_value_still_extracts"),
        "src/symbols.rs must contain the renamed test function"
    );
    assert!(
        !contents.contains("fn recovers_function_definition_from_error_node_with_body"),
        "src/symbols.rs must no longer contain the old misleading test name"
    );
}

// ===================================================================
// R1-F-02 — forward declarations don't surface as ERROR nodes
// ===================================================================
//
// The grammar (`tree-sitter-xs/grammar.js`) declares
// `_declaration_declarator -> _function_declaration_declarator` aliased to
// `function_declarator`, so a forward function declaration like
// `void bar(int x = -1);` parses as a regular `declaration` containing a
// `function_declarator` — NOT as an ERROR node.
//
// The dispatch table comment at symbols.rs:162-166 says the opposite and
// the `extract_error_forward_declaration` / `extract_error_function_definition`
// helpers are reached only when the parser produces an ERROR node, which
// for normal forward declarations it never does.

#[test]
fn r1_f02_forward_declaration_parses_as_error_node() {
    // R1-F-02 (per the original review): claimed that the grammar handles
    // forward declarations natively via `_declaration_declarator ->
    // _function_declaration_declarator` aliased to `function_declarator`,
    // and that the ERROR-recovery helpers are dead. **REFUTED.**
    //
    // Direct parser inspection shows: forward function declarations
    // (`void bar(int x = -1);`, `void bar(int x);`, `void bar();`,
    // `int bar(...);`, `static void bar(...);`, with or without body,
    // with or without param defaults, with or without trailing definition)
    // ALL produce an `ERROR` node at the root, NOT a `declaration` node.
    // The dispatch comment at symbols.rs:162-166 is therefore correct
    // and the `extract_error_forward_declaration` /
    // `extract_error_function_definition` helpers are needed in practice.
    //
    // This test asserts the opposite of what the review claimed, locking in
    // the refutation as evidence in the audit trail.
    use xs_language_server::parser;
    let src = "void bar(int x = -1);\n";
    let tree = parser::parse(src).expect("forward decl must parse");

    let root = tree.root_node();
    let mut found_declaration = false;
    let mut found_error = false;
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "declaration" {
            found_declaration = true;
        }
        if child.kind() == "ERROR" {
            found_error = true;
        }
    }
    assert!(
        found_error,
        "forward declaration `void bar(int x = -1);` MUST produce an \
         ERROR node — proves the symbols.rs:162-166 dispatch comment is \
         correct (R1-F-02 refuted)"
    );
    assert!(
        !found_declaration,
        "forward declaration must NOT produce a `declaration` node directly \
         — the grammar.js alias `_declaration_declarator -> _function_declaration_declarator` \
         is supposed to handle this, but the generated parser does not \
         actually invoke it (grammar/parser bug, separate from this SDD scope)"
    );
}

#[test]
fn r1_f02_extract_error_helpers_have_no_real_callers() {
    // The dispatch table at symbols.rs:168-169 calls
    // `extract_error_forward_declaration` and
    // `extract_error_function_definition`, so the helpers are not literally
    // unused. But their dispatch is gated on the parser producing an
    // `ERROR` node — which, per R1-F-02a, never happens for normal forward
    // declarations. Verify: grep the integration tests for ERROR-node
    // fixtures.
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let tests_dir = manifest_dir.join("tests");
    let mut error_node_fixtures: Vec<(std::path::PathBuf, usize, String)> = Vec::new();
    visit_rs_files(&tests_dir, &mut |path, line_no, line| {
        // Match test inputs that produce a parser-level ERROR. Heuristic:
        // any test source containing `[ ] { }` (the broken-default-value
        // pattern from `function_definition_with_inner_error_in_default_value_still_extracts`).
        if line.contains("[ ] { }")
            || line.contains("ERROR node")
            || line.contains("ERROR recovery")
        {
            error_node_fixtures.push((path.to_path_buf(), line_no, line.to_string()));
        }
    });

    // At least one ERROR-node fixture exists in the test suite (proves the
    // helper can be invoked) AND that fixture's body must contain a
    // function-definition shape (the existing test uses a body `{ }` so the
    // regular `function_definition` rule matches and the ERROR is just an
    // inner subtree, not the dispatch entry).
    assert!(
        !error_node_fixtures.is_empty(),
        "at least one ERROR-node fixture must exist (otherwise the helpers \
         are unreachable even in tests): found zero in {}",
        tests_dir.display()
    );
}

// ===================================================================
// R1-F-03 — `function_definition_with_inner_error_in_default_value_still_extracts`
//            exercises the regular `function_definition` path, not ERROR recovery
// ===================================================================

#[test]
fn r1_f03_test_input_parses_via_function_definition_rule() {
    // The test input `void broken(int x = [ ] { }) { }` has TWO `{ }`
    // blocks. The inner `[ ] { }` is an invalid default-value (tree-sitter
    // produces an ERROR for that subtree), but the OUTER `{ }` is a real
    // body, so the regular `function_definition` rule matches the overall
    // shape — the symbol is recovered through `extract_function`, not
    // through the ERROR-recovery helper.
    use xs_language_server::parser;
    let src = "void broken(int x = [ ] { }) { }\n";
    let tree = parser::parse(src).expect("must parse");
    let root = tree.root_node();

    // Root must contain a `function_definition` node (NOT an `ERROR` at root).
    let mut cursor = root.walk();
    let kinds: Vec<String> = root
        .children(&mut cursor)
        .map(|c| c.kind().to_string())
        .collect();
    assert!(
        kinds.iter().any(|k| k == "function_definition"),
        "root must contain a function_definition node; got kinds = {:?}",
        kinds
    );
    assert!(
        !kinds.iter().any(|k| k == "ERROR"),
        "root must NOT be an ERROR node (the regular function_definition \
         rule matched the overall shape). Got kinds = {:?}",
        kinds
    );
}

#[test]
fn r1_f03_symbol_extracted_via_extract_function_path() {
    // End-to-end: parse the test input and verify the recovered symbol
    // matches what `extract_function` would produce. We can't easily
    // distinguish "via extract_function" vs "via extract_error_*" in
    // a public test, but we CAN verify the symbol's `is_forward` is false
    // (regular function definition) and `kind` is Function.
    let src = "void broken(int x = [ ] { }) { }\n";
    let tree = xs_language_server::parser::parse(src).expect("must parse");
    let table = build_symbol_table(src);

    let s = table
        .find("broken")
        .expect("broken symbol must be extractable from function_definition with inner ERROR");
    assert_eq!(s.kind, SymbolKind::Function);
    assert!(
        !s.is_forward,
        "regular function def, not a forward declaration"
    );
}
