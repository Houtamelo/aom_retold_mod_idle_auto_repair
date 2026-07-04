//! Isolated test for top-level preproc
use xs_parser::parser::{Parser, Diagnostic};
use xs_parser::ast::type_table::TypeTable;
use codespan_reporting::diagnostic::Severity;

fn parse(src: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let _ = Parser::new_with_context(src, &mut diags, TypeTable::with_primitives()).parse(&mut diags);
    diags
}

#[test]
fn simple_top_level_ifdef() {
    let src = "#if (X)\nint y = 1;\n#endif\n";
    let diags = parse(src);
    for d in &diags {
        if d.severity == Severity::Error {
            let pos = d.labels.first().map(|l| l.range.start).unwrap_or(0);
            eprintln!("[{}] {}", pos, d.message);
        }
    }
    let n = diags.iter().filter(|d| d.severity == Severity::Error).count();
    assert_eq!(n, 0, "got {n} errors");
}

#[test]
fn top_level_with_leading_blank() {
    // This is the case that fails in utilities_subset.rs:
    //   \n#if (defined(X) == false)\n#define X\nint y;\n#endif
    let src = "\n#if (defined(MY_INCLUDE_GUARD) == false)\n#define MY_INCLUDE_GUARD\nint globalVar = 5;\n#endif\n";
    let diags = parse(src);
    for d in &diags {
        if d.severity == Severity::Error {
            let pos = d.labels.first().map(|l| l.range.start).unwrap_or(0);
            eprintln!("[{}] {}", pos, d.message);
            let end = (pos + 50).min(src.len());
            eprintln!("    -> {}", src[pos..end].replace('\n', "\\n"));
        }
    }
    let n = diags.iter().filter(|d| d.severity == Severity::Error).count();
    assert_eq!(n, 0, "got {n} errors");
}

#[test]
fn no_leading_newline() {
    let src = "#if (defined(MY_INCLUDE_GUARD) == false)\n#define MY_INCLUDE_GUARD\nint globalVar = 5;\n#endif\n";
    let diags = parse(src);
    let n = diags.iter().filter(|d| d.severity == Severity::Error).count();
    assert_eq!(n, 0, "no leading newline, got {n} errors");
}

#[test]
fn with_leading_newline() {
    let src = "\n#if (defined(MY_INCLUDE_GUARD) == false)\n#define MY_INCLUDE_GUARD\nint globalVar = 5;\n#endif\n";
    let diags = parse(src);
    let n = diags.iter().filter(|d| d.severity == Severity::Error).count();
    assert_eq!(n, 0, "with leading newline, got {n} errors");
}

#[test]
fn with_leading_spaces() {
    let src = "   #if (defined(MY_INCLUDE_GUARD) == false)\n#define MY_INCLUDE_GUARD\nint globalVar = 5;\n#endif\n";
    let diags = parse(src);
    let n = diags.iter().filter(|d| d.severity == Severity::Error).count();
    assert_eq!(n, 0, "with leading spaces, got {n} errors");
}

#[test]
fn preproc_with_false_keyword() {
    // Retail pattern: #if (defined(X) == false)
    let src = "#if (defined(X) == false)\nint y = 1;\n#endif\n";
    let diags = parse(src);
    for d in &diags {
        if d.severity == Severity::Error {
            let pos = d.labels.first().map(|l| l.range.start).unwrap_or(0);
            eprintln!("[{}] {}", pos, d.message);
        }
    }
    let n = diags.iter().filter(|d| d.severity == Severity::Error).count();
    assert_eq!(n, 0, "got {n} errors");
}

#[test]
fn preproc_with_true_keyword() {
    let src = "#if (defined(X) == true)\nint y = 1;\n#endif\n";
    let diags = parse(src);
    let n = diags.iter().filter(|d| d.severity == Severity::Error).count();
    assert_eq!(n, 0, "got {n} errors");
}
