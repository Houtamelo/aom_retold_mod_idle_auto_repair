package com.aomr.xs.settings

import com.intellij.testFramework.LightPlatformTestCase

class XsSettingsTest : LightPlatformTestCase() {

    fun testDefaultsAreEmpty() {
        val settings = XsSettings()
        val state = settings.state
        assertEquals("", state.gamePath)
        assertTrue(state.modPaths.isEmpty())
    }

    fun testLoadStateRestoresValues() {
        val settings = XsSettings()
        val loaded = XsSettings.State(
            gamePath = "/game/aom",
            modPaths = mutableListOf("/mod/one", "/mod/two")
        )
        settings.loadState(loaded)

        val state = settings.state
        assertEquals("/game/aom", state.gamePath)
        assertEquals(listOf("/mod/one", "/mod/two"), state.modPaths)
    }

    fun testSetModPathsReplacesList() {
        val settings = XsSettings()
        settings.setModPaths(listOf("/a", "/b"))
        assertEquals(listOf("/a", "/b"), settings.state.modPaths)

        settings.setModPaths(listOf("/c"))
        assertEquals(listOf("/c"), settings.state.modPaths)
    }

    fun testProjectServiceInstanceReturnsSameComponent() {
        val project = project
        val first = XsSettings.getInstance(project)
        val second = XsSettings.getInstance(project)
        assertSame(first, second)
    }
}
