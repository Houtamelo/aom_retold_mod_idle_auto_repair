package com.aomr.xs.settings

import com.intellij.openapi.vfs.VirtualFile
import java.nio.file.Files
import java.nio.file.Path
import kotlin.streams.asSequence

/**
 * Recursively scans a project root for directories named exactly `game`.
 *
 * Each `game/` directory marks the boundary of an Age of Mythology: Retold
 * mod package; its parent directory is returned as a mod root. Recursion
 * does not enter `game/` itself, so nested `game/` directories inside an
 * already-discovered mod are not reported as separate mods.
 */
object XsModAutoDetector {

    fun scan(projectRoot: VirtualFile): List<String> =
        scan(projectRoot.toNioPath())

    fun scan(projectRoot: Path): List<String> {
        val mods = mutableSetOf<String>()
        scanRecursive(projectRoot, mods)
        return mods.sorted()
    }

    private fun scanRecursive(dir: Path, mods: MutableSet<String>) {
        if (!Files.isDirectory(dir)) return
        val children = Files.list(dir).use { stream -> stream.asSequence().toList() }
        for (child in children) {
            if (!Files.isDirectory(child)) continue
            if (child.fileName?.toString() == "game") {
                val parent = child.parent
                if (parent != null) {
                    mods.add(parent.toAbsolutePath().normalize().toString())
                }
            } else {
                scanRecursive(child, mods)
            }
        }
    }
}
