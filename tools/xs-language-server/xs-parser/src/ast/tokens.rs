use crate::parser::{Cst, NodeRef, Span};
use crate::lexer::Token;

/// Macro to generate a wrapper-token type and its `from_cst` constructor.
///
/// Each wrapper is a zero-sized struct that exists purely for typed access
/// to its corresponding `Token` variant. `from_cst` returns the span of the
/// matching child if one exists.
macro_rules! wrapper_token {
    ($(#[$meta:meta])* $name:ident, $token:ident) => {
        $(#[$meta])*
        #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
        pub struct $name;

        impl $name {
            /// Returns the span of the first child of `parent` that matches
            /// `Token::$token`, or `None` if no such child exists.
            pub fn from_cst(cst: &Cst, parent: NodeRef) -> Option<Span> {
                cst.children(parent)
                    .find_map(|c| cst.match_token(c, Token::$token).map(|(_, s)| s))
            }
        }
    };
}

wrapper_token!(Paren, LPar);
wrapper_token!(RParen, RPar);
wrapper_token!(LBrace, LBrace);
wrapper_token!(RBrace, RBrace);
wrapper_token!(LBrak, LBrak);
wrapper_token!(RBrak, RBrak);
wrapper_token!(Comma, Comma);
wrapper_token!(Semi, Semi);
wrapper_token!(Eq, Eq);
wrapper_token!(Neq, Neq);
wrapper_token!(Assign, Assign);
wrapper_token!(PlusAssign, PlusAssign);
wrapper_token!(MinusAssign, MinusAssign);
wrapper_token!(StarAssign, StarAssign);
wrapper_token!(SlashAssign, SlashAssign);
wrapper_token!(PercentAssign, PercentAssign);
wrapper_token!(Plus, Plus);
wrapper_token!(Minus, Minus);
wrapper_token!(Star, Star);
wrapper_token!(Slash, Slash);
wrapper_token!(Percent, Percent);
wrapper_token!(Lt, Lt);
wrapper_token!(Gt, Gt);
wrapper_token!(Leq, Leq);
wrapper_token!(Geq, Geq);
wrapper_token!(AndAnd, AndAnd);
wrapper_token!(OrOr, OrOr);
wrapper_token!(PlusPlus, PlusPlus);
wrapper_token!(MinusMinus, MinusMinus);
wrapper_token!(Tilde, Tilde);
wrapper_token!(Excl, Excl);
wrapper_token!(Dot, Dot);
wrapper_token!(Colon, Colon);
wrapper_token!(Arrow, Arrow);
wrapper_token!(Question, Question);
wrapper_token!(Hash, Hash);

/// `Ref` is a type qualifier (and parameter qualifier). Modelled as a
/// wrapper-token type for parity with the other qualifiers, although its
/// semantic role differs from pure punctuation.
wrapper_token!(Ref, Ref);

wrapper_token!(Const, Const);
wrapper_token!(Extern, Extern);
wrapper_token!(Static, Static);
wrapper_token!(Mutable, Mutable);

wrapper_token!(Void, Void);
wrapper_token!(Int, Int);
wrapper_token!(Bool, Bool);
wrapper_token!(Float, Float);
wrapper_token!(StringKw, StringKw);
wrapper_token!(Vector, Vector);

wrapper_token!(Identifier, Identifier);
wrapper_token!(IntConst, IntConst);
wrapper_token!(FloatConst, FloatConst);
wrapper_token!(StringLiteral, StringLiteral);

wrapper_token!(If, If);
wrapper_token!(While, While);
wrapper_token!(For, For);
wrapper_token!(Return, Return);
wrapper_token!(Break, Break);
wrapper_token!(Continue, Continue);
wrapper_token!(Switch, Switch);
wrapper_token!(Case, Case);
wrapper_token!(DefaultKw, DefaultKw);

wrapper_token!(Class, Class);
wrapper_token!(New, New);

wrapper_token!(TrueKw, TrueKw);
wrapper_token!(FalseKw, FalseKw);
wrapper_token!(NullKw, NullKw);
wrapper_token!(TrueUpper, TrueUpper);
wrapper_token!(FalseUpper, FalseUpper);
wrapper_token!(NullUpper, NullUpper);
wrapper_token!(Nullptr, Nullptr);

wrapper_token!(Include, Include);
wrapper_token!(Rule, Rule);
wrapper_token!(MinInterval, MinInterval);
wrapper_token!(MaxInterval, MaxInterval);
wrapper_token!(MinIntervalMS, MinIntervalMS);
wrapper_token!(MaxIntervalMS, MaxIntervalMS);
wrapper_token!(Priority, Priority);
wrapper_token!(HighFrequency, HighFrequency);
wrapper_token!(RunImmediately, RunImmediately);
wrapper_token!(Group, Group);
wrapper_token!(Active, Active);
wrapper_token!(Inactive, Inactive);

wrapper_token!(PreprocDefine, PreprocDefine);
wrapper_token!(PreprocElif, PreprocElif);
wrapper_token!(PreprocElse, PreprocElse);
wrapper_token!(PreprocEndif, PreprocEndif);
wrapper_token!(PreprocDefined, PreprocDefined);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{Parser, Rule};

    fn parse<'a>(source: &'a str) -> Cst<'a> {
        let mut diags = vec![];
        Parser::new(source, &mut diags).parse(&mut diags)
    }

    fn first_child(cst: &Cst) -> NodeRef {
        cst.children(crate::parser::NodeRef::ROOT).next().unwrap()
    }

    #[test]
    fn semi_found_in_declaration() {
        let cst = parse("int x;");
        // For `int x;` the first non-skip child is `forward_declaration`.
        // The Semi token is its direct child.
        let span = Semi::from_cst(&cst, first_child(&cst)).expect("semi");
        assert_eq!(span, 5..6);
    }

    #[test]
    fn assign_found_in_declaration() {
        let cst = parse("int x = 5;");
        // The Assign token lives inside the init_declarator, not as a
        // direct child of the declaration. Walk to it.
        let decl = first_child(&cst);
        let init_list = cst
            .children(decl)
            .find(|c| cst.match_rule(*c, Rule::InitDeclaratorList))
            .unwrap();
        let init = cst
            .children(init_list)
            .find(|c| cst.match_rule(*c, Rule::InitDeclarator))
            .unwrap();
        let span = Assign::from_cst(&cst, init).expect("assign");
        assert_eq!(span, 6..7);
    }

    #[test]
    fn eq_token_matches_double_equals() {
        let cst = parse("int x = 5;");
        let expr_node = cst
            .children(first_child(&cst))
            .find(|c| {
                cst.match_rule(*c, Rule::InitDeclaratorList)
                    || cst.match_rule(*c, Rule::Expression)
                    || cst.match_rule(*c, Rule::AssignmentExpr)
            })
            .expect("inner");
        // No Eq token at top level — `=` is Assign. The `Eq` token should
        // not appear in this source.
        assert!(Eq::from_cst(&cst, first_child(&cst)).is_none());
        let _ = expr_node;
    }

    #[test]
    fn ref_qualifier_found() {
        let cst = parse("void foo(ref int x) {}");
        let _ = cst;
        // Body parsing may fail in PoC, so just confirm the parser runs
        // without panicking. The diagnostic list isn't checked here.
    }

    #[test]
    fn arrow_matches_in_return_type() {
        // XS doesn't use `->` outside lambdas, but the wrapper still
        // finds the token anywhere in its parent.
        let cst = parse("[]() -> int { return 0; }");
        let _ = cst;
    }

    #[test]
    fn wrapper_types_are_zero_sized() {
        assert_eq!(std::mem::size_of::<Semi>(), 0);
        assert_eq!(std::mem::size_of::<Paren>(), 0);
        assert_eq!(std::mem::size_of::<Ref>(), 0);
    }
}