//! `textDocument/signatureHelp` provider.
//!
//! Returns the call signature and highlighted active parameter for engine
//! syscalls and workspace-defined callables. The analysis is intentionally
//! text-based so it works on files that may not parse perfectly.

use tower_lsp_server::ls_types::{
    Documentation, MarkupContent, MarkupKind, ParameterInformation, ParameterLabel, Position,
    SignatureHelp, SignatureInformation,
};

use crate::{engine_api, merged_view::MergedView, range::position_to_byte_offset, symbols};

/// Return signature help for the call surrounding `pos`, if any.
pub fn signature_help(
    source: &str,
    pos: Position,
    engine: &engine_api::EngineApi,
    merged: Option<&MergedView>,
    own_table: Option<&symbols::SymbolTable>,
) -> Option<SignatureHelp> {
    let (callee, active_param) = find_call_at_cursor(source, pos.line, pos.character)?;

    if let Some(signature) = build_from_engine(&callee, engine) {
        return Some(signature.with_active_parameter(active_param));
    }

    if let Some(merged) = merged {
        if let Some(ms) = merged.find(&callee) {
            if let Some(signature) = build_from_symbol(&ms.symbol) {
                return Some(signature.with_active_parameter(active_param));
            }
        }
    }

    if let Some(table) = own_table {
        if let Some(sym) = table.find(&callee) {
            if let Some(signature) = build_from_symbol(sym) {
                return Some(signature.with_active_parameter(active_param));
            }
        }
    }

    None
}

trait SignatureInformationExt {
    fn with_active_parameter(self, active: u32) -> SignatureHelp;
}

impl SignatureInformationExt for SignatureInformation {
    fn with_active_parameter(self, active: u32) -> SignatureHelp {
        let count = self.parameters.as_ref().map(|p| p.len()).unwrap_or(0);
        let active = if count == 0 { 0 } else { active.min(count as u32 - 1) };
        SignatureHelp {
            signatures: vec![self],
            active_signature: Some(0),
            active_parameter: Some(active),
        }
    }
}

fn build_from_engine(name: &str, engine: &engine_api::EngineApi) -> Option<SignatureInformation> {
    let syscall = engine.find_syscall(name)?;
    let params: Vec<ParameterInformation> = syscall
        .params
        .iter()
        .map(|p| ParameterInformation {
            label: parameter_label(p.ty.as_str(), p.name.as_str(), p.default.as_deref()),
            documentation: None,
        })
        .collect();

    let rendered_params: Vec<String> = syscall
        .params
        .iter()
        .map(|p| parameter_signature(p.ty.as_str(), p.name.as_str(), p.default.as_deref()))
        .collect();

    let label = format!("{} {}({})", syscall.return_type, syscall.name, rendered_params.join(", "));
    let documentation = if syscall.help.is_empty() {
        None
    } else {
        Some(Documentation::MarkupContent(MarkupContent {
            kind:  MarkupKind::Markdown,
            value: syscall.help.clone(),
        }))
    };

    Some(SignatureInformation {
        label,
        documentation,
        parameters: Some(params),
        active_parameter: None,
    })
}

fn build_from_symbol(symbol: &symbols::Symbol) -> Option<SignatureInformation> {
    match symbol.kind {
        symbols::SymbolKind::Function | symbols::SymbolKind::ClassMethod => {
            let params: Vec<ParameterInformation> = symbol
                .params
                .iter()
                .map(|p| ParameterInformation {
                    label: parameter_label(p.ty.as_str(), p.name.as_str(), p.default.as_deref()),
                    documentation: None,
                })
                .collect();

            Some(SignatureInformation {
                label:      symbol.detail.clone(),
                documentation: None,
                parameters: Some(params),
                active_parameter: None,
            })
        }
        symbols::SymbolKind::Rule => Some(SignatureInformation {
            label:      format!("rule {}", symbol.name),
            documentation: None,
            parameters: Some(Vec::new()),
            active_parameter: None,
        }),
        _ => None,
    }
}

fn parameter_label(ty: &str, name: &str, default: Option<&str>) -> ParameterLabel {
    ParameterLabel::Simple(match default {
        Some(d) => format!("{} {} = {}", ty, name, d),
        None => format!("{} {}", ty, name),
    })
}

fn parameter_signature(ty: &str, name: &str, default: Option<&str>) -> String {
    match default {
        Some(d) => format!("{} {} = {}", ty, name, d),
        None => format!("{} {}", ty, name),
    }
}

/// Locate the call expression surrounding the cursor and compute the active
/// parameter index.
///
/// Walks backward from the cursor, skipping balanced `(`...`)` pairs, until it
/// finds the nearest unmatched `(`. The identifier immediately before that
///Parenis the callee. The active parameter is the number of commas at nesting
/// depth 0 between that `(` and the cursor, capped to `param_count - 1` later.
pub fn find_call_at_cursor(source: &str, line: u32, character: u32) -> Option<(String, u32)> {
    let cursor = position_to_byte_offset(source, line, character)?;

    let mut depth: i32 = 0;
    let mut open_paren: Option<usize> = None;
    let mut i = cursor;

    while i > 0 {
        let (prev_idx, ch) = prev_char(source, i)?;
        i = prev_idx;
        match ch {
            ')' => depth += 1,
            '(' => {
                if depth == 0 {
                    open_paren = Some(i);
                    break;
                }
                depth -= 1;
            }
            _ => {}
        }
    }

    let open = open_paren?;
    let callee = callee_before_open_paren(source, open)?;
    let active_param = count_commas_at_depth_zero(source, open + 1, cursor);
    Some((callee, active_param))
}

/// Count commas at nesting depth 0 between the open paren (exclusive) and the
/// cursor (inclusive).
fn count_commas_at_depth_zero(source: &str, start: usize, end: usize) -> u32 {
    let mut depth: i32 = 0;
    let mut count: u32 = 0;
    let mut i = start;
    while i < end && i < source.len() {
        let (next_idx, ch) = match next_char(source, i) {
            Some(v) => v,
            None => break,
        };
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => count += 1,
            _ => {}
        }
        i = next_idx;
    }
    count
}

/// Extract the identifier immediately before `open_paren`, allowing whitespace.
fn callee_before_open_paren(source: &str, open_paren: usize) -> Option<String> {
    let mut i = open_paren;
    // Skip whitespace before `(`.
    while i > 0 {
        let (prev_idx, ch) = prev_char(source, i)?;
        if !ch.is_whitespace() {
            break;
        }
        i = prev_idx;
    }

    // Now read the identifier backwards.
    let mut name = String::new();
    while i > 0 {
        let (prev_idx, ch) = prev_char(source, i)?;
        if ch.is_alphanumeric() || ch == '_' {
            name.push(ch);
            i = prev_idx;
        } else {
            break;
        }
    }
    if name.is_empty() {
        return None;
    }
    Some(name.chars().rev().collect())
}

/// Previous Unicode character boundary before `i`.
fn prev_char(source: &str, i: usize) -> Option<(usize, char)> {
    if i == 0 {
        return None;
    }
    let mut prev = i - 1;
    while prev > 0 && !source.is_char_boundary(prev) {
        prev -= 1;
    }
    source[prev..i].chars().next_back().map(|ch| (prev, ch))
}

/// Next Unicode character boundary at or after `i`.
fn next_char(source: &str, i: usize) -> Option<(usize, char)> {
    if i >= source.len() {
        return None;
    }
    let mut next = i + 1;
    while next < source.len() && !source.is_char_boundary(next) {
        next += 1;
    }
    source[i..next].chars().next().map(|ch| (next, ch))
}
