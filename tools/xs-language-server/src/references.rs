//! Find all identifier uses of a name in a parsed XS file, plus helpers
//! used by the three LSP handlers:
//!   * `textDocument/references`
//!   * `textDocument/rename`
//!   * `textDocument/prepareRename`
//!
//! Week 4 of the post-spike roadmap. Workspace-wide resolution is week 5+.
//! For now we only walk the single file the cursor is in.
//!
//! `tree_sitter::Node<'a>` is `Copy` — pass by value.

use tower_lsp::lsp_types::{Location, Position, Range, Url};

/// Convert a tree-sitter `Node` to an LSP `Range`.
pub fn node_range(node: tree_sitter::Node<'_>) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range::new(
        Position::new(start.row as u32, start.column as u32),
        Position::new(end.row as u32, end.column as u32),
    )
}

/// Slice of source covered by `node`.
pub fn node_text<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

/// Walk the entire tree (root + every descendant). For every node whose
/// kind is `"identifier"` and whose source slice equals `name`, push its
/// range onto the output.
///
/// The identifier kind in the XS grammar is just `"identifier"`. We don't
/// care which kind of identifier it is (declaration vs. reference) — the
/// caller filters declaration matches via `filter_declaration`.
pub fn find_identifier_uses(tree: &tree_sitter::Tree, source: &str, name: &str) -> Vec<Range> {
    let mut out = Vec::new();
    walk(tree.root_node(), source, name, &mut out);
    out
}

fn walk(node: tree_sitter::Node<'_>, source: &str, name: &str, out: &mut Vec<Range>) {
    if node.kind() == "identifier" && node_text(node, source) == name {
        out.push(node_range(node));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk(child, source, name, out);
    }
}

/// Drop the declaration range when the caller didn't ask for it.
///
/// If `include_declaration` is true, the input is returned unchanged.
/// Otherwise we look up `name` in `table` and filter out any range that
/// equals the symbol's `selection_range`.
pub fn filter_declaration(
    ranges: Vec<Range>,
    table: &crate::symbols::SymbolTable,
    name: &str,
    include_declaration: bool,
) -> Vec<Range> {
    if include_declaration {
        return ranges;
    }
    let decl = match table.find(name) {
        Some(s) => s.selection_range,
        None => return ranges,
    };
    ranges.into_iter().filter(|r| *r != decl).collect()
}

/// Find the identifier node under `(line, character)` and return its range.
/// Used by `prepare_rename` — if the cursor is on whitespace or punctuation
/// (no enclosing identifier), return `None`.
pub fn identifier_range_at(tree: &tree_sitter::Tree, line: u32, character: u32) -> Option<Range> {
    let point = tree_sitter::Point::new(line as usize, character as usize);
    let mut current = tree.root_node().descendant_for_point_range(point, point)?;
    loop {
        if current.kind() == "identifier" {
            return Some(node_range(current));
        }
        match current.parent() {
            Some(p) => current = p,
            None => return None,
        }
    }
}

/// Convert a set of ranges into `Location`s anchored at `uri`.
pub fn to_locations(uri: &Url, ranges: Vec<Range>) -> Vec<Location> {
    ranges
        .into_iter()
        .map(|range| Location {
            uri: uri.clone(),
            range,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser;
    use crate::symbols::SymbolTable;

    fn parse(src: &str) -> tree_sitter::Tree {
        parser::parse(src).expect("parse")
    }

    #[test]
    fn find_finds_declaration_and_uses() {
        // helper is declared once and used in the body of a (different)
        // function `caller`. We only ask for `"helper"` here — the
        // single declaration should match.
        let src = "int helper(int a) { return a; }\n";
        let tree = parse(src);
        let ranges = find_identifier_uses(&tree, src, "helper");
        assert_eq!(ranges.len(), 1, "expected exactly one `helper` occurrence");
    }

    #[test]
    fn find_finds_multiple_callsites() {
        let src = "int helper(int a) { return a; }\n\
                   void caller() { helper(1); helper(2); }\n";
        let tree = parse(src);
        let ranges = find_identifier_uses(&tree, src, "helper");
        // 1 declaration + 2 callsites = 3.
        assert_eq!(
            ranges.len(),
            3,
            "expected 3 `helper` occurrences, got {}",
            ranges.len()
        );
    }

    #[test]
    fn filter_declaration_with_include_true() {
        let ranges = vec![
            Range::new(Position::new(0, 0), Position::new(0, 5)),
            Range::new(Position::new(1, 0), Position::new(1, 5)),
            Range::new(Position::new(2, 0), Position::new(2, 5)),
        ];
        let table = SymbolTable::default();
        let out = filter_declaration(ranges.clone(), &table, "anything", true);
        assert_eq!(out, ranges);
    }

    #[test]
    fn filter_declaration_with_include_false_drops_symbol_table_match() {
        let decl = Range::new(Position::new(0, 4), Position::new(0, 10));
        let ranges = vec![
            decl,
            Range::new(Position::new(1, 2), Position::new(1, 8)),
            Range::new(Position::new(2, 2), Position::new(2, 8)),
        ];
        // Build a fake symbol table whose selection_range matches the decl.
        let mut table = SymbolTable::default();
        table.symbols.push(crate::symbols::Symbol {
            name: "helper".to_string(),
            kind: crate::symbols::SymbolKind::Function,
            ty: "int".to_string(),
            params: vec![],
            is_extern: false,
            is_mutable: false,
            is_static: false,
            is_forward: false,
            visibility: crate::symbols::Visibility::Public,
            full_range: decl,
            selection_range: decl,
            detail: "int helper".to_string(),
        });
        let out = filter_declaration(ranges, &table, "helper", false);
        assert_eq!(out.len(), 2);
        assert!(
            !out.contains(&decl),
            "declaration range should be filtered out"
        );
    }

    #[test]
    fn identifier_range_at_returns_identifier_range() {
        // `   helper(1);` — col 5 is on 'h' of "helper".
        let src = "int helper(int a) { return a; }\n\
                   void caller()\n\
                   {\n   helper(1);\n}\n";
        let tree = parse(src);
        let r = identifier_range_at(&tree, 3, 5).expect("expected range");
        // The identifier "helper" starts at col 3 of line 3 and is 6 chars long.
        assert_eq!(r.start.line, 3);
        assert_eq!(r.start.character, 3);
        assert_eq!(r.end.line, 3);
        assert_eq!(r.end.character, 9);
    }

    #[test]
    fn identifier_range_at_returns_none_when_no_identifier() {
        // `   helper(1);` — col 1 is on a leading space.
        let src = "int helper(int a) { return a; }\n\
                   void caller()\n\
                   {\n   helper(1);\n}\n";
        let tree = parse(src);
        assert!(identifier_range_at(&tree, 3, 1).is_none());
    }
}
