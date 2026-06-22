package com.aomr.xs.editor

import com.aomr.xs.psi.XsTokenTypes
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class XsBraceMatcherTest {

    @Test
    fun pairsReturnThreeStructuralPairs() {
        val pairs = XsBraceMatcher().pairs
        assertEquals(3, pairs.size)
    }

    @Test
    fun isPairedBracesAllowedBeforeTypeIsPermissive() {
        val matcher = XsBraceMatcher()
        assertTrue(matcher.isPairedBracesAllowedBeforeType(XsTokenTypes.LBRACE, XsTokenTypes.IDENTIFIER))
        assertTrue(matcher.isPairedBracesAllowedBeforeType(XsTokenTypes.LPAREN, null))
        assertTrue(matcher.isPairedBracesAllowedBeforeType(XsTokenTypes.LBRACKET, XsTokenTypes.RPAREN))
    }
}
