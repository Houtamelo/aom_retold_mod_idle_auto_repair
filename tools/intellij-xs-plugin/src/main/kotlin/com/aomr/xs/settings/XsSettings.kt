package com.aomr.xs.settings

import com.intellij.openapi.components.PersistentStateComponent
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.State
import com.intellij.openapi.components.Storage
import com.intellij.openapi.components.service
import com.intellij.openapi.project.Project
import java.util.concurrent.CopyOnWriteArrayList

/**
 * Project-level persistent settings for the XS Language Server client.
 *
 * Stores the list of mod roots for the current project. Project-local
 * because the user's "current project" is typically one mod (or a small
 * group of related mods), and that set changes as the user opens
 * different projects.
 *
 * The AoM:R install root (which is the same for every project) lives in
 * the application-level [XsAppSettings].
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
        var modPaths: MutableList<String> = mutableListOf()
    )

    private var state = State()

    private val listeners = CopyOnWriteArrayList<Listener>()

    override fun getState(): State = state

    override fun loadState(state: State) {
        this.state = state
    }

    /**
     * Adds a listener that is notified whenever [setModPaths] mutates the
     * persisted mod list. The listener is invoked synchronously on the same
     * thread that calls [setModPaths].
     */
    fun addModPathsListener(listener: Listener) {
        listeners.add(listener)
    }

    /**
     * Removes a previously registered listener.
     */
    fun removeModPathsListener(listener: Listener) {
        listeners.remove(listener)
    }

    /**
     * Replaces the current mod list with a new one, mutating the persisted state
     * and notifying all registered listeners.
     */
    fun setModPaths(paths: List<String>) {
        state.modPaths.clear()
        state.modPaths.addAll(paths)
        listeners.forEach { it.modPathsChanged(paths) }
    }

    /**
     * Listener contract for mod-list changes.
     */
    fun interface Listener {
        fun modPathsChanged(newPaths: List<String>)
    }

    companion object {
        fun getInstance(project: Project): XsSettings = project.service()
    }
}
