package com.aomr.xs.textmate

import com.aomr.xs.highlight.XsHighlightingLexer
import com.aomr.xs.highlight.XsHighlightingTokenTypes
import com.aomr.xs.psi.XsTokenTypes
import com.intellij.psi.tree.IElementType
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class XsTextMateIncludeKeywordTest {

    @Test
    fun test_include_keyword_in_textmate_grammar() {
        val grammar = javaClass.classLoader.getResourceAsStream("syntaxes/xs.tmLanguage.json")
            ?: throw AssertionError("xs.tmLanguage.json not found on test classpath")
        val text = grammar.use { it.reader().readText() }
        // The TextMate grammar stores the keyword regex inside a JSON string literal,
        // so backslashes are JSON-escaped (i.e. the file contains "\\b" and not "\b").
        // Search for the keyword list as a substring, accepting either form.
        assertTrue(
            "TextMate keyword pattern must contain 'include'",
            text.contains("""\b(include|break|continue|else|for|if|return|while|do)\b""") ||
                text.contains("""\\b(include|break|continue|else|for|if|return|while|do)\\b""")
        )
    }

    @Test
    fun test_native_lexer_recognizes_include_as_keyword() {
        val lexer = XsHighlightingLexer()
        lexer.start("include")
        val tokenType = lexer.tokenType
        assertEquals("'include' should be lexed as a keyword", XsHighlightingTokenTypes.KEYWORD, tokenType)
    }

    @Test
    fun test_native_lexer_recognizes_include_with_semicolon_as_keyword() {
        val lexer = XsHighlightingLexer()
        lexer.start("include \"foo.xs\";")
        val tokenType = lexer.tokenType
        assertEquals("'include' should be lexed as a keyword", XsHighlightingTokenTypes.KEYWORD, tokenType)
    }
}
