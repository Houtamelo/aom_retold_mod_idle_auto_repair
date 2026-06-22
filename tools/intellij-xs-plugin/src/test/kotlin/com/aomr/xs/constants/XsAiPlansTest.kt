package com.aomr.xs.constants

import org.junit.Assert.*
import org.junit.Test

class XsAiPlansTest {

    @Test
    fun sizeMatchesVendoredPlanCount() {
        assertEquals(193, XsAiPlans.size)
    }

    @Test
    fun searchByPrefixReturnsMatchingPlans() {
        // The vendored aiplans.json groups constants by plan kind (attack, build, etc.)
        // so prefixes like "cAttackPlan" are reliable; literal "cPlan" entries do not exist.
        val matches = XsAiPlans.searchByPrefix("cAttackPlan")
        assertTrue("cAttackPlan should match at least one AI plan constant", matches.isNotEmpty())
        assertTrue("All results should start with cAttackPlan", matches.all { it.name.startsWith("cAttackPlan") })
    }

    @Test
    fun lookupReturnsExpectedPlan() {
        val known = XsAiPlans.all().first()
        val lookedUp = XsAiPlans.lookup(known.name)
        assertNotNull("Lookup should return the known plan by name", lookedUp)
        assertEquals(known.name, lookedUp!!.name)
        assertEquals(known.value, lookedUp.value)
        assertEquals(known.variableType, lookedUp.variableType)
        assertEquals(known.variableValue, lookedUp.variableValue)
    }
}
