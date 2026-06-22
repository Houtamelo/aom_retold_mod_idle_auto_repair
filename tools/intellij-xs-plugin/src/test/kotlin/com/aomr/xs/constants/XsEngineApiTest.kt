package com.aomr.xs.constants

import org.junit.Assert.*
import org.junit.Test

class XsEngineApiTest {

    @Test
    fun lookupKnownSyscall() {
        val syscall = XsEngineApi.lookup("aiEcho")
        assertNotNull("aiEcho should be present in engine API", syscall)
        assertEquals("void", syscall!!.returnType)
        assertEquals(1, syscall.params.size)
        assertEquals("string", syscall.params[0].type)
    }

    @Test
    fun lookupUnknownSyscallReturnsNull() {
        assertNull("Unknown syscalls should not resolve", XsEngineApi.lookup("NONEXISTENT"))
    }

    @Test
    fun sizeMatchesVendoredSyscallCount() {
        assertEquals(1805, XsEngineApi.size)
    }

    @Test
    fun searchByPrefixFiltersCaseSensitively() {
        val matches = XsEngineApi.searchByPrefix("aiE").map { it.name }

        assertTrue("aiEcho should match aiE", matches.contains("aiEcho"))
        assertTrue("aiEchoCategory should match aiE", matches.contains("aiEchoCategory"))
        assertTrue("aiEchoWarning should match aiE", matches.contains("aiEchoWarning"))
        assertFalse("kbUnitCount should not match aiE", matches.contains("kbUnitCount"))
    }

    @Test
    fun allReturnsSameCountAsSize() {
        assertEquals(XsEngineApi.size, XsEngineApi.all().size)
        assertTrue("all() should be sorted by name", XsEngineApi.all().zipWithNext { a, b ->
            a.name <= b.name
        }.all { it })
    }
}
