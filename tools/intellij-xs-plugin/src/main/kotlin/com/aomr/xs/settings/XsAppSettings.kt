package com.aomr.xs.settings

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.RoamingType
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.service

/**
 * Application-level (global) persistent settings for the XS Language Server
 * client.
 *
 * Stores the AoM:R install root. This is **global** because every project
 * (every mod the user opens) shares the same game installation; the
 * location is a property of the user's machine, not the project.
 *
 * Per-project settings (the list of mod roots) live in [XsSettings] at
 * [Service.Level.PROJECT].
 *
 * State is written to the IDE's application-wide options directory
 * (e.g. `~/.config/JetBrains/<IDE>/options/xsLspApp.xml` on Linux).
 */
@State(
    name = "XsLspAppSettings",
    storages = [Storage("xsLspApp.xml", roamingType = RoamingType.PER_OS)]
)
@Service(Service.Level.APP)
class XsAppSettings : PersistentStateComponent<XsAppSettings.State> {

    data class State(
        var gamePath: String = ""
    )

    private var state = State()

    override fun getState(): State = state

    override fun loadState(state: State) {
        this.state = state
    }

    /**
     * Replaces the current game folder, mutating the persisted state.
     */
    fun setGamePath(path: String) {
        state.gamePath = path
    }

    companion object {
        fun getInstance(): XsAppSettings = service()
    }
}
