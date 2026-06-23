package com.aomr.xs.navigation

import com.aomr.xs.XsLanguage
import com.aomr.xs.constants.XsEngineApi
import com.aomr.xs.psi.XsTokenTypes
import com.intellij.openapi.util.TextRange
import com.intellij.patterns.PlatformPatterns
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiManager
import com.intellij.psi.PsiReference
import com.intellij.psi.PsiReferenceBase
import com.intellij.psi.PsiReferenceContributor
import com.intellij.psi.PsiReferenceProvider
import com.intellij.psi.PsiReferenceRegistrar
import com.intellij.util.ProcessingContext

/**
 * Contributes PSI references for engine syscall identifiers.
 *
 * When the user presses `Ctrl+B` on a known syscall name, the resulting
 * reference resolves to a generated in-memory stub produced by
 * [XsEngineStubGenerator]. Unknown identifiers receive no references.
 */
class XsEngineReferenceContributor : PsiReferenceContributor() {

    override fun registerReferenceProviders(registrar: PsiReferenceRegistrar) {
        registrar.registerReferenceProvider(
            PlatformPatterns.psiElement(XsTokenTypes.IDENTIFIER).withLanguage(XsLanguage.INSTANCE),
            XsEngineReferenceProvider()
        )
    }
}

private class XsEngineReferenceProvider : PsiReferenceProvider() {
    override fun getReferencesByElement(
        element: PsiElement,
        context: ProcessingContext
    ): Array<PsiReference> {
        val name = element.text
        val syscall = XsEngineApi.lookup(name) ?: return PsiReference.EMPTY_ARRAY
        return arrayOf(XsEngineReference(element, syscall))
    }
}

class XsEngineReference(
    element: PsiElement,
    private val syscall: XsEngineApi.Syscall
) : PsiReferenceBase<PsiElement>(element, TextRange(0, element.textLength)) {

    override fun resolve(): PsiElement? {
        val project = element.project
        val stubFile = stubGenerator.generateStub(syscall)
        return PsiManager.getInstance(project).findFile(stubFile)
    }

    override fun isReferenceTo(element: PsiElement): Boolean =
        resolve()?.isEquivalentTo(element) ?: false

    override fun getVariants(): Array<Any> = emptyArray()

    companion object {
        private val stubGenerator = XsEngineStubGenerator()
    }
}
