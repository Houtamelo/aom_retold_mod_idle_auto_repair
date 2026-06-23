package com.aomr.xs.psi

import com.intellij.lang.ASTFactory
import com.intellij.psi.tree.IElementType
import com.intellij.psi.impl.source.tree.LeafElement

/**
 * Factory that creates custom AST leaf nodes for XS tokens.
 *
 * Only identifiers get a specialized leaf ([XsIdentifier]) so that reference
 * contributors are invoked. All other tokens fall back to the platform default.
 */
class XsASTFactory : ASTFactory() {

    override fun createLeaf(type: IElementType, text: CharSequence): LeafElement? {
        return if (type == XsTokenTypes.IDENTIFIER) {
            XsIdentifier(type, text)
        } else {
            null
        }
    }
}
