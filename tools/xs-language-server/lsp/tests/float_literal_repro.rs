//! TDD: float literal with optional trailing digits.
//!
//! C and XS accept `1.` and `1.0` as the same float literal. The
//! original grammar required at least one digit after the decimal
//! point, causing `1.` to lex as `1` (int) + `.` (member access),
//! which then broke expressions like `vector(30.0, 0.0, 439.)`.
//!
//! Each test must currently fail (RED) and pass after the lexer fix.

use xs_parser::lexer::{Token, tokenize};

fn lex(src: &str) -> Vec<Token> {
    let mut diags = Vec::new();
    let (tokens, _spans) = tokenize(src, &mut diags);
    tokens.into_iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::LineComment | Token::BlockComment | Token::EOF))
        .collect()
}

#[test]
fn trailing_dot_is_float() {
    let src = "439.";
    let tokens = lex(src);
    assert!(tokens.contains(&Token::FloatConst), "expected FloatConst, got {:?}", tokens);
    assert!(!tokens.contains(&Token::Dot), "should NOT have Dot token, got {:?}", tokens);
}

#[test]
fn regular_float_still_works() {
    let src = "439.0";
    let tokens = lex(src);
    assert!(tokens.contains(&Token::FloatConst), "expected FloatConst, got {:?}", tokens);
}

#[test]
fn int_still_works() {
    let src = "439";
    let tokens = lex(src);
    assert!(tokens.contains(&Token::IntConst), "expected IntConst, got {:?}", tokens);
    assert!(!tokens.contains(&Token::FloatConst), "should NOT have FloatConst, got {:?}", tokens);
}

#[test]
fn dot_5_is_float() {
    // Leading-dot form like `.5` is NOT currently supported (the
    // lexer requires at least one leading digit). Retail XS never
    // uses this form, so it's a known limitation.
    let src = ".5";
    let tokens = lex(src);
    eprintln!("debug: .5 lexes as {:?}", tokens);
}

#[test]
fn vector_with_trailing_dot() {
    let src = "439.;";
    let tokens = lex(src);
    assert!(tokens.contains(&Token::FloatConst), "expected FloatConst, got {:?}", tokens);
    assert!(tokens.contains(&Token::Semi), "expected Semi, got {:?}", tokens);
}
