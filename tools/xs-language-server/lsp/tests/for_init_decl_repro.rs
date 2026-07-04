//! Regression test for the `for (declaration; cond; step)` initializer form.
//!
//! Before the PR-B grammar fix, `for (int i = 0; ...)` was rejected and
//! produced multiple ERROR parse diagnostics. This test locks in the fix.

use tower_lsp_server::ls_types::DiagnosticSeverity;
use xs_language_server::diagnostics::collect_diagnostics;

#[test]
fn for_init_declaration_form_has_zero_parse_errors() {
    let source = r#"
void test() {
    int total = 0;
    for (int i = 0; i < 10; i = i + 1) {
        total = total + i;
    }
}
"#;

    let diags = collect_diagnostics(source);
    let errors = diags
        .iter()
        .filter(|d| d.severity == Some(DiagnosticSeverity::ERROR))
        .count();
    assert_eq!(
        errors, 0,
        "expected 0 ERROR parse diagnostics, got {}: {:?}",
        errors, diags
    );
}
