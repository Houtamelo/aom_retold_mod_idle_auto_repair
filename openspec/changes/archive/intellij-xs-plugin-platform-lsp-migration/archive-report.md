# Archive report: intellij-xs-plugin-platform-lsp-migration

## Summary

This change migrated the `intellij-xs-plugin` IDE client from ~600 lines of hand-rolled LSP4J plumbing to IntelliJ Platform's built-in `com.intellij.platform.lsp.api.*` API. The LSP server itself (`tools/xs-language-server/`) was already functional; the plugin was only logging server output instead of surfacing it. The migration fixed three known bugs at once (logged-only diagnostics, missing completion, missing diagnostic integration) and gained additional platform-managed features for free, including semantic highlighting, inlay hints, folding, breadcrumbs, signature help, call/type hierarchy, rename, code lens, and range formatting. Plugin version was bumped from 0.1.4 to 0.1.5.

Implementation was split into an initial apply batch (T1-T15) and a remediation batch. The initial batch created the new provider/descriptor/resolver classes, rewrote the manager and startup activity, deleted the obsolete client/connection classes, and added JUnit tests. Verification exposed a CRITICAL workspace-folder baseline bug and two missing behaviors (settings listener, restart fallback); the remediation batch fixed all three issues and added six covering tests. Final verify verdict was **PASS WITH ISSUES** — the only remaining issue was unrelated working-tree strays from the concurrent `true-include-paste` change, which must be excluded from the eventual commit.

## What changed

- Replaced ~600 lines of hand-rolled LSP4J plumbing with IntelliJ Platform's built-in LSP API
- Fixed 3 known bugs (diagnostics only logged, no completion, no diagnostic integration)
- Gained features for free: semantic highlighting, inlay hints, folding, breadcrumbs, signature help, call/type hierarchy, rename, code lens, range formatting
- Plugin version bumped 0.1.4 → 0.1.5

## Specs synced

- `openspec/specs/spec-lsp-server-lifecycle.md`
- `openspec/specs/spec-workspace-folder-sync.md`
- `openspec/specs/spec-platform-integration.md`

## Files changed

### Code / build

**Created:**

- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsBinaryResolver.kt`
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt`
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspSupportProvider.kt`
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsBinaryResolverTest.kt`
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsLspServerDescriptorTest.kt`
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsLspSupportProviderTest.kt`
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsLspServerManagerTest.kt`
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/startup/XsStartupActivityTest.kt`

**Modified:**

- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerManager.kt`
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/XsSettings.kt`
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/startup/XsStartupActivity.kt`
- `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`
- `tools/intellij-xs-plugin/build.gradle.kts`
- `tools/intellij-xs-plugin/gradle.properties`

**Deleted:**

- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLanguageClient.kt`
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspConnection.kt`

### SDD artifacts (now archived)

- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/archive-report.md`
- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/design.md`
- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/explore.md`
- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/proposal.md`
- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/tasks.md`
- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/verify-report.md`
- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/specs/spec-lsp-server-lifecycle.md`
- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/specs/spec-platform-integration.md`
- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/specs/spec-workspace-folder-sync.md`

## Verify verdict

PASS WITH ISSUES (only pre-existing strays in `mod/` and `openspec/specs/` from concurrent `true-include-paste` change — not part of this work; addressed via explicit-path staging per AGENTS.md pre-commit hygiene).

## Deviations from design

| # | Deviation | Status | Justification | Breaks spec? |
|---|-----------|--------|---------------|--------------|
| 1 | Build target IC → IU 2024.2 | Permanent | Cached IC distribution lacked `com.intellij.modules.lsp` API classes. | No |
| 2 | `XsLspServerDescriptor` extends base `LspServerDescriptor` | Permanent | 2024.2 `ProjectWideLspServerDescriptor` does not expose a vararg workspace-folder constructor; base class preserves one-server-per-project semantics via the provider. | No |
| 3 | Stderr redirect via `startServerProcess()` override | Permanent | Platform default merges stderr into stdout, which would corrupt the JSON-RPC stream. Redirect writes to `<project>/.idea/xs-lsp.log`. | No |
| 4 | Settings listener not wired | Remediated | `XsSettings.Listener` added; `XsLspServerManager` subscribes in `init {}`. | No |
| 5 | No restart fallback on send failure | Remediated | `notifyWorkspaceFoldersChanged()` falls back to `restartServer()` when no server is running or send throws. | No |

## Lessons learned

- Strict TDD was OFF during initial apply (T1-T15) — tests written after impl.
- Strict TDD was ON for remediation work — RED-GREEN cycle followed.
- Policy now active project-wide for LSP/plugin tasks (obs #1344).

## Next steps

- The actual commit is NOT part of this archive (orchestrator handles commit after user smoke test in Rider).
- Smoke test in Rider 2026.2 confirms: diagnostics, completion, hover, go-to-decl, semantic highlighting.
- Plugin .zip: `dist/intellij-xs-plugin-0.1.5.zip`.
