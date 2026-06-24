package com.aomr.xs.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class XsSettingsTest {

    @Test
    fun testDefaultsAreEmpty() {
        val settings = XsSettings()
        val state = settings.state
        assertEquals("", state.gamePath)
        assertTrue(state.modPaths.isEmpty())
    }

    @Test
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

    @Test
    fun testSetModPathsReplacesList() {
        val settings = XsSettings()
        settings.setModPaths(listOf("/a", "/b"))
        assertEquals(listOf("/a", "/b"), settings.state.modPaths)

        settings.setModPaths(listOf("/c"))
        assertEquals(listOf("/c"), settings.state.modPaths)
    }

    @Test
    fun testStateInstanceIsStable() {
        val settings = XsSettings()
        val first = settings.state
        val second = settings.state
        assertSame("PersistentStateComponent should reuse the same state instance", first, second)
    }
}
