//! `textDocument/completion` handler.
//!
//! Day 6-7 of the spike. We don't yet walk the AST to figure out the exact
//! identifier at the cursor; for the spike we just scan back from the
//! cursor over `[A-Za-z0-9_]` characters and treat that as the prefix.
//! Phase 3 adds workspace symbols to the item list and scopes them by file.

use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionParams};

use crate::engine_api::EngineApi;
use crate::semantic::VirtualProject;
use crate::symbols::{Symbol, SymbolKind, Visibility};

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
pub fn complete(
    api: &EngineApi,
    project: Option<&VirtualProject>,
    current_file: Option<&Path>,
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

    if let Some(p) = project {
        add_project_symbols(p, current_file, &prefix, &mut items);
    }

    items
}

fn add_project_symbols(
    project: &VirtualProject,
    current_file: Option<&Path>,
    prefix: &str,
    items: &mut Vec<CompletionItem>,
) {
    // Files included by the current file expose all their symbols; other
    // files only expose `extern` symbols.
    let included = included_files(text_of_current(project, current_file));

    for (path, file) in &project.files {
        let is_current = current_file.map(|c| c == path).unwrap_or(false);
        let is_included = included.iter().any(|target| path.ends_with(target));
        for sym in &file.table.symbols {
            if is_current || is_included || sym.visibility == Visibility::Extern {
                if !prefix.is_empty() && !sym.name.starts_with(prefix) {
                    continue;
                }
                items.push(symbol_to_completion_item(sym));
            }
        }
    }
}

fn text_of_current<'a>(
    project: &'a VirtualProject,
    current_file: Option<&Path>,
) -> Option<&'a String> {
    let cf = current_file?;
    project.files.get(cf).map(|f| &f.source)
}

/// Very lightweight include extraction: scan `source` for
/// `include "path/to/file.xs";` and return the set of relative targets.
fn included_files(source: Option<&String>) -> std::collections::HashSet<PathBuf> {
    use std::collections::HashSet;
    let mut out = HashSet::new();
    let Some(source) = source else { return out };
    for line in source.lines() {
        let line = line.trim();
        if !line.starts_with("include") {
            continue;
        }
        // Extract the quoted substring.
        let Some(start) = line.find('"') else { continue };
        let Some(end) = line[start + 1..].find('"') else { continue };
        let target = &line[start + 1..start + 1 + end];
        out.insert(Path::new(target).to_path_buf());
    }
    out
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
    use std::collections::HashMap;
    use std::path::PathBuf;

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

    fn project(files: &[(&str, &str)]) -> VirtualProject {
        let map: HashMap<PathBuf, String> = files
            .iter()
            .map(|(p, s)| (PathBuf::from(p), s.to_string()))
            .collect();
        VirtualProject::from_files(map)
    }

    fn labels(items: &[CompletionItem]) -> Vec<&str> {
        items.iter().map(|i| i.label.as_str()).collect()
    }

    #[test]
    fn current_file_symbols_are_included() {
        let src = "int gLocal = 1;\n";
        let mut files = HashMap::new();
        let current = PathBuf::from("current.xs");
        files.insert(current.clone(), src.to_string());
        let prj = VirtualProject::from_files(files);
        let items = complete(&api(), Some(&prj), Some(&current), src, &params_at(0, 0));
        assert!(labels(&items).contains(&"gLocal"));
    }

    #[test]
    fn extern_symbols_from_other_files_are_included() {
        let prj = project(&[
            ("current.xs", "void foo() {}\n"),
            ("other.xs", "extern int gExported = 1;\nint gHidden = 2;\n"),
        ]);
        let current = PathBuf::from("current.xs");
        let items = complete(&api(), Some(&prj), Some(&current), "", &params_at(0, 0));
        let labels = labels(&items);
        assert!(labels.contains(&"gExported"), "extern symbol missing");
        assert!(
            !labels.contains(&"gHidden"),
            "file-local symbol from other file should be hidden"
        );
    }

    #[test]
    fn other_file_public_function_is_hidden() {
        // Only `extern` symbols are exported across files; public functions
        // from other files stay hidden.
        let prj = project(&[
            ("current.xs", "void foo() {}\n"),
            ("other.xs", "void helper() {}\n"),
        ]);
        let current = PathBuf::from("current.xs");
        let items = complete(&api(), Some(&prj), Some(&current), "", &params_at(0, 0));
        let labels = labels(&items);
        assert!(!labels.contains(&"helper"), "other-file public function should be hidden");
        assert!(labels.contains(&"foo"));
    }

    #[test]
    fn prefix_filters_workspace_symbols() {
        let prj = project(&[("current.xs", "int alpha = 1;\nint beta = 2;\n")]);
        let current = PathBuf::from("current.xs");
        let items = complete(&api(), Some(&prj), Some(&current), "", &params_at(0, 0));
        // Filter down to prefix "al".
        let filtered: Vec<_> = items.into_iter().filter(|i| i.label.starts_with("al")).collect();
        let labels = labels(&filtered);
        assert!(labels.contains(&"alpha"));
        assert!(!labels.contains(&"beta"));
    }

    #[test]
    fn no_project_means_engine_api_only() {
        let items = complete(&api(), None, None, "", &params_at(0, 0));
        // Empty prefix with an empty engine API returns nothing.
        assert!(items.is_empty());
    }
}