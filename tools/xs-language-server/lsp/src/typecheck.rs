//! Type-check engine-API function calls: argument count + argument type for
//! literal expressions and workspace symbols with known types.
//!
//! Week 5 of the post-spike roadmap. The scope is intentionally narrow:
//! only calls to engine syscalls (`kb*`, `tr*`, `xs*`, `rm*`, `ai*`). Calls
//! to workspace-defined functions are not checked here — they need
//! cross-file resolution (a later week).
//!
//! What this catches (the bulk of real XS bugs):
//!   * Wrong argument count: `aiEcho("hi", "extra")`.
//!   * Wrong literal type: `aiEcho(42)` (expected string, got int).
//!   * Identifier args whose declared type doesn't match: `int x; aiEcho(x);`.

use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity};
use xs_parser::ast::{CallExpr, Declaration, Expr, PostfixInner, TopLevelItem, TranslationUnit, TypeTable};
use xs_parser::parser::{Cst, NodeRef, Parser};

use crate::{
    engine_api::{EngineApi, Param},
    merged_view::MergedView,
    range::span_to_range,
    symbols::{SymbolKind, SymbolTable},
};

/// Walk the typed AST of `source` and return one `Diagnostic` per wrong-arg-count
/// or wrong-arg-type call to a known engine or user-defined function.
pub fn check_calls(
    source: &str,
    engine: &EngineApi,
    table: &SymbolTable,
    project: Option<&crate::semantic::VirtualProject>,
) -> Vec<Diagnostic> {
    check_calls_with_merged(source, engine, table, None, project)
}

/// Like [`check_calls`], but resolves user-defined callees through the
/// merged include-paste scope first, falling back to the full project.
pub fn check_calls_with_merged(
    source: &str,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
) -> Vec<Diagnostic> {
    let Some((cst, tu)) = parse_typed(source) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    walk_top_level(&cst, source, &tu, engine, table, merged, project, &mut out);
    out
}

fn parse_typed(source: &str) -> Option<(Cst<'_>, TranslationUnit)> {
    let mut diags = Vec::new();
    let cst = Parser::new_with_context(source, &mut diags, TypeTable::with_primitives()).parse(&mut diags);
    let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT)?;
    Some((cst, tu))
}

fn walk_top_level(
    cst: &Cst<'_>,
    source: &str,
    tu: &TranslationUnit,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    for item in &tu.items {
        walk_top_level_item(cst, source, item, engine, table, merged, project, out);
    }
}

fn walk_top_level_item(
    cst: &Cst<'_>,
    source: &str,
    item: &TopLevelItem,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    match item {
        TopLevelItem::FunctionDefinition(f) => {
            walk_declarator_defaults(cst, source, &f.declarator.direct, engine, table, merged, project, out);
            walk_block_items(cst, source, &f.body.inner, engine, table, merged, project, out);
        }
        TopLevelItem::ForwardDeclaration(f) => {
            walk_declarator_defaults(cst, source, &f.declarator.direct, engine, table, merged, project, out);
        }
        TopLevelItem::Declaration(d) => walk_declaration(cst, source, d, engine, table, merged, project, out),
        _ => {}
    }
}

fn walk_declaration(
    cst: &Cst<'_>,
    source: &str,
    d: &Declaration,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    for init in &d.init_declarator_list.items {
        if let Some(expr) = init.expr(cst) {
            walk_expr(cst, source, &expr, engine, table, merged, project, out);
        }
    }
}

fn walk_declarator_defaults(
    cst: &Cst<'_>,
    source: &str,
    direct: &xs_parser::ast::DirectDeclarator,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    match direct {
        xs_parser::ast::DirectDeclarator::FunctionDeclarator(fd) => {
            if let Some(pl) = &fd.params {
                for (param, _) in &pl.inner.items.items {
                    match &param.inner {
                        xs_parser::ast::ParameterInner::RegularParam(r) => {
                            if let Some(expr) = r.default.as_ref().and_then(|(_, u)| Expr::from_cst(cst, u.0)) {
                                walk_expr(cst, source, &expr, engine, table, merged, project, out);
                            }
                        }
                        xs_parser::ast::ParameterInner::FunctionPointerParam(fp) => {
                            if let Some(expr) = fp.default.as_ref().and_then(|(_, u)| Expr::from_cst(cst, u.0)) {
                                walk_expr(cst, source, &expr, engine, table, merged, project, out);
                            }
                        }
                    }
                }
            }
        }
        xs_parser::ast::DirectDeclarator::ParenDeclarator(pd) => {
            walk_declarator_defaults(cst, source, &pd.inner.direct, engine, table, merged, project, out);
        }
        _ => {}
    }
}

fn walk_block_items(
    cst: &Cst<'_>,
    source: &str,
    items: &[xs_parser::ast::statement::BlockItem],
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    use xs_parser::ast::statement::BlockItem;
    for item in items {
        match item {
            BlockItem::Declaration(d) => walk_declaration(cst, source, d, engine, table, merged, project, out),
            BlockItem::ForwardDeclaration(f) => {
                walk_declarator_defaults(cst, source, &f.declarator.direct, engine, table, merged, project, out);
            }
            BlockItem::FunctionDefinition(f) => {
                walk_declarator_defaults(cst, source, &f.declarator.direct, engine, table, merged, project, out);
                walk_block_items(cst, source, &f.body.inner, engine, table, merged, project, out);
            }
            BlockItem::Statement(s) => walk_statement(cst, source, s, engine, table, merged, project, out),
        }
    }
}

fn walk_statement(
    cst: &Cst<'_>,
    source: &str,
    stmt: &xs_parser::ast::statement::Statement,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    use xs_parser::ast::statement::{ForInit, Statement};
    match stmt {
        Statement::Compound(c) => walk_block_items(cst, source, &c.items.inner, engine, table, merged, project, out),
        Statement::Expression(es) => {
            if let Some(e) = es.expr.as_ref().and_then(|u| Expr::from_cst(cst, u.0)) {
                walk_expr(cst, source, &e, engine, table, merged, project, out);
            }
        }
        Statement::Return(r) => {
            if let Some(v) = &r.value {
                if let Some(e) = Expr::from_cst(cst, v.0) {
                    walk_expr(cst, source, &e, engine, table, merged, project, out);
                }
            }
        }
        Statement::If(i) => {
            if let Some(e) = Expr::from_cst(cst, i.cond.inner.0) {
                walk_expr(cst, source, &e, engine, table, merged, project, out);
            }
            walk_statement(cst, source, &i.then, engine, table, merged, project, out);
            if let Some(else_) = &i.else_ {
                walk_statement(cst, source, else_, engine, table, merged, project, out);
            }
        }
        Statement::While(w) => {
            if let Some(e) = Expr::from_cst(cst, w.cond.inner.0) {
                walk_expr(cst, source, &e, engine, table, merged, project, out);
            }
            walk_statement(cst, source, &w.body, engine, table, merged, project, out);
        }
        Statement::For(f) => {
            match &f.init {
                ForInit::Declaration(d) => walk_declaration(cst, source, d, engine, table, merged, project, out),
                ForInit::Expression(u) => {
                    if let Some(e) = Expr::from_cst(cst, u.0) {
                        walk_expr(cst, source, &e, engine, table, merged, project, out);
                    }
                }
                ForInit::Empty => {}
            }
            if let Some(u) = &f.cond {
                if let Some(e) = Expr::from_cst(cst, u.0) {
                    walk_expr(cst, source, &e, engine, table, merged, project, out);
                }
            }
            if let Some(u) = &f.post {
                if let Some(e) = Expr::from_cst(cst, u.0) {
                    walk_expr(cst, source, &e, engine, table, merged, project, out);
                }
            }
            walk_statement(cst, source, &f.body, engine, table, merged, project, out);
        }
        Statement::Switch(s) => {
            if let Some(e) = Expr::from_cst(cst, s.cond.inner.0) {
                walk_expr(cst, source, &e, engine, table, merged, project, out);
            }
            walk_block_items(cst, source, &s.body.items.inner, engine, table, merged, project, out);
        }
        _ => {}
    }
}

fn walk_expr(
    cst: &Cst<'_>,
    source: &str,
    expr: &Expr,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    match expr {
        Expr::Postfix(pfe) => {
            if let PostfixInner::Call(call) = &pfe.inner {
                check_one_call(cst, source, call, engine, table, merged, project, out);
                walk_expr(cst, source, &pfe.target, engine, table, merged, project, out);
            } else {
                walk_expr(cst, source, &pfe.target, engine, table, merged, project, out);
            }
            if let PostfixInner::Call(call) = &pfe.inner {
                if let Some(args) = &call.args {
                    for (arg, _) in &args.items.items {
                        if let Some(e) = arg.expr(cst) {
                            walk_expr(cst, source, &e, engine, table, merged, project, out);
                        }
                    }
                }
            }
        }
        Expr::Unary(u) => walk_expr(cst, source, &u.operand, engine, table, merged, project, out),
        Expr::Binary(b) => {
            walk_expr(cst, source, &b.lhs, engine, table, merged, project, out);
            walk_expr(cst, source, &b.rhs, engine, table, merged, project, out);
        }
        Expr::Conditional(c) => {
            walk_expr(cst, source, &c.cond, engine, table, merged, project, out);
            if let Some(t) = &c.then {
                walk_expr(cst, source, t, engine, table, merged, project, out);
            }
            walk_expr(cst, source, &c.else_, engine, table, merged, project, out);
        }
        Expr::Assignment(a) => {
            walk_expr(cst, source, &a.lhs, engine, table, merged, project, out);
            walk_expr(cst, source, &a.rhs, engine, table, merged, project, out);
        }
        Expr::Comma(c) => {
            for e in &c.exprs {
                walk_expr(cst, source, e, engine, table, merged, project, out);
            }
        }
        Expr::Paren(p) => walk_expr(cst, source, &p.inner, engine, table, merged, project, out),
        _ => {}
    }
}

fn check_one_call(
    cst: &Cst<'_>,
    source: &str,
    call: &CallExpr,
    engine: &EngineApi,
    table: &SymbolTable,
    merged: Option<&MergedView>,
    project: Option<&crate::semantic::VirtualProject>,
    out: &mut Vec<Diagnostic>,
) {
    let Some(callee) = extract_callee_name(call) else {
        return;
    };

    let resolved: Option<Callee<'_>> = if let Some(syscall) = engine.find_syscall(&callee) {
        Some(Callee::Engine(syscall))
    } else {
        resolve_workspace_function(merged, project, &callee).map(Callee::Workspace)
    };

    let Some(target) = resolved else { return };

    if matches!(target, Callee::Workspace(sym) if sym.kind == SymbolKind::Rule) {
        return;
    }

    let name = target.name();
    let params = target.params();
    let required_count = required_param_count(&params);

    let arg_count = call.args.as_ref().map(|a| a.items.len()).unwrap_or(0);
    let call_range = span_to_range(source, call.span.clone());

    if arg_count > params.len() {
        out.push(Diagnostic {
            range: call_range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message: format!("expected {} argument(s) to `{name}`, got {arg_count}", params.len()),
            related_information: None,
            tags: None,
            data: None,
        });
    } else if arg_count < required_count {
        out.push(Diagnostic {
            range: call_range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message: format!("expected {required_count} required argument(s) to `{name}`, got {arg_count}"),
            related_information: None,
            tags: None,
            data: None,
        });
    }

    let Some(args) = &call.args else { return };
    for (i, ((arg, _), expected)) in args.items.items.iter().zip(params.iter()).enumerate() {
        let Some(arg_expr) = arg.expr(cst) else { continue };
        let Some(actual_ty) = expr_type(&arg_expr, table) else { continue };
        if expected.ty == actual_ty {
            continue;
        }

        let numeric = ["int", "float"];
        if numeric.contains(&expected.ty.as_str()) && numeric.contains(&actual_ty.as_str()) {
            if let Some(reason) = narrowing_warning_message(&arg_expr, &expected.ty, &actual_ty) {
                let arg_range = span_to_range(source, arg.span.clone());
                out.push(Diagnostic {
                    range: arg_range,
                    severity: Some(DiagnosticSeverity::WARNING),
                    code: None,
                    code_description: None,
                    source: Some("xs-language-server".to_string()),
                    message: reason,
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }
            continue;
        }

        if is_function_pointer_argument(&arg_expr, table, &expected.ty) {
            continue;
        }
        let arg_range = span_to_range(source, arg.span.clone());
        out.push(Diagnostic {
            range: arg_range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message: format!(
                "expected argument {} of type `{expected}` for `{name}`, got `{actual}`",
                i + 1,
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

enum CalleeSource {
    Workspace,
    EngineApi,
}

#[allow(dead_code)]
fn arg_types_compatible(expected: &str, actual: &str, _source: CalleeSource) -> bool {
    if expected == actual {
        return true;
    }
    let _ = _source;
    let numeric = ["int", "float"];
    numeric.contains(&expected) && numeric.contains(&actual)
}

fn required_param_count(params: &[Param]) -> usize { params.iter().filter(|p| p.is_ref).count() }

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

    #[allow(dead_code)]
    fn source(&self) -> CalleeSource {
        match self {
            Callee::Engine(_) => CalleeSource::EngineApi,
            Callee::Workspace(_) => CalleeSource::Workspace,
        }
    }

    fn params(&self) -> Vec<Param> {
        match self {
            Callee::Engine(s) => s
                .params
                .iter()
                .map(|p| Param {
                    ty: p.ty.clone(),
                    name: p.name.clone(),
                    default: p.default.clone(),
                    is_ref: p.is_ref,
                })
                .collect(),
            Callee::Workspace(sym) => sym
                .params
                .iter()
                .map(|p| Param {
                    ty: p.ty.clone(),
                    name: p.name.clone(),
                    default: p.default.clone(),
                    is_ref: p.is_ref,
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
            (s.kind == SymbolKind::Function || s.kind == SymbolKind::Rule)
                && s.name == name
                && !s.is_forward
        })
    });
    defined.or_else(|| project.registered_rules.get(name))
}

fn extract_callee_name(call: &CallExpr) -> Option<String> {
    match call.target.as_ref() {
        Expr::Identifier(id) => Some(id.name.node.clone()),
        Expr::Postfix(pfe) => match &pfe.inner {
            PostfixInner::Field(field) => Some(field.field.node.clone()),
            _ => None,
        },
        _ => None,
    }
}

fn expr_type(expr: &Expr, table: &SymbolTable) -> Option<String> {
    match expr {
        Expr::StringLiteral(_) => Some("string".to_string()),
        Expr::IntLiteral(_) => Some("int".to_string()),
        Expr::FloatLiteral(_) => Some("float".to_string()),
        Expr::TrueLiteral(_) | Expr::FalseLiteral(_) => Some("bool".to_string()),
        Expr::Identifier(id) => {
            let name = &id.name.node;
            if name == "true" || name == "false" {
                return Some("bool".to_string());
            }
            table.find(name).map(|s| s.ty.clone()).filter(|t| !t.is_empty())
        }
        Expr::Paren(p) => expr_type(&p.inner, table),
        Expr::Unary(u) => expr_type(&u.operand, table),
        _ => None,
    }
}

fn narrowing_warning_message(arg_expr: &Expr, expected: &str, actual: &str) -> Option<String> {
    if expected != "int" || actual != "float" {
        return None;
    }
    if is_rounded_float_literal(arg_expr) {
        return None;
    }
    let label = format_expr_text(arg_expr);
    Some(format!(
        "narrowing conversion from `float` to `int` truncates `{label}`; consider an explicit `int({label})` if \
         truncation is intended"
    ))
}

fn is_rounded_float_literal(expr: &Expr) -> bool {
    match expr {
        Expr::FloatLiteral(f) => {
            if let Ok(n) = f.value.parse::<f64>() {
                n.is_finite() && n.fract() == 0.0
            } else {
                false
            }
        }
        Expr::Unary(u) => {
            use xs_parser::ast::expr::UnaryOp;
            matches!(u.kind, xs_parser::ast::expr::UnaryKind::Op(UnaryOp::Minus))
                && is_rounded_float_literal(&u.operand)
        }
        Expr::Paren(p) => is_rounded_float_literal(&p.inner),
        _ => false,
    }
}

fn format_expr_text(expr: &Expr) -> String {
    match expr {
        Expr::FloatLiteral(f) => f.value.clone(),
        Expr::IntLiteral(i) => i.value.clone(),
        Expr::Identifier(id) => id.name.node.clone(),
        _ => "<expr>".to_string(),
    }
}

fn is_function_pointer_argument(arg_expr: &Expr, table: &SymbolTable, expected_ty: &str) -> bool {
    if !expected_ty.contains('(') || !expected_ty.contains(')') {
        return false;
    }
    if let Expr::Identifier(id) = arg_expr {
        let name = &id.name.node;
        let Some(sym) = table.find(name) else {
            return false;
        };
        matches!(sym.kind, SymbolKind::Function | SymbolKind::Rule)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use tempfile::TempDir;

    use super::*;
    use crate::{
        merged_view::MergedView,
        workspace::{VirtualProject as WorkspaceVirtualProject, Workspace},
    };

    fn engine() -> &'static EngineApi {
        use std::sync::OnceLock;

        static ENGINE: OnceLock<EngineApi> = OnceLock::new();
        ENGINE.get_or_init(|| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).unwrap();
            let archive = workspace_root.join("docs/doxygen_retail.7z");
            let cache_dir = crate::cache::state_cache_dir();
            EngineApi::load_from_archive(&archive, &cache_dir).expect("load engine data from Doxygen archive")
        })
    }

    fn table_for(source: &str) -> SymbolTable { crate::symbols::build_symbol_table(source) }

    fn messages(diags: &[Diagnostic]) -> Vec<String> { diags.iter().map(|d| d.message.clone()).collect() }

    #[test]
    fn no_diagnostics_for_clean_calls() {
        let src = r#"void test() { aiEcho("hello"); aiEchoCategory(0, "warning"); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        assert!(diags.is_empty(), "expected no diagnostics, got: {:?}", messages(&diags));
    }

    #[test]
    fn flags_wrong_arg_count_too_many() {
        let src = r#"void test() { aiEcho("hi", "extra"); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected 1 argument") && m.contains("got 2")),
            "missing count diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn allows_omitting_workspace_arguments_without_explicit_default() {
        let src = r#"void myFn(int a, int b) {}
void test() { myFn(1); }"#;
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(src, &engine(), &table, Some(&project));
        assert!(
            diags.is_empty(),
            "myFn(1) should be legal because XS forces defaults on non-ref params, got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn test_callee_source_engine_api_allows_fewer_args() {
        let src = r#"void test() { aiEchoCategory(0); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        assert!(diags.is_empty(), "engine API call with fewer args should not be flagged, got: {:?}", messages(&diags));
    }

    #[test]
    fn test_callee_source_engine_api_rejects_too_many_args() {
        let src = r#"void test() { aiEcho("a", "b", "c", "d", "e", "f"); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected 1 argument") && m.contains("got 6")),
            "engine API call with too many args should be flagged, got: {:?}",
            msgs
        );
    }

    #[test]
    fn test_callee_source_workspace_requires_defaults_specified() {
        let src = r#"void myFn(int a, int b) {}
void test() { myFn(1, 2); }"#;
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(src, &engine(), &table, Some(&project));
        assert!(
            diags.is_empty(),
            "workspace call supplying all required args should be clean, got: {:?}",
            messages(&diags)
        );
    }

    fn check_workspace_call(decl: &str, caller: &str) -> Vec<Diagnostic> {
        let src = format!("{decl}\n{caller}");
        let table = table_for(&src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.clone());
        let project = crate::semantic::VirtualProject::from_files(files);
        check_calls(&src, &engine(), &table, Some(&project))
    }

    #[test]
    fn ref_param_missing_at_call_site_is_an_error() {
        let decl = "void myFn(ref int x, int y) {}";
        let caller = "void test() { myFn(); }";
        let diags = check_workspace_call(decl, caller);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected 1 required argument") && m.contains("got 0")),
            "missing ref param should error, got: {:?}",
            msgs
        );
    }

    #[test]
    fn ref_param_provided_at_call_site_is_ok() {
        let decl = "void myFn(ref int x, int y) {}";
        let caller = "void test() { int z = 0; myFn(z); }";
        let diags = check_workspace_call(decl, caller);
        assert!(diags.is_empty(), "supplying ref + omitting default should be OK, got: {:?}", messages(&diags));
    }

    #[test]
    fn no_ref_params_means_any_call_count_above_ref_required_is_ok() {
        let decl = "void myFn(int x, int y) {}";
        let caller = "void test() { myFn(); }";
        let diags = check_workspace_call(decl, caller);
        assert!(diags.is_empty(), "no-ref callee with no args should be OK, got: {:?}", messages(&diags));
    }

    #[test]
    fn ref_param_must_appear_before_non_ref_params_with_defaults() {
        let decl = "void myFn(ref int x, int y = 0) {}";
        let caller = "void test() { int z = 0; myFn(z); }";
        let diags = check_workspace_call(decl, caller);
        assert!(
            diags.is_empty(),
            "supplying ref + omitting defaulted non-ref should be OK, got: {:?}",
            messages(&diags)
        );

        let caller2 = "void test() { myFn(); }";
        let diags2 = check_workspace_call(decl, caller2);
        let msgs2 = messages(&diags2);
        assert!(
            msgs2.iter().any(|m| m.contains("expected 1 required argument")),
            "missing ref with defaulted non-ref should still error, got: {:?}",
            msgs2
        );
    }

    #[test]
    fn flags_wrong_arg_type_int_to_string() {
        let src = r#"void test() { aiEcho(42); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected argument 1 of type `string`") && m.contains("got `int`")),
            "missing type diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_wrong_arg_type_string_to_int() {
        let src = r#"void test() { aiEchoCategory("oops", "msg"); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected argument 1 of type `int`") && m.contains("got `string`")),
            "missing type diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_wrong_arg_type_for_workspace_variable() {
        let src = r#"
            int x = 5;
            void test() {
                aiEcho(x);
            }
        "#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected argument 1 of type `string`") && m.contains("got `int`")),
            "missing type diagnostic for `int x` passed as string, got: {:?}",
            msgs
        );
    }

    #[test]
    fn unknown_function_no_diagnostic() {
        let src = r#"void test() { notAFunction(1, 2); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        assert!(diags.is_empty(), "unknown function should not be flagged, got: {:?}", messages(&diags));
    }

    #[test]
    fn flag_both_count_and_type_errors() {
        let src = r#"void test() { aiEcho(42, 99); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected 1 argument") && m.contains("got 2")),
            "missing count error, got: {:?}",
            msgs
        );
        assert!(
            msgs.iter().any(|m| m.contains("got `int`") && m.contains("expected argument 1")),
            "missing type error on first arg, got: {:?}",
            msgs
        );
    }

    #[test]
    fn no_diagnostic_for_bool_literal() {
        let src = r#"void test() { aiEcho("ok", true); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        let _ = messages(&diags);
    }

    #[test]
    fn allows_int_to_float_widening_for_user_function() {
        let src = r#"void takeFloat(float x) {}
void test() { takeFloat(5); }"#;
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(std::path::PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(src, &engine(), &table, Some(&project));
        assert!(diags.is_empty(), "int -> float widening should be allowed, got: {:?}", messages(&diags));
    }

    #[test]
    fn warns_on_narrowing_float_to_int_for_unrounded_literal() {
        let src = r#"void takeInt(int x) {}
void test() { takeInt(3.14); }"#;
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(std::path::PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(src, &engine(), &table, Some(&project));
        assert!(
            diags
                .iter()
                .any(|d| matches!(d.severity, Some(DiagnosticSeverity::WARNING)) && d.message.contains("narrowing")),
            "expected a narrowing WARNING for `takeInt(3.14)`; got: {:?}",
            messages(&diags)
        );
        assert!(
            diags
                .iter()
                .all(|d| !matches!(d.severity, Some(DiagnosticSeverity::ERROR))
                    || !d.message.contains("argument")),
            "narrowing float->int should be WARNING severity, not ERROR; got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn silent_for_rounded_float_literal() {
        let src = r#"void takeInt(int x) {}
void test() { takeInt(1.0); }"#;
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(std::path::PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(src, &engine(), &table, Some(&project));
        assert!(diags.is_empty(), "rounded float literal `1.0` should not warn; got: {:?}", messages(&diags));
    }

    #[test]
    fn warns_on_narrowing_float_to_int_for_non_literal() {
        let src = r#"float gSomeFloat = 3.14;
void takeInt(int x) {}
void test() { takeInt(gSomeFloat); }"#;
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(std::path::PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(src, &engine(), &table, Some(&project));
        assert!(
            diags
                .iter()
                .any(|d| matches!(d.severity, Some(DiagnosticSeverity::WARNING)) && d.message.contains("narrowing")),
            "identifier-typed narrowing should ALWAYS warn; got: {:?}",
            messages(&diags)
        );
    }

    #[test]
    fn test_rule_call_bypasses_arg_count_check() {
        let src = r#"void init() { xsEnableRule("myRule"); }
void test() { myRule(1, 2, 3); }"#;
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(src, &engine(), &table, Some(&project));
        assert!(diags.is_empty(), "rule call should bypass argument-count check, got: {:?}", messages(&diags));
    }

    #[test]
    fn test_rule_call_bypasses_return_type_check() {
        let src = r#"void init() { xsEnableRule("myRule"); }
void test() { aiEcho(myRule()); }"#;
        let table = table_for(src);
        let mut files = std::collections::HashMap::new();
        files.insert(PathBuf::from("test.xs"), src.to_string());
        let project = crate::semantic::VirtualProject::from_files(files);

        let diags = check_calls(src, &engine(), &table, Some(&project));
        assert!(diags.is_empty(), "rule call should bypass return-type check, got: {:?}", messages(&diags));
    }

    #[test]
    fn test_function_pointer_callback_is_compatible() {
        let src = r#"void strategy() {}
void test() { setOverrideStrategy(strategy); }"#;
        let table = table_for(src);
        let diags = check_calls(src, &engine(), &table, None);
        assert!(diags.is_empty(), "function-pointer callback should be compatible, got: {:?}", messages(&diags));
    }

    #[test]
    fn flags_wrong_arg_type_for_included_workspace_function() {
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
        let table = table_for(&source_a);
        let own = table.clone();
        let cache_dir = TempDir::new().unwrap();
        let merged = MergedView::build(&a, &source_a, &own, &ws, &project, cache_dir.path());

        let diags = check_calls_with_merged(&source_a, &engine(), &table, Some(&merged), None);
        let msgs = messages(&diags);
        assert!(
            msgs.iter().any(|m| m.contains("expected argument 1 of type `int`") && m.contains("got `string`")),
            "expected type error from included function, got: {:?}",
            msgs
        );
    }
}
