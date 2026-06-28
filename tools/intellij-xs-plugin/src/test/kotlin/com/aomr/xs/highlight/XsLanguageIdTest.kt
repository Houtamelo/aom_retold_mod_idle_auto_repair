package com.aomr.xs.highlight

import com.aomr.xs.XsLanguage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class XsLanguageIdTest {

    @Test
    fun languageIdIsLowercaseXs() {
        assertEquals("XsLanguage id must be lowercase 'xs'", "xs", XsLanguage.INSTANCE.id)
    }

    @Test
    fun pluginXmlHasNoUppercaseXsLanguageReferences() {
        val pluginXml = javaClass.classLoader.getResourceAsStream("META-INF/plugin.xml")
            ?: throw AssertionError("META-INF/plugin.xml not found on test classpath")
        val text = pluginXml.use { it.reader().readText() }
        assertFalse(
            "plugin.xml must not contain any language=\"XS\" attributes",
            text.contains("""language="XS""")
        )
    }
}
