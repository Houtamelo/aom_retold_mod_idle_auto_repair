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