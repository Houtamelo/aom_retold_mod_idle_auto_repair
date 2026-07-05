//! Convert parser diagnostics and semantic checks into LSP `Diagnostic`s.
//!
//! `collect_diagnostics` maps the `lelwel` parser's `Diagnostic` values to
//! LSP severity/ranges. `collect_all` layers definition-time and type-check
//! diagnostics on top.

use std::{collections::HashMap, path::Path, path::PathBuf};

use codespan_reporting::diagnostic::Severity;
use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, NumberOrString, Range, Uri};

use crate::{
    definition_check,
    engine_api::EngineApi,
    merged_view::{IncludeDiagnostic, MergedView},
    range::span_to_range,
    semantic,
    semantic::VirtualProject,
    symbols::SymbolTable,
    typecheck,
};

/// Consume `lelwel` parser diagnostics and convert them to LSP diagnostics.
///
/// This entry point only knows the built-in primitive XS types. For
/// workspace-aware parsing that recognizes user-defined classes such as
/// `BOSystem`, use [`collect_diagnostics_with_types`].
pub fn collect_diagnostics(source: &str) -> Vec<Diagnostic> {
    collect_diagnostics_with_types(source, xs_parser::ast::TypeTable::with_primitives())
}

/// Parse `source` with a caller-supplied type table and return LSP diagnostics.
pub fn collect_diagnostics_with_types(
    source: &str,
    types: xs_parser::ast::TypeTable,
) -> Vec<Diagnostic> {
    let mut parse_diags = Vec::new();
    let _cst = xs_parser::parser::Parser::new_with_context(source, &mut parse_diags, types)
        .parse(&mut parse_diags);
    parse_diags
        .into_iter()
        .map(|d| xs_diagnostic_to_lsp(source, &d))
        .collect()
}

/// Build a `TypeTable` seeded with primitives plus every class name visible
/// across the semantic project.
///
/// Class definitions are top-level constructs, but parser recovery inside a
/// class body can cause the typed AST to drop later classes in the same file.
/// We therefore scan each file's source with the lightweight extractor from
/// `symbols::extract_class_names`, which captures every `class <Identifier>`
/// declaration regardless of whether the body parsed cleanly.
fn type_table_with_project_classes(project: Option<&VirtualProject>) -> xs_parser::ast::TypeTable {
    let mut types = xs_parser::ast::TypeTable::with_primitives();
    let Some(project) = project else { return types };
    for file in project.files.values() {
        for name in crate::symbols::extract_class_names(&file.source) {
            if !types.is_type(&name) {
                types.insert_class(&name);
            }
        }
    }
    types
}

fn xs_diagnostic_to_lsp(source: &str, d: &xs_parser::parser::Diagnostic) -> Diagnostic {
    let range = d
        .labels
        .first()
        .map(|label| span_to_range(source, label.range.clone()))
        .unwrap_or_default();
    let severity = Some(match d.severity {
        Severity::Bug | Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Note => DiagnosticSeverity::INFORMATION,
        Severity::Help => DiagnosticSeverity::HINT,
    });
    Diagnostic {
        range,
        severity,
        code: None,
        code_description: None,
        source: Some("xs-language-server".to_string()),
        message: d.message.clone(),
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Bundle of inputs that drive a single diagnostic pass.
///
/// `file_view` is the forward include-paste view for the file being diagnosed.
/// `root_view` is the full include chain of a root that includes the file; in
/// the V1 single-pass path it is typically `None` (falls back to `file_view)
/// and in the PR-5 multi-root path it is the resolved root chain.
#[derive(Debug, Clone, Copy)]
pub struct DiagnosticContext<'a> {
    pub source: &'a str,
    pub engine: &'a EngineApi,
    pub table: &'a SymbolTable,
    pub project: Option<&'a VirtualProject>,
    pub file: Option<&'a Path>,
    pub file_view: &'a MergedView,
    pub root_view: Option<&'a MergedView>,
}

/// Full diagnostic pass for a file: parse errors + type-check + semantic.
///
/// Primary callers should build a [`DiagnosticContext`] and call this
/// function. The per-root loop in PR-5 will call [`run_pass`] directly.
pub fn collect_all(ctx: &DiagnosticContext<'_>) -> DiagnosticsByUri { run_pass(ctx) }

/// Inner single-root diagnostic pass used by [`collect_all`] and by the
/// PR-5 per-root aggregation loop.
pub fn run_pass(ctx: &DiagnosticContext<'_>) -> DiagnosticsByUri {
    let mut diags: DiagnosticsByUri = HashMap::new();

    let Some(current_file) = ctx.file else {
        return diags;
    };
    let Some(uri) = Uri::from_file_path(current_file) else {
        return diags;
    };

    let types = type_table_with_project_classes(ctx.project);
    let parse_diags = collect_diagnostics_with_types(ctx.source, types);
    if !parse_diags.is_empty() {
        diags.entry(uri.clone()).or_default().extend(parse_diags);
    }

    let definition_diags = definition_check::validate_definitions(ctx.source);
    if !definition_diags.is_empty() {
        diags.entry(uri.clone()).or_default().extend(definition_diags);
    }

    let typecheck_diags =
        typecheck::check_calls_with_merged(ctx.source, ctx.engine, ctx.table, Some(ctx.file_view), ctx.root_view, ctx.project);
    if !typecheck_diags.is_empty() {
        diags.entry(uri.clone()).or_default().extend(typecheck_diags);
    }

    if let Some(p) = ctx.project {
        let extern_diags = semantic::check_extern_collisions(p, current_file, Some(ctx.file_view), ctx.root_view);
        merge_diagnostic_maps(&mut diags, extern_diags);

        let fwd = semantic::check_forward_declarations_for_merged_view(
            p,
            ctx.engine,
            current_file,
            ctx.file_view,
            ctx.root_view,
        );
        if !fwd.is_empty() {
            diags.entry(uri.clone()).or_default().extend(fwd);
        }

        let mut_diags = semantic::check_mutable_redefinitions_for_merged_view(ctx.file_view, ctx.root_view);
        if !mut_diags.is_empty() {
            diags.entry(uri.clone()).or_default().extend(mut_diags);
        }
    }

    for inc in ctx.file_view.missing_includes() {
        diags.entry(uri.clone()).or_default().push(include_diagnostic_to_lsp(inc));
    }

    diags.entry(uri.clone()).or_default();
    diags
}

fn merge_diagnostic_maps(base: &mut DiagnosticsByUri, other: DiagnosticsByUri) {
    for (uri, ds) in other {
        base.entry(uri).or_default().extend(ds);
    }
}

fn include_diagnostic_to_lsp(inc: &IncludeDiagnostic) -> Diagnostic {
    Diagnostic {
        range: inc.range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: Some(tower_lsp_server::ls_types::NumberOrString::String("E0310".to_string())),
        code_description: None,
        source: Some("xs-language-server".to_string()),
        message: format!("include not found: {}", inc.target),
        related_information: None,
        tags: None,
        data: None,
    }
}

/// Diagnostics grouped by the URI to which they belong.
pub type DiagnosticsByUri = HashMap<tower_lsp_server::ls_types::Uri, Vec<Diagnostic>>;

/// Stable classification bucket for a [`Diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCategory {
    ExternCollision,
    UnresolvedSymbol,
    WrongArgCount,
    WrongArgType,
    DefinitionError,
    WrongRangeUri,
    Other,
}

/// Classify a diagnostic by its message content.
pub fn categorize(d: &Diagnostic) -> DiagnosticCategory {
    let msg = d.message.to_ascii_lowercase();
    if msg.contains("duplicate extern") || msg.contains("extern collision") {
        DiagnosticCategory::ExternCollision
    } else if msg.contains("must have a default value")
        || msg.contains("cannot have a default value")
        || msg.contains("must be initialized")
        || msg.contains("must be assigned a constant expression")
    {
        DiagnosticCategory::DefinitionError
    } else if msg.contains("expected") && msg.contains("argument") && msg.contains("of type") {
        DiagnosticCategory::WrongArgType
    } else if msg.contains("expected") && msg.contains("argument") {
        DiagnosticCategory::WrongArgCount
    } else if msg.contains("error 0310") || msg.contains("invalid symbol lookup") || msg.contains("unresolved") {
        DiagnosticCategory::UnresolvedSymbol
    } else {
        DiagnosticCategory::Other
    }
}

/// True if `d` is a duplicate-`extern` / `extern`-collision diagnostic.
pub fn is_duplicate_extern(d: &Diagnostic) -> bool { categorize(d) == DiagnosticCategory::ExternCollision }

/// True if `d` is an unresolved-symbol/use-before-declaration diagnostic.
pub fn is_unresolved_symbol(d: &Diagnostic) -> bool { categorize(d) == DiagnosticCategory::UnresolvedSymbol }

/// True if `d` is a wrong-argument-count diagnostic.
pub fn is_argument_mismatch(d: &Diagnostic) -> bool { categorize(d) == DiagnosticCategory::WrongArgCount }

// -----------------------------------------------------------------------------
// PR-5: multi-root aggregation
// -----------------------------------------------------------------------------

/// One diagnostic produced by a single root during the multi-root pass.
///
/// The `base_message` is the diagnostic's message with any previous root-suffix
/// stripped, so two `PerRootDiagnostic`s from different roots can be compared
/// for equality under the aggregation key.
#[derive(Debug, Clone)]
pub struct PerRootDiagnostic {
    pub uri: Uri,
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub code: Option<NumberOrString>,
    pub category: DiagnosticCategory,
    pub base_message: String,
    pub producing_root: PathBuf,
}

/// Equality key for the aggregation step.
pub type DiagnosticKey = (Uri, Range, DiagnosticCategory, String);

/// True when the supplied diagnostic set contains no cross-file issue.
///
/// "Cross-file issue" = any diagnostic whose [`categorize`] falls under a
/// cross-file-dependent category (unresolved symbol, wrong argument
/// count/type, extern collision). Local parse errors and definition-shape
/// errors are not cross-file, so the multi-root loop is unnecessary for them.
pub fn should_skip_multi_root_pass(by_uri: &DiagnosticsByUri) -> bool {
    for diags in by_uri.values() {
        for d in diags {
            let cat = categorize(d);
            match cat {
                DiagnosticCategory::UnresolvedSymbol
                | DiagnosticCategory::WrongArgCount
                | DiagnosticCategory::WrongArgType
                | DiagnosticCategory::ExternCollision => return false,
                DiagnosticCategory::Other
                | DiagnosticCategory::DefinitionError
                | DiagnosticCategory::WrongRangeUri => {}
            }
        }
    }
    true
}

/// Display name of a producing root: the file path's last component.
fn root_name(path: &Path) -> String {
    path.file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Order the producing roots by the user-confirmed priority:
///   1. currently-open (in the order supplied)
///   2. mod-overlay (paths under any registered mod root, alphabetically)
///   3. alphabetical remainder
fn order_producing_roots(
    producing_roots: &[PathBuf],
    currently_open: &[PathBuf],
    mod_overlay_paths: &[PathBuf],
) -> Vec<PathBuf> {
    let mut open_in_producing: Vec<PathBuf> = currently_open
        .iter()
        .filter(|p| producing_roots.iter().any(|pr| pr == *p))
        .cloned()
        .collect();
    open_in_producing.sort();

    let mut mod_overlay_in_producing: Vec<PathBuf> = producing_roots
        .iter()
        .filter(|p| {
            mod_overlay_paths.iter().any(|m| is_under(p, m))
                && !open_in_producing.iter().any(|o| o == *p)
        })
        .cloned()
        .collect();
    mod_overlay_in_producing.sort();
    mod_overlay_in_producing.dedup();

    let mut alphabetical: Vec<PathBuf> = producing_roots
        .iter()
        .filter(|p| {
            !open_in_producing.iter().any(|o| o == *p)
                && !mod_overlay_in_producing.iter().any(|m| m == *p)
        })
        .cloned()
        .collect();
    alphabetical.sort();
    alphabetical.dedup();

    open_in_producing
        .into_iter()
        .chain(mod_overlay_in_producing)
        .chain(alphabetical)
        .collect()
}

/// True if `path` is under `dir` (the path starts with the dir prefix).
fn is_under(path: &Path, dir: &Path) -> bool {
    if path == dir {
        return false;
    }
    let path_str = path.to_string_lossy();
    let dir_str = dir.to_string_lossy();
    let dir_with_sep = if dir_str.ends_with('/') {
        dir_str.into_owned()
    } else {
        format!("{}/", dir_str)
    };
    path_str.starts_with(&dir_with_sep)
}

/// Format the trailing root-suffix according to the locked aggregation rules.
/// To be called from [`aggregate_results`] which supplies context.
///
/// Behavior:
/// - No producing roots: empty.
/// - Total == 1 AND producing root is the diagnosed file itself (orphan): empty.
/// - Total == 1 AND producing root differs from the diagnosed file: list it.
/// - Total > 1, all producing (= total): universal, empty.
/// - Total > 1, partial (<= 100): list every producing root in priority order.
/// - Total > 1, truncated: top 3 + "...and N more".
pub fn format_root_suffix(
    current_uri: &Uri,
    producing_roots: &[PathBuf],
    total_root_count: usize,
    mod_overlay_paths: &[PathBuf],
    currently_open: &[PathBuf],
) -> String {
    if producing_roots.is_empty() {
        return String::new();
    }

    let current_file_name = std::path::Path::new(current_uri.path().as_str())
        .file_name()
        .map(|f| f.to_string_lossy().into_owned())
        .unwrap_or_default();

    if total_root_count <= 1 {
        // Single root: orphan (self-root) -> no suffix; reachable root -> list it.
        let first_name = root_name(&producing_roots[0]);
        if first_name == current_file_name {
            return String::new();
        }
        return format!("\n  as seen from: {}", first_name);
    }

    // Universal coverage: every root produces AND total > 1 -> no suffix.
    if producing_roots.len() == total_root_count && total_root_count <= 100 {
        return String::new();
    }

    let ordered = order_producing_roots(producing_roots, currently_open, mod_overlay_paths);
    let names: Vec<String> = ordered.iter().map(|p| root_name(p)).collect();

    if total_root_count <= 100 || names.len() <= 3 {
        format!("\n  as seen from: {}", names.join(", "))
    } else {
        let shown: Vec<String> = names.iter().take(3).cloned().collect();
        let rest = names.len() - 3;
        format!("\n  as seen from: {} ...and {} more", shown.join(", "), rest)
    }
}

/// Strip any existing trailing root-suffix from a diagnostic message.
///
/// The inverse of [`format_root_suffix`]. Used by [`PerRootDiagnostic`]
/// extraction to recover the base message before comparison.
pub fn strip_root_suffix(message: &str) -> &str {
    const MARKER: &str = "\n  as seen from:";
    match message.find(MARKER) {
        Some(idx) => &message[..idx],
        None => message,
    }
}

/// Aggregate equivalent [`PerRootDiagnostic`]s across roots into LSP
/// [`Diagnostic`]s grouped by URI.
///
/// Two inputs are equivalent iff their (uri, range, category, base_message)
/// match. Aggregation is deterministic: producing-roots are sorted in priority
/// order (currently-open -> mod-overlay -> alphabetical) before the suffix
/// is computed.
///
/// `total_root_count` is the total number of roots in the multi-root pass
/// (including the diagnosed file's own self-root). `mod_overlay_paths` and
/// `currently_open` drive the producing-roots priority ordering.
pub fn aggregate_results(
    inputs: Vec<PerRootDiagnostic>,
    total_root_count: usize,
    mod_overlay_paths: &[PathBuf],
    currently_open: &[PathBuf],
) -> DiagnosticsByUri {
    let mut groups: HashMap<
        DiagnosticKey,
        (DiagnosticSeverity, Option<NumberOrString>, Vec<PathBuf>),
    > = HashMap::new();
    for prd in inputs {
        let key: DiagnosticKey = (
            prd.uri.clone(),
            prd.range,
            prd.category,
            prd.base_message.clone(),
        );
        let entry = groups.entry(key).or_insert_with(|| {
            (prd.severity, prd.code.clone(), Vec::new())
        });
        entry.2.push(prd.producing_root);
    }

    let mut out: DiagnosticsByUri = HashMap::new();
    for (key, (severity, code, mut roots)) in groups {
        roots.sort();
        roots.dedup();
        let suffix = format_root_suffix(
            &key.0,
            &roots,
            total_root_count,
            mod_overlay_paths,
            currently_open,
        );
        let diag = Diagnostic {
            range: key.1,
            severity: Some(severity),
            code,
            code_description: None,
            source: Some("xs-language-server".to_string()),
            message: format!("{}{}", key.3, suffix),
            related_information: None,
            tags: None,
            data: None,
        };
        out.entry(key.0).or_default().push(diag);
    }
    out
}

impl PerRootDiagnostic {
    /// Extract a [`PerRootDiagnostic`] from a published [`Diagnostic`], with
    /// any prior root-suffix stripped from the message.
    pub fn from_diagnostic(
        diag: &Diagnostic,
        uri: Uri,
        category: DiagnosticCategory,
        producing_root: PathBuf,
    ) -> Self {
        Self {
            uri,
            range: diag.range,
            severity: diag.severity.unwrap_or(DiagnosticSeverity::ERROR),
            code: diag.code.clone(),
            category,
            base_message: strip_root_suffix(&diag.message).to_string(),
            producing_root,
        }
    }
}

#[cfg(test)]
mod tests {
    use tower_lsp_server::ls_types::{Diagnostic, DiagnosticSeverity, Range};

    use super::{DiagnosticCategory, categorize};

    fn diag(message: &str) -> Diagnostic {
        Diagnostic {
            range: Range::default(),
            severity: None,
            code: None,
            code_description: None,
            source: None,
            message: message.to_string(),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    #[test]
    fn test_diagnostic_category_assigns_extern_collision() {
        assert_eq!(
            categorize(&diag("duplicate extern: 'gFoo' is declared extern in a.xs and b.xs")),
            DiagnosticCategory::ExternCollision
        );
        assert_eq!(
            categorize(&diag("extern collision: 'gFoo' is declared extern in a.xs, also defined in b.xs")),
            DiagnosticCategory::ExternCollision
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_unresolved_symbol() {
        assert_eq!(
            categorize(&diag("Error 0310: invalid symbol lookup 'foo' at line 1")),
            DiagnosticCategory::UnresolvedSymbol
        );
        assert_eq!(categorize(&diag("unresolved symbol 'bar' at line 2")), DiagnosticCategory::UnresolvedSymbol);
    }

    #[test]
    fn test_diagnostic_category_assigns_wrong_arg_count() {
        assert_eq!(
            categorize(&diag("expected 4 argument(s) to `aiPlanCreate`, got 2")),
            DiagnosticCategory::WrongArgCount
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_wrong_arg_type() {
        assert_eq!(
            categorize(&diag("expected argument 1 of type `int` for `aiPlanCreate`, got `float`")),
            DiagnosticCategory::WrongArgType
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_definition_error() {
        assert_eq!(
            categorize(&diag("non-ref parameter `x` must have a default value")),
            DiagnosticCategory::DefinitionError
        );
        assert_eq!(
            categorize(&diag("ref parameter `x` cannot have a default value")),
            DiagnosticCategory::DefinitionError
        );
        assert_eq!(categorize(&diag("variable `x` must be initialized")), DiagnosticCategory::DefinitionError);
        assert_eq!(
            categorize(&diag("constant `x` must be assigned a constant expression")),
            DiagnosticCategory::DefinitionError
        );
    }

    #[test]
    fn test_diagnostic_category_assigns_other() {
        assert_eq!(
            categorize(&diag("mutable function 'foo' redefined with different signature")),
            DiagnosticCategory::Other
        );
    }

    #[test]
    fn collect_diagnostics_maps_parse_severity() {
        // `void f(` is missing the parameter list / closing brace, so the
        // typed parser emits at least one ERROR-severity diagnostic.
        let src = "void f(\n";
        let diags = super::collect_diagnostics(src);
        assert!(
            diags.iter().any(|d| matches!(d.severity, Some(DiagnosticSeverity::ERROR))),
            "expected an ERROR diagnostic for malformed source, got: {:?}",
            diags
        );
    }

    #[test]
    fn collect_diagnostics_is_empty_for_valid_source() {
        let src = "void f() {}\n";
        let diags = super::collect_diagnostics(src);
        assert!(diags.is_empty(), "expected no diagnostics for valid source, got: {:?}", diags);
    }

    #[test]
    fn collect_diagnostics_does_not_leak_internal_marker() {
        let src = "void f(\n";
        let diags = super::collect_diagnostics(src);
        for d in &diags {
            assert!(
                !d.message.contains("MISSING") && !d.message.contains("ERROR") && !d.message.contains("node"),
                "message {:?} leaks parser internals",
                d.message
            );
        }
    }

    #[test]
    fn class_typed_local_parses_without_diagnostic() {
        let mut types = xs_parser::ast::TypeTable::with_primitives();
        types.insert_class("Foo");
        let diags = super::collect_diagnostics_with_types("void bar() { Foo f; }\n", types);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for known class-typed local, got: {:?}",
            diags
        );
    }

    #[test]
    fn class_typed_local_init_parses_without_diagnostic() {
        let mut types = xs_parser::ast::TypeTable::with_primitives();
        types.insert_class("Foo");
        let diags = super::collect_diagnostics_with_types("void bar() { Foo f = -1; }\n", types);
        assert!(
            diags.is_empty(),
            "expected no diagnostics for known class-typed local with init, got: {:?}",
            diags
        );
    }

    #[test]
    fn unknown_class_typed_local_still_emits_error() {
        let diags = super::collect_diagnostics("void bar() { Foo f; }\n");
        assert!(
            diags.iter().any(|d| matches!(d.severity, Some(DiagnosticSeverity::ERROR))),
            "expected ERROR diagnostic for unknown class-typed local, got: {:?}",
            diags
        );
    }

    #[test]
    fn class_name_does_not_override_primitive() {
        let mut types = xs_parser::ast::TypeTable::with_primitives();
        types.insert_class("int");
        let diags = super::collect_diagnostics_with_types("void bar() { int x = 0; }\n", types);
        assert!(
            diags.is_empty(),
            "primitive 'int' should still parse when a class of the same name is inserted, got: {:?}",
            diags
        );
    }
}
