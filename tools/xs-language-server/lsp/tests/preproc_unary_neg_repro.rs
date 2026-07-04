//! TDD: unary `+` and `-` in preprocessor expressions.
//!
//! Retail XS uses `-1` in `#if (cSomething == -1)` patterns inside
//! function bodies. Without unary `-`/`+` in `preproc_unary_expr`,
//! the parser rejects `-1` and cascades errors through the rest of
//! the function.

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
fn unary_minus_at_top() {
    let src = "#if (-1)\nint y = 1;\n#endif\n";
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}

#[test]
fn unary_minus_in_equality() {
    // The exact pattern from getCurrentPersonalityName in utilities.xs.
    let src = "#if (a == -1)\nint y = 1;\n#endif\n";
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}

#[test]
fn unary_plus_at_top() {
    let src = "#if (+1)\nint y = 1;\n#endif\n";
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}

#[test]
fn unary_minus_in_elif_branch() {
    // The exact pattern that triggered the cascade in utilities.xs.
    let src = r#"string main()
{
   #if (a == B)
   return "x";
   #elif (a == -1)
   return "y";
   #else
   return "z";
   #endif
}
"#;
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}

#[test]
fn unary_minus_in_complex_expression() {
    // Used in include guards and value checks.
    let src = "#if (-1 + 2 == 1)\nint y = 1;\n#endif\n";
    assert_eq!(count_errors(&parse(src)), 0, "{src:?}");
}
