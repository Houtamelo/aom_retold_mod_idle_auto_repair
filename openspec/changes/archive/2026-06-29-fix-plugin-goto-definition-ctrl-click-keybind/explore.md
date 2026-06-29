## Bug status

CONFIRMED (client-side).

The LSP server already advertises and implements `textDocument/definition`:
- `tools/xs-language-server/src/server.rs:321` sets `definition_provider: Some(OneOf::Left(true))`.
- `tools/xs-language-server/src/server.rs:638-695` implements `goto_definition` and returns a `Location`.

Hover and right-click navigation work, so the server response is valid. The missing piece is on the IntelliJ Platform client side: there is no explicit `GotoDeclarationHandler` registration for Ctrl+Click / the keyboard shortcut path.

## Architecture summary

The plugin is a thin LSP client built on the IntelliJ Platform LSP API (`com.intellij.platform.lsp.api`).

- `META-INF/plugin.xml:20` registers `platform.lsp.serverSupportProvider` → `XsLspSupportProvider`.
- `XsLspSupportProvider.kt:17-28` starts the server when an `.xs` file is opened and a game folder is configured.
- `XsLspServerDescriptor.kt:26-52` describes how to launch the bundled Rust LSP binary and which files it supports.
- The plugin has almost no custom language-side PSI/reference/navigation code: it only provides a lexer, syntax highlighter, brace matcher, commenter, and a stub parser/AST (`XsParserDefinition.kt`, `XsIdentifier.kt`).
- There is **no** custom `GotoDeclarationHandler`, `PsiReferenceContributor`, `navigationGutterIconBuilder`, or `editor-navigate-to-declaration` action override.

In this architecture the platform is expected to auto-wire LSP features (hover, completion, diagnostics, rename, go-to-definition, find usages). Hover and right-click "Go to → Declarations or usages" are auto-wired, but Ctrl+Click and the go-to-definition keybind are not.

## Code-path map

| Entry point | Expected path | What happens today | Evidence |
|-------------|---------------|--------------------|----------|
| **Hover** (mouse over symbol) | Platform LSP hover handler → `textDocument/hover` | **Works**; popup shows `function: {name}\nDefined in: {path}` | Issue report + server hover implementation |
| **Right-click → Go to → Declarations or usages** | Platform LSP references/`textDocument/references` or `GotoDeclarationHandler` EP | **Works** | Issue report |
| **Ctrl+Click** symbol | `CtrlMouseHandler` → `GotoDeclarationAction.getCtrlMouseData` → `GotoDeclarationOrUsageHandler2` → `GotoDeclarationHandler` EP / references | **No-op** | Issue report |
| **Keybind (Ctrl+B / ⌘B)** | `GotoDeclarationAction` → `GotoDeclarationOrUsageHandler2` → `GotoDeclarationHandler` EP / references | **No-op** | Issue report |

Key files in the existing path:
- `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml:20` — LSP server support provider registration.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspSupportProvider.kt:19-28` — server start trigger.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt:26-35` — server descriptor / `isSupportedFile`.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsIdentifier.kt:18-19` — identifier leaf that asks `ReferenceProvidersRegistry` for references.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsParserDefinition.kt:37-38` — stub `createElement` throws (composite nodes only; leaves are handled by `XsASTFactory`).
- `tools/xs-language-server/src/server.rs:321,638-695` — server-side definition provider.

Notably, `plugin.xml` does **not** contain a `<gotoDeclarationHandler>` entry (confirmed by grep of the whole `tools/intellij-xs-plugin/src/main` tree).

## Root cause

The plugin relies on the IntelliJ Platform LSP client to translate `textDocument/definition` into navigation targets. While the platform documentation states that "Go to Declaration (`textDocument/definition`)" is supported since 2024.2, and the platform's Find-Usages/hover paths work for XS, the Ctrl+Click and keybind paths are not producing navigation targets in this configuration.

There are two probable, compounding reasons:

1. **No explicit client-side handler.** `GotoDeclarationHandler.EP_NAME` is not registered for the `xs` language. Because the parser is only a stub (no real PSI beyond `FILE` and identifier leaves), the default navigation fallback paths have no `PsiReference` or named PSI element to resolve.
2. **Platform auto-wiring gap on the definition code path.** Community reports and the JetBrains support forum confirm that in some 2024.2-based setups the LSP client's go-to-declaration wiring is inconsistent even though hover and find-usages work (see the 2023 JetBrains support thread where a JetBrains engineer says code completion and go-to-declaration "should work out of the box" but also asks for `#com.intellij.platform.lsp` logs because in practice they sometimes do not). The right-click "Declarations or usages" path is likely using theplatform's `textDocument/references` support, which is documented as supported in 2024.2, while the `textDocument/definition` path used by Ctrl+Click/Ctrl+B is not returning a target for our lightweight language module in Rider.

Because there is no fallback handler, the user gets no navigation at all from Ctrl+Click or the keyboard shortcut.

References:
- JetBrains support forum on LSP navigation: https://intellij-support.jetbrains.com/hc/en-us/community/posts/13320584737554-LSP-integration-only-working-with-diagnostics-not-completions-or-jumping
- LSP4IJ docs showing go-to-definition is implemented via the `gotoDeclarationHandler` EP: https://github.com/redhat-developer/lsp4ij/blob/0fbcf3ad/docs/LSPSupport.md
- IntelliJ Platform LSP feature table (2024.2): https://plugins.jetbrains.com/docs/intellij/language-server-protocol.html

## Affected files

- `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml:19-40` — needs the `gotoDeclarationHandler` extension registration.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt:26-109` — may need to expose/call the LSP definition request; the handler will locate the active `LspServer` via `LspServerManager`.
- New file: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandler.kt` — implementation of `GotoDeclarationHandler` that forwards to the LSP server and converts `Location` into a navigable `PsiElement`.
- `tools/intellij-xs-plugin/gradle.properties:12` — `pluginVersion` bump (0.2.2 → 0.2.3, patch, because this is a plugin-side bug fix that changes the artifact).

Not affected (no changes expected):
- LSP server (`tools/xs-language-server/`) — `definitionProvider` and `textDocument/definition` already work.
- Syntax highlighting / TextMate files.
- Settings (`tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/`).

## Test infrastructure

Current plugin tests are JUnit 4 `ProjectRule`-based unit tests for settings, LSP support-provider wiring, and binary resolution. There are no code-insight/navigation tests and no tests that exercise `GotoDeclarationHandler`.

A new test will be needed:
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt`
- Should extend `BasePlatformTestCase` or use `IdeaTestFixtureFactory.createCodeInsightFixture` so it can place a caret, invoke navigation, and assert the resulting file/offset.
- Because the real LSP server requires the AoM:R game folder and a bundled binary, tests should inject a seam (e.g., a test-only `XsDefinitionResolver`) that returns canned `Location`s. This mirrors the existing pattern in `XsLspServerManagerTest` where `sendWorkspaceFolderChange` is replaced with a capture double.

If the proposal chooses to implement a `PsiReferenceContributor` instead of (or in addition to) a `GotoDeclarationHandler`, the test will exercise `PsiReference.resolve()` rather than the action-level navigation.

## Plugin version

Current: `0.2.2` (`gradle.properties:12`).

This change touches the plugin artifact and bundled behavior, so per `AGENTS.md` the version must be bumped. Recommended: **patch bump to `0.2.3`** (bug fix, no new public feature).

## Audit cross-reference

Searched `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` and `docs/code-reviews/2026-06-29-lsp-and-plugin-findings.csv` for `GotoDeclaration`, `Ctrl+Click`, `keybind`, `goto.declaration`.

Result: **no overlapping audit findings**. This is a runtime UX bug that the static-only audit did not catch, as noted in the issue doc (`docs/issues/2026-06-29-runtime-issues.md:88-93`).

## Scope recommendation for proposal phase

- **Files to change:**
  - Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandler.kt`
  - Modify `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` to add the extension
  - Modify `tools/intellij-xs-plugin/gradle.properties` to bump `pluginVersion`
- **New tests:**
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt`
- **LOC delta:** +~80–150 lines of Kotlin, +1–2 lines XML, +1 line properties; no deletions expected.
- **Risk:** medium. The fix blocks on an LSP request on the EDT and depends on experimental/internal LSP client APIs (depending on which request mechanism is available in 2024.2). There is also a possibility of duplicate navigation targets if the platform's automatic `textDocument/definition` wiring later starts working.
- **Effort:** 6–10 hours (implementation 3–4h, tests/fixtures 2–3h, Rider smoke test 1–2h).
- **Audit overlap:** none.

Alternative to discuss in proposal: instead of a `GotoDeclarationHandler`, implement a `PsiReferenceContributor`/`PsiReferenceProvider` for `XsIdentifier`. This would make the symbol clickable via the standard reference path, but it is more code and may overlap with the platform's LSP reference provider. The proposal should pick one approach and document the trade-off.

## Risks / unknowns

1. **LSP request API shape in 2024.2.** The exact method on `LspServer` to request `textDocument/definition` needs to be confirmed against the 2024.2 API. The docs mention `LspRequest`/`LspClientNotification` for custom requests; there may be a simpler `sendRequest { lsp4jServer -> ... }` or `executeBlocking` method. This must be resolved before implementation.
2. **EDT blocking.** `GotoDeclarationHandler.getGotoDeclarationTargets` runs in a read action on the EDT. A blocking LSP request can freeze the UI; we may need a timeout and/or use the platform's progress/ModalAlert pattern.
3. **Duplicate targets if platform handler works.** If the platform LSP client's own go-to-definition handler is present and also returns results, the user could see duplicate targets. The implementation should either de-duplicate by URI or guard the handler so it only returns a target when the platform handler does not.
4. **Rider specifics.** The bug was reported in Rider. Ctrl+Click in Rider can be influenced by ReSharper; verification in a real Rider instance is required because headless platform tests may not reproduce Rider's action wiring.
5. **Include-statement navigation is out of scope here.** Issue #4 covers include-path go-to-definition; this change should not try to fix it, but the design should not make it harder to add a second handler later.

## Next step

The proposal phase should decide between a `GotoDeclarationHandler` and a `PsiReferenceContributor`/`PsiReferenceProvider` for XS identifiers, then specify the exact LSP request API and EDT-blocking strategy used to turn `textDocument/definition` results into navigable `PsiElement`s.
