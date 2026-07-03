// SPDX-License-Identifier: MIT
//
// Pass 3 (T13) — organizational split of the parameter-related types.
//
// The actual type definitions live in `declaration.rs` for back-compat
// (other modules including `expr.rs::LambdaExpr` import them from
// there). This module exists for the T13 organizational structure:
// every AST fragment has its own file. All types are re-exported
// from `declaration.rs` so consumers can refer to
// `crate::ast::parameter::ParameterList` instead of
// `crate::ast::declaration::ParameterList` going forward. Pass 4 LSP
// wiring should switch to the new path.
//
// The `from_cst` methods live in `declaration.rs` so existing code
// paths can call `ParameterList::from_cst(cst, n)` through this
// re-export directly.

pub use crate::ast::declaration::{
    FunctionPointerParam, Parameter, ParameterDeclaration, ParameterInner, ParameterList,
    RegularParam,
};

#[cfg(test)]
mod tests {
    //! Smoke checks for the parameter module path. Most parameter-list
    //! behavior is exercised by the richer tests in `declaration.rs`
    //! and `top_level.rs`; this module verifies the re-exports resolve
    //! and a basic ParameterList extraction through the new path.
    use super::*;
    use crate::parser::Parser;
    use crate::ast::cst_helpers::is_skip_token;
    use crate::parser::{Cst, NodeRef, Rule};
    use crate::ast::spanned::CommaSeparatedList;

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
    fn parameter_list_module_path_resolves() {
        // Smoke check — types are reachable through the parameter
        // module path.
        let _: Option<ParameterList> = None;
        let _: Option<ParameterDeclaration> = None;
        let items: CommaSeparatedList<ParameterDeclaration> = CommaSeparatedList::default();
        let _ = items.is_empty();
    }

    #[test]
    fn empty_parameter_list_helper() {
        // The empty() helper is used by `Declarator::from_inline` for
        // empty parameter lists like `void foo()`.
        let empty = ParameterList::empty();
        assert!(empty.is_empty());
        assert_eq!(empty.len(), 0);
    }

    #[test]
    fn parameter_list_extracts_from_function_declarator() {
        // `void foo(int a, string b) = 0;` — confirms ParameterList
        // round-trips through the new module path.
        let cst = parse("void foo(int a, string b) = 0;");
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
            .expect("parameter_list node");
        let list = ParameterList::from_cst(&cst, param_list_node).expect("list");
        assert_eq!(list.items.len(), 2);
    }
}
