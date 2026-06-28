package com.aomr.xs.lsp

import com.aomr.xs.settings.XsSettings
import com.intellij.testFramework.ProjectRule
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class XsLspServerManagerTest {

    @get:Rule
    val projectRule = ProjectRule()

    @After
    fun resetSettings() {
        XsSettings.getInstance(projectRule.project).setModPaths(emptyList())
    }

    @Test
    fun replacingPersistedModListSendsRemovedAndAdded() {
        val project = projectRule.project
        val settings = XsSettings.getInstance(project)
        settings.setModPaths(listOf("/A", "/B"))

        val manager = XsLspServerManager(project)
        val captured = mutableListOf<Pair<List<String>, List<String>>>()
        manager.sendWorkspaceFolderChange = { added, removed ->
            captured.add(added to removed)
            true
        }

        manager.notifyWorkspaceFoldersChanged(listOf("/A", "/C"))

        assertEquals(1, captured.size)
        assertEquals(listOf("/C"), captured[0].first)
        assertEquals(listOf("/B"), captured[0].second)
    }

    @Test
    fun modPathsListenerIsWired() {
        val project = projectRule.project
        val settings = XsSettings.getInstance(project)

        val manager = XsLspServerManager(project)
        val captured = mutableListOf<Pair<List<String>, List<String>>>()
        manager.sendWorkspaceFolderChange = { added, removed ->
            captured.add(added to removed)
            true
        }

        settings.setModPaths(listOf("/A", "/B"))

        assertEquals(1, captured.size)
        assertEquals(listOf("/A", "/B"), captured[0].first)
        assertEquals(emptyList<String>(), captured[0].second)
    }

    @Test
    fun sendFailureTriggersRestart() {
        val project = projectRule.project
        val manager = XsLspServerManager(project)

        val restarts = mutableListOf<Unit>()
        manager.restartServerHandler = { restarts.add(Unit) }
        manager.sendWorkspaceFolderChange = { _, _ -> false }

        manager.notifyWorkspaceFoldersChanged(listOf("/X"))

        assertEquals(1, restarts.size)
    }

    @Test
    fun initialBaselineReflectsPersistedSettings() {
        val project = projectRule.project
        val settings = XsSettings.getInstance(project)
        settings.setModPaths(listOf("/A", "/B"))

        val manager = XsLspServerManager(project)
        val captured = mutableListOf<Pair<List<String>, List<String>>>()
        manager.sendWorkspaceFolderChange = { added, removed ->
            captured.add(added to removed)
            true
        }

        manager.notifyWorkspaceFoldersChanged(listOf("/A", "/B"))

        assertTrue("no delta should be sent when the mod list has not changed", captured.isEmpty())
    }
}
