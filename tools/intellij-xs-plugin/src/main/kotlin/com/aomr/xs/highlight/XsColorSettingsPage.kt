package com.aomr.xs.highlight

import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.fileTypes.SyntaxHighlighter
import com.intellij.openapi.options.colors.AttributesDescriptor
import com.intellij.openapi.options.colors.ColorDescriptor
import com.intellij.openapi.options.colors.ColorSettingsPage
import javax.swing.Icon

/**
 * Color settings page for the XS language.
 *
 * The page is labeled **XS** in **Settings → Editor → Color Scheme**. It exposes:
 * - the original six lexical categories (Keyword, String, Comment, Number, Identifier, Default);
 * - twelve inherited platform categories grouped under **Code**, **Errors and Warnings**, and
 *   **Braces and Operators** using the IntelliJ `//` nested-group convention.
 *
 * `//` in an [AttributesDescriptor] display name creates a collapsible subcategory. For example,
 * `"Braces and Operators//Comma"` places *Comma* under the *Braces and Operators* group.
 */
class XsColorSettingsPage : ColorSettingsPage {

    override fun getDisplayName(): String = "XS"

    override fun getIcon(): Icon? = null

    override fun getHighlighter(): SyntaxHighlighter = XsSyntaxHighlighter()

    override fun getDemoText(): String = DEMO_TEXT

    override fun getAttributeDescriptors(): Array<AttributesDescriptor> = DESCRIPTORS

    override fun getColorDescriptors(): Array<ColorDescriptor> = ColorDescriptor.EMPTY_ARRAY

    override fun getAdditionalHighlightingTagToDescriptorMap(): Map<String, TextAttributesKey> = TAG_MAP

    companion object {

        /**
         * Descriptors shown in the XS color scheme page.
         *
         * Existing lexical categories are listed first, followed by inherited platform categories
         * grouped via `//`. The order here matches the rendered list order.
         */
        private val DESCRIPTORS = arrayOf(
            // Existing lexical categories
            AttributesDescriptor("Keyword", XsTextAttributesKeys.XS_KEYWORD),
            AttributesDescriptor("String", XsTextAttributesKeys.XS_STRING),
            AttributesDescriptor("Comment", XsTextAttributesKeys.XS_COMMENT),
            AttributesDescriptor("Number", XsTextAttributesKeys.XS_NUMBER),
            AttributesDescriptor("Identifier", XsTextAttributesKeys.XS_IDENTIFIER),
            AttributesDescriptor("Default", XsTextAttributesKeys.XS_DEFAULT),
            // Inherited General / Language Defaults categories
            AttributesDescriptor("Code//Identifier under caret", XsTextAttributes.IDENTIFIER_UNDER_CARET),
            AttributesDescriptor("Code//Matched brace", XsTextAttributes.MATCHED_BRACE),
            AttributesDescriptor("Code//Unmatched brace", XsTextAttributes.UNMATCHED_BRACE),
            AttributesDescriptor("Errors and Warnings//Unknown symbol", XsTextAttributes.UNKNOWN_SYMBOL),
            AttributesDescriptor("Braces and Operators//Braces", XsTextAttributes.BRACES),
            AttributesDescriptor("Braces and Operators//Brackets", XsTextAttributes.BRACKETS),
            AttributesDescriptor("Braces and Operators//Comma", XsTextAttributes.COMMA),
            AttributesDescriptor("Braces and Operators//Dot", XsTextAttributes.DOT),
            AttributesDescriptor("Braces and Operators//Operation sign", XsTextAttributes.OPERATION_SIGN),
            AttributesDescriptor("Braces and Operators//Overloaded operator", XsTextAttributes.OVERLOADED_OPERATOR),
            AttributesDescriptor("Braces and Operators//Parentheses", XsTextAttributes.PARENTHESES),
            AttributesDescriptor("Braces and Operators//Semi-colon", XsTextAttributes.SEMI_COLON)
        )

        /**
         * Mapping from synthetic XML tags in [DEMO_TEXT] to the keys used for the preview.
         *
         * Tags such as `<brace>`, `<bracket>`, and `<op>` let the preview demonstrate the
         * newly-added inherited categories alongside the original lexical categories.
         */
        private val TAG_MAP = mapOf(
            "keyword" to XsTextAttributesKeys.XS_KEYWORD,
            "string" to XsTextAttributesKeys.XS_STRING,
            "comment" to XsTextAttributesKeys.XS_COMMENT,
            "number" to XsTextAttributesKeys.XS_NUMBER,
            "identifier" to XsTextAttributesKeys.XS_IDENTIFIER,
            "brace" to XsTextAttributes.BRACES,
            "bracket" to XsTextAttributes.BRACKETS,
            "paren" to XsTextAttributes.PARENTHESES,
            "comma" to XsTextAttributes.COMMA,
            "semicolon" to XsTextAttributes.SEMI_COLON,
            "dot" to XsTextAttributes.DOT,
            "op" to XsTextAttributes.OPERATION_SIGN
        )

        /** Preview snippet rendered on the XS color scheme page. */
        private val DEMO_TEXT = """
            <comment>// XS sample snippet</comment>
            <keyword>void</keyword> <identifier>main</identifier><paren>(</paren><paren>)</paren> <brace>{</brace>
                <keyword>int</keyword> <identifier>count</identifier><bracket>[</bracket><number>42</number><bracket>]</bracket><semicolon>;</semicolon>
                <keyword>string</keyword> <identifier>greeting</identifier> <op>=</op> <string>"hello world"</string><semicolon>;</semicolon>
                <identifier>obj</identifier><dot>.</dot><identifier>field</identifier><semicolon>;</semicolon>
                <keyword>float</keyword> <identifier>items</identifier><comma>,</comma> <identifier>total</identifier><semicolon>;</semicolon>
            <brace>}</brace>
        """.trimIndent()
    }
}
