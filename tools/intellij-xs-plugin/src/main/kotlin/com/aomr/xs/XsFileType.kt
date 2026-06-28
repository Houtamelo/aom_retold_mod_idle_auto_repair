package com.aomr.xs

import com.intellij.openapi.fileTypes.LanguageFileType
import com.intellij.openapi.util.IconLoader
import javax.swing.Icon

class XsFileType private constructor() : LanguageFileType(XsLanguage.INSTANCE) {
    override fun getName(): String = "XS"
    override fun getDescription(): String = "XS script file"
    override fun getDefaultExtension(): String = "xs"
    override fun getIcon(): Icon = ICON

    companion object {
        val INSTANCE = XsFileType()

        @JvmStatic
        val ICON: Icon = IconLoader.getIcon("/icons/xs.svg", XsFileType::class.java)
    }
}
