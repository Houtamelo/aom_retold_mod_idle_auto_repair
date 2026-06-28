package com.aomr.xs

import com.intellij.lang.Language

class XsLanguage private constructor() : Language("xs") {
    companion object {
        val INSTANCE = XsLanguage()
    }
}
