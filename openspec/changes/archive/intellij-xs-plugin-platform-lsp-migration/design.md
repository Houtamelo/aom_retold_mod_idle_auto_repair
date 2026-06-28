# Design: LSP platform API migration

This design translates the proposal + specs into a concrete implementation plan. It captures architecture decisions, file-by-file deltas, and the migration sequence.

## Architecture decisions

### AD-1: Use the platform's `LspServerSupportProvider` (not `LspIntegrationProvider`)

**Decision**: Use `LspServerSupportProvider` (the older but stable API name) rather than `LspIntegrationProvider` (the newer name used by the Prisma plugin).

**Rationale**: While `LspIntegrationProvider` is what Prisma uses in the `idea/261.25134.95` branch, the official IntelliJ Platform docs (https://plugins.jetbrains.com/docs/intellij/language-server-protocol.html) document `LspServerSupportProvider` as the canonical name. Both names coexist in different IntelliJ Platform versions. `LspServerSupportProvider` is documented in the main docs page as the public API; `LspIntegrationProvider` is the newer name introduced in 2025.x.

**Trade-off**: If we use `LspServerSupportProvider` and the user upgrades to a future IntelliJ Platform version that deprecates it, we'd need to migrate. But the docs page is the authoritative source and uses `LspServerSupportProvider`.

### AD-2: Use `ProjectWideLspServerDescriptor` (one server per project)

**Decision**: Subclass `ProjectWideLspServerDescriptor` (one LSP server per IntelliJ project). This means one LSP server instance regardless of how many `.xs` files are open in the project.

**Rationale**: Matches the current architecture (one server per project) and is the standard pattern. Per-file descriptors would be wasteful for a project-level language like XS.

**Trade-off**: None — this is the canonical pattern.

### AD-3: Send workspace folder changes via `didChangeWorkspaceFolders` after init

**Decision**: When the mod list changes after the LSP server has started, send `workspace/didChangeWorkspaceFolders` directly (don't restart the server).

**Rationale**: Restarting the server on every settings change is heavy (re-extract binary, re-init, re-load cache, re-build semantic project). The current plugin already does `didChangeWorkspaceFolders` correctly; we should preserve that behavior.

**Implementation**: Override `LspServerDescriptor.createLsp4jClient()` to get the platform's `Lsp4jClient`, which exposes `getRemoteServer()` to access the LSP4J `LanguageServer` proxy. Send the notification via `server.workspaceService.didChangeWorkspaceFolders(params)`.

If `getRemoteServer()` doesn't exist or the timing is wrong (server not yet initialized), fall back to restarting the server via `LspClientManager.stopAndRestartClientsIfNeeded()`.

### AD-4: Extract binary lazily in descriptor constructor (cached)

**Decision**: Extract the bundled binary to a temp file in `XsLspServerDescriptor.init {}` block. Cache the extracted path in a class-level `@Volatile var`. Return the cached path from `createCommandLine()`.

**Rationale**: The current `XsLspConnection.extractBundledBinary()` already does this lazily with caching. The behavior is correct; just relocate the logic to a pure function (`XsBinaryResolver`) for testability.

**Trade-off**: The first project open takes a small performance hit (binary extraction ~50ms); subsequent opens in the same JVM are free.

### AD-5: No `LspCustomization` opt-outs

**Decision**: Return a default `LspCustomization` (or override nothing). All platform-managed LSP features are enabled.

**Rationale**: The XS plugin has no native PSI-based completion/hover/etc. to compete with the LSP. Default behavior is what we want.

**Reference**: Prisma plugin (`PrismaLspClientDescriptor.kt`) overrides ~14 `LspCustomization` properties to disable platform features it implements natively (`LspCompletionDisabled`, `LspHoverDisabled`, etc.). XS does not need any of these.

### AD-6: Keep `XsStartupActivity` but remove its LSP-startup call

**Decision**: `XsStartupActivity` stays as a `postStartupActivity`, but its `runActivity` method no longer calls `XsLspServerManager.updateSettings()`. Its job is reduced to:
- Show notification if game path missing
- Auto-detect mods if mod list empty
- Persist auto-detected mods to `XsSettings`

The platform's `XsLspSupportProvider.fileOpened()` handles server startup when the first `.xs` file is opened.

**Rationale**: The startup activity is still needed for the first-run UX (notifications, auto-detection). But it shouldn't drive the LSP server lifecycle — that's the platform's job.

### AD-7: Settings-watcher as per-project service

**Decision**: Create a new per-project service `XsSettingsWatcher` (in `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/XsSettingsWatcher.kt`) that:
- Listens for `XsSettings.state.modPaths` changes
- On change, computes the added/removed delta
- Sends `workspace/didChangeWorkspaceFolders` via the running LSP server
- Falls back to server restart if no running server

**Rationale**: This is the cleanest way to keep the current "mod list updates push to LSP" behavior using the platform API. The watcher is the only piece of stateful lifecycle code that needs to live outside the platform's auto-managed flow.

### AD-8: Single PR, single commit

**Decision**: Land all changes in one PR with one commit. Version bump 0.1.4 → 0.1.5 in the same commit.

**Rationale**: Intermediate states would be broken anyway (the old client only logs, so anything that depends on it is already broken). Single revert is easier than multi-PR revert.

### AD-9: Use JUnit tests for the new code (not strict TDD)

**Decision**: Add JUnit tests for the migrated code. The plugin has a working `./gradlew test` harness. Per `openspec/config.yaml`, the project does not enforce strict TDD; this change just happens to benefit from unit tests for the binary-resolution logic.

**Rationale**: `XsBinaryResolver` is a pure function with 5 distinct resolution strategies — exactly the kind of code that benefits from unit tests. Manual smoke testing covers the rest.

## File-by-file deltas

### 1. `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLanguageClient.kt` — DELETE

83 lines deleted entirely. The platform's `Lsp4jClient` replaces it.

### 2. `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspConnection.kt` — REWRITE → split into 3

#### `XsLspConnection.kt` — becomes a thin wrapper, ~30 lines

After migration, the only remaining use of `XsLspConnection` is as a façade over `LspClientManager`. Most of its logic moves to `XsBinaryResolver`. The `XsLspConnection` class either:
- (a) Gets deleted, with the binary-resolution logic moved to `XsLspServerDescriptor.createCommandLine()`
- (b) Gets kept as a thin wrapper around `XsBinaryResolver` for backwards-compat with any external callers (none in this codebase)

**Decision**: Option (a) — delete `XsLspConnection.kt` entirely. No external callers exist. The binary-resolution logic moves to `XsBinaryResolver.kt` and is called directly from `XsLspServerDescriptor.createCommandLine()`.

#### `XsBinaryResolver.kt` — new file, ~60 lines

Pure object that exposes:

```kotlin
object XsBinaryResolver {
    /**
     * Resolves the path to the xs-language-server binary using 5 strategies.
     * Returns the resolved path (always non-null; falls back to "xs-language-server" PATH lookup).
     */
    fun resolve(
        systemProperty: String? = System.getProperty("xs.lsp.path"),
        envVar: String? = System.getenv("XS_LSP_PATH"),
        projectBasePath: String? = null,
        resourceProvider: (String) -> InputStream? = XsBinaryResolver::getResource,
    ): String
    
    /**
     * Extracts the bundled binary from the plugin JAR to a temp file.
     * Caches the extracted path for subsequent calls in the same JVM.
     */
    private fun extractBundled(resourcePath: String = "/bin/xs-language-server"): String?
    
    /**
     * Checks if the project-local binary exists and is executable.
     */
    private fun findProjectBinary(projectBasePath: String?): String?
}
```

The `resourceProvider` parameter is a function reference for getting a resource stream. This makes the class unit-testable: tests pass a fake `resourceProvider` that returns a mock stream.

#### `XsLspServerDescriptor.kt` — new file, ~80 lines

```kotlin
package com.aomr.xs.lsp

import com.aomr.xs.settings.XsAppSettings
import com.intellij.execution.configurations.GeneralCommandLine
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.Lsp4jClient
import com.intellij.platform.lsp.api.ProjectWideLspServerDescriptor
import org.eclipse.lsp4j.WorkspaceFolder
import org.eclipse.lsp4j.services.LanguageServer
import java.io.File

class XsLspServerDescriptor(
    project: Project,
    private val gamePath: String,
) : ProjectWideLspServerDescriptor(project, "XS Language Server") {
    
    @Volatile
    private var extractedBinaryPath: String? = XsBinaryResolver.resolve(
        projectBasePath = project.basePath,
    )
    
    override fun isSupportedFile(file: VirtualFile): Boolean = file.extension == "xs"
    
    override fun createCommandLine(): GeneralCommandLine {
        val binaryPath = extractedBinaryPath ?: "xs-language-server"
        return GeneralCommandLine(binaryPath, "--game-path", gamePath)
            .withEnvironment("RUST_LOG", System.getenv("RUST_LOG") ?: "info")
            .withRedirectErrorStream(false)
    }
    
    /**
     * Override to access the LSP4J LanguageServer proxy for sending
     * workspace/didChangeWorkspaceFolders after init.
     */
    override fun createLsp4jClient(): Lsp4jClient {
        return object : Lsp4jClient(project) {
            override fun notifyWorkspaceFoldersChanged(
                added: List<WorkspaceFolder>,
                removed: List<WorkspaceFolder>,
            ) {
                // Custom hook for workspace folder sync
            }
        }
    }
    
    companion object {
        fun create(project: Project): XsLspServerDescriptor {
            val gamePath = XsAppSettings.getInstance().state.gamePath
            return XsLspServerDescriptor(project, gamePath)
        }
    }
}
```

(Actual implementation will be tuned to the real platform API surface — see "Risks" below.)

### 3. `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspSupportProvider.kt` — new file, ~30 lines

```kotlin
package com.aomr.xs.lsp

import com.aomr.xs.settings.XsAppSettings
import com.intellij.openapi.project.Project
import com.intellij.openapi.vfs.VirtualFile
import com.intellij.platform.lsp.api.LspServerSupportProvider
import com.intellij.platform.lsp.api.LspServerStarter

class XsLspSupportProvider : LspServerSupportProvider {
    override fun fileOpened(
        project: Project,
        file: VirtualFile,
        serverStarter: LspServerStarter,
    ) {
        if (file.extension != "xs") return
        val gamePath = XsAppSettings.getInstance().state.gamePath
        if (gamePath.isBlank()) return
        serverStarter.ensureServerStarted(XsLspServerDescriptor(project, gamePath))
    }
    
    override fun createLspServerWidgetItem(
        lspServer: Any /* LspServer */,
        currentFile: VirtualFile?,
    ) = null
}
```

### 4. `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerManager.kt` — REWRITE → much smaller

The class either:
- (a) Gets deleted entirely (replaced by `XsSettingsWatcher`)
- (b) Gets kept as a thin per-project service that exposes a `notifyWorkspaceFoldersChanged(modPaths: List<String>)` method

**Decision**: Option (b) — keep `XsLspServerManager` but reduce to ~50 lines. Its only responsibility is forwarding mod-path changes to the running LSP server. The server lifecycle is managed by `LspServerManager`.

```kotlin
package com.aomr.xs.lsp

import com.aomr.xs.settings.XsSettings
import com.intellij.openapi.Disposable
import com.intellij.openapi.components.Service
import com.intellij.openapi.components.service
import com.intellij.openapi.diagnostic.Logger
import com.intellij.openapi.project.Project

@Service(Service.Level.PROJECT)
class XsLspServerManager(private val project: Project) : Disposable {
    private val log = Logger.getInstance(XsLspServerManager::class.java)
    
    @Volatile
    private var currentModPaths: List<String> = emptyList()
    
    fun notifyWorkspaceFoldersChanged(newModPaths: List<String>) {
        val previous = currentModPaths.toSet()
        val next = newModPaths.toSet()
        val added = (next - previous).toList()
        val removed = (previous - next).toList()
        currentModPaths = newModPaths
        if (added.isEmpty() && removed.isEmpty()) return
        // Use LspServerManager to find the running server and send the notification
        // (or restart as fallback)
        sendWorkspaceFolderChange(added, removed)
    }
    
    private fun sendWorkspaceFolderChange(addedPaths: List<String>, removedPaths: List<String>) {
        // Implementation: find the running LSP server via LspServerManager.getInstance(project).servers
        // and call server.notifyWorkspaceFoldersChanged(addedFolders, removedFolders)
        // OR: LspClientManager.stopAndRestartClientsIfNeeded(XsLspSupportProvider::class.java)
    }
    
    override fun dispose() {}
    
    companion object {
        fun getInstance(project: Project): XsLspServerManager = project.service()
    }
}
```

### 5. `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/startup/XsStartupActivity.kt` — MINOR edit, ~75 lines

Remove the `XsLspServerManager.getInstance(project).updateSettings(...)` call. Keep everything else.

```kotlin
override fun runActivity(project: Project) {
    val appSettings = XsAppSettings.getInstance()
    val projectSettings = XsSettings.getInstance(project)

    if (appSettings.state.gamePath.isEmpty()) {
        notifyMissingGamePath(project)
        return
    }

    if (projectSettings.state.modPaths.isEmpty()) {
        val projectRoot = project.basePath?.let { LocalFileSystem.getInstance().findFileByPath(it) }
        if (projectRoot != null) {
            val detected = XsModAutoDetector.scan(projectRoot)
            if (detected.isEmpty()) {
                notifyNoModsDetected(project)
            } else {
                projectSettings.setModPaths(detected)
                // Notify the watcher (if the project is already initialized) — otherwise
                // the platform will pick up the new settings on first .xs file open.
                XsLspServerManager.getInstance(project).notifyWorkspaceFoldersChanged(detected)
            }
        } else {
            notifyNoModsDetected(project)
        }
    }
}
```

### 6. `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` — EDIT

**Add** (in `<depends>` block):
```xml
<depends>com.intellij.modules.lsp</depends>
<depends>com.intellij.modules.ultimate</depends>
```

**Replace** (in `<extensions>` block):
```xml
<postStartupActivity implementation="com.aomr.xs.startup.XsStartupActivity"/>
```
→ 
```xml
<platform.lsp.serverSupportProvider implementation="com.aomr.xs.lsp.XsLspSupportProvider"/>
<postStartupActivity implementation="com.aomr.xs.startup.XsStartupActivity"/>
```

(Both stay because the startup activity is still needed for first-run UX.)

### 7. `tools/intellij-xs-plugin/gradle.properties` — EDIT

```diff
- pluginVersion=0.1.4
+ pluginVersion=0.1.5
```

### 8. New test files

- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsBinaryResolverTest.kt` — JUnit tests for the 5 binary-resolution strategies
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsLspSupportProviderTest.kt` — file extension dispatch
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsLspServerDescriptorTest.kt` — `createCommandLine()` args + `isSupportedFile()`

## Migration sequence

Single PR, single commit. Sequence of operations within the commit:

1. Delete `XsLanguageClient.kt` (the broken client)
2. Add `XsBinaryResolver.kt` (testable pure function)
3. Add `XsLspServerDescriptor.kt` (thin descriptor wrapper)
4. Add `XsLspSupportProvider.kt` (registers the descriptor with the platform)
5. Rewrite `XsLspServerManager.kt` (thin per-project service for workspace folder sync)
6. Rewrite `XsStartupActivity.kt` (remove manager.updateSettings() call; add notifyWorkspaceFoldersChanged)
7. Edit `plugin.xml` (add deps; add serverSupportProvider; keep postStartupActivity)
8. Edit `gradle.properties` (bump version)
9. Add 3 new test files
10. Update existing test files if they reference deleted code
11. Run `./gradlew test` — all tests must pass
12. Run `./gradlew buildPlugin` — .zip must build successfully

After commit:
- Smoke test in Rider: open an `.xs` file, verify diagnostics, completion, hover, go-to-decl all work
- Check `.idea/xs-lsp.log` for expected LSP activity
- Check `idea.log` for plugin load errors

## State-machine diagrams

Not applicable — this migration does not introduce new state machines. The platform's `LspServerManager` handles server lifecycle as a state machine internally.

## Architecture decisions recorded

See `decisions` section in the explore.md and proposal.md for the full decision log. Key decisions:

1. Use `LspServerSupportProvider` (not `LspIntegrationProvider`)
2. Use `ProjectWideLspServerDescriptor` (one server per project)
3. Send workspace folder changes via `didChangeWorkspaceFolders` after init (not restart)
4. Extract binary lazily in descriptor constructor (cached)
5. No `LspCustomization` opt-outs (XS has no native PSI)
6. Keep `XsStartupActivity` but remove its LSP-startup call
7. Settings-watcher as per-project service (`XsLspServerManager`)
8. Single PR, single commit
9. JUnit tests for new code (not strict TDD)

## Files shared across mods

**None.** This migration is purely IntelliJ-plugin-side. No XS source files, no mod files, no LSP server changes. Per the project's AGENTS.md cross-mod isolation ADR, the plugin never reads/writes `mod/`.

## Patch maintenance notes

- Plugin 0.1.4 → 0.1.5 is a bug fix (per AGENTS.md plugin version bump policy).
- The migration removes ~600 lines of manual LSP4J plumbing. Future plugin updates are easier because the platform handles LSP lifecycle.
- No regression risk for users with existing 0.1.x installs: the LSP server contract is unchanged; only the client-side wiring changes.

## Risks (recap)

1. **Workspace folder sync hook may not exist** in 2026.2 platform API. Mitigation: fallback to server restart via `LspClientManager.stopAndRestartClientsIfNeeded()`. Both approaches preserve correctness.
2. **Single-PR irreversible mid-flight**. Mitigation: single revert.
3. **Plugin version 0.1.5 needs Rider restart for users upgrading from 0.1.4**. Mitigation: document in the install instructions.
4. **Stub PSI throws `UnsupportedOperationException`**. Mitigation: cosmetic; doesn't affect LSP. Follow-up fix.

## Open questions for the implementer

The exact method signatures of `LspServerDescriptor.createLsp4jClient()` and `LspServerSupportProvider.fileOpened()` need to be verified against the actual 2026.2 platform API. The docs page shows the pattern but may not show all overloads. The implementer should:

1. Read the platform's LSP API source (since 2024.2, attached via `IntelliJ IDEA sources` artifact per docs)
2. Look at Prisma's actual implementation (uses `JSNodeLspClientDescriptor`, slightly different)
3. Adjust the design as needed

The architectural shape (provider + descriptor + binary resolver + settings watcher) is locked; the specific API calls may need adjustment.