//! Regression test for preprocessor directives inside rule modifiers.
//!
//! Retail scripts such as `ai/core/godpowers/godpowers.xs` wrap rule
//! configuration in `#if`/`#else`/`#endif`. Before the PR-E grammar fix
//! these directives were top-level only, so the rule block failed to parse.

use tower_lsp_server::ls_types::DiagnosticSeverity;
use xs_language_server::diagnostics::collect_diagnostics;

#[test]
fn preproc_directives_in_rule_modifiers_parse_without_errors() {
    let source = r#"
rule conditionalRule
#if (cMyCulture == 1)
minInterval 30
group defaultArchaicRules
#else
minInterval 120
group defaultMythicRules
#endif
{
    xsChatData("ok");
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
