package com.aomr.xs.highlight

import com.aomr.xs.XsLanguage
import com.intellij.psi.tree.IElementType

object XsHighlightingTokenTypes {
    @JvmField val KEYWORD = IElementType("XS_KEYWORD", XsLanguage.INSTANCE)
    @JvmField val NUMBER = IElementType("XS_NUMBER", XsLanguage.INSTANCE)
    @JvmField val DEFAULT = IElementType("XS_DEFAULT", XsLanguage.INSTANCE)
}
