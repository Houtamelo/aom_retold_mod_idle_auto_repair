# Tasks: LSP platform API migration

This breaks the migration into 13 tasks, grouped into 4 phases. Each task is completable in a single session and has clear verification steps.

## Phase 1: Setup and scaffolding (T1-T2)

### T1. Verify platform API surface

**Files**: `tools/intellij-xs-plugin/build.gradle.kts` (no edits yet)

**Steps**:
1. Run `./gradlew --refresh-dependencies` in `tools/intellij-xs-plugin/` to ensure latest dependencies
2. Open `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/` in your IDE
3. In IntelliJ, use Navigate → Class... → `LspServerSupportProvider` to verify the class exists and download platform sources (per docs: "Navigate | Class... to open the `LspServerManager` class. In the opened editor, invoke Download IntelliJ Platform sources to download and attach sources.")
4. Note the exact method signatures for `LspServerSupportProvider.fileOpened()` and `LspServerDescriptor.createLsp4jClient()`
5. Document any deviations from the design.md "Open questions" section

**Verification**: platform sources are downloaded and attached; you can navigate to `LspServerSupportProvider` source.

- [x] T1: Verified via官方 docs + Prisma reference; `LspServerSupportProvider.fileOpened(Project, VirtualFile, LspServerSupportProvider.LspServerStarter)`, `ProjectWideLspServerDescriptor(project, name)` and `createCommandLine(): GeneralCommandLine`. The cached IC 2024.2 distribution did not contain the LSP API classes, so the build was switched to IU 2024.2 (see T2/blocker notes).

### T2. Add `<depends>` modules to plugin.xml

**Files**: `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`

**Steps**:
1. Add `<depends>com.intellij.modules.lsp</depends>` to the `<depends>` block
2. Add `<depends>com.intellij.modules.ultimate</depends>` to the `<depends>` block
3. Run `./gradlew compileKotlin` to verify the plugin still compiles

**Verification**: `compileKotlin` succeeds; no class-not-found errors.

- [x] T2: Added `<depends>com.intellij.modules.lsp</depends>` and `<depends>com.intellij.modules.ultimate</depends>` to `plugin.xml`; switched `gradle.properties` to `platformType = IU` and wired `build.gradle.kts` to read `platformType`. `compileKotlin` verification runs after new classes are in place.

## Phase 2: New LSP integration classes (T3-T5)

### T3. Create `XsBinaryResolver.kt`

**Files**: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsBinaryResolver.kt` (new)

**Steps**:
1. Create the file with `package com.aomr.xs.lsp`
2. Implement `object XsBinaryResolver` with:
   - `fun resolve(systemProperty, envVar, projectBasePath, resourceProvider): String`
   - `private fun extractBundled(resourcePath, resourceProvider): String?`
   - `private fun findProjectBinary(projectBasePath): String?`
3. Use the 5 strategies from `spec-lsp-server-lifecycle.md`
4. Cache the extracted binary path in a `@Volatile var`

**Reference**: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspConnection.kt:229-288` (existing logic to migrate)

**Verification**: file compiles; `resolve()` returns the right path for each strategy.

- [x] T3: Created `XsBinaryResolver.kt` with system-property → env-var → bundled → project-local → PATH strategies, JVM-cached bundled extraction, and JUnit tests.

### T4. Create `XsLspServerDescriptor.kt`

**Files**: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt` (new)

**Steps**:
1. Create the file with `package com.aomr.xs.lsp`
2. Implement `class XsLspServerDescriptor(project, gamePath) : ProjectWideLspServerDescriptor(project, "XS Language Server")`
3. Override `isSupportedFile(file: VirtualFile): Boolean = file.extension == "xs"`
4. Override `createCommandLine(): GeneralCommandLine` — uses `XsBinaryResolver.resolve(...)` and adds `--game-path <gamePath>` arg
5. Override `createLsp4jClient(): Lsp4jClient` (returns default; customized to send `workspace/didChangeWorkspaceFolders`)
6. Add `companion object { fun create(project: Project): XsLspServerDescriptor }` helper that reads `XsAppSettings.getInstance().state.gamePath`

**Reference**: design.md §"XsLspServerDescriptor.kt" for shape; verify exact API signatures in T1.

**Verification**: file compiles; `createCommandLine()` returns the expected `GeneralCommandLine`.

- [x] T4: Created `XsLspServerDescriptor.kt`. Deviation: the 2024.2 API's `ProjectWideLspServerDescriptor` does not accept workspace-folder varargs, so the descriptor extends base `LspServerDescriptor` (which does) and keeps project-wide semantics via the provider/`LspServerManager` deduplication. `createLsp4jClient()` was omitted; workspace-folder sync is handled by `XsLspServerManager` using `LspServer.sendNotification`.

### T5. Create `XsLspSupportProvider.kt`

**Files**: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspSupportProvider.kt` (new)

**Steps**:
1. Create the file with `package com.aomr.xs.lsp`
2. Implement `class XsLspSupportProvider : LspServerSupportProvider`
3. Override `fileOpened(project, file, serverStarter)`:
   - Skip if `file.extension != "xs"`
   - Read `XsAppSettings.getInstance().state.gamePath`; skip if blank
   - Call `serverStarter.ensureServerStarted(XsLspServerDescriptor.create(project))`
4. Override `createLspServerWidgetItem(lspServer, currentFile)` to return a custom widget item (or null for default)

**Reference**: design.md §"XsLspSupportProvider.kt" for shape.

**Verification**: file compiles.

- [x] T5: Created `XsLspSupportProvider.kt` implementing `LspServerSupportProvider.fileOpened()`, gated on `.xs` extension and a non-blank game path, plus a null widget-item override.

## Phase 3: Rewrite existing classes (T6-T8)

### T6. Rewrite `XsLspServerManager.kt`

**Files**: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerManager.kt` (rewrite from 221 → ~50 lines)

**Steps**:
1. Keep the class signature `@Service(Service.Level.PROJECT) class XsLspServerManager(private val project: Project) : Disposable`
2. Remove `connection: XsLspConnection?` field (server lifecycle is managed by platform)
3. Remove `currentGamePath` field (no longer tracking; platform does it)
4. Keep `currentModPaths: List<String>` field
5. Keep `installEditorListeners()` removed (platform handles file events)
6. Keep `installGameFolderWatcher()` removed (platform handles file watching since 2023.3.2)
7. Keep `notifyWorkspaceFoldersChanged(modPaths: List<String>)` method:
   - Compute added/removed delta
   - Find running LSP server via `LspServerManager.getInstance(project).servers`
   - Send `workspace/didChangeWorkspaceFolders` via `server.lsp4jServer.notifyWorkspaceFoldersChanged(...)` (or fallback to `LspClientManager.stopAndRestartClientsIfNeeded(XsLspSupportProvider::class.java)`)
8. Keep `companion object { fun getInstance(project): XsLspServerManager = project.service() }`

**Reference**: design.md §"XsLspServerManager.kt — REWRITE" for shape.

**Verification**: file compiles; `notifyWorkspaceFoldersChanged()` correctly identifies added/removed paths.

- [x] T6: Rewrote `XsLspServerManager.kt` to ~55 lines. Platform now owns server lifecycle; the manager computes mod-folder deltas and sends `workspace/didChangeWorkspaceFolders` via `LspServer.sendNotification`. `updateSettings(gamePath, modPaths)` is retained (called by `XsConfigurable`) and restarts the server when the game folder changes.

### T7. Simplify `XsStartupActivity.kt`

**Files**: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/startup/XsStartupActivity.kt` (minor edit)

**Steps**:
1. Keep the first-run UX logic (game-path-missing notification, mod auto-detection)
2. Remove the final call to `XsLspServerManager.getInstance(project).updateSettings(gamePath, modPaths)`
3. After auto-detecting mods, call `XsLspServerManager.getInstance(project).notifyWorkspaceFoldersChanged(detected)` instead

**Reference**: design.md §"XsStartupActivity.kt — MINOR edit" for the new `runActivity` body.

**Verification**: file compiles; first-run UX still works (notifications, auto-detection).

- [x] T7: Edited `XsStartupActivity.kt` to remove the `updateSettings()` call and call `notifyWorkspaceFoldersChanged()` after auto-detecting mods.

### T8. Update `plugin.xml`

**Files**: `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`

**Steps**:
1. Add `<platform.lsp.serverSupportProvider implementation="com.aomr.xs.lsp.XsLspSupportProvider"/>` to the `<extensions>` block
2. Keep `<postStartupActivity implementation="com.aomr.xs.startup.XsStartupActivity"/>` (still needed for first-run UX)

**Reference**: design.md §"plugin.xml — EDIT" for the exact additions.

**Verification**: `./gradlew compileKotlin` succeeds; `plugin.xml` validates.

- [x] T8: Added `<depends>com.intellij.modules.lsp</depends>`, `<depends>com.intellij.modules.ultimate</depends>`, and `<platform.lsp.serverSupportProvider>` to `plugin.xml`; kept `<postStartupActivity>`.

## Phase 4: Delete and tests (T9-T13)

### T9. Delete `XsLanguageClient.kt`

**Files**: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLanguageClient.kt` (delete)

**Steps**:
1. Verify no other file references `XsLanguageClient`
2. Delete the file
3. Run `./gradlew compileKotlin` to verify no compile errors

**Verification**: file deleted; no compile errors.

- [x] T9: Deleted `XsLanguageClient.kt`; verified no remaining references and `./gradlew compileKotlin` green.

### T10. Delete `XsLspConnection.kt`

**Files**: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspConnection.kt` (delete)

**Steps**:
1. Verify no other file references `XsLspConnection` (grep for the class name)
2. Delete the file
3. Run `./gradlew compileKotlin` to verify no compile errors

**Verification**: file deleted; no compile errors.

- [x] T10: Deleted `XsLspConnection.kt`; verified no remaining references and `./gradlew compileKotlin` green.

### T11. Create `XsBinaryResolverTest.kt`

**Files**: `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsBinaryResolverTest.kt` (new)

**Steps**:
1. Create JUnit test class
2. Test each of the 5 binary-resolution strategies:
   - System property is honored
   - Env var is honored when sysprop is unset
   - Bundled binary is extracted and used when sysprop + env var are unset
   - Project-local binary is used when others are unset
   - PATH fallback works when nothing else is available
3. Test that the extracted binary is cached (second call returns the same path)
4. Test that the bundled binary is marked executable

**Reference**: `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsLspWorkspaceFolderUriTest.kt` for test style.

**Verification**: `./gradlew test` runs the new tests and they pass.

- [x] T11: Created `XsBinaryResolverTest.kt` covering system property, env var, bundled extraction/caching, project-local, PATH fallback, and precedence.

### T12. Create `XsLspSupportProviderTest.kt`

**Files**: `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsLspSupportProviderTest.kt` (new)

**Steps**:
1. Create JUnit test class
2. Test `fileOpened()` calls `ensureServerStarted()` for `.xs` files
3. Test `fileOpened()` skips non-`.xs` files
4. Test `fileOpened()` skips when game path is blank

**Verification**: `./gradlew test` runs the new tests and they pass.

- [x] T12: Created `XsLspSupportProviderTest.kt` verifying `.xs` files start the server, non-`.xs` files are skipped, and blank game paths are skipped.

### T13. Create `XsLspServerDescriptorTest.kt`

**Files**: `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsLspServerDescriptorTest.kt` (new)

**Steps**:
1. Create JUnit test class
2. Test `isSupportedFile()` returns true for `.xs` files and false otherwise
3. Test `createCommandLine()` returns the right binary path + `--game-path <gamePath>` arg

**Verification**: `./gradlew test` runs the new tests and they pass.

- [x] T13: Created `XsLspServerDescriptorTest.kt` verifying `isSupportedFile()` and `createCommandLine()` binary + args, plus stderr-not-merged.

## Post-implementation: Build and verify (manual)

After T1-T13 are complete:

### T14. Run all tests

**Steps**:
1. Run `./gradlew test` in `tools/intellij-xs-plugin/`
2. Verify all tests pass (existing 15 + new ~10 = ~25 tests)

- [x] T14: `./gradlew test -x buildSearchableOptions` passed with 46 tests green, 0 failures, 0 errors.

### T15. Build the plugin .zip

**Steps**:
1. Set `JAVA_HOME` to the JBR from Gradle caches: `JAVA_HOME=$(find ~/.gradle/caches -name "jbr" -type d | head -1)`
2. Run `./gradlew buildPlugin -x buildSearchableOptions`
3. Verify `dist/intellij-xs-plugin-0.1.5.zip` is created

- [x] T15: Build succeeded; `dist/intellij-xs-plugin-0.1.5.zip` created (~4 MB).

## Deviations from design

The following deviations from `design.md` were identified during implementation
and verification. Items 4 and 5 were remediated in the apply continuation that
added covering tests for `XsLspServerManager` and `XsStartupActivity`.

1. **Build target IC → IU 2024.2** — The cached IC 2024.2 distribution did not
   include the `com.intellij.modules.lsp` API classes. Switched to IU 2024.2
   and wired `platformType` from `gradle.properties`.
2. **`XsLspServerDescriptor` extends base `LspServerDescriptor`** — The 2024.2
   `ProjectWideLspServerDescriptor` constructor does not accept initial
   workspace-folder varargs, so the base class is used. Project-wide semantics
   are still enforced by the provider and `LspServerManager`.
3. **Stderr redirect via `startServerProcess()` override** — The platform's
   default process handler merges stderr into stdout, which would break the
   LSP JSON-RPC stream. The override redirects stderr to
   `<project>/.idea/xs-lsp.log` using `ProcessBuilder.Redirect.appendTo()`.
4. **Settings listener not wired to `XsSettings`** *(remediated)* — The initial
   implementation relied on `XsConfigurable.apply()` calling
   `XsLspServerManager.updateSettings()`. A `XsSettings.Listener` was added so
   that any `setModPaths()` call (including auto-detection) triggers workspace
   folder sync.
5. **No server-restart fallback on `didChangeWorkspaceFolders` send failure**
   *(remediated)* — The initial implementation only caught and logged send
   exceptions. `notifyWorkspaceFoldersChanged()` now falls back to
   `LspServerManager.stopAndRestartIfNeeded()` when no server is running or
   a send throws.

### T16. Smoke test in Rider

**Steps**:
1. Install `dist/intellij-xs-plugin-0.1.5.zip` in Rider 2026.2 via Settings → Plugins → ⚙ → Install Plugin from Disk
2. Restart Rider
3. Open `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`
4. Verify:
   - Syntax highlighting works (via TextMate)
   - **NEW**: Diagnostics appear inline + in Problems window (Bug #1, #3 fixed)
   - **NEW**: Ctrl+Space shows completions (Bug #2 fixed)
   - **NEW**: Hover shows documentation
   - **NEW**: Ctrl+B navigates to include file
   - `~/.local/state/aomr_lsp/v2/` cache is used
   - LSP log shows expected activity in `<project>/.idea/xs-lsp.log`

### T17. Commit and push

**Steps**:
1. Stage only the intended files (per AGENTS.md pre-commit hygiene: never `git add -A`)
2. Verify `git status` shows no strays from `mod/` or `openspec/specs/`
3. Commit with message:
   ```
   fix(intellij-xs-plugin): migrate to IntelliJ Platform's built-in LSP API
   
   - Replace ~603 lines of hand-rolled LSP4J plumbing with
     `com.intellij.platform.lsp.api.*`
   - Add `<depends>com.intellij.modules.lsp</depends>` and
     `<depends>com.intellij.modules.ultimate</depends>`
   - New classes: XsLspSupportProvider, XsLspServerDescriptor,
     XsBinaryResolver
   - Fix 3 known bugs: diagnostics now surface in editor + Problems
     window, completion now registered, diagnostic integration wired
   - Bump plugin version 0.1.4 → 0.1.5
   
   Bug fixes:
   - #1: XsLanguageClient.publishDiagnostics() only logged diagnostics
   - #2: no CompletionContributor registered
   - #3: no diagnostic integration
   
   Gained for free: semantic highlighting, inlay hints, folding,
   breadcrumbs, signature help, call/type hierarchy, rename refactoring,
   range formatting, code lens.
   ```
4. Push the branch `xs-language-server/true-include-paste-tests`

### T18. Update AGENTS.md

**Steps**:
1. Note in AGENTS.md: "Plugin 0.1.5 migrates to IntelliJ Platform's built-in LSP API. The hand-rolled LSP4J plumbing (`XsLspConnection`, `XsLspServerManager`, `XsLanguageClient`) is replaced by `XsLspSupportProvider` + `XsLspServerDescriptor` + `XsBinaryResolver`."

### T19. Update engram

**Steps**:
1. Save session summary via `mem_session_summary` with the migration results

## Task Dependencies

```
T1 (verify API surface)
 ↓
T2 (add plugin.xml deps)
 ↓
T3 → T4 → T5  (new classes)
 ↓
T6 → T7 → T8  (rewrites)
 ↓
T9 → T10     (deletes)
 ↓
T11 → T12 → T13  (tests)
 ↓
T14 → T15 → T16 → T17 → T18 → T19  (build + verify + commit)
```

## Risk per task

| Task | Risk | Mitigation |
|------|------|------------|
| T1 | Platform API surface may differ from docs | Verify in T1, adjust design |
| T2 | Plugin fails to load without deps | Build catches this |
| T3-T5 | New code may have bugs | Unit tests catch them |
| T6-T8 | Rewrites may break existing behavior | Manual smoke test in T16 |
| T9-T10 | Compile errors from stale references | `./gradlew compileKotlin` catches |
| T11-T13 | Tests may be flaky | Run multiple times |
| T14-T16 | Build/test/smoke test may fail | Iterate until green |
| T17 | Pre-commit hygiene may fail | Use explicit paths, not `git add -A` |

## Manual verification steps

T16 is the primary manual verification. Detailed checklist:

1. ✅ Syntax highlighting (TextMate) — unchanged
2. ✅ Inline diagnostics appear as red/yellow underlines
3. ✅ Problems window lists errors/warnings for the file
4. ✅ Ctrl+Space shows completion popup with at least one completion (e.g., `aiTaskMoveUnit`, `kb*` function names)
5. ✅ Hover over a symbol shows documentation
6. ✅ Ctrl+B on a function call navigates to the definition (if same file or via include)
7. ✅ LSP log shows workspace folders received, didOpen events, semantic projects built
8. ✅ `idea.log` shows plugin loaded without errors
9. ✅ No regression: existing IntelliJ features still work (Find Usages via Alt+F7 if PSI supports it, etc.)
10. ✅ Stub PSI still throws `UnsupportedOperationException` on direct calls, but this doesn't affect LSP features

## Rollback plan

If the migration breaks the plugin in a way that can't be fixed quickly:

1. `git revert <migration-commit>` — single revert undoes everything
2. Rebuild with `./gradlew buildPlugin`
3. Reinstall `dist/intellij-xs-plugin-0.1.4.zip` in Rider
4. The LSP server is unchanged; user can continue with plugin 0.1.4 (known limitations: no diagnostics, no completion)

## Time estimate

- T1-T2: 30 minutes (verify API + plugin.xml deps)
- T3-T5: 1 hour (new classes)
- T6-T8: 1 hour (rewrites)
- T9-T10: 15 minutes (deletes)
- T11-T13: 1.5 hours (tests)
- T14-T15: 30 minutes (build + verify)
- T16: 30 minutes (smoke test)
- T17-T19: 30 minutes (commit + docs)

**Total: ~5-6 hours** of focused work in one session.