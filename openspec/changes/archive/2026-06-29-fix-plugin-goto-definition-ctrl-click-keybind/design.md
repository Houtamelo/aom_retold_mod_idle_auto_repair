# Design: `fix-plugin-goto-definition-ctrl-click-keybind`

## Technical Approach

Add a single `GotoDeclarationHandler` that forwards Ctrl+Click / `Ctrl+B` / `⌘B` triggers for `.xs` files to the running XS LSP server via `textDocument/definition`. The handler lives in a new `navigation` package and is registered in `META-INF/plugin.xml`. A small `XsDefinitionResolver` seam keeps the LSP call testable.

## Architecture Decisions

| # | Decision | Choice | Rationale |
|---|---|---|---|
| ADR-1 | Handler type | `GotoDeclarationHandler` | Minimal scope: it only fills the broken Ctrl+Click/keybind path. `PsiReferenceContributor` would change how XS identifiers expose references and could break Find Usages. |
| ADR-2 | LSP request API | `LspServer.sendRequestSync(5000)` with try/catch | Matches the existing `sendNotification` call (`XsLspServerManager.kt:134`) and the 2024.2 platform LSP API described in the proposal. The 5 s timeout caps EDT blocking; exceptions/timeouts return empty results with no balloon. |
| ADR-3 | EP order | `order="last"` | Acts as a fallback. If the platform's own LSP wiring or another handler later returns targets, this handler only runs when earlier handlers produce nothing, reducing duplicate-target risk. |
| ADR-4 | External (vanilla `game/`) files | Convert URI → `VirtualFile` via `VirtualFileManager`, then `PsiManager.findFile` + leaf at line/column | Lets the editor open a non-project `.xs` buffer; if conversion fails, log and skip that target. |
| ADR-5 | Test seam | Constructor-injected `XsDefinitionResolver` interface | A `sealed interface` is unnecessary for two implementations (production LSP + test stub). The plugin.xml instantiation uses the Kotlin default argument; tests pass a stub. |

## Data Flow

```
Ctrl+Click / Ctrl+B
        │
        ▼
XsGotoDeclarationHandler.getGotoDeclarationTargets()
        │
        ├─ non-XS / no editor / no server → null / empty
        │
        ▼
XsLspDefinitionResolver.resolve(server, file, offset)
        │
        ▼
server.sendRequestSync(5000) { textDocument/definition }
        │
        ▼
Location[] / LocationLink[] → VirtualFile → PsiFile → PsiElement at offset
```

## File Changes

| File | Action | Description |
|---|---|---|
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandler.kt` | Create | Implements `GotoDeclarationHandler`; filters XS files, finds server, delegates to resolver. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsDefinitionResolver.kt` | Create | `XsDefinitionResolver` interface + `XsLspDefinitionResolver` production implementation. |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` | Create | Strict-TDD unit tests via `BasePlatformTestCase` and a stub resolver. |
| `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml:19-40` | Modify | Register `<gotoDeclarationHandler implementation="..." order="last"/>`. |
| `tools/intellij-xs-plugin/gradle.properties:12` | Modify | Patch bump `pluginVersion` `0.2.2 → 0.2.3`. |

## Interfaces / Contracts

### `XsGotoDeclarationHandler`

```kotlin
package com.aomr.xs.navigation

class XsGotoDeclarationHandler(
    private val resolver: XsDefinitionResolver = XsLspDefinitionResolver,
) : GotoDeclarationHandler {

    override fun getGotoDeclarationTargets(
        sourceElement: PsiElement?,
        offset: Int,
        editor: Editor?,
    ): Array<PsiElement>? {
        if (editor == null) return null
        if (sourceElement?.language != XsLanguage.INSTANCE) return null

        val file = sourceElement.containingFile?.virtualFile
            ?: sourceElement.containingFile?.originalFile?.virtualFile
            ?: return null

        val project = sourceElement.project
        val server = LspServerManager.getInstance(project)
            .getServersForProvider(XsLspSupportProvider::class.java)
            .firstOrNull { it.isSupportedFile(file) }
            ?: return emptyArray()

        return try {
            resolver.resolve(server, file, offset).toTypedArray()
        } catch (e: Exception) {
            LOG.warn("XS goto definition failed", e)
            emptyArray()
        }
    }
}
```

### `XsDefinitionResolver`

```kotlin
interface XsDefinitionResolver {
    fun resolve(server: LspServer, file: VirtualFile, offset: Int): List<PsiElement>
}

object XsLspDefinitionResolver : XsDefinitionResolver {

    override fun resolve(server: LspServer, file: VirtualFile, offset: Int): List<PsiElement> {
        val document = FileDocumentManager.getInstance().getDocument(file) ?: return emptyList()
        val line = document.getLineNumber(offset)
        val col = offset - document.getLineStartOffset(line)
        val params = DefinitionParams(TextDocumentIdentifier(file.url), Position(line, col))

        val response = try {
            server.sendRequestSync(5000) { ls -> ls.textDocumentService.definition(params) }
        } catch (e: Exception) {
            return emptyList()
        }

        return when {
            response == null -> emptyList()
            response.isLeft -> response.left.mapNotNull { it.toPsiElement(server.project) }
            else -> response.right.mapNotNull { it.toPsiElement(server.project) }
        }
    }

    private fun Location.toPsiElement(project: Project): PsiElement? =
        psiElementAt(project, uri.toVirtualFile() ?: return null, range.start.line, range.start.character)

    private fun LocationLink.toPsiElement(project: Project): PsiElement? =
        psiElementAt(project, targetUri.toVirtualFile() ?: return null, targetRange.start.line, targetRange.start.character)

    private fun String.toVirtualFile(): VirtualFile? =
        runCatching { VirtualFileManager.getInstance().findFileByUrl(this) }.getOrNull()

    private fun psiElementAt(project: Project, file: VirtualFile, line: Int, char: Int): PsiElement? {
        if (!file.isValid) return null
        val psiFile = PsiManager.getInstance(project).findFile(file) ?: return null
        val doc = PsiDocumentManager.getInstance(project).getDocument(psiFile) ?: return null
        val offset = (doc.getLineStartOffset(line) + char).coerceAtMost(doc.textLength)
        return psiFile.findElementAt(offset)
    }
}
```

### `plugin.xml` registration

```xml
<extensions defaultExtensionNs="com.intellij">
    <platform.lsp.serverSupportProvider implementation="com.aomr.xs.lsp.XsLspSupportProvider"/>

    <gotoDeclarationHandler
        id="com.aomr.xs.gotoDeclarationHandler"
        implementation="com.aomr.xs.navigation.XsGotoDeclarationHandler"
        order="last"/>

    <!-- existing extensions unchanged -->
</extensions>
```

## Testing Strategy

| Layer | What | Approach |
|---|---|---|
| Unit | Handler filters, resolver delegation, result mapping | `XsGotoDeclarationHandlerTest` extends `BasePlatformTestCase` and uses a stub `XsDefinitionResolver`. |
| Build | Plugin packages | `./gradlew buildPlugin` produces `dist/intellij-xs-plugin-0.2.3.zip`. |
| Manual | Rider Ctrl+Click / keybind / multi-target chooser / hover regression | Install built zip in Rider, open an XS mod file, trigger goto. |

## Test Cases (strict TDD)

Tests live in `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt`.

```kotlin
class XsGotoDeclarationHandlerTest : BasePlatformTestCase() {

    fun testSingleTargetReturnsOneNavigable() {
        val psiFile = myFixture.configureByText("a.xs", "void foo() {}\nvoid bar() { foo(); }")
        val target = psiFile.findElementAt(psiFile.text.indexOf("foo"))!!
        val handler = XsGotoDeclarationHandler(TestResolver(listOf(target)))

        val call = psiFile.findElementAt(psiFile.text.lastIndexOf("foo"))!!
        val result = handler.getGotoDeclarationTargets(call, call.textOffset, myFixture.editor)

        assertSize(1, result ?: emptyArray())
        assertSame(target, result!![0])
    }

    fun testMultiTargetReturnsMultipleNavigables() {
        val psiFile = myFixture.configureByText("a.xs", "void a() {}")
        val t1 = psiFile.findElementAt(psiFile.text.indexOf("a"))!!
        val t2 = psiFile.findElementAt(psiFile.text.lastIndexOf("a"))!!
        val handler = XsGotoDeclarationHandler(TestResolver(listOf(t1, t2)))
        val result = handler.getGotoDeclarationTargets(t1, 0, myFixture.editor)
        assertSize(2, result ?: emptyArray())
    }

    fun testNonXsFileReturnsNull() {
        val txt = myFixture.configureByText("a.txt", "hello")
        val handler = XsGotoDeclarationHandler(TestResolver(emptyList()))
        assertNull(handler.getGotoDeclarationTargets(txt.findElementAt(0), 0, myFixture.editor))
    }

    fun testLspTimeoutReturnsEmpty() {
        val psiFile = myFixture.configureByText("a.xs", "void a() {}")
        val handler = XsGotoDeclarationHandler(TestResolver(emptyList()))
        val element = psiFile.findElementAt(0)!!
        val result = handler.getGotoDeclarationTargets(element, 0, myFixture.editor)
        assertNotNull(result)
        assertEmpty(result!!.asList())
    }

    fun testExternalFileNavigatesCorrectly() {
        val psiFile = myFixture.configureByText("a.xs", "void a() {}")
        val external = PsiFileFactory.getInstance(project)
            .createFileFromText("external.xs", XsLanguage.INSTANCE, "void b() {}")!!
        val handler = XsGotoDeclarationHandler(TestResolver(listOf(external)))
        val result = handler.getGotoDeclarationTargets(psiFile.findElementAt(0)!!, 0, myFixture.editor)
        assertSize(1, result ?: emptyArray())
        assertEquals("external.xs", result!![0].containingFile.name)
    }

    private class TestResolver(private val results: List<PsiElement>) : XsDefinitionResolver {
        override fun resolve(server: LspServer, file: VirtualFile, offset: Int) = results
    }
}
```

## Strict-TDD Task List

1. Add `testSingleTargetReturnsOneNavigable`; run → FAIL.
2. Add `testMultiTargetReturnsMultipleNavigables`; run → FAIL.
3. Add `testNonXsFileReturnsNull`; run → FAIL.
4. Add `testLspTimeoutReturnsEmpty`; run → FAIL.
5. Add `testExternalFileNavigatesCorrectly`; run → FAIL.
6. Create `XsGotoDeclarationHandler.kt` returning `null`; register in `plugin.xml` with `order="last"`; run tests → FAIL (non-XS passes, others return wrong results).
7. Define `XsDefinitionResolver` and `XsLspDefinitionResolver` in `XsDefinitionResolver.kt`.
8. Wire handler to call `resolver.resolve()` and return the array; run tests → PASS.
9. Run `./gradlew :test`; confirm all tests green.
10. Run `./gradlew buildPlugin`; confirm `dist/intellij-xs-plugin-0.2.3.zip`.
11. Bump `tools/intellij-xs-plugin/gradle.properties:12` `pluginVersion` to `0.2.3`.
12. Hand off to orchestrator for commit.

## Migration / Rollout

No migration. Removing the new handler and reverting `plugin.xml` + `gradle.properties` restores the previous behavior.

## Out-of-scope Reminders

- Issue #1 only; Issues 2–4 are not addressed.
- No changes to `tools/xs-language-server/`.
- Hover, right-click **Go to → Declarations or usages**, and Find Usages must remain untouched.
- Include-statement navigation is deferred to a later change; this handler must not block adding a second `GotoDeclarationHandler`.

## Open Questions

None anticipated.
