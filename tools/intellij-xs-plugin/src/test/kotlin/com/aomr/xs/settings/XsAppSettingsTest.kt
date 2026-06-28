package com.aomr.xs.settings

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * Application-level (global) settings store. Holds the AoM:R install
 * root, which is shared across every project. Per-project state lives in
 * [XsSettings].
 */
class XsAppSettingsTest {

    @Test
    fun testDefaultsAreEmpty() {
        val settings = XsAppSettings()
        assertEquals("", settings.state.gamePath)
    }

    @Test
    fun testLoadStateRestoresValue() {
        val settings = XsAppSettings()
        val loaded = XsAppSettings.State(gamePath = "/game/aom")
        settings.loadState(loaded)
        assertEquals("/game/aom", settings.state.gamePath)
    }

    @Test
    fun testSetGamePathUpdatesState() {
        val settings = XsAppSettings()
        settings.setGamePath("/game/aom")
        assertEquals("/game/aom", settings.state.gamePath)

        settings.setGamePath("/game/other")
        assertEquals("/game/other", settings.state.gamePath)
    }
}
