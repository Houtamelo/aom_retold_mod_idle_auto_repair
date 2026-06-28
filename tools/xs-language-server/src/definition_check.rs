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
//!
//! These checks mirror the compiler so the LSP can flag errors before the
//! file is ever loaded by the engine.

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};
use tree_sitter::{Node, Tree};

/// Walk `tree` and return one diagnostic per definition that violates an
/// XS compiler rule.
pub fn validate_definitions(tree: &Tree, source: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_definition" => validate_function_params(child, source, &mut out),
            "declaration" => validate_declaration(child, source, &mut out),
            _ => {}
        }
    }
    out
}

fn validate_function_params(node: Node<'_>, source: &str, out: &mut Vec<Diagnostic>) {
    let Some(param_list) = find_named_child(node, "parameter_list") else {
        return;
    };
    let mut cursor = param_list.walk();
    for param in param_list.children(&mut cursor) {
        if param.kind() != "parameter_declaration" {
            continue;
        }
        let Some(name_node) = find_named_child(param, "identifier") else {
            continue;
        };
        let name = node_text(name_node, source);
        let is_ref = parameter_is_ref(param);
        let has_default = parameter_has_default(param);

        if is_ref && has_default {
            out.push(diagnostic(
                node_range(name_node),
                format!("ref parameter `{name}` cannot have a default value"),
            ));
        } else if !is_ref && !has_default {
            out.push(diagnostic(
                node_range(name_node),
                format!("non-ref parameter `{name}` must have a default value"),
            ));
        }
    }
}

/// True if the parameter declaration carries a `ref` type qualifier.
fn parameter_is_ref(param: Node<'_>) -> bool {
    let mut cursor = param.walk();
    for child in param.children(&mut cursor) {
        if child.kind() == "type_qualifier" {
            let mut inner = child.walk();
            for token in child.children(&mut inner) {
                if token.kind() == "ref" {
                    return true;
                }
            }
        }
    }
    false
}

/// True if the parameter declaration has an `= default` clause.
fn parameter_has_default(param: Node<'_>) -> bool {
    let mut cursor = param.walk();
    param.children(&mut cursor).any(|c| c.kind() == "=")
}

fn validate_declaration(node: Node<'_>, source: &str, out: &mut Vec<Diagnostic>) {
    // Forward function declaration: validate its parameters too.
    if let Some(declarator) = find_named_child(node, "function_declarator") {
        validate_function_params(declarator, source, out);
        return;
    }

    // `const` declarations must have a constant RHS expression.
    if let Some(init) = find_named_child(node, "init_declarator") {
        if declaration_has_modifier(node, "const") {
            if let Some(value_node) = init.child_by_field_name("value") {
                if !is_constant_expression(value_node, source) {
                    let name_node = find_named_child(init, "identifier");
                    let name = name_node.map(|n| node_text(n, source)).unwrap_or("?");
                    let range = name_node.map(node_range).unwrap_or_else(|| node_range(node));
                    out.push(diagnostic(
                        range,
                        format!("constant `{name}` must be assigned a constant expression"),
                    ));
                }
            }
        }
        return;
    }

    // No initializer: only SCALAR-typed variables are required to have one.
    // Class/struct instances (e.g. `AttackWave gFoo;`) are accepted by the
    // compiler with default field values, so they don't need an initializer
    // at the declaration site. The scalar set is fixed and known from the
    // grammar: `bool`, `int`, `float`, `string`, `vector`. Anything else is
    // a class type.
    if declaration_has_modifier(node, "extern") {
        return;
    }
    let Some(name_node) = find_named_child(node, "identifier") else {
        return;
    };
    let name = node_text(name_node, source);
    let type_node = find_named_child(node, "primitive_type")
        .or_else(|| find_named_child(node, "array_type"));
    match type_node {
        Some(ty) if is_scalar_type(ty, source) => {
            out.push(diagnostic(
                node_range(name_node),
                format!("variable `{name}` of scalar type must be initialized"),
            ));
        }
        _ => {
            // Class/struct type (or no resolvable type) — compiler accepts.
        }
    }
}

/// True if `type_node` is a scalar XS type per the grammar's
/// `primitive_type` set (`bool`, `int`, `float`, `string`, `vector`),
/// or an array of one of those.
///
/// The grammar at `tree-sitter-xs/src/grammar.json:1714` enumerates exactly:
///   bool | int | float | string | vector | void
/// We exclude `void` (no variable can be declared `void`) and treat any
/// other named type or array of named type as a class.
fn is_scalar_type(type_node: Node<'_>, source: &str) -> bool {
    if type_node.kind() == "array_type" {
        if let Some(elem) = type_node.child_by_field_name("element") {
            return is_scalar_type(elem, source);
        }
        return false;
    }
    if type_node.kind() != "primitive_type" {
        return false;
    }
    let text = node_text(type_node, source);
    !matches!(text, "void")
}

/// True if `node` (a `declaration`) has a storage-class/type-qualifier
/// child whose keyword text is `keyword` (e.g. `"extern"` or `"const"`).
fn declaration_has_modifier(node: Node<'_>, keyword: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "storage_class_specifier" && child.kind() != "type_qualifier" {
            continue;
        }
        let mut inner = child.walk();
        for token in child.children(&mut inner) {
            if token.kind() == keyword {
                return true;
            }
        }
    }
    false
}

/// Conservative check for a constant RHS expression: literals, identifiers,
/// unary/binary/parenthesized expressions over other constant expressions,
/// and the narrow set of built-in type constructors (`vector(...)`).
/// Everything else (function calls other than `vector`) is rejected.
fn is_constant_expression(node: Node<'_>, source: &str) -> bool {
    match node.kind() {
        "number_literal" | "string_literal" | "true" | "false" | "identifier" => true,
        "unary_expression" => node
            .child_by_field_name("argument")
            .map_or(false, |c| is_constant_expression(c, source)),
        "parenthesized_expression" => node
            .named_children(&mut node.walk())
            .next()
            .map_or(false, |c| is_constant_expression(c, source)),
        "expression" => node
            .named_children(&mut node.walk())
            .next()
            .map_or(false, |c| is_constant_expression(c, source)),
        // Binary expression over constants. The XS compiler accepts
        // `const int X = cFoo + 1;` and similar arithmetic over other
        // constants. Both operands must be constant expressions.
        "binary_expression" => {
            let mut cursor = node.walk();
            let operands: Vec<_> = node
                .named_children(&mut cursor)
                .filter(|c| c.kind() != "operator")
                .collect();
            operands.iter().all(|op| is_constant_expression(*op, source))
                && !operands.is_empty()
        }
        // `vector(...)` is a built-in type constructor. The XS engine treats
        // it as a literal value, not a function call result. The shipped game
        // scripts use it for every `const vector X = vector(...)` patrol-point
        // definition (~85 occurrences). All other call expressions
        // (`aiEcho(...)`, `xsVectorSet(...)`, etc.) remain rejected.
        "call_expression" => node
            .child_by_field_name("function")
            .map(|f| node_text(f, source) == "vector")
            .unwrap_or(false),
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

fn node_range(node: Node<'_>) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range::new(
        Position::new(start.row as u32, start.column as u32),
        Position::new(end.row as u32, end.column as u32),
    )
}

fn node_text<'a>(node: Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn find_named_child<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|c| c.kind() == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;

    fn parse(src: &str) -> Tree {
        parser::parse(src).expect("parse")
    }

    #[test]
    fn flags_non_ref_param_without_default() {
        let src = "void f(int x) {}\n";
        let diags = validate_definitions(&parse(src), src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("non-ref parameter `x` must have a default value")),
            "expected default-required diagnostic, got: {:?}", msgs
        );
    }

    #[test]
    fn flags_ref_param_with_default() {
        let src = "void f(ref int x = 0) {}\n";
        let diags = validate_definitions(&parse(src), src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("ref parameter `x` cannot have a default value")),
            "expected ref-cannot-have-default diagnostic, got: {:?}", msgs
        );
    }

    #[test]
    fn flags_uninitialized_top_level_variable() {
        // Scalar types (int, float, bool, string, vector) must be
        // initialized at declaration time. Class/struct instances are
        // exempted (see `allows_uninitialized_class_variable`).
        let src = "int x;\n";
        let diags = validate_definitions(&parse(src), src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "expected uninit-scalar diagnostic, got: {:?}", msgs
        );
    }

    #[test]
    fn allows_uninitialized_class_variable() {
        // Class/struct instances are accepted by the compiler with
        // default field values. The shipped game scripts use this pattern
        // heavily for things like `AttackWave gLandAttackWave;` which
        // are then configured via setter calls.
        let src = "AttackWave gFoo;\n";
        let diags = validate_definitions(&parse(src), src);
        assert!(
            diags.is_empty(),
            "class declaration without init should be allowed, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn flags_uninitialized_float_variable() {
        // Same rule as int — float is a scalar type per the grammar.
        let src = "float x;\n";
        let diags = validate_definitions(&parse(src), src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "float should be flagged as scalar, got: {:?}", msgs
        );
    }

    #[test]
    fn flags_uninitialized_bool_variable() {
        let src = "bool x;\n";
        let diags = validate_definitions(&parse(src), src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "bool should be flagged as scalar, got: {:?}", msgs
        );
    }

    #[test]
    fn flags_uninitialized_string_variable() {
        let src = "string x;\n";
        let diags = validate_definitions(&parse(src), src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "string should be flagged as scalar, got: {:?}", msgs
        );
    }

    #[test]
    fn flags_uninitialized_vector_variable() {
        let src = "vector x;\n";
        let diags = validate_definitions(&parse(src), src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("variable `x` of scalar type must be initialized")),
            "vector should be flagged as scalar, got: {:?}", msgs
        );
    }

    #[test]
    fn allows_uninitialized_extern_variable() {
        // `extern` declarations name a definition elsewhere and are
        // allowed without an initializer regardless of type.
        let src = "extern int gFoo;\n";
        let diags = validate_definitions(&parse(src), src);
        assert!(
            diags.is_empty(),
            "extern int should be allowed without init, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn flags_constant_assigned_non_constant() {
        let src = "const int x = aiEcho(\"hi\");\n";
        let diags = validate_definitions(&parse(src), src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("constant `x` must be assigned a constant expression")),
            "expected non-const-RHS diagnostic, got: {:?}", msgs
        );
    }

    #[test]
    fn allows_constant_assigned_literal() {
        let src = "const int x = 42;\n";
        let diags = validate_definitions(&parse(src), src);
        assert!(
            diags.is_empty(),
            "literal RHS should be allowed, got: {:?}", diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn allows_constant_assigned_other_constant() {
        let src = "const int cOne = 1;\nconst int cTwo = cOne;\n";
        let diags = validate_definitions(&parse(src), src);
        assert!(
            diags.is_empty(),
            "constant reference RHS should be allowed, got: {:?}", diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn allows_constant_assigned_arithmetic_over_constants() {
        // The XS compiler accepts `const int X = cFoo + 1;` — arithmetic
        // over other constants is itself a constant expression. Used
        // heavily in the shipped game scripts (e.g. `human_assist.xs`).
        let src = "const int cOne = 1;\nconst int cTwo = cOne + 1;\n";
        let diags = validate_definitions(&parse(src), src);
        assert!(
            diags.is_empty(),
            "binary expression over constants should be allowed, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn allows_constant_assigned_vector_constructor() {
        // `vector(...)` is a built-in type constructor. The XS engine
        // treats it as a literal value, not a function call result. The
        // shipped game scripts use it heavily for `const vector X = vector(...)`
        // patrol-point definitions.
        let src = "const vector v = vector(1.0, 2.0, 3.0);\n";
        let diags = validate_definitions(&parse(src), src);
        assert!(
            diags.is_empty(),
            "vector(...) should be a constant constructor, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }

    #[test]
    fn rejects_constant_assigned_other_call() {
        // `aiEcho(...)` is a real function call, not a constructor. The
        // engine does not treat it as a constant expression. The whitelisted
        // exception (`vector`) is narrowly scoped.
        let src = "const int x = aiEcho(\"hi\");\n";
        let diags = validate_definitions(&parse(src), src);
        assert!(
            diags.iter().any(|d| d.message.contains("constant `x` must be assigned a constant expression")),
            "non-constructor call should be rejected, got: {:?}",
            diags.iter().map(|d| &d.message).collect::<Vec<_>>()
        );
    }
}
