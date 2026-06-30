package com.aomr.xs.textmate

import com.google.gson.JsonObject
import com.google.gson.JsonParser
import org.junit.Assert.*
import org.junit.Test
import java.io.InputStreamReader
import java.nio.file.Files

class XsTextMateHighlightingTest {

    @Test
    fun bundleProviderExposesGrammar() {
        val bundles = XsTextMateBundleProvider().getBundles()
        assertFalse("XS TextMate bundle should be provided", bundles.isEmpty())

        val bundle = bundles.first()
        assertEquals("XS", bundle.name)

        val grammarPath = bundle.path.resolve("syntaxes/xs.tmLanguage.json")
        assertTrue("Grammar file should exist in bundle path", Files.exists(grammarPath))
    }

    @Test
    fun grammarScopeMapping() {
        val stream = XsTextMateBundleProvider::class.java.classLoader.getResourceAsStream("syntaxes/xs.tmLanguage.json")
        assertNotNull("Grammar resource should be bundled", stream)

        val grammar = stream!!.use { s ->
            JsonParser.parseReader(InputStreamReader(s, Charsets.UTF_8)).asJsonObject
        }

        assertEquals("source.xs", grammar.get("scopeName").asString)

        val patterns = grammar.getAsJsonArray("patterns")
        assertNotNull(patterns)
        assertTrue("Grammar should define token patterns", patterns.size() > 0)

        val repository = grammar.getAsJsonObject("repository")
        assertNotNull("Grammar should define a repository", repository)

        assertHasScope(repository, "comments", "comment.block.c")
        assertHasScope(repository, "strings", "string.quoted.double.c")
        assertHasScope(repository, "numbers", "constant.numeric.decimal.c")
    }

    private fun assertHasScope(repository: JsonObject, key: String, expectedScope: String) {
        val entry = repository.getAsJsonObject(key) ?: throw AssertionError("Missing repository entry: $key")
        assertTrue(
            "Expected repository '$key' to contain scope '$expectedScope'",
            entry.toString().contains(expectedScope)
        )
    }
}
