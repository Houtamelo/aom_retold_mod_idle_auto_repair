//! TDD: bit-shift operator support.
//!
//! These tests drive the parser to accept XS expressions like
//! `int x = 1 << 4;`, `int y = 8 >> 1;`, `a <<= 2;`, `b >>= 3;`.
//!
//! Run: cargo test --test bit_shift_repro
//!
//! Each test must currently fail (RED) before the lexer/grammar
//! changes, and pass (GREEN) after.

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
fn shift_left_rvalue() {
    let src = "void main() { int x = 1 << 4; }";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}

#[test]
fn shift_right_rvalue() {
    let src = "void main() { int y = 8 >> 1; }";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}

#[test]
fn shift_left_assign() {
    let src = "void main() { int a = 0; a <<= 2; }";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}

#[test]
fn shift_right_assign() {
    let src = "void main() { int b = 0; b >>= 3; }";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}

#[test]
fn shift_in_global_initializer() {
    // The exact pattern from rm_locs.xs:
    //   extern const int cBiasForward = (1 << 0);
    let src = "extern const int cBiasForward = (1 << 0);";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}

#[test]
fn chained_shifts() {
    let src = "void main() { int z = (1 << 0) | (1 << 4); }";
    let n = count_errors(&parse(src));
    assert_eq!(n, 0, "expected zero errors, got {n} for {src:?}");
}