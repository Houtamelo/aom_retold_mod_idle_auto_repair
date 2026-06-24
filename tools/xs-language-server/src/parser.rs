//! Thin wrapper around tree-sitter's `Parser` for XS source text.
//!
//! Building the `Parser` is cheap, but setting the language is a small
//! startup cost — kept here so call sites can `parse(src)` without
//! touching the tree-sitter API directly.

use tree_sitter::{Language, Parser, Tree};
use tree_sitter_language::LanguageFn;

/// Convenience: hand back the tree-sitter `Language` for XS.
pub fn xs_language() -> Language {
    (tree_sitter_xs::LANGUAGE as LanguageFn).into()
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

    fn named_child<'tree>(
        node: tree_sitter::Node<'tree>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'tree>> {
        let mut cursor = node.walk();
        node.named_children(&mut cursor).find(|c| c.kind() == kind)
    }

    fn first_child_by_kind<'tree>(
        node: tree_sitter::Node<'tree>,
        kind: &str,
    ) -> Option<tree_sitter::Node<'tree>> {
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
        let lambda = first_child_by_kind(param_decls[1], "lambda_expression")
            .expect("lambda_expression default");
        assert!(
            lambda.child_by_field_name("return").is_some(),
            "lambda should have an explicit return type"
        );
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
        assert_eq!(
            named_child(param_decls[0], "identifier")
                .map(|n| &source[n.byte_range()]),
            Some("x")
        );
    }
}