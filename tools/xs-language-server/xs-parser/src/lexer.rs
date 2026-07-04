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
