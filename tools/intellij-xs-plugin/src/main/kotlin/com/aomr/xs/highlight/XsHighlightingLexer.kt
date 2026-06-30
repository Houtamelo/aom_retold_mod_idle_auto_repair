package com.aomr.xs.highlight

import com.aomr.xs.psi.XsLexerAdapter
import com.aomr.xs.psi.XsTokenTypes
import com.intellij.lexer.DelegateLexer
import com.intellij.psi.tree.IElementType

class XsHighlightingLexer : DelegateLexer(XsLexerAdapter()) {

    override fun getTokenType(): IElementType? {
        val type = delegate.tokenType ?: return null
        if (type == XsTokenTypes.IDENTIFIER) {
            val text = tokenText.toString().lowercase()
            when {
                text in XS_KEYWORDS -> return XsHighlightingTokenTypes.KEYWORD
                NUMBER_REGEX.matches(text) -> return XsHighlightingTokenTypes.NUMBER
            }
        } else if (type == XsTokenTypes.XS_OTHER && NUMBER_REGEX.matches(tokenText)) {
            return XsHighlightingTokenTypes.NUMBER
        }
        return type
    }

    companion object {
        private val XS_KEYWORDS = setOf(
            "void", "int", "bool", "float", "string", "vector",
            "if", "else", "while", "for", "return", "mutable", "extern",
            "const", "rule", "class", "ref", "default", "switch", "case",
            "break", "continue", "true", "false",
            "include"
        )

        private val NUMBER_REGEX = Regex("[0-9]+(\\.[0-9]+)?([eE][+-]?[0-9]+)?")
    }
}
