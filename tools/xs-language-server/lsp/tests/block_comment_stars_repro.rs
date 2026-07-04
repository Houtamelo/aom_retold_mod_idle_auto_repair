//! TDD: block comments with star runs (5+ stars in a row).
//!
//! The XS retail game uses decorative block comments like:
//! `/* ***** Some text ***** */`
//! where there are 5+ stars in a row on both sides of the comment.
//! The current regex `r"/\\*([^*]|\\*+[^*/])*\\*/"` fails to match `*****` followed by `/`
//! because the `\\*+[^*/]` alternative requires a non-star, non-slash character after the
//! run of stars. The 4th star is followed by the 5th star (excluded by `[^*/]`), and
//! the 5th star is followed by `/` (excluded by `[^*/]`). So the regex cannot match
//! 5 stars followed by `/`.
//!
//! Each test should currently fail (RED) and pass after the regex fix.

use xs_parser::lexer::{tokenize, Token};
use xs_parser::parser::Diagnostic;

fn lex_all(src: &str) -> (Vec<Token>, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let (tokens, _spans) = tokenize(src, &mut diags);
    (tokens, diags)
}

fn lex_non_trivia(src: &str) -> Vec<Token> {
    let (tokens, _) = lex_all(src);
    tokens
        .into_iter()
        .filter(|t| !matches!(t, Token::Whitespace | Token::LineComment | Token::BlockComment | Token::EOF))
        .collect()
}

#[test]
fn five_stars_each_side() {
    let src = "/***** Some text *****/\nint x = 1;";
    let (tokens, diags) = lex_all(src);
    println!("tokens: {tokens:?}");
    println!("diags: {diags:?}");
    // The block comment should be lexed as a single BlockComment token.
    let has_block_comment = tokens.iter().any(|t| matches!(t, Token::BlockComment));
    assert!(has_block_comment, "expected a BlockComment token, got {tokens:?}");

    // After filtering out the comment, we should see only: Int, Identifier, Assign, IntConst, Semi
    let interesting = lex_non_trivia(src);
    println!("non-trivia: {interesting:?}");
    let no_errors = interesting.iter().all(|t| !matches!(t, Token::Error));
    assert!(no_errors, "no Error tokens expected, got {interesting:?}");

    // No lexer diagnostics expected
    assert!(diags.is_empty(), "no lexer diagnostics expected, got {diags:?}");
}

#[test]
fn three_stars_each_side() {
    let src = "/*** Some text ***/\nint y = 2;";
    let (tokens, diags) = lex_all(src);
    println!("tokens: {tokens:?}");
    println!("diags: {diags:?}");
    let has_block_comment = tokens.iter().any(|t| matches!(t, Token::BlockComment));
    assert!(has_block_comment, "expected a BlockComment token, got {tokens:?}");

    let no_errors = lex_non_trivia(src).iter().all(|t| !matches!(t, Token::Error));
    assert!(no_errors, "no Error tokens expected");
    assert!(diags.is_empty(), "no lexer diagnostics expected, got {diags:?}");
}

#[test]
fn comment_only_stars() {
    let src = "/*****/";
    let (tokens, diags) = lex_all(src);
    println!("tokens: {tokens:?}");
    println!("diags: {diags:?}");
    let has_block_comment = tokens.iter().any(|t| matches!(t, Token::BlockComment));
    assert!(has_block_comment, "expected a BlockComment token, got {tokens:?}");

    let no_errors = lex_non_trivia(src).iter().all(|t| !matches!(t, Token::Error));
    assert!(no_errors, "no Error tokens expected");
    assert!(diags.is_empty(), "no lexer diagnostics expected, got {diags:?}");
}

#[test]
fn comment_with_text_and_inline_stars() {
    // Common in xs: /*** text * mid * end ***/
    let src = "/*** text * mid * end ***/";
    let (tokens, diags) = lex_all(src);
    println!("tokens: {tokens:?}");
    println!("diags: {diags:?}");
    let has_block_comment = tokens.iter().any(|t| matches!(t, Token::BlockComment));
    assert!(has_block_comment, "expected a BlockComment token, got {tokens:?}");

    let no_errors = lex_non_trivia(src).iter().all(|t| !matches!(t, Token::Error));
    assert!(no_errors, "no Error tokens expected");
    assert!(diags.is_empty(), "no lexer diagnostics expected, got {diags:?}");
}
