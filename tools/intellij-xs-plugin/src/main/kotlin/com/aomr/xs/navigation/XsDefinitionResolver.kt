package com.aomr.xs.navigation

import com.intellij.openapi.editor.Document
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.openapi.vfs.VirtualFileManager
import com.intellij.platform.lsp.api.LspServer
import com.intellij.psi.PsiDocumentManager
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiManager
import org.eclipse.lsp4j.DefinitionParams
import org.eclipse.lsp4j.Location
import org.eclipse.lsp4j.LocationLink
import org.eclipse.lsp4j.Position
import org.eclipse.lsp4j.TextDocumentIdentifier

/**
 * Testable seam between [XsGotoDeclarationHandler] and the LSP server.
 *
 * The [server] parameter is nullable so unit tests can inject a stub resolver
 * without spinning up a real LSP server; production implementations return an
 * empty result when no server is available.
 */
interface XsDefinitionResolver {
    fun resolve(server: LspServer?, file: VirtualFile, offset: Int): List<PsiElement>
}

/**
 * Production implementation of [XsDefinitionResolver].
 *
 * Sends a synchronous `textDocument/definition` request to the active LSP
 * server, converts every returned [Location] or [LocationLink] into a
 * navigable [PsiElement], and swallows LSP exceptions so that network or
 * server errors do not break the editor navigation path.
 */
object XsLspDefinitionResolver : XsDefinitionResolver {

    override fun resolve(server: LspServer?, file: VirtualFile, offset: Int): List<PsiElement> {
        if (server == null) return emptyList()

        val document = FileDocumentManager.getInstance().getDocument(file) ?: return emptyList()
        val line = document.getLineNumber(offset)
        val col = offset - document.getLineStartOffset(line)
        val params = DefinitionParams(TextDocumentIdentifier(file.url), Position(line, col))

        val response = try {
            server.sendRequestSync(5000) { ls -> ls.textDocumentService.definition(params) }
        } catch (e: Exception) {
            return emptyList()
        }

        return when {
            response == null -> emptyList()
            response.isLeft -> response.left.mapNotNull { it.toPsiElement(server.descriptor.project) }
            else -> response.right.mapNotNull { it.toPsiElement(server.descriptor.project) }
        }
    }

    private fun Location.toPsiElement(project: Project): PsiElement? {
        val virtualFile = uri.toVirtualFile() ?: return null
        return psiElementAt(project, virtualFile, range.start.line, range.start.character)
    }

    private fun LocationLink.toPsiElement(project: Project): PsiElement? {
        val virtualFile = targetUri.toVirtualFile() ?: return null
        return psiElementAt(project, virtualFile, targetRange.start.line, targetRange.start.character)
    }

    private fun String.toVirtualFile(): VirtualFile? =
        runCatching { VirtualFileManager.getInstance().findFileByUrl(this) }.getOrNull()

    private fun psiElementAt(project: Project, file: VirtualFile, line: Int, char: Int): PsiElement? {
        if (!file.isValid) return null
        val psiFile = PsiManager.getInstance(project).findFile(file) ?: return null
        val doc = PsiDocumentManager.getInstance(project).getDocument(psiFile)
            ?: FileDocumentManager.getInstance().getDocument(file)
            ?: return null
        val offset = (doc.getLineStartOffset(line) + char).coerceAtMost(doc.textLength)
        return psiFile.findElementAt(offset)
    }
}
