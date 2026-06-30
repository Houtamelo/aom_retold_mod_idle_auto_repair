package com.aomr.xs.textmate

import com.aomr.xs.XsLanguage
import com.google.gson.JsonElement
import com.google.gson.JsonParser
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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
 * to register a bundle whose directory structure does not match a
 * known format. The user-visible error is:
 *   ``` `XS` bundle has an unknown format ```
 *   or
 *   ``` TextMate bundle XS cannot be loaded ```
 *
 * The actual detection algorithm in
 * `org.jetbrains.plugins.textmate.bundles.BundleType$Companion.detectBundleType`
 * (verified by disassembling the bundled IntelliJ 2024.2 TextMate plugin
 * via `javap -c`) is:
 *
 *   1. If `<dir>`'s extension is `.tmBundle` -> TEXTMATE
 *   2. Else if `<dir>/package.json` is a regular file -> **VSCODE**
 *   3. Else if any direct child of `<dir>` ends in `.tmLanguage`
 *      or `.tmPreferences` -> SUBLIME
 *   4. Else if `<dir>/info.plist` is a regular file -> TEXTMATE
 *   5. Otherwise -> UNDEFINED ("unknown format")
 *
 * NOTE: the algorithm does NOT inspect subdirectories. It only looks at
 * the immediate children of `<dir>` for `.tmLanguage`/`.tmPreferences`,
 * and at `<dir>/package.json` and `<dir>/info.plist` for VSCode and
 * TextMate-bundle detection. A grammar sitting in `<dir>/syntaxes/...`
 * with no marker file at the root returns UNDEFINED.
 *
 * After the bundle type is detected, the VSCode reader walks the
 * `contributes.grammars[]` array declared in `package.json`, reads each
 * `path` field (relative to the package.json), parses the grammar JSON,
 * and validates it has `name`, `scopeName`, `fileTypes`, and `patterns`.
 *
 * This test class replicates BOTH the detection algorithm AND the
 * package.json structure check, so a regression in either layer is
 * caught in CI without booting a real IDE.
 */
class XsTextMateBundleFormatTest {

    @Test
    fun bundleDirectoryIsRecognizedAsVscode() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val type = detectBundleType(bundle.path)
        assertNotEqualsBundleType(
            "XS bundle must not be UNDEFINED — that triggers " +
                "`bundle has an unknown format` in the IntelliJ TextMate plugin",
            BundleType.UNDEFINED, type
        )
        assertEquals(
            "XS bundle must be detected as VSCODE so the IntelliJ TextMate " +
                "plugin walks the `contributes.grammars[]` array in package.json. " +
                "Detected as $type.",
            BundleType.VSCODE, type
        )
    }

    @Test
    fun bundleHasPackageJsonAtRoot() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val packageJson = bundle.path.resolve("package.json")
        assertTrue(
            "XS bundle must have a package.json at its root — this is the only " +
                "file the IntelliJ TextMate plugin inspects to decide whether the " +
                "bundle is a VSCode-format extension (the only supported format " +
                "for a plugin-deployed grammar in a subdirectory)",
            Files.isRegularFile(packageJson)
        )
    }

    @Test
    fun bundleHasSyntaxesSubdirectory() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val syntaxesDir = bundle.path.resolve("syntaxes")
        assertTrue(
            "XS bundle must have a syntaxes/ subdirectory holding the grammar file",
            Files.isDirectory(syntaxesDir)
        )
    }

    @Test
    fun bundleGrammarFileExists() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val grammar = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        assertTrue("XS grammar file must exist in the bundle", Files.exists(grammar))
    }

    @Test
    fun packageJsonDeclaresGrammarEntry() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val packageJson = bundle.path.resolve("package.json")
        val obj = parseJson(packageJson)

        val contributes = obj.getAsJsonObject("contributes")
        assertNotNull("package.json must have a `contributes` object", contributes)

        val grammars = contributes.getAsJsonArray("grammars")
        assertNotNull(
            "package.json must declare `contributes.grammars` — the IntelliJ " +
                "TextMate plugin reads this array to find the grammar files",
            grammars
        )
        assertTrue("`contributes.grammars` must declare at least one grammar", grammars.size() > 0)

        // Verify the first grammar entry points at our grammar file and uses
        // matching language / scopeName values.
        val first = grammars.get(0).asJsonObject
        assertFieldIsNonEmptyString(first, "language",
            "Each `contributes.grammars[]` entry needs a `language` field")
        assertEquals(
            "TextMate grammar language must match XsLanguage.ID",
            XsLanguage.INSTANCE.id,
            first.get("language").asString
        )
        assertFieldIsNonEmptyString(first, "scopeName",
            "Each `contributes.grammars[]` entry needs a `scopeName` field")
        assertFieldIsNonEmptyString(first, "path",
            "Each `contributes.grammars[]` entry needs a `path` field " +
                "(relative to package.json)")

        val path = first.get("path").asString
        assertTrue(
            "Grammar `path` must point at the grammar file. " +
                "Expected './syntaxes/xs.tmLanguage.json' (or similar), got: $path",
            path.endsWith("xs.tmLanguage.json")
        )
    }

    @Test
    fun packageJsonGrammarPathResolvesToExistingFile() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val packageJson = bundle.path.resolve("package.json")
        val grammars = parseJson(packageJson).getAsJsonObject("contributes")
            .getAsJsonArray("grammars")
        val path = grammars.get(0).asJsonObject.get("path").asString

        // Resolve relative to package.json's parent.
        val resolved = packageJson.parent.resolve(path).normalize()
        assertTrue(
            "Grammar path '$path' from package.json must resolve to an existing " +
                "file inside the bundle. Resolved: $resolved",
            Files.isRegularFile(resolved)
        )
    }

    @Test
    fun grammarJsonHasAllRequiredTextMateFields() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val grammar = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        val obj = parseJson(grammar)

        assertFieldIsNonEmptyString(obj, "name",
            "TextMate grammar must have a non-empty 'name'")
        assertFieldIsNonEmptyString(obj, "scopeName",
            "TextMate grammar must have a non-empty 'scopeName' " +
                "(e.g. 'source.<ext>')")

        val fileTypes = obj.getAsJsonArray("fileTypes")
        assertNotNull(
            "TextMate grammar must have a 'fileTypes' array — missing 'fileTypes' " +
                "causes the bundle to be rejected",
            fileTypes
        )
        assertTrue("'fileTypes' must be a non-empty array of file extensions",
            fileTypes.size() > 0)
        fileTypes.forEach { el ->
            assertTrue(
                "'fileTypes' entries must be non-empty strings; got: $el",
                el.isJsonPrimitive && el.asString.isNotEmpty()
            )
        }

        val patterns = obj.getAsJsonArray("patterns")
        assertNotNull("TextMate grammar must have a 'patterns' array", patterns)
        assertTrue("'patterns' must be a non-empty array", patterns.size() > 0)
    }

    @Test
    fun grammarFileTypesIncludeXs() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val grammar = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        val obj = parseJson(grammar)
        val fileTypes = obj.getAsJsonArray("fileTypes")
        val hasXs = (0 until fileTypes.size()).any { fileTypes.get(it).asString == "xs" }
        assertTrue(
            "XS grammar should declare the 'xs' file extension in fileTypes; got $fileTypes",
            hasXs
        )
    }

    @Test
    fun grammarScopeNameMatchesXSLanguage() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val grammar = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        val obj = parseJson(grammar)
        val scopeName = obj.get("scopeName").asString
        assertEquals(
            "XS grammar should use scope 'source.xs' so the IDE can route .xs files",
            "source.xs", scopeName
        )
    }

    @Test
    fun grammarHasNoStorageTypesRuleForPrimitiveTypes() {
        val bundle = XsTextMateBundleProvider().getBundles().first()
        val grammar = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        val obj = parseJson(grammar)

        // The old `storage_types` repository block matched int/float/bool/void/string/vector/class
        // and tagged them with storage.type.built-in.primitive.c, which pre-empted the
        // semantic-token layer. Ensure no such pattern remains anywhere in the grammar.
        val primitivePattern = Regex("""(?i)(?<!\w)(int|float|bool|void|string|vector|class)(?!\w)""")
        val violations = collectViolations(obj, primitivePattern)
        assertTrue(
            "No pattern should match primitive type keywords and tag them with " +
                "storage.type.built-in.primitive.c; violations: $violations",
            violations.isEmpty()
        )
    }

    private fun collectViolations(element: JsonElement, primitivePattern: Regex): List<String> {
        val results = mutableListOf<String>()
        if (element.isJsonObject) {
            val obj = element.asJsonObject
            val name = obj.get("name")?.takeIf { it.isJsonPrimitive }?.asString
            val match = obj.get("match")?.takeIf { it.isJsonPrimitive }?.asString
            if (name == "storage.type.built-in.primitive.c" && match != null && primitivePattern.containsMatchIn(match)) {
                results.add(match)
            }
            for (key in obj.keySet()) {
                results.addAll(collectViolations(obj.get(key), primitivePattern))
            }
        } else if (element.isJsonArray) {
            for (item in element.asJsonArray) {
                results.addAll(collectViolations(item, primitivePattern))
            }
        }
        return results
    }

    // -- helpers --

    private enum class BundleType { TEXTMATE, SUBLIME, VSCODE, UNDEFINED }

    /**
     * Faithful port of `BundleType.Companion.detectBundleType(Path)` from
     * `org.jetbrains.plugins.textmate.bundles.BundleType` in the bundled
     * IntelliJ 2024.2 TextMate plugin. Verified via `javap -c` — see
     * KDoc above.
     */
    private fun detectBundleType(directory: Path): BundleType {
        if (!Files.isDirectory(directory)) return BundleType.UNDEFINED

        // 1. Extension == "tmBundle" -> TEXTMATE
        val ext = com.intellij.openapi.util.io.FileUtilRt.getExtension(
            directory.fileName.toString()
        )
        if (ext == "tmBundle") return BundleType.TEXTMATE

        // 2. package.json at root -> VSCODE
        if (Files.isRegularFile(directory.resolve("package.json"))) {
            return BundleType.VSCODE
        }

        // 3. Any direct child ending in .tmLanguage or .tmPreferences -> SUBLIME
        val hasTmFiles = try {
            Files.list(directory).use { stream ->
                stream.toList().any { child ->
                    val name = child.fileName.toString()
                    name.endsWith(".tmLanguage") || name.endsWith(".tmPreferences")
                }
            }
        } catch (e: Exception) {
            false
        }
        if (hasTmFiles) return BundleType.SUBLIME

        // 4. info.plist at root -> TEXTMATE
        if (Files.isRegularFile(directory.resolve("info.plist"))) {
            return BundleType.TEXTMATE
        }

        // 5. Otherwise UNDEFINED
        return BundleType.UNDEFINED
    }

    private fun parseJson(path: Path): com.google.gson.JsonObject {
        val text = Files.readString(path)
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
        assertFalse("$message — field '$key' must be a non-empty string", value.isEmpty())
    }

    private fun assertNotEqualsBundleType(
        message: String, notExpected: BundleType, actual: BundleType
    ) {
        if (actual == notExpected) {
            throw AssertionError("$message — actual was $actual")
        }
    }
}