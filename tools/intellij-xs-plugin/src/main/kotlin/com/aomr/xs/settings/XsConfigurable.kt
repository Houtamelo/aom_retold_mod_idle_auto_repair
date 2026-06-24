package com.aomr.xs.settings

import com.aomr.xs.lsp.XsLspServerManager
import com.intellij.openapi.fileChooser.FileChooserDescriptorFactory
import com.intellij.openapi.options.Configurable
import com.intellij.openapi.project.Project
import com.intellij.openapi.ui.TextFieldWithBrowseButton
import com.intellij.openapi.vfs.LocalFileSystem
import com.intellij.ui.CollectionListModel
import com.intellij.ui.ToolbarDecorator
import com.intellij.ui.components.JBList
import com.intellij.ui.dsl.builder.Align
import com.intellij.ui.dsl.builder.panel
import java.awt.GraphicsEnvironment
import java.awt.datatransfer.DataFlavor
import java.awt.datatransfer.StringSelection
import java.awt.datatransfer.Transferable
import javax.swing.JComponent
import javax.swing.JList
import javax.swing.ListSelectionModel
import javax.swing.TransferHandler

/**
 * Settings UI for the XS Language Server client.
 *
 * Appears under Settings → Languages & Frameworks → XS Language Server.
 *
 * Two scopes of settings:
 * - **Game folder** is global (one game install per machine). Backed by
 *   [XsAppSettings] and shared across every project.
 * - **Mod paths** are per-project. Backed by [XsSettings].
 */
class XsConfigurable(private val project: Project) : Configurable {

    private val appSettings = XsAppSettings.getInstance()
    private val projectSettings = XsSettings.getInstance(project)
    private val gamePathField = TextFieldWithBrowseButton()
    private val modModel = CollectionListModel<String>()
    private val modList = JBList(modModel)

    init {
        modList.selectionMode = ListSelectionModel.SINGLE_SELECTION
        // Drag-and-drop reorder is not available in headless environments
        // (e.g. CI / buildSearchableOptions). Guard so plugin packaging still
        // works; the list retains +/− buttons for editing in all modes.
        if (!GraphicsEnvironment.isHeadless()) {
            modList.dragEnabled = true
            modList.transferHandler = ReorderTransferHandler()
        }
    }

    override fun getDisplayName(): String = DISPLAY_NAME

    override fun createComponent(): JComponent {
        gamePathField.addBrowseFolderListener(
            "Select Game Folder",
            "Age of Mythology: Retold install root (contains game/ and doxygen_retail.7z)",
            project,
            FileChooserDescriptorFactory.createSingleFolderDescriptor()
        )

        val decoratedList = ToolbarDecorator.createDecorator(modList)
            .setAddAction { addModFolder() }
            .setRemoveAction { removeSelectedModFolder() }
            .createPanel()

        return panel {
            row("Game folder:") {
                cell(gamePathField)
                    .align(Align.FILL)
                    .resizableColumn()
            }
            row("Mods:") {
                cell(decoratedList)
                    .align(Align.FILL)
                    .resizableColumn()
            }
            row {
                button("Auto-detect") { runAutoDetect() }
            }
        }
    }

    override fun isModified(): Boolean {
        return gamePathField.text != appSettings.state.gamePath ||
            modModel.items != projectSettings.state.modPaths
    }

    override fun apply() {
        // Game folder is global — persists across every project.
        appSettings.setGamePath(gamePathField.text.trim())
        // Mod paths are project-local.
        projectSettings.setModPaths(modModel.items)
        // Notify the LSP manager so it can update its workspace folders
        // (no restart needed for a mod list change) or restart (if the
        // game path changed).
        XsLspServerManager.getInstance(project).updateSettings(
            gamePath = appSettings.state.gamePath,
            modPaths = projectSettings.state.modPaths
        )
    }

    override fun reset() {
        gamePathField.text = appSettings.state.gamePath
        modModel.removeAll()
        modModel.add(projectSettings.state.modPaths)
    }

    override fun disposeUIResources() {
        modModel.removeAll()
    }

    private fun addModFolder() {
        val descriptor = FileChooserDescriptorFactory.createSingleFolderDescriptor()
            .withTitle("Select Mod Root")
            .withDescription("Folder that contains a game/ subdirectory")
        com.intellij.openapi.fileChooser.FileChooser.chooseFile(
            descriptor,
            project,
            null
        ) { file ->
            val path = file.path
            if (path !in modModel.items) {
                modModel.add(path)
            }
        }
    }

    private fun removeSelectedModFolder() {
        val selected = modList.selectedIndex
        if (selected >= 0) {
            modModel.remove(selected)
        }
    }

    private fun runAutoDetect() {
        val root = project.basePath?.let { LocalFileSystem.getInstance().findFileByPath(it) }
            ?: return
        val detected = XsModAutoDetector.scan(root)
        if (detected.isEmpty()) {
            com.intellij.openapi.ui.Messages.showWarningDialog(
                project,
                "No mod folders found. Add paths manually below.",
                "Auto-detect"
            )
            return
        }
        for (path in detected) {
            if (path !in modModel.items) {
                modModel.add(path)
            }
        }
    }

    /**
     * Simple drag-reorder handler for the mod list: dragging an item moves it
     * to the drop index. Data is carried as the stringified path; the actual
     * move is performed by index bookkeeping in [getSourceActions] and
     * [importData].
     */
    private inner class ReorderTransferHandler : TransferHandler() {

        private var sourceIndex = -1

        override fun getSourceActions(c: JComponent): Int {
            sourceIndex = modList.selectedIndex
            return MOVE
        }

        override fun createTransferable(c: JComponent): Transferable? {
            val index = modList.selectedIndex
            if (index < 0) return null
            return StringSelection(modModel.getElementAt(index))
        }

        override fun canImport(support: TransferSupport): Boolean {
            return support.isDataFlavorSupported(DataFlavor.stringFlavor)
        }

        override fun importData(support: TransferSupport): Boolean {
            if (!canImport(support)) return false
            val dropLocation = support.dropLocation as? JList.DropLocation ?: return false
            val targetIndex = dropLocation.index.coerceIn(0, modModel.size - 1)
            val path = support.transferable.getTransferData(DataFlavor.stringFlavor) as? String ?: return false
            val currentIndex = modModel.items.indexOf(path)
            if (currentIndex < 0) return false
            if (currentIndex == targetIndex) return true
            modModel.remove(currentIndex)
            val insertIndex = if (targetIndex > currentIndex) targetIndex - 1 else targetIndex
            modModel.add(insertIndex, path)
            modList.selectedIndex = insertIndex
            return true
        }
    }

    companion object {
        const val DISPLAY_NAME = "XS Language Server"
    }
}
