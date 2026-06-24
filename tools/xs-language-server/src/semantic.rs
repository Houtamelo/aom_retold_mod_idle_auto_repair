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

use anyhow::Context;
use tower_lsp::lsp_types::{Diagnostic, DiagnosticSeverity, Position, Range};

use crate::parser;
use crate::symbols::{Symbol, SymbolKind, SymbolTable, Visibility};

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
    pub fn load_from_workspace(
        workspace: &crate::workspace::Workspace,
        project: &crate::workspace::VirtualProject,
        cache_dir: &Path,
    ) -> anyhow::Result<Self> {
        let mut files = HashMap::new();
        for (rel, path) in project.visible_files(workspace) {
            // Prefer the cached symbol table when available; otherwise parse
            // directly from disk.
            let symbols = crate::cache::load_or_parse_symbols(&path, &rel, cache_dir)?;
            let source = std::fs::read_to_string(&path)
                .with_context(|| format!("reading source for semantic analysis: {path:?}"))?;
            files.insert(path, ParsedFile {
                source,
                table: symbols,
            });
        }
        Ok(Self { files })
    }
}

/// Convenience wrapper around a semantic project.
pub struct SemanticChecker {
    pub project: VirtualProject,
}

impl SemanticChecker {
    pub fn new(project: VirtualProject) -> Self {
        Self { project }
    }

    pub fn check_all(&self, current_file: &Path) -> Vec<Diagnostic> {
        check_all(&self.project, current_file)
    }
}

/// Run all semantic checks and return the combined diagnostics.
pub fn check_all(project: &VirtualProject, current_file: &Path) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    out.extend(check_extern_collisions(project));
    out.extend(check_forward_declarations(project, current_file));
    out.extend(check_mutable_redefinitions(project));
    out
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
pub fn check_forward_declarations(project: &VirtualProject, current_file: &Path) -> Vec<Diagnostic> {
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
    use std::path::PathBuf;

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
        let diags = check_forward_declarations(&prj, &p("a.xs"));
        assert!(diags.is_empty(), "expected no forward-decl errors, got {diags:?}");
    }

    #[test]
    fn missing_forward_declaration_is_error() {
        let prj = project(&[("a.xs", "void foo() { bar(); }\nvoid bar() {}\n")]);
        let diags = check_forward_declarations(&prj, &p("a.xs"));
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("before declaration"));
        assert!(diags[0].message.contains("bar"));
    }

    #[test]
    fn mutable_function_is_forward_callable() {
        let prj = project(&[("a.xs", "mutable void foo() {}\nvoid bar() { foo(); }\nvoid foo() {}\n")]);
        let diags = check_forward_declarations(&prj, &p("a.xs"));
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
        let diags = check_forward_declarations(&prj, &p("a.xs"));
        assert!(diags.is_empty(), "cross-file function should be visible, got {diags:?}");
    }

    #[test]
    fn unresolved_symbol_emits_error_0310() {
        let prj = project(&[("a.xs", "void foo() { doesNotExist(); }\n")]);
        let diags = check_forward_declarations(&prj, &p("a.xs"));
        assert_eq!(diags.len(), 1);
        assert!(diags[0].message.contains("Error 0310"));
        assert!(diags[0].message.contains("doesNotExist"));
    }
}
