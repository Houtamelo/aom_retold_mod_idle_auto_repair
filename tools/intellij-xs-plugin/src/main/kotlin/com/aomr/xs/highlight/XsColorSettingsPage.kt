package com.aomr.xs.highlight

import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.fileTypes.SyntaxHighlighter
import com.intellij.openapi.options.colors.AttributesDescriptor
import com.intellij.openapi.options.colors.ColorDescriptor
import com.intellij.openapi.options.colors.ColorSettingsPage
import javax.swing.Icon

class XsColorSettingsPage : ColorSettingsPage {

    override fun getDisplayName(): String = "xs"

    override fun getIcon(): Icon? = null

    override fun getHighlighter(): SyntaxHighlighter = XsSyntaxHighlighter()

    override fun getDemoText(): String = DEMO_TEXT

    override fun getAttributeDescriptors(): Array<AttributesDescriptor> = DESCRIPTORS

    override fun getColorDescriptors(): Array<ColorDescriptor> = ColorDescriptor.EMPTY_ARRAY

    override fun getAdditionalHighlightingTagToDescriptorMap(): Map<String, TextAttributesKey> = TAG_MAP

    companion object {
        private val DESCRIPTORS = arrayOf(
            AttributesDescriptor("Keyword", XsTextAttributesKeys.XS_KEYWORD),
            AttributesDescriptor("String", XsTextAttributesKeys.XS_STRING),
            AttributesDescriptor("Comment", XsTextAttributesKeys.XS_COMMENT),
            AttributesDescriptor("Number", XsTextAttributesKeys.XS_NUMBER),
            AttributesDescriptor("Identifier", XsTextAttributesKeys.XS_IDENTIFIER),
            AttributesDescriptor("Default", XsTextAttributesKeys.XS_DEFAULT)
        )

        private val TAG_MAP = mapOf(
            "keyword" to XsTextAttributesKeys.XS_KEYWORD,
            "string" to XsTextAttributesKeys.XS_STRING,
            "comment" to XsTextAttributesKeys.XS_COMMENT,
            "number" to XsTextAttributesKeys.XS_NUMBER,
            "identifier" to XsTextAttributesKeys.XS_IDENTIFIER
        )

        private val DEMO_TEXT = """
            <comment>// XS sample snippet</comment>
            <keyword>void</keyword> <identifier>main</identifier>() {
                <keyword>int</keyword> <identifier>count</identifier> = <number>42</number>;
                <keyword>string</keyword> <identifier>greeting</identifier> = <string>"hello world"</string>;
            }
        """.trimIndent()
    }
}
