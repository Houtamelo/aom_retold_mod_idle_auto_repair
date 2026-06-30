package com.aomr.xs.highlight

import com.aomr.xs.psi.XsTokenTypes
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class XsSyntaxHighlighterFactoryTest {

    @Test
    fun pluginXmlRegistersXsSyntaxHighlighterFactory() {
        val pluginXml = javaClass.classLoader.getResourceAsStream("META-INF/plugin.xml")
            ?: throw AssertionError("META-INF/plugin.xml not found on test classpath")
        val text = pluginXml.use { it.reader().readText() }
        assertTrue(
            "plugin.xml must register XsSyntaxHighlighterFactory for language xs",
            text.contains("""<lang.syntaxHighlighterFactory language="xs" implementationClass="com.aomr.xs.highlight.XsSyntaxHighlighterFactory""" )
        )
    }

    @Test
    fun factoryProducesHighlighter() {
        val factory = XsSyntaxHighlighterFactory()
        val highlighter = factory.getSyntaxHighlighter(null, null)
        assertNotNull("XsSyntaxHighlighterFactory must produce a SyntaxHighlighter", highlighter)
        assertTrue("Highlighter must be XsSyntaxHighlighter", highlighter is XsSyntaxHighlighter)
    }

    @Test
    fun highlighterMapsKeywordsStringsCommentsAndNumbers() {
        val factory = XsSyntaxHighlighterFactory()
        val highlighter = factory.getSyntaxHighlighter(null, null)
        val lexer = highlighter.highlightingLexer

        lexer.start(
            """
            // comment
            void main(int x = 1) { string s = "hi"; }
            """.trimIndent()
        )

        val seenKeys = mutableSetOf<com.intellij.openapi.editor.colors.TextAttributesKey>()
        var tokenCount = 0
        while (lexer.tokenType != null) {
            tokenCount++
            val keys = highlighter.getTokenHighlights(lexer.tokenType)
            if (lexer.tokenType != XsTokenTypes.IDENTIFIER) {
                assertTrue("Every non-identifier token must map to at least one TextAttributesKey", keys.isNotEmpty())
            }
            seenKeys.addAll(keys)
            lexer.advance()
        }
        assertTrue("Expected at least one token to tokenize", tokenCount > 0)

        assertTrue("Keyword highlight must be present", seenKeys.contains(XsTextAttributesKeys.XS_KEYWORD))
        assertTrue("String highlight must be present", seenKeys.contains(XsTextAttributesKeys.XS_STRING))
        assertTrue("Comment highlight must be present", seenKeys.contains(XsTextAttributesKeys.XS_COMMENT))
        assertTrue("Number highlight must be present", seenKeys.contains(XsTextAttributesKeys.XS_NUMBER))
    }

    @Test
    fun identifiersReceiveNoDefaultHighlight() {
        val factory = XsSyntaxHighlighterFactory()
        val highlighter = factory.getSyntaxHighlighter(null, null)
        val lexer = highlighter.highlightingLexer
        lexer.start("myVar")

        assertNotNull("Lexer must produce a token for identifier", lexer.tokenType)
        val keys = highlighter.getTokenHighlights(lexer.tokenType)
        assertTrue(
            "IDENTIFIER must not contribute a default color; let semantic tokens win",
            keys.isEmpty()
        )
        assertEquals("Identifier token must not be highlighted as keyword", false, keys.contains(XsTextAttributesKeys.XS_KEYWORD))
    }
}
