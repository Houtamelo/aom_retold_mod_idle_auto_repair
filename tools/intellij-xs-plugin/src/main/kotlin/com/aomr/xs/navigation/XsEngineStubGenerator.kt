package com.aomr.xs.navigation

import com.aomr.xs.XsFileType
import com.aomr.xs.XsLanguage
import com.aomr.xs.constants.XsEngineApi
import com.intellij.testFramework.LightVirtualFile

/**
 * Generates in-memory XS documentation stubs for engine syscalls.
 *
 * Each stub is a [LightVirtualFile] containing a valid-looking signature and the
 * help text from [XsEngineApi]. Stubs are cached by syscall name so repeated
 * `Ctrl+B` requests resolve without regenerating the text.
 */
class XsEngineStubGenerator {
    private val cache = mutableMapOf<String, LightVirtualFile>()

    fun generateStub(syscall: XsEngineApi.Syscall): LightVirtualFile =
        cache.getOrPut(syscall.name) {
            val filename = "${syscall.name} [XS Engine Stub].xs"
            LightVirtualFile(filename, XsFileType.INSTANCE, renderStubContent(syscall)).apply {
                setLanguage(XsLanguage.INSTANCE)
            }
        }

    /**
     * Clears the in-memory stub cache. Intended for use if the engine data is ever
     * reloaded after the plugin has started.
     */
    fun invalidateCache() {
        cache.clear()
    }

    private fun renderStubContent(syscall: XsEngineApi.Syscall): String {
        val signature = formatStubSignature(syscall)
        val helpText = syscall.help.trim().ifEmpty { "(no help available)" }
        val sourceLink = doxygenLink(syscall.filename)
        val paramsComment = if (syscall.params.isEmpty()) {
            "// No parameters."
        } else {
            syscall.params.joinToString("\n") { "//   ${it.type} ${it.name}" }
        }

        return buildString {
            appendLine("// Generated stub for ${syscall.name} (${syscall.filename})")
            appendLine("// Source: $sourceLink")
            appendLine()
            appendLine("$signature;")
            appendLine("// $helpText")
            appendLine()
            appendLine(paramsComment)
        }.trimEnd()
    }

    private fun formatStubSignature(syscall: XsEngineApi.Syscall): String {
        val params = syscall.params.joinToString(", ") { "${it.type} ${it.name}" }
        return "${syscall.returnType} ${syscall.name}($params)"
    }

    private fun doxygenLink(filename: String): String {
        // Doxygen HTML output replaces the dot between basename and extension
        // with "_8", e.g. aifuncs.cpp -> aifuncs_8cpp.html.
        val base = filename.substringBeforeLast(".", filename)
        val ext = filename.substringAfterLast(".", "")
        val suffix = if (ext.isEmpty()) "" else "_8$ext"
        return "docs/doxygen_retail/${base}${suffix}.html"
    }
}
