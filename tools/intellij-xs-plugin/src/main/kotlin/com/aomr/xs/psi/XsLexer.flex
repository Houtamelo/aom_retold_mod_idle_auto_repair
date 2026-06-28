package com.aomr.xs.psi;

import com.intellij.lexer.FlexLexer;
import com.intellij.psi.tree.IElementType;

%%

%class XsLexer
%implements FlexLexer
%unicode
%function advance
%type IElementType

%state IN_STRING
%state IN_CHAR

WHITE_SPACE=[ \t\n\f\r]+
IDENTIFIER=[A-Za-z_][A-Za-z_0-9]*

%%

<YYINITIAL> {
    {WHITE_SPACE}     { return XsTokenTypes.WHITE_SPACE; }
    "//" [^\r\n]*     { return XsTokenTypes.LINE_COMMENT; }
    "/*" ~"*/"        { return XsTokenTypes.BLOCK_COMMENT; }
    "{"               { return XsTokenTypes.LBRACE; }
    "}"               { return XsTokenTypes.RBRACE; }
    "("               { return XsTokenTypes.LPAREN; }
    ")"               { return XsTokenTypes.RPAREN; }
    "["               { return XsTokenTypes.LBRACKET; }
    "]"               { return XsTokenTypes.RBRACKET; }
    ","               { return XsTokenTypes.COMMA; }
    "\""              { yybegin(IN_STRING); return XsTokenTypes.STRING_QUOTE; }
    "'"               { yybegin(IN_CHAR); return XsTokenTypes.CHAR_QUOTE; }
    {IDENTIFIER}      { return XsTokenTypes.IDENTIFIER; }
    [^]               { return XsTokenTypes.XS_OTHER; }
}

<IN_STRING> {
    "\""              { yybegin(YYINITIAL); return XsTokenTypes.STRING_QUOTE; }
    [^\"\\]+         { return XsTokenTypes.STRING_LITERAL; }
    "\\" .           { return XsTokenTypes.STRING_LITERAL; }
}

<IN_CHAR> {
    "'"               { yybegin(YYINITIAL); return XsTokenTypes.CHAR_QUOTE; }
    [^\'\\]          { return XsTokenTypes.CHAR_LITERAL; }
    "\\" .           { return XsTokenTypes.CHAR_LITERAL; }
}
