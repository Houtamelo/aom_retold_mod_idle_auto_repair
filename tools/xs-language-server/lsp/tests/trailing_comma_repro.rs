//! TDD: trailing comma in function-call argument lists.
//!
//! Retail XS (military_units.xs L2503) uses:
//!   `createSimpleTrainPlan(cUnitTypeTlamanihSpearman, 1, gLandAreaGroupID, gMilitaryTrainingCategoryID,);`
//! i.e. an argument list with a trailing comma. The current
//! `argument_list` rule requires a trailing `,` only between
//! arguments, not after the last one.

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
                eprintln!("error: {}", d.message);
            }
        }
    }
    assert_eq!(count_errors(&diags), 0, "src = {src:?}");
}

#[test]
fn trailing_comma_in_call() {
    let src = "void f() { g(1, 2, 3,); }";
    assert_no_errors(src);
}

#[test]
fn trailing_comma_in_real_call() {
    let src = "void f() { createSimpleTrainPlan(cUnitTypeTlamanihSpearman, 1, gLandAreaGroupID, gMilitaryTrainingCategoryID,); }";
    assert_no_errors(src);
}

#[test]
fn no_trailing_comma_still_works() {
    let src = "void f() { g(1, 2, 3); }";
    assert_no_errors(src);
}

#[test]
fn empty_args_still_works() {
    let src = "void f() { g(); }";
    assert_no_errors(src);
}
