//! Thin wrapper around tree-sitter's `Parser` for XS source text.
//!
//! Building the `Parser` is cheap, but setting the language is a small
//! startup cost — kept here so call sites can `parse(src)` without
//! touching the tree-sitter API directly.

use std::path::Path;

use tower_lsp_server::ls_types::{Position, Range};
use tree_sitter::{Language, Parser, Tree};
use tree_sitter_language::LanguageFn;

/// Convenience: hand back the tree-sitter `Language` for XS.
pub fn xs_language() -> Language { (tree_sitter_xs::LANGUAGE as LanguageFn).into() }

/// Extract every `include_directive` from a parsed XS file.
///
/// Returns the include target with quotes stripped and the full source range
/// of the directive. This replaces the line-regex approximation previously
/// used in `completion.rs`.
pub fn extract_include_directives(tree: &Tree, source: &str) -> Vec<(String, Range)> {
    let mut out = Vec::new();
    let root = tree.root_node();
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() != "include_directive" {
            continue;
        }
        let range = node_range(child);
        let Some(path_node) = child.child_by_field_name("path") else {
            continue;
        };
        let text = &source[path_node.byte_range()];
        let target = text.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(text);
        out.push((target.to_string(), range));
    }
    out
}

/// If the cursor lies inside the path token of an `include_directive`,
/// return the unquoted include target. Otherwise return `None`.
///
/// `file` is accepted for diagnostic/logging symmetry but is not used by
/// the current implementation.
pub fn detect_include_path_at_position(_file: &Path, source: &str, line: u32, col: u32) -> Option<String> {
    let tree = parse(source)?;
    let root = tree.root_node();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() != "include_directive" {
            continue;
        }
        if !range_contains(child, line, col) {
            continue;
        }
        let Some(path_node) = child.child_by_field_name("path") else {
            continue;
        };
        if path_node.kind() != "string_literal" {
            continue;
        }
        if !range_contains(path_node, line, col) {
            // Cursor is on the `include` keyword or the trailing `;` but
            // not inside the quoted path — fall through to identifier logic.
            return None;
        }
        let text = &source[path_node.byte_range()];
        let target = text.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(text);
        return Some(target.to_string());
    }

    None
}

fn range_contains(node: tree_sitter::Node<'_>, line: u32, col: u32) -> bool {
    let start = node.start_position();
    let end = node.end_position();

    let after_start = line > start.row as u32 || (line == start.row as u32 && col >= start.column as u32);
    let before_end = line < end.row as u32 || (line == end.row as u32 && col < end.column as u32);

    after_start && before_end
}

fn node_range(node: tree_sitter::Node<'_>) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range::new(Position::new(start.row as u32, start.column as u32), Position::new(end.row as u32, end.column as u32))
}

/// Parse `source` as XS. Returns `None` if the language fails to install
/// (which would indicate a packaging bug, not a source-level issue).
pub fn parse(source: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&xs_language()).ok()?;
    parser.parse(source, None)
}

#[cfg(test)]
mod tests {
    use super::parse;

    fn named_child<'tree>(node: tree_sitter::Node<'tree>, kind: &str) -> Option<tree_sitter::Node<'tree>> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).find(|c| c.kind() == kind)
    }

    fn first_child_by_kind<'tree>(node: tree_sitter::Node<'tree>, kind: &str) -> Option<tree_sitter::Node<'tree>> {
        let mut cursor = node.walk();
        node.children(&mut cursor).find(|c| c.kind() == kind)
    }

    #[test]
    fn function_with_function_pointer_default_parses_as_function_definition() {
        let source = "void boVillager(int a = -1, void(int) afterQueue = [](int id = -1) {}) { }\n";
        let tree = parse(source).expect("parse");
        let root = tree.root_node();
        let func = root
            .named_children(&mut root.walk())
            .find(|c| c.kind() == "function_definition")
            .expect("function_definition");

        let name_node = named_child(func, "identifier").expect("name");
        assert_eq!(&source[name_node.byte_range()], "boVillager");

        let params = named_child(func, "parameter_list").expect("parameter_list");
        let param_decls: Vec<_> = params
            .named_children(&mut params.walk())
            .filter(|c| c.kind() == "parameter_declaration")
            .collect();
        assert_eq!(param_decls.len(), 2);

        let second = param_decls[1];
        assert!(
            named_child(second, "function_pointer_type").is_some(),
            "second parameter should have a function_pointer_type"
        );
        assert!(
            first_child_by_kind(second, "lambda_expression").is_some(),
            "second parameter default should be a lambda_expression"
        );
    }

    #[test]
    fn lambda_with_return_type_parses() {
        let source = "void boConditionalWait(int planID = -1, bool() condition = []() -> bool { return(true); }) { }\n";
        let tree = parse(source).expect("parse");
        let root = tree.root_node();
        let func = root
            .named_children(&mut root.walk())
            .find(|c| c.kind() == "function_definition")
            .expect("function_definition");
        let name_node = named_child(func, "identifier").expect("name");
        assert_eq!(&source[name_node.byte_range()], "boConditionalWait");

        let params = named_child(func, "parameter_list").expect("parameter_list");
        let param_decls: Vec<_> = params
            .named_children(&mut params.walk())
            .filter(|c| c.kind() == "parameter_declaration")
            .collect();
        assert_eq!(param_decls.len(), 2);
        let lambda = first_child_by_kind(param_decls[1], "lambda_expression").expect("lambda_expression default");
        assert!(lambda.child_by_field_name("return").is_some(), "lambda should have an explicit return type");
    }

    #[test]
    fn function_with_simple_default_still_parses() {
        let source = "void bar(int x = -1) { }\n";
        let tree = parse(source).expect("parse");
        let root = tree.root_node();
        let func = root
            .named_children(&mut root.walk())
            .find(|c| c.kind() == "function_definition")
            .expect("function_definition");
        let name_node = named_child(func, "identifier").expect("name");
        assert_eq!(&source[name_node.byte_range()], "bar");

        let params = named_child(func, "parameter_list").expect("parameter_list");
        let param_decls: Vec<_> = params
            .named_children(&mut params.walk())
            .filter(|c| c.kind() == "parameter_declaration")
            .collect();
        assert_eq!(param_decls.len(), 1);
        assert_eq!(named_child(param_decls[0], "identifier").map(|n| &source[n.byte_range()]), Some("x"));
    }
}
