//! `textDocument/completion` handler.
//!
//! Day 6-7 of the spike. We don't yet walk the AST to figure out the exact
//! identifier at the cursor; for the spike we just scan back from the
//! cursor over `[A-Za-z0-9_]` characters and treat that as the prefix.
//! Richer context (member access `.`, namespace `::`) is post-spike.

use tower_lsp::lsp_types::{CompletionItem, CompletionItemKind, CompletionParams};

use crate::engine_api::EngineApi;

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
pub fn complete(api: &EngineApi, text: &str, params: &CompletionParams) -> Vec<CompletionItem> {
    let pos = params.text_document_position.position;
    let prefix = prefix_at_cursor(text, pos.line, pos.character);
    let mut items: Vec<CompletionItem> = api
        .matching_syscalls(&prefix)
        .map(syscall_item)
        .collect();
    items.extend(api.matching_aiplans(&prefix).map(aiplan_item));
    items
}