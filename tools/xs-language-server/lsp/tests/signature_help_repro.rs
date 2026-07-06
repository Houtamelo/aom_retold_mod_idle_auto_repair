//! Integration tests for `textDocument/signatureHelp`.

use std::path::Path;
use std::sync::OnceLock;

use tempfile::TempDir;
use tower_lsp_server::ls_types::{ParameterLabel, Position, Uri};
use xs_language_server::engine_api::EngineApi;
use xs_language_server::merged_view::MergedView;
use xs_language_server::signature_help::signature_help;
use xs_language_server::symbols::{SymbolKind, build_full_symbol_table, build_symbol_table};
use xs_language_server::workspace::{VirtualProject, Workspace};

fn engine_api() -> &'static EngineApi {
    static API: OnceLock<EngineApi> = OnceLock::new();
    API.get_or_init(|| {
        let archive = Path::new("../../../docs/doxygen_retail.7z");
        let cache_dir = TempDir::new().expect("temp dir");
        EngineApi::load_from_archive(archive, cache_dir.path()).expect("load engine API")
    })
}

#[test]
fn engine_syscall_signature_at_open_paren() {
    let source = "void test() {\n   aiEchoCategory(0, \"warning\");\n}\n";
    // Cursor just after the opening `(` of `aiEchoCategory(`.
    let pos = Position::new(1, 18);
    let result = signature_help(source, pos, engine_api(), None, None);
    let help = result.expect("expected SignatureHelp for engine syscall");
    assert_eq!(help.active_parameter, Some(0));
    assert!(!help.signatures.is_empty(), "expected at least one signature");
    let label = &help.signatures[0].label;
    assert!(
        label.contains("aiEchoCategory"),
        "signature label should contain the syscall name, got: {label}"
    );
}

#[test]
fn engine_syscall_active_parameter_advances_after_comma() {
    let source = "void test() {\n   aiEchoCategory(0, \"warning\");\n}\n";
    // Cursor just after the first comma (active param should advance to 1).
    let pos = Position::new(1, 20);
    let result = signature_help(source, pos, engine_api(), None, None);
    let help = result.expect("expected SignatureHelp after comma");
    assert_eq!(help.active_parameter, Some(1));
}

#[test]
fn unknown_function_returns_none() {
    let source = "void test() {\n   totallyUnknownFn(1, 2);\n}\n";
    let pos = Position::new(1, 22);
    let result = signature_help(source, pos, engine_api(), None, None);
    assert!(result.is_none(), "unknown function should return no signature help");
}

#[test]
fn workspace_function_signature_in_current_file() {
    let source = "\nvoid repairUnit(int id) {}\n\nvoid caller() {\n   repairUnit(1);\n}\n";
    let table = build_full_symbol_table(source);
    let sym = table.find("repairUnit").expect("repairUnit symbol");
    assert_eq!(sym.kind, SymbolKind::Function);

    // Cursor just after the opening `(` of `repairUnit(`.
    let pos = Position::new(4, 14);
    let result = signature_help(source, pos, engine_api(), None, Some(&table));
    let help = result.expect("expected SignatureHelp for workspace function");
    assert_eq!(help.active_parameter, Some(0));
    let label = &help.signatures[0].label;
    assert!(
        label.contains("repairUnit") && label.contains("int id"),
        "signature label should show function and parameter, got: {label}"
    );
}

#[test]
fn nested_call_reports_inner_signature() {
    // Inside `inner(b)` the cursor should see `inner`, not `outer`.
    let source = "\nvoid inner(int x, int y) {}\nvoid outer(int a, int b) {}\n\nvoid caller() {\n   outer(inner(1, 2), 3);\n}\n";
    let table = build_full_symbol_table(source);

    // Cursor on `2` inside the `inner(...)` call.
    // Line 5: "   outer(inner(1, 2), 3);"
    // indices: 3 'o', 8 '(', 9 'i', 15 '(', 16 '1', 17 ',', 18 ' ', 19 '2'
    let pos = Position::new(5, 19);
    let result = signature_help(source, pos, engine_api(), None, Some(&table));
    let help = result.expect("expected SignatureHelp for inner call");
    assert_eq!(help.active_parameter, Some(1));
    let label = &help.signatures[0].label;
    assert!(
        label.contains("inner") && label.contains("int y"),
        "expected inner signature, got: {label}"
    );
}

#[test]
fn rule_signature_has_no_parameters() {
    let source = "rule cleanupLingering\nminInterval 5\nactive\n{\n}\n\nvoid caller() {\n   cleanupLingering();\n}\n";
    let table = build_full_symbol_table(source);
    let sym = table.find("cleanupLingering").expect("rule symbol");
    assert_eq!(sym.kind, SymbolKind::Rule);

    // Cursor just after the opening `(` of `cleanupLingering(`.
    // Line 7 is "   cleanupLingering();"
    let pos = Position::new(7, 20);
    let result = signature_help(source, pos, engine_api(), None, Some(&table));
    let help = result.expect("expected SignatureHelp for rule call");
    assert_eq!(help.active_parameter, Some(0));
    assert!(
        help.signatures[0].label.contains("cleanupLingering"),
        "expected rule name in label, got: {}",
        help.signatures[0].label
    );
    assert!(
        help.signatures[0].parameters.as_ref().map(|p| p.is_empty()).unwrap_or(true),
        "rule signature should have no parameters"
    );
}

#[test]
fn included_workspace_function_signature_via_merged_view() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path();

    let includer = root.join("game").join("ai").join("main.xs");
    let included = root.join("game").join("ai").join("util.xs");
    std::fs::create_dir_all(includer.parent().unwrap()).unwrap();
    std::fs::write(&included, "void includedFn(int a, string b) {}\n").unwrap();
    std::fs::write(
        &includer,
        "include \"util.xs\";\n\nvoid caller() {\n   includedFn(1, \"x\");\n}\n",
    )
    .unwrap();

    let mut ws = Workspace::new(root.to_path_buf());
    let mod_root = root.join("mod");
    std::fs::create_dir_all(&mod_root).unwrap();
    ws.register_mod(Uri::from_file_path(&mod_root).unwrap()).unwrap();
    let project = ws.build_virtual_project(ws.mods().first().unwrap());

    let source = std::fs::read_to_string(&includer).unwrap();
    let own_table = build_symbol_table(&source);
    let cache_dir = TempDir::new().unwrap();
    let merged = MergedView::build(&includer, &source, &own_table, &ws, &project, cache_dir.path());

    let pos = Position::new(3, 14);
    let result = signature_help(&source, pos, engine_api(), Some(&merged), None);
    let help = result.expect("expected SignatureHelp for included function");
    assert_eq!(help.active_parameter, Some(0));
    let label = &help.signatures[0].label;
    assert!(
        label.contains("includedFn") && label.contains("int a") && label.contains("string b"),
        "expected included function signature, got: {label}"
    );
}

#[test]
fn engine_syscall_signature_exposes_default_value() {
    // aiSetHandler documents defaults for both parameters (handlerName = "",
    // eventType = -1). REQ-SIG-01 says ParameterInformation items MUST expose
    // the documented default values.
    let source = "void test() {\n   aiSetHandler(\"handler\", 0);\n}\n";
    // Cursor just after the opening `(` of `aiSetHandler(`.
    let pos = Position::new(1, 16);
    let result = signature_help(source, pos, engine_api(), None, None);
    let help = result.expect("expected SignatureHelp for aiSetHandler");

    let params = help.signatures[0]
        .parameters
        .as_ref()
        .expect("expected parameters");
    assert_eq!(params.len(), 2, "expected two parameters");
    let label_strings: Vec<String> = params
        .iter()
        .map(|p| match &p.label {
            ParameterLabel::Simple(s) => s.clone(),
            ParameterLabel::LabelOffsets(_) => unreachable!(),
        })
        .collect();
    assert!(
        label_strings.iter().all(|s| s.contains('=')),
        "every parameter label should expose its default value, got: {:?}",
        label_strings
    );
    assert!(
        help.signatures[0].label.contains("handlerName = \"\""),
        "signature label should include the string default, got: {}",
        help.signatures[0].label
    );
    assert!(
        help.signatures[0].label.contains("eventType = -1"),
        "signature label should include the int default, got: {}",
        help.signatures[0].label
    );
}

#[test]
fn active_parameter_clamped_to_last_index() {
    // aiSetHandler has exactly two parameters. With three commas before the
    // cursor, the raw comma count is past the last parameter; REQ-SIG-05 says
    // activeParameter MUST be clamped to paramCount - 1.
    let source = "void test() {\n   aiSetHandler(\"a\", 0, 1,\n}\n";
    let pos = Position::new(1, 25);
    let result = signature_help(source, pos, engine_api(), None, None);
    let help = result.expect("expected SignatureHelp for clamped active param");
    assert_eq!(
        help.active_parameter,
        Some(1),
        "activeParameter should be clamped to paramCount - 1"
    );
}

#[test]
fn engine_syscall_signature_exposes_bool_default() {
    // kbUnitGetPower documents defaults for both parameters, including a bool
    // false. Triangulates the defaults-exposure path with a different type.
    let source = "void test() {\n   kbUnitGetPower(-1, true);\n}\n";
    // Cursor just after the opening `(` of `kbUnitGetPower(`.
    let pos = Position::new(1, 18);
    let result = signature_help(source, pos, engine_api(), None, None);
    let help = result.expect("expected SignatureHelp for kbUnitGetPower");
    assert!(
        help.signatures[0].label.contains("ignoreCurrentHealth = false"),
        "expected bool default in signature label, got: {}",
        help.signatures[0].label
    );
}

#[test]
fn active_parameter_clamped_for_single_param_function() {
    // aiEcho takes a single string argument. Extra commas should still clamp
    // activeParameter to 0.
    let source = "void test() {\n   aiEcho(\"a\", \"b\", \"c\",\n}\n";
    let pos = Position::new(1, 23);
    let result = signature_help(source, pos, engine_api(), None, None);
    let help = result.expect("expected SignatureHelp for single-param clamp");
    assert_eq!(help.active_parameter, Some(0));
}
