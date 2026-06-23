package com.aomr.xs.documentation

import com.aomr.xs.completion.XsCompletionContributor
import com.aomr.xs.constants.XsEngineApi
import com.intellij.lang.documentation.DocumentationProvider
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiManager

/**
 * Hover documentation provider for the XS engine syscall API.
 *
 * The provider is intentionally PSI-agnostic: it looks up the text of the element
 * under the caret in [XsEngineApi]. If the text is a known syscall name, it renders
 * the signature, help paragraph, parameter list, and source filename as HTML.
 * Unknown identifiers return `null`, which tells the platform not to show a popup.
 */
class XsDocumentationProvider : DocumentationProvider {

    override fun getDocumentationElementForLink(psiManager: PsiManager, link: String, context: PsiElement?): PsiElement? = null

    override fun getDocumentationElementForLookupItem(psiManager: PsiManager, lookupItem: Any?, context: PsiElement?): PsiElement? = null

    override fun getQuickNavigateInfo(element: PsiElement?, originalElement: PsiElement?): String? {
        val syscall = resolve(element, originalElement) ?: return null
        return XsCompletionContributor.formatSignature(syscall)
    }

    override fun generateDoc(element: PsiElement?, originalElement: PsiElement?): String? {
        val syscall = resolve(element, originalElement) ?: return null
        return renderDocumentation(syscall)
    }

    /**
     * Renders a syscall as HTML hover documentation.
     *
     * Visible for testing so scenario 4 (empty help) can be verified with a
     * synthetic syscall, because the vendored `syscalls.json` does not contain
     * any entries with blank help text.
     */
    internal fun renderDocumentation(syscall: XsEngineApi.Syscall): String {
        val signature = XsCompletionContributor.formatSignature(syscall)
        val helpText = syscall.help.trim().ifEmpty { "(no help available)" }
        val paramsSection = renderParams(syscall)

        return """
            <html>
            <body>
                <h2>${escapeHtml(signature)}</h2>
                <p>${escapeHtml(helpText)}</p>
                <h3>Parameters</h3>
                $paramsSection
                <p><small>Source: ${escapeHtml(syscall.filename)}</small></p>
            </body>
            </html>
        """.trimIndent()
    }

    private fun resolve(element: PsiElement?, originalElement: PsiElement?): XsEngineApi.Syscall? {
        val name = element?.text?.trim().takeIf { !it.isNullOrEmpty() }
            ?: originalElement?.text?.trim().takeIf { !it.isNullOrEmpty() }
            ?: return null
        return XsEngineApi.lookup(name)
    }

    private fun renderParams(syscall: XsEngineApi.Syscall): String {
        if (syscall.params.isEmpty()) {
            return "<p><i>No parameters.</i></p>"
        }
        return "<ul>" + syscall.params.joinToString("") { param ->
            val default = param.defaultValue.trim()
            val line = if (default.isEmpty()) {
                "${param.type} ${param.name}"
            } else {
                "${param.type} ${param.name} — default: $default"
            }
            "<li>${escapeHtml(line)}</li>"
        } + "</ul>"
    }

    private fun escapeHtml(text: String): String =
        text.replace("&", "&amp;")
            .replace("<", "&lt;")
            .replace(">", "&gt;")
            .replace("\"", "&quot;")
}
