package com.aomr.xs.navigation

import com.aomr.xs.XsFileType
import com.aomr.xs.constants.XsEngineApi
import com.intellij.testFramework.LightVirtualFile
import com.intellij.testFramework.fixtures.BasePlatformTestCase

class XsEngineNavigationTest : BasePlatformTestCase() {

    override fun getTestDataPath(): String = "src/test/testData"

    /**
     * Covers the five P1.6 engine-stub navigation scenarios.
     */
    fun testEngineNavigationScenarios() {
        // Test 1: Ctrl+B on aiEcho resolves to a generated LightVirtualFile stub.
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { aiE<caret>cho(\"hi\"); }")
        val reference = myFixture.file.findReferenceAt(myFixture.caretOffset)
            ?: error("Expected a reference on aiEcho")
        val target = reference.resolve()
            ?: error("Expected aiEcho to resolve to a generated stub")
        assertTrue(
            "Resolved file should be a LightVirtualFile",
            target.containingFile.virtualFile is LightVirtualFile
        )

        // Use the resolved PSI file text for the content assertions.
        val stubText = target.containingFile.text

        // Test 2: the stub contains the syscall signature.
        assertTrue(
            "Stub should contain the aiEcho signature",
            stubText.contains("void aiEcho(string text)")
        )

        // Test 3: the stub contains the help text from XsEngineApi.
        val aiEchoHelp = XsEngineApi.lookup("aiEcho")?.help ?: error("missing aiEcho help")
        assertTrue(
            "Stub should contain the aiEcho help text",
            stubText.contains(aiEchoHelp)
        )

        // Test 4: unknown identifier does not produce a reference.
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { fict<caret>ionalCall(); }")
        val unknownRef = myFixture.file.findReferenceAt(myFixture.caretOffset)
        assertNull(
            "Unknown identifiers should not produce references",
            unknownRef
        )

        // Test 5: generating the same stub twice returns the cached instance.
        val generator = XsEngineStubGenerator()
        val aiEchoSyscall = XsEngineApi.lookup("aiEcho") ?: error("missing aiEcho")
        val first = generator.generateStub(aiEchoSyscall)
        val second = generator.generateStub(aiEchoSyscall)
        assertSame(
            "Cache should return the same LightVirtualFile instance",
            first,
            second
        )
    }
}

/**
 * Closes verify-report gap: the generated engine stub's virtual file name
 * clearly identifies it as a generated documentation view.
 */
class XsEngineStubNameTest : BasePlatformTestCase() {

    override fun getTestDataPath(): String = "src/test/testData"

    fun testEngineStubHasDescriptiveName() {
        myFixture.configureByText(XsFileType.INSTANCE, "void test() { aiE<caret>cho(\"hi\"); }")
        val reference = myFixture.file.findReferenceAt(myFixture.caretOffset)
            ?: error("Expected a reference on aiEcho")
        val target = reference.resolve()
            ?: error("Expected aiEcho to resolve to a generated stub")

        val file = target.containingFile
        assertTrue(
            "Stub file should be a LightVirtualFile",
            file.virtualFile is LightVirtualFile
        )

        val name = file.virtualFile.name
        assertTrue(
            "Stub filename should mark it as generated (got '$name')",
            name.contains("[XS Engine Stub]")
        )
    }
}
