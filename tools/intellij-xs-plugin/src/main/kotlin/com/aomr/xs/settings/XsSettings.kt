package com.aomr.xs.settings

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.service
import com.intellij.openapi.project.Project

/**
 * Project-level persistent settings for the XS Language Server client.
 *
 * State is written to `.idea/xsLsp.xml` (or the equivalent per-project
 * IDEA state location) and restored when the project reopens.
 */
@State(
    name = "XsLspSettings",
    storages = [Storage("xsLsp.xml")]
)
@Service(Service.Level.PROJECT)
class XsSettings : PersistentStateComponent<XsSettings.State> {

    data class State(
        var gamePath: String = "",
        var modPaths: MutableList<String> = mutableListOf()
    )

    private var state = State()

    override fun getState(): State = state

    override fun loadState(state: State) {
        this.state = state
    }

    /**
     * Replaces the current mod list with a new one, mutating the persisted state.
     */
    fun setModPaths(paths: List<String>) {
        state.modPaths.clear()
        state.modPaths.addAll(paths)
    }

    companion object {
        fun getInstance(project: Project): XsSettings = project.service()
    }
}
