package com.aomr.xs.completion

import com.aomr.xs.XsLanguage
import com.aomr.xs.constants.XsEngineApi
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
import com.intellij.util.ProcessingContext

/**
 * Completion contributor for the XS engine syscall API.
 *
 * Two providers share this contributor:
 * - Syscall name completion (P1.2): triggered when typing an identifier prefix.
 * - Default parameter-value completion (P1.3): triggered when the caret is inside
 *   the parenthesis list of a known syscall call, immediately after `(` or `,`.
 *
 * Call-site detection is delegated to [XsCallContextDetector] so that parameter
 * info and navigation can reuse the same PSI-less logic.
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
            if (XsCallContextDetector.isInStringOrComment(file, offset)) return
            // When inside a known syscall call we let the default-value provider
            // handle the position, so the two providers never overlap.
            if (XsCallContextDetector.findCallContext(file, offset) != null) return

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
            if (XsCallContextDetector.isInStringOrComment(file, offset)) return

            val callContext = XsCallContextDetector.findCallContext(file, offset) ?: return
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

    companion object {
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
