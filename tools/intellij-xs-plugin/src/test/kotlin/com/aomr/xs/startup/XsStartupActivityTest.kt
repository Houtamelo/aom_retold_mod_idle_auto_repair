package com.aomr.xs.startup

import com.aomr.xs.lsp.XsLspServerManager
import com.aomr.xs.settings.XsAppSettings
import com.aomr.xs.settings.XsSettings
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.testFramework.ProjectRule
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import java.io.File

class XsStartupActivityTest {

    @get:Rule
    val projectRule = ProjectRule()

    @After
    fun resetSettings() {
        XsAppSettings.getInstance().setGamePath("")
        XsSettings.getInstance(projectRule.project).setModPaths(emptyList())
    }

    @Test
    fun missingGamePathSkipsAutoDetectionAndServerNotification() {
        val project = projectRule.project
        XsAppSettings.getInstance().setGamePath("")

        // Create mod folders so the only reason nothing happens is the
        // missing game path.
        createGameFoldersUnderProjectRoot(project, "mod_a", "mod_b")

        val settings = XsSettings.getInstance(project)
        val manager = XsLspServerManager(project)
        val captured = mutableListOf<Pair<List<String>, List<String>>>()
        manager.sendWorkspaceFolderChange = { added, removed ->
            captured.add(added to removed)
            true
        }

        XsStartupActivity().runActivity(project)

        assertTrue("mod list should remain empty when game path is missing", settings.state.modPaths.isEmpty())
        assertTrue("no workspace-folder notification should be sent when game path is missing", captured.isEmpty())
    }

    @Test
    fun autoDetectedModsAreSavedAndNotified() {
        val project = projectRule.project
        XsAppSettings.getInstance().setGamePath("/game/age of mythology retold")

        createGameFoldersUnderProjectRoot(project, "mod_a", "mod_b")

        val settings = XsSettings.getInstance(project)
        val manager = XsLspServerManager(project)
        val captured = mutableListOf<Pair<List<String>, List<String>>>()
        manager.sendWorkspaceFolderChange = { added, removed ->
            captured.add(added to removed)
            true
        }

        XsStartupActivity().runActivity(project)

        val detected = settings.state.modPaths
        assertEquals(2, detected.size)
        assertTrue("expected mod_a root", detected.any { it.endsWith("mod_a") })
        assertTrue("expected mod_b root", detected.any { it.endsWith("mod_b") })

        assertEquals(1, captured.size)
        val added = captured[0].first
        assertEquals(2, added.size)
        assertTrue(added.any { it.endsWith("mod_a") })
        assertTrue(added.any { it.endsWith("mod_b") })
    }

    private fun createGameFoldersUnderProjectRoot(project: com.intellij.openapi.project.Project, vararg modNames: String) {
        val root = project.basePath?.let { File(it) }
            ?: throw AssertionError("project base path is null")
        root.mkdirs()
        for (name in modNames) {
            File(root, "$name/game/ai").mkdirs()
        }
        LocalFileSystem.getInstance().refreshIoFiles(listOf(root), false, true, null)
    }
}
