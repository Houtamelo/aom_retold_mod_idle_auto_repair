package com.aomr.xs.constants

import com.google.gson.annotations.SerializedName
import com.google.gson.Gson
import java.io.InputStreamReader

/**
 * Static snapshot of the AoM:R AI plan constants vendored in `aiplans.json`.
 *
 * Loaded lazily, exactly like [XsEngineApi], to keep completion providers allocation-free
 * on the EDT.
 */
object XsAiPlans {

    data class AiPlan(
        val name: String,
        val value: Int,
        @SerializedName("variable_type")
        val variableType: String,
        @SerializedName("variable_value")
        val variableValue: String
    )

    private data class AiPlanRoot(
        val constants: List<AiPlan>
    )

    private val plans: List<AiPlan> = loadPlans()
    private val byName: Map<String, AiPlan> = plans.associateBy { it.name }
    private val sortedByName: List<AiPlan> = plans.sortedBy { it.name }

    /** Returns the AI plan constant with the exact given [name], or `null` if unknown. */
    fun lookup(name: String): AiPlan? = byName[name]

    /** Returns every AI plan constant whose name starts with [prefix] (case-sensitive). */
    fun searchByPrefix(prefix: String): List<AiPlan> =
        if (prefix.isEmpty()) sortedByName
        else sortedByName.filter { it.name.startsWith(prefix) }

    /** Returns all bundled AI plan constants in alphabetical order. */
    fun all(): List<AiPlan> = sortedByName

    /** Total number of bundled AI plan constants. */
    val size: Int get() = plans.size

    private fun loadPlans(): List<AiPlan> {
        val stream = XsAiPlans::class.java.classLoader.getResourceAsStream("aiplans.json")
            ?: error("Bundled aiplans.json is missing; cannot load XS AI plan constants.")

        return stream.use { s ->
            Gson().fromJson(InputStreamReader(s, Charsets.UTF_8), AiPlanRoot::class.java).constants
        }
    }
}
