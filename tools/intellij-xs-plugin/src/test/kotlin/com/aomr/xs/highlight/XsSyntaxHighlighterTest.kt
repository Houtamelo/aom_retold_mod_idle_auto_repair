package com.aomr.xs.highlight

import com.aomr.xs.psi.XsTokenTypes
import com.intellij.openapi.editor.colors.TextAttributesKey
import org.junit.Assert.assertTrue
import org.junit.Test

class XsSyntaxHighlighterTest {

    private fun highlightsFor(text: String): Array<TextAttributesKey> {
        val highlighter = XsSyntaxHighlighter()
        val lexer = highlighter.getHighlightingLexer()
        lexer.start(text)
        val tokenType = lexer.tokenType
        return highlighter.getTokenHighlights(tokenType)
    }

    @Test
    fun test_braces_map_to_xs_braces() {
        assertTrue("'{' should map to BRACES", highlightsFor("{").contains(XsTextAttributes.BRACES))
        assertTrue("'}' should map to BRACES", highlightsFor("}").contains(XsTextAttributes.BRACES))
    }

    @Test
    fun test_brackets_map_to_xs_brackets() {
        assertTrue("'[' should map to BRACKETS", highlightsFor("[").contains(XsTextAttributes.BRACKETS))
        assertTrue("']' should map to BRACKETS", highlightsFor("]").contains(XsTextAttributes.BRACKETS))
    }

    @Test
    fun test_comma_maps_to_xs_comma() {
        assertTrue(", should map to COMMA", highlightsFor(",").contains(XsTextAttributes.COMMA))
    }

    @Test
    fun test_dot_maps_to_xs_dot() {
        assertTrue(". should map to DOT", highlightsFor(".").contains(XsTextAttributes.DOT))
    }

    @Test
    fun test_operation_sign_maps_to_xs_operation_sign() {
        assertTrue("= should map to OPERATION_SIGN", highlightsFor("=").contains(XsTextAttributes.OPERATION_SIGN))
        assertTrue("+ should map to OPERATION_SIGN", highlightsFor("+").contains(XsTextAttributes.OPERATION_SIGN))
    }

    @Test
    fun test_overloaded_operator_key_returned_for_operation_sign_token() {
        // The OPERATION_SIGN lexer token is shared for all operators. The highlighter returns
        // both OPERATION_SIGN and OVERLOADED_OPERATOR so both color-scheme categories can be
        // customized by users who want distinct overloaded-operator coloring.
        val keys = highlightsFor("+")
        assertTrue("+ should map to OPERATION_SIGN", keys.contains(XsTextAttributes.OPERATION_SIGN))
        assertTrue("+ should also map to OVERLOADED_OPERATOR", keys.contains(XsTextAttributes.OVERLOADED_OPERATOR))
    }

    @Test
    fun test_parentheses_map_to_xs_parentheses() {
        assertTrue("( should map to PARENTHESES", highlightsFor("(").contains(XsTextAttributes.PARENTHESES))
        assertTrue(") should map to PARENTHESES", highlightsFor(")").contains(XsTextAttributes.PARENTHESES))
    }

    @Test
    fun test_semicolon_maps_to_xs_semicolon() {
        assertTrue("; should map to SEMI_COLON", highlightsFor(";").contains(XsTextAttributes.SEMI_COLON))
    }
}
