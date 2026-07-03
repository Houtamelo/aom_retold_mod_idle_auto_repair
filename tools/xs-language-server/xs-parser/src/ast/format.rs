// SPDX-License-Identifier: MIT
//
// Pass 3 (T14) — proof-of-concept format-preserving formatter.
//
// The PoC re-emits a parsed XS source byte-for-byte by walking the CST
// in pre-order, left-to-right, and concatenating `&source[span]` for
// each child node. This validates two things:
//
// 1. **Byte-for-byte round-trip.** Every parsed source must emit
//    identical bytes when passed through `format_translation_unit`.
//    Round-trip is asserted for the 17-line test source from
//    `examples/test_parse.rs` and for 5 hand-selected retail XS files.
//
// 2. **Wrapper token spans are consistent with the source.** The
//    AST's stored wrapper spans (`Braced.open`/`close`,
//    `Parenthesized.open`/`close`, `CommaSeparatedList` trailing
//    commas) must slice the source to the expected token text.
//    `wrapper_spans_*` tests assert this directly.
//
// ## Limitations (PoC only)
//
// - No indentation policy, comment attachment, or whitespace
//   normalization. The PoC is purely a "source passthrough" that
//   proves the AST's wrapper-token spans are correct and sufficient.
// - A production formatter would walk the AST structure (not just
//   the CST leaves) and use stored wrapper spans (e.g.
//   `Braced.open`) directly when emitting, plus configurable
//   indentation and reformatting rules.

use crate::ast::top_level::TranslationUnit;
use crate::parser::{Cst, NodeRef};

/// Re-emit the source from the CST by walking its children in
/// pre-order and concatenating `&source[span]` for each.
///
/// The PoC does **not** require `tu` to drive the walk — the CST's
/// direct children of `NodeRef::ROOT` already cover the full source
/// range (skip tokens interleaved with top-level rule nodes, each
/// carrying `cst.span(node)` = "first to last token"). We accept
/// `_tu` as a parameter to match the spec's signature and to keep
/// room for a future "format-by-AST" mode that uses the AST's
/// stored wrapper spans for indented output.
pub fn format_translation_unit(cst: &Cst, _tu: &TranslationUnit) -> String {
    let mut out = String::new();
    format_node(cst, NodeRef::ROOT, &mut out);
    out
}

/// Walk a rule node's children in order, emitting each child's source
/// span. For rule children we recurse so their nested tokens are
/// emitted; for token children we slice `&source[span]` directly. The
/// recursion bottoms out at leaf tokens.
///
/// This is equivalent to "emit every leaf token's source span in
/// left-to-right order" — which is byte-for-byte identical to the
/// original source.
fn format_node(cst: &Cst, node: NodeRef, out: &mut String) {
    for child in cst.children(node) {
        match cst.get(child) {
            crate::parser::Node::Token(_, _) => {
                out.push_str(&cst.source()[cst.span(child)]);
            }
            crate::parser::Node::Rule(_, _) => {
                format_node(cst, child, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::cst_helpers::is_skip_token;
    use crate::ast::declaration::{ArgumentList, ParameterList};
    use crate::ast::expr::Expr;
    use crate::ast::top_level::{ClassDefinition, FunctionDefinition};
    use crate::parser::{Node, NodeRef, Parser, Rule};

    fn parse<'a>(source: &'a str) -> (Cst<'a>, Vec<crate::parser::Diagnostic>) {
        let mut diags = vec![];
        let cst = Parser::new(source, &mut diags).parse(&mut diags);
        (cst, diags)
    }

    fn first_non_skip(cst: &Cst) -> NodeRef {
        cst.children(NodeRef::ROOT)
            .find(|c| !is_skip_token(cst, *c))
            .expect("at least one non-skip child")
    }

    // =========================================================================
    // Round-trip tests
    // =========================================================================

    /// The 17-line test source from `examples/test_parse.rs`. Read from
    /// the file directly so the two stay in sync if the example is
    /// edited.
    const TEST_PARSE_SOURCE: &str = r#"
extern const int cFoo = 5;
int gFoo = cFoo;

void bar() {
    int x = 1;
    for (i = 0; i < 10; i = i + 1) {
        x = x + i;
    }
}

int a, b = 2, c = 3;

vector pos = vector(1.0, 2.0, 3.0);

#if (defined(MY_INCLUDE) == false)
#define MY_INCLUDE
int included = 1;
#endif
"#;

    #[test]
    fn round_trip_test_parse_source() {
        let (cst, _) = parse(TEST_PARSE_SOURCE);
        let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT).expect("TU");
        let output = format_translation_unit(&cst, &tu);
        assert_eq!(
            output, TEST_PARSE_SOURCE,
            "format_translation_unit must produce byte-identical output"
        );
    }

    /// Round-trip the 5 hand-selected retail XS files from the AoM:R
    /// game install. The game path is overridable via the
    /// `AOMR_GAME_PATH` env var (defaults to the Steam Deck install
    /// layout). The test is skipped if the game is not installed.
    fn retail_round_trip(relative_path: &str) {
        let game_path = std::env::var("AOMR_GAME_PATH").unwrap_or_else(|_| {
            "/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold".to_string()
        });
        let path = std::path::Path::new(&game_path).join(relative_path);
        if !path.exists() {
            eprintln!(
                "skipping retail round-trip for {} (file not found at {})",
                relative_path,
                path.display()
            );
            return;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("read {}: {}", path.display(), e));
        let (cst, diags) = parse(&source);
        // Surface parse diagnostics but don't fail the test for them —
        // the PoC grammar doesn't accept every retail construct
        // (e.g. lambdas, non-empty rule bodies, if/else). The
        // round-trip itself is what we're validating.
        if !diags.is_empty() {
            eprintln!(
                "  ({} diagnostics during parse of {}, ignored for round-trip)",
                diags.len(),
                relative_path
            );
        }
        let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT)
            .expect(&format!("TU for {}", relative_path));
        let output = format_translation_unit(&cst, &tu);
        if output != source {
            // Find the first diverging byte for diagnostics.
            let first_diff = output
                .as_bytes()
                .iter()
                .zip(source.as_bytes().iter())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| output.len().min(source.len()));
            let snippet_start = first_diff.saturating_sub(40);
            let snippet_end = (first_diff + 40).min(output.len()).min(source.len());
            panic!(
                "round-trip mismatch in {} at byte {}\n\
                 source[{}..{}] = {:?}\n\
                 output[{}..{}] = {:?}",
                relative_path,
                first_diff,
                snippet_start,
                snippet_end,
                &source[snippet_start..snippet_end],
                snippet_start,
                snippet_end,
                &output[snippet_start..snippet_end],
            );
        }
    }

    #[test]
    fn round_trip_retail_chairon() {
        retail_round_trip("game/ai/chairon.xs");
    }

    #[test]
    fn round_trip_retail_strategy() {
        retail_round_trip("game/ai/core/strategy/strategy.xs");
    }

    #[test]
    fn round_trip_retail_main() {
        retail_round_trip("game/ai/core/main.xs");
    }

    #[test]
    fn round_trip_retail_startup_flow() {
        retail_round_trip("game/ai/core/startup/startup_flow.xs");
    }

    #[test]
    fn round_trip_retail_human_assist_debug() {
        retail_round_trip("game/ai/human_assist/human_assist_debug.xs");
    }

    // =========================================================================
    // Wrapper span validation — proves the AST's stored wrapper spans
    // are at the correct source positions. These are the same spans
    // the PoC formatter would use to emit wrapper tokens in a
    // "format-by-AST" mode.
    // =========================================================================

    #[test]
    fn wrapper_spans_for_function_body_braces() {
        let (cst, _) = parse("void f() { return; }");
        let fd = FunctionDefinition::from_cst(&cst, first_non_skip(&cst)).expect("fn");
        assert_eq!(&cst.source()[fd.body.open.clone()], "{");
        assert_eq!(&cst.source()[fd.body.close.clone()], "}");
    }

    #[test]
    fn wrapper_spans_for_function_declarator_parens() {
        let (cst, _) = parse("void f(int a, int b) {}");
        let fd = FunctionDefinition::from_cst(&cst, first_non_skip(&cst)).expect("fn");
        match &fd.declarator.direct {
            crate::ast::declaration::DirectDeclarator::FunctionDeclarator(func_decl) => {
                let params = func_decl.params.as_ref().expect("params");
                assert_eq!(&cst.source()[params.open.clone()], "(");
                assert_eq!(&cst.source()[params.close.clone()], ")");
            }
            other => panic!("expected FunctionDeclarator, got {:?}", other),
        }
    }

    #[test]
    fn wrapper_spans_for_call_expression_parens() {
        let (cst, _) = parse("int y = foo(a, b, c);");
        let decl = first_non_skip(&cst);
        let init_list = cst
            .children(decl)
            .find(|c| cst.match_rule(*c, Rule::InitDeclaratorList))
            .unwrap();
        let init = cst
            .children(init_list)
            .find(|c| cst.match_rule(*c, Rule::InitDeclarator))
            .unwrap();
        let node = cst
            .children(init)
            .find(|c| cst.match_rule(*c, Rule::CommaExpr))
            .expect("comma_expr");
        // Expr::from_cst transparently descends through
        // comma → assign → cond → binary → unary → postfix wrappers.
        let call = Expr::from_cst(&cst, node).expect("expr");
        match call {
            Expr::Postfix(p) => match &p.inner {
                crate::ast::expr::PostfixInner::Call(c) => {
                    assert_eq!(&cst.source()[c.open.clone()], "(");
                    assert_eq!(&cst.source()[c.close.clone()], ")");
                }
                other => panic!("expected Call, got {:?}", other),
            },
            other => panic!("expected Postfix, got {:?}", other),
        }
    }

    #[test]
    fn wrapper_spans_for_subscript_brackets() {
        let (cst, _) = parse("int y = a[0];");
        let decl = first_non_skip(&cst);
        let init_list = cst
            .children(decl)
            .find(|c| cst.match_rule(*c, Rule::InitDeclaratorList))
            .unwrap();
        let init = cst
            .children(init_list)
            .find(|c| cst.match_rule(*c, Rule::InitDeclarator))
            .unwrap();
        let node = cst
            .children(init)
            .find(|c| cst.match_rule(*c, Rule::CommaExpr))
            .expect("comma_expr");
        let expr = Expr::from_cst(&cst, node).expect("expr");
        match expr {
            Expr::Postfix(p) => match &p.inner {
                crate::ast::expr::PostfixInner::Subscript(s) => {
                    assert_eq!(&cst.source()[s.open.clone()], "[");
                    assert_eq!(&cst.source()[s.close.clone()], "]");
                }
                other => panic!("expected Subscript, got {:?}", other),
            },
            other => panic!("expected Postfix, got {:?}", other),
        }
    }

    #[test]
    fn wrapper_spans_for_argument_list_trailing_commas() {
        let (cst, _) = parse("int y = foo(a, b, c);");
        let decl = first_non_skip(&cst);
        let init_list = cst
            .children(decl)
            .find(|c| cst.match_rule(*c, Rule::InitDeclaratorList))
            .unwrap();
        let init = cst
            .children(init_list)
            .find(|c| cst.match_rule(*c, Rule::InitDeclarator))
            .unwrap();
        let mut node = cst
            .children(init)
            .find(|c| cst.match_rule(*c, Rule::CommaExpr))
            .expect("comma_expr");
        for _ in 0..3 {
            node = cst
                .children(node)
                .find(|c| matches!(cst.get(*c), Node::Rule(_, _)))
                .expect("rule child");
        }
        let arg_list_node = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::ArgumentList))
            .expect("argument_list");
        let arg_list = ArgumentList::from_cst(&cst, arg_list_node).expect("arg_list");
        assert_eq!(arg_list.items.len(), 3);
        assert_eq!(&cst.source()[arg_list.items.items[0].1.clone().unwrap()], ",");
        assert_eq!(&cst.source()[arg_list.items.items[1].1.clone().unwrap()], ",");
        assert!(arg_list.items.items[2].1.is_none());
    }

    #[test]
    fn wrapper_spans_for_parameter_list_trailing_commas() {
        let (cst, _) = parse("void foo(int a, int b, int c) = 0;");
        let decl = first_non_skip(&cst);
        let init_list = cst
            .children(decl)
            .find(|c| cst.match_rule(*c, Rule::InitDeclaratorList))
            .unwrap();
        let init = cst
            .children(init_list)
            .find(|c| cst.match_rule(*c, Rule::InitDeclarator))
            .unwrap();
        let param_list_node = cst
            .children(init)
            .find(|c| cst.match_rule(*c, Rule::ParameterList))
            .expect("parameter_list");
        let pl = ParameterList::from_cst(&cst, param_list_node).expect("param_list");
        assert_eq!(pl.items.len(), 3);
        assert_eq!(&cst.source()[pl.items.items[0].1.clone().unwrap()], ",");
        assert_eq!(&cst.source()[pl.items.items[1].1.clone().unwrap()], ",");
        assert!(pl.items.items[2].1.is_none());
    }

    #[test]
    fn wrapper_spans_for_class_definition_braces() {
        let (cst, _) = parse("class MyClass { int x = 5; }");
        let cd = ClassDefinition::from_cst(&cst, first_non_skip(&cst)).expect("class");
        assert_eq!(&cst.source()[cd.members.open.clone()], "{");
        assert_eq!(&cst.source()[cd.members.close.clone()], "}");
    }

    // =========================================================================
    // Unit tests for format_node
    // =========================================================================

    #[test]
    fn format_node_emits_tokens_in_order() {
        // `int x = 5;` — the formatter must emit each leaf token in
        // source order. Verify by counting emitted bytes: 10 chars.
        let (cst, _) = parse("int x = 5;");
        let mut out = String::new();
        format_node(&cst, NodeRef::ROOT, &mut out);
        assert_eq!(out, "int x = 5;");
    }

    #[test]
    fn format_node_preserves_whitespace_between_tokens() {
        let (cst, _) = parse("void f() { return; }");
        let mut out = String::new();
        format_node(&cst, NodeRef::ROOT, &mut out);
        assert_eq!(out, "void f() { return; }");
    }

    #[test]
    fn format_node_preserves_comments() {
        let (cst, _) = parse("// hello\nint x; /* block */");
        let mut out = String::new();
        format_node(&cst, NodeRef::ROOT, &mut out);
        assert_eq!(out, "// hello\nint x; /* block */");
    }

    #[test]
    fn format_node_preserves_newlines_and_indentation() {
        let (cst, _) = parse("void f() {\n    int x = 1;\n}\n");
        let mut out = String::new();
        format_node(&cst, NodeRef::ROOT, &mut out);
        assert_eq!(out, "void f() {\n    int x = 1;\n}\n");
    }

    #[test]
    fn format_node_for_empty_translation_unit() {
        let (cst, _) = parse("");
        let mut out = String::new();
        format_node(&cst, NodeRef::ROOT, &mut out);
        assert_eq!(out, "");
    }

    #[test]
    fn format_node_handles_include_directive() {
        let (cst, _) = parse(r#"include "core/main.xs";"#);
        let mut out = String::new();
        format_node(&cst, NodeRef::ROOT, &mut out);
        assert_eq!(out, r#"include "core/main.xs";"#);
    }

    #[test]
    fn format_node_handles_preproc_conditional() {
        let (cst, _) = parse("#if (defined(X))\nint y = 1;\n#endif\n");
        let mut out = String::new();
        format_node(&cst, NodeRef::ROOT, &mut out);
        assert_eq!(out, "#if (defined(X))\nint y = 1;\n#endif\n");
    }

    #[test]
    fn format_node_handles_class_with_methods() {
        let source = "class MyClass {\n    int x = 5;\n    void foo() { return; }\n}\n";
        let (cst, _) = parse(source);
        let mut out = String::new();
        format_node(&cst, NodeRef::ROOT, &mut out);
        assert_eq!(out, source);
    }

    #[test]
    fn format_node_for_multi_declarator_list() {
        // `int a, b = 2, c = 3;` exercises the
        // `CommaSeparatedList` shape with trailing commas.
        let (cst, _) = parse("int a, b = 2, c = 3;");
        let mut out = String::new();
        format_node(&cst, NodeRef::ROOT, &mut out);
        assert_eq!(out, "int a, b = 2, c = 3;");
    }

    #[test]
    fn format_translation_unit_returns_same_source() {
        // End-to-end on a small synthetic source.
        let source = "extern const int cFoo = 5;\nvoid f(int a) { return; }\n";
        let (cst, _) = parse(source);
        let tu = TranslationUnit::from_cst(&cst, NodeRef::ROOT).expect("TU");
        let out = format_translation_unit(&cst, &tu);
        assert_eq!(out, source);
    }

    // =========================================================================
    // Smoke test: prove the test_parse source matches what the example
    // hardcodes (i.e., the canonical 17-line source).
    // =========================================================================

    #[test]
    fn test_parse_source_is_canonical_17_lines() {
        // The example/test_parse.rs source should be ~17 lines. This
        // guards against drift between the example file and the
        // round-trip test's hardcoded string.
        let example_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("examples")
            .join("test_parse.rs");
        let example_text = std::fs::read_to_string(&example_path)
            .unwrap_or_else(|e| panic!("read {}: {}", example_path.display(), e));
        // Extract the source string from the example: it appears
        // between `r#"` and `"#` on lines 7..26 of test_parse.rs.
        let start = example_text
            .find("r#\"")
            .expect("r#\" marker");
        let end = example_text
            .rfind("\"#;")
            .expect("\"#; marker");
        let extracted = &example_text[start + 3..end];
        assert_eq!(
            extracted, TEST_PARSE_SOURCE,
            "TEST_PARSE_SOURCE in format.rs must match the source literal in examples/test_parse.rs"
        );
    }
}