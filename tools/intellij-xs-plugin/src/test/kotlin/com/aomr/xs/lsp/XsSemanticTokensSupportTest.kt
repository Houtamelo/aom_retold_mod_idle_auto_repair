package com.aomr.xs.lsp

import org.junit.Assert.assertEquals
import org.junit.Test

class XsSemanticTokensSupportTest {

    @Test
    fun legend_matches_lsp_tokenTypes() {
        val s = XsSemanticTokensSupport()
        assertEquals(
            listOf("function", "variable", "type", "constant", "rule"),
            s.tokenTypes,
        )
    }

    @Test
    fun legend_matches_lsp_tokenModifiers() {
        val s = XsSemanticTokensSupport()
        assertEquals(
            listOf("engine", "modded", "unmodded", "local", "static", "extern", "member"),
            s.tokenModifiers,
        )
    }
}
