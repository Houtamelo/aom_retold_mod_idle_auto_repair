package com.aomr.xs.editor

/**
 * Declarative list of characters that wrap the current selection.
 *
 * Note: IntelliJ Platform 2024.2 (the plugin's target baseline) does not yet
 * expose `com.intellij.lang.surroundingPair.SurroundingPairsProvider`. The
 * actual typing behavior for `(`, `[`, `{`, `"`, and `'` is handled by the
 * platform's brace/quote handling using [XsBraceMatcher] and [XsQuoteHandler].
 * This class is kept as a typed source-of-truth so the set of pairs can be
 * asserted in tests and reused later when the API becomes available.
 */
class XsSurroundingPairsProvider {
    fun getPairs(): List<Pair<String, String>> = PAIRS

    companion object {
        private val PAIRS = listOf(
            "(" to ")",
            "[" to "]",
            "{" to "}",
            "\"" to "\"",
            "'" to "'"
        )
    }
}
