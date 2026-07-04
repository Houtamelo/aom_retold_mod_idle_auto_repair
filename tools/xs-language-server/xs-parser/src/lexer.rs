use super::parser::{Diagnostic, Span};
use codespan_reporting::diagnostic::Label;
use logos::Logos;

#[derive(Debug, Clone, PartialEq, Default)]
pub enum LexerError {
    #[default]
    Invalid,
    // TODO: add more errors if required
}

impl LexerError {
    pub fn into_diagnostic(self, span: Span) -> Diagnostic {
        match self {
            Self::Invalid => Diagnostic::error()
                .with_message("invalid token")
                .with_label(Label::primary((), span)),
        }
    }
}

// TODO: implement lexer
#[allow(clippy::upper_case_acronyms)]
#[derive(Logos, Debug, PartialEq, Copy, Clone)]
#[logos(error = LexerError)]
pub enum Token {
    EOF,
    #[token("void")]
    Void,
    #[token("int")]
    Int,
    #[token("bool")]
    Bool,
    #[token("float")]
    Float,
    #[token("string")]
    StringKw,
    #[token("vector")]
    Vector,
    #[token("extern")]
    Extern,
    #[token("static")]
    Static,
    #[token("mutable")]
    Mutable,
    #[token("const")]
    Const,
    #[token("ref")]
    Ref,
    #[token("if")]
    If,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("return")]
    Return,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("switch")]
    Switch,
    #[token("case")]
    Case,
    #[token("default")]
    DefaultKw,
    #[token("rule")]
    Rule,
    #[token("minInterval")]
    MinInterval,
    #[token("maxInterval")]
    MaxInterval,
    #[token("minIntervalMS")]
    MinIntervalMS,
    #[token("maxIntervalMS")]
    MaxIntervalMS,
    #[token("priority")]
    Priority,
    #[token("highFrequency")]
    HighFrequency,
    #[token("runImmediately")]
    RunImmediately,
    #[token("group")]
    Group,
    #[token("active")]
    Active,
    #[token("inactive")]
    Inactive,
    #[token("class")]
    Class,
    #[token("new")]
    New,
    #[token("include")]
    Include,
    #[token("#")]
    Hash,
    #[token("define")]
    PreprocDefine,
    #[token("elif")]
    PreprocElif,
    #[token("else")]
    PreprocElse,
    #[token("endif")]
    PreprocEndif,
    #[token("defined")]
    PreprocDefined,
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Identifier,
    #[regex(r"[0-9]+")]
    IntConst,
    #[regex(r"[0-9]+\.[0-9]+")]
    FloatConst,
    #[regex(r#""[^"\\]*(\\.[^"\\]*)*""#)]
    StringLiteral,
    #[token("true")]
    TrueKw,
    #[token("false")]
    FalseKw,
    #[token("null")]
    NullKw,
    #[token("TRUE")]
    TrueUpper,
    #[token("FALSE")]
    FalseUpper,
    #[token("NULL")]
    NullUpper,
    #[token("nullptr")]
    Nullptr,
    #[token("(")]
    LPar,
    #[token(")")]
    RPar,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBrak,
    #[token("]")]
    RBrak,
    #[token(",")]
    Comma,
    #[token(";")]
    Semi,
    #[token(":")]
    Colon,
    #[token(".")]
    Dot,
    #[token("->")]
    Arrow,
    #[token("?")]
    Question,
    #[token("=")]
    Assign,
    #[token("+=")]
    PlusAssign,
    #[token("-=")]
    MinusAssign,
    #[token("*=")]
    StarAssign,
    #[token("/=")]
    SlashAssign,
    #[token("%=")]
    PercentAssign,
    #[token("&=")]
    AmpAssign,
    #[token("|=")]
    PipeAssign,
    #[token("^=")]
    XorAssign,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("==")]
    Eq,
    #[token("!=")]
    Neq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("<=")]
    Leq,
    #[token(">=")]
    Geq,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("&")]
    Amp,
    #[token("|")]
    Pipe,
    #[token("^")]
    Caret,
    #[token("~")]
    Tilde,
    #[token("!")]
    Excl,
    #[token("++")]
    PlusPlus,
    #[token("--")]
    MinusMinus,
    #[regex(r"//[^\n]*", allow_greedy = true)]
    LineComment,
    #[regex(r"/\*([^*]|\*+[^*/])*\*/")]
    BlockComment,
    #[regex(r"[ \t\n\r\f]+")]
    Whitespace,
    Error,
}

// TODO: extend tokenization (e.g. check for mismatched parentheses)
pub fn tokenize(
    source: &str,
    diags: &mut Vec<Diagnostic>,
) -> (Vec<Token>, Vec<Span>) {
    let lexer = Token::lexer(source);
    let mut tokens = vec![];
    let mut spans = vec![];

    for (token, span) in lexer.spanned() {
        match token {
            Ok(token) => {
                tokens.push(token);
            }
            Err(err) => {
                diags.push(err.into_diagnostic(span.clone()));
                tokens.push(Token::Error);
            }
        }
        spans.push(span);
    }
    (tokens, spans)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: tokenize `source` and return the vector of non-skip/Error tokens
    /// together with a count of lexer errors.
    fn tokenize_non_skip(source: &str) -> (Vec<Token>, usize) {
        let mut diags = Vec::new();
        let (tokens, _spans) = tokenize(source, &mut diags);
        let error_count = tokens.iter().filter(|t| **t == Token::Error).count();
        let filtered: Vec<_> = tokens
            .into_iter()
            .filter(|t| !matches!(t, Token::Whitespace | Token::LineComment | Token::BlockComment | Token::EOF))
            .collect();
        (filtered, error_count)
    }

    #[test]
    fn block_comment_with_multiple_stars() {
        let source = "/***** Increase army sizes *****/\nint x = 1;";
        let (tokens, errors) = tokenize_non_skip(source);
        assert_eq!(errors, 0, "block comment with repeated stars should lex cleanly");
        assert!(tokens.contains(&Token::Int));
        assert!(tokens.contains(&Token::Identifier));
        assert!(tokens.contains(&Token::IntConst));
    }

    #[test]
    fn block_comment_multiline_preserve_offsets() {
        let source = "/* line1\n * line2\n */\nint y = 2;";
        let (tokens, errors) = tokenize_non_skip(source);
        assert_eq!(errors, 0, "multi-line block comment should lex cleanly");
        assert!(tokens.contains(&Token::Int));
        assert!(tokens.contains(&Token::IntConst));
    }

    #[test]
    fn block_comment_empty() {
        let source = "/**/\nint z = 3;";
        let (tokens, errors) = tokenize_non_skip(source);
        assert_eq!(errors, 0, "empty block comment should lex cleanly");
        assert!(tokens.contains(&Token::Int));
    }

    #[test]
    fn adjacent_block_comments_parsed_separately() {
        let source = "/* a */ /* b */ int w = 4;";
        let (tokens, errors) = tokenize_non_skip(source);
        assert_eq!(errors, 0, "adjacent block comments should produce two skip tokens");
        assert!(tokens.contains(&Token::Int));
        assert!(tokens.contains(&Token::IntConst));
    }

    #[test]
    fn debug_godpowers_header() {
        let source = "//==============================================================================\n/* godpowers.xs\n\n   This file contains all logic for the management of god powers.\n\n*/\n//==============================================================================";
        let mut diags = Vec::new();
        let (tokens, spans) = tokenize(source, &mut diags);
        eprintln!("diags: {:?}", diags);
        for (t, s) in tokens.iter().zip(spans.iter()) {
            eprintln!("{:?} @ {}..{}: {:?}", t, s.start, s.end, &source[s.clone()]);
        }
    }

    #[test]
    fn block_comments_do_not_merge_across_code() {
        let source = "/* preInit() */\nvoid preInit()\n{\n}\n/* postInit() */\nvoid postInit()\n{\n}";
        let mut diags = Vec::new();
        let (tokens, spans) = tokenize(source, &mut diags);
        assert!(diags.is_empty(), "source with two block comments should lex without errors");
        let comment_spans: Vec<_> = tokens
            .iter()
            .zip(spans.iter())
            .filter(|(t, _)| **t == Token::BlockComment)
            .map(|(_, s)| s.clone())
            .collect();
        assert_eq!(
            comment_spans.len(),
            2,
            "expected two separate block comment tokens, got {:?}",
            comment_spans
        );
        // Each comment should be short; a greedy regex would merge them into one
        // span covering the whole file.
        assert!(
            comment_spans[0].end - comment_spans[0].start < 20,
            "first block comment span unexpectedly long: {:?}",
            comment_spans[0]
        );
        assert!(
            comment_spans[1].end - comment_spans[1].start < 20,
            "second block comment span unexpectedly long: {:?}",
            comment_spans[1]
        );
    }
}
