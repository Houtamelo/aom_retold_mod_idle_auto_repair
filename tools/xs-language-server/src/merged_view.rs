//! Textual-paste include resolution.
//!
//! For an analysed file, `MergedView` collects the file's own symbols plus
//! the visibility-filtered symbols from every directly and transitively
//! included file. Included `static`/file-local variables are hidden;
//! `extern` variables and public functions are visible. Cycles terminate
//! cleanly and missing or unreadable targets become diagnostics rather than
//! fatal errors.
//!
//! Include-line propagation: each included symbol carries an `effective_line`
//! that is the line of the earliest `include` directive in the analysed file
//! that reaches the symbol. The builder walks includes depth-first in source
//! order; the first reaching root edge sets `effective_line`, and subsequent
//! edges to an already-visited file are skipped. Cycles are therefore bounded
//! by the `visited` guard, and a symbol's effective line is stable for the
//! lifetime of the `MergedView`.
//!
//! Resilience: a single bad include target (binary `.xs` random-map data,
//! permission error, etc.) produces a `tracing::warn!` and is omitted from
//! the merged view, but the merged view for the rest of the file STILL
//! builds. The user-visible diagnostic is preserved on
//! [`MergedView::missing_includes`] and [`MergedView::unreadable_includes`].

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Range;

use crate::symbols::{Symbol, SymbolKind, SymbolTable, Visibility};
use crate::workspace::{IncludeEdge, IncludeRoot, VirtualProject, Workspace};
use crate::{cache, parser};

/// Where a symbol in a merged view came from and how it entered the scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisibilityProvenance {
    /// Defined in the file being analysed.
    OwnFile,
    /// Pasted from a file directly included by the analysed file.
    DirectInclude {
        /// Absolute path of the file that defines the symbol.
        origin: PathBuf,
        /// Absolute path of the file whose `include` directive introduced
        /// this symbol into the current scope.
        introduced_at: PathBuf,
        /// 0-indexed line of that `include` directive in `introduced_at`.
        include_line: u32,
        /// 0-indexed line of the earliest `include` directive in the
        /// analysed file that reaches this symbol.
        effective_line: u32,
    },
    /// Pasted from a file included transitively through one or more includes.
    TransitiveInclude {
        /// Absolute path of the file that defines the symbol.
        origin: PathBuf,
        /// Absolute path of the file whose `include` directive introduced
        /// this symbol into the current scope.
        introduced_at: PathBuf,
        /// 0-indexed line of that `include` directive in `introduced_at`.
        include_line: u32,
        /// 1 = direct include, 2 = included by a direct include, etc.
        depth: usize,
        /// 0-indexed line of the earliest `include` directive in the
        /// analysed file that reaches this symbol.
        effective_line: u32,
    },
}

impl VisibilityProvenance {
    /// The absolute path of the file that defines the symbol.
    pub fn origin(&self) -> Option<&Path> {
        match self {
            VisibilityProvenance::OwnFile => None,
            VisibilityProvenance::DirectInclude { origin, .. } => Some(origin),
            VisibilityProvenance::TransitiveInclude { origin, .. } => Some(origin),
        }
    }

    /// The line at which the symbol becomes visible in the analysed file.
    /// Returns `0` for own-file symbols and the include line for included
    /// symbols.
    pub fn include_line(&self) -> u32 {
        match self {
            VisibilityProvenance::OwnFile => 0,
            VisibilityProvenance::DirectInclude { include_line, .. } => *include_line,
            VisibilityProvenance::TransitiveInclude { include_line, .. } => *include_line,
        }
    }

    /// The 0-indexed line of the earliest `include` directive in the analysed
    /// file that reaches this symbol. Returns `0` for own-file symbols.
    pub fn effective_line(&self) -> u32 {
        match self {
            VisibilityProvenance::OwnFile => 0,
            VisibilityProvenance::DirectInclude { effective_line, .. } => *effective_line,
            VisibilityProvenance::TransitiveInclude { effective_line, .. } => *effective_line,
        }
    }

    /// Inclusion depth: `0` for own-file symbols, `1` for direct includes,
    /// `2+` for transitive includes.
    pub fn depth(&self) -> usize {
        match self {
            VisibilityProvenance::OwnFile => 0,
            VisibilityProvenance::DirectInclude { .. } => 1,
            VisibilityProvenance::TransitiveInclude { depth, .. } => *depth,
        }
    }
}

/// A symbol visible in the merged scope, with its provenance metadata.
#[derive(Debug, Clone)]
pub struct MergedSymbol {
    pub symbol: Symbol,
    pub provenance: VisibilityProvenance,
}

impl MergedSymbol {
    /// The 0-indexed line used for ordering this symbol in the analysed file.
    /// For own-file symbols this is the symbol's definition line; for included
    /// symbols it is the earliest current-file `include` line that reaches it.
    pub fn effective_line(&self) -> u32 {
        match self.provenance {
            VisibilityProvenance::OwnFile => self.symbol.selection_range.start.line,
            _ => self.provenance.effective_line(),
        }
    }
}

/// Directed graph of resolved includes for one file.
#[derive(Debug, Default, Clone)]
pub struct IncludeGraph {
    edges: Vec<IncludeEdge>,
    cyclic: bool,
}

impl IncludeGraph {
    /// All resolved include edges.
    pub fn edges(&self) -> &[IncludeEdge] {
        &self.edges
    }

    /// Return every file that has an edge pointing at `target`.
    pub fn dependents(&self, target: &Path) -> Vec<&Path> {
        self.edges
            .iter()
            .filter(|e| e.to == target)
            .map(|e| e.from.as_path())
            .collect()
    }

    /// True if a cycle was encountered during graph construction.
    pub fn is_cyclic(&self) -> bool {
        self.cyclic
    }
}

/// Why an include target was not folded into the merged scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncludeDiagnosticKind {
    /// The include target could not be resolved — no file exists at the
    /// resolved relative path under the mod overlay or vanilla game folder.
    Missing,
    /// The include target exists but could not be read or parsed (binary
    /// `.xs` random-map data, invalid UTF-8, parse failure, etc.).
    Unreadable,
}

/// Diagnostic produced when an include target cannot be brought into scope.
#[derive(Debug, Clone)]
pub struct IncludeDiagnostic {
    pub target: String,
    pub from: PathBuf,
    pub root: IncludeRoot,
    pub range: Range,
    pub kind: IncludeDiagnosticKind,
}

impl IncludeDiagnostic {
    /// A `Missing` diagnostic — the include target does not resolve.
    pub fn missing(target: String, from: PathBuf, root: IncludeRoot, range: Range) -> Self {
        Self {
            target,
            from,
            root,
            range,
            kind: IncludeDiagnosticKind::Missing,
        }
    }

    /// An `Unreadable` diagnostic — the include target exists but failed
    /// to read or parse.
    pub fn unreadable(target: String, from: PathBuf, root: IncludeRoot, range: Range) -> Self {
        Self {
            target,
            from,
            root,
            range,
            kind: IncludeDiagnosticKind::Unreadable,
        }
    }
}

/// The resolved, filtered scope of one file.
#[derive(Debug, Clone)]
pub struct MergedView {
    current_file: PathBuf,
    own_table: SymbolTable,
    symbols: Vec<MergedSymbol>,
    graph: IncludeGraph,
    missing: Vec<IncludeDiagnostic>,
    sources: std::collections::HashMap<PathBuf, String>,
    tables: std::collections::HashMap<PathBuf, SymbolTable>,
}

/// Cache key for an already-built `MergedView`.
///
/// The merged scope depends only on the current file text and the text of
/// every resolved include, so we hash those contents and sort by path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MergedViewCacheKey {
    pub current_hash: String,
    pub includes: Vec<(PathBuf, String)>,
}

impl MergedViewCacheKey {
    pub fn new(current_source: &str, view: &MergedView) -> Self {
        let current_hash = crate::cache::sha256_bytes(current_source.as_bytes());
        let mut includes: Vec<_> = view
            .files()
            .filter(|p| *p != view.current_file())
            .map(|p| {
                (
                    p.to_path_buf(),
                    crate::cache::sha256_bytes(view.source(p).unwrap_or("").as_bytes()),
                )
            })
            .collect();
        includes.sort_by(|a, b| a.0.cmp(&b.0));
        Self {
            current_hash,
            includes,
        }
    }
}

/// Error returned when `MergedView::build` cannot complete due to an I/O
/// failure or an include cycle. Missing include targets are **not** errors;
/// they are surfaced as `IncludeDiagnostic`s.
///
/// **Removed** in the LSP robustness refactor: the merged view builder no
/// longer propagates I/O or parse failures to its caller. Each `Result` from
/// per-include work is matched locally; on error a `tracing::warn!` is
/// emitted and the include target is skipped via an `IncludeDiagnostic` of
/// kind [`IncludeDiagnosticKind::Unreadable`]. This guarantees that a single
/// bad include target in the workspace (a binary `.xs` random-map file, a
/// permissions error, etc.) can never abort the merged-view build for the
/// rest of the file. The previous `MergeError` enum is intentionally gone so
/// callers cannot accidentally reintroduce fail-fast behaviour by `?`-ing
/// the builder's result.
impl MergedView {
    /// Build the merged view for `file` from its (already-parsed) source text.
    ///
    /// * Resolves every `include` through `workspace::resolve_include_edge`.
    /// * Loads included-file symbol tables through `cache::load_or_parse_symbols`.
    /// * Cycles are terminated cleanly; symbols seen up to the re-entry remain.
    /// * Missing targets become `IncludeDiagnostic`s of kind
    ///   [`IncludeDiagnosticKind::Missing`]; unreadable / unparseable targets
    ///   become diagnostics of kind [`IncludeDiagnosticKind::Unreadable`].
    ///   Neither aborts the build.
    pub fn build(
        file: &Path,
        source: &str,
        own_table: &SymbolTable,
        workspace: &Workspace,
        project: &VirtualProject,
        cache_dir: &Path,
    ) -> MergedView {
        let mut view = MergedView {
            current_file: file.to_path_buf(),
            own_table: own_table.clone(),
            symbols: Vec::new(),
            graph: IncludeGraph::default(),
            missing: Vec::new(),
            sources: std::collections::HashMap::new(),
            tables: std::collections::HashMap::new(),
        };

        view.sources.insert(file.to_path_buf(), source.to_string());
        view.tables.insert(file.to_path_buf(), own_table.clone());

        for sym in &own_table.symbols {
            view.symbols.push(MergedSymbol {
                symbol: sym.clone(),
                provenance: VisibilityProvenance::OwnFile,
            });
        }

        let tree = parser::parse(source);
        let directives = tree
            .as_ref()
            .map(|t| parser::extract_include_directives(t, source))
            .unwrap_or_default();
        let current_rel = relative_path_for(project, workspace, file);
        let mut visited = HashSet::new();
        visited.insert(file.to_path_buf());

        for (target, range) in directives {
            let line = range.start.line;
            match workspace.resolve_include_edge(project, &current_rel, &target, file, line) {
                Ok((to_path, edge)) => {
                    if visited.contains(&to_path) {
                        view.graph.edges.push(edge);
                        view.graph.cyclic = true;
                        continue;
                    }

                    visited.insert(to_path.clone());
                    view.graph.edges.push(edge.clone());

                    // Defensive: even though resolve_file now rejects non-.xs
                    // targets, a future regression shouldn't cascade into a
                    // UTF-8 decode failure here. Skip the merge for anything
                    // that isn't an XS source file.
                    if !crate::workspace::is_xs_file(&to_path) {
                        continue;
                    }

                    let rel = relative_path_for(project, workspace, &to_path);
                    let table = match cache::load_or_parse_symbols(&to_path, &rel, cache_dir) {
                        Ok(t) => t,
                        Err(e) => {
                            tracing::warn!(
                                target: "merged_view",
                                include = %to_path.display(),
                                error = %e,
                                "skipping bad include target (load_or_parse_symbols failed)"
                            );
                            view.missing.push(IncludeDiagnostic::unreadable(
                                target,
                                file.to_path_buf(),
                                edge.root,
                                range,
                            ));
                            continue;
                        }
                    };
                    let child_source = match std::fs::read_to_string(&to_path) {
                        Ok(s) => s,
                        Err(e) => {
                            tracing::warn!(
                                target: "merged_view",
                                include = %to_path.display(),
                                error = %e,
                                "skipping bad include target (could not read source)"
                            );
                            view.missing.push(IncludeDiagnostic::unreadable(
                                target,
                                file.to_path_buf(),
                                edge.root,
                                range,
                            ));
                            continue;
                        }
                    };

                    add_included_symbols(&table, &to_path, file, line, line, 1, &mut view);
                    view.sources.insert(to_path.clone(), child_source.clone());
                    view.tables.insert(to_path.clone(), table);

                    walk_includes(
                        &to_path,
                        &child_source,
                        1,
                        line,
                        workspace,
                        project,
                        cache_dir,
                        &mut visited,
                        &mut view,
                    );
                }
                Err(e) => {
                    let root = match &e {
                        crate::workspace::ResolveError::NotFound { root, .. } => *root,
                        crate::workspace::ResolveError::UnknownIncludeRoot => IncludeRoot::Ai,
                    };
                    view.missing.push(IncludeDiagnostic::missing(
                        target,
                        file.to_path_buf(),
                        root,
                        range,
                    ));
                }
            }
        }

        view
    }

    /// Find a merged symbol by exact name. Later declarations shadow earlier
    /// ones, matching `SymbolTable::find` semantics.
    pub fn find(&self, name: &str) -> Option<&MergedSymbol> {
        self.symbols.iter().rev().find(|ms| ms.symbol.name == name)
    }

    /// All merged symbols whose name starts with `prefix`.
    pub fn matching(&self, prefix: &str) -> impl Iterator<Item = &MergedSymbol> {
        self.symbols
            .iter()
            .filter(move |ms| ms.symbol.name.starts_with(prefix))
    }

    /// The include graph built during merge.
    pub fn graph(&self) -> &IncludeGraph {
        &self.graph
    }

    /// All include diagnostics: missing targets, unreadable targets, and
    /// any future reason a target could not be folded into scope.
    pub fn missing_includes(&self) -> &[IncludeDiagnostic] {
        &self.missing
    }

    /// Include targets that could not be resolved (no file at the resolved
    /// relative path). This is a convenience filter over [`Self::missing_includes`].
    pub fn unresolved_includes(&self) -> impl Iterator<Item = &IncludeDiagnostic> {
        self.missing
            .iter()
            .filter(|d| d.kind == IncludeDiagnosticKind::Missing)
    }

    /// Include targets that exist on disk but failed to read or parse
    /// (binary `.xs` random-map data, permissions errors, parse failure).
    /// The merged view was still built for the rest of the file; these
    /// targets were just omitted from the closure.
    pub fn unreadable_includes(&self) -> impl Iterator<Item = &IncludeDiagnostic> {
        self.missing
            .iter()
            .filter(|d| d.kind == IncludeDiagnosticKind::Unreadable)
    }

    /// The analysed file's own symbol table.
    pub fn own_table(&self) -> &SymbolTable {
        &self.own_table
    }

    /// Source text for a file in the include closure, if available.
    pub fn source(&self, path: &Path) -> Option<&str> {
        self.sources.get(path).map(|s| s.as_str())
    }

    /// All files participating in the merged view (own file + includes).
    pub fn files(&self) -> impl Iterator<Item = &Path> {
        std::iter::once(self.current_file.as_path()).chain(self.tables.keys().map(|p| p.as_path()))
    }

    /// Per-file symbol tables for every file in the include closure.
    pub fn tables(&self) -> &std::collections::HashMap<PathBuf, SymbolTable> {
        &self.tables
    }

    /// The absolute path of the file being analysed.
    pub fn current_file(&self) -> &Path {
        &self.current_file
    }

    /// Earliest line at which `name` is visible from the current file.
    /// Returns the symbol's definition line for own-file symbols and the
    /// earliest current-file `include` line for included symbols.
    pub fn visibility_line(&self, name: &str) -> Option<u32> {
        self.find(name).map(|ms| ms.effective_line())
    }

    /// All merged symbols.
    pub fn symbols(&self) -> &[MergedSymbol] {
        &self.symbols
    }
}

// Internal recursive helper; parameter count is driven by the include-graph
// traversal context and is not worth splitting for a purely stylistic lint.
#[allow(clippy::too_many_arguments)]
fn walk_includes(
    file: &Path,
    source: &str,
    depth: usize,
    effective_line: u32,
    workspace: &Workspace,
    project: &VirtualProject,
    cache_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    view: &mut MergedView,
) {
    let rel = relative_path_for(project, workspace, file);
    let tree = parser::parse(source);
    let directives = tree
        .as_ref()
        .map(|t| parser::extract_include_directives(t, source))
        .unwrap_or_default();

    for (target, range) in directives {
        let line = range.start.line;
        match workspace.resolve_include_edge(project, &rel, &target, file, line) {
            Ok((to_path, edge)) => {
                if visited.contains(&to_path) {
                    view.graph.edges.push(edge);
                    view.graph.cyclic = true;
                    continue;
                }

                visited.insert(to_path.clone());
                view.graph.edges.push(edge.clone());

                if !crate::workspace::is_xs_file(&to_path) {
                    continue;
                }

                let rel = relative_path_for(project, workspace, &to_path);
                let table = match cache::load_or_parse_symbols(&to_path, &rel, cache_dir) {
                    Ok(t) => t,
                    Err(e) => {
                        tracing::warn!(
                            target: "merged_view",
                            include = %to_path.display(),
                            error = %e,
                            "skipping bad include target (load_or_parse_symbols failed)"
                        );
                        view.missing.push(IncludeDiagnostic::unreadable(
                            target,
                            file.to_path_buf(),
                            edge.root,
                            range,
                        ));
                        continue;
                    }
                };
                let child_source = match std::fs::read_to_string(&to_path) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(
                            target: "merged_view",
                            include = %to_path.display(),
                            error = %e,
                            "skipping bad include target (could not read source)"
                        );
                        view.missing.push(IncludeDiagnostic::unreadable(
                            target,
                            file.to_path_buf(),
                            edge.root,
                            range,
                        ));
                        continue;
                    }
                };

                add_included_symbols(
                    &table,
                    &to_path,
                    file,
                    effective_line,
                    line,
                    depth + 1,
                    view,
                );
                view.sources.insert(to_path.clone(), child_source.clone());
                view.tables.insert(to_path.clone(), table);

                walk_includes(
                    &to_path,
                    &child_source,
                    depth + 1,
                    effective_line,
                    workspace,
                    project,
                    cache_dir,
                    visited,
                    view,
                );
            }
            Err(e) => {
                let root = match &e {
                    crate::workspace::ResolveError::NotFound { root, .. } => *root,
                    crate::workspace::ResolveError::UnknownIncludeRoot => IncludeRoot::Ai,
                };
                view.missing.push(IncludeDiagnostic::missing(
                    target,
                    file.to_path_buf(),
                    root,
                    range,
                ));
            }
        }
    }
}

fn add_included_symbols(
    table: &SymbolTable,
    origin: &Path,
    introduced_at: &Path,
    effective_line: u32,
    include_line: u32,
    depth: usize,
    view: &mut MergedView,
) {
    for sym in &table.symbols {
        if !include_visible(sym) {
            continue;
        }
        let provenance = if depth == 1 {
            VisibilityProvenance::DirectInclude {
                origin: origin.to_path_buf(),
                introduced_at: introduced_at.to_path_buf(),
                include_line,
                effective_line,
            }
        } else {
            VisibilityProvenance::TransitiveInclude {
                origin: origin.to_path_buf(),
                introduced_at: introduced_at.to_path_buf(),
                include_line,
                depth,
                effective_line,
            }
        };
        view.symbols.push(MergedSymbol {
            symbol: sym.clone(),
            provenance,
        });
    }
}

fn include_visible(sym: &Symbol) -> bool {
    match sym.kind {
        SymbolKind::Variable | SymbolKind::Constant => sym.visibility == Visibility::Extern,
        SymbolKind::Function | SymbolKind::Rule | SymbolKind::Class => {
            sym.visibility != Visibility::Local
        }
        SymbolKind::ClassField | SymbolKind::ClassMethod => false,
    }
}

fn relative_path_for(project: &VirtualProject, workspace: &Workspace, abs_path: &Path) -> String {
    project
        .file_overrides
        .iter()
        .find(|(_, p)| *p == abs_path)
        .map(|(rel, _)| rel.clone())
        .or_else(|| workspace.game_relative_path(abs_path))
        .unwrap_or_else(|| abs_path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbols;
    use crate::workspace::{VirtualProject as WorkspaceVirtualProject, Workspace};
    use tempfile::TempDir;
    use tower_lsp::lsp_types::Url;

    fn write(path: &Path, content: &str) -> std::io::Result<()> {
        std::fs::create_dir_all(path.parent().unwrap())?;
        std::fs::write(path, content)
    }

    fn build_view(file: &Path, game_path: &Path, mod_path: Option<&Path>) -> MergedView {
        let mut ws = Workspace::new(game_path.to_path_buf());
        let project = if let Some(m) = mod_path {
            ws.register_mod(Url::from_file_path(m).unwrap()).unwrap();
            ws.build_virtual_project(ws.mods().first().unwrap())
        } else {
            VirtualProject::default()
        };
        let source = std::fs::read_to_string(file).unwrap();
        let tree = parser::parse(&source).unwrap();
        let own = symbols::build_symbol_table(&tree, &source);
        let cache_dir = TempDir::new().unwrap();
        MergedView::build(file, &source, &own, &ws, &project, cache_dir.path())
    }

    #[test]
    fn direct_include_makes_symbol_visible() {
        // Scenario 4
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        write(&a, "include \"b.xs\";\n").unwrap();
        write(&b, "void helper() {}").unwrap();

        let view = build_view(&a, root, None);
        let ms = view.find("helper").expect("helper visible");
        assert_eq!(ms.symbol.kind, SymbolKind::Function);
        assert!(matches!(
            ms.provenance,
            VisibilityProvenance::DirectInclude { .. }
        ));
    }

    #[test]
    fn transitive_include_makes_symbol_visible() {
        // Scenario 5
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        let c = root.join("game").join("ai").join("c.xs");
        write(&a, "include \"b.xs\";\n").unwrap();
        write(&b, "include \"c.xs\";\n").unwrap();
        write(&c, "void deep() {}").unwrap();

        let view = build_view(&a, root, None);
        let ms = view.find("deep").expect("deep visible");
        assert_eq!(ms.symbol.kind, SymbolKind::Function);
        assert!(matches!(
            ms.provenance,
            VisibilityProvenance::TransitiveInclude { depth: 2, .. }
        ));
    }

    #[test]
    fn include_cycle_terminates_cleanly() {
        // Scenario 6
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        write(&a, "include \"b.xs\";\nvoid aFn() {}\n").unwrap();
        write(&b, "include \"a.xs\";\nvoid bFn() {}\n").unwrap();

        let view = build_view(&a, root, None);
        assert!(view.graph().is_cyclic());
        assert!(view.find("aFn").is_some());
        assert!(view.find("bFn").is_some());
        // No duplicate symbols should be introduced because of the visited set.
        let a_count = view
            .symbols()
            .iter()
            .filter(|ms| ms.symbol.name == "aFn")
            .count();
        let b_count = view
            .symbols()
            .iter()
            .filter(|ms| ms.symbol.name == "bFn")
            .count();
        assert_eq!(a_count, 1, "own symbol should appear once");
        assert_eq!(b_count, 1, "included symbol should appear once");
    }

    #[test]
    fn missing_include_target_produces_diagnostic() {
        // Scenario 7
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        write(&a, "include \"nonexistent.xs\";\ninclude \"b.xs\";\n").unwrap();
        write(&b, "void helper() {}").unwrap();

        let view = build_view(&a, root, None);
        let missing: Vec<_> = view
            .missing_includes()
            .iter()
            .map(|d| d.target.clone())
            .collect();
        assert!(missing.contains(&"nonexistent.xs".to_string()));
        // Other includes still resolve.
        assert!(view.find("helper").is_some());
    }

    #[test]
    fn mod_overlay_on_include_target_is_preferred() {
        // Scenario 8
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let mod_root = root.join("mod_a");
        let a = mod_root.join("game").join("ai").join("a.xs");
        let vanilla_b = root.join("game").join("ai").join("b.xs");
        let overlay_b = mod_root.join("game").join("ai").join("b.xs");
        write(&a, "include \"b.xs\";\n").unwrap();
        write(&vanilla_b, "void vanilla() {}").unwrap();
        write(&overlay_b, "void overlay() {}").unwrap();

        let view = build_view(&a, root, Some(&mod_root));
        let ms = view.find("overlay").expect("overlay symbol from mod");
        assert!(matches!(
            ms.provenance,
            VisibilityProvenance::DirectInclude { .. }
        ));
        assert!(view.find("vanilla").is_none());
    }

    #[test]
    fn static_variable_in_included_file_is_hidden() {
        // Scenario 9 (bonus coverage)
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        write(&a, "include \"b.xs\";\n").unwrap();
        write(&b, "static int gHidden = 0;\n").unwrap();

        let view = build_view(&a, root, None);
        assert!(view.find("gHidden").is_none());
    }

    #[test]
    fn extern_variable_in_included_file_is_visible() {
        // Scenario 10 (bonus coverage)
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        write(&a, "include \"b.xs\";\n").unwrap();
        write(&b, "extern int gShared = 0;\n").unwrap();

        let view = build_view(&a, root, None);
        let ms = view.find("gShared").expect("extern variable visible");
        assert_eq!(ms.symbol.kind, SymbolKind::Variable);
    }

    #[test]
    fn public_function_in_included_file_is_visible() {
        // Scenario 11 (bonus coverage)
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        write(&a, "include \"b.xs\";\n").unwrap();
        write(&b, "void helper() {}\n").unwrap();

        let view = build_view(&a, root, None);
        assert!(view.find("helper").is_some());
    }

    #[test]
    fn cache_key_matches_for_identical_closure() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        write(&a, "include \"b.xs\";\n").unwrap();
        write(&b, "void helper() {}\n").unwrap();

        let view = build_view(&a, root, None);
        let key1 = MergedViewCacheKey::new("void main() {}", &view);
        let key2 = MergedViewCacheKey::new("void main() {}", &view);
        assert_eq!(key1, key2);
    }

    #[test]
    fn cache_key_differs_when_include_changes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        write(&a, "include \"b.xs\";\n").unwrap();
        write(&b, "void helper() {}\n").unwrap();

        let view1 = build_view(&a, root, None);
        let key1 = MergedViewCacheKey::new("void main() {}", &view1);

        std::fs::write(&b, "void changed() {}").unwrap();
        let view2 = build_view(&a, root, None);
        let key2 = MergedViewCacheKey::new("void main() {}", &view2);

        assert_ne!(key1, key2);
    }

    // -----------------------------------------------------------------------
    // Robustness: a single bad include target must not abort the merged-view
    // build. These tests are the regression guard for the user's complaint:
    //   "The LSP needs to be more robust, it can't give up on every single
    //    bad file. If a file fails to parse, it should simply output an
    //    error message, then temporarily exclude that file from compilation."
    // -----------------------------------------------------------------------

    /// Build a `MergedView` for a fake `game/random_maps/` includer that
    /// includes a sibling `.xs` file. The directory choice matters: under
    /// `game/random_maps/` the include-root is `RandomMap`, so includes
    /// like `"binary_target.xs"` resolve to `random_maps/binary_target.xs`
    /// in the same directory. The include target is written BEFORE the
    /// build runs (see `include_target_bytes` parameter), so the build can
    /// find it on disk.
    fn build_random_map_view(
        includer_content: &str,
        include_target_filename: &str,
        include_target_bytes: Option<&[u8]>,
    ) -> MergedView {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let rm_dir = root.join("game").join("random_maps");
        std::fs::create_dir_all(&rm_dir).unwrap();

        let includer = rm_dir.join("test_map.xs");
        std::fs::write(&includer, includer_content).unwrap();

        if let Some(bytes) = include_target_bytes {
            let target = rm_dir.join(include_target_filename);
            std::fs::write(&target, bytes).unwrap();
        }

        let ws = Workspace::new(root.to_path_buf());
        let project = WorkspaceVirtualProject::default();
        let source = std::fs::read_to_string(&includer).unwrap();
        let tree = parser::parse(&source).unwrap();
        let own = crate::symbols::build_symbol_table(&tree, &source);
        let cache_dir = TempDir::new().unwrap();
        MergedView::build(&includer, &source, &own, &ws, &project, cache_dir.path())
    }

    /// A `.xs` include target whose contents are binary garbage (the AoM:R
    /// `game/random_maps/*.xs` random-map case) MUST NOT abort the
    /// merged-view build. The view must still contain the includer's own
    /// symbols and the bad include must be reported as an
    /// `Unreadable` diagnostic.
    #[test]
    fn build_succeeds_when_include_target_is_binary_xs() {
        let view = build_random_map_view(
            "void ownFn() {}\ninclude \"aso_grasslands.xs\";\n",
            "aso_grasslands.xs",
            Some(&[0xff, 0xfe, 0x00, 0xab, 0xcd, 0xef, 0x01]),
        );

        // The own-file symbol must still be present.
        assert!(
            view.find("ownFn").is_some(),
            "own-file symbols must be present even when an include target is unreadable"
        );

        // The bad include must be recorded as an Unreadable diagnostic.
        let unreadable_targets: Vec<&str> = view
            .unreadable_includes()
            .map(|d| d.target.as_str())
            .collect();
        assert!(
            unreadable_targets.contains(&"aso_grasslands.xs"),
            "bad include target should be reported as Unreadable, got {unreadable_targets:?}"
        );

        // The merged view's own symbol table must be intact.
        assert!(view.own_table().find("ownFn").is_some());
    }

    /// A missing include target (the file does not exist on disk at all)
    /// MUST NOT abort the merged-view build. It is reported as a `Missing`
    /// diagnostic; the rest of the file's symbols are still in scope.
    #[test]
    fn build_succeeds_when_include_target_is_missing() {
        let view = build_random_map_view(
            "void ownFn() {}\ninclude \"never_exists.xs\";\n",
            "never_exists.xs",
            None, // do not write the target file at all
        );

        // No panic, no Result-returning signature: build() returned a MergedView.
        // Own-file symbols must still be present.
        assert!(
            view.find("ownFn").is_some(),
            "own-file symbols must be present when an include target is missing"
        );

        let missing_targets: Vec<&str> = view
            .unresolved_includes()
            .map(|d| d.target.as_str())
            .collect();
        assert!(
            missing_targets.contains(&"never_exists.xs"),
            "missing include target should be reported as Missing, got {missing_targets:?}"
        );
    }

    /// `walk_includes` (the recursive helper) is exercised transitively
    /// through `build`. A bad include target deep in a transitive include
    /// chain must not abort the build either: the sibling good include
    /// still contributes its symbols, and the bad one is recorded as
    /// `Unreadable`.
    #[test]
    fn walk_includes_skips_unreadable_includes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        // All files under `game/ai/` so the include-root is `Ai` and
        // resolves relative to that prefix.
        let a = root.join("game").join("ai").join("a.xs");
        let b = root.join("game").join("ai").join("b.xs");
        let c = root.join("game").join("ai").join("c.xs");

        write(&a, "include \"b.xs\";\nvoid aFn() {}\n").unwrap();
        write(&b, "include \"c.xs\";\nvoid bFn() {}\n").unwrap();
        // c.xs exists as a binary blob — the recursive walker should skip it.
        std::fs::write(&c, [0xffu8, 0xfe, 0x00, 0xab, 0xcd]).unwrap();

        let ws = Workspace::new(root.to_path_buf());
        let project = WorkspaceVirtualProject::default();
        let source = std::fs::read_to_string(&a).unwrap();
        let tree = parser::parse(&source).unwrap();
        let own = crate::symbols::build_symbol_table(&tree, &source);
        let cache_dir = TempDir::new().unwrap();

        let view = MergedView::build(&a, &source, &own, &ws, &project, cache_dir.path());

        assert!(
            view.find("aFn").is_some(),
            "own-file symbol must be present"
        );
        assert!(
            view.find("bFn").is_some(),
            "transitively-included text file's symbol must be present even when a sibling include target is unreadable"
        );

        let unreadable: Vec<&str> = view
            .unreadable_includes()
            .map(|d| d.target.as_str())
            .collect();
        assert!(
            unreadable.contains(&"c.xs"),
            "transitive bad include target should be reported as Unreadable, got {unreadable:?}"
        );
    }
}
