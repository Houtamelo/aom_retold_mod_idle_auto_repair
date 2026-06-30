package com.aomr.xs.lsp

import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.platform.lsp.api.customization.LspSemanticTokensSupport

/**
 * Bridges the platform LSP semantic-token provider to the XS color scheme.
 *
 * The 2024.2.2 `LspSemanticTokensSupport` API returns `List<String>` from
 * `tokenTypes`/`tokenModifiers` and receives token/modifier identifiers as
 * strings in `getTextAttributesKey`. This class advertises the exact same
 * type/modifier legends the XS LSP server uses so modifier bitsets are not
 * silently dropped by the platform before [getTextAttributesKey] is called.
 *
 * It delegates the actual (tokenType, modifiers) mapping to
 * [XsSemanticTokensConverter] so the same logic is unit-testable without the
 * platform.
 */
internal class XsSemanticTokensSupport : LspSemanticTokensSupport() {
    override fun getTextAttributesKey(tokenType: String, modifiers: List<String>): TextAttributesKey =
        XsSemanticTokensConverter.convert(tokenType, modifiers.toSet())

    override val tokenTypes: List<String> = listOf(
        XsSemanticTokensConverter.TOKEN_TYPE_FUNCTION,
        XsSemanticTokensConverter.TOKEN_TYPE_VARIABLE,
        XsSemanticTokensConverter.TOKEN_TYPE_TYPE,
        XsSemanticTokensConverter.TOKEN_TYPE_CONSTANT,
        XsSemanticTokensConverter.TOKEN_TYPE_RULE,
    )

    override val tokenModifiers: List<String> = listOf(
        "engine", "modded", "unmodded", "local", "static", "extern", "member",
    )
}
