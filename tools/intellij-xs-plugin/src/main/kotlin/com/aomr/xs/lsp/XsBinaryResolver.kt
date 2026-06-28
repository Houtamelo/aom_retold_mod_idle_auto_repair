package com.aomr.xs.lsp

import com.intellij.openapi.diagnostic.Logger
import java.io.File
import java.io.InputStream
import java.nio.file.Files

/**
 * Resolves the path to the `xs-language-server` binary.
 *
 * The resolution order mirrors the old `XsLspConnection.resolveBinaryPath()`
 * behaviour and is deliberately easy to unit-test:
 *
 * 1. Java system property `-Dxs.lsp.path=...`
 * 2. Environment variable `XS_LSP_PATH=...`
 * 3. Bundled binary extracted from the plugin JAR resources
 * 4. Project-local binary at `tools/xs-language-server/target/release/xs-language-server`
 * 5. Plain `xs-language-server` lookup on the system `PATH`
 */
object XsBinaryResolver {

    private const val BUNDLED_RESOURCE_PATH = "/bin/xs-language-server"

    private val log = Logger.getInstance(XsBinaryResolver::class.java)

    /**
     * The extracted bundled binary is reused for the lifetime of the JVM.
     * It is extracted on first use and the temp file is deleted on JVM exit.
     */
    @Volatile
    private var cachedBundledPath: String? = null

    /**
     * Resolves the binary path using the supplied environment.
     *
     * @param systemProperty value of `-Dxs.lsp.path` (null to skip)
     * @param envVar value of `XS_LSP_PATH` (null to skip)
     * @param projectBasePath project base directory used for the project-local lookup
     * @param resourceProvider function returning an [InputStream] for a resource path;
     *                         defaults to loading from the plugin classpath
     */
    fun resolve(
        systemProperty: String? = System.getProperty("xs.lsp.path"),
        envVar: String? = System.getenv("XS_LSP_PATH"),
        projectBasePath: String? = null,
        resourceProvider: (String) -> InputStream? = { XsBinaryResolver::class.java.getResourceAsStream(it) },
    ): String {
        systemProperty?.takeIf { it.isNotBlank() }?.let {
            log.info("Using LSP binary from -Dxs.lsp.path: $it")
            return it
        }
        envVar?.takeIf { it.isNotBlank() }?.let {
            log.info("Using LSP binary from XS_LSP_PATH: $it")
            return it
        }
        extractBundled(resourceProvider)?.let {
            log.info("Using bundled LSP binary: $it")
            return it
        }
        findProjectBinary(projectBasePath)?.let {
            log.info("Using project-local LSP binary: $it")
            return it
        }
        log.warn(
            "No LSP binary found via system property, env, bundled, or project; " +
                "falling back to PATH lookup. Install via IntelliJ plugin or set XS_LSP_PATH."
        )
        return "xs-language-server"
    }

    /**
     * Extracts the bundled `bin/xs-language-server` resource to an executable
     * temp file. The result is cached for the remainder of the JVM lifetime.
     */
    private fun extractBundled(resourceProvider: (String) -> InputStream?): String? {
        cachedBundledPath?.let { return it }
        val stream = resourceProvider(BUNDLED_RESOURCE_PATH) ?: return null
        return try {
            val tempDir = Files.createTempDirectory("xs-lsp-").toFile()
            tempDir.deleteOnExit()
            val tempFile = File(tempDir, "xs-language-server")
            stream.use { input ->
                tempFile.outputStream().use { output ->
                    input.copyTo(output)
                }
            }
            // Mark as executable for the owner; group/other permissions are not
            // required because the JVM spawns the child process.
            tempFile.setExecutable(true, false)
            tempFile.deleteOnExit()
            cachedBundledPath = tempFile.absolutePath
            tempFile.absolutePath
        } catch (e: Exception) {
            log.warn("Failed to extract bundled LSP binary", e)
            null
        }
    }

    /**
     * Looks for a development build next to the project or the current working
     * directory.
     */
    private fun findProjectBinary(projectBasePath: String?): String? {
        val base = projectBasePath?.let { File(it) } ?: File(".")
        val candidates = listOf(
            base.resolve("tools/xs-language-server/target/release/xs-language-server"),
            File("tools/xs-language-server/target/release/xs-language-server")
        )
        return candidates.firstOrNull { it.exists() && it.canExecute() }?.absolutePath
    }
}
