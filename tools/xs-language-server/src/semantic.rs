//! Cross-file XS semantic diagnostics.
//!
//! This module models a virtual project as a set of parsed files and checks
//! engine-enforced linking rules:
//!
//!   * `extern` collisions across files
//!   * use-before-definition (unless `mutable` or forward-declared)
//!   * `mutable` redefinition signature equality
//!   * unresolved function-call symbols
//!
//! It intentionally does not do full type inference; it only resolves rules
//! that the AoM:R engine will reject at load time.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::engine_api::{EngineApi, EngineSignature};
use crate::merged_view::{MergedView, VisibilityProvenance};
use crate::parser;
use crate::symbols::{Symbol, SymbolKind, SymbolTable, Visibility};
use crate::workspace::{Workspace, VirtualProject as WorkspaceVirtualProject};

/// A single file inside the virtual project, ready for semantic analysis.
#[derive(Debug, Clone)]
pub struct ParsedFile {
    pub source: String,
    pub table: SymbolTable,
}

/// A virtual project used for semantic analysis.
///
/// Unlike `workspace::VirtualProject` (which only records overlay mappings),
/// this structure holds the parsed source + symbol table for every visible
/// file in the project. It is cheap to build for the small mod files used in
/// unit tests and is rebuilt from disk in the LSP server when a file changes.
#[derive(Debug, Clone, Default)]
pub struct VirtualProject {
    pub files: HashMap<PathBuf, ParsedFile>,
}

impl VirtualProject {
    /// Build a project directly from a map of file contents. Used by unit
    /// tests and fixtures; the LSP server builds from the workspace instead.
    pub fn from_files(files: HashMap<PathBuf, String>) -> Self {
        let mut out = HashMap::new();
        for (path, source) in files {
            if let Some(table) = parser::parse(&source)
                .map(|tree| crate::symbols::build_symbol_table(&tree, &source))
            {
                out.insert(path, ParsedFile { source, table });
            }
        }
        Self { files: out }
    }

    /// Build a semantic project from a `workspace::VirtualProject` by parsing
    /// every visible file. `cache_dir` is used to reuse cached symbol tables
    /// for game-folder files.
    ///
    /// Files that fail to load (binary `.xs` random-map data, IO error,
    /// unreadable due to permissions, etc.) are SKIPPED with a warning
    /// log rather than aborting the whole project build. The semantic
    /// analysis proceeds with the remaining files; a file that fails to
    /// parse in one pass will be retried on the next pass if its content
    /// has changed.
    pub fn load_from_workspace(
        workspace: &crate::workspace::Workspace,
        project: &crate::workspace::VirtualProject,
        cache_dir: &Path,
    ) -> Self {
        let mut files = HashMap::new();
        let visible = project.visible_files(workspace);
        tracing::debug!(
            "semantic::load_from_workspace: walking {} visible files",
            visible.len()
        );
        for (rel, path) in visible {
            // Prefer the cached symbol table when available; otherwise parse
            // directly from disk. load_or_parse_symbols already swallows
            // parse / read failures and returns an empty table for the
            // offending file, so a single bad file in the workspace can't
            // abort the build of the other 300+.
            let symbols = match crate::cache::load_or_parse_symbols(&path, &rel, cache_dir) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        "load_or_parse_symbols failed for {} ({}); skipping",
                        path.display(),
                        e
                    );
                    continue;
                }
            };
            // If the source can't be read as UTF-8 (a random-map binary
            // .xs that slipped through), skip the file with a warning.
            let source = match std::fs::read_to_string(&path) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(
                        "could not read source for {} ({}); excluding from semantic project",
                        path.display(),
                        e
                    );
                    continue;
                }
            };
            files.insert(path, ParsedFile { source, table: symbols });
        }
        Self { files }
    }
}

/// Builtin XS type constructors / language keywords that the semantic layer
/// treats as resolvable callables even though they are not workspace symbols
/// and are not listed in the engine-API Doxygen extraction.
const BUILTIN_CALLEES: &[&str] = &[
    "vector",
    "xsVectorSet",
    "xsVectorGetX",
    "xsVectorGetY",
    "xsVectorGetZ",
];

/// Result of resolving a callee name in the current scope.
pub enum Resolution<'a> {
    /// A workspace-defined function or rule.
    Workspace(&'a Symbol),
    /// An engine-API syscall or builtin callable.
    Engine(&'a EngineSignature),
    /// Not resolved anywhere.
    Unresolved,
}

impl std::fmt::Debug for Resolution<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Resolution::Workspace(s) => write!(f, "Workspace({})", s.name),
            Resolution::Engine(s) => write!(f, "Engine({})", s.name),
            Resolution::Unresolved => write!(f, "Unresolved"),
        }
    }
}

fn is_callable_symbol(s: &Symbol) -> bool {
    s.kind == SymbolKind::Function
}

/// Resolve a callee name against the workspace, the engine API, and builtin
/// constructs. Workspace definitions take precedence over engine symbols.
pub fn resolve_callee<'a>(
    project: &'a VirtualProject,
    engine: &'a EngineApi,
    merged: Option<&'a MergedView>,
    name: &str,
    call_line: u32,
) -> Resolution<'a> {
    if let Some(m) = merged {
        if let Some(ms) = m.find(name) {
            if is_callable_symbol(&ms.symbol)
                && (ms.symbol.is_mutable
                    || ms.provenance.include_line() < call_line
                    || (matches!(ms.provenance, VisibilityProvenance::OwnFile)
                        && ms.symbol.selection_range.start.line < call_line))
            {
                return Resolution::Workspace(&ms.symbol);
            }
        }
    }

    if let Some(file) = project.files.values().find(|f| f.table.find(name).is_some()) {
        if let Some(sym) = file.table.find(name) {
            if is_callable_symbol(sym) {
                return Resolution::Workspace(sym);
            }
        }
    }

    if let Some(syscall) = engine.lookup(name) {
        return Resolution::Engine(syscall);
    }

    Resolution::Unresolved
}

/// Convenience wrapper around a semantic project.
pub struct SemanticChecker {
    pub project: VirtualProject,
}

impl SemanticChecker {
    pub fn new(project: VirtualProject) -> Self {
        Self { project }
    }

    /// Run all semantic checks using an empty engine API. Tests that only
    /// exercise workspace symbols can use this convenience wrapper.
    pub fn check_all(&self, current_file: &Path) -> Vec<Diagnostic> {
        check_all(&self.project, &EngineApi::default(), current_file)
    }
}

/// Run all semantic checks and return the combined diagnostics.
pub fn check_all(
    project: &VirtualProject,
    engine: &EngineApi,
    current_file: &Path,
) -> Vec<Diagnostic> {
    let merged = build_merged_view_from_project(project, current_file);
    let mut out = Vec::new();
    out.extend(check_extern_collisions(project));
    if let Some(mv) = merged.as_ref() {
        out.extend(check_forward_declarations_for_merged_view(
            project, engine, current_file, mv,
        ));
        out.extend(check_mutable_redefinitions_for_merged_view(mv));
    } else {
        // Fallback for tests/fixtures that don't sit under a `game/` root.
        out.extend(check_forward_declarations(project, engine, current_file));
        out.extend(check_mutable_redefinitions(project));
    }
    out
}

// --- merged-view helpers ---------------------------------------------------

/// Try to infer the game-install root for an absolute file path.
///
/// Walks up ancestors looking for a directory that contains a `game/`
/// subdirectory and where `current_file` lies inside that `game/` tree.
fn infer_game_root(current_file: &Path) -> Option<PathBuf> {
    let mut dir = current_file.parent()?;
    loop {
        let game_dir = dir.join("game");
        if game_dir.is_dir() && current_file.starts_with(&game_dir) {
            return Some(dir.to_path_buf());
        }
        dir = dir.parent()?;
    }
}

/// Build a `MergedView` for `current_file` from an in-memory semantic project.
///
/// This lets the integration test harness (`tests/game_folder_parse.rs`)
/// benefit from true include-paste semantics without changing its public API.
fn build_merged_view_from_project(
    project: &VirtualProject,
    current_file: &Path,
) -> Option<MergedView> {
    let game_root = infer_game_root(current_file)?;
    let file = project.files.get(current_file)?;
    let source = &file.source;
    let own_table = &file.table;
    let ws = Workspace::new(game_root);
    let project = WorkspaceVirtualProject::default();
    let cache_dir = crate::cache::state_cache_dir();
    Some(MergedView::build(current_file, source, own_table, &ws, &project, &cache_dir))
}

/// Effective line for ordering symbols in the merged translation unit.
///
/// Own-file symbols keep their source position; included symbols are treated
/// as pasted at the line of the `include` directive that introduced them.
fn effective_line(ms: &crate::merged_view::MergedSymbol) -> u32 {
    match ms.provenance {
        VisibilityProvenance::OwnFile => ms.symbol.selection_range.start.line,
        _ => ms.provenance.include_line(),
    }
}

/// Detect `extern` collisions: if any file declares `X` as `extern`, no other
/// file in the same project may declare or define `X` in any form.
pub fn check_extern_collisions(project: &VirtualProject) -> Vec<Diagnostic> {
    let mut by_name: HashMap<String, Vec<(PathBuf, &Symbol)>> = HashMap::new();
    for (path, file) in &project.files {
        for sym in &file.table.symbols {
            by_name
                .entry(sym.name.clone())
                .or_default()
                .push((path.clone(), sym));
        }
    }

    let mut diags = Vec::new();
    for (name, occurrences) in &by_name {
        // Files that have at least one `extern` occurrence of this name.
        let extern_files: HashSet<&Path> = occurrences
            .iter()
            .filter(|(_, s)| s.visibility == Visibility::Extern)
            .map(|(p, _)| p.as_path())
            .collect();

        if extern_files.is_empty() {
            continue;
        }

        // Every occurrence in a different file from the extern declaration is
        // a collision. Duplicate `extern` declarations in different files also
        // collide.
        for (path, sym) in occurrences {
            if !extern_files.contains(path.as_path()) {
                let extern_file = extern_files.iter().next().unwrap();
                diags.push(Diagnostic {
                    range: sym.selection_range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(tower_lsp::lsp_types::NumberOrString::String(
                        "E0310".to_string(),
                    )),
                    code_description: None,
                    source: Some("xs-language-server".to_string()),
                    message: format!(
                        "extern collision: '{}' is declared extern in {}, also defined in {}",
                        name,
                        extern_file.display(),
                        path.display()
                    ),
                    related_information: None,
                    tags: None,
                    data: None,
                });
            } else if extern_files.len() > 1 {
                // More than one file has `extern X`.
                let other_file = extern_files
                    .iter()
                    .find(|&&p| p != path.as_path())
                    .unwrap();
                diags.push(Diagnostic {
                    range: sym.selection_range,
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(tower_lsp::lsp_types::NumberOrString::String(
                        "E0310".to_string(),
                    )),
                    code_description: None,
                    source: Some("xs-language-server".to_string()),
                    message: format!(
                        "duplicate extern: '{}' is declared extern in {} and {}",
                        name,
                        other_file.display(),
                        path.display()
                    ),
                    related_information: None,
                    tags: None,
                    data: None,
                });
            }
        }
    }

    diags
}

/// Validate that every function call in `current_file` is preceded by a
/// definition, forward declaration, or `mutable` declaration.
pub fn check_forward_declarations(
    project: &VirtualProject,
    engine: &EngineApi,
    current_file: &Path,
) -> Vec<Diagnostic> {
    let Some(file) = project.files.get(current_file) else {
        return Vec::new();
    };
    let Some(tree) = parser::parse(&file.source) else {
        return Vec::new();
    };

    // Ranges of function definitions in this file, used to detect a call that
    // sits inside its own definition (self-recursion is not a forward-decl
    // issue).
    let own_function_ranges: Vec<(String, Range)> = file
        .table
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function && !s.is_forward)
        .map(|s| (s.name.clone(), s.full_range))
        .collect();

    let calls = collect_calls(&tree, &file.source);
    let mut diags = Vec::new();

    for (callee, callee_range) in calls {
        // Skip self-calls: a function calling itself is not a use-before-def.
        if own_function_ranges
            .iter()
            .any(|(n, r)| n == &callee && contains_range(*r, callee_range) && r.start.line < callee_range.start.line)
        {
            continue;
        }

        if forward_callable(project, current_file, &callee, callee_range.start.line) {
            continue;
        }

        // Engine-API syscalls and builtin XS constructs are always callable.
        if engine.lookup(&callee).is_some() || BUILTIN_CALLEES.contains(&callee.as_str()) {
            continue;
        }

        // If the symbol is not defined anywhere in the project, flag it as
        // an unresolved symbol (engine Error 0310).
        let resolved_anywhere = project
            .files
            .values()
            .any(|f| f.table.find(&callee).is_some());

        let message = if resolved_anywhere {
            format!(
                "'{}' used at line {} before declaration; add forward declaration or mark 'mutable'",
                callee,
                callee_range.start.line + 1
            )
        } else {
            format!(
                "Error 0310: invalid symbol lookup '{}' at line {}",
                callee,
                callee_range.start.line + 1
            )
        };

        diags.push(Diagnostic {
            range: callee_range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(tower_lsp::lsp_types::NumberOrString::String("E0310".to_string())),
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message,
            related_information: None,
            tags: None,
            data: None,
        });
    }

    diags
}

/// Check that each `mutable` function is redefined with the exact same
/// signature (name + parameter types + default values).
pub fn check_mutable_redefinitions(project: &VirtualProject) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    for (_path, file) in &project.files {
        let funcs: Vec<&Symbol> = file
            .table
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .collect();

        let mut by_name: HashMap<String, Vec<&Symbol>> = HashMap::new();
        for sym in funcs {
            by_name.entry(sym.name.clone()).or_default().push(sym);
        }

        for (name, defs) in by_name {
            let ordered = order_by_line(defs);
            for (i, sym) in ordered.iter().enumerate() {
                if !sym.is_mutable {
                    continue;
                }
                for later in &ordered[i + 1..] {
                    if !same_signature(sym, later) {
                        diags.push(Diagnostic {
                            range: later.selection_range,
                            severity: Some(DiagnosticSeverity::ERROR),
                            code: Some(tower_lsp::lsp_types::NumberOrString::String(
                                "E0310".to_string(),
                            )),
                            code_description: None,
                            source: Some("xs-language-server".to_string()),
                            message: format!(
                                "mutable function '{}' redefined with different signature at line {}",
                                name,
                                later.selection_range.start.line + 1
                            ),
                            related_information: None,
                            tags: None,
                            data: None,
                        });
                    }
                }
            }
        }
    }

    diags
}

/// Validate forward declarations using the merged include-paste scope.
///
/// A call resolves if the callee is defined or forward-declared earlier in
/// the same file, is declared `mutable`, or is pasted from an `include`
/// directive whose line precedes the call.
pub fn check_forward_declarations_for_merged_view(
    project: &VirtualProject,
    engine: &EngineApi,
    current_file: &Path,
    merged: &MergedView,
) -> Vec<Diagnostic> {
    let Some(file) = project.files.get(current_file) else {
        return Vec::new();
    };
    let Some(tree) = parser::parse(&file.source) else {
        return Vec::new();
    };

    // Ranges of own-file function definitions, used to detect a call that
    // sits inside its own definition (self-recursion is allowed).
    let own_function_ranges: Vec<(String, Range)> = merged
        .own_table()
        .symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function && !s.is_forward)
        .map(|s| (s.name.clone(), s.full_range))
        .collect();

    let calls = collect_calls(&tree, &file.source);
    let mut diags = Vec::new();

    for (callee, callee_range) in calls {
        // Skip self-calls.
        if own_function_ranges
            .iter()
            .any(|(n, r)| n == &callee && contains_range(*r, callee_range) && r.start.line < callee_range.start.line)
        {
            continue;
        }

        if forward_callable_merged(merged, &callee, callee_range.start.line) {
            continue;
        }

        // Engine-API syscalls and builtin XS constructs are always callable.
        if engine.lookup(&callee).is_some() || BUILTIN_CALLEES.contains(&callee.as_str()) {
            continue;
        }

        // If the symbol is present anywhere in the merged scope, the call
        // is a use-before-definition; otherwise it's a true unknown symbol.
        let resolved_anywhere = merged.find(&callee).is_some();

        let message = if resolved_anywhere {
            format!(
                "'{}' used at line {} before declaration; add forward declaration or mark 'mutable'",
                callee,
                callee_range.start.line + 1
            )
        } else {
            format!(
                "Error 0310: invalid symbol lookup '{}' at line {}",
                callee,
                callee_range.start.line + 1
            )
        };

        diags.push(Diagnostic {
            range: callee_range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(tower_lsp::lsp_types::NumberOrString::String("E0310".to_string())),
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message,
            related_information: None,
            tags: None,
            data: None,
        });
    }

    diags
}

/// Check `mutable` redefinitions across the merged include-paste scope.
///
/// Included files behave like textual paste, so the effective translation
/// unit for redefinition is the current file plus its resolved includes.
pub fn check_mutable_redefinitions_for_merged_view(merged: &MergedView) -> Vec<Diagnostic> {
    let mut diags = Vec::new();

    let mut by_name: HashMap<String, Vec<&crate::merged_view::MergedSymbol>> = HashMap::new();
    for ms in merged
        .symbols()
        .iter()
        .filter(|ms| ms.symbol.kind == SymbolKind::Function)
    {
        by_name.entry(ms.symbol.name.clone()).or_default().push(ms);
    }

    for (name, syms) in by_name {
        let mut ordered = syms;
        ordered.sort_by_key(|ms| effective_line(ms));
        for (i, ms) in ordered.iter().enumerate() {
            if !ms.symbol.is_mutable {
                continue;
            }
            for later in &ordered[i + 1..] {
                if !same_signature(&ms.symbol, &later.symbol) {
                    diags.push(Diagnostic {
                        range: later.symbol.selection_range,
                        severity: Some(DiagnosticSeverity::ERROR),
                        code: Some(tower_lsp::lsp_types::NumberOrString::String(
                            "E0310".to_string(),
                        )),
                        code_description: None,
                        source: Some("xs-language-server".to_string()),
                        message: format!(
                            "mutable function '{}' redefined with different signature at line {}",
                            name,
                            later.symbol.selection_range.start.line + 1
                        ),
                        related_information: None,
                        tags: None,
                        data: None,
                    });
                }
            }
        }
    }

    diags
}

fn forward_callable_merged(merged: &MergedView, callee: &str, call_line: u32) -> bool {
    for ms in merged
        .symbols()
        .iter()
        .filter(|ms| ms.symbol.kind == SymbolKind::Function && ms.symbol.name == callee)
    {
        if ms.symbol.is_mutable {
            return true;
        }
        let def_line = match ms.provenance {
            VisibilityProvenance::OwnFile => ms.symbol.selection_range.start.line,
            _ => ms.provenance.include_line(),
        };
        if def_line < call_line {
            return true;
        }
    }
    false
}

// --- internal helpers ---

fn contains_range(outer: Range, inner: Range) -> bool {
    (outer.start.line < inner.start.line
        || (outer.start.line == inner.start.line && outer.start.character <= inner.start.character))
        && (outer.end.line > inner.end.line
            || (outer.end.line == inner.end.line && outer.end.character >= inner.end.character))
}

fn order_by_line(syms: Vec<&Symbol>) -> Vec<&Symbol> {
    let mut sorted = syms;
    sorted.sort_by_key(|s| s.selection_range.start.line);
    sorted
}

fn same_signature(a: &Symbol, b: &Symbol) -> bool {
    if a.name != b.name || a.ty != b.ty {
        return false;
    }
    if a.params.len() != b.params.len() {
        return false;
    }
    a.params
        .iter()
        .zip(b.params.iter())
        .all(|(pa, pb)| pa.ty == pb.ty && pa.default == pb.default)
}

/// Is `callee` callable from `current_file` at the given line?
///
/// A function is callable if any of the following is true:
///   * it is defined earlier in the same file,
///   * it has been forward-declared earlier in the same file,
///   * it is declared `mutable` anywhere in the same file,
///   * it is defined (not just declared) in any other file of the project.
fn forward_callable(
    project: &VirtualProject,
    current_file: &Path,
    callee: &str,
    call_line: u32,
) -> bool {
    // Same-file rules: definition/forward-decl before the call, or mutable.
    if let Some(file) = project.files.get(current_file) {
        let mut seen_def_before = false;
        let mut seen_mutable = false;
        let mut seen_forward_before = false;

        for sym in &file.table.symbols {
            if sym.kind != SymbolKind::Function || sym.name != callee {
                continue;
            }
            let def_line = sym.selection_range.start.line;
            if def_line < call_line {
                if sym.is_forward {
                    seen_forward_before = true;
                } else {
                    seen_def_before = true;
                }
            }
            if sym.is_mutable {
                seen_mutable = true;
            }
        }

        if seen_mutable || seen_def_before || seen_forward_before {
            return true;
        }
    }

    // Defined in another file? Functions are visible across files in the same
    // virtual project regardless of `extern`.
    project.files.iter().any(|(path, file)| {
        path != current_file
            && file.table.symbols.iter().any(|s| {
                s.kind == SymbolKind::Function && s.name == callee && !s.is_forward
            })
    })
}

/// Collect bare identifier call expressions from a tree.
fn collect_calls(tree: &tree_sitter::Tree, source: &str) -> Vec<(String, Range)> {
    let mut out = Vec::new();
    walk_calls(tree.root_node(), source, &mut out);
    out
}

fn walk_calls(node: tree_sitter::Node<'_>, source: &str, out: &mut Vec<(String, Range)>) {
    if node.kind() == "call_expression" {
        if let Some(name) = call_callee_name(node, source) {
            out.push((name, node_range(node)));
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        walk_calls(child, source, out);
    }
}

fn call_callee_name(call_node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    // We only check bare identifier calls; method calls (`obj.method`) are
    // not forward-declaration checked by the engine.
    let func_node = find_named_child(call_node, "identifier")?;
    Some(node_text(func_node, source).to_string())
}

fn node_range(node: tree_sitter::Node<'_>) -> Range {
    let start = node.start_position();
    let end = node.end_position();
    Range::new(
        Position::new(start.row as u32, start.column as u32),
        Position::new(end.row as u32, end.column as u32),
    )
}

fn node_text<'a>(node: tree_sitter::Node<'a>, source: &'a str) -> &'a str {
    &source[node.byte_range()]
}

fn find_named_child<'a>(
    node: tree_sitter::Node<'a>,
    kind: &str,
) -> Option<tree_sitter::Node<'a>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).find(|c| c.kind() == kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::{Path, PathBuf};
    use std::sync::OnceLock;

    use tempfile::TempDir;

    use crate::engine_api::EngineApi;
    use crate::merged_view::MergedView;
    use crate::workspace::{VirtualProject as WorkspaceVirtualProject, Workspace};

    fn p(name: &str) -> PathBuf {
        PathBuf::from(name)
    }

    fn project(files: &[(&str, &str)]) -> VirtualProject {
        let map: HashMap<PathBuf, String> = files
            .iter()
            .map(|(path, src)| (p(path), src.to_string()))
            .collect();
        VirtualProject::from_files(map)
    }

    fn engine_api() -> &'static EngineApi {
        use crate::cache;
        static ENGINE: OnceLock<EngineApi> = OnceLock::new();
        ENGINE.get_or_init(|| {
            let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let workspace_root = manifest_dir
                .parent()
                .and_then(|p| p.parent())
                .expect("manifest inside workspace");
            let archive = workspace_root.join("docs/doxygen_retail.7z");
            let cache_dir = cache::state_cache_dir();
            EngineApi::load_from_archive(&archive, &cache_dir)
                .expect("load engine API from Doxygen archive")
        })
    }

    #[test]
    fn test_resolve_callee_finds_engine_api() {
        let prj = project(&[("a.xs", "void foo() {}\n")]);
        let res = resolve_callee(&prj, engine_api(), None, "kbUnitGetPosition", 10);
        assert!(
            matches!(res, Resolution::Engine(_)),
            "expected engine API resolution, got {res:?}"
        );
    }

    #[test]
    fn test_resolve_callee_prefers_workspace_over_engine_api() {
        let prj = project(&[("a.xs", "void kbUnitGetPosition() {}\n")]);
        let res = resolve_callee(&prj, engine_api(), None, "kbUnitGetPosition", 10);
        assert!(
            matches!(res, Resolution::Workspace(_)),
            "expected workspace shadow of engine API, got {res:?}"
        );
    }

    #[test]
    fn test_resolve_callee_returns_none_for_truly_unknown() {
        let prj = project(&[("a.xs", "void foo() {}\n")]);
        let res = resolve_callee(&prj, engine_api(), None, "foobarBaz", 10);
        assert!(
            matches!(res, Resolution::Unresolved),
            "expected unresolved for unknown callee, got {res:?}"
        );
    }

    /// Write fixtures under a temporary `game/` root, build a semantic
    /// `VirtualProject` for the files, and a `MergedView` for `current_rel`.
    ///
    /// The `TempDir` is returned so callers can keep the fixture files alive
    /// on disk for code paths that re-infer the game root.
    fn merged_fixture(
        files: &[(&str, &str)],
        current_rel: &str,
    ) -> (TempDir, VirtualProject, MergedView, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let mut map = HashMap::new();
        for (rel, src) in files {
            let path = root.join("game").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let mut f = std::fs::File::create(&path).unwrap();
            f.write_all(src.as_bytes()).unwrap();
            map.insert(path.clone(), src.to_string());
        }
        let prj = VirtualProject::from_files(map);
        let current = root.join("game").join(current_rel);
        let source = prj.files.get(&current).unwrap().source.clone();
        let own = prj.files.get(&current).unwrap().table.clone();
        let ws = Workspace::new(root.to_path_buf());
        let project = WorkspaceVirtualProject::default();
        let cache_dir = TempDir::new().unwrap();
        let merged = MergedView::build(&current, &source, &own, &ws, &project, cache_dir.path());
        (tmp, prj, merged, current)
    }

    #[test]
    fn extern_collision_between_files() {
        let prj = project(&[
            ("a.xs", "extern int gFoo = 5;\n"),
            ("b.xs", "int gFoo = 5;\n"),
        ]);
        let diags = check_extern_collisions(&prj);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("extern collision"));
        assert!(diags[0].message.contains("gFoo"));
    }

    #[test]
    fn duplicate_extern_between_files_is_collision() {
        let prj = project(&[
            ("a.xs", "extern int gFoo = 5;\n"),
            ("b.xs", "extern int gFoo = 5;\n"),
        ]);
        let diags = check_extern_collisions(&prj);
        assert_eq!(diags.len(), 2, "both extern declarations should be flagged");
    }

    #[test]
    fn file_local_same_name_is_ok() {
        let prj = project(&[
            ("a.xs", "int localOnly = 1;\n"),
            ("b.xs", "int localOnly = 2;\n"),
        ]);
        let diags = check_extern_collisions(&prj);
        assert!(diags.is_empty());
    }

    #[test]
    fn forward_declaration_allows_call() {
        let prj = project(&[("a.xs", "void bar();\nvoid foo() { bar(); }\nvoid bar() {}\n")]);
        let diags = check_forward_declarations(&prj, &EngineApi::default(), &p("a.xs"));
        assert!(diags.is_empty(), "expected no forward-decl errors, got {diags:?}");
    }

    #[test]
    fn missing_forward_declaration_is_error() {
        let prj = project(&[("a.xs", "void foo() { bar(); }\nvoid bar() {}\n")]);
        let diags = check_forward_declarations(&prj, &EngineApi::default(), &p("a.xs"));
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("before declaration"));
        assert!(diags[0].message.contains("bar"));
    }

    #[test]
    fn mutable_function_is_forward_callable() {
        let prj = project(&[("a.xs", "mutable void foo() {}\nvoid bar() { foo(); }\nvoid foo() {}\n")]);
        let diags = check_forward_declarations(&prj, &EngineApi::default(), &p("a.xs"));
        assert!(diags.is_empty(), "mutable should be forward-callable, got {diags:?}");
    }

    #[test]
    fn mutable_redefinition_same_signature_is_ok() {
        let prj = project(&[("a.xs", "mutable void foo(int x = 1) {}\nvoid foo(int x = 1) {}\n")]);
        let diags = check_mutable_redefinitions(&prj);
        assert!(diags.is_empty(), "expected ok, got {diags:?}");
    }

    #[test]
    fn mutable_redefinition_different_signature_is_error() {
        let prj = project(&[("a.xs", "mutable void foo(int x) {}\nvoid foo(float y) {}\n")]);
        let diags = check_mutable_redefinitions(&prj);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("different signature"));
        assert!(diags[0].message.contains("foo"));
    }

    #[test]
    fn cross_file_function_is_callable() {
        let prj = project(&[
            ("a.xs", "void foo() { bar(); }\n"),
            ("b.xs", "void bar() {}\n"),
        ]);
        let diags = check_forward_declarations(&prj, &EngineApi::default(), &p("a.xs"));
        assert!(diags.is_empty(), "cross-file function should be visible, got {diags:?}");
    }

    #[test]
    fn unresolved_symbol_emits_error_0310() {
        let prj = project(&[("a.xs", "void foo() { doesNotExist(); }\n")]);
        let diags = check_forward_declarations(&prj, &EngineApi::default(), &p("a.xs"));
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Error 0310"));
        assert!(diags[0].message.contains("doesNotExist"));
    }

    #[test]
    fn call_before_include_is_error() {
        // spec-semantic-diagnostics.md: `bar()` before `include "b.xs"` must
        // error even though b.xs defines bar.
        let (_tmp, prj, merged, current) = merged_fixture(
            &[
                ("ai/a.xs", "void foo() { bar(); }\ninclude \"b.xs\";\n"),
                ("ai/b.xs", "void bar() {}\n"),
            ],
            "ai/a.xs",
        );
        let diags = check_forward_declarations_for_merged_view(&prj, &EngineApi::default(), &current, &merged);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("before declaration"));
        assert!(diags[0].message.contains("bar"));
    }

    #[test]
    fn call_after_include_is_clean() {
        // Scenario 11: included public function is visible after the include.
        let (_tmp, prj, merged, current) = merged_fixture(
            &[
                ("ai/a.xs", "include \"b.xs\";\nvoid foo() { bar(); }\n"),
                ("ai/b.xs", "void bar() {}\n"),
            ],
            "ai/a.xs",
        );
        let diags = check_forward_declarations_for_merged_view(&prj, &EngineApi::default(), &current, &merged);
        assert!(diags.is_empty(), "expected clean diagnostics, got {diags:?}");
    }

    #[test]
    fn included_static_function_is_unresolved() {
        // Scenario 9 variant: `static` makes a function local to its file.
        let (_tmp, prj, merged, current) = merged_fixture(
            &[
                ("ai/a.xs", "include \"b.xs\";\nvoid foo() { hidden(); }\n"),
                ("ai/b.xs", "static void hidden() {}\n"),
            ],
            "ai/a.xs",
        );
        let diags = check_forward_declarations_for_merged_view(&prj, &EngineApi::default(), &current, &merged);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Error 0310"));
        assert!(diags[0].message.contains("hidden"));
    }

    #[test]
    fn mutable_function_in_included_file_is_callable() {
        // Scenario 12: mutable helper from an include resolves anywhere.
        let (_tmp, prj, merged, current) = merged_fixture(
            &[
                (
                    "ai/a.xs",
                    "include \"b.xs\";\nvoid foo() { helper(5); }\n",
                ),
                ("ai/b.xs", "mutable void helper(int x = -1) {}\n"),
            ],
            "ai/a.xs",
        );
        let diags = check_forward_declarations_for_merged_view(&prj, &EngineApi::default(), &current, &merged);
        assert!(
            diags.is_empty(),
            "mutable included function should resolve, got {diags:?}"
        );
    }

    #[test]
    fn mutable_redefinition_across_include_is_checked() {
        // The mutable definition (own file) precedes the included
        // redefinition with a different default.
        let (_tmp, _prj, merged, _current) = merged_fixture(
            &[
                (
                    "ai/a.xs",
                    "mutable void helper(int x = 1) {}\ninclude \"b.xs\";\n",
                ),
                ("ai/b.xs", "void helper(int x = 2) {}\n"),
            ],
            "ai/a.xs",
        );
        let diags = check_mutable_redefinitions_for_merged_view(&merged);
        assert!(diags.iter().any(|d| d.message.contains("different signature")));
    }

    // -----------------------------------------------------------------------
    // Include-paste semantic fixtures (T11)
    // -----------------------------------------------------------------------

    #[test]
    fn direct_include_resolves_symbol() {
        // Scenario 4: a file that includes include_helper.xs can call helper().
        let (_tmp, prj, merged, current) = merged_fixture(
            &[
                (
                    "ai/include_forward_decl_ok.xs",
                    include_str!("semantic_fixtures/include_forward_decl_ok.xs"),
                ),
                (
                    "ai/include_helper.xs",
                    include_str!("semantic_fixtures/include_helper.xs"),
                ),
            ],
            "ai/include_forward_decl_ok.xs",
        );
        assert!(
            merged.find("helper").is_some(),
            "helper should be visible in the merged view"
        );
        let diags = check_forward_declarations_for_merged_view(&prj, &EngineApi::default(), &current, &merged);
        assert!(diags.is_empty(), "direct include should resolve, got {diags:?}");
    }

    #[test]
    fn transitive_include_resolves_symbol() {
        // Scenario 5: a includes forward_decl_ok (which includes helper);
        // helper is visible transitively.
        let (_tmp, prj, merged, current) = merged_fixture(
            &[
                (
                    "ai/transitive.xs",
                    "include \"include_forward_decl_ok.xs\";\nvoid caller() { helper(); }\n",
                ),
                (
                    "ai/include_forward_decl_ok.xs",
                    include_str!("semantic_fixtures/include_forward_decl_ok.xs"),
                ),
                (
                    "ai/include_helper.xs",
                    include_str!("semantic_fixtures/include_helper.xs"),
                ),
            ],
            "ai/transitive.xs",
        );
        let ms = merged
            .find("helper")
            .expect("helper should be visible transitively");
        assert!(
            matches!(
                ms.provenance,
                VisibilityProvenance::TransitiveInclude { depth: 2, .. }
            ),
            "helper should be a depth-2 transitive include, got {:?}",
            ms.provenance
        );
        let diags = check_forward_declarations_for_merged_view(&prj, &EngineApi::default(), &current, &merged);
        assert!(
            diags.is_empty(),
            "transitive include should resolve, got {diags:?}"
        );
    }

    #[test]
    fn cycle_includes_do_not_loop() {
        // Scenario 6: a <-> b include cycle terminates cleanly.
        let (_tmp, _prj, merged, _current) = merged_fixture(
            &[
                (
                    "ai/include_cycle_a.xs",
                    include_str!("semantic_fixtures/include_cycle_a.xs"),
                ),
                (
                    "ai/include_cycle_b.xs",
                    include_str!("semantic_fixtures/include_cycle_b.xs"),
                ),
            ],
            "ai/include_cycle_a.xs",
        );
        assert!(merged.graph().is_cyclic(), "cycle should be detected");
        assert!(merged.find("aFn").is_some(), "own symbol should be present");
        assert!(merged.find("bFn").is_some(), "included symbol should be present");
        // Symbols from the cycle should appear exactly once due to the visited set.
        let a_count = merged
            .symbols()
            .iter()
            .filter(|ms| ms.symbol.name == "aFn")
            .count();
        let b_count = merged
            .symbols()
            .iter()
            .filter(|ms| ms.symbol.name == "bFn")
            .count();
        assert_eq!(a_count, 1);
        assert_eq!(b_count, 1);
    }

    #[test]
    fn missing_include_target_produces_diagnostic() {
        // Scenario 7: include_missing.xs includes nonexistent.xs.
        let (_tmp, _prj, merged, _current) = merged_fixture(
            &[
                (
                    "ai/include_missing.xs",
                    include_str!("semantic_fixtures/include_missing.xs"),
                ),
                (
                    "ai/include_helper.xs",
                    include_str!("semantic_fixtures/include_helper.xs"),
                ),
            ],
            "ai/include_missing.xs",
        );
        let missing_targets: Vec<_> = merged
            .missing_includes()
            .iter()
            .map(|d| d.target.clone())
            .collect();
        assert!(
            missing_targets.contains(&"nonexistent.xs".to_string()),
            "missing target diagnostic should mention nonexistent.xs, got {missing_targets:?}"
        );
        // The other include should still resolve.
        assert!(merged.find("helper").is_some());
    }

    #[test]
    fn static_symbol_in_included_file_is_hidden() {
        // Scenario 9 variant: static gHidden from the included file must not
        // be visible in the includer's merged scope.
        let (_tmp, _prj, merged, _current) = merged_fixture(
            &[
                (
                    "ai/include_static_hidden.xs",
                    include_str!("semantic_fixtures/include_static_hidden.xs"),
                ),
                (
                    "ai/include_static_helper.xs",
                    include_str!("semantic_fixtures/include_static_helper.xs"),
                ),
            ],
            "ai/include_static_hidden.xs",
        );
        assert!(
            merged.find("gHidden").is_none(),
            "static variable from included file should be hidden"
        );
    }

    /// Regression guard for the user's complaint:
    ///   "The LSP needs to be more robust, it can't give up on every
    ///    single bad file."
    ///
    /// `load_from_workspace` MUST NOT panic or abort when one of the
    /// visible files is unreadable (random-map binary `.xs`). It must
    /// silently skip the bad file with a `tracing::warn!` and return a
    /// `VirtualProject` that still contains the good files' symbols.
    #[test]
    fn load_from_workspace_skips_bad_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join("game").join("ai")).unwrap();

        let good_path = root.join("game").join("ai").join("human_assist.xs");
        std::fs::write(&good_path, "void liveHelper() {}\n").unwrap();

        // Mimic the AoM:R random-map binary `.xs` layout: a `.xs` file that
        // exists but cannot be decoded as UTF-8. The walker skips it via
        // `is_readable_xs_file`, but this test exercises the
        // `load_from_workspace` fallback for paths that slip through.
        let bad_path = root.join("game").join("random_maps").join("aso_grasslands.xs");
        std::fs::create_dir_all(bad_path.parent().unwrap()).unwrap();
        std::fs::write(&bad_path, [0xffu8, 0xfe, 0x00, 0xab, 0xcd]).unwrap();

        let ws = Workspace::new(root.to_path_buf());
        let project = WorkspaceVirtualProject::default();
        let cache_dir = TempDir::new().unwrap();

        let semantic_project = VirtualProject::load_from_workspace(&ws, &project, cache_dir.path());

        // The good file must be in the project.
        assert!(
            semantic_project.files.contains_key(&good_path),
            "good file must be in the semantic project; got: {:?}",
            semantic_project.files.keys().collect::<Vec<_>>()
        );
        // The good file's symbol must be parseable.
        let good_table = semantic_project
            .files
            .get(&good_path)
            .expect("good file present")
            .table
            .clone();
        assert!(
            good_table.find("liveHelper").is_some(),
            "good file's symbols must be present in the project"
        );

        // The bad file must NOT be in the project (it was either skipped
        // by the walker before reaching this layer, or skipped inside
        // load_or_parse_symbols).
        assert!(
            !semantic_project.files.contains_key(&bad_path),
            "unreadable .xs file must be skipped by load_from_workspace"
        );
    }
}
