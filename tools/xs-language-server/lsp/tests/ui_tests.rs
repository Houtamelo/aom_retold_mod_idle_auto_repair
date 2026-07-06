//! rustc-style UI test harness for xs-parser diagnostics.
//!
//! Run as `cargo test --manifest-path tools/xs-language-server/Cargo.toml
//! --test ui_tests [-- --bless]`.
//!
//! The harness discovers `.xs` files under `lsp/tests/ui/`, parses
//! `//@ check-pass`/`//@ check-fail` directives, extracts inline
//! `//~ ERROR <message>` markers, and compares rendered parser diagnostics
//! against sibling `.err` golden files.  With `--bless`, golden files are
//! (re)generated.
//!
//! Because this is a `harness = false` test target, libtest discovery is
//! disabled.  Auxiliary unit-style checks for the harness's own helpers are
//! executed through the `--self-tests` CLI argument.

use std::ffi::OsStr;
use std::fmt::{self, Display};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use codespan_reporting::diagnostic::{LabelStyle, Severity};
use codespan_reporting::files::SimpleFile;
use codespan_reporting::term::termcolor::NoColor;
use codespan_reporting::term::{self, Config};
use walkdir::WalkDir;

use xs_language_server::range::span_to_range;
use xs_language_server::symbols::extract_class_names;
use xs_parser::ast::type_table::TypeTable;
use xs_parser::parser::{compact_recovery_messages, Diagnostic, Parser};

// --------------------------------------------------------------------------
// Data structures
// --------------------------------------------------------------------------

/// A discovered UI test file with its parsed metadata.
#[derive(Debug, Clone)]
struct UiTest {
    /// Absolute path to the `.xs` source.
    path: PathBuf,
    /// Stable relative path used as the frozen file label (`ui/...`).
    relative: String,
    /// Raw source text.
    source: String,
    /// Parsed directive.
    directive: Directive,
    /// Inline `//~ ERROR` markers.
    markers: Vec<Marker>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Directive {
    CheckPass,
    CheckFail,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Marker {
    /// 1-based physical line.
    line: u32,
    /// Exact expected message text.
    message: String,
}

/// A parser diagnostic anchored to a 1-based line for matching.
#[derive(Debug, Clone)]
struct UiDiagnostic {
    line: u32,
    message: String,
    raw: Diagnostic,
}

/// Reasons a UI test can fail.
#[derive(Debug, Clone)]
enum TestFailure {
    BadDirective(String),
    MissingDirective,
    ConflictingDirectives,
    EmptyMarker { line: u32 },
    CheckFailNeedsMarker,
    UnexpectedDiagnostic { line: u32, message: String },
    MissingDiagnostic { line: u32, expected: String },
    SpanMismatch { marker_line: u32, diagnostic_line: u32 },
    MissingErr { relative: String },
    ErrMismatch { relative: String, diff: String },
    ErrPresentForCheckPass { relative: String },
    IoError(String),
}

impl Display for TestFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestFailure::BadDirective(msg) => write!(f, "{}", msg),
            TestFailure::MissingDirective => {
                write!(f, "missing //@ check-pass or //@ check-fail directive at top of file")
            }
            TestFailure::ConflictingDirectives => write!(f, "conflicting directives"),
            TestFailure::EmptyMarker { line } => {
                write!(f, "empty //~ ERROR marker on line {}", line)
            }
            TestFailure::CheckFailNeedsMarker => {
                write!(f, "check-fail requires at least one //~ ERROR marker")
            }
            TestFailure::UnexpectedDiagnostic { line, message } => {
                write!(f, "unexpected error on line {}: {}", line, message)
            }
            TestFailure::MissingDiagnostic { line, expected } => {
                write!(f, "expected error matching \"{}\" on line {}, found none", expected, line)
            }
            TestFailure::SpanMismatch {
                marker_line,
                diagnostic_line,
            } => {
                write!(
                    f,
                    "marker on line {} bound to diagnostic on line {} (expected {})",
                    marker_line, diagnostic_line, marker_line
                )
            }
            TestFailure::MissingErr { relative } => {
                write!(f, "missing {}.err — run with --bless to generate", relative)
            }
            TestFailure::ErrMismatch { relative, diff } => {
                write!(f, "{}.err mismatch:\n{}", relative, diff)
            }
            TestFailure::ErrPresentForCheckPass { relative } => {
                write!(f, "{}.err present for //@ check-pass test", relative)
            }
            TestFailure::IoError(msg) => write!(f, "io error: {}", msg),
        }
    }
}

// --------------------------------------------------------------------------
// Entry point
// --------------------------------------------------------------------------

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--self-tests") {
        let ok = run_self_tests();
        process::exit(if ok { 0 } else { 1 });
    }

    let bless = args.iter().any(|a| a == "--bless")
        || std::env::var("BLESS").as_deref() == Ok("1");

    let ui_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("ui");
    let tests = discover_tests(&ui_root);

    let mut failures = 0usize;
    for path in &tests {
        match load_test(path, &ui_root).and_then(|test| run_test(&test, bless)) {
            Ok(()) => println!("[PASS] {}", relative_path(path, &ui_root)),
            Err(err) => {
                failures += 1;
                println!(
                    "[FAIL] {}: {}",
                    relative_path(path, &ui_root),
                    err
                );
            }
        }
    }

    if failures > 0 {
        eprintln!("{} UI test(s) failed", failures);
        process::exit(1);
    }
}

// --------------------------------------------------------------------------
// Discovery
// --------------------------------------------------------------------------

fn discover_tests(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if path.extension() != Some(OsStr::new("xs")) {
            continue;
        }
        out.push(path.to_path_buf());
    }
    out
}

fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

// --------------------------------------------------------------------------
// Loading
// --------------------------------------------------------------------------

fn load_test(path: &Path, ui_root: &Path) -> Result<UiTest, TestFailure> {
    let relative = relative_path(path, ui_root);
    let source = read_source(path)?;
    let directive = parse_directive(&source)?;
    let markers = extract_markers(&source)?;
    Ok(UiTest {
        path: path.to_path_buf(),
        relative,
        source,
        directive,
        markers,
    })
}

fn read_source(path: &Path) -> Result<String, TestFailure> {
    fs::read_to_string(path).map_err(|e| TestFailure::IoError(format!("{}: {}", path.display(), e)))
}

// --------------------------------------------------------------------------
// Directives
// --------------------------------------------------------------------------

fn parse_directive(source: &str) -> Result<Directive, TestFailure> {
    let mut saw_check_pass = false;
    let mut saw_check_fail = false;

    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with("//") {
            // First non-comment source line reached.
            break;
        }
        let after_comment = &trimmed[2..];
        if !after_comment.starts_with('@') {
            continue;
        }
        match parse_directive_inner(after_comment) {
            Some(Directive::CheckPass) => saw_check_pass = true,
            Some(Directive::CheckFail) => saw_check_fail = true,
            None => {
                return Err(TestFailure::BadDirective(format!(
                    "unrecognized directive: {}",
                    line.trim()
                )));
            }
        }
    }

    if saw_check_pass && saw_check_fail {
        return Err(TestFailure::ConflictingDirectives);
    }
    if saw_check_pass {
        return Ok(Directive::CheckPass);
    }
    if saw_check_fail {
        return Ok(Directive::CheckFail);
    }
    Err(TestFailure::MissingDirective)
}

fn parse_directive_inner(text: &str) -> Option<Directive> {
    let body = text.strip_prefix('@')?;
    let body = body.split("//").next().unwrap_or(body);
    let body = body.trim();
    match body {
        "check-pass" => Some(Directive::CheckPass),
        "check-fail" => Some(Directive::CheckFail),
        _ => None,
    }
}

// --------------------------------------------------------------------------
// Markers
// --------------------------------------------------------------------------

fn extract_markers(source: &str) -> Result<Vec<Marker>, TestFailure> {
    let mut markers = Vec::new();
    for (idx, line) in source.lines().enumerate() {
        let line_no = (idx + 1) as u32;
        if let Some(pos) = line.find("//~ ERROR") {
            let after = &line[pos + "//~ ERROR".len()..];
            // Drop any trailing `//` comment after the marker text.
            let after = after.split("//").next().unwrap_or(after);
            let message = after.trim().to_string();
            if message.is_empty() {
                return Err(TestFailure::EmptyMarker { line: line_no });
            }
            markers.push(Marker { line: line_no, message });
        }
    }
    Ok(markers)
}

// --------------------------------------------------------------------------
// Parser invocation
// --------------------------------------------------------------------------

fn run_parser(source: &str) -> Vec<UiDiagnostic> {
    let mut diags: Vec<Diagnostic> = Vec::new();
    let mut type_table = TypeTable::with_primitives();
    for class in extract_class_names(source) {
        type_table.insert_class(&class);
    }
    let _cst = Parser::new_with_context(source, &mut diags, type_table).parse(&mut diags);
    compact_recovery_messages(source, &mut diags);

    let mut out: Vec<UiDiagnostic> = diags
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .map(|d| {
            let primary = d
                .labels
                .iter()
                .find(|l| l.style == LabelStyle::Primary)
                .cloned()
                .or_else(|| d.labels.first().cloned())
                .map(|l| l.range)
                .unwrap_or(0..0);
            let range = span_to_range(source, primary);
            let line = range.start.line + 1;
            let message = d.message.clone();
            UiDiagnostic {
                line,
                message,
                raw: d,
            }
        })
        .collect();

    out.sort_by_key(|d| (d.line, d.message.clone(), d.raw.labels.first().map(|l| l.range.start).unwrap_or(0)));
    out
}

// --------------------------------------------------------------------------
// Rendering
// --------------------------------------------------------------------------

fn render(test: &UiTest, diags: &[UiDiagnostic]) -> String {
    if diags.is_empty() {
        return String::new();
    }
    let mut sorted: Vec<&UiDiagnostic> = diags.iter().collect();
    sorted.sort_by_key(|d| {
        let start = d
            .raw
            .labels
            .iter()
            .find(|l| l.style == LabelStyle::Primary)
            .map(|l| l.range.start)
            .or_else(|| d.raw.labels.first().map(|l| l.range.start))
            .unwrap_or(0);
        (d.line, start, d.message.clone())
    });

    let file = SimpleFile::new(&test.relative, &test.source);
    let config = Config::default();
    let mut pieces = Vec::with_capacity(sorted.len());
    for ui_diag in sorted {
        let mut writer = NoColor::new(Vec::<u8>::new());
        term::emit_to_write_style(&mut writer, &config, &file, &ui_diag.raw).ok();
        pieces.push(String::from_utf8(writer.into_inner()).unwrap_or_default());
    }
    pieces.join("\n")
}

// --------------------------------------------------------------------------
// Validation
// --------------------------------------------------------------------------

fn validate(test: &UiTest, diags: &[UiDiagnostic]) -> Result<(), TestFailure> {
    match test.directive {
        Directive::CheckPass => {
            if let Some(first) = diags.first() {
                return Err(TestFailure::UnexpectedDiagnostic {
                    line: first.line,
                    message: first.message.clone(),
                });
            }
            Ok(())
        }
        Directive::CheckFail => {
            if test.markers.is_empty() {
                return Err(TestFailure::CheckFailNeedsMarker);
            }
            let mut unmatched_markers: Vec<&Marker> = test.markers.iter().collect();
            let mut unmatched_diags: Vec<&UiDiagnostic> = diags.iter().collect();

            // Exact (line + message) matches first.
            let mut i = 0;
            while i < unmatched_markers.len() {
                let marker = unmatched_markers[i];
                if let Some(pos) = unmatched_diags
                    .iter()
                    .position(|d| d.line == marker.line && d.message == marker.message)
                {
                    unmatched_markers.remove(i);
                    unmatched_diags.remove(pos);
                } else {
                    i += 1;
                }
            }

            // Prefer missing-diagnostic errors; they are what the test author
            // needs to act on first.
            if let Some(marker) = unmatched_markers.first() {
                return Err(TestFailure::MissingDiagnostic {
                    line: marker.line,
                    expected: marker.message.clone(),
                });
            }

            if let Some(diag) = unmatched_diags.first() {
                return Err(TestFailure::UnexpectedDiagnostic {
                    line: diag.line,
                    message: diag.message.clone(),
                });
            }

            Ok(())
        }
    }
}

// --------------------------------------------------------------------------
// Golden-file compare / bless
// --------------------------------------------------------------------------

fn compare_or_bless(test: &UiTest, rendered: &str, bless: bool) -> Result<(), TestFailure> {
    let err_path = test.path.with_extension("err");
    match test.directive {
        Directive::CheckPass => {
            if err_path.exists() {
                return Err(TestFailure::ErrPresentForCheckPass {
                    relative: test.relative.clone(),
                });
            }
            Ok(())
        }
        Directive::CheckFail => {
            if bless {
                if !rendered.is_empty() {
                    write_err_file(&err_path, rendered)?;
                }
                println!("{}.err blessed", test.relative);
                Ok(())
            } else if !err_path.exists() {
                Err(TestFailure::MissingErr {
                    relative: test.relative.clone(),
                })
            } else {
                let expected = read_source(&err_path)?;
                if expected == rendered {
                    Ok(())
                } else {
                    Err(TestFailure::ErrMismatch {
                        relative: test.relative.clone(),
                        diff: build_diff(&expected, rendered),
                    })
                }
            }
        }
    }
}

fn write_err_file(path: &Path, contents: &str) -> Result<(), TestFailure> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| TestFailure::IoError(e.to_string()))?;
    }
    let mut file = fs::File::create(path).map_err(|e| TestFailure::IoError(e.to_string()))?;
    file.write_all(contents.as_bytes())
        .map_err(|e| TestFailure::IoError(e.to_string()))
}

fn build_diff(expected: &str, actual: &str) -> String {
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let len = expected_lines.len().max(actual_lines.len());
    let mut diverged_at: Option<usize> = None;
    for i in 0..len {
        let e = expected_lines.get(i).unwrap_or(&"");
        let a = actual_lines.get(i).unwrap_or(&"");
        if e != a {
            diverged_at = Some(i);
            break;
        }
    }
    let mut out = String::new();
    if let Some(idx) = diverged_at {
        let start = idx.saturating_sub(5);
        let end = (idx + 6).min(len);
        for i in start..end {
            let e = expected_lines.get(i).unwrap_or(&"");
            let a = actual_lines.get(i).unwrap_or(&"");
            if i == idx {
                out.push_str(&format!("- {}\n", e));
                out.push_str(&format!("+ {}\n", a));
            } else {
                out.push_str(&format!("  {}\n", if i < expected_lines.len() { e } else { a }));
            }
        }
    } else if expected != actual {
        // Trailing newline difference.
        out.push_str("- (trailing newline)\n");
        out.push_str("+ (no trailing newline)\n");
    }
    out
}

// --------------------------------------------------------------------------
// Per-test runner
// --------------------------------------------------------------------------

fn run_test(test: &UiTest, bless: bool) -> Result<(), TestFailure> {
    let diags = run_parser(&test.source);
    validate(test, &diags)?;
    let rendered = render(test, &diags);
    compare_or_bless(test, &rendered, bless)
}

// --------------------------------------------------------------------------
// Self-tests
// --------------------------------------------------------------------------

fn run_self_tests() -> bool {
    let mut ok = true;

    let tests: Vec<(&str, fn() -> Result<(), String>)> = vec![
        ("parse_directive_check_pass", _self_test_parse_directive_check_pass),
        ("parse_directive_check_fail", _self_test_parse_directive_check_fail),
        ("parse_directive_missing", _self_test_parse_directive_missing),
        ("parse_directive_conflicting", _self_test_parse_directive_conflicting),
        ("parse_directive_whitespace", _self_test_parse_directive_whitespace),
        ("extract_markers_basic", _self_test_extract_markers_basic),
        ("extract_markers_empty", _self_test_extract_markers_empty),
        ("extract_markers_none", _self_test_extract_markers_none),
        ("extract_markers_trailing_comment", _self_test_extract_markers_trailing_comment),
        ("render_basic", _self_test_render_basic),
        ("render_determinism", _self_test_render_determinism),
        ("validate_check_pass_ok", _self_test_validate_check_pass_ok),
        ("validate_check_pass_fails", _self_test_validate_check_pass_fails),
        ("validate_check_fail_match", _self_test_validate_check_fail_match),
        ("validate_check_fail_wrong_line", _self_test_validate_check_fail_wrong_line),
        ("validate_check_fail_extra_diag", _self_test_validate_check_fail_extra_diag),
        ("validate_check_fail_no_markers", _self_test_validate_check_fail_no_markers),
        ("validate_error_substrings", _self_test_validate_error_substrings),
        ("run_parser_clean", _self_test_run_parser_clean),
        ("run_parser_missing_semi", _self_test_run_parser_missing_semi),
        ("bless_writes_err_file", _self_test_bless_writes_err_file),
        ("discover_tests_ignores_non_xs", _self_test_discover_tests_ignores_non_xs),
        ("directive_bad", _self_test_directive_bad),
        ("marker_leading_whitespace", _self_test_marker_leading_whitespace),
    ];

    for (name, test) in tests {
        match test() {
            Ok(()) => println!("[SELF-TEST PASS] {}", name),
            Err(msg) => {
                eprintln!("[SELF-TEST FAIL] {}: {}", name, msg);
                ok = false;
            }
        }
    }

    ok
}

fn _self_test_parse_directive_check_pass() -> Result<(), String> {
    let source = "//@ check-pass\nvoid f() {}\n";
    assert_eq!(parse_directive(source).map_err(|e| e.to_string())?, Directive::CheckPass);
    Ok(())
}

fn _self_test_parse_directive_check_fail() -> Result<(), String> {
    let source = "//@ check-fail\nvoid f() {}\n";
    assert_eq!(parse_directive(source).map_err(|e| e.to_string())?, Directive::CheckFail);
    Ok(())
}

fn _self_test_parse_directive_missing() -> Result<(), String> {
    let source = "void f() {}\n";
    match parse_directive(source) {
        Err(TestFailure::MissingDirective) => Ok(()),
        other => Err(format!("expected MissingDirective, got {:?}", other)),
    }
}

fn _self_test_parse_directive_conflicting() -> Result<(), String> {
    let source = "//@ check-pass\n//@ check-fail\nvoid f() {}\n";
    match parse_directive(source) {
        Err(TestFailure::ConflictingDirectives) => Ok(()),
        other => Err(format!("expected ConflictingDirectives, got {:?}", other)),
    }
}

fn _self_test_parse_directive_whitespace() -> Result<(), String> {
    let source = "   //@   check-pass  // foo\nvoid f() {}\n";
    assert_eq!(parse_directive(source).map_err(|e| e.to_string())?, Directive::CheckPass);
    Ok(())
}

fn _self_test_directive_bad() -> Result<(), String> {
    let source = "//@ bad-directive\nvoid f() {}\n";
    match parse_directive(source) {
        Err(TestFailure::BadDirective(_)) => Ok(()),
        other => Err(format!("expected BadDirective, got {:?}", other)),
    }
}

fn _self_test_extract_markers_basic() -> Result<(), String> {
    let source = "void f()\n{\n   return 5 //~ ERROR missing ';'\n}\n";
    let markers = extract_markers(source).map_err(|e| e.to_string())?;
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].line, 3);
    assert_eq!(markers[0].message, "missing ';'");
    Ok(())
}

fn _self_test_extract_markers_empty() -> Result<(), String> {
    let source = "//~ ERROR  \n";
    match extract_markers(source) {
        Err(TestFailure::EmptyMarker { line }) if line == 1 => Ok(()),
        other => Err(format!("expected EmptyMarker on line 1, got {:?}", other)),
    }
}

fn _self_test_extract_markers_none() -> Result<(), String> {
    let source = "void f() {}\n";
    let markers = extract_markers(source).map_err(|e| e.to_string())?;
    assert!(markers.is_empty());
    Ok(())
}

fn _self_test_extract_markers_trailing_comment() -> Result<(), String> {
    let source = "void f() { return 5; } //~ ERROR missing ';' // also\n";
    let markers = extract_markers(source).map_err(|e| e.to_string())?;
    assert_eq!(markers[0].message, "missing ';'");
    Ok(())
}

fn _self_test_marker_leading_whitespace() -> Result<(), String> {
    let source = "void f() { return 5; }   //~ ERROR missing ';'\n";
    let markers = extract_markers(source).map_err(|e| e.to_string())?;
    assert_eq!(markers.len(), 1);
    assert_eq!(markers[0].message, "missing ';'");
    Ok(())
}

fn _self_test_render_basic() -> Result<(), String> {
    let test = UiTest {
        path: PathBuf::from("harness_self/render_input.xs"),
        relative: "harness_self/render_input.xs".to_string(),
        source: "void f()\n{\n   return 5\n}\n".to_string(),
        directive: Directive::CheckFail,
        markers: vec![],
    };
    let diag = Diagnostic::error()
        .with_message("missing ';'")
        .with_label(codespan_reporting::diagnostic::Label::primary((), 20..28));
    let ui_diag = UiDiagnostic {
        line: 3,
        message: "missing ';'".to_string(),
        raw: diag,
    };
    let rendered = render(&test, &[ui_diag]);
    assert!(
        rendered.contains("error: missing ';'"),
        "expected 'error: missing \";\"' in:\n{}",
        rendered
    );
    assert!(
        rendered.contains("harness_self/render_input.xs"),
        "expected frozen label in:\n{}",
        rendered
    );
    assert!(
        rendered.contains("return 5"),
        "expected source line in:\n{}",
        rendered
    );
    assert!(
        rendered.contains('^'),
        "expected underline in:\n{}",
        rendered
    );
    assert!(
        !rendered.contains('\u{1b}'),
        "expected no ANSI escapes in:\n{}",
        rendered
    );
    Ok(())
}

fn _self_test_render_determinism() -> Result<(), String> {
    let test = UiTest {
        path: PathBuf::from("harness_self/order.xs"),
        relative: "harness_self/order.xs".to_string(),
        source: "a\nb\n".to_string(),
        directive: Directive::CheckFail,
        markers: vec![],
    };
    let d1 = Diagnostic::error()
        .with_message("second")
        .with_label(codespan_reporting::diagnostic::Label::primary((), 2..3));
    let d2 = Diagnostic::error()
        .with_message("first")
        .with_label(codespan_reporting::diagnostic::Label::primary((), 0..1));
    let diags = vec![
        UiDiagnostic {
            line: 2,
            message: "second".to_string(),
            raw: d1,
        },
        UiDiagnostic {
            line: 1,
            message: "first".to_string(),
            raw: d2,
        },
    ];
    let rendered = render(&test, &diags);
    let pos_first = rendered
        .find("error: first")
        .ok_or_else(|| format!("'first' not in:\n{}", rendered))?;
    let pos_second = rendered
        .find("error: second")
        .ok_or_else(|| format!("'second' not in:\n{}", rendered))?;
    assert!(
        pos_first < pos_second,
        "expected diagnostics ordered by position:\n{}",
        rendered
    );
    Ok(())
}

fn make_test(source: &str, directive: Directive, markers: Vec<Marker>) -> UiTest {
    UiTest {
        path: PathBuf::from("harness_self/synthetic.xs"),
        relative: "harness_self/synthetic.xs".to_string(),
        source: source.to_string(),
        directive,
        markers,
    }
}

fn _self_test_validate_check_pass_ok() -> Result<(), String> {
    let test = make_test("void f() {}", Directive::CheckPass, vec![]);
    validate(&test, &[]).map_err(|e| e.to_string())
}

fn _self_test_validate_check_pass_fails() -> Result<(), String> {
    let test = make_test("void f() {}", Directive::CheckPass, vec![]);
    let diag = UiDiagnostic {
        line: 1,
        message: "oops".to_string(),
        raw: Diagnostic::error().with_message("oops"),
    };
    match validate(&test, &[diag]) {
        Err(TestFailure::UnexpectedDiagnostic { line, message }) if line == 1 && message == "oops" => {
            Ok(())
        }
        other => Err(format!("expected UnexpectedDiagnostic, got {:?}", other)),
    }
}

fn _self_test_validate_check_fail_match() -> Result<(), String> {
    let test = make_test(
        "void f() {\n  return 5\n}",
        Directive::CheckFail,
        vec![Marker {
            line: 2,
            message: "missing ';'".to_string(),
        }],
    );
    let diag = UiDiagnostic {
        line: 2,
        message: "missing ';'".to_string(),
        raw: Diagnostic::error().with_message("missing ';'"),
    };
    validate(&test, &[diag]).map_err(|e| e.to_string())
}

fn _self_test_validate_check_fail_wrong_line() -> Result<(), String> {
    let test = make_test(
        "void f() {\n  return 5\n}",
        Directive::CheckFail,
        vec![Marker {
            line: 2,
            message: "missing ';'".to_string(),
        }],
    );
    let diag = UiDiagnostic {
        line: 3,
        message: "missing ';'".to_string(),
        raw: Diagnostic::error().with_message("missing ';'"),
    };
    match validate(&test, &[diag]) {
        Err(TestFailure::MissingDiagnostic { line, expected }) if line == 2 && expected == "missing ';'" => Ok(()),
        other => Err(format!("expected MissingDiagnostic, got {:?}", other)),
    }
}

fn _self_test_validate_check_fail_extra_diag() -> Result<(), String> {
    let test = make_test(
        "void f() {\n  return 5\n}",
        Directive::CheckFail,
        vec![Marker {
            line: 2,
            message: "missing ';'".to_string(),
        }],
    );
    let diags = vec![
        UiDiagnostic {
            line: 2,
            message: "missing ';'".to_string(),
            raw: Diagnostic::error().with_message("missing ';'"),
        },
        UiDiagnostic {
            line: 1,
            message: "extra".to_string(),
            raw: Diagnostic::error().with_message("extra"),
        },
    ];
    match validate(&test, &diags) {
        Err(TestFailure::UnexpectedDiagnostic { line, message }) if line == 1 && message == "extra" => Ok(()),
        other => Err(format!("expected UnexpectedDiagnostic, got {:?}", other)),
    }
}

fn _self_test_validate_check_fail_no_markers() -> Result<(), String> {
    let test = make_test("void f() {}", Directive::CheckFail, vec![]);
    match validate(&test, &[]) {
        Err(TestFailure::CheckFailNeedsMarker) => Ok(()),
        other => Err(format!("expected CheckFailNeedsMarker, got {:?}", other)),
    }
}

fn _self_test_validate_error_substrings() -> Result<(), String> {
    let missing = TestFailure::MissingDiagnostic {
        line: 3,
        expected: "missing ';'".to_string(),
    };
    let s = missing.to_string();
    assert!(s.contains(r#"expected error matching "missing ';'" on line 3, found none"#), "{}", s);

    let mismatch = TestFailure::SpanMismatch {
        marker_line: 3,
        diagnostic_line: 4,
    };
    let s = mismatch.to_string();
    assert!(s.contains("marker on line 3 bound to diagnostic on line 4 (expected 3)"), "{}", s);

    let missing_err = TestFailure::MissingErr {
        relative: "ui/foo".to_string(),
    };
    let s = missing_err.to_string();
    assert!(s.contains("missing ui/foo.err — run with --bless to generate"), "{}", s);

    let present = TestFailure::ErrPresentForCheckPass {
        relative: "ui/foo".to_string(),
    };
    let s = present.to_string();
    assert!(s.contains("ui/foo.err present for //@ check-pass test"), "{}", s);

    Ok(())
}

fn _self_test_run_parser_clean() -> Result<(), String> {
    let diags = run_parser("void f() { return; }");
    assert!(diags.is_empty(), "expected no errors, got {:?}", diags);
    Ok(())
}

fn _self_test_run_parser_missing_semi() -> Result<(), String> {
    let diags = run_parser("void f() {\n   return 5\n}");
    assert_eq!(diags.len(), 1, "expected one error, got {:?}", diags);
    assert_eq!(diags[0].line, 2);
    assert_eq!(diags[0].message, "missing ';'");
    Ok(())
}

fn _self_test_bless_writes_err_file() -> Result<(), String> {
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let xs = tmp.path().join("blessed.xs");
    let err = tmp.path().join("blessed.err");
    fs::write(
        &xs,
        "//@ check-fail\nvoid f() {\n   return 5 //~ ERROR missing ';'\n}\n",
    )
    .map_err(|e| e.to_string())?;

    let source = fs::read_to_string(&xs).map_err(|e| e.to_string())?;
    let test = UiTest {
        path: xs.clone(),
        relative: "blessed.xs".to_string(),
        source,
        directive: Directive::CheckFail,
        markers: vec![Marker {
            line: 3,
            message: "missing ';'".to_string(),
        }],
    };

    let diags = run_parser(&test.source);
    validate(&test, &diags).map_err(|e| e.to_string())?;
    let rendered = render(&test, &diags);
    assert!(!rendered.is_empty());
    compare_or_bless(&test, &rendered, true).map_err(|e| e.to_string())?;
    assert!(err.exists(), ".err file was not created");

    // A second non-bless run should pass against the golden file.
    compare_or_bless(&test, &rendered, false).map_err(|e| e.to_string())?;
    Ok(())
}

fn _self_test_discover_tests_ignores_non_xs() -> Result<(), String> {
    let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
    fs::write(tmp.path().join("a.xs"), "//@ check-pass\n").map_err(|e| e.to_string())?;
    fs::write(tmp.path().join("b.XS"), "//@ check-pass\n").map_err(|e| e.to_string())?;
    fs::write(tmp.path().join("notes.md"), "hi\n").map_err(|e| e.to_string())?;
    let found = discover_tests(tmp.path());
    assert_eq!(found.len(), 1, "expected only a.xs, got {:?}", found);
    assert!(found[0].file_name() == Some(OsStr::new("a.xs")));
    Ok(())
}
