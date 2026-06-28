package com.aomr.xs.settings

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.nio.file.Paths

class XsModAutoDetectorTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    @Test
    fun testDetectsModRootsAndStopsAtGameBoundaries() {
        val root = tempFolder.root.toPath()

        // Two valid mod roots.
        createFile(root, "my_mod/game/ai/foo.xs")
        createFile(root, "my_mod/README.md")
        createFile(root, "nested/another_mod/game/ai/bar.xs")
        createFile(root, "nested/another_mod/subdir/some_file.xs")

        // A directory without a game/ folder should not be detected.
        createFile(root, "nested/standalone/some_file.xs")

        val detected = XsModAutoDetector.scan(root)

        assertEquals(2, detected.size)
        assertTrue("expected my_mod root", detected.any { it.endsWith("my_mod") })
        assertTrue("expected another_mod root", detected.any { it.endsWith("another_mod") })
        assertFalse("standalone should not be detected", detected.any { it.endsWith("standalone") })
    }

    @Test
    fun testNestedGameDirectoriesStopRecursion() {
        val root = tempFolder.root.toPath()

        // mod/game/sub/game/ should only report mod/, not sub/.
        createFile(root, "mod/game/sub/game/inner.xs")

        val detected = XsModAutoDetector.scan(root)

        assertEquals(1, detected.size)
        assertTrue("expected mod root", detected.any { it.endsWith("mod") })
    }

    private fun createFile(root: java.nio.file.Path, relativePath: String) {
        val path = root.resolve(Paths.get(relativePath))
        path.parent?.toFile()?.mkdirs()
        path.toFile().writeText("")
    }
}
