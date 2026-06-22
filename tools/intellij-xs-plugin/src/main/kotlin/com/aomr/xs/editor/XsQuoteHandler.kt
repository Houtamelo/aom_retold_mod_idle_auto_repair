package com.aomr.xs.editor

import com.aomr.xs.psi.XsTokenTypes
import com.intellij.codeInsight.editorActions.QuoteHandler
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.editor.highlighter.HighlighterIterator

class XsQuoteHandler : QuoteHandler {

    override fun isOpeningQuote(iterator: HighlighterIterator?, offset: Int): Boolean =
        isQuoteToken(iterator?.tokenType)

    override fun isClosingQuote(iterator: HighlighterIterator?, offset: Int): Boolean =
        isQuoteToken(iterator?.tokenType)

    override fun hasNonClosedLiteral(editor: Editor?, iterator: HighlighterIterator?, offset: Int): Boolean = false

    override fun isInsideLiteral(iterator: HighlighterIterator?): Boolean =
        isQuoteToken(iterator?.tokenType)

    private fun isQuoteToken(tokenType: Any?): Boolean =
        tokenType == XsTokenTypes.STRING_QUOTE || tokenType == XsTokenTypes.CHAR_QUOTE
}
