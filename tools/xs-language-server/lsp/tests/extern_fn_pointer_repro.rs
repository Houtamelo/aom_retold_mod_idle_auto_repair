//! TDD: top-level `extern` function-pointer-typed declarations.
//!
//! Retail XS uses:
//!   `extern void() gStartupBOArchaic = []() {};`
//!   `extern bool(int, int, int) gFarmPlacementOverride = ...;`
//! at the top level. The `function_pointer_declaration` rule
//! currently does not allow the optional `extern` storage-class
//! specifier, so the parser falls through to `declaration`, which
//! expects a type specifier — and `extern` is not a valid
//! type_specifier in the current grammar.

use xs_parser::parser::{Parser, Diagnostic};
use xs_parser::ast::type_table::TypeTable;
use codespan_reporting::diagnostic::Severity;

fn parse(src: &str) -> Vec<Diagnostic> {
    let mut types = TypeTable::with_primitives();
    let mut diags = Vec::new();
    let _ = Parser::new_with_context(src, &mut diags, types).parse(&mut diags);
    diags
}

fn count_errors(diags: &[Diagnostic]) -> usize {
    diags.iter().filter(|d| d.severity == Severity::Error).count()
}

fn assert_no_errors(src: &str) {
    let diags = parse(src);
    if count_errors(&diags) > 0 {
        for d in &diags {
            if d.severity == Severity::Error {
                eprintln!("error at {}: {}", d.labels.first().map(|l| l.range.start).unwrap_or(0), d.message);
            }
        }
    }
    assert_eq!(count_errors(&diags), 0, "src = {src:?}");
}

#[test]
fn extern_void_no_params() {
    let src = "extern void() gStartupBOArchaic = []() {};";
    assert_no_errors(src);
}

#[test]
fn extern_bool_multi_params() {
    let src = "extern bool(int, int, int) gFarmPlacementOverride = [](int planID = -1, int bpID = -1, int baseID = -1) -> bool { return (false); };";
    assert_no_errors(src);
}

#[test]
fn bare_function_pointer_still_works() {
    // Regression guard: a non-extern function-pointer decl must still parse.
    let src = "void() gStartup = []() {};";
    assert_no_errors(src);
}

#[test]
fn regular_extern_still_works() {
    // Regression guard: a regular extern declaration must still parse.
    let src = "extern int gArmy = 0;";
    assert_no_errors(src);
}
