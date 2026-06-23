package com.aomr.xs.documentation

import com.aomr.xs.constants.XsEngineApi
import com.aomr.xs.psi.XsTokenTypes
import com.intellij.psi.impl.source.tree.LeafPsiElement
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class XsHoverTest {

    private val provider = XsDocumentationProvider()

    private fun elementWithName(name: String) = LeafPsiElement(XsTokenTypes.IDENTIFIER, name)

    /**
     * Scenario 1: Hover over `aiEcho` shows the syscall name, parameter type,
     * parameter name, and the full help paragraph.
     */
    @Test
    fun hoverOverAiEchoShowsSignatureHelpAndParameters() {
        val aiEchoHelp = XsEngineApi.lookup("aiEcho")?.help ?: error("missing aiEcho in engine API")
        val doc = provider.generateDoc(elementWithName("aiEcho"), null)
            ?: error("Expected hover documentation for aiEcho")

        assertTrue("Doc should contain syscall name aiEcho", doc.contains("aiEcho"))
        assertTrue("Doc should mention parameter type string", doc.contains("string"))
        assertTrue("Doc should mention parameter name text", doc.contains("text"))
        assertTrue(
            "Doc should contain the original help text",
            decodeHtmlEntities(doc).contains(aiEchoHelp)
        )
    }

    /**
     * Scenario 2: Hover over `kbUnitCount` returns a string containing the
     * syscall name and its return type.
     */
    @Test
    fun hoverOverKbUnitCountShowsSignature() {
        val doc = provider.generateDoc(elementWithName("kbUnitCount"), null)
            ?: error("Expected hover documentation for kbUnitCount")

        assertTrue("Doc should contain kbUnitCount", doc.contains("kbUnitCount"))
        assertTrue("Doc should show the return type int", doc.contains("int"))
    }

    /**
     * Scenario 3: Hover over an unknown identifier returns null, which tells
     * the platform not to show a popup.
     */
    @Test
    fun hoverOverUnknownIdentifierReturnsNull() {
        assertNull(
            "Unknown identifiers should not produce documentation",
            provider.generateDoc(elementWithName("fictionalName"), null)
        )
    }

    /**
     * Scenario 4: A syscall with an empty help field shows a deterministic
     * placeholder instead of a blank popup.
     */
    @Test
    fun hoverOverEmptyHelpSyscallShowsPlaceholder() {
        val blankHelpSyscall = XsEngineApi.Syscall(
            name = "xsEmptyHelpTest",
            help = "   ",
            returnType = "void",
            params = emptyList(),
            filename = "test.cpp"
        )
        val emptyDoc = provider.renderDocumentation(blankHelpSyscall)
        assertTrue("Doc should show the placeholder for empty help", emptyDoc.contains("(no help available)"))

        val aiEchoSyscall = XsEngineApi.lookup("aiEcho") ?: error("missing aiEcho")
        assertFalse(
            "Non-empty help should not show the placeholder",
            provider.renderDocumentation(aiEchoSyscall).contains("(no help available)")
        )
    }

    /**
     * Scenario 5: `generateDoc` must not block the UI. It performs only an
     * in-memory lookup and string formatting, so 1,000 calls should finish
     * well under 100 ms.
     */
    @Test(timeout = 100_000L) // Safety net: fail if the test hangs (100 s).
    fun generateDocIsFast() {
        val element = elementWithName("aiEcho")
        val start = System.nanoTime()
        repeat(1000) {
            provider.generateDoc(element, null)
        }
        val elapsedMs = (System.nanoTime() - start) / 1_000_000.0
        assertTrue(
            "generateDoc took ${elapsedMs}ms for 1000 calls, exceeding 100ms budget",
            elapsedMs < 100.0
        )
    }

    private fun decodeHtmlEntities(html: String): String =
        html.replace("&quot;", "\"")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&amp;", "&")
}
