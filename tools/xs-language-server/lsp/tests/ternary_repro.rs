//! TDD: ternary `?:` operator support.
//!
//! Used heavily in utilities.xs:
//!   return gMainGatherBase != -1 ? gMainGatherBase : kbBaseGetMainID(cMyID);
//!
//! Each test must currently fail (RED) before the grammar change,
//! and pass (GREEN) after.

use xs_parser::parser::{Parser, Diagnostic};
use xs_parser::ast::type_table::TypeTable;
use codespan_reporting::diagnostic::Severity;

fn parse(src: &str) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let _ = Parser::new_with_context(src, &mut diags, TypeTable::with_primitives()).parse(&mut diags);
    diags
}

fn count_errors(diags: &[Diagnostic]) -> usize {
    diags.iter().filter(|d| d.severity == Severity::Error).count()
}

#[test]
fn simple_ternary() {
    let src = "void main() { int x = (a > 0) ? a : -a; }";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}

#[test]
fn ternary_in_return() {
    let src = "int getMainGatherBaseID() { return gX != -1 ? gX : kbGetID(cMyID); }";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}

#[test]
fn nested_ternary() {
    let src = "void main() { int x = (a > 0) ? ((a > 5) ? 5 : a) : -a; }";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}

#[test]
fn ternary_with_function_call() {
    let src = "void main() { int x = cond ? f(a) : g(b, c); }";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}