use std::env;
use std::fs;
use std::path::Path;

use tree_sitter::{Language, Node, Parser, TreeCursor};
use tree_sitter_language::LanguageFn;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: day1-probe <path-to-xs-file> [--grammar c|xs]");
        std::process::exit(2);
    }
    let path = Path::new(&args[1]);
    let source = fs::read_to_string(path).expect("failed to read input file");

    // Allow overriding the grammar choice.
    let grammar = if args.len() >= 4 && args[2] == "--grammar" {
        &args[3]
    } else {
        "c"
    };

    let language: Language = match grammar {
        "c" => (tree_sitter_c::LANGUAGE as LanguageFn).into(),
        "xs" => (tree_sitter_xs::LANGUAGE as LanguageFn).into(),
        other => {
            eprintln!("unknown grammar '{other}'");
            std::process::exit(2);
        }
    };
    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .expect("failed to set language");

    let tree = parser
        .parse(&source, None)
        .expect("parser returned None on valid input");

    let root = tree.root_node();
    let mut cursor = tree.walk();

    // Walk the tree and count ERROR/MISSING nodes.
    let mut error_count = 0usize;
    let mut missing_count = 0usize;
    let mut total_nodes = 0usize;
    let mut max_depth = 0usize;
    let mut samples: Vec<(usize, String, String)> = Vec::new();
    walk(
        &mut cursor,
        0,
        &mut error_count,
        &mut missing_count,
        &mut total_nodes,
        &mut max_depth,
        &mut samples,
        20,
    );

    println!("=== grammar: {grammar} ===");
    println!("file: {}", path.display());
    println!("bytes: {}", source.len());
    println!("lines: {}", source.lines().count());
    println!("total nodes: {total_nodes}");
    println!("max depth: {max_depth}");
    println!("ERROR nodes: {error_count}");
    println!("MISSING nodes: {missing_count}");
    println!("root kind: {}", root.kind());
    println!("root has_error: {}", root.has_error());

    // --tree or --tree-at <line> prints the parse tree (or just one node) as S-expressions.
    let dump_tree = args.iter().any(|a| a == "--tree");
    let tree_at: Option<usize> = args
        .iter()
        .position(|a| a == "--tree-at")
        .and_then(|i| args.get(i + 1).and_then(|s| s.parse().ok()));
    if dump_tree {
        print_node(&root, source.as_bytes(), 0);
    } else if let Some(line) = tree_at {
        // Find the smallest non-leaf node containing the line.
        let mut best: Option<Node> = None;
        let mut best_size: usize = usize::MAX;
        find_smallest_container(root, line, &mut best, &mut best_size);
        match best {
            Some(node) => print_node(&node, source.as_bytes(), 0),
            None => println!("(no node contains line {line})"),
        }
    }

    if !samples.is_empty() {
        println!("\n--- first {0} error/missing nodes ---", samples.len());
        for (depth, kind, pos) in &samples {
            println!("  depth={depth} kind={kind} pos={pos}");
        }
    }
}

fn walk(
    cursor: &mut TreeCursor,
    depth: usize,
    errors: &mut usize,
    missing: &mut usize,
    total: &mut usize,
    max_depth: &mut usize,
    samples: &mut Vec<(usize, String, String)>,
    sample_cap: usize,
) {
    let node = cursor.node();
    *total += 1;
    if depth > *max_depth {
        *max_depth = depth;
    }
    let kind = node.kind();
    if kind == "ERROR" {
        *errors += 1;
        if samples.len() < sample_cap {
            let pos = format!(
                "{}:{}",
                node.start_position().row + 1,
                node.start_position().column + 1
            );
            samples.push((depth, kind.to_string(), pos));
        }
    }
    if node.is_missing() {
        *missing += 1;
        if samples.len() < sample_cap {
            let pos = format!(
                "{}:{}",
                node.start_position().row + 1,
                node.start_position().column + 1
            );
            samples.push((depth, "MISSING".to_string(), pos));
        }
    }
    if cursor.goto_first_child() {
        walk(
            cursor,
            depth + 1,
            errors,
            missing,
            total,
            max_depth,
            samples,
            sample_cap,
        );
        while cursor.goto_next_sibling() {
            walk(
                cursor,
                depth + 1,
                errors,
                missing,
                total,
                max_depth,
                samples,
                sample_cap,
            );
        }
        cursor.goto_parent();
    }
}

fn find_smallest_container<'a>(
    node: Node<'a>,
    line: usize,
    best: &mut Option<Node<'a>>,
    best_size: &mut usize,
) {
    let start = node.start_position().row;
    let end = node.end_position().row;
    if !(start <= line && line <= end) {
        return;
    }
    let size = node.end_byte() - node.start_byte();
    if node.child_count() > 0 && size < *best_size {
        *best = Some(node);
        *best_size = size;
    }
    for child in node.children(&mut node.walk()) {
        find_smallest_container(child, line, best, best_size);
    }
}

fn print_node(node: &Node, source: &[u8], indent: usize) {
    let start = node.start_position();
    let end = node.end_position();
    let pad = " ".repeat(indent);
    let flag = if node.is_error() {
        "[ERR]"
    } else if node.is_missing() {
        "[MISS]"
    } else {
        ""
    };
    let text = node
        .utf8_text(source)
        .unwrap_or("<non-utf8>")
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(60)
        .collect::<String>();
    println!(
        "{pad}{flag}{kind} @ {line}:{col}-{end_line}:{end_col} '{text}'",
        kind = node.kind(),
        line = start.row + 1,
        col = start.column + 1,
        end_line = end.row + 1,
        end_col = end.column + 1,
    );
    for child in node.children(&mut node.walk()) {
        print_node(&child, source, indent + 2);
    }
}
