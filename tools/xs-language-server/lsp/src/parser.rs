//! Thin wrapper around the lewel-typed `xs_parser` for the LSP crate.
//!
//! Hides parser initialization and provides the few AST helpers the rest
//! of the LSP still needs: parsing, include-directive extraction, and
//! include-path detection under the cursor.

use std::path::Path;

use tower_lsp_server::ls_types::Range;
use xs_parser::ast::{TopLevelItem, TranslationUnit, TypeTable};
use xs_parser::parser::{Cst, NodeRef, Parser};

pub type Diagnostic = xs_parser::parser::Diagnostic;

/// Parse `source` as XS using the built-in primitive type table.
pub fn parse(source: &str) -> (Cst<'_>, Vec<Diagnostic>) {
    parse_with_types(source, &TypeTable::with_primitives())
}

/// Parse `source` as XS using a caller-supplied type table.
pub fn parse_with_types<'a>(source: &'a str, types: &TypeTable) -> (Cst<'a>, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let cst = Parser::new_with_context(source, &mut diags, types.clone()).parse(&mut diags);
    (cst, diags)
}

/// Extract every `include` directive from a parsed XS file.
///
/// Returns the include target with quotes stripped and the full source range
/// of the directive path. This replaces the line-regex approximation
/// previously used in `completion.rs`.
pub fn extract_include_directives(cst: &Cst, source: &str) -> Vec<(String, Range)> {
    let mut out = Vec::new();
    let Some(tu) = TranslationUnit::from_cst(cst, NodeRef::ROOT) else {
        return out;
    };
    for item in &tu.items {
        if let TopLevelItem::IncludeDirective(inc) = item {
            let target = strip_quotes(&inc.path.node);
            out.push((target.to_string(), crate::range::span_to_range(source, inc.path.span.clone())));
        }
    }
    out
}

/// If the cursor lies inside the path token of an `include` directive,
/// return the unquoted include target. Otherwise return `None`.
///
/// `file` is accepted for diagnostic/logging symmetry but is not used by
/// the current implementation.
pub fn detect_include_path_at_position(_file: &Path, source: &str, line: u32, col: u32) -> Option<String> {
    let (cst, _) = parse(source);
    let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT)?;
    let offset = crate::range::position_to_byte_offset(source, line, col)?;

    for item in &tu.items {
        let TopLevelItem::IncludeDirective(inc) = item else { continue };
        if contains(&inc.span, offset) {
            if contains(&inc.path.span, offset) {
                return Some(strip_quotes(&inc.path.node).to_string());
            }
            // Cursor is on the `include` keyword or the trailing `;` but
            // not inside the quoted path — fall through to identifier logic.
            return None;
        }
    }

    None
}

fn strip_quotes(s: &str) -> &str {
    s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s)
}

fn contains(span: &std::ops::Range<usize>, offset: usize) -> bool {
    span.start <= offset && offset < span.end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_returns_cst_and_diagnostics() {
        let source = "void foo() {}\n";
        let (cst, diags) = parse(source);
        assert_eq!(cst.source(), source);
        assert!(diags.is_empty());
    }

    #[test]
    fn extract_include_directives_strips_quotes() {
        let source = r#"include "core/main.xs";
void foo() {}
"#;
        let (cst, _) = parse(source);
        let directives = extract_include_directives(&cst, source);
        assert_eq!(directives.len(), 1);
        assert_eq!(directives[0].0, "core/main.xs");
    }

    #[test]
    fn detect_include_path_finds_cursor_in_path() {
        let source = r#"include "core/main.xs";
"#;
        // "core" starts at line 0, column 9.
        let path = detect_include_path_at_position(Path::new("/tmp/a.xs"), source, 0, 11);
        assert_eq!(path, Some("core/main.xs".to_string()));
    }

    #[test]
    fn detect_include_path_none_outside_path() {
        let source = r#"include "core/main.xs";
"#;
        let path = detect_include_path_at_position(Path::new("/tmp/a.xs"), source, 0, 3);
        assert_eq!(path, None);
    }
}
