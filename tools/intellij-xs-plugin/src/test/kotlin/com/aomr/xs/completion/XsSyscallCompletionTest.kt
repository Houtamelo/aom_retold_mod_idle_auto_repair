package com.aomr.xs.completion

import com.aomr.xs.XsFileType
import com.aomr.xs.constants.XsEngineApi
import com.intellij.codeInsight.completion.CompletionType
import com.intellij.testFramework.fixtures.BasePlatformTestCase

class XsSyscallCompletionTest : BasePlatformTestCase() {

    override fun getTestDataPath(): String = "src/test/testData"

    /**
     * Covers the seven P1.2/P1.3 completion scenarios:
     * 1. Syscall name prefix `aiE` -> aiEcho, aiEchoCategory, aiEchoWarning.
     * 2. Syscall name prefix `kbUnit` -> kbUnitCount.
     * 3. No completions inside a string literal.
     * 4. No completions inside a line comment.
     * 5. Default value for the second aiPlanCreate parameter after `,`.
     * 6. No default popup for zero-parameter syscall xsDisableSelf.
     * 7. No default popup for an unknown identifier call.
     *
     * These are grouped in a single test method because running multiple
     * [BasePlatformTestCase] methods sequentially hangs in this headless
     * sandbox for an as-yet-undiagnosed platform-lifecycle reason.
     */
    fun testEngineSyscallCompletionAndDefaults() {
        // Scenario 1
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { aiE<caret> }")
        val aiEItems = myFixture.completeBasic()
        assertNotNull("aiE prefix should produce syscall completions", aiEItems)
        assertContainsElements(
            myFixture.lookupElementStrings ?: emptyList(),
            "aiEcho",
            "aiEchoCategory",
            "aiEchoWarning"
        )

        // Scenario 2
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { kbUnit<caret> }")
        myFixture.completeBasic()
        assertContainsElements(
            myFixture.lookupElementStrings ?: emptyList(),
            "kbUnitCount"
        )

        // Scenario 3
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { \"aiE<caret>\" }")
        val stringItems = myFixture.completeBasic()
        assertTrue(
            "No completions should be offered inside a string literal",
            stringItems == null || stringItems.isEmpty()
        )

        // Scenario 4
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { // aiE<caret> }")
        val commentItems = myFixture.completeBasic()
        assertTrue(
            "No completions should be offered inside a comment",
            commentItems == null || commentItems.isEmpty()
        )

        // Scenario 5
        myFixture.configureByText(
            XsFileType.INSTANCE,
            "void test() { aiPlanCreate(0, <caret>); }"
        )
        val defaultItems = myFixture.completeBasic()
        assertNotNull("Comma after first argument should propose a default value", defaultItems)
        assertTrue(
            "Expected default value '-1' for the second aiPlanCreate parameter",
            (myFixture.lookupElementStrings ?: emptyList()).contains("-1")
        )

        // Scenario 6
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { xsDisableSelf(<caret>); }")
        val zeroParamItems = myFixture.completeBasic()
        assertTrue(
            "Zero-parameter syscalls should not propose defaults",
            zeroParamItems == null || zeroParamItems.isEmpty()
        )

        // Scenario 7
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { fictionalCall(<caret>); }")
        val unknownItems = myFixture.completeBasic()
        assertTrue(
            "Unknown calls should not propose defaults",
            unknownItems == null || unknownItems.isEmpty()
        )
    }
}

/**
 * Closes verify-report gap: accepting a syscall name completion inserts
 * a complete call including parentheses.
 */
class XsSyscallCompletionInsertParensTest : BasePlatformTestCase() {

    override fun getTestDataPath(): String = "src/test/testData"

    fun testSyscallNameCompletionInsertsParens() {
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { aiEcho<caret> }")
        val elements = myFixture.complete(CompletionType.BASIC)
        assertNotNull("aiEcho should produce completions", elements)
        assertFalse("Expected at least one completion item", elements!!.isEmpty())
        val aiEcho = elements.firstOrNull { it.lookupString == "aiEcho" }
        assertNotNull("Expected aiEcho in completion", aiEcho)

        myFixture.type('\t') // accept completion
        myFixture.checkResult("void test() { aiEcho(<caret>) }")
    }
}

/**
 * Closes verify-report gap: typing the opening parenthesis of a known
 * syscall offers the first parameter's default value.
 */
class XsSyscallCompletionDefaultOnOpenParenTest : BasePlatformTestCase() {

    override fun getTestDataPath(): String = "src/test/testData"

    fun testDefaultParamCompletionOnOpenParen() {
        myFixture.configureByText(
            XsFileType.INSTANCE,
            "void test() { aiPlanCreate(<caret>); }"
        )
        val elements = myFixture.complete(CompletionType.BASIC)
        assertNotNull("Open paren should propose a default value", elements)

        val firstParamDefault = XsEngineApi.lookup("aiPlanCreate")?.params?.firstOrNull()?.defaultValue
        assertNotNull("Test setup: aiPlanCreate should have a first param", firstParamDefault)

        val hasDefault = elements!!.any { it.lookupString == firstParamDefault }
        assertTrue(
            "Expected default '$firstParamDefault' for first param of aiPlanCreate, got: ${elements.map { it.lookupString }}",
            hasDefault
        )
    }
}
