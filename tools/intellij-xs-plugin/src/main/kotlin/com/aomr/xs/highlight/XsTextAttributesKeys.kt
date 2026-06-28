package com.aomr.xs.highlight

import com.intellij.openapi.editor.DefaultLanguageHighlighterColors
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.editor.colors.TextAttributesKey.createTextAttributesKey

object XsTextAttributesKeys {
    val XS_KEYWORD: TextAttributesKey = createTextAttributesKey("XS_KEYWORD", DefaultLanguageHighlighterColors.KEYWORD)
    val XS_STRING: TextAttributesKey = createTextAttributesKey("XS_STRING", DefaultLanguageHighlighterColors.STRING)
    val XS_COMMENT: TextAttributesKey = createTextAttributesKey("XS_COMMENT", DefaultLanguageHighlighterColors.LINE_COMMENT)
    val XS_NUMBER: TextAttributesKey = createTextAttributesKey("XS_NUMBER", DefaultLanguageHighlighterColors.NUMBER)
    val XS_IDENTIFIER: TextAttributesKey = createTextAttributesKey("XS_IDENTIFIER", DefaultLanguageHighlighterColors.IDENTIFIER)
    val XS_DEFAULT: TextAttributesKey = createTextAttributesKey("XS_DEFAULT", DefaultLanguageHighlighterColors.IDENTIFIER)
}
