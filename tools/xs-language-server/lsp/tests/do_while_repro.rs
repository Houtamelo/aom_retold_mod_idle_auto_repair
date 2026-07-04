//! TDD: `do { ... } while (expr);` statement.
//!
//! Retail XS uses do-while loops in rm_util.xs and similar files:
//!   do
//!   {
//!      areaName = template + " " + index;
//!      index++;
//!   } while(rmAreaGetID(areaName) != cInvalidID);
//!
//! The current `statement^` rule does not model the do-while form;
//! the comment in xs.llw explicitly says "no do { } while()" is
//! intentional. This TDD suite verifies that the statement is
//! added (and the regression guards keep working).

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
fn do_while_with_block() {
    let src = "void f() { do { x = 1; } while(true); }";
    assert_no_errors(src);
}

#[test]
fn do_while_with_single_statement() {
    let src = "void f() { do x = 1; while(true); }";
    assert_no_errors(src);
}

#[test]
fn do_while_with_empty_statement() {
    // `do; while(true);` already works; this is a regression guard.
    let src = "void f() { do; while(true); }";
    assert_no_errors(src);
}

#[test]
fn do_while_with_complex_condition() {
    let src = "void f() { int x = 1; do { x = 2; } while(x != 1); }";
    assert_no_errors(src);
}

#[test]
fn do_while_inside_function_body() {
    let src = "string findNextAreaName(string template, int startIndex) {
        int index = startIndex;
        string areaName = cEmptyString;
        do
        {
           areaName = template + \" \" + index;
           index++;
        } while(rmAreaGetID(areaName) != cInvalidID);
        return areaName;
    }";
    assert_no_errors(src);
}

#[test]
fn while_still_works() {
    // Regression guard: regular while loop must still parse.
    let src = "void f() { while(true) { x = 1; } }";
    assert_no_errors(src);
}

#[test]
fn for_still_works() {
    let src = "void f() { for(int i = 0; i < 10; i++) { x = i; } }";
    assert_no_errors(src);
}
