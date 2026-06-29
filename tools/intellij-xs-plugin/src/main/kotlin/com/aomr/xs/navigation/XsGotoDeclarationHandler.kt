package com.aomr.xs.navigation

import com.aomr.xs.lsp.XsLspSupportProvider
import com.intellij.codeInsight.navigation.actions.GotoDeclarationHandler
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.editor.Editor
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerManager
import com.intellij.psi.PsiElement

/**
 * Goto-declaration handler for `.xs` files.
 *
 * Forwards Ctrl+Click / `Ctrl+B` / `⌘B` triggers to the active XS LSP server
 * via [XsLspDefinitionResolver]. Returns `null` for non-XS files so the
 * platform's default handlers remain in control. Exceptions and timeouts are
 * logged and surfaced as empty result sets, never as UI balloons.
 */
class XsGotoDeclarationHandler(
    private val resolver: XsDefinitionResolver = XsLspDefinitionResolver,
) : GotoDeclarationHandler {

    override fun getGotoDeclarationTargets(
        sourceElement: PsiElement?,
        offset: Int,
        editor: Editor?,
    ): Array<PsiElement>? {
        if (editor == null) return null

        val source = sourceElement ?: return null
        val file = source.containingFile?.virtualFile
            ?: source.containingFile?.originalFile?.virtualFile
            ?: return null
        if (file.extension != "xs") return null

        val project = source.project
        val server = LspServerManager.getInstance(project)
            .getServersForProvider(XsLspSupportProvider::class.java)
            .firstOrNull { it.descriptor.isSupportedFile(file) }

        return try {
            resolver.resolve(server, file, offset).toTypedArray()
        } catch (e: Exception) {
            LOG.warn("XS goto definition failed", e)
            emptyArray()
        }
    }

    companion object {
        private val LOG = Logger.getInstance(XsGotoDeclarationHandler::class.java)
    }
}
