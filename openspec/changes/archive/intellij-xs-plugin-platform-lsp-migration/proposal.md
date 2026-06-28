# Proposal: Migrate intellij-xs-plugin to IntelliJ Platform's built-in LSP API

## Intent

Fix the 3 known bugs that prevent the `intellij-xs-plugin` from "doing anything" in the IDE (no syntax highlighting, no completion, no error checking) by replacing the hand-rolled LSP4J integration (`XsLspConnection` + `XsLspServerManager` + `XsLanguageClient`, ~603 lines) with IntelliJ Platform's built-in LSP API at `com.intellij.platform.lsp.api.*`. The result is ~250 fewer lines of code, all 3 bugs fixed at once, and several features gained for free (semantic highlighting, inlay hints, folding, breadcrumbs, signature help, call/type hierarchy, rename, code lens, range formatting).

The LSP server itself (`tools/xs-language-server/`) is already working correctly. The fix is purely on the IntelliJ-client side.

## Scope

### In scope

1. Delete `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLanguageClient.kt`
2. Rewrite `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspConnection.kt` to a thin `XsBinaryResolver` helper (~60 lines, testable)
3. Rewrite `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerManager.kt` to a thin per-project service that listens to `XsSettings` changes and triggers workspace folder sync
4. Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspSupportProvider.kt` (~30 lines) — `LspServerSupportProvider` subclass
5. Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt` (~80 lines) — `ProjectWideLspServerDescriptor` subclass with binary resolution + workspace folder sync
6. Simplify `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/startup/XsStartupActivity.kt` — remove the `XsLspServerManager.updateSettings()` call (platform handles server start)
7. Edit `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` — add `<depends>com.intellij.modules.lsp</depends>` + `<depends>com.intellij.modules.ultimate</depends>`; swap `<postStartupActivity>` for `<platform.lsp.serverSupportProvider>`
8. Bump `pluginVersion` 0.1.4 → 0.1.5 in `tools/intellij-xs-plugin/gradle.properties`
9. Add JUnit tests:
   - `XsBinaryResolverTest.kt` — 5 binary-resolution strategies
   - `XsLspSupportProviderTest.kt` — file extension dispatch
   - `XsLspServerDescriptorTest.kt` — `createCommandLine()` args + `isSupportedFile()`

### Out of scope

1. Changes to the LSP server (`tools/xs-language-server/`) — server is working correctly
2. Changes to XS mod source files (`mod/`) — protected by project policy
3. Stub PSI implementation (`XsParserDefinition`, etc.) — stays as-is, even though it throws `UnsupportedOperationException`. The platform's LSP integration doesn't depend on it working.
4. TextMate grammar (`syntaxes/xs.tmLanguage.json`) — stays as-is
5. Distribution (publishing to Marketplace) — deferred per project policy until functional end-to-end

### Estimated impact

- **~−250 net lines** of Kotlin code
- **+3 new test files**
- **+2 plugin.xml deps**
- **+1 version bump** (0.1.4 → 0.1.5)
- **1 PR, single commit**

## Approach

The migration replaces the hand-rolled LSP4J plumbing with two new classes that delegate to `com.intellij.platform.lsp.api.*`:

### Architecture after migration

```
┌─────────────────────────────────────────────────────────────────┐
│ IntelliJ Platform (Rider 2026.2)                                │
│                                                                 │
│  XsLspSupportProvider (fileOpened) ─▶ XsLspServerDescriptor     │
│                                          │                      │
│                                          ├──▶ XsBinaryResolver │
│                                          │     (extracts bin)   │
│                                          ├──▶ createCommandLine()│
│                                          │     (--game-path)    │
│                                          ▼                      │
│                              ┌──────────────────────────┐       │
│  ┌──platform-managed────────│ LspServerManager (built-in)│──┐  │
│  │   didOpen/didChange       └──────────────────────────┘  │  │
│  │   diagnostics surfacing                                  │  │
│  │   completion contributor                                │  │
│  │   semantic highlighting                                  │  │
│  │   inlay hints, folding, breadcrumbs, ...                │  │
│  └───                                                       │  │
│                                                              │  │
│  XsSettings ─── change listener ──▶ XsLspServerManager       │  │
│                                       (workspace folder sync)│  │
└─────────────────────────────────────────────────────────────────┘
                                                                │
                                                                ▼
                                                    ┌──────────────────────┐
                                                    │ xs-language-server   │
                                                    │ (Rust, stdio)        │
                                                    └──────────────────────┘
```

### Key design decisions

1. **No `LspCustomization` opt-outs.** The Prisma plugin (canonical reference) disables ~14 platform-LSP features because it implements them natively in Kotlin. The XS plugin has no native PSI; it gets everything from the LSP. Default behavior = correct behavior for XS.

2. **Game folder as CLI arg.** Pass `--game-path <path>` from `createCommandLine()`. The LSP server caches the engine API at startup (extracted from `doxygen_retail.7z`); passing it as an init option via `getWorkspaceConfiguration()` would arrive too late.

3. **Mod list as workspace folders, sent via `didChangeWorkspaceFolders` after init.** Subscribe to `XsSettings` state changes. When mod list changes, find the running server via `LspServerManager.getInstance(project).servers` and send the delta. Fallback if no platform hook: trigger `LspClientManager.stopAndRestartClientsIfNeeded()`.

4. **One PR, single commit.** Intermediate states would be broken anyway (the old client only logs, so anything that depends on it is already broken). Single revert is easier than multi-PR revert.

5. **Stub PSI stays.** The `XsParserDefinition.createElement()` throws `UnsupportedOperationException`. This may have been interfering with TextMate-based syntax highlighting by claiming to handle XS while throwing on every call. With the platform API in place, the platform's LSP integration doesn't depend on PSI working. Cosmetic issue; can be fixed in a follow-up.

### Build artifacts

After implementation:
- `./gradlew test` — runs JUnit tests; expect new test files to pass
- `./gradlew buildPlugin` (with `JAVA_HOME` set to the JBR from Gradle caches; `-x buildSearchableOptions`) — produces `dist/intellij-xs-plugin-0.1.5.zip`
- Verify the .zip's bundled `bin/xs-language-server` matches the source binary SHA
- Install in Rider, open an `.xs` file, verify:
  - Syntax highlighting works (via TextMate, unchanged)
  - Ctrl+Space shows completions (NEW, was Bug #2)
  - Errors appear inline + in Problems window (NEW, was Bugs #1+#3)
  - Hover shows documentation (NEW, free feature)
  - "Go to Declaration" navigates between files (NEW, free feature)

### Reference implementation

`JetBrains/intellij-plugins/prisma/src/org/intellij/prisma/ide/lsp/` — Apache 2.0:
- `PrismaLspClientDescriptor.kt` (~4.9 KB) — shows `LspCustomization` opt-outs (we don't need any)
- `PrismaLspIntegrationProvider.kt` (~2.7 KB) — shows the `LspIntegrationProvider.fileOpened` pattern
- `PrismaLspServerActivationRule.kt` (~1.5 KB) — shows the activation rule pattern (we can skip; we use file extension only)

The Prisma plugin is JetBrains-owned and uses the same API surface. Total reference: ~10 KB of Kotlin.

## Rollback Plan

The migration is contained to `tools/intellij-xs-plugin/`. To roll back:

1. `git revert <migration-commit>` — single revert undoes all changes
2. Rebuild with `./gradlew buildPlugin`
3. Reinstall the previous plugin .zip in Rider
4. The LSP server is unchanged; the user can continue using plugin 0.1.4 with its known-but-different limitations

Risk: if the user has already installed 0.1.5 and the migration breaks the plugin, they're temporarily unable to use any XS language features (LSP doesn't connect). Mitigation: keep a backup copy of `dist/intellij-xs-plugin-0.1.4.zip` locally before installing 0.1.5.

## Patch-Maintenance Impact on `human_assist.xs`

**None.** The migration is purely on the IntelliJ plugin side. No XS source files (`mod/*/game/ai/human_assist/human_assist.xs` or any feature includes) are modified. The combined mod's `human_assist.xs` is untouched.

## Risks

1. **Workspace folder sync has no documented platform API hook in 2026.2.** Fallback (restart server on settings change) works fine; the mod list rarely changes during a session. **Mitigation**: implement both (try the hook first; fall back to restart).
2. **Plugin fails to load if `<depends>com.intellij.modules.lsp</depends>` is missing or unavailable.** **Mitigation**: build will catch this immediately.
3. **Stub PSI still throws `UnsupportedOperationException`.** **Mitigation**: cosmetic issue, follow-up fix. Doesn't block the migration.
4. **Single-PR migration is irreversible mid-flight.** **Mitigation**: single revert undoes everything.
5. **Test coverage gaps.** Some paths (e.g., the actual LSP initialize handshake) can only be tested with the LSP server running, which requires the game folder. **Mitigation**: keep manual smoke test in `docs/smoke-test-setup.md` as the integration test.

## Success Criteria

1. All 3 known bugs fixed (verified by smoke test: open `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs` in Rider; expect diagnostics in Problems window, completion popup on Ctrl+Space, semantic highlighting)
2. New JUnit tests pass (`./gradlew test`)
3. Plugin .zip builds without errors (`./gradlew buildPlugin`)
4. Plugin installs and loads in Rider 2026.2 without errors (`idea.log` clean)
5. LSP log shows expected activity (workspace folders received, didOpen events, semantic projects built)
6. Single PR, single commit, single version bump
7. No XS source files modified

## Ready for Spec

**Yes.** The proposal is well-scoped; specs will define the specific behavior contracts for each component.