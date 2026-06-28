package com.aomr.xs.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Project-local settings store. Holds the list of mod roots for the
 * current project. The game folder is **not** here — that lives in
 * [XsAppSettings] at the application level.
 */
class XsSettingsTest {

    @Test
    fun testDefaultsAreEmpty() {
        val settings = XsSettings()
        val state = settings.state
        assertTrue(state.modPaths.isEmpty())
    }

    @Test
    fun testLoadStateRestoresValues() {
        val settings = XsSettings()
        val loaded = XsSettings.State(
            modPaths = mutableListOf("/mod/one", "/mod/two")
        )
        settings.loadState(loaded)

        val state = settings.state
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
