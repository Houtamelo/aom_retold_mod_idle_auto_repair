//! TDD: `ref` qualifier combined with a user-defined array type in
//! function parameters (Biome.xs).
//!
//! Retail XS uses:
//!   `int randomizeFromCandidates(ref HuntData[] candidates)`
//!   `bool getHuntCandidates(ref HuntData[] candidatesOut, ...)`
//! The `ref` type-qualifier + user-defined-type (`HuntData`) +
//! trailing-array-decoration (`[]`) combination was rejected by the
//! parser because `parameter_declaration`'s inner alternative only
//! knew about the `function_pointer_param` (`( ... ) name`) and
//! `regular_param` (`name`) forms, neither of which start with `[`.

use xs_parser::parser::{Parser, Diagnostic};
use xs_parser::ast::type_table::TypeTable;
use codespan_reporting::diagnostic::Severity;

fn parse(src: &str) -> Vec<Diagnostic> {
    let mut types = TypeTable::with_primitives();
    for name in ["HuntData"] {
        types.insert_class(name);
    }
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
                eprintln!("error: {}", d.message);
            }
        }
    }
    assert_eq!(count_errors(&diags), 0, "src = {src:?}");
}

#[test]
fn ref_user_array_param() {
    assert_no_errors("void foo(ref HuntData[] candidates) {}");
}

#[test]
fn ref_user_array_param_with_other_params() {
    assert_no_errors("void foo(ref HuntData[] candidatesOut, int flags, float minFood = 0.0) {}");
}

#[test]
fn ref_primitive_array_still_works() {
    assert_no_errors("void foo(ref int[] arr) {}");
}

#[test]
fn plain_user_array_param_still_works() {
    // No `ref` — the array decoration is in the declarator.
    assert_no_errors("void foo(HuntData[] candidates) {}");
}

#[test]
fn plain_user_param_still_works() {
    assert_no_errors("void foo(ref HuntData data) {}");
}
