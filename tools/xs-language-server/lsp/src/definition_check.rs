//! Definition-time validation: rules enforced by the XS compiler at parse time.
//!
//! The XS compiler rejects several declaration patterns outright:
//!
//!   * non-`ref` parameters without a default value
//!   * `ref` parameters that carry a default value
//!   * top-level variables of scalar type without an initializer
//!   * `const` variables initialized with a non-constant expression
//!
//! Note: class/struct instances (e.g. `AttackWave gFoo;`) are accepted by
//! the compiler with default field values, so they don't need an initializer
//! at the declaration site. The scalar set is fixed by the grammar
//! (`bool | int | float | string | vector`) — anything else is a class.

use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Range};
use xs_parser::ast::{
    ClassDefinition, ClassMember, Declaration, Expr, ForwardDeclaration, FunctionDefinition,
    ParameterDeclaration, ParameterInner, TopLevelItem, TranslationUnit, Type, TypeSpecifier,
};
use xs_parser::parser::{Cst, NodeRef, Parser};

use crate::range::span_to_range;

/// Walk the typed AST of `source` and return one diagnostic per definition
/// that violates an XS compiler rule.
pub fn validate_definitions(source: &str) -> Vec<Diagnostic> {
    let Some((cst, tu)) = parse_typed(source) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for item in &tu.items {
        validate_top_level_item(&cst, source, item, &mut out);
    }
    out
}

fn parse_typed(source: &str) -> Option<(Cst<'_>, TranslationUnit)> {
    let mut diags = Vec::new();
    let cst = Parser::new_with_context(source, &mut diags, xs_parser::ast::TypeTable::with_primitives()).parse(&mut diags);
    let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT)?;
    Some((cst, tu))
}

fn base_name(direct: &xs_parser::ast::DirectDeclarator) -> &xs_parser::ast::Identifier {
    match direct {
        xs_parser::ast::DirectDeclarator::IdentDeclarator(id) => &id.name,
        xs_parser::ast::DirectDeclarator::ArrayDeclarator(ad) => &ad.base,
        xs_parser::ast::DirectDeclarator::FunctionDeclarator(fd) => &fd.base,
        xs_parser::ast::DirectDeclarator::ParenDeclarator(pd) => base_name(&pd.inner.direct),
    }
}

fn validate_top_level_item(
    cst: &Cst<'_>,
    source: &str,
    item: &TopLevelItem,
    out: &mut Vec<Diagnostic>,
) {
    match item {
        TopLevelItem::FunctionDefinition(f) => validate_function_definition(cst, source, f, out),
        TopLevelItem::ForwardDeclaration(f) => validate_forward_declaration(cst, source, f, out),
        TopLevelItem::ClassDefinition(c) => validate_class_definition(cst, source, c, out),
        TopLevelItem::Declaration(d) => validate_declaration(cst, source, d, out),
        _ => {}
    }
}

fn validate_function_definition(
    cst: &Cst<'_>,
    source: &str,
    f: &FunctionDefinition,
    out: &mut Vec<Diagnostic>,
) {
    validate_declarator_params(cst, source, &f.declarator.direct, out);
    validate_block_items(cst, source, &f.body.inner, out);
}

fn validate_forward_declaration(
    cst: &Cst<'_>,
    source: &str,
    f: &ForwardDeclaration,
    out: &mut Vec<Diagnostic>,
) {
    validate_declarator_params(cst, source, &f.declarator.direct, out);

    let is_extern = f
        .decl_specs
        .storage
        .iter()
        .any(|(s, _)| *s == xs_parser::ast::StorageClassSpecifier::Extern);
    if is_extern {
        return;
    }

    // A bare variable forward declaration (`int x;`) also needs an
    // initializer if its type is scalar.
    let is_function = matches!(f.declarator.direct, xs_parser::ast::DirectDeclarator::FunctionDeclarator(_));
    if is_function {
        return;
    }
    if is_scalar_type(&f.decl_specs.ty) {
        let name = &base_name(&f.declarator.direct).node;
        let name_range = span_to_range(source, base_name(&f.declarator.direct).span.clone());
        out.push(diagnostic(
            name_range,
            format!("variable `{name}` of scalar type must be initialized"),
        ));
    }
}

fn validate_class_definition(
    cst: &Cst<'_>,
    source: &str,
    c: &ClassDefinition,
    out: &mut Vec<Diagnostic>,
) {
    for member in &c.members.inner {
        match member {
            ClassMember::FunctionDefinition(f) => validate_function_definition(cst, source, f, out),
            ClassMember::ForwardDeclaration(f) => validate_forward_declaration(cst, source, f, out),
            _ => {}
        }
    }
}

fn validate_declaration(
    cst: &Cst<'_>,
    source: &str,
    d: &Declaration,
    out: &mut Vec<Diagnostic>,
) {
    if d.decl_specs.storage.iter().any(|(s, _)| *s == xs_parser::ast::StorageClassSpecifier::Extern) {
        return;
    }

    let is_const = d.decl_specs.is_const();
    for init in &d.init_declarator_list.items {
        let name = &base_name(&init.declarator.direct).node;
        let name_range = span_to_range(source, base_name(&init.declarator.direct).span.clone());

        if is_const {
            if let Some(expr) = init.expr(cst) {
                if !is_constant_expression(cst, &expr) {
                    out.push(diagnostic(
                        name_range,
                        format!("constant `{name}` must be assigned a constant expression"),
                    ));
                }
            } else {
                out.push(diagnostic(
                    name_range,
                    format!("constant `{name}` must be assigned a constant expression"),
                ));
            }
            continue;
        }

        let has_initializer = init.initializer.is_some();
        if !has_initializer && is_scalar_type(&d.decl_specs.ty) {
            out.push(diagnostic(
                name_range,
                format!("variable `{name}` of scalar type must be initialized"),
            ));
        }
    }
}

fn validate_declarator_params(
    cst: &Cst<'_>,
    source: &str,
    direct: &xs_parser::ast::DirectDeclarator,
    out: &mut Vec<Diagnostic>,
) {
    match direct {
        xs_parser::ast::DirectDeclarator::FunctionDeclarator(fd) => {
            if let Some(pl) = &fd.params {
                for (param, _) in &pl.inner.items.items {
                    validate_parameter(cst, source, param, out);
                }
            }
        }
        xs_parser::ast::DirectDeclarator::ParenDeclarator(pd) => {
            validate_declarator_params(cst, source, &pd.inner.direct, out);
        }
        _ => {}
    }
}

fn validate_parameter(
    cst: &Cst<'_>,
    source: &str,
    param: &ParameterDeclaration,
    out: &mut Vec<Diagnostic>,
) {
    let name = param_name(param);
    let name_range = name.map(|n| span_to_range(source, n.span.clone()));
    let Some(name_range) = name_range else { return };
    let name = name.unwrap().node.clone();

    let is_ref = param.decl_specs.is_ref();
    let has_default = parameter_has_default(cst, param);

    if is_ref && has_default {
        out.push(diagnostic(name_range, format!("ref parameter `{name}` cannot have a default value")));
    } else if !is_ref && !has_default {
        out.push(diagnostic(
            name_range,
            format!("non-ref parameter `{name}` must have a default value"),
        ));
    }
}

fn param_name(param: &ParameterDeclaration) -> Option<&xs_parser::ast::Identifier> {
    match &param.inner {
        ParameterInner::RegularParam(r) => r.name.as_ref(),
        ParameterInner::FunctionPointerParam(fp) => Some(&fp.name),
    }
}

fn parameter_has_default(cst: &Cst<'_>, param: &ParameterDeclaration) -> bool {
    match &param.inner {
        ParameterInner::RegularParam(r) => r.default.as_ref().and_then(|(_, u)| Expr::from_cst(cst, u.0)).is_some(),
        ParameterInner::FunctionPointerParam(fp) => {
            fp.default.as_ref().and_then(|(_, u)| Expr::from_cst(cst, u.0)).is_some()
        }
    }
}

fn validate_block_items(
    cst: &Cst<'_>,
    source: &str,
    items: &[xs_parser::ast::statement::BlockItem],
    out: &mut Vec<Diagnostic>,
) {
    use xs_parser::ast::statement::{BlockItem, ForInit, Statement};
    for item in items {
        match item {
            BlockItem::Declaration(d) => validate_declaration(cst, source, d, out),
            BlockItem::ForwardDeclaration(f) => validate_forward_declaration(cst, source, f, out),
            BlockItem::FunctionDefinition(f) => validate_function_definition(cst, source, f, out),
            BlockItem::Statement(s) => match s {
                Statement::If(i) => {
                    validate_statement(cst, source, &i.then, out);
                    if let Some(else_) = &i.else_ {
                        validate_statement(cst, source, else_, out);
                    }
                }
                Statement::While(w) => validate_statement(cst, source, &w.body, out),
                Statement::For(f) => {
                    if let ForInit::Declaration(d) = &f.init {
                        validate_declaration(cst, source, d, out);
                    }
                    validate_statement(cst, source, &f.body, out);
                }
                Statement::Compound(c) => validate_block_items(cst, source, &c.items.inner, out),
                Statement::Switch(s) => {
                    validate_block_items(cst, source, &s.body.items.inner, out);
                }
                _ => {}
            },
        }
    }
}

fn validate_statement(
    cst: &Cst<'_>,
    source: &str,
    stmt: &xs_parser::ast::statement::Statement,
    out: &mut Vec<Diagnostic>,
) {
    validate_block_items(cst, source, &[], out); // no-op to keep signature uniform
    use xs_parser::ast::statement::{ForInit, Statement};
    match stmt {
        Statement::Compound(c) => validate_block_items(cst, source, &c.items.inner, out),
        Statement::If(i) => {
            validate_statement(cst, source, &i.then, out);
            if let Some(else_) = &i.else_ {
                validate_statement(cst, source, else_, out);
            }
        }
        Statement::While(w) => validate_statement(cst, source, &w.body, out),
        Statement::For(f) => {
            if let ForInit::Declaration(d) = &f.init {
                validate_declaration(cst, source, d, out);
            }
            validate_statement(cst, source, &f.body, out);
        }
        Statement::Switch(s) => {
            validate_block_items(cst, source, &s.body.items.inner, out);
        }
        _ => {}
    }
}

/// True when `ty` denotes a scalar XS type (`bool`, `int`, `float`,
/// `string`, `vector`). Arrays of scalar types are also scalar.
fn is_scalar_type(ty: &TypeSpecifier) -> bool {
    if ty.is_array {
        return is_scalar_type(&xs_parser::ast::DeclarationSpecifiers {
            storage: vec![],
            ty: xs_parser::ast::type_system::TypeSpecifier {
                ty: ty.ty.clone(),
                is_array: false,
                fn_pointer: None,
                span: ty.span.clone(),
            },
            type_quals: vec![],
            span: ty.span.clone(),
        }.ty);
    }
    matches!(
        ty.ty,
        Type::Bool | Type::Int | Type::Float | Type::String | Type::Vector
    )
}

/// Conservative constant-expression check over the typed AST. Accepts
/// literals, identifiers, unary/binary expressions over constants,
/// parenthesised constants, and the special `vector(...)` constructor.
fn is_constant_expression(cst: &Cst<'_>, expr: &Expr) -> bool {
    match expr {
        Expr::IntLiteral(_)
        | Expr::FloatLiteral(_)
        | Expr::StringLiteral(_)
        | Expr::TrueLiteral(_)
        | Expr::FalseLiteral(_)
        | Expr::NullLiteral(_)
        | Expr::Default(_)
        | Expr::Identifier(_) => true,
        Expr::Paren(p) => is_constant_expression(cst, &p.inner),
        Expr::Unary(u) => is_constant_expression(cst, &u.operand),
        Expr::Binary(b) => {
            is_constant_expression(cst, &b.lhs) && is_constant_expression(cst, &b.rhs)
        }
        Expr::Conditional(c) => {
            is_constant_expression(cst, &c.cond)
                && c.then.as_ref().map_or(true, |e| is_constant_expression(cst, e))
                && is_constant_expression(cst, &c.else_)
        }
        Expr::Comma(c) => c.exprs.iter().all(|e| is_constant_expression(cst, e)),
        Expr::Postfix(p) => {
            // Only `vector(...)` is accepted as a constant constructor.
            if let xs_parser::ast::expr::PostfixInner::Call(_call) = &p.inner {
                if let Expr::Identifier(id) = p.target.as_ref() {
                    if id.name.node == "vector" {
                        return true;
                    }
                }
            }
            false
        }
        _ => false,
    }
}

fn diagnostic(range: Range, message: String) -> Diagnostic {
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("xs-language-server".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flags_non_ref_param_without_default() {
        let src = "void f(int x) {}\n";
        let diags = validate_definitions(src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter()
                .any(|m| m.contains("non-ref parameter `x` must have a default value")),
            "expected default-required diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_ref_param_with_default() {
        let src = "void f(ref int x = 0) {}\n";
        let diags = validate_definitions(src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter()
                .any(|m| m.contains("ref parameter `x` cannot have a default value")),
            "expected ref-cannot-have-default diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_uninitialized_top_level_variable() {
        let src = "int x;\n";
        let diags = validate_definitions(src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter()
                .any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "expected uninit-scalar diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn allows_uninitialized_class_variable() {
        let src = "AttackWave gFoo;\n";
        let diags = validate_definitions(src);
        assert!(
            diags.is_empty(),
            "class declaration without init should be allowed, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn flags_uninitialized_float_variable() {
        let src = "float x;\n";
        let diags = validate_definitions(src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter()
                .any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "float should be flagged as scalar, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_uninitialized_bool_variable() {
        let src = "bool x;\n";
        let diags = validate_definitions(src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter()
                .any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "bool should be flagged as scalar, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_uninitialized_string_variable() {
        let src = "string x;\n";
        let diags = validate_definitions(src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter()
                .any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "string should be flagged as scalar, got: {:?}",
            msgs
        );
    }

    #[test]
    fn flags_uninitialized_vector_variable() {
        let src = "vector x;\n";
        let diags = validate_definitions(src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter()
                .any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "vector should be flagged as scalar, got: {:?}",
            msgs
        );
    }

    #[test]
    fn allows_uninitialized_extern_variable() {
        let src = "extern int gFoo;\n";
        let diags = validate_definitions(src);
        assert!(
            diags.is_empty(),
            "extern int should be allowed without init, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn flags_constant_assigned_non_constant() {
        let src = "const int x = aiEcho(\"hi\");\n";
        let diags = validate_definitions(src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter()
                .any(|m| m.contains("constant `x` must be assigned a constant expression")),
            "expected non-const-RHS diagnostic, got: {:?}",
            msgs
        );
    }

    #[test]
    fn allows_constant_assigned_literal() {
        let src = "const int x = 42;\n";
        let diags = validate_definitions(src);
        assert!(
            diags.is_empty(),
            "literal RHS should be allowed, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn allows_constant_assigned_other_constant() {
        let src = "const int cOne = 1;\nconst int cTwo = cOne;\n";
        let diags = validate_definitions(src);
        assert!(
            diags.is_empty(),
            "constant reference RHS should be allowed, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn allows_constant_assigned_arithmetic_over_constants() {
        let src = "const int cOne = 1;\nconst int cTwo = cOne + 1;\n";
        let diags = validate_definitions(src);
        assert!(
            diags.is_empty(),
            "binary expression over constants should be allowed, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn allows_constant_assigned_vector_constructor() {
        let src = "const vector v = vector(1.0, 2.0, 3.0);\n";
        let diags = validate_definitions(src);
        assert!(
            diags.is_empty(),
            "vector(...) should be a constant constructor, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rejects_constant_assigned_other_call() {
        let src = "const int x = aiEcho(\"hi\");\n";
        let diags = validate_definitions(src);
        assert!(
            diags.iter().any(|d| d
                .message
                .contains("constant `x` must be assigned a constant expression")),
            "non-constructor call should be rejected, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
}
