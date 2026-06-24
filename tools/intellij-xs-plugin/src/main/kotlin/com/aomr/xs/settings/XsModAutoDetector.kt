package com.aomr.xs.settings

import com.intellij.openapi.vfs.VirtualFile

/**
 * Recursively scans a project root for directories named exactly `game`.
 *
 * Each `game/` directory marks the boundary of an Age of Mythology: Retold
 * mod package; its parent directory is returned as a mod root. Recursion
 * does not enter `game/` itself, so nested `game/` directories inside an
 * already-discovered mod are not reported as separate mods.
 */
object XsModAutoDetector {

    fun scan(projectRoot: VirtualFile): List<String> {
        val mods = mutableSetOf<String>()
        scanRecursive(projectRoot, mods)
        return mods.sorted()
    }

    private fun scanRecursive(dir: VirtualFile, mods: MutableSet<String>) {
        if (!dir.isDirectory) return
        for (child in dir.children) {
            if (!child.isDirectory) continue
            if (child.name == "game") {
                val parent = child.parent
                if (parent != null) {
                    mods.add(parent.path)
                }
            } else {
                scanRecursive(child, mods)
            }
        }
    }
}
