@file:Suppress("DEPRECATION")
package com.aomr.xs

import com.intellij.openapi.fileTypes.FileTypeConsumer
import com.intellij.openapi.fileTypes.FileTypeFactory

@Suppress("DEPRECATION")
class XsFileTypeFactory : FileTypeFactory() {
    override fun createFileTypes(consumer: FileTypeConsumer) {
        consumer.consume(XsFileType.INSTANCE, "xs")
    }
}
