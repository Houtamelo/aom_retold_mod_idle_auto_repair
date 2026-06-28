package com.aomr.xs.lsp

import com.aomr.xs.settings.XsAppSettings
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServer
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.platform.lsp.api.lsWidget.LspServerWidgetItem

/**
 * Tells the IntelliJ Platform when to start the XS Language Server.
 *
 * The platform calls [fileOpened] for every opened file. If the file is an
 * `.xs` file and the user has configured a game folder, we ask the platform to
 * ensure the server described by [XsLspServerDescriptor] is running.
 */
class XsLspSupportProvider : LspServerSupportProvider {

    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        serverStarter: LspServerSupportProvider.LspServerStarter,
    ) {
        if (file.extension != "xs") return
        val gamePath = XsAppSettings.getInstance().state.gamePath
        if (gamePath.isBlank()) return
        serverStarter.ensureServerStarted(XsLspServerDescriptor.create(project))
    }

    override fun createLspServerWidgetItem(
        lspServer: LspServer,
        currentFile: VirtualFile?,
    ): LspServerWidgetItem? = null
}
