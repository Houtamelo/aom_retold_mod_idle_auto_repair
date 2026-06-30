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
            text.contains("""<colorSettingsPage implementation="com.aomr.xs.highlight.XsColorSettingsPage""")
        )
    }

    @Test
    fun test_display_name_is_XS_uppercase() {
        val page = XsColorSettingsPage()
        assertEquals("XS", page.displayName)
    }

    @Test
    fun test_existing_six_categories_still_present() {
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
    fun test_attribute_descriptors_contain_12_new_keys() {
        val page = XsColorSettingsPage()
        val descriptors = page.attributeDescriptors
        val keys = descriptors.map { it.key }.toSet()
        assertTrue("IDENTIFIER_UNDER_CARET descriptor must be present", keys.contains(XsTextAttributes.IDENTIFIER_UNDER_CARET))
        assertTrue("MATCHED_BRACE descriptor must be present", keys.contains(XsTextAttributes.MATCHED_BRACE))
        assertTrue("UNMATCHED_BRACE descriptor must be present", keys.contains(XsTextAttributes.UNMATCHED_BRACE))
        assertTrue("UNKNOWN_SYMBOL descriptor must be present", keys.contains(XsTextAttributes.UNKNOWN_SYMBOL))
        assertTrue("BRACES descriptor must be present", keys.contains(XsTextAttributes.BRACES))
        assertTrue("BRACKETS descriptor must be present", keys.contains(XsTextAttributes.BRACKETS))
        assertTrue("COMMA descriptor must be present", keys.contains(XsTextAttributes.COMMA))
        assertTrue("DOT descriptor must be present", keys.contains(XsTextAttributes.DOT))
        assertTrue("OPERATION_SIGN descriptor must be present", keys.contains(XsTextAttributes.OPERATION_SIGN))
        assertTrue("OVERLOADED_OPERATOR descriptor must be present", keys.contains(XsTextAttributes.OVERLOADED_OPERATOR))
        assertTrue("PARENTHESES descriptor must be present", keys.contains(XsTextAttributes.PARENTHESES))
        assertTrue("SEMI_COLON descriptor must be present", keys.contains(XsTextAttributes.SEMI_COLON))
    }

    @Test
    fun demoTextIsNonEmptyXsSnippet() {
        val page = XsColorSettingsPage()
        val demo = page.demoText
        assertNotNull("Demo text must not be null", demo)
        assertFalse("Demo text must not be blank", demo.isNullOrBlank())
        assertTrue("Demo text must contain a keyword", demo!!.contains("void"))
    }

    @Test
    fun test_identifier_under_caret_descriptor_documents_global_setting() {
        val page = XsColorSettingsPage()
        val descriptors = page.attributeDescriptors
        val descriptor = descriptors.find { it.displayName.contains("Identifier under caret") }
            ?: throw AssertionError("Identifier under caret descriptor not found")
        assertTrue(
            "Descriptor label must document the global limitation",
            descriptor.displayName.contains("uses global General", ignoreCase = true)
        )
    }
}
