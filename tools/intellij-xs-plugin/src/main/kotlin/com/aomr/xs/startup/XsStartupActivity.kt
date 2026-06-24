package com.aomr.xs.startup

import com.aomr.xs.lsp.XsLspServerManager
import com.aomr.xs.settings.XsAppSettings
import com.aomr.xs.settings.XsModAutoDetector
import com.aomr.xs.settings.XsSettings
import com.intellij.notification.NotificationAction
import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.actionSystem.AnActionEvent
import com.intellij.openapi.options.ShowSettingsUtil
import com.intellij.openapi.project.Project
import com.intellij.openapi.startup.StartupActivity
import com.intellij.openapi.vfs.LocalFileSystem

/**
 * First-run UX for the XS Language Server client.
 *
 * On project open this activity:
 * 1. Reads the global [XsAppSettings] (game folder) and the
 *    project-local [XsSettings] (mod list).
 * 2. If the game folder is empty, shows a non-blocking notification that
 *    opens the XS Language Server settings page.
 * 3. If the mod list is empty, runs [XsModAutoDetector] and persists results.
 * 4. Warns when auto-detection finds no mod folders.
 * 5. Starts the LSP server and sends the workspace folder list.
 */
class XsStartupActivity : StartupActivity.DumbAware {

    override fun runActivity(project: Project) {
        val appSettings = XsAppSettings.getInstance()
        val projectSettings = XsSettings.getInstance(project)

        if (appSettings.state.gamePath.isEmpty()) {
            notifyMissingGamePath(project)
            return
        }

        if (projectSettings.state.modPaths.isEmpty()) {
            val projectRoot = project.basePath?.let { LocalFileSystem.getInstance().findFileByPath(it) }
            if (projectRoot != null) {
                val detected = XsModAutoDetector.scan(projectRoot)
                if (detected.isEmpty()) {
                    notifyNoModsDetected(project)
                } else {
                    projectSettings.setModPaths(detected)
                }
            } else {
                notifyNoModsDetected(project)
            }
        }

        XsLspServerManager.getInstance(project).updateSettings(
            gamePath = appSettings.state.gamePath,
            modPaths = projectSettings.state.modPaths
        )
    }

    private fun notifyMissingGamePath(project: Project) {
        val notification = NotificationGroupManager.getInstance()
            .getNotificationGroup("XS Language Server")
            .createNotification(
                "Game folder not configured",
                "Open Settings → Languages & Frameworks → XS Language Server to set the Age of Mythology: Retold install root.",
                NotificationType.WARNING
            )
            .addAction(OpenSettingsAction(project))
        notification.notify(project)
    }

    private fun notifyNoModsDetected(project: Project) {
        val notification = NotificationGroupManager.getInstance()
            .getNotificationGroup("XS Language Server")
            .createNotification(
                "No mod folders detected",
                "No game/ directories were found in this project. Add mod paths manually in Settings → XS Language Server → Mods.",
                NotificationType.WARNING
            )
            .addAction(OpenSettingsAction(project))
        notification.notify(project)
    }

    private class OpenSettingsAction(private val project: Project) : NotificationAction("Open Settings") {
        override fun actionPerformed(e: AnActionEvent, notification: com.intellij.notification.Notification) {
            ShowSettingsUtil.getInstance().showSettingsDialog(project, com.aomr.xs.settings.XsConfigurable.DISPLAY_NAME)
            notification.expire()
        }
    }
}
