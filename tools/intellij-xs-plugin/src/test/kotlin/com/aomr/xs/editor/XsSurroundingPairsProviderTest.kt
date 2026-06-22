package com.aomr.xs.editor

import org.junit.Assert.assertEquals
import org.junit.Test

class XsSurroundingPairsProviderTest {

    @Test
    fun fivePairsAreDeclared() {
        val pairs = XsSurroundingPairsProvider().getPairs()
        assertEquals(
            listOf(
                "(" to ")",
                "[" to "]",
                "{" to "}",
                "\"" to "\"",
                "'" to "'"
            ),
            pairs
        )
    }
}
