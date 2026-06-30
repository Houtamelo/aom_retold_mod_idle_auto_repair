package com.aomr.xs.highlight

import com.aomr.xs.psi.XsTokenTypes
import com.intellij.lexer.Lexer
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.fileTypes.SyntaxHighlighterBase
import com.intellij.psi.tree.IElementType

class XsSyntaxHighlighter : SyntaxHighlighterBase() {

    override fun getHighlightingLexer(): Lexer = XsHighlightingLexer()

    override fun getTokenHighlights(tokenType: IElementType?): Array<TextAttributesKey> {
        if (tokenType == null) return EMPTY_KEYS
        return when (tokenType) {
            XsTokenTypes.LINE_COMMENT, XsTokenTypes.BLOCK_COMMENT -> arrayOf(XsTextAttributesKeys.XS_COMMENT)
            XsTokenTypes.STRING_LITERAL, XsTokenTypes.CHAR_LITERAL,
            XsTokenTypes.STRING_QUOTE, XsTokenTypes.CHAR_QUOTE -> arrayOf(XsTextAttributesKeys.XS_STRING)
            XsTokenTypes.IDENTIFIER -> EMPTY_KEYS
            XsTokenTypes.WHITE_SPACE -> arrayOf(XsTextAttributesKeys.XS_DEFAULT)
            XsTokenTypes.LBRACE, XsTokenTypes.RBRACE -> arrayOf(XsTextAttributes.BRACES)
            XsTokenTypes.LBRACKET, XsTokenTypes.RBRACKET -> arrayOf(XsTextAttributes.BRACKETS)
            XsTokenTypes.LPAREN, XsTokenTypes.RPAREN -> arrayOf(XsTextAttributes.PARENTHESES)
            XsTokenTypes.COMMA -> arrayOf(XsTextAttributes.COMMA)
            XsTokenTypes.DOT -> arrayOf(XsTextAttributes.DOT)
            XsTokenTypes.SEMI_COLON -> arrayOf(XsTextAttributes.SEMI_COLON)
            XsTokenTypes.OPERATION_SIGN -> arrayOf(XsTextAttributes.OPERATION_SIGN, XsTextAttributes.OVERLOADED_OPERATOR)
            XsHighlightingTokenTypes.KEYWORD -> arrayOf(XsTextAttributesKeys.XS_KEYWORD)
            XsHighlightingTokenTypes.NUMBER -> arrayOf(XsTextAttributesKeys.XS_NUMBER)
            XsHighlightingTokenTypes.DEFAULT -> arrayOf(XsTextAttributesKeys.XS_DEFAULT)
            else -> arrayOf(XsTextAttributesKeys.XS_DEFAULT)
        }
    }

    companion object {
        private val EMPTY_KEYS = arrayOf<TextAttributesKey>()
    }
}
