package com.aomr.xs.highlight

import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.fileTypes.SyntaxHighlighter
import com.intellij.openapi.options.colors.AttributesDescriptor
import com.intellij.openapi.options.colors.ColorDescriptor
import com.intellij.openapi.options.colors.ColorSettingsPage
import javax.swing.Icon

/**
 * Color settings page for the XS language.
 *
 * The page is labeled **XS** in **Settings → Editor → Color Scheme**. It exposes:
 * - the original six lexical categories (Keyword, String, Comment, Number, Identifier, Default);
 * - twelve inherited platform categories grouped under **Code**, **Errors and Warnings**, and
 *   **Braces and Operators** using the IntelliJ `//` nested-group convention.
 *
 * `//` in an [AttributesDescriptor] display name creates a collapsible subcategory. For example,
 * `"Braces and Operators//Comma"` places *Comma* under the *Braces and Operators* group.
 */
class XsColorSettingsPage : ColorSettingsPage {

    override fun getDisplayName(): String = "XS"

    override fun getIcon(): Icon? = null

    override fun getHighlighter(): SyntaxHighlighter = XsSyntaxHighlighter()

    override fun getDemoText(): String = DEMO_TEXT

    override fun getAttributeDescriptors(): Array<AttributesDescriptor> = DESCRIPTORS

    override fun getColorDescriptors(): Array<ColorDescriptor> = ColorDescriptor.EMPTY_ARRAY

    override fun getAdditionalHighlightingTagToDescriptorMap(): Map<String, TextAttributesKey> = TAG_MAP

    companion object {

        /**
         * Descriptors shown in the XS color scheme page.
         *
         * Existing lexical categories are listed first, followed by inherited platform categories
         * grouped via `//`. The order here matches the rendered list order.
         */
        private val DESCRIPTORS = arrayOf(
            // Existing lexical categories
            AttributesDescriptor("Keyword", XsTextAttributesKeys.XS_KEYWORD),
            AttributesDescriptor("String", XsTextAttributesKeys.XS_STRING),
            AttributesDescriptor("Comment", XsTextAttributesKeys.XS_COMMENT),
            AttributesDescriptor("Number", XsTextAttributesKeys.XS_NUMBER),
            AttributesDescriptor("Identifier", XsTextAttributesKeys.XS_IDENTIFIER),
            AttributesDescriptor("Default", XsTextAttributesKeys.XS_DEFAULT),
            // Inherited General / Language Defaults categories
            AttributesDescriptor("Code//Identifier under caret (uses global General — per-language override is not supported in Rider)", XsTextAttributes.IDENTIFIER_UNDER_CARET),
            AttributesDescriptor("Code//Matched brace", XsTextAttributes.MATCHED_BRACE),
            AttributesDescriptor("Code//Unmatched brace", XsTextAttributes.UNMATCHED_BRACE),
            AttributesDescriptor("Errors and Warnings//Unknown symbol", XsTextAttributes.UNKNOWN_SYMBOL),
            AttributesDescriptor("Braces and Operators//Braces", XsTextAttributes.BRACES),
            AttributesDescriptor("Braces and Operators//Brackets", XsTextAttributes.BRACKETS),
            AttributesDescriptor("Braces and Operators//Comma", XsTextAttributes.COMMA),
            AttributesDescriptor("Braces and Operators//Dot", XsTextAttributes.DOT),
            AttributesDescriptor("Braces and Operators//Operation sign", XsTextAttributes.OPERATION_SIGN),
            AttributesDescriptor("Braces and Operators//Overloaded operator", XsTextAttributes.OVERLOADED_OPERATOR),
            AttributesDescriptor("Braces and Operators//Parentheses", XsTextAttributes.PARENTHESES),
            AttributesDescriptor("Braces and Operators//Semi-colon", XsTextAttributes.SEMI_COLON),
            // Semantic-token-driven categories (Bucket C, plugin 0.6.0)
            AttributesDescriptor("Identifier//Function//Engine function", XsTextAttributes.FUNCTION_ENGINE),
            AttributesDescriptor("Identifier//Function//UnModded function", XsTextAttributes.FUNCTION_UNMODDED),
            AttributesDescriptor("Identifier//Function//Modded function", XsTextAttributes.FUNCTION_MODDED),
            AttributesDescriptor("Identifier//Variable//Local variable", XsTextAttributes.VARIABLE_LOCAL),
            AttributesDescriptor("Identifier//Variable//Static variable", XsTextAttributes.VARIABLE_STATIC),
            AttributesDescriptor("Identifier//Type//Built-in type", XsTextAttributes.TYPE_BUILTIN),
            AttributesDescriptor("Identifier//Type//UnModded class", XsTextAttributes.TYPE_UNMODDED_CLASS),
            AttributesDescriptor("Identifier//Type//Modded class", XsTextAttributes.TYPE_MODDED_CLASS),
            // Additional semantic-token categories (Bucket C finish, plugin 0.7.0)
            AttributesDescriptor("Identifier//Variable//Constant", XsTextAttributes.CONSTANT),
            AttributesDescriptor("Identifier//Function//Rule", XsTextAttributes.RULE),
            AttributesDescriptor("Identifier//Variable//Extern UnModded", XsTextAttributes.VARIABLE_EXTERN_UNMODDED),
            AttributesDescriptor("Identifier//Variable//Extern Modded", XsTextAttributes.VARIABLE_EXTERN_MODDED),
            // Member semantic-token categories (Bucket C remainder, plugin 0.8.0)
            AttributesDescriptor("Identifier//Function//Method Engine", XsTextAttributes.METHOD_ENGINE),
            AttributesDescriptor("Identifier//Function//Method UnModded", XsTextAttributes.METHOD_UNMODDED),
            AttributesDescriptor("Identifier//Function//Method Modded", XsTextAttributes.METHOD_MODDED),
            AttributesDescriptor("Identifier//Variable//Field Engine", XsTextAttributes.FIELD_ENGINE),
            AttributesDescriptor("Identifier//Variable//Field UnModded", XsTextAttributes.FIELD_UNMODDED),
            AttributesDescriptor("Identifier//Variable//Field Modded", XsTextAttributes.FIELD_MODDED)
        )

        /**
         * Mapping from synthetic XML tags in [DEMO_TEXT] to the keys used for the preview.
         *
         * Tags fall into three groups:
         *  - **Syntactic categories** (`<keyword>`, `<string>`, etc.) demonstrate the original
         *    six lexical categories plus the inherited platform categories from
         *    [XsTextAttributes].
         *  - **Semantic-token categories** (`<functionengine>`, `<variablelocal>`, etc.) mirror
         *    the legend advertised by the LSP semantic-tokens response so the preview can show
         *    every colour a fully-running LSP instance would assign.
         *
         * Tags such as `<brace>`, `<bracket>`, and `<op>` let the preview demonstrate the
         * newly-added inherited categories alongside the original lexical categories.
         */
        private val TAG_MAP = mapOf(
            // -- Syntactic categories (XS lexical) --
            "keyword" to XsTextAttributesKeys.XS_KEYWORD,
            "string" to XsTextAttributesKeys.XS_STRING,
            "comment" to XsTextAttributesKeys.XS_COMMENT,
            "number" to XsTextAttributesKeys.XS_NUMBER,
            "identifier" to XsTextAttributesKeys.XS_IDENTIFIER,
            // -- Inherited platform categories (Braces and Operators) --
            "brace" to XsTextAttributes.BRACES,
            "bracket" to XsTextAttributes.BRACKETS,
            "paren" to XsTextAttributes.PARENTHESES,
            "comma" to XsTextAttributes.COMMA,
            "semicolon" to XsTextAttributes.SEMI_COLON,
            "dot" to XsTextAttributes.DOT,
            "op" to XsTextAttributes.OPERATION_SIGN,
            // -- Semantic-token categories (functions, variables, types) --
            "functionengine" to XsTextAttributes.FUNCTION_ENGINE,
            "functionunmodded" to XsTextAttributes.FUNCTION_UNMODDED,
            "functionmodded" to XsTextAttributes.FUNCTION_MODDED,
            "variablelocal" to XsTextAttributes.VARIABLE_LOCAL,
            "variablestatic" to XsTextAttributes.VARIABLE_STATIC,
            "variableexternunmodded" to XsTextAttributes.VARIABLE_EXTERN_UNMODDED,
            "variableexternmodded" to XsTextAttributes.VARIABLE_EXTERN_MODDED,
            "typebuiltin" to XsTextAttributes.TYPE_BUILTIN,
            "typeunmoddedclass" to XsTextAttributes.TYPE_UNMODDED_CLASS,
            "typemoddedclass" to XsTextAttributes.TYPE_MODDED_CLASS,
            "constant" to XsTextAttributes.CONSTANT,
            "rule" to XsTextAttributes.RULE,
            // -- Semantic-token categories (class members, plugin 0.8.0) --
            "methodengine" to XsTextAttributes.METHOD_ENGINE,
            "methodunmodded" to XsTextAttributes.METHOD_UNMODDED,
            "methodmodded" to XsTextAttributes.METHOD_MODDED,
            "fieldengine" to XsTextAttributes.FIELD_ENGINE,
            "fieldunmodded" to XsTextAttributes.FIELD_UNMODDED,
            "fieldmodded" to XsTextAttributes.FIELD_MODDED
        )

        /**
         * Preview snippet rendered on the XS color scheme page.
         *
         * The snippet deliberately exercises **every** colour category (syntactic + inherited
         * + semantic-token) and **every** syntactic feature of the XS language so a user
         * opening **Settings → Editor → Color Scheme → XS** sees a real preview of how each
         * category renders in their project.
         */
        private val DEMO_TEXT = """
            <comment>// XS sample: every color category + every language feature</comment>

            <comment>// Preprocessor: include directive</comment>
            <keyword>include</keyword> <string>"my_mod/utilities.xs"</string><semicolon>;</semicolon>

            <comment>// Top-level declarations, extern, const, static</comment>
            <keyword>extern</keyword> <keyword>int</keyword> <variableexternunmodded>gFoo</variableexternunmodded> <op>=</op> <number>-1</number><semicolon>;</semicolon>
            <keyword>extern</keyword> <keyword>int</keyword> <variableexternmodded>gCustomPlan</variableexternmodded> <op>=</op> <number>0</number><semicolon>;</semicolon>
            <keyword>const</keyword> <keyword>int</keyword> <constant>MAX_UNITS</constant> <op>=</op> <number>200</number><semicolon>;</semicolon>
            <keyword>const</keyword> <keyword>float</keyword> <constant>PI</constant> <op>=</op> <number>3.14159</number><semicolon>;</semicolon>
            <keyword>const</keyword> <keyword>string</keyword> <constant>GREETING</constant> <op>=</op> <string>"hello, world"</string><semicolon>;</semicolon>
            <keyword>const</keyword> <keyword>int</keyword> <constant>cNextReserve</constant> <op>=</op> <constant>cReserveIdle</constant> <op>+</op> <number>1</number><semicolon>;</semicolon>
            <keyword>static</keyword> <keyword>int</keyword> <variablestatic>gCounter</variablestatic> <op>=</op> <number>0</number><semicolon>;</semicolon>
            <keyword>mutable</keyword> <keyword>void</keyword> <functionunmodded>noop</functionunmodded><paren>(</paren><paren>)</paren> <brace>{</brace><brace>}</brace>

            <comment>// Function with array allocation, control flow + all operators</comment>
            <keyword>void</keyword> <functionunmodded>main</functionunmodded><paren>(</paren><paren>)</paren> <brace>{</brace>
                <keyword>int</keyword><bracket>[</bracket><bracket>]</bracket> <variablelocal>primes</variablelocal> <op>=</op> <keyword>new</keyword> <keyword>int</keyword><bracket>[</bracket><number>42</number><bracket>]</bracket><semicolon>;</semicolon>
                <keyword>string</keyword> <variablelocal>greeting</variablelocal> <op>=</op> <string>"hi"</string><semicolon>;</semicolon>
                <keyword>bool</keyword> <variablelocal>flag</variablelocal> <op>=</op> <keyword>true</keyword> <op>&amp;&amp;</op> <keyword>false</keyword><semicolon>;</semicolon>
                <keyword>float</keyword> <variablelocal>ratio</variablelocal> <op>=</op> <number>1.5</number><op>/</op><number>2.0</number><semicolon>;</semicolon>
                <keyword>if</keyword> <paren>(</paren><variablelocal>count</variablelocal> <op>>=</op> <number>0</number> <op>&amp;&amp;</op> <variablelocal>ratio</variablelocal> <op>!=</op> <number>0.0</number><paren>)</paren> <brace>{</brace>
                    <keyword>while</keyword> <paren>(</paren><variablelocal>count</variablelocal> <op>></op> <number>0</number><paren>)</paren> <brace>{</brace>
                        <variablelocal>count</variablelocal> <op>=</op> <variablelocal>count</variablelocal> <op>-</op> <number>1</number><semicolon>;</semicolon>
                        <keyword>break</keyword><semicolon>;</semicolon>
                    <brace>}</brace>
                <brace>}</brace> <keyword>else</keyword> <brace>{</brace>
                    <keyword>continue</keyword><semicolon>;</semicolon>
                <brace>}</brace>
            <brace>}</brace>

            <comment>// Default arguments + multi-param function</comment>
            <keyword>int</keyword> <functionunmodded>defaultFn</functionunmodded><paren>(</paren><keyword>int</keyword> <identifier>a</identifier> <op>=</op> <number>-1</number><comma>,</comma> <keyword>int</keyword> <identifier>b</identifier> <op>=</op> <number>0</number><paren>)</paren> <brace>{</brace>
                <keyword>return</keyword> <identifier>a</identifier> <op>*</op> <identifier>b</identifier><semicolon>;</semicolon>
            <brace>}</brace>

            <comment>// Built-in types used as values (vector literal)</comment>
            <typebuiltin>vector</typebuiltin> <variablelocal>origin</variablelocal> <op>=</op> <typebuiltin>vector</typebuiltin><paren>(</paren><number>0.0</number><comma>,</comma> <number>0.0</number><comma>,</comma> <number>0.0</number><paren>)</paren><semicolon>;</semicolon>

            <comment>// Engine API call (engine function from the bundled engine_api.json)</comment>
            <keyword>int</keyword> <variablelocal>n</variablelocal> <op>=</op> <functionengine>kbUnitCreate</functionengine><paren>(</paren><number>1</number><comma>,</comma> <number>0</number><comma>,</comma> <number>0</number><paren>)</paren><semicolon>;</semicolon>
            <keyword>int</keyword> <variablelocal>m</variablelocal> <op>=</op> <functionengine>aiPlanGetNumberByType</functionengine><paren>(</paren><number>42</number><paren>)</paren><semicolon>;</semicolon>

            <comment>// Rule with plan attributes</comment>
            <keyword>rule</keyword> <rule>tick</rule>
            <identifier>minInterval</identifier> <number>15</number>
            <identifier>active</identifier>
            <brace>{</brace>
                <keyword>int</keyword> <variablelocal>tickCount</variablelocal> <op>=</op> <number>0</number><semicolon>;</semicolon>
                <keyword>static</keyword> <keyword>int</keyword> <variablelocal>lastTick</variablelocal> <op>=</op> <op>-</op><number>1</number><semicolon>;</semicolon>
            <brace>}</brace>

            <comment>// Class declaration (unmodded) with fields and methods</comment>
            <keyword>class</keyword> <typeunmoddedclass>PlayerInfo</typeunmoddedclass> <brace>{</brace>
                <keyword>int</keyword> <fieldunmodded>mID</fieldunmodded> <op>=</op> <number>-1</number><semicolon>;</semicolon>
                <keyword>string</keyword> <fieldunmodded>mName</fieldunmodded> <op>=</op> <string>""</string><semicolon>;</semicolon>
                <keyword>vector</keyword> <fieldunmodded>mLocation</fieldunmodded> <op>=</op> <typebuiltin>cInvalidVector</typebuiltin><semicolon>;</semicolon>

                <keyword>int</keyword> <methodunmodded>getID</methodunmodded><paren>(</paren><paren>)</paren> <brace>{</brace> <keyword>return</keyword> <fieldunmodded>mID</fieldunmodded><semicolon>;</semicolon> <brace>}</brace>
                <keyword>void</keyword> <methodunmodded>setName</methodunmodded><paren>(</paren><keyword>string</keyword> <identifier>name</identifier> <op>=</op> <string>""</string><paren>)</paren> <brace>{</brace>
                    <fieldunmodded>mName</fieldunmodded> <op>=</op> <identifier>name</identifier><semicolon>;</semicolon>
                <brace>}</brace>
            <brace>}</brace>

            <comment>// Member access through a modded-class instance (fields + methods)</comment>
            <keyword>void</keyword> <functionunmodded>testAccess</functionunmodded><paren>(</paren><paren>)</paren> <brace>{</brace>
                <typemoddedclass>PlayerInfo</typemoddedclass> <variablelocal>info</variablelocal><semicolon>;</semicolon>
                <variablelocal>info</variablelocal><dot>.</dot><fieldmodded>mID</fieldmodded> <op>=</op> <number>1</number><semicolon>;</semicolon>
                <variablelocal>info</variablelocal><dot>.</dot><methodmodded>setName</methodmodded><paren>(</paren><string>"test"</string><paren>)</paren><semicolon>;</semicolon>
                <keyword>int</keyword> <variablelocal>id</variablelocal> <op>=</op> <variablelocal>info</variablelocal><dot>.</dot><methodmodded>getID</methodmodded><paren>(</paren><paren>)</paren><semicolon>;</semicolon>
                <keyword>vector</keyword> <variablelocal>pos</variablelocal> <op>=</op> <variablelocal>info</variablelocal><dot>.</dot><fieldmodded>mLocation</fieldmodded><semicolon>;</semicolon>
            <brace>}</brace>
        """.trimIndent()
    }
}
