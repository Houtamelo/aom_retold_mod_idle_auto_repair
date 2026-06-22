package com.aomr.xs.psi

import com.aomr.xs.XsLanguage
import com.aomr.xs.psi.psi.XsFile
import com.intellij.lang.ASTNode
import com.intellij.lang.ParserDefinition
import com.intellij.lang.PsiParser
import com.intellij.lexer.Lexer
import com.intellij.openapi.project.Project
import com.intellij.psi.FileViewProvider
import com.intellij.psi.PsiElement
import com.intellij.psi.PsiFile
import com.intellij.psi.tree.IFileElementType
import com.intellij.psi.tree.TokenSet

class XsParserDefinition : ParserDefinition {

    override fun createLexer(project: Project): Lexer = XsLexerAdapter()

    override fun createParser(project: Project): PsiParser = PsiParser { _, builder ->
        val mark = builder.mark()
        while (builder.tokenType != null) {
            builder.advanceLexer()
        }
        mark.done(FILE)
        builder.treeBuilt
    }

    override fun getFileNodeType(): IFileElementType = FILE

    override fun getWhitespaceTokens(): TokenSet = WHITESPACES

    override fun getCommentTokens(): TokenSet = COMMENTS

    override fun getStringLiteralElements(): TokenSet = STRINGS

    override fun createElement(node: ASTNode): PsiElement =
        throw UnsupportedOperationException("Full XS PSI factory is not implemented until P2")

    override fun createFile(viewProvider: FileViewProvider): PsiFile = XsFile(viewProvider)

    companion object {
        val FILE = IFileElementType("XS_FILE", XsLanguage.INSTANCE)
        val WHITESPACES = TokenSet.create(XsTokenTypes.WHITE_SPACE)
        val COMMENTS = TokenSet.create(XsTokenTypes.LINE_COMMENT, XsTokenTypes.BLOCK_COMMENT)
        val STRINGS = TokenSet.create(XsTokenTypes.STRING_QUOTE, XsTokenTypes.CHAR_QUOTE)
    }
}
