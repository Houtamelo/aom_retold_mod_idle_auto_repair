package com.aomr.xs.completion

import com.aomr.xs.XsLanguage
import com.aomr.xs.constants.XsEngineApi
import com.aomr.xs.psi.XsLexerAdapter
import com.aomr.xs.psi.XsTokenTypes
import com.intellij.codeInsight.completion.CompletionContributor
import com.intellij.codeInsight.completion.CompletionParameters
import com.intellij.codeInsight.completion.CompletionProvider
import com.intellij.codeInsight.completion.CompletionResultSet
import com.intellij.codeInsight.completion.CompletionType
import com.intellij.codeInsight.completion.InsertHandler
import com.intellij.codeInsight.completion.InsertionContext
import com.intellij.codeInsight.lookup.LookupElement
import com.intellij.codeInsight.lookup.LookupElementBuilder
import com.intellij.patterns.PlatformPatterns
import com.intellij.psi.PsiFile
import com.intellij.psi.tree.IElementType
import com.intellij.psi.tree.TokenSet
import com.intellij.util.ProcessingContext

/**
 * Completion contributor for the XS engine syscall API.
 *
 * Two providers share this contributor:
 * - Syscall name completion (P1.2): triggered when typing an identifier prefix.
 * - Default parameter-value completion (P1.3): triggered when the caret is inside
 *   the parenthesis list of a known syscall call, immediately after `(` or `,`.
 *
 * The two providers use runtime guards so that only one produces results for a
 * given caret position, even though both are registered for [CompletionType.BASIC].
 */
class XsCompletionContributor : CompletionContributor() {

    init {
        // P1.2: complete engine syscall names at identifier positions.
        extend(
            CompletionType.BASIC,
            PlatformPatterns.psiElement(XsTokenTypes.IDENTIFIER).withLanguage(XsLanguage.INSTANCE),
            SyscallNameCompletionProvider()
        )

        // P1.3: propose default values for the current syscall argument.
        extend(
            CompletionType.BASIC,
            PlatformPatterns.psiElement().withLanguage(XsLanguage.INSTANCE),
            SyscallDefaultCompletionProvider()
        )
    }

    private class SyscallNameCompletionProvider : CompletionProvider<CompletionParameters>() {
        override fun addCompletions(
            parameters: CompletionParameters,
            context: ProcessingContext,
            result: CompletionResultSet
        ) {
            val file = parameters.originalFile
            val offset = parameters.offset
            if (isInStringOrComment(file, offset)) return
            // When inside a known syscall call we let the default-value provider
            // handle the position, so the two providers never overlap.
            if (findCallContext(file, offset) != null) return

            val prefix = extractPrefix(parameters)
            for (syscall in XsEngineApi.searchByPrefix(prefix)) {
                result.addElement(
                    LookupElementBuilder.create(syscall.name)
                        .withTailText(" ${formatSignature(syscall)}")
                        .withInsertHandler(SyscallInsertHandler)
                )
            }
        }
    }

    private class SyscallDefaultCompletionProvider : CompletionProvider<CompletionParameters>() {
        override fun addCompletions(
            parameters: CompletionParameters,
            context: ProcessingContext,
            result: CompletionResultSet
        ) {
            val file = parameters.originalFile
            val offset = parameters.offset
            if (isInStringOrComment(file, offset)) return

            val callContext = findCallContext(file, offset) ?: return
            val syscall = XsEngineApi.lookup(callContext.functionName) ?: return
            val paramIndex = callContext.paramIndex
            if (paramIndex < 0 || paramIndex >= syscall.params.size) return

            val defaultValue = syscall.params[paramIndex].defaultValue
            if (defaultValue.isEmpty()) return

            result.addElement(
                LookupElementBuilder.create(defaultValue)
                    .withPresentableText(defaultValue)
                    .withTailText(" — default for ${syscall.params[paramIndex].name}")
            )
        }
    }

    private data class CallContext(val functionName: String, val paramIndex: Int)

    companion object {
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

        fun formatSignature(syscall: XsEngineApi.Syscall): String =
            "${syscall.returnType} ${syscall.name}(" +
                syscall.params.joinToString(", ") { "${it.type} ${it.name}" } +
                ")"

        /**
         * Reads the identifier text that ends at the caret. This is the prefix
         * the platform will use to filter lookup items.
         */
        fun extractPrefix(parameters: CompletionParameters): String {
            val position = parameters.position
            val start = position.textRange.startOffset
            val end = parameters.offset.coerceAtMost(position.textRange.endOffset)
            return if (end <= start) "" else position.text.subSequence(0, end - start).toString()
        }

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
         * and, if the function name is a known engine syscall, returns the argument
         * index for the current caret position.
         */
        private fun findCallContext(file: PsiFile, offset: Int): CallContext? {
            val text = file.text
            val tokens = tokenize(text)
            val index = tokenIndexAtOffset(tokens, offset) ?: return null
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

    private object SyscallInsertHandler : InsertHandler<LookupElement> {
        override fun handleInsert(context: InsertionContext, item: LookupElement) {
            val editor = context.editor
            val tail = context.tailOffset
            context.document.insertString(tail, "(")
            context.document.insertString(tail + 1, ")")
            context.commitDocument()
            editor.caretModel.moveToOffset(tail + 1)
        }
    }
}
