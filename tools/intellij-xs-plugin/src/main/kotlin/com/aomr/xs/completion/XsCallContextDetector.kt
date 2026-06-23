package com.aomr.xs.completion

import com.aomr.xs.psi.XsLexerAdapter
import com.aomr.xs.psi.XsTokenTypes
import com.intellij.psi.PsiFile
import com.intellij.psi.tree.IElementType
import com.intellij.psi.tree.TokenSet

/**
 * PSI-less call-site detector shared by completion, parameter info, and
 * engine-syscall reference resolution.
 */
object XsCallContextDetector {

    data class CallContext(val functionName: String, val paramIndex: Int)

    private val NON_CODE_TOKENS = TokenSet.create(
        XsTokenTypes.LINE_COMMENT,
        XsTokenTypes.BLOCK_COMMENT,
        XsTokenTypes.STRING_LITERAL,
        XsTokenTypes.CHAR_LITERAL,
        XsTokenTypes.STRING_QUOTE,
        XsTokenTypes.CHAR_QUOTE
    )

    private val LEFT_GLUE_TOKENS = TokenSet.orSet(
        TokenSet.create(XsTokenTypes.WHITE_SPACE),
        NON_CODE_TOKENS
    )

    /**
     * Returns true when the caret sits inside a comment or string/char literal.
     */
    fun isInStringOrComment(file: PsiFile, offset: Int): Boolean {
        val lexer = XsLexerAdapter()
        lexer.start(file.text, 0, file.text.length, 0)
        while (lexer.tokenType != null) {
            if (offset in lexer.tokenStart until lexer.tokenEnd) {
                return NON_CODE_TOKENS.contains(lexer.tokenType)
            }
            lexer.advance()
        }
        return false
    }

    /**
     * Attempts to locate the enclosing function-call parentheses around [offset]
     * and, if the preceding identifier looks like a function call, returns the
     * function name and the current argument index (number of preceding commas
     * at nesting depth zero).
     */
    fun findCallContext(file: PsiFile, offset: Int): CallContext? {
        val text = file.text
        val tokens = tokenize(text)
        if (tokens.isEmpty()) return null
        val index = when {
            offset >= text.length -> tokens.lastIndex
            else -> tokenIndexAtOffset(tokens, offset) ?: return null
        }
        if (NON_CODE_TOKENS.contains(tokens[index].type)) return null

        // If the caret is immediately before the closing ')', treat that ')'
        // as the call boundary rather than as a sibling parenthesis.
        val atClosingParen = tokens[index].type == XsTokenTypes.RPAREN && offset == tokens[index].start
        val boundaryIndex = if (atClosingParen) index - 1 else index

        var parenDepth = 0
        var lparenIndex = -1
        for (i in boundaryIndex downTo 0) {
            when (tokens[i].type) {
                XsTokenTypes.RPAREN -> parenDepth++
                XsTokenTypes.LPAREN -> {
                    if (parenDepth == 0) {
                        lparenIndex = i
                        break
                    }
                    parenDepth--
                }
            }
        }
        if (lparenIndex <= 0) return null

        val rparenIndex = if (atClosingParen) index else findMatchingRParen(tokens, lparenIndex)
        val endIndex = if (rparenIndex >= 0) rparenIndex else tokens.size
        var commaCount = 0
        var commaDepth = 0
        for (i in (lparenIndex + 1) until minOf(boundaryIndex, endIndex)) {
            when (tokens[i].type) {
                XsTokenTypes.LPAREN -> commaDepth++
                XsTokenTypes.RPAREN -> commaDepth--
                XsTokenTypes.COMMA -> if (commaDepth == 0) commaCount++
            }
        }

        var nameIndex = lparenIndex - 1
        while (nameIndex >= 0 && LEFT_GLUE_TOKENS.contains(tokens[nameIndex].type)) {
            nameIndex--
        }
        if (nameIndex < 0 || tokens[nameIndex].type != XsTokenTypes.IDENTIFIER) return null

        val functionName = text.substring(tokens[nameIndex].start, tokens[nameIndex].end)
        return CallContext(functionName, commaCount)
    }

    private fun findMatchingRParen(tokens: List<TokenInfo>, lparenIndex: Int): Int {
        var depth = 1
        for (i in (lparenIndex + 1) until tokens.size) {
            when (tokens[i].type) {
                XsTokenTypes.LPAREN -> depth++
                XsTokenTypes.RPAREN -> {
                    depth--
                    if (depth == 0) return i
                }
            }
        }
        return -1
    }

    private fun tokenize(text: String): List<TokenInfo> {
        val lexer = XsLexerAdapter()
        lexer.start(text, 0, text.length, 0)
        val result = mutableListOf<TokenInfo>()
        while (lexer.tokenType != null) {
            result.add(TokenInfo(lexer.tokenType, lexer.tokenStart, lexer.tokenEnd))
            lexer.advance()
        }
        return result
    }

    private fun tokenIndexAtOffset(tokens: List<TokenInfo>, offset: Int): Int? {
        tokens.forEachIndexed { i, token ->
            if (offset >= token.start && offset < token.end) return i
        }
        return null
    }

    private data class TokenInfo(val type: IElementType?, val start: Int, val end: Int)
}
