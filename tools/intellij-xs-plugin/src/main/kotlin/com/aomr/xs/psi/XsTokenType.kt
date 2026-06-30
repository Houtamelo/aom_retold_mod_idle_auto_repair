package com.aomr.xs.psi

import com.aomr.xs.XsLanguage
import com.intellij.psi.tree.IElementType

object XsTokenTypes {
    @JvmField val WHITE_SPACE = IElementType("XS_WHITE_SPACE", XsLanguage.INSTANCE)
    @JvmField val LINE_COMMENT = IElementType("XS_LINE_COMMENT", XsLanguage.INSTANCE)
    @JvmField val BLOCK_COMMENT = IElementType("XS_BLOCK_COMMENT", XsLanguage.INSTANCE)
    @JvmField val LBRACE = IElementType("XS_LBRACE", XsLanguage.INSTANCE)
    @JvmField val RBRACE = IElementType("XS_RBRACE", XsLanguage.INSTANCE)
    @JvmField val LPAREN = IElementType("XS_LPAREN", XsLanguage.INSTANCE)
    @JvmField val RPAREN = IElementType("XS_RPAREN", XsLanguage.INSTANCE)
    @JvmField val LBRACKET = IElementType("XS_LBRACKET", XsLanguage.INSTANCE)
    @JvmField val RBRACKET = IElementType("XS_RBRACKET", XsLanguage.INSTANCE)
    @JvmField val STRING_QUOTE = IElementType("XS_STRING_QUOTE", XsLanguage.INSTANCE)
    @JvmField val CHAR_QUOTE = IElementType("XS_CHAR_QUOTE", XsLanguage.INSTANCE)
    @JvmField val STRING_LITERAL = IElementType("XS_STRING_LITERAL", XsLanguage.INSTANCE)
    @JvmField val CHAR_LITERAL = IElementType("XS_CHAR_LITERAL", XsLanguage.INSTANCE)
    @JvmField val IDENTIFIER = IElementType("XS_IDENTIFIER", XsLanguage.INSTANCE)
    @JvmField val COMMA = IElementType("XS_COMMA", XsLanguage.INSTANCE)
    @JvmField val DOT = IElementType("XS_DOT", XsLanguage.INSTANCE)
    @JvmField val SEMI_COLON = IElementType("XS_SEMI_COLON", XsLanguage.INSTANCE)
    @JvmField val OPERATION_SIGN = IElementType("XS_OPERATION_SIGN", XsLanguage.INSTANCE)
    @JvmField val XS_OTHER = IElementType("XS_OTHER", XsLanguage.INSTANCE)
}
