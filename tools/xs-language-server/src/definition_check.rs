//! Definition-time validation: rules enforced by the XS compiler at parse time.
//!
//! The XS compiler rejects several declaration patterns outright:
//!
//!   * non-`ref` parameters without a default value
//!   * `ref` parameters that carry a default value
//!   * top-level variables without an initializer
//!   * `const` variables initialized with a non-constant expression
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

    let is_extern = declaration_has_modifier(node, "extern");

    if let Some(init) = find_named_child(node, "init_declarator") {
        if declaration_has_modifier(node, "const") {
            if let Some(value_node) = init.child_by_field_name("value") {
                if !is_constant_expression(value_node) {
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

    // No initializer: only file-owned variables are required to have one.
    // `extern` declarations name a definition elsewhere and are allowed
    // without an initializer.
    if is_extern {
        return;
    }

    if let Some(name_node) = find_named_child(node, "identifier") {
        let name = node_text(name_node, source);
        out.push(diagnostic(
            node_range(name_node),
            format!("variable `{name}` must be initialized"),
        ));
    }
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
/// and unary expressions over other constant expressions. Everything else
/// (function calls, binary expressions, etc.) is rejected.
fn is_constant_expression(node: Node<'_>) -> bool {
    match node.kind() {
        "number_literal" | "string_literal" | "true" | "false" | "identifier" => true,
        "unary_expression" => node
            .child_by_field_name("argument")
            .map_or(false, is_constant_expression),
        "parenthesized_expression" => node
            .named_children(&mut node.walk())
            .next()
            .map_or(false, is_constant_expression),
        "expression" => node
            .named_children(&mut node.walk())
            .next()
            .map_or(false, is_constant_expression),
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
        let src = "int x;\n";
        let diags = validate_definitions(&parse(src), src);
        let msgs: Vec<_> = diags.iter().map(|d| d.message.as_str()).collect();
        assert!(
            msgs.iter().any(|m| m.contains("variable `x` must be initialized")),
            "expected uninit diagnostic, got: {:?}", msgs
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
}
