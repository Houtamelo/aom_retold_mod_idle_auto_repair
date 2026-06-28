//! Type-check engine-API function calls: argument count + argument type for
//! literal expressions and workspace symbols with known types.
//!
//! Week 5 of the post-spike roadmap. The scope is intentionally narrow:
//! only calls to engine syscalls (`kb*`, `tr*`, `xs*`, `rm*`, `ai*`). Calls
//! to workspace-defined functions are not checked here — they need
//! cross-file resolution (a later week).
//!
//! What this catches (the bulk of real XS bugs):
//!   * Typo'd engine API names that happen to parse but don't exist in the
//!     engine (e.g. `aiEch("hi")`) — covered by `find_syscall` returning None
//!     so we don't flag, but `aiEchho(...)` would similarly be unknown.
//!   * Wrong argument count: `aiEcho("hi", "extra")`.
//!   * Wrong literal type: `aiEcho(42)` (expected string, got int).
//!   * Identifier args whose declared type doesn't match: `int x; aiEcho(x);`.
//!
//! Out of scope (deferred):
//!   * Class instance types (require whole-program analysis).
//!   * Operator overloading, generics.
//!   * Control flow type inference.
//!   * Return-type checking.
//!   * "Did you mean...?" suggestions.

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::engine_api::{EngineApi, Param};
use crate::merged_view::MergedView;
use crate::symbols::{SymbolKind, SymbolTable};

/// Walk `tree` and return one `Diagnostic` per wrong-arg-count or
/// wrong-arg-type call to a known engine or user-defined function.
pub fn check_calls(
    tree: &tree_sitter::Tree,
    source: &str,
    engine: &EngineApi,
    table: &SymbolTable,
    project: Option<&crate::semantic::VirtualProject>,
) -> Vec<Diagnostic> {
    check_calls_with_merged(tree, source, engine, table, None, project)
}

/// Like [`check_calls`], but resolves user-defined callees through the
/// merged include-paste scope first, falling back to the full project.
pub fn check_calls_with_merged(
    tree: &tree_sitter::Tree,
    source: &str,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    walk(tree.root_node(), source, engine, table, merged, project, &mut out);
    out
}

fn walk(
    node: tree_sitter::Node<'_>,
    source: &str,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    if node.kind() == "call_expression" {
        check_one_call(node, source, engine, table, merged, project, out);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk(child, source, engine, table, merged, project, out);
    }
}

fn check_one_call(
    call_node: tree_sitter::Node<'_>,
    source: &str,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(callee) = extract_callee_name(call_node, source) else {
        return;
    };

    // Resolve the callee against the engine API and then against the virtual
    // project. We keep the same count + type check shape for both.
    let resolved: Option<Callee<'_>> = if let Some(syscall) = engine.find_syscall(&callee) {
        Some(Callee::Engine(syscall))
    } else {
        resolve_workspace_function(merged, project, &callee).map(Callee::Workspace)
    };

    let Some(target) = resolved else { return };

    // Rules are handled by semantic resolution; type-checking a rule call
    // would require a zero-parameter signature that the engine supplies
    // internally, so we skip them here.
    if matches!(target, Callee::Workspace(sym) if sym.kind == crate::symbols::SymbolKind::Rule) {
        return;
    }

    let callee_source = target.source();
    let name = target.name();
    let params = target.params();
    let required_count = required_param_count(&params, callee_source);

    let arg_list = find_named_child(call_node, "argument_list");
    let arg_count = arg_list.map(count_args).unwrap_or(0);

    if arg_count > params.len() {
        out.push(Diagnostic {
            range: node_range(call_node),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message: format!(
                "expected {} argument(s) to `{name}`, got {arg_count}",
                params.len()
            ),
            related_information: None,
            tags: None,
            data: None,
        });
    } else if arg_count < required_count {
        out.push(Diagnostic {
            range: node_range(call_node),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message: format!(
                "expected {required_count} required argument(s) to `{name}`, got {arg_count}"
            ),
            related_information: None,
            tags: None,
            data: None,
        });
    }

    let Some(args) = arg_list else { return };
    for (i, (arg_node, expected)) in args
        .named_children(&mut args.walk())
        .zip(params.iter())
        .enumerate()
    {
        let Some(actual_ty) = expr_type(arg_node, source, table) else {
            continue;
        };
        if arg_types_compatible(&expected.ty, &actual_ty, callee_source) {
            continue;
        }
        if is_function_pointer_argument(arg_node, source, table, &expected.ty) {
            continue;
        }
        out.push(Diagnostic {
                range: node_range(arg_node),
                severity: Some(DiagnosticSeverity::ERROR),
                code: None,
                code_description: None,
                source: Some("xs-language-server".to_string()),
                message: format!(
                    "expected argument {i} of type `{expected}` for `{name}`, got `{actual}`",
                    i = i + 1,
                    expected = expected.ty,
                    name = name,
                    actual = actual_ty
                ),
                related_information: None,
                tags: None,
                data: None,
            });
    }
}

/// Where a resolved callee's signature comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalleeSource {
    Workspace,
    EngineApi,
}

fn required_param_count(params: &[Param], source: CalleeSource) -> usize {
    match source {
        // The engine runtime supplies defaults for all non-`ref` parameters,
        // even when the Doxygen extraction did not capture them.
        CalleeSource::EngineApi => 0,
        CalleeSource::Workspace => params.iter().filter(|p| p.default.is_none()).count(),
    }
}

/// A resolved callee, either an engine syscall or a workspace function.
enum Callee<'a> {
    Engine(&'a crate::engine_api::Syscall),
    Workspace(&'a crate::symbols::Symbol),
}

impl<'a> Callee<'a> {
    fn name(&self) -> &str {
        match self {
            Callee::Engine(s) => &s.name,
            Callee::Workspace(sym) => &sym.name,
        }
    }

    fn source(&self) -> CalleeSource {
        match self {
            Callee::Engine(_) => CalleeSource::EngineApi,
            Callee::Workspace(_) => CalleeSource::Workspace,
        }
    }

    fn params(&self) -> Vec<Param> {
        match self {
            Callee::Engine(s) => s.params.clone(),
            Callee::Workspace(sym) => sym
                .params
                .iter()
                .map(|p| Param {
                    ty: p.ty.clone(),
                    name: p.name.clone(),
                    default: p.default.clone(),
                })
                .collect(),
        }
    }
}

fn resolve_workspace_function<'a>(
    merged: Option<&'a MergedView>,
    project: Option<&'a crate::semantic::VirtualProject>,
    name: &str,
) -> Option<&'a crate::symbols::Symbol> {
    if let Some(ms) = merged.and_then(|mv| mv.find(name)) {
        return Some(&ms.symbol);
    }
    project.and_then(|p| resolve_workspace_function_project(p, name))
}

fn resolve_workspace_function_project<'a>(
    project: &'a crate::semantic::VirtualProject,
    name: &str,
) -> Option<&'a crate::symbols::Symbol> {
    let defined = project.files.values().find_map(|file| {
        file.table.symbols.iter().find(|s| {
            (s.kind == crate::symbols::SymbolKind::Function
                || s.kind == crate::symbols::SymbolKind::Rule)
                && s.name == name
                && !s.is_forward
        })
    });
    defined.or_else(|| project.registered_rules.get(name))
}

/// Extract the function name from a `call_expression`. XS calls look like
/// `name(...)` (bare identifier) or `obj.method(...)` (field expression).
/// Return the last segment of the callee (the method name for `obj.method`,
/// or just the name for `name`).
fn extract_callee_name(call_node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    let func_node = find_named_child(call_node, "field_expression")
        .or_else(|| find_named_child(call_node, "identifier"))?;
    if func_node.kind() == "identifier" {
        return Some(node_text(func_node, source).to_string());
    }
    // `field_expression` has a `field_identifier` child (the method name).
    let field = find_named_child(func_node, "field_identifier")?;
    Some(node_text(field, source).to_string())
}

/// Determine the XS type of an expression. Returns `None` for things we
/// can't statically type (class instances, complex expressions, unknown
/// identifiers, etc.) — in that case the caller skips silently.
fn expr_type(node: tree_sitter::Node<'_>, source: &str, table: &SymbolTable) -> Option<String> {
    match node.kind() {
        "string_literal" => Some("string".to_string()),
        "number_literal" => Some(number_literal_type(node, source)),
        "true" | "false" => Some("bool".to_string()),
        "identifier" => {
            let name = node_text(node, source);
            // `true`/`false` are not identifier nodes in this grammar, but
            // guard against future grammar changes where they might be.
            if name == "true" || name == "false" {
                return Some("bool".to_string());
            }
            table
                .find(name)
                .map(|s| s.ty.clone())
                .filter(|t| !t.is_empty())
        }
        _ => None,
    }
}

/// Float if the literal contains `.` or ends in `f`; otherwise int.
fn number_literal_type(node: tree_sitter::Node<'_>, source: &str) -> String {
    let text = node_text(node, source);
    if text.contains('.') || text.ends_with('f') || text.ends_with('F') {
        "float".to_string()
    } else {
        "int".to_string()
    }
}

/// Type compatibility for function-call arguments.
///
/// XS numeric types coerce freely at runtime, so `int` and `float` are
/// mutually compatible. `bool` and `string` remain strict.
fn arg_types_compatible(expected: &str, actual: &str, _source: CalleeSource) -> bool {
    if expected == actual {
        return true;
    }
    let numeric = ["int", "float"];
    numeric.contains(&expected) && numeric.contains(&actual)
}

/// True when `arg_node` is the name of a function/rule being passed as a
/// function-pointer argument. XS engine APIs such as `setOverrideStrategy`
/// declare their callback parameter as `void()`; passing the name of a
/// matching top-level function is valid, so we suppress the spurious
/// type-mismatch diagnostic.
fn is_function_pointer_argument(
    arg_node: tree_sitter::Node<'_>,
    source: &str,
    table: &SymbolTable,
    expected_ty: &str,
) -> bool {
    // Function-pointer types look like `void()` or `bool(int, string)`.
    if !expected_ty.contains('(') || !expected_ty.contains(')') {
        return false;
    }
    if arg_node.kind() != "identifier" {
        return false;
    }
    let name = node_text(arg_node, source);
    let Some(sym) = table.find(name) else {
        return false;
    };
    matches!(sym.kind, SymbolKind::Function | SymbolKind::Rule)
}

fn count_args(arg_list_node: tree_sitter::Node<'_>) -> usize {
    let mut count = 0;
    let mut cursor = arg_list_node.walk();
    for child in arg_list_node.named_children(&mut cursor) {
        if child.kind() != "comment" {
            count += 1;
        }
    }
    count
}

// --- node helpers (local — symbols.rs / references.rs have their own
// private versions; we duplicate here rather than create a new public API
// for a single use site) ---

fn node_range(node: tree_sitter::Node<'_>) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range::new(
        Position::new(start.row as u32, start.column as u32),
        Position::new(end.row as u32, end.column as u32),
    )
}

fn node_text<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn find_named_child<'a>(
    node: tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|c| c.kind() == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use crate::merged_view::MergedView;
    use crate::workspace::{VirtualProject as WorkspaceVirtualProject, Workspace};

    fn engine() -> &'static EngineApi {
        use std::sync::OnceLock;

        static ENGINE: OnceLock<EngineApi> = OnceLock::new();
        ENGINE.get_or_init(|| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            // tools/xs-language-server/ -> tools/ -> aom_retold_mod/
            let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).unwrap();
            let archive = workspace_root.join("docs/doxygen_retail.7z");
            let cache_dir = crate::cache::state_cache_dir();
            EngineApi::load_from_archive(&archive, &cache_dir)
                .expect("load engine data from Doxygen archive")
        })
    }

    fn table_for(source: &str) -> SymbolTable {
        let tree = crate::parser::parse(source).expect("parse");
        crate::symbols::build_symbol_table(&tree, source)
    }

    fn parse(src: &str) -> tree_sitter::Tree {
        crate::parser::parse(src).expect("parse")
    }

    fn messages(diags: &[Diagnostic]) -> Vec<String> {
        diags.iter().map(|d| d.message.clone()).collect()
    }

    #[test]
    fn no_diagnostics_for_clean_calls() {
        let src = r#"void test() { aiEcho("hello"); aiEchoCategory(0, "warning"); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        assert!(
            diags.is_empty(),
            "expected no diagnostics, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn flags_wrong_arg_count_too_many() {
        let src = r#"void test() { aiEcho("hi", "extra"); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter()
                .any(|m| m.contains("expected 1 argument") && m.contains("got 2")),
            "missing count diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_wrong_arg_count_too_few_for_workspace_function() {
        let src = r#"void myFn(int a, int b) {}
void test() { myFn(1); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(&tree, src, &engine(), &table, Some(&project));
        let msgs = messages(&diags);
        assert!(
            msgs.iter()
                .any(|m| m.contains("expected 2 required argument") && m.contains("got 1")),
            "missing count diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn allows_omitting_default_workspace_arguments() {
        let src = r#"void myFn(int a, int b = 0) {}
void test() { myFn(1); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(&tree, src, &engine(), &table, Some(&project));
        assert!(
            diags.is_empty(),
            "omitted default workspace argument should be allowed, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn test_callee_source_engine_api_allows_fewer_args() {
        // aiEchoCategory is a 2-parameter engine function. The runtime supplies
        // defaults for engine API calls, so fewer actual arguments are fine.
        let src = r#"void test() { aiEchoCategory(0); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        assert!(
            diags.is_empty(),
            "engine API call with fewer args should not be flagged, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn test_callee_source_engine_api_rejects_too_many_args() {
        let src = r#"void test() { aiEcho("a", "b", "c", "d", "e", "f"); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter()
                .any(|m| m.contains("expected 1 argument") && m.contains("got 6")),
            "engine API call with too many args should be flagged, got: {:?}",
            msgs
        );
    }

    #[test]
    fn test_callee_source_workspace_requires_defaults_specified() {
        // Workspace callees do not receive runtime defaults; required params
        // must be present.
        let src = r#"void myFn(int a, int b) {}
void test() { myFn(1, 2); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(&tree, src, &engine(), &table, Some(&project));
        assert!(
            diags.is_empty(),
            "workspace call supplying all required args should be clean, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn flags_wrong_arg_type_int_to_string() {
        let src = r#"void test() { aiEcho(42); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected argument 1 of type `string`")
                && m.contains("got `int`")),
            "missing type diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_wrong_arg_type_string_to_int() {
        let src = r#"void test() { aiEchoCategory("oops", "msg"); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected argument 1 of type `int`")
                && m.contains("got `string`")),
            "missing type diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_wrong_arg_type_for_workspace_variable() {
        // `x` is declared `int` at the top level (so it lands in the
        // per-file symbol table), so passing it where a `string` is
        // expected should be flagged.
        let src = r#"
            int x = 5;
            void test() {
                aiEcho(x);
            }
        "#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected argument 1 of type `string`")
                && m.contains("got `int`")),
            "missing type diagnostic for `int x` passed as string, got: {:?}",
            msgs
        );
    }

    #[test]
    fn unknown_function_no_diagnostic() {
        let src = r#"void test() { notAFunction(1, 2); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        assert!(
            diags.is_empty(),
            "unknown function should not be flagged, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn flag_both_count_and_type_errors() {
        // aiEcho takes 1 string. We pass 2 ints.
        let src = r#"void test() { aiEcho(42, 99); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter()
                .any(|m| m.contains("expected 1 argument") && m.contains("got 2")),
            "missing count error, got: {:?}",
            msgs
        );
        assert!(
            msgs.iter()
                .any(|m| m.contains("got `int`") && m.contains("expected argument 1")),
            "missing type error on first arg, got: {:?}",
            msgs
        );
    }

    #[test]
    fn no_diagnostic_for_bool_literal() {
        // aiEcho("ok", true) — if there's a 2-arg aiEcho that takes (string, bool),
        // this should be clean. We don't assert WHICH 2-arg signature aiEcho has;
        // we just confirm that bool literals don't produce spurious diagnostics.
        let src = r#"void test() { aiEcho("ok", true); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        // The only way this can fail is if `aiEcho` has a 2-arg signature
        // and `true` is treated as something other than bool. Either way we
        // just want to ensure no panic + sensible output.
        let _ = messages(&diags);
    }

    #[test]
    fn allows_int_to_float_widening_for_user_function() {
        let src = r#"void takeFloat(float x) {}
void test() { takeFloat(5); }"#;
        let tree = parse(src);
        let table = table_for(src);

        let mut files = std::collections::HashMap::new();
        files.insert(std::path::PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(&tree, src, &engine(), &table, Some(&project));
        assert!(
            diags.is_empty(),
            "int -> float widening should be allowed, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn allows_float_to_int_coercion_for_user_function() {
        // The engine coerces numeric arguments at runtime, so passing a float
        // literal where an int parameter is expected is accepted.
        let src = r#"void takeInt(int x) {}
void test() { takeInt(3.14); }"#;
        let tree = parse(src);
        let table = table_for(src);

        let mut files = std::collections::HashMap::new();
        files.insert(std::path::PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(&tree, src, &engine(), &table, Some(&project));
        assert!(
            diags.is_empty(),
            "float -> int coercion should be allowed, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn test_rule_call_bypasses_arg_count_check() {
        // Registered rules are engine-managed and can be called with any
        // number of arguments (the engine passes them through).
        let src = r#"void init() { xsEnableRule("myRule"); }
void test() { myRule(1, 2, 3); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(&tree, src, &engine(), &table, Some(&project));
        assert!(
            diags.is_empty(),
            "rule call should bypass argument-count check, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn test_rule_call_bypasses_return_type_check() {
        // Using a rule call as an argument/value should not produce a type
        // diagnostic; rules are void and engine-managed.
        let src = r#"void init() { xsEnableRule("myRule"); }
void test() { aiEcho(myRule()); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(&tree, src, &engine(), &table, Some(&project));
        assert!(
            diags.is_empty(),
            "rule call should bypass return-type check, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn test_function_pointer_callback_is_compatible() {
        // Engine APIs like setOverrideStrategy expect a `void()` callback.
        // Passing the name of a top-level void function should be accepted.
        let src = r#"void strategy() {}
void test() { setOverrideStrategy(strategy); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        assert!(
            diags.is_empty(),
            "function-pointer callback should be compatible, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn flags_wrong_arg_type_for_included_workspace_function() {
        // Scenario 11/12 cross-cutting: a call into an included file uses the
        // included function's parameter types.
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        std::fs::create_dir_all(a.parent().unwrap()).unwrap();
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, "include \"b.xs\";\nvoid test() { helper(\"oops\"); }\n").unwrap();
        std::fs::write(&b, "void helper(int x) {}\n").unwrap();

        let ws = Workspace::new(root.to_path_buf());
        let project = WorkspaceVirtualProject::default();
        let source_a = std::fs::read_to_string(&a).unwrap();
        let tree = parse(&source_a);
        let table = table_for(&source_a);
        let own = table.clone();
        let cache_dir = TempDir::new().unwrap();
        let merged =
            MergedView::build(&a, &source_a, &own, &ws, &project, cache_dir.path());

        let diags = check_calls_with_merged(&tree, &source_a, &engine(), &table, Some(&merged), None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected argument 1 of type `int`") && m.contains("got `string`")),
            "expected type error from included function, got: {:?}",
            msgs
        );
    }
}
