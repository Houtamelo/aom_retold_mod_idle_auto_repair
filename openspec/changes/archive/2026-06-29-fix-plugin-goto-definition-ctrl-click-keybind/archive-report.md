# Archive Report: fix-plugin-goto-definition-ctrl-click-keybind

## Status
Archived

## Date
2026-06-29

## Artifacts
- Spec (NEW): `openspec/specs/spec-xs-goto-declaration-handler.md`
- Spec (synced from prior): N/A
- Design: `openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/design.md`
- Tasks: `openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/tasks.md`
- Verify report: `openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/verify-report.md`
- Proposal: `openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/proposal.md`
- Exploration: `openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/explore.md`

## Outcome
This change resolves Issue #1 by adding a plugin-side `GotoDeclarationHandler` that forwards Ctrl+Click and the go-to-definition keybind (`Ctrl+B` / `⌘B`) for `.xs` files to the active XS LSP server via `textDocument/definition`. The handler lives in a new `navigation` package, is registered in `META-INF/plugin.xml` with `order="last"`, and uses a small `XsDefinitionResolver` seam so the LSP call is unit-testable without a real server. Right-click **Go to → Declarations or usages** and hover continue to use their existing platform paths and remain unchanged.

The implementation adds five strict-TDD tests in `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` covering single target, multi-target, non-XS file filtering, empty-result/timeout, and external-file navigation. `./gradlew buildPlugin` produces `dist/intellij-xs-plugin-0.2.3.zip` with the handler registered. Full `./gradlew :test` reports 67 tests completed (62 baseline + 5 new), with one pre-existing environmental failure in `XsStartupActivityTest.autoDetectedModsAreSavedAndNotified` caused by untracked mod directories unrelated to this change.

## Test count trajectory
- Before: 62
- After: 67 (+5 from `XsGotoDeclarationHandlerTest.kt`)

## Cross-references
- User issue: `docs/issues/2026-06-29-runtime-issues.md` Issue 1 (now marked Resolved)
- Related archived change: `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/` (Batch D — same strict-TDD verification pattern)
- Plugin test fixture gotcha: `AGENTS.md` documents that `BasePlatformTestCase` `.xs` buffers should be identified by `virtualFile.extension == "xs"` rather than `PsiElement.language`, because headless fixtures report the platform `TEXT` language for PSI leaves
- Version policy: `AGENTS.md` plugin version bump policy (PATCH bump for bug fixes with no new public feature)

## Commit guidance
The orchestrator will commit the change with the following Conventional Commits message:
```
fix(xs-plugin): wire GotoDeclarationHandler for Ctrl+Click/Ctrl+B goto-definition (Issue #1)

Adds com.aomr.xs.navigation.XsGotoDeclarationHandler, registered in
META-INF/plugin.xml with order="last", forwarding to the active XS LSP
server's textDocument/definition endpoint. Includes XsDefinitionResolver
seam and XsGotoDeclarationHandlerTest with five strict-TDD cases.

Bumps pluginVersion 0.2.2 → 0.2.3.
Closes Issue #1 from docs/issues/2026-06-29-runtime-issues.md.
```

## Plugin version
Bumped `0.2.2 → 0.2.3` (PATCH, per `AGENTS.md`; bug fix with no new public plugin API).
