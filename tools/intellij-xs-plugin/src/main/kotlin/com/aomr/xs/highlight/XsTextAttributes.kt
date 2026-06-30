package com.aomr.xs.highlight

import com.intellij.openapi.editor.DefaultLanguageHighlighterColors
import com.intellij.openapi.editor.colors.CodeInsightColors
import com.intellij.openapi.editor.colors.EditorColors
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.editor.colors.TextAttributesKey.createTextAttributesKey

/**
 * Inherited Rider / IntelliJ color categories exposed under **XS** color scheme page.
 *
 * Each key is prefixed with `XS_` and falls back to a platform-defined key so that
 * the new categories adopt the active IDE scheme by default. Users can override
 * them in **Settings → Editor → Color Scheme → XS**.
 */
object XsTextAttributes {

    /** Identifier under caret; inherits the platform caret-line identifier highlight. */
    val IDENTIFIER_UNDER_CARET = createTextAttributesKey("XS_IDENTIFIER_UNDER_CARET", EditorColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES)

    /** Matched brace indicator; inherits platform matched-brace background. */
    val MATCHED_BRACE = createTextAttributesKey("XS_MATCHED_BRACE", CodeInsightColors.MATCHED_BRACE_ATTRIBUTES)

    /** Unmatched brace indicator; inherits platform unmatched-brace error background. */
    val UNMATCHED_BRACE = createTextAttributesKey("XS_UNMATCHED_BRACE", CodeInsightColors.UNMATCHED_BRACE_ATTRIBUTES)

    /** Unknown / unresolved symbol indicator; inherits platform wrong-reference highlighting. */
    val UNKNOWN_SYMBOL = createTextAttributesKey("XS_UNKNOWN_SYMBOL", CodeInsightColors.WRONG_REFERENCES_ATTRIBUTES)

    /** Curly braces `{` and `}`; inherits platform **Braces and Operators → Braces**. */
    val BRACES = createTextAttributesKey("XS_BRACES", DefaultLanguageHighlighterColors.BRACES)

    /** Square brackets `[` and `]`; inherits platform **Braces and Operators → Brackets**. */
    val BRACKETS = createTextAttributesKey("XS_BRACKETS", DefaultLanguageHighlighterColors.BRACKETS)

    /** Comma separator; inherits platform **Braces and Operators → Comma**. */
    val COMMA = createTextAttributesKey("XS_COMMA", DefaultLanguageHighlighterColors.COMMA)

    /** Dot / member-access `.`; inherits platform **Braces and Operators → Dot**. */
    val DOT = createTextAttributesKey("XS_DOT", DefaultLanguageHighlighterColors.DOT)

    /** Operation sign (e.g. `=`, `+`, `-`); inherits platform **Braces and Operators → Operation sign**. */
    val OPERATION_SIGN = createTextAttributesKey("XS_OPERATION_SIGN", DefaultLanguageHighlighterColors.OPERATION_SIGN)

    /**
     * Overloaded operator; no dedicated platform key exists, so it inherits the same
     * default as [OPERATION_SIGN] while appearing as a separate configurable category.
     */
    val OVERLOADED_OPERATOR = createTextAttributesKey("XS_OVERLOADED_OPERATOR", DefaultLanguageHighlighterColors.OPERATION_SIGN)

    /** Parentheses `(` and `)`; inherits platform **Braces and Operators → Parentheses**. */
    val PARENTHESES = createTextAttributesKey("XS_PARENTHESES", DefaultLanguageHighlighterColors.PARENTHESES)

    /** Semicolon `;`; inherits platform **Braces and Operators → Semi-colon**. */
    val SEMI_COLON = createTextAttributesKey("XS_SEMI_COLON", DefaultLanguageHighlighterColors.SEMICOLON)

    // -------- Semantic-token-driven categories (Bucket C, plugin 0.6.0) --------

    /** Function defined in the engine API; inherits platform **Function declaration**. */
    val FUNCTION_ENGINE = createTextAttributesKey("XS_FUNCTION_ENGINE", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)

    /** Function defined in a vanilla `<AOMR>/game/` file (not overridden in the mod). */
    val FUNCTION_UNMODDED = createTextAttributesKey("XS_FUNCTION_UNMODDED", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)

    /** Function defined in a mod's `game/` overlay file. */
    val FUNCTION_MODDED = createTextAttributesKey("XS_FUNCTION_MODDED", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)

    /** Local variable declared inside a function body or block. */
    val VARIABLE_LOCAL = createTextAttributesKey("XS_VARIABLE_LOCAL", DefaultLanguageHighlighterColors.LOCAL_VARIABLE)

    /** `static` storage-class variable. */
    val VARIABLE_STATIC = createTextAttributesKey("XS_VARIABLE_STATIC", DefaultLanguageHighlighterColors.STATIC_FIELD)

    /** Built-in primitive type (`bool`, `int`, `float`, `string`, `vector`). */
    val TYPE_BUILTIN = createTextAttributesKey("XS_TYPE_BUILTIN", DefaultLanguageHighlighterColors.KEYWORD)

    /** Class declared in a vanilla `<AOMR>/game/` file (not overridden). */
    val TYPE_UNMODDED_CLASS = createTextAttributesKey("XS_TYPE_UNMODDED_CLASS", DefaultLanguageHighlighterColors.IDENTIFIER)

    /** Class declared in a mod's `game/` overlay file. */
    val TYPE_MODDED_CLASS = createTextAttributesKey("XS_TYPE_MODDED_CLASS", DefaultLanguageHighlighterColors.IDENTIFIER)

    // -------- Additional semantic-token-driven categories (Bucket C finish, plugin 0.7.0) --------

    /** Constant (`const`) reference. */
    val CONSTANT = createTextAttributesKey("XS_CONSTANT", DefaultLanguageHighlighterColors.CONSTANT)

    /** Rule reference. */
    val RULE = createTextAttributesKey("XS_RULE", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)

    /** `extern` variable declared in vanilla (unmodded) code. */
    val VARIABLE_EXTERN_UNMODDED = createTextAttributesKey("XS_VARIABLE_EXTERN_UNMODDED", DefaultLanguageHighlighterColors.GLOBAL_VARIABLE)

    /** `extern` variable declared in a mod's overlay. */
    val VARIABLE_EXTERN_MODDED = createTextAttributesKey("XS_VARIABLE_EXTERN_MODDED", DefaultLanguageHighlighterColors.GLOBAL_VARIABLE)
}
