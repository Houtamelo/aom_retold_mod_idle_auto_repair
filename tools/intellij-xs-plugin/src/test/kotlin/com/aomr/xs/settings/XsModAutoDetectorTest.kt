package com.aomr.xs.settings

import com.intellij.openapi.vfs.VirtualFile
import com.intellij.testFramework.PlatformTestUtil
import com.intellij.testFramework.fixtures.BasePlatformTestCase

class XsModAutoDetectorTest : BasePlatformTestCase() {

    fun testDetectsModRootsAndStopsAtGameBoundaries() {
        val root = createSyntheticProject()
        val detected = XsModAutoDetector.scan(root)

        assertEquals(2, detected.size)
        assertTrue("expected my_mod root", detected.any { it.endsWith("my_mod") })
        assertTrue("expected another_mod root", detected.any { it.endsWith("another_mod") })
        assertFalse("standalone should not be detected", detected.any { it.endsWith("standalone") })
    }

    private fun createSyntheticProject(): VirtualFile {
        val root = myFixture.tempDirFixture.findOrCreateDir("tmp")
        PlatformTestUtil.waitForAlarm(100)

        writeFile(root, "my_mod/game/ai/foo.xs", "void foo() {}")
        writeFile(root, "my_mod/README.md", "# Mod")
        writeFile(root, "nested/another_mod/game/ai/bar.xs", "void bar() {}")
        writeFile(root, "nested/another_mod/subdir/some_file.xs", "void baz() {}")
        writeFile(root, "nested/standalone/some_file.xs", "void qux() {}")

        return root
    }

    private fun writeFile(root: VirtualFile, relativePath: String, content: String) {
        val parts = relativePath.split("/")
        var dir = root
        for (part in parts.dropLast(1)) {
            dir = dir.findChild(part) ?: dir.createChildDirectory(this, part)
        }
        val file = dir.findChild(parts.last()) ?: dir.createChildData(this, parts.last())
        file.setBinaryContent(content.toByteArray())
    }
}
