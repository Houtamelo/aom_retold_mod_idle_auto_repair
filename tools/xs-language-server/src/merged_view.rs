//! Textual-paste include resolution.
//!
//! For an analysed file, `MergedView` collects the file's own symbols plus
//! the visibility-filtered symbols from every directly and transitively
//! included file. Included `static`/file-local variables are hidden;
//! `extern` variables and public functions are visible. Cycles terminate
//! cleanly and missing targets become diagnostics rather than fatal errors.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
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

/// Diagnostic produced when an include target cannot be resolved.
#[derive(Debug, Clone)]
pub struct IncludeDiagnostic {
    pub target: String,
    pub from: PathBuf,
    pub root: IncludeRoot,
    pub range: Range,
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
#[derive(Debug)]
pub enum MergeError {
    Io { path: PathBuf, source: anyhow::Error },
    Cycle(Vec<PathBuf>),
}

impl std::fmt::Display for MergeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MergeError::Io { path, source } => {
                write!(f, "failed to read include {}: {source}", path.display())
            }
            MergeError::Cycle(paths) => write!(
                f,
                "include cycle detected: {}",
                paths.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(" -> ")
            ),
        }
    }
}

impl std::error::Error for MergeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MergeError::Io { source, .. } => Some(source.as_ref()),
            MergeError::Cycle(_) => None,
        }
    }
}

impl MergedView {
    /// Build the merged view for `file` from its (already-parsed) source text.
    ///
    /// * Resolves every `include` through `workspace::resolve_include_edge`.
    /// * Loads included-file symbol tables through `cache::load_or_parse_symbols`.
    /// * Cycles are terminated cleanly; symbols seen up to the re-entry remain.
    /// * Missing targets become `IncludeDiagnostic`s, not fatal errors.
    pub fn build(
        file: &Path,
        source: &str,
        own_table: &SymbolTable,
        workspace: &Workspace,
        project: &VirtualProject,
        cache_dir: &Path,
    ) -> Result<MergedView, MergeError> {
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

                    let rel = relative_path_for(project, workspace, &to_path);
                    let table = cache::load_or_parse_symbols(&to_path, &rel, cache_dir)
                        .with_context(|| {
                            format!("loading symbol table for {}", to_path.display())
                        })
                        .map_err(|e| MergeError::Io {
                            path: to_path.clone(),
                            source: e,
                        })?;
                    let child_source = std::fs::read_to_string(&to_path)
                        .with_context(|| format!("reading source for {}", to_path.display()))
                        .map_err(|e| MergeError::Io {
                            path: to_path.clone(),
                            source: e,
                        })?;

                    add_included_symbols(&table, &to_path, file, line, 1, &mut view);
                    view.sources.insert(to_path.clone(), child_source.clone());
                    view.tables.insert(to_path.clone(), table);

                    walk_includes(
                        &to_path,
                        &child_source,
                        1,
                        workspace,
                        project,
                        cache_dir,
                        &mut visited,
                        &mut view,
                    )?;
                }
                Err(e) => {
                    let root = match &e {
                        crate::workspace::ResolveError::NotFound { root, .. } => *root,
                        crate::workspace::ResolveError::UnknownIncludeRoot => IncludeRoot::Ai,
                    };
                    view.missing.push(IncludeDiagnostic {
                        target,
                        from: file.to_path_buf(),
                        root,
                        range,
                    });
                }
            }
        }

        Ok(view)
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

    /// Diagnostics for include targets that could not be resolved.
    pub fn missing_includes(&self) -> &[IncludeDiagnostic] {
        &self.missing
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
    /// Returns `0` for own-file symbols and the relevant `include_line` for
    /// included symbols.
    pub fn visibility_line(&self, name: &str) -> Option<u32> {
        self.find(name).map(|ms| ms.provenance.include_line())
    }

    /// All merged symbols.
    pub fn symbols(&self) -> &[MergedSymbol] {
        &self.symbols
    }
}

fn walk_includes(
    file: &Path,
    source: &str,
    depth: usize,
    workspace: &Workspace,
    project: &VirtualProject,
    cache_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    view: &mut MergedView,
) -> Result<(), MergeError> {
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

                let rel = relative_path_for(project, workspace, &to_path);
                let table = cache::load_or_parse_symbols(&to_path, &rel, cache_dir)
                    .with_context(|| {
                        format!("loading symbol table for {}", to_path.display())
                    })
                    .map_err(|e| MergeError::Io {
                        path: to_path.clone(),
                        source: e,
                    })?;
                let child_source = std::fs::read_to_string(&to_path)
                    .with_context(|| format!("reading source for {}", to_path.display()))
                    .map_err(|e| MergeError::Io {
                        path: to_path.clone(),
                        source: e,
                    })?;

                add_included_symbols(&table, &to_path, file, line, depth + 1, view);
                view.sources.insert(to_path.clone(), child_source.clone());
                view.tables.insert(to_path.clone(), table);

                    walk_includes(
                        &to_path, &child_source, depth + 1, workspace, project, cache_dir, visited,
                        view,
                    )?;
            }
            Err(e) => {
                let root = match &e {
                    crate::workspace::ResolveError::NotFound { root, .. } => *root,
                    crate::workspace::ResolveError::UnknownIncludeRoot => IncludeRoot::Ai,
                };
                view.missing.push(IncludeDiagnostic {
                    target,
                    from: file.to_path_buf(),
                    root,
                    range,
                });
            }
        }
    }

    Ok(())
}

fn add_included_symbols(
    table: &SymbolTable,
    origin: &Path,
    introduced_at: &Path,
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
            }
        } else {
            VisibilityProvenance::TransitiveInclude {
                origin: origin.to_path_buf(),
                introduced_at: introduced_at.to_path_buf(),
                include_line,
                depth,
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
        SymbolKind::Function | SymbolKind::Rule => sym.visibility != Visibility::Local,
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
    use crate::workspace::Workspace;
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
        MergedView::build(file, &source, &own, &ws, &project, cache_dir.path()).unwrap()
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
}
