package com.aomr.xs.lsp

import com.aomr.xs.settings.XsAppSettings
import com.aomr.xs.settings.XsSettings
import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.execution.process.OSProcessHandler
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerDescriptor
import com.intellij.platform.lsp.api.customization.LspSemanticTokensSupport
import java.io.File
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.Paths

/**
 * Platform LSP server descriptor for XS.
 *
 * One server is started per IntelliJ project. The server is launched lazily by
 * the platform when the first `.xs` file is opened; this class only describes
 * *how* to start it:
 * - which binary to run (via [XsBinaryResolver])
 * - the `--game-path` argument
 * - the mod list as initial workspace folders
 */
class XsLspServerDescriptor(
    project: Project,
    private val gamePath: String,
) : LspServerDescriptor(
    project,
    "XS Language Server",
    *currentModWorkspaceFolders(project)
) {

    override fun isSupportedFile(file: VirtualFile): Boolean = file.extension == "xs"

    override val lspSemanticTokensSupport: LspSemanticTokensSupport = XsSemanticTokensSupport()

    override fun createCommandLine(): GeneralCommandLine {
        // Resolve the binary fresh at startup. The bundled/resource extraction
        // is cached, so this is cheap after the first call.
        val binaryPath = XsBinaryResolver.resolve(projectBasePath = project.basePath)

        val commandLine = GeneralCommandLine(binaryPath, "--game-path", currentGamePath())
            .withRedirectErrorStream(false)

        // Forward RUST_LOG so users can opt into debug-level LSP logging by
        // setting the env var before launching the IDE.
        System.getenv("RUST_LOG")?.let {
            commandLine.withEnvironment("RUST_LOG", it)
        }

        return commandLine
    }

    /**
     * Redirects the LSP server's stderr to a project log file. The platform's
     * default process handler merges stderr into stdout, which breaks the LSP
     * JSON-RPC stream, so we start the process ourselves with stderr appended
     * to `<project>/.idea/xs-lsp.log`.
     */
    override fun startServerProcess(): OSProcessHandler {
        val commandLine = createCommandLine()
        val logFile = resolveLspLogFile()
        Files.createDirectories(logFile.parent)

        val commands = commandLine.getCommandLineList(null)
        val process = ProcessBuilder(commands)
            .directory(commandLine.workDirectory)
            .redirectError(ProcessBuilder.Redirect.appendTo(logFile.toFile()))
            .apply {
                environment().putAll(commandLine.environment)
            }
            .start()

        return OSProcessHandler(process, commandLine.commandLineString)
    }

    /**
     * Reads the game folder from application settings every time the server is
     * started or restarted. This guarantees that a game-path change picked up
     * by [XsLspServerManager.updateSettings] is reflected without having to
     * recreate the descriptor instance.
     */
    private fun currentGamePath(): String = gamePath.ifBlank {
        XsAppSettings.getInstance().state.gamePath
    }

    /**
     * Path of the file the LSP writes its stderr to. One file per project,
     * appended across sessions, in the project's `.idea/` directory.
     */
    private fun resolveLspLogFile(): Path {
        val basePath = project.basePath
            ?: return Paths.get(System.getProperty("java.io.tmpdir"), "xs-lsp.log")
        return Paths.get(basePath, ".idea", "xs-lsp.log")
    }

    companion object {
        fun create(project: Project): XsLspServerDescriptor {
            val gamePath = XsAppSettings.getInstance().state.gamePath
            return XsLspServerDescriptor(project, gamePath)
        }

        private fun currentModWorkspaceFolders(project: Project): Array<VirtualFile> {
            val modPaths = XsSettings.getInstance(project).state.modPaths
            val localFileSystem = LocalFileSystem.getInstance()
            return modPaths.mapNotNull { localFileSystem.refreshAndFindFileByPath(it) }.toTypedArray()
        }
    }
}
