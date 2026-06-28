package com.aomr.xs.editor

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class XsCommenterTest {

    @Test
    fun commentPrefixesMatchXsSyntax() {
        val commenter = XsCommenter()
        assertEquals("//", commenter.lineCommentPrefix)
        assertEquals("/*", commenter.blockCommentPrefix)
        assertEquals("*/", commenter.blockCommentSuffix)
        assertNull(commenter.commentedBlockCommentPrefix)
        assertNull(commenter.commentedBlockCommentSuffix)
    }
}
