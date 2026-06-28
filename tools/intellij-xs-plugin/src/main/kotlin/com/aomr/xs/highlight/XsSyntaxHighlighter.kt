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
            XsTokenTypes.IDENTIFIER -> arrayOf(XsTextAttributesKeys.XS_IDENTIFIER)
            XsTokenTypes.WHITE_SPACE -> arrayOf(XsTextAttributesKeys.XS_DEFAULT)
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
