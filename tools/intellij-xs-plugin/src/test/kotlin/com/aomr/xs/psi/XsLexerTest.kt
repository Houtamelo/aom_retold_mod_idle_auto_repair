package com.aomr.xs.psi

import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Paths

class XsLexerTest {

    @Test
    fun tokenizesHumanAssistXs() {
        val text = Files.readString(Paths.get("src/test/testData/human_assist.xs"))
        val lexer = XsLexerAdapter()
        lexer.start(text)

        val counts = mutableMapOf<com.intellij.psi.tree.IElementType, Int>()
        while (lexer.tokenType != null) {
            val type = lexer.tokenType!!
            counts[type] = counts.getOrDefault(type, 0) + 1
            lexer.advance()
        }

        assertTrue("Expected at least one LBRACE", counts.getOrDefault(XsTokenTypes.LBRACE, 0) >= 1)
        assertTrue("Expected at least one RBRACE", counts.getOrDefault(XsTokenTypes.RBRACE, 0) >= 1)
        assertTrue("Expected at least one LPAREN", counts.getOrDefault(XsTokenTypes.LPAREN, 0) >= 1)
        assertTrue("Expected at least one RPAREN", counts.getOrDefault(XsTokenTypes.RPAREN, 0) >= 1)
        assertTrue("Expected at least one LBRACKET", counts.getOrDefault(XsTokenTypes.LBRACKET, 0) >= 1)
        assertTrue("Expected at least one RBRACKET", counts.getOrDefault(XsTokenTypes.RBRACKET, 0) >= 1)
        assertTrue("Expected at least one LINE_COMMENT", counts.getOrDefault(XsTokenTypes.LINE_COMMENT, 0) >= 1)
        assertTrue("Expected at least one BLOCK_COMMENT", counts.getOrDefault(XsTokenTypes.BLOCK_COMMENT, 0) >= 1)
        assertTrue("Expected at least one STRING_QUOTE", counts.getOrDefault(XsTokenTypes.STRING_QUOTE, 0) >= 1)
        assertTrue("Expected at least one IDENTIFIER", counts.getOrDefault(XsTokenTypes.IDENTIFIER, 0) >= 1)
    }
}
