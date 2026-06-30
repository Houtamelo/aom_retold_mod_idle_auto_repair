package com.aomr.xs.lsp

import com.aomr.xs.highlight.XsTextAttributes
import com.intellij.openapi.editor.colors.TextAttributesKey

/**
 * Maps LSP semantic-token classifications to XS color-scheme categories.
 *
 * The XS language server (Slice 2 of Issue #3 / `add-lsp-semantic-tokens`)
 * emits semantic tokens with a 3-token-type × 5-modifier legend:
 *
 * - Token types: `function`, `variable`, `type`
 * - Modifiers:    `engine`, `modded`, `unmodded`, `static`, `extern`
 *
 * Each (token-type, modifiers-set) tuple maps to one of the
 * `XsTextAttributes` keys that were registered with the color-scheme
 * page in `XsColorSettingsPage.getAttributeDescriptors()`. Tokens that
 * don't fall into any known category fall back to `XS_DEFAULT` so the
 * editor stays visually consistent.
 *
 * The converter is intentionally stateless — the LSP server is the
 * source of truth for origin classification; the plugin only renders
 * the result.
 */
object XsSemanticTokensConverter {

    /** Token-type identifiers; mirror the LSP server's legend order. */
    const val TOKEN_TYPE_FUNCTION = "function"
    const val TOKEN_TYPE_VARIABLE = "variable"
    const val TOKEN_TYPE_TYPE = "type"
    const val TOKEN_TYPE_CONSTANT = "constant"
    const val TOKEN_TYPE_RULE = "rule"

    /** Modifier identifiers; mirror the LSP server's legend order. */
    const val MODIFIER_ENGINE = "engine"
    const val MODIFIER_MODDED = "modded"
    const val MODIFIER_UNMODDED = "unmodded"
    const val MODIFIER_STATIC = "static"
    const val MODIFIER_EXTERN = "extern"
    const val MODIFIER_MEMBER = "member"

    /**
     * Translate an LSP semantic-token classification to an XS color-scheme key.
     *
     * @param tokenType the LSP token type (`function` / `variable` / `type`).
     * @param modifiers the LSP token modifiers (set of `engine` / `modded` /
     *                  `unmodded` / `static` / `extern`).
     * @return the XS color-scheme `TextAttributesKey` for the requested
     *         classification, or `XS_DEFAULT` for unknown combinations.
     */
    fun convert(tokenType: String, modifiers: Set<String>): TextAttributesKey {
        val modded = modifiers.contains(MODIFIER_MODDED)
        val unmodded = modifiers.contains(MODIFIER_UNMODDED)
        val engine = modifiers.contains(MODIFIER_ENGINE)
        val isStatic = modifiers.contains(MODIFIER_STATIC)
        val isExtern = modifiers.contains(MODIFIER_EXTERN)
        val isMember = modifiers.contains(MODIFIER_MEMBER)

        return when (tokenType) {
            TOKEN_TYPE_FUNCTION -> when {
                isMember && engine -> XsTextAttributes.METHOD_ENGINE
                isMember && modded -> XsTextAttributes.METHOD_MODDED
                isMember && unmodded -> XsTextAttributes.METHOD_UNMODDED
                engine -> XsTextAttributes.FUNCTION_ENGINE
                modded -> XsTextAttributes.FUNCTION_MODDED
                unmodded -> XsTextAttributes.FUNCTION_UNMODDED
                else -> XsTextAttributes.FUNCTION_UNMODDED
            }
            TOKEN_TYPE_VARIABLE -> when {
                isMember && engine -> XsTextAttributes.FIELD_ENGINE
                isMember && modded -> XsTextAttributes.FIELD_MODDED
                isMember && unmodded -> XsTextAttributes.FIELD_UNMODDED
                isExtern && unmodded -> XsTextAttributes.VARIABLE_EXTERN_UNMODDED
                isExtern && modded -> XsTextAttributes.VARIABLE_EXTERN_MODDED
                isStatic -> XsTextAttributes.VARIABLE_STATIC
                else -> XsTextAttributes.VARIABLE_LOCAL
            }
            TOKEN_TYPE_TYPE -> when {
                engine -> XsTextAttributes.TYPE_BUILTIN
                modded -> XsTextAttributes.TYPE_MODDED_CLASS
                unmodded -> XsTextAttributes.TYPE_UNMODDED_CLASS
                else -> XsTextAttributes.TYPE_UNMODDED_CLASS
            }
            TOKEN_TYPE_CONSTANT -> XsTextAttributes.CONSTANT
            TOKEN_TYPE_RULE -> XsTextAttributes.RULE
            else -> com.aomr.xs.highlight.XsTextAttributesKeys.XS_DEFAULT
        }
    }
}