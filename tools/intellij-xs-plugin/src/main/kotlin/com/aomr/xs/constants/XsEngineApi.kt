package com.aomr.xs.constants

import com.google.gson.annotations.SerializedName
import com.google.gson.Gson
import java.io.InputStreamReader

/**
 * Static snapshot of the AoM:R engine syscall API vendored in `syscalls.json`.
 *
 * The singleton is initialized lazily on first access: it parses the bundled JSON once
 * and builds immutable indexes for O(1) exact lookup and efficient prefix iteration.
 */
object XsEngineApi {

    data class Syscall(
        val name: String,
        val help: String,
        @SerializedName("return_type")
        val returnType: String,
        val params: List<SyscallParam>,
        val filename: String
    )

    data class SyscallParam(
        val type: String,
        val name: String,
        @SerializedName("default")
        val defaultValue: String
    )

    private data class SyscallRoot(
        val syscalls: List<Syscall>
    )

    private val syscalls: List<Syscall> = loadSyscalls()
    private val byName: Map<String, Syscall> = syscalls.associateBy { it.name }
    private val sortedByName: List<Syscall> = syscalls.sortedBy { it.name }

    /**
     * Returns the syscall with the exact given [name], or `null` if it is not part of the
     * bundled engine API.
     */
    fun lookup(name: String): Syscall? = byName[name]

    /**
     * Returns every syscall whose name starts with [prefix]. Matching is case-sensitive,
     * which matches XS identifier semantics.
     */
    fun searchByPrefix(prefix: String): List<Syscall> =
        if (prefix.isEmpty()) sortedByName
        else sortedByName.filter { it.name.startsWith(prefix) }

    /** Returns all bundled syscalls in alphabetical order by name. */
    fun all(): List<Syscall> = sortedByName

    /** Total number of bundled syscalls. */
    val size: Int get() = syscalls.size

    private fun loadSyscalls(): List<Syscall> {
        val stream = XsEngineApi::class.java.classLoader.getResourceAsStream("syscalls.json")
            ?: error("Bundled syscalls.json is missing; cannot load XS engine API.")

        return stream.use { s ->
            Gson().fromJson(InputStreamReader(s, Charsets.UTF_8), SyscallRoot::class.java).syscalls
        }
    }
}
