package com.aomr.xs.lsp

import com.aomr.xs.settings.XsSettings
import com.intellij.openapi.Disposable
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.editor.event.DocumentEvent
import com.intellij.openapi.editor.event.DocumentListener
import com.intellij.openapi.fileEditor.FileDocumentManager
import com.intellij.openapi.fileEditor.FileEditorManager
import com.intellij.openapi.fileEditor.FileEditorManagerListener
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.openapi.vfs.VirtualFileEvent
import com.intellij.openapi.vfs.VirtualFileListener
import com.intellij.openapi.vfs.VirtualFileManager
import org.eclipse.lsp4j.DidChangeWatchedFilesParams
import org.eclipse.lsp4j.FileChangeType
import org.eclipse.lsp4j.FileEvent
import java.io.File

/**
 * Per-project service that owns the LSP connection and wires IntelliJ's
 * editor events to LSP didOpen / didChange / didClose.
 *
 * Settings changes drive the lifecycle:
 * - On game-path change the server is restarted with the new `--game-path`.
 * - On mod-list change `workspace/didChangeWorkspaceFolders` is sent with the
 *   delta so the server updates its virtual projects without a restart.
 */
@Service(Service.Level.PROJECT)
class XsLspServerManager(private val project: Project) : Disposable {

    private val log = Logger.getInstance(XsLspServerManager::class.java)
    private var connection: XsLspConnection? = null
    private var currentGamePath: String = ""
    private var currentModPaths: List<String> = emptyList()
    private val versions = mutableMapOf<String, Int>()
    private var virtualFileListener: VirtualFileListener? = null

    init {
        installEditorListeners()
    }

    /**
     * Starts the LSP server with the given settings, applying workspace folders
     * and the game-folder watcher once the handshake completes.
     */
    @Synchronized
    fun start(settings: XsSettings.State = XsSettings.getInstance(project).state) {
        if (settings.gamePath.isBlank()) {
            log.info("Game path not configured; LSP server will not start.")
            return
        }
        val wasRunning = connection != null
        if (wasRunning && settings.gamePath == currentGamePath) {
            // Game path unchanged; just ensure workspace folders are current.
            updateWorkspaceFolders(settings.modPaths)
            return
        }
        stop()
        val conn = XsLspConnection(project, settings.gamePath)
        if (!conn.start()) {
            showError("Failed to start XS Language Server for game path: ${settings.gamePath}")
            return
        }
        connection = conn
        currentGamePath = settings.gamePath
        installGameFolderWatcher(settings.gamePath)
        updateWorkspaceFolders(settings.modPaths)
    }

    /**
     * Reacts to a settings change. Restarts the server when the game path
     * changes; re-sends workspace folders when the mod list changes.
     */
    fun updateSettings(settings: XsSettings.State) {
        start(settings)
    }

    @Synchronized
    override fun dispose() {
        log.info("Disposing XS LSP server manager")
        stop()
    }

    @Synchronized
    private fun stop() {
        removeGameFolderWatcher()
        connection?.stop()
        connection = null
        currentGamePath = ""
        currentModPaths = emptyList()
        versions.clear()
    }

    private fun installEditorListeners() {
        project.messageBus.connect(this).subscribe(
            FileEditorManagerListener.FILE_EDITOR_MANAGER,
            object : FileEditorManagerListener {
                override fun fileOpened(source: FileEditorManager, file: VirtualFile) {
                    if (file.extension == "xs") onFileOpened(file)
                }

                override fun fileClosed(source: FileEditorManager, file: VirtualFile) {
                    if (file.extension == "xs") onFileClosed(file)
                }
            }
        )

        val factory = com.intellij.openapi.editor.EditorFactory.getInstance()
        factory.eventMulticaster.addDocumentListener(
            object : DocumentListener {
                override fun documentChanged(event: DocumentEvent) {
                    val file = FileDocumentManager.getInstance().getFile(event.document) ?: return
                    if (file.extension != "xs") return
                    onFileChanged(file, event.document.text)
                }
            },
            this
        )
    }

    private fun installGameFolderWatcher(gamePath: String) {
        removeGameFolderWatcher()
        val gameDir = File(gamePath).resolve("game").normalize()
        if (!gameDir.exists()) return

        val listener = object : VirtualFileListener {
            private fun maybeForward(event: VirtualFileEvent, kind: FileChangeType) {
                val file = event.file
                if (file.extension != "xs") return
                val path = File(file.path).normalize().path
                if (!path.startsWith(gameDir.path, ignoreCase = true)) return
                val uri = file.toUriString()
                val conn = connection ?: return
                conn.didChangeWatchedFiles(
                    DidChangeWatchedFilesParams(
                        listOf(FileEvent(uri, kind))
                    )
                )
            }

            override fun contentsChanged(event: VirtualFileEvent) = maybeForward(event, FileChangeType.Changed)
            override fun fileCreated(event: VirtualFileEvent) = maybeForward(event, FileChangeType.Created)
            override fun fileDeleted(event: VirtualFileEvent) = maybeForward(event, FileChangeType.Deleted)
        }
        VirtualFileManager.getInstance().addVirtualFileListener(listener, this)
        virtualFileListener = listener
    }

    private fun removeGameFolderWatcher() {
        virtualFileListener?.let {
            VirtualFileManager.getInstance().removeVirtualFileListener(it)
        }
        virtualFileListener = null
    }

    @Synchronized
    private fun updateWorkspaceFolders(updated: List<String>) {
        val previous = currentModPaths.toSet()
        val next = updated.toSet()
        val added = (next - previous).toList()
        val removed = (previous - next).toList()
        currentModPaths = updated
        if (added.isEmpty() && removed.isEmpty()) return
        connection?.changeWorkspaceFolders(
            added.map { File(it).toURI().toString() },
            removed.map { File(it).toURI().toString() }
        )
    }

    private fun onFileOpened(file: VirtualFile) {
        val doc = FileDocumentManager.getInstance().getDocument(file) ?: return
        val uri = file.toUriString()
        val version = bumpVersion(uri)
        log.info("didOpen: $uri (v$version, ${doc.textLength} chars)")
        connection?.didOpen(uri, doc.text, version)
    }

    private fun onFileChanged(file: VirtualFile, text: String) {
        val uri = file.toUriString()
        val version = bumpVersion(uri)
        connection?.didChange(uri, text, version)
    }

    private fun onFileClosed(file: VirtualFile) {
        val uri = file.toUriString()
        log.info("didClose: $uri")
        connection?.didClose(uri)
        versions.remove(uri)
    }

    private fun bumpVersion(uri: String): Int {
        val next = (versions[uri] ?: 0) + 1
        versions[uri] = next
        return next
    }

    private fun VirtualFile.toUriString(): String = File(path).toURI().toString()

    private fun showError(message: String) {
        val group = com.intellij.notification.NotificationGroupManager.getInstance()
            .getNotificationGroup("XS Language Server")
        val notification = group.createNotification(message, com.intellij.notification.NotificationType.ERROR)
        notification.notify(project)
    }

    companion object {
        fun getInstance(project: Project): XsLspServerManager = project.service()
    }
}
