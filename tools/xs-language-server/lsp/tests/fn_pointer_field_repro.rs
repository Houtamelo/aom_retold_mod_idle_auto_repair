//! TDD: function-pointer-typed class fields.
//!
//! Retail XS class declarations like:
//! ```xs
//! class Strategy {
//!    void(int) mWaveStartCallback = attackWaveDefaultCallback;
//!    bool(ref StrategyData) mUpdateFunc = [](ref StrategyData data) -> bool { return true; };
//!    void() mStartupBO = []() {};
//! }
//! ```
//! require parsing `void(int)` etc. as a type specifier for class fields.
//!
//! These patterns previously failed to parse because the `class_member`
//! rule only knew about `function_definition_rule`, `forward_declaration_rule`,
//! and `field_declaration`. None of them recognize a leading
//! `primitive_type '('` as a function-pointer-typed field.
//!
//! Each test parses a tiny class containing one function-pointer field
//! and asserts that no ERROR diagnostics are produced.

use xs_parser::parser::{Parser, Diagnostic};
use xs_parser::ast::type_table::TypeTable;
use codespan_reporting::diagnostic::Severity;

fn parse(src: &str) -> Vec<Diagnostic> {
    let mut types = TypeTable::with_primitives();
    // Seed common types that appear in retail fn-ptr fields
    for name in ["StrategyData", "Strategy"] {
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
                eprintln!("error at {}: {}", d.labels.first().map(|l| l.range.start).unwrap_or(0), d.message);
            }
        }
    }
    assert_eq!(count_errors(&diags), 0, "src = {src:?}");
}

#[test]
fn field_with_int_param_function_pointer() {
    let src = "class Foo { void(int) onPlanCreate = [](int planID = -1) {}; };";
    assert_no_errors(src);
}

#[test]
fn field_with_bool_return_no_params() {
    let src = "class Foo { bool() condition = []() -> bool { return true; }; };";
    assert_no_errors(src);
}

#[test]
fn field_with_void_return_no_params() {
    let src = "class Foo { void() onInit = []() {}; };";
    assert_no_errors(src);
}

#[test]
fn field_with_ref_param_function_pointer() {
    let src = "class Foo { bool(ref StrategyData) mUpdateFunc = [](ref StrategyData data) -> bool { return true; }; };";
    assert_no_errors(src);
}

#[test]
fn field_with_multi_param_function_pointer() {
    let src = "class Foo { bool(int, int, int) gHandler = [](int a = 0, int b = 0, int c = 0) -> bool { return true; }; };";
    assert_no_errors(src);
}

#[test]
fn field_with_identifier_initializer() {
    let src = "class Foo { void(int) mCallback = attackWaveDefaultCallback; };";
    assert_no_errors(src);
}

#[test]
fn multiple_function_pointer_fields() {
    let src = "\
class Strategy {
   void() mInit = []() {};
   bool(ref StrategyData) mUpdateFunc = [](ref StrategyData data) -> bool { return true; };
   void(ref StrategyData) mDestroyFunc = [](ref StrategyData data) {};
   float() mGetCurrentScore = []() -> float { return 0.0; };
};
";
    assert_no_errors(src);
}

#[test]
fn regular_field_still_works() {
    // Regression guard: a plain `int x;` field must still parse.
    let src = "class Foo { int x = 0; string s = \"hi\"; };";
    assert_no_errors(src);
}

#[test]
fn function_method_still_works() {
    // Regression guard: a method definition must still parse.
    let src = "class Foo { void bar() { int x = 0; } };";
    assert_no_errors(src);
}
