//! `textDocument/completion` handler.
//!
//! Day 6-7 of the spike. We don't yet walk the AST to figure out the exact
//! identifier at the cursor; for the spike we just scan back from the
//! cursor over `[A-Za-z0-9_]` characters and treat that as the prefix.
//! Phase 3 adds workspace symbols to the item list and scopes them by file.

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionParams};

use crate::engine_api::EngineApi;
use crate::merged_view::MergedView;
use crate::symbols::{Symbol, SymbolKind};

/// Return the identifier prefix immediately before the cursor, or `""`.
pub fn prefix_at_cursor(text: &str, line: u32, character: u32) -> String {
    let line_str = match text.lines().nth(line as usize) {
        Some(l) => l,
        None => return String::new(),
    };
    let col = (character as usize).min(line_str.len());
    let prefix_start = line_str[..col]
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '_')
        .last()
        .map(|(i, _)| i)
        .unwrap_or(col);
    line_str[prefix_start..col].to_string()
}

/// Build a `CompletionItem` for a syscall.
fn syscall_item(s: &crate::engine_api::Syscall) -> CompletionItem {
    let detail = format!(
        "{}({})",
        s.name,
        s.params
            .iter()
            .map(|p| format!("{} {}", p.ty, p.name))
            .collect::<Vec<_>>()
            .join(", ")
    );
    CompletionItem {
        label: s.name.clone(),
        kind: Some(CompletionItemKind::FUNCTION),
        detail: Some(format!("{} {}", s.return_type, detail)),
        documentation: Some(tower_lsp::lsp_types::Documentation::MarkupContent(
            tower_lsp::lsp_types::MarkupContent {
                kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                value: s.help.clone(),
            },
        )),
        ..Default::default()
    }
}

fn aiplan_item(c: &crate::engine_api::AiplanConstant) -> CompletionItem {
    CompletionItem {
        label: c.name.clone(),
        kind: Some(CompletionItemKind::CONSTANT),
        detail: Some(format!(
            "{} {} = {}",
            c.variable_type, c.name, c.variable_value
        )),
        documentation: Some(tower_lsp::lsp_types::Documentation::MarkupContent(
            tower_lsp::lsp_types::MarkupContent {
                kind: tower_lsp::lsp_types::MarkupKind::Markdown,
                value: format!("AI-plan constant (raw value: {})", c.value),
            },
        )),
        ..Default::default()
    }
}

/// Compute the completion list for the cursor position.
///
/// `merged` is the textual-paste scope for the file being edited. It already
/// contains the current file's symbols and the visibility-filtered symbols
/// from every resolved include, so no project-wide fallback is needed.
pub fn complete(
    api: &EngineApi,
    merged: Option<&MergedView>,
    text: &str,
    params: &CompletionParams,
) -> Vec<CompletionItem> {
    let pos = params.text_document_position.position;
    let prefix = prefix_at_cursor(text, pos.line, pos.character);
    let mut items: Vec<CompletionItem> = api
        .matching_syscalls(&prefix)
        .map(syscall_item)
        .collect();
    items.extend(api.matching_aiplans(&prefix).map(aiplan_item));

    if let Some(mv) = merged {
        items.extend(complete_merged(mv, &prefix));
    }

    items
}

/// Build completion items from the merged include-paste scope.
///
/// `MergedView` already filters visibility (`static`/file-local variables
/// from includes are hidden; `extern` variables and public functions are
/// kept), so we only need to match the prefix.
fn complete_merged(merged: &MergedView, prefix: &str) -> Vec<CompletionItem> {
    merged
        .matching(prefix)
        .map(|ms| symbol_to_completion_item(&ms.symbol))
        .collect()
}

fn symbol_to_completion_item(sym: &Symbol) -> CompletionItem {
    let kind = match sym.kind {
        SymbolKind::Rule => CompletionItemKind::FUNCTION,
        SymbolKind::Function => CompletionItemKind::FUNCTION,
        SymbolKind::Variable => CompletionItemKind::VARIABLE,
        SymbolKind::Constant => CompletionItemKind::CONSTANT,
    };
    CompletionItem {
        label: sym.name.clone(),
        kind: Some(kind),
        detail: Some(sym.detail.clone()),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use crate::merged_view::MergedView;
    use crate::parser;
    use crate::symbols;
    use crate::workspace::{VirtualProject, Workspace};

    fn api() -> EngineApi {
        EngineApi::default()
    }

    fn params_at(line: u32, character: u32) -> CompletionParams {
        use tower_lsp::lsp_types::TextDocumentIdentifier;
        CompletionParams {
            text_document_position: tower_lsp::lsp_types::TextDocumentPositionParams {
                text_document: TextDocumentIdentifier {
                    uri: tower_lsp::lsp_types::Url::parse("file:///tmp/test.xs").unwrap(),
                },
                position: tower_lsp::lsp_types::Position::new(line, character),
            },
            work_done_progress_params: Default::default(),
            partial_result_params: Default::default(),
            context: None,
        }
    }

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|i| i.label.as_str()).collect()
    }

    /// Build a `MergedView` for a fixture file under a temporary `game/` root.
    fn build_merged_view(current_rel: &str, current_src: &str, extra: &[(&str, &str)]) -> MergedView {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let current = root.join("game").join(current_rel);
        std::fs::create_dir_all(current.parent().unwrap()).unwrap();
        std::fs::write(&current, current_src).unwrap();
        for (rel, src) in extra {
            let path = root.join("game").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, src).unwrap();
        }

        let ws = Workspace::new(root.to_path_buf());
        let project = VirtualProject::default();
        let source = std::fs::read_to_string(&current).unwrap();
        let tree = parser::parse(&source).unwrap();
        let own = symbols::build_symbol_table(&tree, &source);
        let cache_dir = TempDir::new().unwrap();
        MergedView::build(&current, &source, &own, &ws, &project, cache_dir.path())
    }

    #[test]
    fn current_file_symbols_are_included() {
        let src = "int gLocal = 1;\n";
        let merged = build_merged_view("ai/current.xs", src, &[]);
        let items = complete(&api(), Some(&merged), src, &params_at(1, 0));
        assert!(labels(&items).contains(&"gLocal"));
    }

    #[test]
    fn included_file_symbols_are_offered() {
        // Scenario 13: a.xs includes b.xs and b.xs defines a function.
        let merged = build_merged_view(
            "ai/a.xs",
            "include \"b.xs\";\n",
            &[("ai/b.xs", "void included() {}\n")],
        );
        let items = complete(&api(), Some(&merged), "", &params_at(1, 0));
        let labels = labels(&items);
        assert!(
            labels.contains(&"included"),
            "included function missing: {:?}",
            labels
        );
    }

    #[test]
    fn included_visibility_filters_static_and_keeps_extern() {
        let merged = build_merged_view(
            "ai/a.xs",
            "include \"b.xs\";\n",
            &[(
                "ai/b.xs",
                "static int gHidden = 0;\nextern int gShared = 1;\n",
            )],
        );
        let items = complete(&api(), Some(&merged), "", &params_at(1, 0));
        let labels = labels(&items);
        assert!(labels.contains(&"gShared"), "extern variable should be visible");
        assert!(
            !labels.contains(&"gHidden"),
            "static variable from include should be hidden"
        );
    }

    #[test]
    fn prefix_filters_included_symbols() {
        let merged = build_merged_view(
            "ai/a.xs",
            "include \"b.xs\";\n",
            &[("ai/b.xs", "void alpha() {}\nvoid beta() {}\n")],
        );
        // Cursor after the include line with prefix "al".
        let text = "include \"b.xs\";\nal";
        let items = complete(&api(), Some(&merged), text, &params_at(1, 2));
        let labels = labels(&items);
        assert!(labels.contains(&"alpha"));
        assert!(!labels.contains(&"beta"));
    }

    #[test]
    fn no_merged_view_means_engine_api_only() {
        let items = complete(&api(), None, "", &params_at(0, 0));
        // Empty prefix with an empty engine API returns nothing.
        assert!(items.is_empty());
    }
}