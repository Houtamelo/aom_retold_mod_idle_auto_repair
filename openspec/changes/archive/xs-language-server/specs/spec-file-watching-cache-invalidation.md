# File Watching and Cache Invalidation Specification

## Capability summary
The LSP server SHALL register a `workspace/didChangeWatchedFiles` watcher for `<game-path>/game/**/*.xs` and maintain per-file parse caches under `~/.local/state/aomr_lsp/game_parse/v1/<mtime>-<sha256>.json`. Changed files invalidate their cached parse results and per-file symbol tables; the server performs no filesystem polling.

## Rationale
Mod files depend on vanilla files. If a user edits a vanilla file, dependent mod diagnostics must refresh. Client-side file watching lets the server react to changes without polling or re-parsing the entire game folder on every keystroke.

## Scenarios

### Scenario: happy path — watcher registration
- GIVEN a client that supports dynamic watcher registration
- WHEN the server initializes
- THEN it registers a watcher for `<game-path>/game/**/*.xs`

### Scenario: happy path — file change invalidates cache
- GIVEN a vanilla file `game/ai/core/core.xs` is cached
- WHEN the client notifies the server that the file changed
- THEN the server discards the cached parse result and recomputes diagnostics for open mod files that depend on it

### Scenario: edge case — content hash unchanged
- GIVEN a file receives a change notification but its mtime and SHA-256 are unchanged
- WHEN the server processes the notification
- THEN it keeps the existing cached parse result

### Scenario: negative case — unsupported client
- GIVEN a client that does not support dynamic watcher registration
- WHEN the server initializes
- THEN it does not register a watcher
- AND it degrades gracefully (game-folder diagnostics may be stale until a file is reopened or the server restarts)

### Scenario: edge case — closed mod file dependency changes
- GIVEN a mod file is closed but was previously diagnosed against a cached vanilla parse
- AND the vanilla file changes
- WHEN the server receives the change notification
- THEN it invalidates the cached vanilla parse and symbol table only
- AND it does not re-diagnose closed files until they are reopened

## XS-engine constraints
- Files under `game/` may be updated by game patches outside the IDE. The server cannot detect these without client notifications.
- Parse results depend on the state of included files; invalidating a parse cache must propagate to files that include it.

## Out of scope
- Server-initiated polling of the game folder.
- Real-time re-diagnosis of all closed files; only open files and their dependencies are re-diagnosed promptly.
- Watching files outside the registered game path.

## Verification approach
- Automated: LSP integration test simulates a watched-file change notification and asserts that dependent diagnostics are updated.
- Manual: edit a vanilla file in a separate editor while a mod file is open in IntelliJ, and confirm diagnostics refresh.

## Acceptance criteria
1. The server SHALL register a `workspace/didChangeWatchedFiles` watcher for `<game-path>/game/**/*.xs`.
2. The server SHALL cache per-file parse results at `~/.local/state/aomr_lsp/game_parse/v1/<mtime>-<sha256>.json`.
3. The server SHALL invalidate a file's cached parse result when it receives a `changed` or `deleted` notification for that file.
4. The server SHALL maintain a per-file symbol table cache for cross-file resolution and invalidate it alongside the parse cache.
5. The server SHALL NOT poll the filesystem from the server process.
6. The server SHALL treat identical mtime+hash as unchanged and skip re-parsing.
7. Change notifications SHALL propagate invalidation to files that include the changed file.
8. The server SHALL degrade gracefully when the client does not support dynamic watcher registration.
