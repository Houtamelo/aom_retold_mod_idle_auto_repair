package com.aomr.xs.textmate

import org.jetbrains.plugins.textmate.api.TextMateBundleProvider
import java.io.IOException
import java.io.InputStream
import java.net.URL
import java.nio.file.Files
import java.nio.file.Path
import java.nio.file.StandardCopyOption

class XsTextMateBundleProvider : TextMateBundleProvider {
    override fun getBundles(): List<TextMateBundleProvider.PluginBundle> {
        return try {
            val bundleDir = Files.createTempDirectory("xs-textmate-bundle")
            copyResource("syntaxes/xs.tmLanguage.json", bundleDir.resolve("syntaxes/xs.tmLanguage.json"))
            listOf(TextMateBundleProvider.PluginBundle("XS", bundleDir))
        } catch (e: IOException) {
            throw RuntimeException("Failed to prepare XS TextMate bundle", e)
        }
    }

    private fun copyResource(resourcePath: String, target: Path) {
        val resource: URL = XsTextMateBundleProvider::class.java.classLoader.getResource(resourcePath)
            ?: throw IOException("Resource not found: $resourcePath")
        Files.createDirectories(target.parent)
        resource.openStream().use { input: InputStream ->
            Files.copy(input, target, StandardCopyOption.REPLACE_EXISTING)
        }
    }
}
