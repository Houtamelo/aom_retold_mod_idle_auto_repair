package com.aomr.xs.parameterInfo

import com.aomr.xs.completion.XsCallContextDetector
import com.aomr.xs.completion.XsCompletionContributor
import com.aomr.xs.constants.XsEngineApi
import com.intellij.lang.parameterInfo.CreateParameterInfoContext
import com.intellij.lang.parameterInfo.ParameterInfoHandler
import com.intellij.lang.parameterInfo.ParameterInfoUIContext
import com.intellij.lang.parameterInfo.UpdateParameterInfoContext
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile

/**
 * Parameter-info handler for engine syscalls.
 *
 * The implementation is PSI-less: it reuses [XsCallContextDetector] to locate the
 * enclosing call, looks the name up in [XsEngineApi], and asks the platform to
 * highlight the parameter that corresponds to the current caret position.
 */
class XsParameterInfoHandler : ParameterInfoHandler<PsiElement, XsEngineApi.Syscall> {

    override fun findElementForParameterInfo(context: CreateParameterInfoContext): PsiElement? {
        val file = context.file ?: return null
        val offset = context.offset
        val syscall = findOwner(file, offset) ?: return null
        context.itemsToShow = arrayOf(syscall)
        return elementAt(file, offset)
    }

    override fun showParameterInfo(element: PsiElement, context: CreateParameterInfoContext) {
        context.showHint(element, element.textRange.startOffset, this)
    }

    override fun findElementForUpdatingParameterInfo(context: UpdateParameterInfoContext): PsiElement? {
        val file = context.file ?: return null
        return elementAt(file, context.offset)
    }

    override fun updateParameterInfo(parameterOwner: PsiElement, context: UpdateParameterInfoContext) {
        val file = context.file ?: return
        context.setCurrentParameter(findParameterIndex(file, context.offset))
    }

    override fun updateUI(p: XsEngineApi.Syscall, context: ParameterInfoUIContext) {
        val signature = XsCompletionContributor.formatSignature(p)
        val index = context.currentParameterIndex.coerceIn(-1, p.params.size - 1)
        val (highlightStart, highlightEnd) = highlightRange(p, index)
        context.setupUIComponentPresentation(
            signature,
            highlightStart,
            highlightEnd,
            false,
            false,
            false,
            context.defaultParameterColor
        )
    }

    // -------------------------------------------------------------------------
    // Test-friendly helpers (also match the simplified methods described in the
    // change specification, even though the platform interface uses contexts).
    // -------------------------------------------------------------------------

    fun getParameterOwner(file: PsiFile?, offset: Int): Any? = file?.let { findOwner(it, offset) }

    fun getParametersForOwner(owner: Any?, context: PsiElement?, offset: Int): Array<Any>? {
        val syscall = owner as? XsEngineApi.Syscall ?: return null
        return syscall.params.toTypedArray<Any>()
    }

    fun updateUI(p: Any?, context: PsiElement?, offset: Int, label: String?): String? {
        val syscall = p as? XsEngineApi.Syscall ?: return null
        return renderParameterInfo(syscall, findParameterIndex(syscall, context, offset))
    }

    fun findParameterIndex(owner: Any?, context: PsiElement?, offset: Int): Int {
        val syscall = owner as? XsEngineApi.Syscall ?: return -1
        return findParameterIndex(syscall, context, offset)
    }

    fun findArgumentPosition(owner: Any?, context: PsiElement?, offsetInCalledElement: Int): Int {
        // The engine stub has no real argument offsets; map a position back to
        // the parameter index that would be active there.
        return findParameterIndex(owner, context, offsetInCalledElement)
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    private fun findOwner(file: PsiFile, offset: Int): XsEngineApi.Syscall? {
        if (XsCallContextDetector.isInStringOrComment(file, offset)) return null
        val callContext = XsCallContextDetector.findCallContext(file, offset) ?: return null
        return XsEngineApi.lookup(callContext.functionName)
    }

    private fun findParameterIndex(syscall: XsEngineApi.Syscall, context: PsiElement?, offset: Int): Int {
        val file = context?.containingFile ?: return -1
        return findParameterIndex(syscall, file, offset)
    }

    private fun findParameterIndex(syscall: XsEngineApi.Syscall, file: PsiFile, offset: Int): Int {
        if (syscall.params.isEmpty()) return -1
        val callContext = XsCallContextDetector.findCallContext(file, offset) ?: return -1
        if (callContext.functionName != syscall.name) return -1
        return callContext.paramIndex.coerceIn(0, syscall.params.size - 1)
    }

    internal fun findParameterIndex(file: PsiFile, offset: Int): Int {
        val syscall = findOwner(file, offset) ?: return -1
        return findParameterIndex(syscall, file, offset)
    }

    internal fun renderParameterInfo(syscall: XsEngineApi.Syscall, currentParameterIndex: Int = 0): String {
        val signature = XsCompletionContributor.formatSignature(syscall)
        if (syscall.params.isEmpty()) return signature
        val index = currentParameterIndex.coerceIn(0, syscall.params.size - 1)
        val (start, end) = highlightRange(syscall, index)
        if (start < 0) return signature
        return buildString {
            append(signature, 0, start)
            append("<b>")
            append(signature, start, end)
            append("</b>")
            append(signature, end, signature.length)
        }
    }

    private fun highlightRange(syscall: XsEngineApi.Syscall, index: Int): Pair<Int, Int> {
        if (syscall.params.isEmpty() || index < 0 || index >= syscall.params.size) return -1 to -1
        val prefix = "${syscall.returnType} ${syscall.name}("
        var start = prefix.length
        for (i in 0 until index) {
            start += "${syscall.params[i].type} ${syscall.params[i].name}".length + 2 // ", "
        }
        val end = start + "${syscall.params[index].type} ${syscall.params[index].name}".length
        return start to end
    }

    private fun elementAt(file: PsiFile, offset: Int): PsiElement? {
        val safeOffset = offset.coerceIn(0, (file.textLength - 1).coerceAtLeast(0))
        return file.findElementAt(safeOffset)
    }
}
