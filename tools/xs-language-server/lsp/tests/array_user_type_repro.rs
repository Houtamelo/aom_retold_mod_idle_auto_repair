//! TDD: array-typed user-defined class declarations.
//!
//! Retail XS uses `ConstraintParameters[] vConstraints = default;` in
//! rm_forests.xs. The grammar currently supports `int[]` (primitive
//! array type) but not `UserClass[]` (user-defined array type).
//!
//! Each test must currently fail (RED) and pass after the grammar fix.

use xs_parser::parser::{Parser, Diagnostic};
use xs_parser::ast::type_table::TypeTable;
use codespan_reporting::diagnostic::Severity;

fn parse(src: &str) -> Vec<Diagnostic> {
    let mut types = TypeTable::with_primitives();
    types.insert_class("ConstraintParameters");
    let mut diags = Vec::new();
    let _ = Parser::new_with_context(src, &mut diags, types).parse(&mut diags);
    diags
}

fn count_errors(diags: &[Diagnostic]) -> usize {
    diags.iter().filter(|d| d.severity == Severity::Error).count()
}

#[test]
fn primitive_array_still_works() {
    let src = "int[] vConstraints = default;";
    let diags = parse(src);
    if count_errors(&diags) > 0 {
        for d in &diags {
            if d.severity == Severity::Error {
                eprintln!("[{}] {}", d.labels.first().map(|l| l.range.start).unwrap_or(0), d.message);
            }
        }
    }
    assert_eq!(count_errors(&diags), 0, "{src:?}");
}

#[test]
fn user_class_array() {
    let src = "ConstraintParameters[] vConstraints = default;";
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}

#[test]
fn user_class_array_no_initializer() {
    let src = "ConstraintParameters[] vConstraints;";
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}

#[test]
fn user_class_array_with_default_initializer() {
    // Retail uses `default` (not brace-lists) to initialize arrays.
    let src = "ConstraintParameters[] vConstraints = default;";
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}

#[test]
fn multiple_user_class_arrays() {
    // The exact pattern in rm_forests.xs:
    let src = r#"
ConstraintParameters[] vConstraints = default;
int[] vTypes = default;
float[] vTypeChances = default;
"#;
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}

#[test]
fn user_class_array_in_function_body() {
    let src = r#"
void main() {
   ConstraintParameters[] local = default;
}
"#;
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}
