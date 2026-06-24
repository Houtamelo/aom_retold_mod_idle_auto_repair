package com.aomr.xs.lsp

import com.intellij.notification.NotificationGroupManager
import com.intellij.notification.NotificationType
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project
import org.eclipse.lsp4j.MessageActionItem
import org.eclipse.lsp4j.MessageParams
import org.eclipse.lsp4j.PublishDiagnosticsParams
import org.eclipse.lsp4j.RegistrationParams
import org.eclipse.lsp4j.ShowMessageRequestParams
import org.eclipse.lsp4j.services.LanguageClient
import java.util.concurrent.CompletableFuture

/**
 * Receives notifications and requests from the XS Language Server.
 *
 * Diagnostics are logged; [showMessage] is forwarded to the IDE notification
 * balloon so users see server warnings such as "file not part of any
 * registered mod" without opening the log.
 */
class XsLanguageClient(private val project: Project) : LanguageClient {

    private val log = Logger.getInstance(XsLanguageClient::class.java)

    override fun telemetryEvent(event: Any?) {
        log.debug("LSP telemetry: $event")
    }

    override fun publishDiagnostics(diagnostics: PublishDiagnosticsParams) {
        val uri = diagnostics.uri
        val count = diagnostics.diagnostics.size
        if (count == 0) {
            log.info("LSP: cleared diagnostics for $uri")
        } else {
            log.info("LSP: $count diagnostic(s) for $uri")
            diagnostics.diagnostics.forEach { d ->
                val sev = when (d.severity) {
                    org.eclipse.lsp4j.DiagnosticSeverity.Error -> "ERROR"
                    org.eclipse.lsp4j.DiagnosticSeverity.Warning -> "WARN"
                    org.eclipse.lsp4j.DiagnosticSeverity.Information -> "INFO"
                    org.eclipse.lsp4j.DiagnosticSeverity.Hint -> "HINT"
                    else -> "?"
                }
                log.info("  ${d.range.start.line}:${d.range.start.character} - sev=$sev - ${d.message}")
            }
        }
    }

    override fun showMessage(messageParams: MessageParams) {
        val type = when (messageParams.type) {
            org.eclipse.lsp4j.MessageType.Error -> NotificationType.ERROR
            org.eclipse.lsp4j.MessageType.Warning -> NotificationType.WARNING
            org.eclipse.lsp4j.MessageType.Info -> NotificationType.INFORMATION
            else -> NotificationType.INFORMATION
        }
        showBalloon(messageParams.message, type)
    }

    override fun showMessageRequest(requestParams: ShowMessageRequestParams): CompletableFuture<MessageActionItem> {
        log.info("LSP showMessageRequest: ${requestParams.message}")
        return CompletableFuture.completedFuture(null)
    }

    override fun logMessage(message: MessageParams) {
        log.info("LSP logMessage [${message.type}]: ${message.message}")
    }

    override fun registerCapability(params: RegistrationParams): CompletableFuture<Void> {
        log.info("LSP registerCapability: ${params.registrations.map { it.method }}")
        // The server may dynamically register watchers; the plugin already
        // watches the game folder via VirtualFileListener, so no further action
        // is required here. Future work can map registrations to IDE roots.
        return CompletableFuture.completedFuture(null)
    }

    private fun showBalloon(message: String, type: NotificationType) {
        val group = NotificationGroupManager.getInstance()
            .getNotificationGroup("XS Language Server")
        val notification = group.createNotification("XS Language Server", message, type)
        notification.notify(project)
    }
}
