//! `textDocument/documentLink` provider.
//!
//! Returns clickable links for every `include "..."` directive whose target
//! resolves through the workspace, following the same mod-overlay-first,
//! vanilla-fallback rules used by go-to-definition.

use std::path::Path;

use tower_lsp_server::ls_types::{DocumentLink, Position, Range, Uri};

use crate::{parser, workspace};

/// Generate a `DocumentLink` for each resolvable `include "..."` directive in
/// `source`.
///
/// `current_file` is the absolute path of the file containing the includes.
/// `workspace` and `project` are used to resolve the include target through
/// the same overlay-aware machinery as semantic analysis.
pub fn document_links(
    source: &str,
    current_file: &Path,
    workspace: &workspace::Workspace,
    project: &workspace::VirtualProject,
) -> Vec<DocumentLink> {
    let (cst, _) = parser::parse(source);
    let directives = parser::extract_include_directives(&cst, source);

    let mut links = Vec::new();
    for (target, range) in directives {
        let Some(path_range) = shrink_range_by_quotes(range) else {
            continue;
        };
        let Some(resolved) = workspace.resolve_include_for_file(project, current_file, &target) else {
            continue;
        };
        let Some(uri) = Uri::from_file_path(&resolved) else {
            continue;
        };
        links.push(DocumentLink {
            range: path_range,
            target: Some(uri),
            tooltip: None,
            data: None,
        });
    }
    links
}

/// Shrink a range that includes the surrounding quotes so it covers only the
/// path literal text. Returns `None` for malformed/unexpected ranges.
fn shrink_range_by_quotes(range: Range) -> Option<Range> {
    if range.end.line != range.start.line {
        return None;
    }
    if range.end.character <= range.start.character + 1 {
        return None;
    }
    Some(Range::new(
        Position::new(range.start.line, range.start.character + 1),
        Position::new(range.end.line, range.end.character - 1),
    ))
}
