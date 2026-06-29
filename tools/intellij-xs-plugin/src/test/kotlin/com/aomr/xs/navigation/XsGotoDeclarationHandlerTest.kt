package com.aomr.xs.navigation

import com.aomr.xs.XsLanguage
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServer
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFileFactory
import com.intellij.testFramework.fixtures.BasePlatformTestCase

/**
 * Strict-TDD tests for [XsGotoDeclarationHandler].
 *
 * Uses a stub [XsDefinitionResolver] so the suite does not need a running LSP
 * server. Verifies the handler filters non-XS files, delegates to the resolver,
 * maps single and multiple targets, and tolerates an empty resolver result.
 */
class XsGotoDeclarationHandlerTest : BasePlatformTestCase() {

    fun testSingleTargetReturnsOneNavigable() {
        val psiFile = myFixture.configureByText("a.xs", "void foo() {}\nvoid bar() { foo(); }")
        val target = psiFile.findElementAt(psiFile.text.indexOf("foo"))!!
        val handler = XsGotoDeclarationHandler(TestResolver(listOf(target)))

        val call = psiFile.findElementAt(psiFile.text.lastIndexOf("foo"))!!
        val result = handler.getGotoDeclarationTargets(call, call.textOffset, myFixture.editor)

        assertSize(1, result ?: emptyArray())
        assertSame(target, result!![0])
    }

    fun testMultiTargetReturnsMultipleNavigables() {
        val psiFile = myFixture.configureByText("a.xs", "void a() {}")
        val t1 = psiFile.findElementAt(psiFile.text.indexOf("a"))!!
        val t2 = psiFile.findElementAt(psiFile.text.lastIndexOf("a"))!!
        val handler = XsGotoDeclarationHandler(TestResolver(listOf(t1, t2)))
        val result = handler.getGotoDeclarationTargets(t1, 0, myFixture.editor)
        assertSize(2, result ?: emptyArray())
    }

    fun testNonXsFileReturnsNull() {
        val txt = myFixture.configureByText("a.txt", "hello")
        val handler = XsGotoDeclarationHandler(TestResolver(emptyList()))
        assertNull(handler.getGotoDeclarationTargets(txt.findElementAt(0), 0, myFixture.editor))
    }

    fun testLspTimeoutReturnsEmpty() {
        val psiFile = myFixture.configureByText("a.xs", "void a() {}")
        val handler = XsGotoDeclarationHandler(TestResolver(emptyList()))
        val element = psiFile.findElementAt(0)!!
        val result = handler.getGotoDeclarationTargets(element, 0, myFixture.editor)
        assertNotNull(result)
        assertEmpty(result!!.asList())
    }

    fun testExternalFileNavigatesCorrectly() {
        val sourceFile = myFixture.configureByText("a.xs", "void a() {}")
        val editor = myFixture.editor

        val externalFile = myFixture.configureByText("external.xs", "void b() {}")
        val externalTarget = externalFile.findElementAt(externalFile.text.indexOf("b"))!!

        val handler = XsGotoDeclarationHandler(TestResolver(listOf(externalTarget)))
        val result = handler.getGotoDeclarationTargets(sourceFile.findElementAt(0)!!, 0, editor)
        assertSize(1, result ?: emptyArray())
        assertEquals("external.xs", result!![0].containingFile.name)
    }

    private class TestResolver(private val results: List<PsiElement>) : XsDefinitionResolver {
        override fun resolve(server: LspServer?, file: VirtualFile, offset: Int) = results
    }
}
