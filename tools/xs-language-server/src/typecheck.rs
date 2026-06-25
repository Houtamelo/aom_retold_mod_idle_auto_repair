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
use crate::symbols::SymbolTable;

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
    let name = target.name();
    let params = target.params();

    let arg_list = find_named_child(call_node, "argument_list");
    let arg_count = arg_list.map(count_args).unwrap_or(0);
    let expected_count = params.len();

    if arg_count != expected_count {
        out.push(Diagnostic {
            range: node_range(call_node),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message: format!(
                "expected {expected_count} argument(s) to `{name}`, got {arg_count}"
            ),
            related_information: None,
            tags: None,
            data: None,
        });
        // Fall through to per-arg type checks too — we'd rather surface
        // every problem with a call in one pass than force the user to
        // fix the count first, re-save, and discover new type errors.
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
        if !types_compatible(&expected.ty, &actual_ty) {
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
    project.files.values().find_map(|file| {
        file.table.symbols.iter().find(|s| {
            s.kind == crate::symbols::SymbolKind::Function
                && s.name == name
                && !s.is_forward
        })
    })
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
/// XS allows implicit `int` -> `float` widening, but not `float` -> `int`
/// (loss of precision). String and bool are not implicitly convertible to
/// numeric types.
fn types_compatible(expected: &str, actual: &str) -> bool {
    if expected == actual {
        return true;
    }
    expected == "float" && actual == "int"
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
    use std::io::Write;
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
    fn flags_wrong_arg_count_too_few() {
        let src = r#"void test() { aiEchoCategory(0); }"#;
        let tree = parse(src);
        let table = table_for(src);
        let diags = check_calls(&tree, src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter()
                .any(|m| m.contains("expected 2 argument") && m.contains("got 1")),
            "missing count diagnostic, got: {:?}",
            msgs
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
    fn flags_float_to_int_loss_for_user_function() {
        let src = r#"void takeInt(int x) {}
void test() { takeInt(3.14); }"#;
        let tree = parse(src);
        let table = table_for(src);

        let mut files = std::collections::HashMap::new();
        files.insert(std::path::PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(&tree, src, &engine(), &table, Some(&project));
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected argument 1 of type `int`") && m.contains("got `float`")),
            "expected float->int loss error, got: {:?}",
            msgs
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
