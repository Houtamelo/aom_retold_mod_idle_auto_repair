package com.aomr.xs.lsp

import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import org.eclipse.lsp4j.ClientCapabilities
import org.eclipse.lsp4j.DidChangeTextDocumentParams
import org.eclipse.lsp4j.DidChangeWatchedFilesParams
import org.eclipse.lsp4j.DidCloseTextDocumentParams
import org.eclipse.lsp4j.DidOpenTextDocumentParams
import org.eclipse.lsp4j.InitializeParams
import org.eclipse.lsp4j.InitializedParams
import org.eclipse.lsp4j.Registration
import org.eclipse.lsp4j.RegistrationParams
import org.eclipse.lsp4j.TextDocumentContentChangeEvent
import org.eclipse.lsp4j.TextDocumentIdentifier
import org.eclipse.lsp4j.TextDocumentItem
import org.eclipse.lsp4j.VersionedTextDocumentIdentifier
import org.eclipse.lsp4j.WorkspaceFolder
import org.eclipse.lsp4j.WorkspaceFoldersChangeEvent
import org.eclipse.lsp4j.launch.LSPLauncher
import org.eclipse.lsp4j.services.LanguageServer
import java.io.File
import java.util.concurrent.TimeUnit

/**
 * One LSP session: spawns the Rust `xs-language-server` binary with
 * `--game-path`, performs the initialize handshake, and exposes server
 * methods for the manager to call.
 *
 * Binary path resolution order:
 *  1. `-Dxs.lsp.path=...` system property
 *  2. `XS_LSP_PATH` environment variable
 *  3. `xs-language-server` resolved from the project build directory
 *     (`tools/xs-language-server/target/release/...` is not used; cargo
 *     builds to `tools/xs-language-server/target/release/xs-language-server`)
 *  4. `xs-language-server` on PATH
 *
 * Build the server with:
 *   cd tools/xs-language-server && cargo build --release
 */
class XsLspConnection(
    private val project: Project,
    private val gamePath: String
) {
    private val log = Logger.getInstance(XsLspConnection::class.java)
    private var process: Process? = null
    private var server: LanguageServer? = null

    @Synchronized
    fun start(): Boolean {
        if (server != null) return true
        val binaryPath = resolveBinaryPath()
        log.info("Starting XS LSP server: $binaryPath --game-path $gamePath")
        try {
            val pb = ProcessBuilder(binaryPath, "--game-path", gamePath)
            pb.redirectErrorStream(true)
            val proc = pb.start()
            process = proc

            val client = XsLanguageClient(project)
            val launcher = LSPLauncher.createClientLauncher(
                client,
                proc.inputStream,
                proc.outputStream,
            )
            val remote: LanguageServer = launcher.remoteProxy
            launcher.startListening()
            server = remote

            val initParams = InitializeParams().apply {
                processId = ProcessHandle.current().pid().toInt()
                rootUri = null
                workspaceFolders = emptyList()
                capabilities = ClientCapabilities()
                trace = "off"
            }
            val initResult = remote.initialize(initParams).get(30, TimeUnit.SECONDS)
            remote.initialized(InitializedParams())
            log.info("XS LSP server initialized: ${initResult.serverInfo ?: "(no serverInfo)"}")
            return true
        } catch (e: Exception) {
            log.error("Failed to start XS LSP server", e)
            stop()
            return false
        }
    }

    @Synchronized
    fun stop() {
        log.info("Stopping XS LSP server")
        try { server?.shutdown()?.get(5, TimeUnit.SECONDS) } catch (_: Exception) {}
        try { server?.exit() } catch (_: Exception) {}
        try { process?.destroy() } catch (_: Exception) {}
        try { process?.waitFor(2, TimeUnit.SECONDS) } catch (_: Exception) {}
        server = null
        process = null
    }

    @Synchronized
    fun isAlive(): Boolean = process?.isAlive == true && server != null

    @Synchronized
    fun changeWorkspaceFolders(added: List<String>, removed: List<String>) {
        val s = server ?: return
        val addedFolders = added.map { WorkspaceFolder(it.toFileUri(), File(it).name) }
        val removedFolders = removed.map { WorkspaceFolder(it.toFileUri(), File(it).name) }
        val event = WorkspaceFoldersChangeEvent(addedFolders, removedFolders)
        val params = org.eclipse.lsp4j.DidChangeWorkspaceFoldersParams().apply { this.event = event }
        try {
            s.workspaceService.didChangeWorkspaceFolders(params)
            log.info("Sent workspace/didChangeWorkspaceFolders: +${added.size} -${removed.size}")
        } catch (e: Exception) {
            log.warn("Failed to send workspace/didChangeWorkspaceFolders", e)
        }
    }

    @Synchronized
    fun registerGameFolderWatcher(gamePath: String) {
        val s = server ?: return
        val glob = "$gamePath/game/**/*.xs"
        val registration = Registration(
            "xs-game-folder-watcher",
            "workspace/didChangeWatchedFiles",
            org.eclipse.lsp4j.DidChangeWatchedFilesRegistrationOptions(
                listOf(org.eclipse.lsp4j.FileSystemWatcher(glob))
            )
        )
        try {
            s.client.registerCapability(RegistrationParams(listOf(registration)))
            log.info("Registered game-folder watcher: $glob")
        } catch (e: Exception) {
            log.warn("Failed to register game-folder watcher", e)
        }
    }

    @Synchronized
    fun didChangeWatchedFiles(params: DidChangeWatchedFilesParams) {
        val s = server ?: return
        try {
            s.workspaceService.didChangeWatchedFiles(params)
        } catch (e: Exception) {
            log.warn("Failed to send workspace/didChangeWatchedFiles", e)
        }
    }

    @Synchronized
    fun didOpen(uri: String, text: String, version: Int) {
        val params = DidOpenTextDocumentParams().apply {
            textDocument = TextDocumentItem().apply {
                this.uri = uri
                languageId = "xs"
                this.version = version
                this.text = text
            }
        }
        try {
            server?.textDocumentService?.didOpen(params)
        } catch (e: Exception) {
            log.warn("didOpen failed for $uri", e)
        }
    }

    @Synchronized
    fun didChange(uri: String, text: String, version: Int) {
        val params = DidChangeTextDocumentParams().apply {
            textDocument = VersionedTextDocumentIdentifier().apply {
                this.uri = uri
                this.version = version
            }
            contentChanges = listOf(
                TextDocumentContentChangeEvent().apply { this.text = text }
            )
        }
        try {
            server?.textDocumentService?.didChange(params)
        } catch (e: Exception) {
            log.warn("didChange failed for $uri", e)
        }
    }

    @Synchronized
    fun didClose(uri: String) {
        val params = DidCloseTextDocumentParams().apply {
            textDocument = TextDocumentIdentifier().apply { this.uri = uri }
        }
        try {
            server?.textDocumentService?.didClose(params)
        } catch (e: Exception) {
            log.warn("didClose failed for $uri", e)
        }
    }

    private fun resolveBinaryPath(): String {
        return System.getProperty("xs.lsp.path")
            ?: System.getenv("XS_LSP_PATH")
            ?: findProjectBinary()
            ?: "xs-language-server"
    }

    private fun findProjectBinary(): String? {
        val candidates = listOf(
            File(project.basePath ?: ".").resolve("tools/xs-language-server/target/release/xs-language-server"),
            File("tools/xs-language-server/target/release/xs-language-server")
        )
        return candidates.firstOrNull { it.canExecute() }?.absolutePath
    }

    private fun String.toFileUri(): String = File(this).toURI().toString()
}
