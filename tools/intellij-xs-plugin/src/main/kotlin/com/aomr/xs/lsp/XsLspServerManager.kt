package com.aomr.xs.lsp

import com.aomr.xs.settings.XsSettings
import com.intellij.openapi.application.ApplicationManager
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import com.intellij.platform.lsp.api.LspServer
import com.intellij.platform.lsp.api.LspServerManager
import org.eclipse.lsp4j.DidChangeWorkspaceFoldersParams
import org.eclipse.lsp4j.WorkspaceFolder
import org.eclipse.lsp4j.WorkspaceFoldersChangeEvent
import java.io.File

/**
 * Per-project service that keeps the LSP server in sync with the user's
 * configured mod list.
 *
 * The platform owns the server process lifecycle; this manager only:
 * - Computes added/removed mod-folder deltas.
 * - Sends `workspace/didChangeWorkspaceFolders` to running servers.
 * - Restarts the server when the game folder changes.
 */
@Service(Service.Level.PROJECT)
class XsLspServerManager(private val project: Project) : com.intellij.openapi.Disposable {

    private val log = Logger.getInstance(XsLspServerManager::class.java)

    @Volatile
    private var currentModPaths: List<String> = emptyList()

    @Volatile
    private var currentGamePath: String = ""

    /**
     * Test/production seam for sending a workspace-folder delta.
     *
     * In production this delegates to [doSendWorkspaceFolderChange], which
     * forwards the notification to every running XS LSP server. Tests replace
     * the property with a capture double.
     */
    internal var sendWorkspaceFolderChange: (added: List<String>, removed: List<String>) -> Boolean =
        ::doSendWorkspaceFolderChange

    /**
     * Test seam for [restartServer]. When set, it is invoked instead of the
     * platform's [LspServerManager.stopAndRestartIfNeeded].
     */
    internal var restartServerHandler: (() -> Unit)? = null

    init {
        val settings = XsSettings.getInstance(project)
        // Initialise the delta baseline from persisted settings so that the
        // first settings change correctly removes folders that are no longer
        // configured (see verify-report "CRITICAL" finding).
        currentModPaths = settings.state.modPaths.toList()
        settings.addModPathsListener(XsSettings.Listener { notifyWorkspaceFoldersChanged(it) })
    }

    /**
     * Reacts to a settings change coming from the XS settings page.
     *
     * A game-folder change requires a server restart because the LSP server
     * caches the engine API at startup. A mod-list change is normally
     * propagated through the [XsSettings] listener registered in [init]; this
     * method is retained for callers that bypass [XsSettings.setModPaths].
     */
    fun updateSettings(gamePath: String, modPaths: List<String>) {
        if (gamePath.isBlank()) {
            log.info("Game path not configured; LSP server will not be restarted.")
            return
        }
        if (gamePath != currentGamePath) {
            currentGamePath = gamePath
            currentModPaths = modPaths
            restartServer()
        } else {
            notifyWorkspaceFoldersChanged(modPaths)
        }
    }

    /**
     * Computes the delta between the previous and new mod lists and sends
     * `workspace/didChangeWorkspaceFolders` to any running XS LSP servers.
     * If the send cannot be performed (no running server or exception), the
     * server is restarted so the new mod list is picked up in `initialize`.
     */
    fun notifyWorkspaceFoldersChanged(newModPaths: List<String>) {
        val previous = currentModPaths.toSet()
        val next = newModPaths.toSet()
        val added = (next - previous).toList()
        val removed = (previous - next).toList()
        currentModPaths = newModPaths
        if (added.isEmpty() && removed.isEmpty()) return

        val sent = try {
            sendWorkspaceFolderChange(added, removed)
        } catch (e: Exception) {
            log.warn("Failed to send workspace/didChangeWorkspaceFolders", e)
            false
        }
        if (!sent) {
            restartServer()
        }
    }

    private fun doSendWorkspaceFolderChange(addedPaths: List<String>, removedPaths: List<String>): Boolean {
        val addedFolders = addedPaths.map { pathToWorkspaceFolder(it) }
        val removedFolders = removedPaths.map { pathToWorkspaceFolder(it) }
        val event = WorkspaceFoldersChangeEvent(addedFolders, removedFolders)
        val params = DidChangeWorkspaceFoldersParams().apply { this.event = event }

        val servers = LspServerManager.getInstance(project).getServersForProvider(XsLspSupportProvider::class.java)
        if (servers.isEmpty()) {
            log.info("No running XS LSP server; falling back to restart to apply workspace-folder changes.")
            return false
        }

        var successCount = 0
        for (server in servers) {
            successCount += if (sendToServer(server, params, addedPaths, removedPaths)) 1 else 0
        }
        return successCount > 0
    }

    private fun sendToServer(
        server: LspServer,
        params: DidChangeWorkspaceFoldersParams,
        addedPaths: List<String>,
        removedPaths: List<String>,
    ): Boolean {
        return try {
            server.sendNotification { lsp4jServer ->
                lsp4jServer.workspaceService.didChangeWorkspaceFolders(params)
            }
            log.info(
                "Sent workspace/didChangeWorkspaceFolders to ${server.descriptor.presentableName}: " +
                    "+${addedPaths.size} -${removedPaths.size}"
            )
            for (path in addedPaths) {
                log.info("  + mod folder (path): $path")
            }
            for (path in removedPaths) {
                log.info("  - mod folder (path): $path")
            }
            true
        } catch (e: Exception) {
            log.warn("Failed to send workspace/didChangeWorkspaceFolders to ${server.descriptor.presentableName}", e)
            false
        }
    }

    private fun pathToWorkspaceFolder(path: String): WorkspaceFolder {
        return WorkspaceFolder(File(path).toURI().toString(), File(path).name)
    }

    private fun restartServer() {
        restartServerHandler?.invoke() ?: run {
            ApplicationManager.getApplication().invokeLater(Runnable {
                LspServerManager.getInstance(project).stopAndRestartIfNeeded(XsLspSupportProvider::class.java)
            }, project.disposed)
        }
    }

    override fun dispose() {
        // The platform stops the LSP server when the project is closed.
    }

    companion object {
        fun getInstance(project: Project): XsLspServerManager = project.service()
    }
}
