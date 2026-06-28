package com.aomr.xs

import com.intellij.lang.Language

class XsLanguage private constructor() : Language("XS") {
    companion object {
        val INSTANCE = XsLanguage()
    }
}
