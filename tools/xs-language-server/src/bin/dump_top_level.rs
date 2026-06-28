//! Custom AST dump for top-level constructs (rule_definition,
//! function_definition, declaration, preproc_def). Used to design the
//! symbol-table extraction logic in week 3. Not part of the LSP.

use std::env;
use std::fs;

use tree_sitter::{Language, Parser};
use tree_sitter_language::LanguageFn;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = &args[1];
    let source = fs::read_to_string(path).unwrap();

    let language: Language = (tree_sitter_xs::LANGUAGE as LanguageFn).into();
    let mut parser = Parser::new();
    parser.set_language(&language).unwrap();
    let tree = parser.parse(&source, None).unwrap();

    let root = tree.root_node();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        let kind = child.kind();
        let start = child.start_position();
        let end = child.end_position();
        let text = &source[child.byte_range()];

        if matches!(
            kind,
            "rule_definition" | "function_definition" | "declaration" | "preproc_def"
        ) {
            println!(
                "{} @ {}:{}-{}:{} '{:.70}'",
                kind,
                start.row + 1,
                start.column + 1,
                end.row + 1,
                end.column + 1,
                text.replace('\n', "\\n")
            );
            let mut sub = child.walk();
            for grandchild in child.children(&mut sub) {
                let gkind = grandchild.kind();
                let gtext = &source[grandchild.byte_range()];
                if gkind == "comment" || gkind == "rule_modifier" {
                    continue;
                }
                println!(
                    "  {} @ {}:{} '{:.60}'",
                    gkind,
                    grandchild.start_position().row + 1,
                    grandchild.start_position().column + 1,
                    gtext.replace('\n', "\\n")
                );
                // Drill into declarators / function declarators
                if matches!(
                    gkind,
                    "function_declarator" | "init_declarator" | "declarator"
                ) {
                    let mut sub2 = grandchild.walk();
                    for gg in grandchild.children(&mut sub2) {
                        let ggk = gg.kind();
                        let ggt = &source[gg.byte_range()];
                        if matches!(
                            ggk,
                            "identifier"
                                | "primitive_type"
                                | "array_type"
                                | "parameter_list"
                                | "storage_class_specifier"
                                | "type_qualifier"
                                | "field_identifier"
                        ) {
                            println!(
                                "    {} @ {}:{} '{:.50}'",
                                ggk,
                                gg.start_position().row + 1,
                                gg.start_position().column + 1,
                                ggt.replace('\n', "\\n")
                            );
                        }
                    }
                }
            }
            println!();
        }
    }
}