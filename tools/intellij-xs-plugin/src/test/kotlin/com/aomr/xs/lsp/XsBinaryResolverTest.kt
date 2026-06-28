package com.aomr.xs.lsp

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.ByteArrayInputStream
import java.io.File

class XsBinaryResolverTest {

    @get:Rule
    val tempFolder = TemporaryFolder()

    @Before
    fun resetBundledCache() {
        // The extracted binary is cached at the object level. Reset it between
        // tests so one test's bundled extraction does not leak into the next.
        val instance = XsBinaryResolver::class.objectInstance ?: return
        XsBinaryResolver::class.java.getDeclaredField("cachedBundledPath").apply {
            isAccessible = true
            set(instance, null)
        }
    }

    @Test
    fun systemPropertyIsHonored() {
        val custom = tempFolder.newFile("sysprop-xs-lsp").apply { setExecutable(true) }.absolutePath
        val resolved = XsBinaryResolver.resolve(systemProperty = custom)
        assertEquals(custom, resolved)
    }

    @Test
    fun envVarIsHonoredWhenSyspropUnset() {
        val custom = tempFolder.newFile("env-xs-lsp").apply { setExecutable(true) }.absolutePath
        val resolved = XsBinaryResolver.resolve(systemProperty = null, envVar = custom)
        assertEquals(custom, resolved)
    }

    @Test
    fun bundledBinaryIsExtractedAndUsed() {
        val resolved = XsBinaryResolver.resolve(
            systemProperty = null,
            envVar = null,
            resourceProvider = { _ -> ByteArrayInputStream("fake binary".toByteArray()) }
        )
        assertTrue("bundled path should be extracted", resolved.startsWith(System.getProperty("java.io.tmpdir")))
        assertTrue("extracted file should exist", File(resolved).exists())
        assertTrue("extracted file should be executable", File(resolved).canExecute())
    }

    @Test
    fun extractedBinaryIsCached() {
        val first = XsBinaryResolver.resolve(
            systemProperty = null,
            envVar = null,
            resourceProvider = { _ -> ByteArrayInputStream("fake binary".toByteArray()) }
        )
        val second = XsBinaryResolver.resolve(
            systemProperty = null,
            envVar = null,
            resourceProvider = { _ -> throw AssertionError("should not be called again") }
        )
        assertEquals(first, second)
    }

    @Test
    fun projectLocalBinaryIsUsedWhenHigherStrategiesFail() {
        val projectRoot = tempFolder.root
        val binary = projectRoot
            .resolve("tools/xs-language-server/target/release/xs-language-server")
            .apply { parentFile.mkdirs() }
        binary.writeText("fake xs-language-server")
        assertTrue("test setup failed: could not mark project binary executable", binary.setExecutable(true))

        val resolved = XsBinaryResolver.resolve(
            systemProperty = null,
            envVar = null,
            projectBasePath = projectRoot.absolutePath,
            resourceProvider = { _ -> null }
        )
        assertEquals(binary.absolutePath, resolved)
    }

    @Test
    fun fallsBackToPathLookup() {
        val resolved = XsBinaryResolver.resolve(
            systemProperty = null,
            envVar = null,
            projectBasePath = tempFolder.root.absolutePath,
            resourceProvider = { _ -> null }
        )
        assertEquals("xs-language-server", resolved)
    }

    @Test
    fun syspropBeatsEnvVar() {
        val sysprop = tempFolder.newFile("sysprop-xs").apply { setExecutable(true) }.absolutePath
        val env = tempFolder.newFile("env-xs").apply { setExecutable(true) }.absolutePath
        val resolved = XsBinaryResolver.resolve(systemProperty = sysprop, envVar = env)
        assertEquals(sysprop, resolved)
        assertFalse(resolved.contains("env-xs"))
    }
}
