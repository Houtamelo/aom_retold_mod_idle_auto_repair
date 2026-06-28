package com.aomr.xs.editor

import com.aomr.xs.psi.XsTokenTypes
import com.intellij.lang.BracePair
import com.intellij.lang.PairedBraceMatcher
import com.intellij.psi.PsiFile
import com.intellij.psi.tree.IElementType

class XsBraceMatcher : PairedBraceMatcher {

    override fun getPairs(): Array<BracePair> = PAIRS

    override fun isPairedBracesAllowedBeforeType(lbraceType: IElementType, contextType: IElementType?): Boolean = true

    override fun getCodeConstructStart(file: PsiFile?, openingBraceOffset: Int): Int = openingBraceOffset

    companion object {
        private val PAIRS = arrayOf(
            BracePair(XsTokenTypes.LPAREN, XsTokenTypes.RPAREN, true),
            BracePair(XsTokenTypes.LBRACKET, XsTokenTypes.RBRACKET, true),
            BracePair(XsTokenTypes.LBRACE, XsTokenTypes.RBRACE, true)
        )
    }
}
