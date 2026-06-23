package com.aomr.xs.psi

import com.intellij.psi.PsiReference
import com.intellij.psi.impl.source.resolve.reference.ReferenceProvidersRegistry
import com.intellij.psi.impl.source.tree.LeafPsiElement
import com.intellij.psi.tree.IElementType

/**
 * Custom leaf PSI element for XS identifiers.
 *
 * Standard [LeafPsiElement] does not consult [ReferenceProvidersRegistry], so
 * reference contributors registered for XS would never be queried. This override
 * enables `Ctrl+B` on engine syscall names and, in later phases, workspace
 * identifiers.
 */
class XsIdentifier(type: IElementType, text: CharSequence) : LeafPsiElement(type, text) {

    override fun getReferences(): Array<PsiReference> =
        ReferenceProvidersRegistry.getReferencesFromProviders(this)
}
