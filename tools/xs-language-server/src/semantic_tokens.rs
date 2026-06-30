//! LSP semantic-token provider for XS.
//!
//! Emits `textDocument/semanticTokens/full` tokens for functions, variables,
//! and type references classified by origin (`engine`, `modded`, `unmodded`)
//! and, for variables, by storage class (`local`, `static`).

use std::path::Path;

use tower_lsp::lsp_types::{
    Range, SemanticToken, SemanticTokenModifier, SemanticTokenType, SemanticTokensFullOptions,
    SemanticTokensLegend, SemanticTokensOptions, SemanticTokensServerCapabilities,
};

use crate::engine_api;
use crate::merged_view::MergedView;
use crate::symbols::{self, Symbol, SymbolKind, Visibility};
use crate::workspace::{VirtualProject, Workspace};

const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::TYPE,
    SemanticTokenType::new("constant"),
    SemanticTokenType::new("rule"),
];

const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::new("engine"),
    SemanticTokenModifier::new("modded"),
    SemanticTokenModifier::new("unmodded"),
    SemanticTokenModifier::new("local"),
    SemanticTokenModifier::new("static"),
    SemanticTokenModifier::new("extern"),
];

/// A single semantic token in source-order, before LSP delta encoding.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub line: u32,
    pub char: u32,
    pub len: u32,
    pub token_type: SemanticTokenType,
    pub modifiers: Vec<SemanticTokenModifier>,
}

/// Server capability advertised to LSP clients.
pub fn server_capabilities() -> SemanticTokensServerCapabilities {
    SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
        legend: SemanticTokensLegend {
            token_types: TOKEN_TYPES.into(),
            token_modifiers: TOKEN_MODIFIERS.into(),
        },
        full: Some(SemanticTokensFullOptions::Bool(true)),
        range: Some(false),
        work_done_progress_options: Default::default(),
    })
}

/// Compute semantic tokens for `source`.
///
/// `current_file` is the absolute path of the file being analysed, used to
/// classify the origin of symbols defined in it. `own_table` is the per-file
/// symbol table (including local variable declarations). `merged` is the
/// include-paste merged view, if one was built.
pub fn compute_tokens(
    source: &str,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
) -> Vec<Token> {
    let Some(tree) = crate::parser::parse(source) else {
        return Vec::new();
    };

    let mut tokens = Vec::new();
    let mut cursor = tree.root_node().walk();
    walk_for_tokens(
        tree.root_node(),
        source,
        &mut cursor,
        current_file,
        own_table,
        merged,
        engine,
        workspace,
        project,
        &mut tokens,
    );

    // LSP requires tokens sorted by position with stable order.
    tokens.sort_by(|a, b| a.line.cmp(&b.line).then(a.char.cmp(&b.char)));
    tokens
}

fn walk_for_tokens(
    node: tree_sitter::Node<'_>,
    source: &str,
    cursor: &mut tree_sitter::TreeCursor<'_>,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
    tokens: &mut Vec<Token>,
) {
    match node.kind() {
        "identifier" => {
            if let Some(token) = classify_identifier(
                node,
                source,
                current_file,
                own_table,
                merged,
                engine,
                workspace,
                project,
            ) {
                tokens.push(token);
            }
        }
        "_type_identifier" | "type_identifier" => {
            if let Some(token) = classify_type_identifier(
                node,
                source,
                current_file,
                own_table,
                merged,
                workspace,
                project,
            ) {
                tokens.push(token);
            }
        }
        "primitive_type" => {
            tokens.push(Token {
                line: node.start_position().row as u32,
                char: node.start_position().column as u32,
                len: (node.end_byte() - node.start_byte()) as u32,
                token_type: SemanticTokenType::TYPE,
                modifiers: vec![SemanticTokenModifier::new("engine")],
            });
        }
        _ => {}
    }

    if cursor.goto_first_child() {
        loop {
            walk_for_tokens(
                cursor.node(),
                source,
                cursor,
                current_file,
                own_table,
                merged,
                engine,
                workspace,
                project,
                tokens,
            );
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn node_text<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn node_range(node: tree_sitter::Node<'_>) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range::new(
        tower_lsp::lsp_types::Position::new(start.row as u32, start.column as u32),
        tower_lsp::lsp_types::Position::new(end.row as u32, end.column as u32),
    )
}

fn classify_identifier(
    node: tree_sitter::Node<'_>,
    source: &str,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    engine: &engine_api::SharedEngineApi,
    workspace: &Workspace,
    project: &VirtualProject,
) -> Option<Token> {
    let name = node_text(node, source);
    if name.is_empty() {
        return None;
    }

    let (symbol, defining_path): (Option<&Symbol>, Option<&Path>) = if let Some(ms) =
        merged.and_then(|m| m.find(name))
    {
        (
            Some(&ms.symbol),
            ms.provenance.origin(),
        )
    } else if let Some(sym) = own_table.find(name) {
        (Some(sym), current_file)
    } else if engine.find_syscall(name).is_some() {
        return Some(Token {
            line: node.start_position().row as u32,
            char: node.start_position().column as u32,
            len: (node.end_byte() - node.start_byte()) as u32,
            token_type: SemanticTokenType::FUNCTION,
            modifiers: vec![SemanticTokenModifier::new("engine")],
        });
    } else {
        (None, None)
    };

    let Some(symbol) = symbol else { return None };
    let origin = defining_path
        .map(|p| classify_origin(p, workspace, project))
        .unwrap_or(Origin::Engine);
    let token_type = match symbol.kind {
        SymbolKind::Function => SemanticTokenType::FUNCTION,
        SymbolKind::Variable => SemanticTokenType::VARIABLE,
        SymbolKind::Constant => SemanticTokenType::new("constant"),
        SymbolKind::Rule => SemanticTokenType::new("rule"),
        SymbolKind::Class => SemanticTokenType::TYPE,
    };

    Some(Token {
        line: node.start_position().row as u32,
        char: node.start_position().column as u32,
        len: (node.end_byte() - node.start_byte()) as u32,
        token_type,
        modifiers: classify_modifiers(symbol, origin),
    })
}

fn classify_type_identifier(
    node: tree_sitter::Node<'_>,
    source: &str,
    current_file: Option<&Path>,
    own_table: &symbols::SymbolTable,
    merged: Option<&MergedView>,
    workspace: &Workspace,
    project: &VirtualProject,
) -> Option<Token> {
    let name = node_text(node, source);
    if name.is_empty() {
        return None;
    }

    let defining_path: Option<&Path> = if let Some(ms) = merged.and_then(|m| m.find(name)) {
        ms.provenance.origin()
    } else if own_table.find(name).is_some() {
        current_file
    } else {
        None
    };

    let origin = defining_path
        .map(|p| classify_origin(p, workspace, project))
        .unwrap_or(Origin::Engine);

    Some(Token {
        line: node.start_position().row as u32,
        char: node.start_position().column as u32,
        len: (node.end_byte() - node.start_byte()) as u32,
        token_type: SemanticTokenType::TYPE,
        modifiers: vec![origin_modifier(origin)],
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Engine,
    Modded,
    Unmodded,
}

fn classify_origin(file: &Path, workspace: &Workspace, project: &VirtualProject) -> Origin {
    if project.file_overrides.values().any(|p| p == file) {
        Origin::Modded
    } else if workspace.game_relative_path(file).is_some() {
        Origin::Unmodded
    } else {
        Origin::Engine
    }
}

fn origin_modifier(origin: Origin) -> SemanticTokenModifier {
    match origin {
        Origin::Engine => SemanticTokenModifier::new("engine"),
        Origin::Modded => SemanticTokenModifier::new("modded"),
        Origin::Unmodded => SemanticTokenModifier::new("unmodded"),
    }
}

fn classify_modifiers(symbol: &Symbol, origin: Origin) -> Vec<SemanticTokenModifier> {
    let mut modifiers = vec![origin_modifier(origin)];
    if symbol.kind == SymbolKind::Variable || symbol.kind == SymbolKind::Constant {
        if symbol.is_static {
            modifiers.push(SemanticTokenModifier::new("static"));
        } else if symbol.visibility == Visibility::Local {
            modifiers.push(SemanticTokenModifier::new("local"));
        }
    }
    if symbol.kind == SymbolKind::Variable && symbol.is_extern {
        modifiers.push(SemanticTokenModifier::new("extern"));
    }
    modifiers
}

/// Encode a slice of tokens into the LSP `SemanticTokens.data` format.
pub fn encode(tokens: &[Token]) -> Vec<SemanticToken> {
    let mut data = Vec::with_capacity(tokens.len());
    let mut prev_line: u32 = 0;
    let mut prev_char: u32 = 0;

    for token in tokens {
        let type_index = TOKEN_TYPES
            .iter()
            .position(|t| *t == token.token_type)
            .expect("token type in legend") as u32;
        let mut modifier_mask: u32 = 0;
        for m in &token.modifiers {
            if let Some(idx) = TOKEN_MODIFIERS.iter().position(|lm| *lm == *m) {
                modifier_mask |= 1 << idx;
            }
        }

        let delta_line = token.line - prev_line;
        let delta_start = if delta_line == 0 {
            token.char - prev_char
        } else {
            token.char
        };

        data.push(SemanticToken {
            delta_line,
            delta_start,
            length: token.len,
            token_type: type_index,
            token_modifiers_bitset: modifier_mask,
        });

        prev_line = token.line;
        prev_char = token.char;
    }

    data
}

/// Extract all identifier/type nodes from a source file, marking whether each
/// is a declaration. Used internally and exposed for unit tests.
pub fn extract_symbols(
    tree: &tree_sitter::Tree,
    source: &str,
) -> Vec<(String, symbols::SymbolKind, Range, bool)> {
    let mut out = Vec::new();
    let mut cursor = tree.root_node().walk();
    collect_symbols(tree.root_node(), source, &mut cursor, &mut out);
    out
}

fn collect_symbols(
    node: tree_sitter::Node<'_>,
    source: &str,
    cursor: &mut tree_sitter::TreeCursor<'_>,
    out: &mut Vec<(String, symbols::SymbolKind, Range, bool)>,
) {
    match node.kind() {
        "function_definition" => {
            if let Some(name_node) = node.child_by_field_name("declarator") {
                // tree-sitter-xs grammar names the function identifier field "declarator"
                // in function_definition; fall back to named child search if missing.
                if name_node.kind() == "identifier" {
                    out.push((
                        node_text(name_node, source).to_string(),
                        SymbolKind::Function,
                        node_range(name_node),
                        true,
                    ));
                }
            }
        }
        "declaration" => {
            if let Some(init) = node.child_by_field_name("declarator") {
                // For variables the declarator is an init_declarator node; the
                // actual identifier may sit deeper. We approximate with the
                // first identifier child.
                if let Some(id) = find_first_identifier(init, source) {
                    out.push((id.0, SymbolKind::Variable, id.1, true));
                }
            }
        }
        "identifier" => {
            out.push((
                node_text(node, source).to_string(),
                SymbolKind::Variable,
                node_range(node),
                false,
            ));
        }
        "_type_identifier" => {
            out.push((
                node_text(node, source).to_string(),
                SymbolKind::Variable, // placeholder kind for type usage
                node_range(node),
                false,
            ));
        }
        _ => {}
    }

    if cursor.goto_first_child() {
        loop {
            collect_symbols(cursor.node(), source, cursor, out);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn find_first_identifier(node: tree_sitter::Node<'_>, source: &str) -> Option<(String, Range)> {
    if node.kind() == "identifier" {
        return Some((node_text(node, source).to_string(), node_range(node)));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(id) = find_first_identifier(child, source) {
            return Some(id);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_symbols_flags_function_declaration() {
        let source = "void foo(int x) { int y = 1; }";
        let tree = crate::parser::parse(source).expect("parse");
        let symbols = extract_symbols(&tree, source);
        // We expect at least the function name flagged as a declaration.
        assert!(
            symbols.iter().any(|(n, k, _, is_decl)| n == "foo" && *k == SymbolKind::Function && *is_decl),
            "function declaration should be flagged"
        );
    }

    #[test]
    fn encode_empty_tokens_is_empty() {
        assert!(encode(&[]).is_empty());
    }

    #[test]
    fn encode_two_tokens_on_same_line() {
        let tokens = vec![
            Token {
                line: 0,
                char: 0,
                len: 3,
                token_type: SemanticTokenType::FUNCTION,
                modifiers: vec![SemanticTokenModifier::new("engine")],
            },
            Token {
                line: 0,
                char: 5,
                len: 2,
                token_type: SemanticTokenType::VARIABLE,
                modifiers: vec![SemanticTokenModifier::new("local")],
            },
        ];
        let data = encode(&tokens);
        assert_eq!(data.len(), 2);
        assert_eq!(data[0].delta_line, 0);
        assert_eq!(data[0].delta_start, 0);
        assert_eq!(data[0].length, 3);
        assert_eq!(data[0].token_type, 0);
        assert_eq!(data[0].token_modifiers_bitset, 1);
        assert_eq!(data[1].delta_line, 0);
        assert_eq!(data[1].delta_start, 5);
        assert_eq!(data[1].length, 2);
        assert_eq!(data[1].token_type, 1);
        assert_eq!(data[1].token_modifiers_bitset, 8);
    }

    #[test]
    fn classify_origin_detects_overlay() {
        let tmp = tempfile::tempdir().unwrap();
        let game_root = tmp.path().join("game");
        let mod_root = tmp.path().join("mod");
        let overlay = mod_root.join("game").join("ai").join("f.xs");
        std::fs::create_dir_all(overlay.parent().unwrap()).unwrap();
        std::fs::write(&overlay, "").unwrap();

        let mut ws = Workspace::new(tmp.path().to_path_buf());
        let uri = tower_lsp::lsp_types::Url::from_file_path(&mod_root).unwrap();
        ws.register_mod(uri).unwrap();
        let entry = ws.mods().first().unwrap();
        let project = ws.build_virtual_project(entry);

        assert_eq!(
            classify_origin(&overlay, &ws, &project),
            Origin::Modded
        );
        assert_eq!(
            classify_origin(&game_root.join("ai").join("g.xs"), &ws, &project),
            Origin::Unmodded
        );
        assert_eq!(
            classify_origin(Path::new("/tmp/orphan.xs"), &ws, &project),
            Origin::Engine
        );
    }
}
