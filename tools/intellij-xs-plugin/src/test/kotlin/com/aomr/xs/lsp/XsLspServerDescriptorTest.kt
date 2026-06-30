package com.aomr.xs.lsp

import com.aomr.xs.settings.XsAppSettings
import com.intellij.platform.lsp.api.LspServerDescriptor
import com.intellij.testFramework.LightVirtualFile
import com.intellij.testFramework.ProjectRule
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class XsLspServerDescriptorTest {

    @get:Rule
    val projectRule = ProjectRule()

    @get:Rule
    val tempFolder = TemporaryFolder()

    private val gamePath = "/game/age of mythology retold"

    @Before
    fun setState() {
        val binary = tempFolder.newFile("xs-language-server").apply { setExecutable(true) }
        System.setProperty("xs.lsp.path", binary.absolutePath)
        XsAppSettings.getInstance().setGamePath(gamePath)
    }

    @After
    fun clearState() {
        System.clearProperty("xs.lsp.path")
        XsAppSettings.getInstance().setGamePath("")
    }

    private fun createDescriptor(): LspServerDescriptor {
        return XsLspServerDescriptor.create(projectRule.project)
    }

    @Test
    fun isSupportedFileReturnsTrueForXs() {
        val descriptor = createDescriptor()
        assertTrue(descriptor.isSupportedFile(LightVirtualFile("foo.xs", "")))
    }

    @Test
    fun isSupportedFileReturnsFalseForOtherExtensions() {
        val descriptor = createDescriptor()
        assertFalse(descriptor.isSupportedFile(LightVirtualFile("foo.txt", "")))
        assertFalse(descriptor.isSupportedFile(LightVirtualFile("foo", "")))
    }

    @Test
    fun createCommandLineUsesResolvedBinaryAndGamePath() {
        val descriptor = createDescriptor()
        val commandLine = descriptor.createCommandLine()

        assertEquals(System.getProperty("xs.lsp.path"), commandLine.exePath)

        val params = commandLine.parametersList.parameters
        assertEquals("--game-path", params[0])
        assertEquals(gamePath, params[1])
    }

    @Test
    fun createCommandLineDoesNotMergeStderr() {
        val descriptor = createDescriptor()
        val commandLine = descriptor.createCommandLine()
        assertFalse("stderr must remain a separate stream for the LSP JSON-RPC channel", commandLine.isRedirectErrorStream)
    }

    @Test
    fun semanticTokensSupportIsXsSpecific() {
        val descriptor = createDescriptor()
        assertTrue(
            "descriptor should expose an XsSemanticTokensSupport customizer",
            descriptor.lspSemanticTokensSupport is XsSemanticTokensSupport
        )
    }
}
