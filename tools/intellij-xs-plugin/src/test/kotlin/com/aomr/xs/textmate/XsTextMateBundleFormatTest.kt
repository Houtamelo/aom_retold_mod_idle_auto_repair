package com.aomr.xs.textmate

import com.google.gson.JsonParser
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.file.Files
import java.nio.file.Path
import kotlin.streams.toList

/**
 * Format validation for the XS TextMate bundle.
 *
 * The IntelliJ TextMate plugin (org.jetbrains.plugins.textmate) refuses
 * to register a bundle whose directory structure or grammar JSON does
 * not match a known format. The user-visible error is:
 *   `` `XS` bundle has an unknown format ``
 *
 * The structural detection logic in
 * `org.jetbrains.plugins.textmate.bundles.BundleType$Companion.detectBundleType`
 * is:
 *
 *   1. If `<dir>/package.json` exists -> VSCODE
 *   2. Else if any child has a `.tmLanguage` or `.tmPreferences` suffix
 *      at the root -> SUBLIME
 *   3. Else if `<dir>/syntaxes/` is a directory or `<dir>/info.plist` is
 *      a file -> TEXTMATE
 *   4. Otherwise -> UNDEFINED (the "unknown format" error)
 *
 * After the bundle type is detected, the plugin walks `syntaxes/`
 * looking for grammar files (matching `*.tmLanguage` and
 * `*.tmLanguage.json`), parses each as a JSON grammar, and validates
 * that it has the required fields. A grammar that lacks `name`,
 * `scopeName`, `fileTypes`, or `patterns` causes the whole bundle
 * to fail registration with the same "unknown format" error.
 *
 * This test replicates the full detection and validation logic so we
 * catch format regressions in CI without requiring a real IDE.
 */
class XsTextMateBundleFormatTest {

    @Test
    fun bundleDirectoryIsRecognizedAsTextMate() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val type = detectBundleType(bundle.path)
        assertNotEquals(
            "XS bundle should not be UNDEFINED — would trigger " +
                "`bundle has an unknown format` in the IntelliJ TextMate plugin",
            BundleType.UNDEFINED, type
        )
        assertEquals("XS bundle has a syntaxes/ subdir, so it should be detected as TEXTMATE",
            BundleType.TEXTMATE, type)
    }

    @Test
    fun bundleHasSyntaxesSubdirectory() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val syntaxesDir = bundle.path.resolve("syntaxes")
        assertTrue("XS bundle must have a syntaxes/ subdirectory for TextMate detection",
            Files.isDirectory(syntaxesDir))
    }

    @Test
    fun bundleGrammarFileExists() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val grammar = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        assertTrue("XS grammar file must exist in the bundle", Files.exists(grammar))
    }

    @Test
    fun grammarJsonHasAllRequiredTextMateFields() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val grammar = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        val obj = parseJson(grammar)

        // Required by the TextMate grammar spec; the IntelliJ TextMate
        // plugin's parser rejects a grammar that is missing any of these.
        assertFieldIsNonEmptyString(obj, "name",
            "TextMate grammar must have a non-empty 'name'")
        assertFieldIsNonEmptyString(obj, "scopeName",
            "TextMate grammar must have a non-empty 'scopeName' " +
                "(e.g. 'source.<ext>')")

        // `fileTypes` is what tells the IntelliJ TextMate plugin which
        // file extensions to apply the grammar to. Without it, the
        // grammar is registered but not associated with any file, and
        // the bundle is reported as "unknown format".
        val fileTypes = obj.getAsJsonArray("fileTypes")
        assertNotNull("TextMate grammar must have a 'fileTypes' array — " +
            "missing 'fileTypes' causes the bundle to be rejected as 'unknown format'",
            fileTypes)
        assertTrue("'fileTypes' must be a non-empty array of file extensions",
            fileTypes.size() > 0)
        fileTypes.forEach { el ->
            assertTrue("'fileTypes' entries must be non-empty strings; got: $el",
                el.isJsonPrimitive && el.asString.isNotEmpty())
        }

        // `patterns` is the top-level token list. Without it, the
        // grammar has no rules to apply.
        val patterns = obj.getAsJsonArray("patterns")
        assertNotNull("TextMate grammar must have a 'patterns' array — " +
            "missing 'patterns' causes the bundle to be rejected", patterns)
        assertTrue("'patterns' must be a non-empty array", patterns.size() > 0)
    }

    @Test
    fun grammarFileTypesIncludeXs() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val grammar = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        val obj = parseJson(grammar)
        val fileTypes = obj.getAsJsonArray("fileTypes")
        val hasXs = (0 until fileTypes.size()).any { fileTypes.get(it).asString == "xs" }
        assertTrue("XS grammar should declare the 'xs' file extension in fileTypes; got $fileTypes",
            hasXs)
    }

    @Test
    fun grammarScopeNameMatchesXSLanguage() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val grammar = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        val obj = parseJson(grammar)
        val scopeName = obj.get("scopeName").asString
        assertEquals("XS grammar should use scope 'source.xs' so the IDE can route .xs files",
            "source.xs", scopeName)
    }

    // -- helpers --

    /**
     * Replicates `BundleType.Companion.detectBundleType(Path)` from
     * `org.jetbrains.plugins.textmate.bundles.BundleType` in the
     * bundled IntelliJ TextMate plugin.
     */
    private enum class BundleType { TEXTMATE, SUBLIME, VSCODE, UNDEFINED }

    private fun detectBundleType(directory: Path): BundleType {
        val packageJson = directory.resolve("package.json")
        if (Files.isRegularFile(packageJson)) return BundleType.VSCODE

        val hasTmFiles = try {
            Files.list(directory).use { stream ->
                stream.toList().any { child ->
                    child.fileName.toString().endsWith(".tmLanguage") ||
                        child.fileName.toString().endsWith(".tmPreferences")
                }
            }
        } catch (e: Exception) {
            false
        }
        if (hasTmFiles) return BundleType.SUBLIME

        val syntaxes = directory.resolve("syntaxes")
        val infoPlist = directory.resolve("info.plist")
        if (Files.isDirectory(syntaxes) || Files.isRegularFile(infoPlist)) {
            return BundleType.TEXTMATE
        }
        return BundleType.UNDEFINED
    }

    private fun parseJson(grammarPath: Path): com.google.gson.JsonObject {
        val text = Files.readString(grammarPath)
        return JsonParser.parseString(text).asJsonObject
    }

    private fun assertFieldIsNonEmptyString(
        obj: com.google.gson.JsonObject, key: String, message: String
    ) {
        val element = obj.get(key)
        assertNotNull("$message — field '$key' is missing", element)
        assertTrue("$message — field '$key' must be a non-null string",
            !element.isJsonNull && element.isJsonPrimitive)
        val value = element.asString
        assertFalse("$message — field '$key' must be a non-empty string",
            value.isEmpty())
    }
}
