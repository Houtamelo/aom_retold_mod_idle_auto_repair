# Verify Report: fix-plugin-goto-definition-ctrl-click-keybind

**Verifier:** sdd-verify executor (adversarial review, fresh context)  
**Date:** 2026-06-29  
**Plugin version under test:** `0.2.3`

## Verdict

**PASS WITH WARNINGS**

The implementation matches the spec, design, and tasks for Issue #1. The handler is registered, the new tests pass, the plugin artifact builds, and the version is bumped. However, two independent concerns remain: (1) the resolver blocks the EDT for up to 5 s, which contradicts the spec's claim that the editor remains responsive, and (2) the timeout scenario is covered only by a test that exercises the empty-result path, not an actual timeout.

## Test results

| Command | Result |
| ------- | ------ |
| `./gradlew :test --tests "com.aomr.xs.navigation.XsGotoDeclarationHandlerTest"` | **PASS** — 5/5 tests green |
| `./gradlew :test` (full suite) | **PARTIAL** — 67 tests completed, 1 pre-existing failure |
| `./gradlew :buildPlugin` | **PASS** |
| Inspect `dist/intellij-xs-plugin-0.2.3.zip` `META-INF/plugin.xml` | **PASS** — `<gotoDeclarationHandler>` present |

- **Total plugin tests:** 67
- **New tests pass:** 5 of 5 in `XsGotoDeclarationHandlerTest`
- **Pre-existing failures:** `XsStartupActivityTest.autoDetectedModsAreSavedAndNotified` — **environmental**. The test expects exactly 2 auto-detected mod roots (`mod_a` / `mod_b`) but untracked directories `mod/spire_ai/` and `mod/test_targeting/` cause 8 roots to be discovered. This failure exists independently of this change.
- **Build:** clean success
- **Plugin .zip:** built (`dist/intellij-xs-plugin-0.2.3.zip`, ~4.1 MB)
- **.zip `META-INF/plugin.xml`:** contains `<gotoDeclarationHandler id="com.aomr.xs.gotoDeclarationHandler" implementation="com.aomr.xs.navigation.XsGotoDeclarationHandler" order="last"/>`
- **.zip version:** `0.2.3`

## Spec coverage

| Requirement | Test | Status |
| ----------- | ---- | ------ |
| R1: Ctrl+Click navigates to a single definition | `testSingleTargetReturnsOneNavigable` | PASS |
| R2: Keybind navigates to a single definition | `testSingleTargetReturnsOneNavigable` (same handler path) | PASS |
| R3: Multi-target returns show a chooser | `testMultiTargetReturnsMultipleNavigables` | PASS — returns multiple `PsiElement`s; platform chooser popup is not unit-testable |
| R4: Right-click menu remains functional | (not unit-tested) | UNCOVERED — no `PsiReferenceContributor` added, menu path untouched |
| R5: Hover remains functional | (not unit-tested) | UNCOVERED — no hover extension modified |
| R6: Non-XS files do nothing | `testNonXsFileReturnsNull` | PASS |
| R7: LSP timeout does not freeze the editor | `testLspTimeoutReturnsEmpty` | PARTIAL — see adversarial findings |
| R8: External vanilla game file navigation works | `testExternalFileNavigatesCorrectly` | PASS — uses fixture temp file, not a real external `file://` URI |
| R9: Strict-TDD unit test added | `XsGotoDeclarationHandlerTest` exists and passes | PASS |

## Adversarial findings

1. **EDT blocking risk (real).** `XsGotoDeclarationHandler.getGotoDeclarationTargets` forwards to `XsLspDefinitionResolver`, which calls `server.sendRequestSync(5000)` on the EDT. Because `GotoDeclarationAction.findTargetElementsNoVS` invokes handlers synchronously on the EDT, a slow or hung LSP server will freeze the UI for up to 5 seconds. The spec R7 says the handler shall leave the editor responsive on timeout; the implementation only caps the freeze at 5 s. There is no progress indicator, no modal progress, and no async resolution.

2. **Timeout test does not test a timeout (real).** `testLspTimeoutReturnsEmpty` is misnamed: it injects a stub resolver that returns `emptyList()` and then asserts the result is a non-null empty array. It does not make the stub throw a `TimeoutException` / `ExecutionException`, so the production exception-swallowing path (`try/catch` around `sendRequestSync`) is not exercised by any automated test.

3. **External-file test does not exercise real URI conversion (medium).** `testExternalFileNavigatesCorrectly` creates the target via `myFixture.configureByText("external.xs", ...)`, so the target is a fixture temp file, not a vanilla `game/` file referenced by a `file://` URI. The `VirtualFileManager.findFileByUrl` conversion path is therefore not covered by an automated test.

4. **tasks.md has unstaged checkbox updates (cosmetic).** After staging, the apply agent checked off phases 5–7 in `tasks.md`. The change is related and benign, but the file currently has an unstaged modification.

## Working tree state

- **Staged vs HEAD (intended changes):**
  - `AGENTS.md` — added plugin test fixture gotcha
  - `docs/issues/2026-06-29-runtime-issues.md` — Issue 1 marked resolved
  - `openspec/changes/fix-plugin-goto-definition-ctrl-click-keybind/design.md` — new
  - `openspec/changes/fix-plugin-goto-definition-ctrl-click-keybind/explore.md` — new
  - `openspec/changes/fix-plugin-goto-definition-ctrl-click-keybind/proposal.md` — new
  - `openspec/changes/fix-plugin-goto-definition-ctrl-click-keybind/specs/spec-xs-goto-declaration-handler.md` — new
  - `openspec/changes/fix-plugin-goto-definition-ctrl-click-keybind/tasks.md` — new (checkboxes updated in working tree only)
  - `openspec/changes/fix-plugin-goto-definition-ctrl-click-keybind/verify-report.md` — this report
  - `tools/intellij-xs-plugin/gradle.properties` — `pluginVersion` `0.2.2 → 0.2.3`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsDefinitionResolver.kt` — new
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandler.kt` — new
  - `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` — `<gotoDeclarationHandler>` registration
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` — new

- **Unstaged modifications (not part of this change):**
  - `openspec/changes/fix-plugin-goto-definition-ctrl-click-keybind/tasks.md` — checkbox completion updates
  - `scripts/deploy-mods.sh`
  - `tools/xs-language-server/` (many files)

- **pluginVersion bump:** `0.2.2 → 0.2.3` (PATCH, per AGENTS.md)
- **Plugin .zip:** built and inspected

## Deviations from design

The following deviations were already reported by the apply agent and are confirmed by this review:

1. **Handler filter.** The design filtered by `sourceElement.language == XsLanguage.INSTANCE`. The implementation filters by `file.extension != "xs"` because headless `BasePlatformTestCase` PSI leaves report `TEXT` language. The AGENTS.md plugin test fixture gotcha documents this.
2. **Nullability of `LspServer`.** The design's `XsDefinitionResolver.resolve` took a non-null `LspServer`. The implementation widens it to `LspServer?` so unit tests can use a stub without a real server. The production implementation returns `emptyList()` when `server == null`.
3. **LSP server accessors.** The design used `server.isSupportedFile(file)` and `server.project`. The 2024.2 platform API exposes these via `server.descriptor.isSupportedFile(file)` and `server.descriptor.project`.
4. **No-server handling.** Instead of `?: return emptyArray()` after the server lookup, the implementation passes `null` to the resolver, which returns an empty list.
5. **File naming.** The proposal sketched a separate `XsLspDefinitionResolver.kt`, but the implementation places both the interface and the production object in `XsDefinitionResolver.kt`. This is a harmless implementation detail.

All deviations are benign and do not break any acceptance criterion.

## Recommendation

**Commit** the staged changes. The warnings are real but do not invalidate the fix:

- The EDT-blocking concern is inherent to the chosen `GotoDeclarationHandler` extension point and was accepted in the design (ADR-2). If non-blocking behavior becomes a hard requirement later, a different architecture (e.g., async `DirectNavigationProvider` or platform LSP wiring fix) would be needed.
- The timeout test gap should be closed in a follow-up by adding a stub resolver that throws an exception to exercise the `catch` path. This is a test-quality issue, not a functional defect.

## Risks

- UI freezes of up to 5 seconds if the LSP server is slow or hung.
- No automated proof that the timeout exception path actually swallows errors and returns an empty result.
- No automated proof that real `file://` URIs from the vanilla `game/` directory convert to navigable `PsiElement`s.
- If a future platform version starts auto-wiring `textDocument/definition` for Ctrl+Click, duplicate targets could appear despite `order="last"`; a manual Rider smoke test is still required.
