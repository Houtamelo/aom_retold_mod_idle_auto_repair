//! Regression test for lambda expressions used as function-pointer defaults.
//!
//! Before the PR-D grammar fixes, `void(int) afterQueue = [](int id = -1) {}`
//! inside a parameter list produced multiple ERROR parse diagnostics.

use tower_lsp_server::ls_types::DiagnosticSeverity;
use xs_language_server::diagnostics::collect_diagnostics_with_types;
use xs_parser::ast::TypeTable;

#[test]
fn function_pointer_default_with_lambda_has_zero_parse_errors() {
    let source = r#"
void boVillager(int villagerUnitType = -1, int villagerResourceType = -1, void(int) afterQueue = [](int id = -1) {}) {}
"#;

    let diags = collect_diagnostics_with_types(source, TypeTable::with_primitives());
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
