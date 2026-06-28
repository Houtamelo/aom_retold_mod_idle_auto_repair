package com.aomr.xs.psi.psi

import com.aomr.xs.XsFileType
import com.aomr.xs.XsLanguage
import com.intellij.extapi.psi.PsiFileBase
import com.intellij.psi.FileViewProvider

class XsFile(viewProvider: FileViewProvider) : PsiFileBase(viewProvider, XsLanguage.INSTANCE) {
    override fun getFileType() = XsFileType.INSTANCE
}
