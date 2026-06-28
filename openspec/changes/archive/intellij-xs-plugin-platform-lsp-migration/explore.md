# Exploration: LSP platform API migration for intellij-xs-plugin

## Current State

The `intellij-xs-plugin` bridges a custom Rust LSP server (`tools/xs-language-server/`) to Rider 2026.2 using hand-rolled LSP4J plumbing. The LSP server runs correctly (verified via `<project>/.idea/xs-lsp.log` — workspace folders are received, `did_open` events flow, semantic projects build). However, the IDE never surfaces what the LSP returns, so the plugin "does nothing" in the editor.

### Architecture today

```
┌─────────────────────────────────────────────────────────────────┐
│ IntelliJ Platform (Rider 2026.2)                                │
│                                                                 │
│  FileEditorManagerListener ──┐                                  │
│  DocumentListener ──────────┼──▶ XsLspServerManager             │
│  VirtualFileListener ───────┘    │                              │
│                                  ├──▶ XsLspConnection           │
│  XsStartupActivity ──────────────┘    │                         │
│                                       ├──▶ LSPLauncher          │
│                                       ├──▶ XsLanguageClient     │
│                                       │     (logs only)         │
│                                       └──▶ process              │
└─────────────────────────────────────────────────────────────────┘
                                                                │
                                                                ▼
                                                    ┌──────────────────────┐
                                                    │ xs-language-server   │
                                                    │ (Rust, stdio)        │
                                                    └──────────────────────┘
```

### Files involved

- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspConnection.kt` — **299 lines**. Spawns the Rust binary, performs the LSP initialize handshake, exposes server methods. Resolves the binary via 5 strategies (sysprop, env, bundled, project-local, PATH). Extracts the bundled binary from `bin/xs-language-server` to a temp file on first use.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerManager.kt` — **221 lines**. Per-project service. Installs `FileEditorManagerListener` and `DocumentListener` for `didOpen`/`didChange`/`didClose`. Installs a `VirtualFileListener` on the game folder to forward file-change events to `workspace/didChangeWatchedFiles`. Manages workspace folder sync via `XsLspConnection.changeWorkspaceFolders()`.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLanguageClient.kt` — **83 lines**. **`publishDiagnostics()` only logs diagnostics** (lines 30-48). Never surfaces them in the editor or Problems window. `showMessage` is forwarded to notification balloons correctly.
- `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` — **36 lines**. Registers FileType, TextMate bundle, stub PSI factory, brace matcher, quote handler, commenter, settings, and `postStartupActivity`. **Missing**: `<lang.completion.contributor language="XS" .../>`, `<externalAnnotator language="XS" .../>`, `ProblemRequestor`. **Missing**: `<depends>com.intellij.modules.lsp</depends>`.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/XsAppSettings.kt` — **53 lines**. APP-level persistent state: `gamePath` (AoM:R install root). Stored at `~/.config/JetBrains/<IDE>/options/xsLspApp.xml`. KEEP AS-IS.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/XsSettings.kt` — **54 lines**. PROJECT-level persistent state: `modPaths` (list of mod roots). Stored at `.idea/xsLsp.xml`. KEEP AS-IS.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/startup/XsStartupActivity.kt` — **89 lines**. First-run UX. Shows notification if game path is missing. Auto-detects mods via `XsModAutoDetector`. Calls `XsLspServerManager.updateSettings()`. NEEDS MINOR REVISION — the post-startup manager call goes away (platform manages server start).
- `tools/intellij-xs-plugin/build.gradle.kts` — Build config with `project.buildDir = file("../../dist/.build")`, `copyDistributionToDist` task, `stageLspServer` task (renamed from `copyLspServerToResources`). KEEP AS-IS.
- `tools/intellij-xs-plugin/gradle.properties` — `pluginVersion=0.1.4`, `pluginSinceBuild=242`, `pluginUntilBuild=263.*`. **Will need bump to 0.1.5** (bug fix per AGENTS.md policy).

### Three confirmed bugs (from previous session's diagnosis)

1. **`XsLanguageClient.publishDiagnostics()` only logs** (`XsLanguageClient.kt:30-48`). LSP diagnostics never appear in the editor or Problems window.
2. **No `CompletionContributor` registered** (`plugin.xml` lacks `<lang.completion.contributor language="XS" .../>`). Ctrl+Space does nothing.
3. **No diagnostic integration** (`plugin.xml` lacks `<externalAnnotator .../>` and `ProblemRequestor` integration). Even if diagnostics reached the client, there is no path from `publishDiagnostics()` to inline highlighting.

### Why the bugs exist

The plugin author wrote a `LanguageClient` that just logs server notifications, treating "LSP integration" as "JSON-RPC plumbing". They forgot that the IDE needs the data surfaced via IntelliJ's own extension points (`CompletionContributor`, `ExternalAnnotator`, etc.). This is the canonical LSP4J-from-scratch mistake.

## Affected Areas

| File                                                                                | Lines now | Action     | Why                                                                                                                            |
| ----------------------------------------------------------------------------------- | --------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `src/main/kotlin/com/aomr/xs/lsp/XsLanguageClient.kt`                                | 83        | **DELETE** | Replaced by platform's `Lsp4jClient` (diagnostics auto-surface).                                                              |
| `src/main/kotlin/com/aomr/xs/lsp/XsLspConnection.kt`                                 | 299       | **REWRITE** | Most replaced by `ProjectWideLspServerDescriptor`. Keep binary-resolution + extraction logic (move into descriptor helper).    |
| `src/main/kotlin/com/aomr/xs/lsp/XsLspServerManager.kt`                              | 221       | **REWRITE** | Most replaced by platform's auto-binding `LspServerSupportProvider.fileOpened()`. Keep settings-watcher + workspace-folder sync logic only. |
| `src/main/kotlin/com/aomr/xs/startup/XsStartupActivity.kt`                           | 89        | **MINOR**  | Remove `XsLspServerManager.updateSettings()` call — the platform starts the server on first `.xs` file open. Keep the first-run UX (game-path-missing notification, mod auto-detection, settings save). |
| `src/main/resources/META-INF/plugin.xml`                                             | 36        | **EDIT**   | Add `<depends>com.intellij.modules.lsp</depends>` and `<depends>com.intellij.modules.ultimate</depends>`. Replace `<postStartupActivity>` with `LspServerSupportProvider` extension. |
| `src/main/kotlin/com/aomr/xs/lsp/XsLspSupportProvider.kt`                            | (new)     | **CREATE** | ~30 lines. Subclass `LspServerSupportProvider`. `fileOpened()` calls `ensureServerStarted(XsLspServerDescriptor)`.            |
| `src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt`                           | (new)     | **CREATE** | ~80 lines. Subclass `ProjectWideLspServerDescriptor`. Override `isSupportedFile()`, `createCommandLine()` (uses binary-resolution helper), and a workspace-folder sync hook. |
| `src/main/kotlin/com/aomr/xs/lsp/XsBinaryResolver.kt`                                | (new)     | **CREATE** | ~60 lines. The 5-strategy binary-resolution logic from `XsLspConnection.resolveBinaryPath()` + `extractBundledBinary()` + `findProjectBinary()`. Pure function; easy to unit-test. |
| `src/test/kotlin/com/aomr/xs/lsp/XsBinaryResolverTest.kt`                            | (new)     | **CREATE** | Unit tests for the binary-resolution strategies (5 strategies; mock each environment). |
| `gradle.properties`                                                                  | 1 line    | **EDIT**   | `pluginVersion=0.1.4 → 0.1.5` (bug fix per AGENTS.md policy).                                                                |

**Net change**: approximately **−250 lines** of code (600 deleted + 350 new), plus +2 new tests, plus 2 plugin.xml deps, plus 1 version bump.

## Approaches

### 1. **Migrate to `com.intellij.platform.lsp.api.*`** (RECOMMENDED)

**Pros**:
- Fixes all 3 bugs at once (diagnostics surface via `Lsp4jClient`, completion via platform-managed `CompletionContributor`, etc.)
- Eliminates ~600 lines of manual LSP4J plumbing (XsLanguageClient.kt + most of XsLspConnection.kt + most of XsLspServerManager.kt)
- Gets features for free that aren't even listed in the bugs (semantic highlighting, inlay hints, folding, breadcrumbs, signature help, call/type hierarchy, rename refactoring, range formatting, code lens)
- Cleaner architecture; no custom JSON-RPC plumbing to maintain
- Prisma ORM plugin (JetBrains-owned, Apache 2.0, in `JetBrains/intellij-plugins/prisma/`) is the canonical reference — ~10 KB of Kotlin using the same API
- For XS: no native PSI-based completion/hover/etc., so unlike Prisma we need NO `LspCustomization` opt-outs — we get the full LSP-driven behavior

**Cons**:
- Requires `<depends>com.intellij.modules.lsp</depends>` and `<depends>com.intellij.modules.ultimate</depends>` — plugin won't run in IDEA Community or Android Studio. The current plugin also requires commercial IDEs (TextMate + custom PSI), so no regression here.
- Workspace folder sync (mod list) needs custom handling — see "Open design questions" below
- Migration touches 5 files (3 deletes/rewrites + 3 creates + 2 edits). Well-scoped but non-trivial.
- One-time risk: any subtle behavior gap in the platform API vs the hand-rolled client

**Effort**: Medium (4-6 hours of focused work, 1 PR).

### 2. **Fix the 3 bugs in place**

**Pros**:
- Smaller change (one file mostly — XsLanguageClient.kt)
- No architecture change
- Preserves current file structure

**Cons**:
- Still need to add `CompletionContributor`, `ExternalAnnotator`/`ProblemRequestor` manually — these are non-trivial IntelliJ extension points to get right
- Still need to wire diagnostics to editor — the XsLanguageClient must call `DaemonCodeAnalyzer.getInstance(project).restart()` and the Problems window's `ExternalAnnotator` interface
- Leaves ~600 lines of plumbing that does the same thing the platform does
- Misses free features: semantic highlighting, inlay hints, folding, breadcrumbs, signature help, call/type hierarchy, rename, etc.
- Doesn't fix the underlying architecture problem — every future feature needs hand-rolled integration
- The 3 bugs are symptoms of the same root cause: bypassing the platform's intended LSP integration pattern

**Effort**: Medium-High (more code, less idiomatic, leaves debt).

### 3. **Switch to the user's already-installed `lsp4ij`** (NOT recommended)

The user has `com.redhat.devtools.lsp4ij` 0.20.1 installed (EPL 2.0, 327 stars, 70 contributors). It's a generic LSP client that handles all of this for free.

**Pros**:
- Zero plugin code change
- Battle-tested LSP integration
- User already trusts it

**Cons**:
- Generic LSP client — not a single-language plugin (the user wants a first-class XS experience)
- Loses the bundled-binary auto-extract convenience (lsp4ij requires the user to point it at a binary path)
- Adds a dependency on a third-party plugin (currently installed, but distribution risk if user moves to a fresh machine)
- The user's custom plugin is the project's stated deliverable per the AGENTS.md
- 2.5 years old (fails the 5-year criterion for "battle-tested")

**Effort**: Low (no plugin code) but high risk (loses bundled binary, requires user setup).

## Recommendation

**Approach 1: Migrate to `com.intellij.platform.lsp.api.*`.**

The platform API exists since 2024.2 specifically to make this kind of migration trivial. The user is on Rider 2026.2 = all features available. The XS plugin has no native PSI, so we get everything from the LSP for free (no `LspCustomization` opt-outs needed). The Prisma plugin is a thin 10 KB reference using the same API. Net code reduction of ~250 lines. All 3 known bugs fixed automatically. Several more bugs we don't even know about yet (e.g., semantic highlighting not working) fixed for free.

## Open Design Questions

### Q1. Workspace folder sync (mod list)

After the platform manages the LSP server lifecycle (started on first `.xs` file open), how does the plugin push the user's configured mod list (`XsSettings.modPaths`) to the server as workspace folders?

**Resolution**: Subscribe to `XsSettings` state changes from a custom `LspServerCustomization.workspaceFoldersUpdateHook` (likely exists in 2026.2). When mod list changes, find the running server via `LspServerManager.getInstance(project).servers` and call `server.lspClient.sendNotification("workspace/didChangeWorkspaceFolders", params)`. Alternative: implement an `LspServerStateListener` that fires when the server is ready, and re-push the current mod list.

Fallback if no platform hook exists: Implement a small per-project service that listens to `XsSettings` and restarts the LSP server (via `LspClientManager.stopAndRestartClientsIfNeeded()`) when mod paths change. Simpler, slightly more disruptive.

### Q2. Game folder as init option

The LSP server requires the game folder path at startup. Today this is passed as `--game-path <path>` CLI arg.

**Resolution**: Keep it as a CLI arg in `createCommandLine()`. The descriptor is constructed with the game folder in its constructor (resolved from `XsAppSettings.gamePath`). If the game path changes after server start, restart the server via the same `stopAndRestartClientsIfNeeded()` mechanism.

Alternative: pass via `LspServerDescriptor.getWorkspaceConfiguration()` — Prisma uses this for `prisma.enableDiagnostics`. But the LSP server expects the game folder before `initialize` completes (it caches the engine API at startup), so CLI arg is more reliable.

### Q3. Bundled binary extraction

Today the binary is extracted from `src/main/resources/bin/xs-language-server` to a temp file on first use. The platform API takes a `GeneralCommandLine` directly.

**Resolution**: Extract once when the descriptor is constructed (in `init {}` block), store the extracted path in a field, return it from `createCommandLine()`. The extracted temp file is deleted on JVM exit. This logic lives in `XsBinaryResolver.kt` as a pure function — easy to unit-test.

### Q4. File watching (workspace/didChangeWatchedFiles)

Today the plugin installs a `VirtualFileListener` on the game folder and forwards `contentsChanged`/`fileCreated`/`fileDeleted` events to `workspace/didChangeWatchedFiles`. The LSP server already registers this capability dynamically.

**Resolution**: The platform handles this automatically via `LspServerDescriptor`. Per the docs (since 2023.3.2): "Client-side file watcher" — the platform registers file watchers and forwards changes to the LSP. **Delete `installGameFolderWatcher()` and `removeGameFolderWatcher()` entirely.**

### Q5. Migration sequencing

Two options:
- **One PR** (delete old + add new in a single commit): ~600 lines deleted, ~350 lines added. Diff is large but coherent. Easy to review as a single change. Easy to revert if it breaks.
- **Two PRs** (add new alongside old; switch plugin.xml to register the new descriptor; delete old): Each PR smaller. Risk of two broken states if user installs mid-migration.

**Resolution**: One PR. The migration is self-contained; intermediate states would be broken anyway (the old client only logs, so anything that depends on it is already broken). One PR is easier to review and revert. Bundle version bump in same commit.

### Q6. plugin.xml deps and extensions

**Add**:
- `<depends>com.intellij.modules.lsp</depends>`
- `<depends>com.intellij.modules.ultimate</depends>`

**Replace**:
- `<postStartupActivity implementation="com.aomr.xs.startup.XsStartupActivity"/>` → `<platform.lsp.serverSupportProvider implementation="com.aomr.xs.lsp.XsLspSupportProvider"/>` (or `com.intellij.platform.lsp.serverSupportProvider` per docs)

The `XsStartupActivity` itself remains but is simplified — it no longer calls `XsLspServerManager.updateSettings()`. Its job is reduced to: (a) notify if game path missing, (b) auto-detect mods, (c) save settings. The platform takes over from there.

**Keep**:
- `<fileTypeFactory>` (XsFileTypeFactory)
- `<textmate.bundleProvider>` (XsTextMateBundleProvider)
- `<lang.ast.factory>`, `<lang.parserDefinition>`, `<lang.braceMatcher>`, `<lang.quoteHandler>`, `<lang.commenter>` — all the stub PSI extensions stay (they're harmless; the platform's LSP will provide real completion/hover/etc.)
- `<notificationGroup>`
- `<projectConfigurable>` (XsConfigurable)

**Note on the stub PSI**: `XsParserDefinition.createElement()` currently throws `UnsupportedOperationException("Full XS PSI factory is not implemented until P2")`. This may have been interfering with TextMate-based syntax highlighting by claiming to handle XS while throwing on every call. With the platform API in place, this becomes a non-issue — the platform's LSP integration doesn't depend on the PSI working.

### Q7. Test coverage

**Add**:
- `src/test/kotlin/com/aomr/xs/lsp/XsBinaryResolverTest.kt` — JUnit tests for the 5 binary-resolution strategies. Each test sets/unsets env vars + system properties + temp files. This is the most important test to add — the binary resolution has the most failure modes.
- `src/test/kotlin/com/aomr/xs/lsp/XsLspSupportProviderTest.kt` — Light test that `XsLspSupportProvider.fileOpened()` calls `ensureServerStarted()` when given an `.xs` file and skips when given a non-`.xs` file. Uses `LspServerStarter` mock.
- `src/test/kotlin/com/aomr/xs/lsp/XsLspServerDescriptorTest.kt` — Test that `createCommandLine()` returns the right binary path + args. Test that `isSupportedFile()` returns true for `.xs` files.

**Note**: `openspec/config.yaml` says `tdd: false`, `test_command: ""`. This applies to the XS mod source code (no automated test runner). The IntelliJ plugin DOES have a test harness (`./gradlew test`, JUnit). Tests are possible and recommended for the migration.

## Risks

1. **Plugin fails to load if `<depends>com.intellij.modules.lsp</depends>` is missing or unavailable**. Mitigation: the dep is required for the platform API; without it the plugin won't compile. Build will catch this immediately.

2. **Workspace folder sync doesn't have a documented platform API hook in 2026.2**. Mitigation: research confirms fallback (restart server on settings change) works fine; the mod list rarely changes during a session.

3. **Custom `LspServerSupportProvider` extensions may have edge cases not covered by the Prisma reference**. Mitigation: Prisma is JetBrains' own plugin using the same API; if there are edge cases, they're documented in IntelliJ Platform source.

4. **Stub PSI still throws `UnsupportedOperationException`** — even with platform API, this might cause issues at plugin load time if any extension point calls `createElement()`. Mitigation: keep the throw but add a TODO; consider making it return a no-op AST node instead (cheap fix).

5. **The migration is irreversible mid-flight**. If the new descriptor has a bug, the plugin is broken until the bug is fixed. Mitigation: single PR means single revert. The old code is in git history.

6. **Rider 2026.2 may have introduced LSP API breaking changes since 2024.2**. Mitigation: Rider 2026.2 = the same year as plugin development. No version drift risk.

## Ready for Proposal

**Yes.** Recommend launching `sdd-propose` with the migration direction locked in. The change is well-scoped: ~250 lines net reduction, 1 PR, 3 new files, 5 modified files, all 3 known bugs fixed, plus several features gained for free.

## References

- Official docs: https://plugins.jetbrains.com/docs/intellij/language-server-protocol.html
- Prisma ORM plugin (canonical reference, Apache 2.0):
  - https://github.com/JetBrains/intellij-plugins/blob/master/prisma/src/org/intellij/prisma/ide/lsp/PrismaLspClientDescriptor.kt
  - https://github.com/JetBrains/intellij-plugins/blob/master/prisma/src/org/intellij/prisma/ide/lsp/PrismaLspIntegrationProvider.kt
  - https://github.com/JetBrains/intellij-plugins/blob/master/prisma/src/org/intellij/prisma/ide/lsp/PrismaLspServerActivationRule.kt
- Existing project observations: Engram #1332 (platform API research), #1334 (3 bugs + migration path)
- Previous plugin design: `openspec/changes/intellij-xs-plugin/` (Kotlin plugin P0-P1, completed)