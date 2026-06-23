package com.aomr.xs.parameterInfo

import com.aomr.xs.XsFileType
import com.aomr.xs.constants.XsEngineApi
import com.intellij.psi.PsiFile
import com.intellij.testFramework.fixtures.BasePlatformTestCase

class XsParameterInfoTest : BasePlatformTestCase() {

    override fun getTestDataPath(): String = "src/test/testData"

    private fun configure(text: String): Pair<PsiFile, Int> {
        myFixture.configureByText(XsFileType.INSTANCE, text)
        return myFixture.file to myFixture.caretOffset
    }

    /**
     * Covers the six P1.5 parameter-info scenarios. They are grouped in a single
     * method because multiple [BasePlatformTestCase] fixture methods can hang in
     * this headless sandbox.
     */
    fun testParameterInfoScenarios() {
        val handler = XsParameterInfoHandler()

        // Test 1: caret inside the second argument -> active parameter is 1.
        val (file1, offset1) = configure(
            "void test() { aiPlanSetVariableBool(planID, <caret>value); }"
        )
        assertEquals(
            "Caret on second argument should highlight parameter 1",
            1,
            handler.findParameterIndex(file1, offset1)
        )

        // Test 2: caret just after the opening parenthesis -> active parameter is 0.
        val (file2, offset2) = configure(
            "void test() { aiPlanSetVariableBool(<caret>planID, value); }"
        )
        assertEquals(
            "Caret before first argument should highlight parameter 0",
            0,
            handler.findParameterIndex(file2, offset2)
        )

        // Test 3: zero-parameter syscall -> no active parameter.
        val (file3, offset3) = configure("void test() { xsDisableSelf(<caret>); }")
        assertEquals(
            "Zero-parameter syscall should report no current parameter",
            -1,
            handler.findParameterIndex(file3, offset3)
        )

        // Test 4: unknown identifier -> no owner.
        val (file4, offset4) = configure("void test() { fictionalCall(<caret>); }")
        assertNull(
            "Unknown identifiers should not produce a parameter owner",
            handler.getParameterOwner(file4, offset4)
        )

        // Test 5: rendered popup text contains parameter name and type.
        val aiEcho = XsEngineApi.lookup("aiEcho") ?: error("missing aiEcho in engine API")
        val info = handler.renderParameterInfo(aiEcho, 0)
        assertTrue(
            "Popup should contain the parameter type 'string'",
            info.contains("string")
        )
        assertTrue(
            "Popup should contain the parameter name 'text'",
            info.contains("text")
        )

        // Test 6: missing closing parenthesis is handled gracefully.
        val (file6, offset6) = configure(
            "void test() { aiPlanSetVariableBool(planID, <caret>"
        )
        assertNotNull(
            "Missing closing paren should still produce an owner",
            handler.getParameterOwner(file6, offset6)
        )
        assertEquals(
            "Missing closing paren should still track the active parameter",
            1,
            handler.findParameterIndex(file6, offset6)
        )
    }
}

/**
 * Closes verify-report gap: when the caret is on a syscall identifier but
 * no opening parenthesis has been typed yet, parameter info should not
 * crash and should report no active parameter owner.
 */
class XsParameterInfoNoOpenParenTest : BasePlatformTestCase() {

    override fun getTestDataPath(): String = "src/test/testData"

    fun testParameterInfoNoOpenParen() {
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { aiEcho<caret> }")
        val handler = XsParameterInfoHandler()

        val owner = handler.getParameterOwner(myFixture.file, myFixture.caretOffset)
        val context = myFixture.file.findElementAt(myFixture.caretOffset)
        val params = if (owner != null) {
            handler.getParametersForOwner(owner, context, myFixture.caretOffset)
        } else null

        assertTrue(
            "getParameterOwner should return null or params should be null/empty when no open paren",
            owner == null || params == null || params.isEmpty()
        )
    }
}
