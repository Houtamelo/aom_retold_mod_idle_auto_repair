use std::env;
use std::fs;
use std::path::Path;

use tree_sitter::{Language, Parser, TreeCursor};
use tree_sitter_language::LanguageFn;

fn main() {
    let args: Vec<String> = env::args().collect();
    let path = Path::new(&args[1]);
    let source = fs::read_to_string(path).expect("failed to read input file");
    let lines: Vec<&str> = source.lines().collect();

    let mut parser = Parser::new();
    let language: Language = (tree_sitter_xs::LANGUAGE as LanguageFn).into();
    parser.set_language(&language).expect("set language");

    let tree = parser.parse(&source, None).unwrap();
    let root = tree.root_node();
    let mut cursor = root.walk();

    let target_lines: Vec<usize> = vec![17, 124, 423, 526, 527, 773];
    for line in target_lines {
        if line == 0 || line > lines.len() { continue; }
        let src_line = lines[line - 1];
        eprintln!("\n=== Line {}: {} ===", line, src_line);
        walk(&mut cursor, 0, line, &lines);
    }
}

fn walk(cursor: &mut TreeCursor, depth: usize, target: usize, lines: &[&str]) {
    let node = cursor.node();
    let start = node.start_position();
    let end = node.end_position();
    if (start.row + 1 == target || end.row + 1 == target)
        && (node.is_error() || node.is_missing() || depth < 4) {
        eprintln!("{}{}: kind={}, error={}, missing={}, range={}:{}-{}:{}",
                  " ".repeat(depth), depth, node.kind(),
                  node.is_error(), node.is_missing(),
                  start.row + 1, start.column + 1, end.row + 1, end.column + 1);
    }
    if cursor.goto_first_child() {
        walk(cursor, depth + 1, target, lines);
        while cursor.goto_next_sibling() {
            walk(cursor, depth + 1, target, lines);
        }
        cursor.goto_parent();
    }
}
