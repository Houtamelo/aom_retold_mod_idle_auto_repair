package com.aomr.xs.highlight

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

class XsColorSettingsPageTest {

    @Test
    fun pluginXmlRegistersXsColorSettingsPage() {
        val pluginXml = javaClass.classLoader.getResourceAsStream("META-INF/plugin.xml")
            ?: throw AssertionError("META-INF/plugin.xml not found on test classpath")
        val text = pluginXml.use { it.reader().readText() }
        assertTrue(
            "plugin.xml must register XsColorSettingsPage",
            text.contains("""<colorSettingsPage implementation="com.aomr.xs.highlight.XsColorSettingsPage""" )
        )
    }

    @Test
    fun displayNameIsLowercaseXs() {
        val page = XsColorSettingsPage()
        assertEquals("xs", page.displayName)
    }

    @Test
    fun descriptorsCoverRequiredCategories() {
        val page = XsColorSettingsPage()
        val descriptors = page.attributeDescriptors
        assertTrue("Descriptors must not be empty", descriptors.isNotEmpty())
        val keys = descriptors.map { it.key }.toSet()
        assertTrue("Keyword descriptor must be present", keys.contains(XsTextAttributesKeys.XS_KEYWORD))
        assertTrue("String descriptor must be present", keys.contains(XsTextAttributesKeys.XS_STRING))
        assertTrue("Comment descriptor must be present", keys.contains(XsTextAttributesKeys.XS_COMMENT))
        assertTrue("Number descriptor must be present", keys.contains(XsTextAttributesKeys.XS_NUMBER))
        assertTrue("Identifier descriptor must be present", keys.contains(XsTextAttributesKeys.XS_IDENTIFIER))
        assertTrue("Default descriptor must be present", keys.contains(XsTextAttributesKeys.XS_DEFAULT))
    }

    @Test
    fun demoTextIsNonEmptyXsSnippet() {
        val page = XsColorSettingsPage()
        val demo = page.demoText
        assertNotNull("Demo text must not be null", demo)
        assertFalse("Demo text must not be blank", demo.isNullOrBlank())
        assertTrue("Demo text must contain a keyword", demo!!.contains("void"))
    }
}
