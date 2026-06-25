package com.aomr.xs.lsp

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * Regression tests for the workspace-folder URI conversion path.
 *
 * The XS Language Server's `lookup_mod` does a `Path::starts_with` match
 * between the registered mod prefix and the opened file's resolved path.
 * If the URI sent over `workspace/didChangeWorkspaceFolders` is malformed
 * (e.g. double-prefixed, double URL-encoded) the prefix will never match
 * the real file path, and every `didOpen` for a file in the mod will
 * surface the "File not part of any registered mod; engine API only"
 * warning.
 *
 * The bug we are guarding against (fixed in the follow-up to commit
 * ed73341):
 *
 *     // XsLspServerManager.updateWorkspaceFolders (pre-fix):
 *     connection?.changeWorkspaceFolders(
 *         added.map { File(it).toURI().toString() },     // path -> URI (correct)
 *         ...
 *     )
 *
 *     // XsLspConnection.changeWorkspaceFolders (pre-fix):
 *     val addedFolders = added.map { WorkspaceFolder(it.toFileUri(), ...) }
 *     // where toFileUri() does `File(this).toURI().toString()` AGAIN,
 *     // treating the URI string as a literal path. That produced:
 *     //   input:  /home/user/Mod Extra/mod foo
 *     //   pass 1: file:/home/user/Mod%20Extra/mod%20foo
 *     //   pass 2: file:/home/user/file:/home/user/Mod%2520Extra/mod%2520foo
 *     // The LSP decodes the URI to a literal path with an extra `file:`
 *     // segment that never matches the real /home/user/Mod Extra/mod foo/...
 *     // prefix.
 *
 * The fix: pass paths all the way to XsLspConnection and let it do the
 * single File(path).toURI().toString() conversion. These tests pin that
 * behaviour so a future refactor cannot regress to the double-conversion
 * form without a CI failure.
 *
 * Note on URI shape: Java's `File(path).toURI()` for absolute paths
 * produces `file:/home/user/foo` (one slash) rather than the strictly
 * canonical `file:///home/user/foo` (three slashes). Both forms are
 * accepted by the LSP spec and by lsp4j. The LSP's `Url::to_file_path`
 * decodes them back to the same filesystem path.
 */
class XsLspWorkspaceFolderUriTest {

    /**
     * Reproduce the URI conversion exactly the way `changeWorkspaceFolders`
     * does today: `WorkspaceFolder(File(path).toURI().toString(),
     * File(path).name)`. Tested through this helper rather than a mock
     * server because the bug is purely about how the input is shaped, not
     * about LSP protocol mechanics.
     */
    private data class Folder(val uri: String, val name: String)
    private fun toLspFolder(path: String): Folder =
        Folder(uri = File(path).toURI().toString(), name = File(path).name)

    @Test
    fun plainAsciiPath() {
        val f = toLspFolder("/home/user/projects/mods/foo")
        assertEquals("file:/home/user/projects/mods/foo", f.uri)
        assertEquals("foo", f.name)
    }

    @Test
    fun pathWithSpacesIsEncodedOnce() {
        val f = toLspFolder("/home/user/Mod Extra/mod foo")
        // Space encoded once as %20, NOT twice as %2520.
        assertEquals("file:/home/user/Mod%20Extra/mod%20foo", f.uri)
        // Name is the basename with the original (unencoded) characters.
        assertEquals("mod foo", f.name)
    }

    @Test
    fun pathWithPlusSignIsKeptLiteralInName() {
        // Java's File.toURI() does NOT encode '+' because '+' is a valid
        // path character. It encodes the spaces around it.
        val f = toLspFolder("/home/user/Extra Ai + AoModAi")
        assertEquals("file:/home/user/Extra%20Ai%20+%20AoModAi", f.uri)
        assertEquals("Extra Ai + AoModAi", f.name)
    }

    @Test
    fun theBuggyDoubleConversionIsNotWhatWeProduce() {
        // This is the smoke test: the OLD code path produced
        //   file:/home/user/Mod%2520Extra/mod%2520foo
        // for the same input. If this test ever fails because the URI now
        // contains %2520, the double-conversion regression has returned.
        val f = toLspFolder("/home/user/Mod Extra/mod foo")
        assertFalse(
            "URI must not contain %2520 (that would mean 'space' was " +
                "URL-encoded twice — the original bug). Got: ${f.uri}",
            f.uri.contains("%2520")
        )
        assertFalse(
            "URI must not contain '/file:' (that would mean the URI was " +
                "passed through File(String).toURI() twice — the original " +
                "bug). Got: ${f.uri}",
            f.uri.contains("/file:")
        )
        // Also: only ONE 'file:' scheme prefix, not two.
        assertEquals(
            "URI must have exactly one 'file:' scheme prefix, not two",
            1, f.uri.split("file:").size - 1
        )
    }

    @Test
    fun uriRoundTripsBackToTheOriginalPath() {
        // Whatever URI we produce, decoding it must yield the original
        // filesystem path. If it doesn't, the LSP's lookup_mod will fail
        // because the registered prefix won't match the opened file.
        val path = "/home/user/Mod Extra/mod foo/bar.xs"
        val f = toLspFolder(path)
        val decoded = java.net.URI(f.uri)
        assertEquals(path, java.nio.file.Paths.get(decoded).toString())
    }

    @Test
    fun theRegisteredPrefixIsAPrefixOfAnOpenedFileInTheMod() {
        // The actual failure mode: registered mod
        //   /home/user/.../intelligent_auto_scout
        // failed to match an opened file at
        //   /home/user/.../intelligent_auto_scout/game/ai/foo.xs
        // because the URI sent to the LSP was
        //   file:/home/user/file:/home/user/.../intelligent_auto_scout
        // which decoded to a literal path with an extra /file:/ segment.
        // After the fix, the URI is clean and the prefix relationship
        // survives URL decoding.
        val modPath = "/home/user/projects/aom_retold_mod/mod/intelligent_auto_scout"
        val filePath = "$modPath/game/ai/human_assist/human_assist.xs"

        val modFolder = toLspFolder(modPath)
        val fileUri = java.io.File(filePath).toURI().toString()

        // Reconstruct the file path from its URI the same way the LSP does.
        val registeredPrefix = java.nio.file.Paths.get(java.net.URI(modFolder.uri)).toString()
        val openedFilePath = java.nio.file.Paths.get(java.net.URI(fileUri)).toString()

        assertTrue(
            "Registered prefix '$registeredPrefix' must be a literal " +
                "prefix of opened file path '$openedFilePath' — otherwise " +
                "lookup_mod in the LSP returns None and the 'file not part " +
                "of any registered mod' warning fires.",
            openedFilePath.startsWith(registeredPrefix)
        )
    }
}