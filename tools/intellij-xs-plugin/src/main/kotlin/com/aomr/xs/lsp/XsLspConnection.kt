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
import org.eclipse.lsp4j.TextDocumentContentChangeEvent
import org.eclipse.lsp4j.TextDocumentIdentifier
import org.eclipse.lsp4j.TextDocumentItem
import org.eclipse.lsp4j.VersionedTextDocumentIdentifier
import org.eclipse.lsp4j.WorkspaceFolder
import org.eclipse.lsp4j.WorkspaceFoldersChangeEvent
import org.eclipse.lsp4j.launch.LSPLauncher
import org.eclipse.lsp4j.services.LanguageServer
import java.io.File
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths
import java.util.concurrent.TimeUnit

/**
 * One LSP session: spawns the Rust `xs-language-server` binary with
 * `--game-path`, performs the initialize handshake, and exposes server
 * methods for the manager to call.
 *
 * Binary path resolution order:
 *  1. `-Dxs.lsp.path=...` system property
 *  2. `XS_LSP_PATH` environment variable
 *  3. The `xs-language-server` binary bundled inside the plugin's
 *     resources at `bin/xs-language-server`. Extracted to a temp
 *     directory on first use; the extracted copy is reused for
 *     subsequent sessions in the same IDE run.
 *  4. Project-local binary at `tools/xs-language-server/target/release/xs-language-server`
 *     (convenience for development).
 *  5. `xs-language-server` on the `PATH`.
 *
 * The LSP writes its log output to stderr (via `tracing_subscriber::fmt()
 * .with_writer(std::io::stderr)`). We redirect that stream to a file at
 * `<project>/.idea/xs-lsp.log` so the user can inspect what the server is
 * actually doing (which workspace folders it received, which mod owns a
 * file, why a "file not part of any registered mod" warning fired). The
 * previous implementation merged stderr into stdout (`redirectErrorStream
 * (true)`), which silently dropped every log line because lsp4j's
 * MessageReader only accepts `Content-Length:` headers and JSON bodies on
 * that stream.
 *
 * To rebuild the bundled binary:
 *   cd tools/xs-language-server && cargo build --release
 *   cd tools/intellij-xs-plugin && ./gradlew copyLspServerToResources
 *   ./gradlew buildPlugin
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
        val logFile = resolveLspLogFile()
        log.info("Starting XS LSP server: $binaryPath --game-path $gamePath")
        log.info("XS LSP log file: $logFile")
        try {
            val pb = ProcessBuilder(binaryPath, "--game-path", gamePath)
            // Pipe the LSP's stderr to a file so users can inspect what the
            // server is doing. DO NOT merge into stdout: that stream carries
            // JSON-RPC and lsp4j's MessageReader discards anything that
            // isn't `Content-Length:` headers + JSON bodies.
            Files.createDirectories(logFile.parent)
            pb.redirectError(ProcessBuilder.Redirect.appendTo(logFile.toFile()))
            // Pass RUST_LOG through to the LSP so users can opt into
            // debug-level logging by setting the env var before launching
            // the IDE. Defaults stay at the LSP's own default (info).
            System.getenv("RUST_LOG")?.let { pb.environment()["RUST_LOG"] = it }
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

    /**
     * Path of the file the LSP writes its stderr to. One file per project,
     * appended across sessions, in the project's `.idea/` directory (which
     * is in `.gitignore` for this repo, so logs never accidentally get
     * committed). Surfaced in [start]'s log line so users know where to
     * look.
     */
    private fun resolveLspLogFile(): Path {
        val basePath = project.basePath
            ?: return Paths.get(System.getProperty("java.io.tmpdir"), "xs-lsp.log")
        return Paths.get(basePath, ".idea", "xs-lsp.log")
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
        System.getProperty("xs.lsp.path")?.takeIf { it.isNotBlank() }?.let {
            log.info("Using LSP binary from -Dxs.lsp.path: $it"); return it
        }
        System.getenv("XS_LSP_PATH")?.takeIf { it.isNotBlank() }?.let {
            log.info("Using LSP binary from XS_LSP_PATH: $it"); return it
        }
        extractBundledBinary()?.let {
            log.info("Using bundled LSP binary: $it"); return it
        }
        findProjectBinary()?.let {
            log.info("Using project-local LSP binary: $it"); return it
        }
        log.warn("No LSP binary found via system property, env, bundled, or project; " +
            "falling back to PATH lookup. Install via IntelliJ plugin or set XS_LSP_PATH.")
        return "xs-language-server"
    }

    private fun findProjectBinary(): String? {
        val candidates = listOf(
            File(project.basePath ?: ".").resolve("tools/xs-language-server/target/release/xs-language-server"),
            File("tools/xs-language-server/target/release/xs-language-server")
        )
        return candidates.firstOrNull { it.canExecute() }?.absolutePath
    }

    /**
     * Extract the `bin/xs-language-server` resource that was bundled into the
     * plugin at build time (see `build.gradle.kts::copyLspServerToResources`)
     * to a temp file, make it executable, and return its path.
     *
     * The extracted binary is reused across sessions within a single IDE run
     * (cached in [bundledBinaryPath]). It is deleted on JVM exit.
     */
    private fun extractBundledBinary(): String? {
        bundledBinaryPath?.let { return it }
        val resourcePath = "/bin/xs-language-server"
        val stream = XsLspConnection::class.java.getResourceAsStream(resourcePath) ?: return null
        return try {
            val tempDir = Files.createTempDirectory("xs-lsp-").toFile()
            tempDir.deleteOnExit()
            val tempFile = File(tempDir, "xs-language-server")
            stream.use { input ->
                tempFile.outputStream().use { output ->
                    input.copyTo(output)
                }
            }
            // Mark as executable for the owner; group/other perms are not
            // required since the JVM spawns the child process.
            tempFile.setExecutable(true, false)
            tempFile.deleteOnExit()
            bundledBinaryPath = tempFile.absolutePath
            tempFile.absolutePath
        } catch (e: Exception) {
            log.warn("Failed to extract bundled LSP binary", e)
            null
        } finally {
            try { stream.close() } catch (_: Exception) {}
        }
    }

    private fun String.toFileUri(): String = File(this).toURI().toString()

    companion object {
        /**
         * Cached path to the extracted bundled binary. Populated on first
         * call to [extractBundledBinary]. Cleared automatically when the
         * JVM exits because the underlying temp file is `deleteOnExit`.
         */
        @Volatile
        private var bundledBinaryPath: String? = null
    }
}
