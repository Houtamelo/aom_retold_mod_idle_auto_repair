package com.aomr.xs.lsp

import com.aomr.xs.highlight.XsTextAttributes
import org.junit.Assert.assertEquals
import org.junit.Test

class XsSemanticTokensConverterTest {

    @Test
    fun engineFunctionMapsToFunctionEngine() {
        assertEquals(
            XsTextAttributes.FUNCTION_ENGINE,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_FUNCTION,
                setOf(XsSemanticTokensConverter.MODIFIER_ENGINE)
            )
        )
    }

    @Test
    fun moddedFunctionMapsToFunctionModded() {
        assertEquals(
            XsTextAttributes.FUNCTION_MODDED,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_FUNCTION,
                setOf(XsSemanticTokensConverter.MODIFIER_MODDED)
            )
        )
    }

    @Test
    fun unmoddedFunctionMapsToFunctionUnmodded() {
        assertEquals(
            XsTextAttributes.FUNCTION_UNMODDED,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_FUNCTION,
                setOf(XsSemanticTokensConverter.MODIFIER_UNMODDED)
            )
        )
    }

    @Test
    fun localVariableMapsToVariableLocal() {
        assertEquals(
            XsTextAttributes.VARIABLE_LOCAL,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_VARIABLE,
                emptySet()
            )
        )
    }

    @Test
    fun staticVariableMapsToVariableStatic() {
        assertEquals(
            XsTextAttributes.VARIABLE_STATIC,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_VARIABLE,
                setOf(XsSemanticTokensConverter.MODIFIER_STATIC)
            )
        )
    }

    @Test
    fun builtinTypeMapsToTypeBuiltin() {
        assertEquals(
            XsTextAttributes.TYPE_BUILTIN,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_TYPE,
                setOf(XsSemanticTokensConverter.MODIFIER_ENGINE)
            )
        )
    }

    @Test
    fun unmoddedClassMapsToTypeUnmoddedClass() {
        assertEquals(
            XsTextAttributes.TYPE_UNMODDED_CLASS,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_TYPE,
                setOf(XsSemanticTokensConverter.MODIFIER_UNMODDED)
            )
        )
    }

    @Test
    fun moddedClassMapsToTypeModdedClass() {
        assertEquals(
            XsTextAttributes.TYPE_MODDED_CLASS,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_TYPE,
                setOf(XsSemanticTokensConverter.MODIFIER_MODDED)
            )
        )
    }

    @Test
    fun unknownTokenTypeMapsToDefault() {
        assertEquals(
            com.aomr.xs.highlight.XsTextAttributesKeys.XS_DEFAULT,
            XsSemanticTokensConverter.convert(
                "unknown-token-type",
                setOf(XsSemanticTokensConverter.MODIFIER_ENGINE)
            )
        )
    }

    @Test
    fun functionWithoutAnyOriginModifierMapsToUnmodded() {
        // Functions called without an explicit engine/modded/unmodded modifier
        // are assumed to be unmodded (a conservative default).
        assertEquals(
            XsTextAttributes.FUNCTION_UNMODDED,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_FUNCTION,
                emptySet()
            )
        )
    }

    @Test
    fun constantMapsToConstant() {
        assertEquals(
            XsTextAttributes.CONSTANT,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_CONSTANT,
                setOf(XsSemanticTokensConverter.MODIFIER_MODDED)
            )
        )
    }

    @Test
    fun ruleMapsToRule() {
        assertEquals(
            XsTextAttributes.RULE,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_RULE,
                setOf(XsSemanticTokensConverter.MODIFIER_UNMODDED)
            )
        )
    }

    @Test
    fun externVariableUnmoddedMapsToVariableExternUnmodded() {
        assertEquals(
            XsTextAttributes.VARIABLE_EXTERN_UNMODDED,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_VARIABLE,
                setOf(
                    XsSemanticTokensConverter.MODIFIER_EXTERN,
                    XsSemanticTokensConverter.MODIFIER_UNMODDED
                )
            )
        )
    }

    @Test
    fun externVariableModdedMapsToVariableExternModded() {
        assertEquals(
            XsTextAttributes.VARIABLE_EXTERN_MODDED,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_VARIABLE,
                setOf(
                    XsSemanticTokensConverter.MODIFIER_EXTERN,
                    XsSemanticTokensConverter.MODIFIER_MODDED
                )
            )
        )
    }

    @Test
    fun engineMemberMethodMapsToMethodEngine() {
        assertEquals(
            XsTextAttributes.METHOD_ENGINE,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_FUNCTION,
                setOf(
                    XsSemanticTokensConverter.MODIFIER_MEMBER,
                    XsSemanticTokensConverter.MODIFIER_ENGINE
                )
            )
        )
    }

    @Test
    fun unmoddedMemberMethodMapsToMethodUnmodded() {
        assertEquals(
            XsTextAttributes.METHOD_UNMODDED,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_FUNCTION,
                setOf(
                    XsSemanticTokensConverter.MODIFIER_MEMBER,
                    XsSemanticTokensConverter.MODIFIER_UNMODDED
                )
            )
        )
    }

    @Test
    fun moddedMemberMethodMapsToMethodModded() {
        assertEquals(
            XsTextAttributes.METHOD_MODDED,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_FUNCTION,
                setOf(
                    XsSemanticTokensConverter.MODIFIER_MEMBER,
                    XsSemanticTokensConverter.MODIFIER_MODDED
                )
            )
        )
    }

    @Test
    fun engineMemberFieldMapsToFieldEngine() {
        assertEquals(
            XsTextAttributes.FIELD_ENGINE,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_VARIABLE,
                setOf(
                    XsSemanticTokensConverter.MODIFIER_MEMBER,
                    XsSemanticTokensConverter.MODIFIER_ENGINE
                )
            )
        )
    }

    @Test
    fun unmoddedMemberFieldMapsToFieldUnmodded() {
        assertEquals(
            XsTextAttributes.FIELD_UNMODDED,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_VARIABLE,
                setOf(
                    XsSemanticTokensConverter.MODIFIER_MEMBER,
                    XsSemanticTokensConverter.MODIFIER_UNMODDED
                )
            )
        )
    }

    @Test
    fun moddedMemberFieldMapsToFieldModded() {
        assertEquals(
            XsTextAttributes.FIELD_MODDED,
            XsSemanticTokensConverter.convert(
                XsSemanticTokensConverter.TOKEN_TYPE_VARIABLE,
                setOf(
                    XsSemanticTokensConverter.MODIFIER_MEMBER,
                    XsSemanticTokensConverter.MODIFIER_MODDED
                )
            )
        )
    }
}