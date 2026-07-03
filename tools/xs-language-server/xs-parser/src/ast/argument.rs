// SPDX-License-Identifier: MIT
//
// Pass 3 (T13) — organizational split of `Argument`/`ArgumentList`.
//
// The actual type definitions live in `declaration.rs` for back-compat
// (other modules like `top_level.rs` and `statement.rs` import them
// from there). This module exists for the T13 organizational
// structure: every AST fragment has its own file. All types are
// re-exported from `declaration.rs` so consumers can refer to
// `crate::ast::argument::Argument` instead of
// `crate::ast::declaration::Argument` going forward. Pass 4 LSP
// wiring should switch to the new path.
//
// The `from_cst` methods also live in `declaration.rs` so existing
// code paths (e.g. `CallExpr::from_cst` in `expr.rs`) can call
// `ArgumentList::from_cst(cst, n)` through this re-export directly.

pub use crate::ast::declaration::{Argument, ArgumentList};

#[cfg(test)]
mod tests {
    //! Verifies that `ArgumentList::from_cst` is reachable through the
    //! `argument` module path and that the comma-separator round-trip
    //! works (3 args, 2 trailing commas, last item has none).
    use super::*;
    use crate::parser::Parser;
    use crate::ast::cst_helpers::is_skip_token;
    use crate::parser::{Cst, NodeRef, Rule};

    fn parse(source: &str) -> Cst<'_> {
        let mut diags = vec![];
        Parser::new(source, &mut diags).parse(&mut diags)
    }

    fn first_non_skip(cst: &Cst) -> NodeRef {
        cst.children(NodeRef::ROOT)
            .find(|c| !is_skip_token(cst, *c))
            .expect("at least one non-skip child")
    }

    #[test]
    fn argument_list_module_path_resolves() {
        // Smoke check — the ArgumentList type is reachable through
        // the argument module path.
        let _: Option<ArgumentList> = None;
    }

    #[test]
    fn argument_list_extracts_three_args_with_commas() {
        // `foo(1, 2, 3)` is a clean synthetic case with a real
        // argument list (the grammar prefers call_expr over
        // vector_literal when the target is an Identifier).
        let cst = parse("int x = foo(1, 2, 3);");
        let decl = first_non_skip(&cst);
        let init_list = cst
            .children(decl)
            .find(|c| cst.match_rule(*c, Rule::InitDeclaratorList))
            .unwrap();
        let init = cst
            .children(init_list)
            .find(|c| cst.match_rule(*c, Rule::InitDeclarator))
            .unwrap();
        // Walk to call_expr by way of passthrough:
        //   comma_expr -> binary_expr -> unary_expr -> postfix_expr -> call_expr.
        let mut node = cst
            .children(init)
            .find(|c| cst.match_rule(*c, Rule::CommaExpr))
            .expect("comma_expr");
        for _ in 0..3 {
            node = cst
                .children(node)
                .find(|c| matches!(cst.get(*c), crate::parser::Node::Rule(_, _)))
                .expect("rule child");
        }
        let arg_list_node = cst
            .children(node)
            .find(|c| cst.match_rule(*c, Rule::ArgumentList))
            .expect("argument_list node");
        let arg_list = ArgumentList::from_cst(&cst, arg_list_node).expect("argument list");
        assert_eq!(arg_list.items.len(), 3);
        // Items 0..N-1 carry trailing commas; item N-1 does not.
        assert!(arg_list.items.items[0].1.is_some());
        assert!(arg_list.items.items[1].1.is_some());
        assert!(arg_list.items.items[2].1.is_none());
    }

    #[test]
    fn argument_extracts_inner_expression() {
        // Single-arg call: `foo(a)` → one Argument with IdentifierExpr.
        let cst = parse("int y = foo(a);");
        let decl = first_non_skip(&cst);
        let init_list = cst
            .children(decl)
            .find(|c| cst.match_rule(*c, Rule::InitDeclaratorList))
            .unwrap();
        let init = cst
            .children(init_list)
            .find(|c| cst.match_rule(*c, Rule::InitDeclarator))
            .unwrap();
        let comma = cst
            .children(init)
            .find(|c| cst.match_rule(*c, Rule::CommaExpr))
            .expect("comma_expr");
        let inner = cst
            .children(comma)
            .find(|c| matches!(cst.get(*c), crate::parser::Node::Rule(_, _)))
            .unwrap();
        // Drill down to the call_expr: passthrough comma→binary→unary→postfix→call_expr.
        let call_node = cst
            .children(inner)
            .find(|c| matches!(cst.get(*c), crate::parser::Node::Rule(_, _)))
            .unwrap();
        let call_inner = cst
            .children(call_node)
            .find(|c| matches!(cst.get(*c), crate::parser::Node::Rule(_, _)))
            .unwrap();
        let arg_list_node = cst
            .children(call_inner)
            .find(|c| cst.match_rule(*c, Rule::ArgumentList))
            .expect("argument_list in call_expr");
        let arg_list = ArgumentList::from_cst(&cst, arg_list_node).expect("argument list");
        assert_eq!(arg_list.items.len(), 1);
        // Use the T12 helper to extract the typed Expr.
        let arg = &arg_list.items.items[0].0;
        let expr = arg.expr(&cst).expect("expr");
        assert!(matches!(expr, crate::ast::expr::Expr::Identifier(_)));
    }
}
