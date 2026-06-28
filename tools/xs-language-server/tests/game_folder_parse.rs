//! Integration tests that exercise the LSP against the full AoM:R game folder.
//!
//! The tests read the game folder path from the `AOMR_GAME_PATH` environment
//! variable (the same key `main.rs` honours on startup) and walk every `.xs`
//! file under `<AOMR_GAME_PATH>/game/`. They are designed to skip cleanly when
//! `AOMR_GAME_PATH` is not set, so a bare `cargo test` on CI without the game
//! folder still passes — only the existing unit tests run.
//!
//! Run them locally with:
//!
//! ```text
//! AOMR_GAME_PATH=/path/to/Age\ of\ Mythology\ Retold \
//!     cargo test --manifest-path tools/xs-language-server/Cargo.toml \
//!         --test game_folder_parse -- --nocapture
//! ```

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use xs_language_server::diagnostics::{collect_all, DiagnosticCategory};
use xs_language_server::engine_api::EngineApi;
use xs_language_server::merged_view::MergedView;
use xs_language_server::parser;
use xs_language_server::semantic::{VirtualProject as SemProject};
use xs_language_server::symbols;
use xs_language_server::workspace::{VirtualProject, Workspace};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Resolve the `game/` subfolder from `AOMR_GAME_PATH`. Returns `None` if the
/// env var is unset or the expected layout is missing — every test below
/// treats `None` as a skip rather than a failure.
fn resolve_game_dir() -> Option<PathBuf> {
    let raw = std::env::var_os("AOMR_GAME_PATH")?;
    let game_path = PathBuf::from(raw);
    let game_dir = game_path.join("game");
    if !game_dir.is_dir() {
        eprintln!("{game_dir:?} is not a directory, skipping");
        return None;
    }
    Some(game_dir)
}

/// The result of walking `game_dir`: every parseable `.xs` file plus a count
/// of `.xs` files that were skipped because they are not valid UTF-8. The
/// shipped AoM:R game stores some `.xs` files as binary serialised data
/// (under `random_maps/`); these are not parseable as XS source and must be
/// skipped rather than treated as failures.
struct GameFiles {
    text: Vec<PathBuf>,
    binary_skipped: usize,
}

/// Recursively walk `game_dir`, returning every `.xs` file whose first few KB
/// are valid UTF-8. The list is sorted by path for deterministic output.
fn collect_xs_files(game_dir: &Path) -> GameFiles {
    let mut text: Vec<PathBuf> = Vec::new();
    let mut binary_skipped: usize = 0;
    for entry in walkdir::WalkDir::new(game_dir)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| entry.path().extension().and_then(|s| s.to_str()) == Some("xs"))
    {
        let path = entry.path().to_path_buf();
        match read_utf8_head(&path) {
            Ok(_) => text.push(path),
            Err(_) => binary_skipped += 1,
        }
    }
    text.sort();
    GameFiles {
        text,
        binary_skipped,
    }
}

/// Probe the first 64 KiB of `path` as UTF-8. We don't load the whole file
/// up front — `game/random_maps/` contains ~20 binary files with `.xs`
/// extensions (AoM:R serialised random-map data), and a full read of each
/// is wasteful. 64 KiB is enough to catch a malformed BOM or invalid byte
/// sequence in any real-world XS source.
fn read_utf8_head(path: &Path) -> std::io::Result<()> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut buf = [0u8; 65536];
    let n = file.read(&mut buf)?;
    std::str::from_utf8(&buf[..n])
        .map(|_| ())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

/// Path of `file` relative to `game_dir`, with POSIX separators regardless of
/// host platform. Returns `None` if `file` is not actually under `game_dir`.
fn relativize_to_game(game_dir: &Path, file: &Path) -> Option<String> {
    let rel = file.strip_prefix(game_dir).ok()?;
    let rel_str = rel.to_string_lossy();
    let normalized = if std::path::MAIN_SEPARATOR == '/' {
        rel_str.into_owned()
    } else {
        rel_str.replace(std::path::MAIN_SEPARATOR, "/")
    };
    Some(normalized)
}

/// Return the set of `.xs` files that appear as the target of an
/// `include "..."` directive anywhere in `files`.
///
/// Included files are pasted into their includers and are therefore not
/// meaningful to analyze in isolation. Limiting the semantic pipeline to
/// non-included (top-level) files avoids a flood of false-positive
/// "unresolved" diagnostics for symbols that are visible in every valid
/// includer.
fn collect_included_files(game_dir: &Path, files: &[PathBuf]) -> HashSet<PathBuf> {
    let mut targets = HashSet::new();
    for path in files {
        let Some(from_rel) = relativize_to_game(game_dir, path) else {
            continue;
        };
        let prefix = if from_rel.starts_with("ai/") {
            "ai/"
        } else if from_rel.starts_with("data/trigger/") {
            "data/trigger/"
        } else if from_rel.starts_with("random_maps/") {
            "random_maps/"
        } else {
            continue;
        };

        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        for line in source.lines() {
            let line = line.trim();
            if !line.starts_with("include") {
                continue;
            }
            let Some(start) = line.find('"') else { continue };
            let rest = &line[start + 1..];
            let Some(end) = rest.find('"') else { continue };
            let mut target = rest[..end].replace('\\', "/");
            if !target.starts_with(prefix) {
                target = format!("{prefix}{target}");
            }
            targets.insert(game_dir.join(target));
        }
    }
    targets
}

/// Count tree-sitter `ERROR` nodes under `root` that are NOT recoverable
/// forward declarations. The LSP's `symbols.rs` already recovers forward
/// function declarations (`void bar(int x = -1);`) by walking ERROR nodes
/// that satisfy the criteria below; everything else is a genuine parse
/// problem worth flagging.
fn count_unexpected_errors(root: tree_sitter::Node<'_>) -> usize {
    let mut count = 0usize;
    walk_errors(root, &mut count);
    count
}

fn walk_errors(node: tree_sitter::Node<'_>, count: &mut usize) {
    if node.is_error() && !is_recoverable_forward_decl(node) {
        *count += 1;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        walk_errors(child, count);
    }
}

/// Mirror of `symbols::extract_error_forward_declaration`'s eligibility check.
/// A recoverable forward declaration must be a top-level ERROR node that
/// carries a primitive/array type, an identifier, a parameter list, and no
/// function body.
fn is_recoverable_forward_decl(node: tree_sitter::Node<'_>) -> bool {
    if node.kind() != "ERROR" {
        return false;
    }
    let Some(parent) = node.parent() else {
        return false;
    };
    if parent.kind() != "translation_unit" {
        return false;
    }
    let mut cursor = node.walk();
    let mut has_type = false;
    let mut has_identifier = false;
    let mut has_param_list = false;
    let mut has_compound = false;
    for child in node.named_children(&mut cursor) {
        match child.kind() {
            "primitive_type" | "array_type" => has_type = true,
            "identifier" => has_identifier = true,
            "parameter_list" => has_param_list = true,
            "compound_statement" => has_compound = true,
            _ => {}
        }
    }
    has_type && has_identifier && has_param_list && !has_compound
}

// ---------------------------------------------------------------------------
// Test 1: parse every file, build a symbol table, assert no unexpected errors.
// ---------------------------------------------------------------------------

#[test]
fn parse_every_game_folder_file_completes_without_unexpected_errors() {
    let Some(game_dir) = resolve_game_dir() else {
        eprintln!("AOMR_GAME_PATH not set, skipping game folder parse test");
        return;
    };
    let GameFiles { text: files, binary_skipped } = collect_xs_files(&game_dir);
    assert!(!files.is_empty(), "no parseable .xs files found under {game_dir:?}");

    let mut total_symbols = 0usize;
    let mut total_unexpected_errors = 0usize;
    let mut error_files: Vec<(PathBuf, usize)> = Vec::new();

    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => panic!("failed to read {path:?}: {e}"),
        };
        let tree = match parser::parse(&source) {
            Some(t) => t,
            None => panic!("parser returned None for {path:?}"),
        };

        let unexpected = count_unexpected_errors(tree.root_node());
        if unexpected > 0 {
            error_files.push((path.clone(), unexpected));
        }
        total_unexpected_errors += unexpected;

        let table = symbols::build_symbol_table(&tree, &source);
        total_symbols += table.symbols.len();
    }

    println!(
        "Parsed {} files ({} binary .xs skipped), {} symbols, {} unexpected errors",
        files.len(),
        binary_skipped,
        total_symbols,
        total_unexpected_errors
    );
    if !error_files.is_empty() {
        for (path, n) in error_files.iter().take(10) {
            println!("  {n} unexpected ERROR(s) in {path:?}");
        }
        if error_files.len() > 10 {
            println!("  ... and {} more files with errors", error_files.len() - 10);
        }
    }

    assert!(!files.is_empty(), "should have parsed at least one file");
    // The shipped AoM:R game contains well over 1000 top-level symbols across
    // rules, functions, variables and constants — this is a smoke test for
    // "the LSP actually saw something in the file".
    assert!(
        total_symbols > 1000,
        "expected > 1000 symbols across the game folder, got {total_symbols}"
    );

    // Grammar-coverage note: the tree-sitter XS grammar still does not model
    // every construct the engine accepts (e.g. `class Foo { int[] bar; }`
    // member declarations, `new ClassName(...)` instantiation, `#if`/`#define`
    // preprocessor directives). Those surfaces surface as `ERROR` nodes that
    // `symbols.rs` cannot recover as forward declarations. Empirically the
    // game folder produces roughly 1500 such errors across ~300 parseable
    // files. We assert two regression guards:
    //
    //   1. The total stays below 3000 (≈ 2x headroom over today's 1571).
    //   2. No single file exceeds 100 errors — the worst offender today is
    //      `norse_classical.xs` at 72 (large AI strategy files use many
    //      class member declarations), so 100 is generous but a real
    //      grammar regression (say, dropping the `array_type` rule entirely)
    //      would push a file well past that ceiling.
    let total_threshold = 3000usize;
    let per_file_threshold = 100usize;
    assert!(
        total_unexpected_errors <= total_threshold,
        "got {} unexpected parse errors (threshold {total_threshold}). \
         First offenders: {:?}",
        total_unexpected_errors,
        error_files.iter().take(5).collect::<Vec<_>>()
    );
    let outliers: Vec<_> = error_files
        .iter()
        .filter(|(_, n)| *n > per_file_threshold)
        .collect();
    assert!(
        outliers.is_empty(),
        "files with more than {per_file_threshold} unexpected errors: {outliers:?}"
    );
}

// ---------------------------------------------------------------------------
// Test 2: every game-folder file resolves through the workspace.
// ---------------------------------------------------------------------------

#[test]
fn every_game_folder_file_resolves_in_workspace() {
    let Some(game_dir) = resolve_game_dir() else {
        eprintln!("AOMR_GAME_PATH not set, skipping workspace resolution test");
        return;
    };
    let GameFiles { text: files, binary_skipped } = collect_xs_files(&game_dir);
    assert!(!files.is_empty(), "no parseable .xs files found under {game_dir:?}");

    // The workspace is rooted at the install dir (the parent of `game/`),
    // matching the constructor signature in `Workspace::new`.
    let game_root = game_dir
        .parent()
        .expect("game/ has a parent directory")
        .to_path_buf();
    let workspace = Workspace::new(game_root);
    let project = VirtualProject::default();

    let mut seen_rels: HashSet<String> = HashSet::new();

    for path in &files {
        let rel = relativize_to_game(&game_dir, path)
            .unwrap_or_else(|| panic!("file {path:?} is not under {game_dir:?}"));
        seen_rels.insert(rel.clone());

        // The workspace must know about this file. Either it sits directly
        // in the game folder (no mod overlay), or — if a mod shadowed it —
        // a lookup via `game_file_path` still returns the vanilla path.
        let resolved = workspace
            .resolve_file(&project, &rel)
            .unwrap_or_else(|| panic!("workspace could not resolve {rel}"));
        assert!(
            resolved.exists(),
            "workspace resolved {rel} to {resolved:?}, which does not exist"
        );
    }

    println!(
        "Workspace resolved {} relative paths across {} parseable .xs files ({} binary .xs skipped)",
        seen_rels.len(),
        files.len(),
        binary_skipped
    );

    assert_eq!(
        seen_rels.len(),
        files.len(),
        "every walked file should produce a unique relative path"
    );
    // The shipped AoM:R game ships 322 .xs files total — 302 parseable text
    // files plus ~20 binary serialised entries under `random_maps/` that we
    // skip. We are content with "the workspace can find them all" and only
    // check that we got well over half the total.
    assert!(
        files.len() >= 250,
        "expected >= 250 parseable .xs files, found {}",
        files.len()
    );
}

// ---------------------------------------------------------------------------
// Test 3: full semantic pipeline emits a bounded number of unresolved-symbol
// diagnostics.
//
// KNOWN LIMITATION: the LSP's include-paste implementation is approximated
// (per-line regex + path matching; see `docs/blockers/pending-task-include-paste.md`).
// That means calls to functions defined in files reached only through an
// `include "..."` chain surface as `Error 0310: invalid symbol lookup` even
// though the engine resolves them at runtime. The biggest single offender is
// the AI build-order framework: `boVillager`, `boBuild`, `boExecute`,
// `boIncreaseTimeout`, `boConditionalWait`, etc. are defined in
// `game/ai/core/bo_system/bo_system.xs` and called from every strategy file
// under `game/ai/core/<civ>/`.
//
// We assert a regression guard rather than a strict zero: the unresolved
// count must stay below the empirically measured baseline (~5335 on the
// current AoM:R release) plus 10% headroom. When the include-paste fix lands
// this threshold can be tightened; eventually the assertion should be
// `unresolved == 0`.
// ---------------------------------------------------------------------------

/// Top-10 `bo_*` AI build-order callees used as regression guards. PR 1 made
/// these parse as `Function` symbols; PR 3/4 verify that they resolve through
/// the include-paste scope with zero unresolved-symbol diagnostics.
const TOP_BO_CALLEES: &[&str] = &[
    "boBuild",
    "boVillager",
    "boUnit",
    "boConditionalWait",
    "boAdvance",
    "boIncreaseTimeout",
    "boTransaction",
    "boEnd",
    "boExecute",
    "boTech",
];

/// Result of the full diagnostic pipeline over every top-level game folder
/// file, cached so the per-category assertions can share the expensive scan.
#[derive(Debug, Clone)]
struct DiagnosticReport {
    duplicate_extern_count: usize,
    wrong_diagnostic_uri_count: usize,
    unresolved_symbol_count: usize,
    wrong_arg_count_count: usize,
    rule_call_unresolved_count: usize,
    /// Sum of all non-"Other" diagnostics returned by the pipeline across
    /// the analyzed files (used as a catch-all gate).
    total_diagnostic_count: usize,
    /// Counts only for the top `bo_*` callees.
    callee_counts: HashMap<String, usize>,
    /// Representative failures per category.
    examples: HashMap<DiagnosticCategory, Vec<String>>,
}

fn analyze_top_level_diagnostics() -> Option<DiagnosticReport> {
    use std::sync::OnceLock;
    static CACHE: OnceLock<Option<DiagnosticReport>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            let game_dir = resolve_game_dir()?;
            let GameFiles { text: files, binary_skipped } = collect_xs_files(&game_dir);
            assert!(!files.is_empty(), "no parseable .xs files found under {game_dir:?}");

            // Analyze only top-level files: files that are included by another file
            // are pasted into their includer's scope, so checking them in isolation
            // would produce spurious unresolved-symbol diagnostics.
            //
            // Random-map scripts are also excluded: they rely heavily on RM-specific
            // engine functions that are absent from the Doxygen archive, so the
            // workspace-only semantic checker cannot meaningfully judge them.
            let included = collect_included_files(&game_dir, &files);
            let top_level_files: Vec<PathBuf> = files
                .iter()
                .filter(|p| {
                    let Some(rel) = relativize_to_game(&game_dir, p) else {
                        return false;
                    };
                    !included.contains(*p) && !rel.starts_with("random_maps/")
                })
                .cloned()
                .collect();

            let engine_api = load_engine_api(&game_dir);

            // Build the semantic project from all game-folder files in one go. This
            // re-parses every file (the parser is cheap enough) and produces a
            // virtual project that cross-file semantic checks can iterate.
            let mut sources: HashMap<PathBuf, String> = HashMap::new();
            for path in &files {
                let source = std::fs::read_to_string(path)
                    .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
                sources.insert(path.clone(), source);
            }
            let project = SemProject::from_files(sources);

            let mut rule_names = HashSet::new();
            for file in project.files.values() {
                for sym in &file.table.symbols {
                    if sym.kind == xs_language_server::symbols::SymbolKind::Rule {
                        rule_names.insert(sym.name.clone());
                    }
                }
            }

            let game_root = game_dir.parent()?.to_path_buf();
            let workspace = Workspace::new(game_root);
            let ws_project = VirtualProject::default();
            let cache_dir = xs_language_server::cache::state_cache_dir();

            let mut duplicate_extern = 0usize;
            let mut wrong_uri = 0usize;
            let mut unresolved_symbol = 0usize;
            let mut wrong_arg_count = 0usize;
            let mut rule_call_unresolved = 0usize;
            let mut total = 0usize;
            let mut callee_counts: HashMap<String, usize> = TOP_BO_CALLEES
                .iter()
                .map(|c| (c.to_string(), 0))
                .collect();
            let mut examples: HashMap<DiagnosticCategory, Vec<String>> = HashMap::new();
            let mut record_example = |cat, path: &Path, message: &str| {
                let entry = examples.entry(cat).or_default();
                if entry.len() < 5 {
                    entry.push(format!("{path:?}: {message}"));
                }
            };

            for path in &top_level_files {
                let source = std::fs::read_to_string(path)
                    .unwrap_or_else(|e| panic!("failed to read {path:?}: {e}"));
                let tree = parser::parse(&source)
                    .unwrap_or_else(|| panic!("parser returned None for {path:?}"));
                let table = symbols::build_symbol_table(&tree, &source);
                let merged =
                    MergedView::build(path, &source, &table, &workspace, &ws_project, &cache_dir);
                let diags = collect_all(
                    &tree,
                    &source,
                    &engine_api,
                    &table,
                    Some(&project),
                    Some(path),
                    Some(&merged),
                );

                let line_count = source.lines().count() as u32;

                for d in diags {
                    let cat = xs_language_server::diagnostics::categorize(&d);
                    match cat {
                        DiagnosticCategory::ExternCollision => {
                            duplicate_extern += 1;
                            record_example(cat, path, &d.message);
                        }
                        DiagnosticCategory::WrongArgCount => {
                            wrong_arg_count += 1;
                            record_example(cat, path, &d.message);
                        }
                        DiagnosticCategory::UnresolvedSymbol => {
                            unresolved_symbol += 1;
                            if let Some(callee) = extract_callee_from_diagnostic(&d.message) {
                                if rule_names.contains(&callee) {
                                    rule_call_unresolved += 1;
                                }
                                if let Some(count) = callee_counts.get_mut(&callee) {
                                    *count += 1;
                                }
                            }
                            record_example(cat, path, &d.message);
                        }
                        _ => {}
                    }

                    if d.range.start.line > d.range.end.line
                        || d.range.start.line >= line_count
                        || d.range.end.line > line_count
                    {
                        wrong_uri += 1;
                        record_example(
                            DiagnosticCategory::WrongRangeUri,
                            path,
                            &format!("{} (range {:?} outside {} lines)", d.message, d.range, line_count),
                        );
                    }

                    if !matches!(cat, DiagnosticCategory::Other) {
                        total += 1;
                    }
                }
            }

            println!(
                "Diagnostic pipeline across {} top-level files ({} binary .xs skipped): \
                 duplicate_extern={}, wrong_uri={}, unresolved_symbol={}, \
                 wrong_arg_count={}, rule_call_unresolved={}, total={}",
                top_level_files.len(),
                binary_skipped,
                duplicate_extern,
                wrong_uri,
                unresolved_symbol,
                wrong_arg_count,
                rule_call_unresolved,
                total,
            );
            if !examples.is_empty() {
                for (cat, exs) in &examples {
                    println!("  {cat:?} examples:");
                    for ex in exs {
                        println!("    {ex}");
                    }
                }
            }

            Some(DiagnosticReport {
                duplicate_extern_count: duplicate_extern,
                wrong_diagnostic_uri_count: wrong_uri,
                unresolved_symbol_count: unresolved_symbol,
                wrong_arg_count_count: wrong_arg_count,
                rule_call_unresolved_count: rule_call_unresolved,
                total_diagnostic_count: total,
                callee_counts,
                examples,
            })
        })
        .clone()
}

#[test]
fn test_game_folder_duplicate_extern_count_is_zero() {
    let Some(report) = analyze_top_level_diagnostics() else {
        eprintln!("AOMR_GAME_PATH not set, skipping duplicate-extern test");
        return;
    };
    assert!(
        report.duplicate_extern_count == 0,
        "found {} duplicate-extern diagnostic(s). Examples: {:?}",
        report.duplicate_extern_count,
        report.examples.get(&DiagnosticCategory::ExternCollision),
    );
}

#[test]
fn test_game_folder_wrong_diagnostic_uri_count_is_zero() {
    let Some(report) = analyze_top_level_diagnostics() else {
        eprintln!("AOMR_GAME_PATH not set, skipping wrong-URI test");
        return;
    };
    assert!(
        report.wrong_diagnostic_uri_count == 0,
        "found {} diagnostic(s) with a URI/range mismatch. Examples: {:?}",
        report.wrong_diagnostic_uri_count,
        report.examples.get(&DiagnosticCategory::WrongRangeUri),
    );
}

#[test]
fn test_game_folder_unresolved_symbol_count_is_zero() {
    let Some(report) = analyze_top_level_diagnostics() else {
        eprintln!("AOMR_GAME_PATH not set, skipping unresolved-symbol test");
        return;
    };
    assert!(
        report.unresolved_symbol_count == 0,
        "found {} unresolved-symbol diagnostic(s). Examples: {:?}",
        report.unresolved_symbol_count,
        report.examples.get(&DiagnosticCategory::UnresolvedSymbol),
    );
}

#[test]
fn test_game_folder_wrong_arg_count_count_is_zero() {
    let Some(report) = analyze_top_level_diagnostics() else {
        eprintln!("AOMR_GAME_PATH not set, skipping wrong-arg-count test");
        return;
    };
    assert!(
        report.wrong_arg_count_count == 0,
        "found {} wrong-arg-count diagnostic(s). Examples: {:?}",
        report.wrong_arg_count_count,
        report.examples.get(&DiagnosticCategory::WrongArgCount),
    );
}

#[test]
fn test_game_folder_rule_call_unresolved_count_is_zero() {
    let Some(report) = analyze_top_level_diagnostics() else {
        eprintln!("AOMR_GAME_PATH not set, skipping rule-call-unresolved test");
        return;
    };
    assert!(
        report.rule_call_unresolved_count == 0,
        "found {} unresolved rule-call diagnostic(s). Examples: {:?}",
        report.rule_call_unresolved_count,
        report.examples.get(&DiagnosticCategory::UnresolvedSymbol),
    );
}

#[test]
fn test_game_folder_total_diagnostic_count_is_zero() {
    let Some(report) = analyze_top_level_diagnostics() else {
        eprintln!("AOMR_GAME_PATH not set, skipping total-diagnostic test");
        return;
    };
    assert!(
        report.total_diagnostic_count == 0,
        "found {} total non-parse diagnostic(s). Examples: {:?}",
        report.total_diagnostic_count,
        report.examples,
    );
}

#[test]
fn top_bo_callees_have_zero_unresolved_calls() {
    let Some(report) = analyze_top_level_diagnostics() else {
        eprintln!("AOMR_GAME_PATH not set, skipping top-bo-callee guard test");
        return;
    };

    let mut all_zero = true;
    for callee in TOP_BO_CALLEES {
        let count = report.callee_counts.get(*callee).copied().unwrap_or(0);
        if count == 0 {
            println!("PASS: {} has zero unresolved-symbol diagnostics", callee);
        } else {
            println!(
                "FAIL: {} has {} unresolved-symbol diagnostic(s)",
                callee, count
            );
            all_zero = false;
        }
    }
    assert!(
        all_zero,
        "one or more top bo_* callees have unresolved-symbol diagnostics"
    );
}

/// Load the engine API for the doxygen archive that ships next to `game/`.
fn load_engine_api(game_dir: &Path) -> EngineApi {
    let install_root = game_dir
        .parent()
        .expect("game/ directory must have a parent install root");
    let archive = install_root.join("doxygen_retail.7z");
    let cache_dir = xs_language_server::cache::state_cache_dir();
    EngineApi::load_from_archive(&archive, &cache_dir)
        .unwrap_or_else(|e| panic!("could not load engine API from {archive:?}: {e}"))
}

/// Pull the callee name out of an `Error 0310: invalid symbol lookup 'X' at
/// line N` message. Returns `None` for malformed messages so the caller can
/// decide whether to count them.
fn extract_callee_from_diagnostic(message: &str) -> Option<String> {
    let after = message.split("invalid symbol lookup '").nth(1)?;
    let name = after.split('\'').next()?;
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}
