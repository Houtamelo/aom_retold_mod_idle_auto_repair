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
            // The IntelliJ TextMate plugin's BundleType.detectBundleType algorithm only
            // inspects the ROOT of the bundle directory. It returns UNDEFINED ("unknown
            // format") if the root has no package.json / .tmLanguage / .tmPreferences /
            // info.plist — it never walks into syntaxes/. So we must register the
            // bundle as a VSCode-format extension: package.json at root, with the
            // grammar file referenced at syntaxes/xs.tmLanguage.json. This mirrors
            // what real VSCode extensions look like and triggers the VSCODE branch of
            // the detection algorithm.
            copyResource("package.json", bundleDir.resolve("package.json"))
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
