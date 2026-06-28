package com.aomr.xs.lsp

import com.aomr.xs.settings.XsAppSettings
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerDescriptor
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.testFramework.ProjectRule
import com.intellij.testFramework.LightVirtualFile
import org.junit.After
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test

class XsLspSupportProviderTest {

    @get:Rule
    val projectRule = ProjectRule()

    private val provider = XsLspSupportProvider()

    @Before
    fun setGamePath() {
        XsAppSettings.getInstance().setGamePath("/game/aom")
    }

    @After
    fun resetGamePath() {
        XsAppSettings.getInstance().setGamePath("")
    }

    @Test
    fun startsServerForXsFiles() {
        val starter = CapturingStarter()
        provider.fileOpened(projectRule.project, LightVirtualFile("test.xs", ""), starter)
        assertNotNull("ensureServerStarted should be called for .xs files", starter.capturedDescriptor)
    }

    @Test
    fun skipsNonXsFiles() {
        val starter = CapturingStarter()
        provider.fileOpened(projectRule.project, LightVirtualFile("test.txt", ""), starter)
        assertNull("ensureServerStarted should not be called for .txt files", starter.capturedDescriptor)
    }

    @Test
    fun skipsWhenGamePathIsBlank() {
        XsAppSettings.getInstance().setGamePath("")
        val starter = CapturingStarter()
        provider.fileOpened(projectRule.project, LightVirtualFile("test.xs", ""), starter)
        assertNull("ensureServerStarted should not be called when game path is blank", starter.capturedDescriptor)
    }

    @Test
    fun descriptorSupportsXsFiles() {
        val starter = CapturingStarter()
        provider.fileOpened(projectRule.project, LightVirtualFile("test.xs", ""), starter)
        val descriptor = starter.capturedDescriptor ?: throw AssertionError("descriptor was not created")
        assertTrue("descriptor should support .xs files", descriptor.isSupportedFile(LightVirtualFile("foo.xs", "")))
    }

    private class CapturingStarter : LspServerSupportProvider.LspServerStarter {
        var capturedDescriptor: LspServerDescriptor? = null

        override fun ensureServerStarted(descriptor: LspServerDescriptor) {
            capturedDescriptor = descriptor
        }
    }
}
