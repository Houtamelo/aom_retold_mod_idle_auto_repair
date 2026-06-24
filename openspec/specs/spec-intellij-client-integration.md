# IntelliJ Client Integration Specification

> **Added/updated by change:** `xs-language-server` (workspace & engine-data redesign)  
> **Archived:** 2026-06-24  
> **Change verdict:** PASS WITH DEVIATIONS

## Capability summary
The IntelliJ plugin SHALL act as a thin LSP client: it SHALL provide a settings page for the game folder and mod list, auto-detect mod roots by scanning for `game/` directories, start the Rust LSP server with `--game-path`, send `workspace/didChangeWorkspaceFolders` notifications, register `didChangeWatchedFiles` watchers for `<game-folder>/game/**/*.xs`, and vend the TextMate grammar already present at `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`.

## Rationale
A thin client lets the plugin offload language semantics to the Rust server while keeping the editor integration (settings, file watching, lifecycle) where the IntelliJ Platform APIs are strongest.

## Scenarios

### Scenario: happy path — auto-detect mods on first run
- GIVEN the mod list is empty
- AND the project contains `mod/idle_auto_repair/game/` and `mod/intelligent_auto_scout/game/`
- WHEN the user clicks Auto-detect
- THEN the plugin adds `mod/idle_auto_repair/` and `mod/intelligent_auto_scout/` to the mod list

### Scenario: edge case — auto-detect stops at game/ boundaries
- GIVEN a nested directory `mod/parent/game/sub/game/`
- WHEN auto-detect runs
- THEN it adds `mod/parent/` only
- AND it does not add `mod/parent/game/sub/` as a mod root

### Scenario: happy path — LSP startup
- GIVEN a valid game folder and mod list are configured
- WHEN the IDE starts
- THEN `StartupActivity.DumbAware` launches the Rust LSP binary with `--game-path <game-folder>`
- AND it sends `workspace/didChangeWorkspaceFolders` with the detected mod URIs

### Scenario: settings change — mod list changes
- GIVEN the user adds or removes a mod folder
- WHEN the change is applied
- THEN the plugin re-sends `workspace/didChangeWorkspaceFolders` with the updated list

### Scenario: settings change — game folder changes
- GIVEN the user changes the game folder
- WHEN the change is applied
- THEN the plugin restarts the LSP server with the new `--game-path`

### Scenario: negative case — file outside any registered mod
- GIVEN the user opens an `.xs` file outside all registered mod folders
- WHEN the server sends `window/showMessage`
- THEN the plugin displays the notification: "file not part of any registered mod; engine API only"

### Scenario: first-run warning
- GIVEN auto-detect runs and finds no `game/` folders
- WHEN it finishes
- THEN the plugin shows a non-blocking warning that no mods were detected

## XS-engine constraints
- The plugin is not the language authority; all semantic decisions come from the Rust server. It MUST NOT implement its own completion or diagnostics logic.
- The TextMate grammar is already extracted from the official game VS Code extension and committed; the plugin SHALL bundle it, not regenerate it.

## Out of scope
- A VS Code extension client. (`extracted/xs.vsix` is removed from scope.)
- Marketplace publishing, signing, or update channels.
- Server-side parsing or engine-data extraction logic.

## Verification approach
- Automated: Gradle UI tests or lightweight component tests for settings persistence and auto-detect algorithm.
- Manual: install the plugin, set the game path, click Auto-detect, open a mod `.xs` file, and verify diagnostics arrive from the Rust server.

## Acceptance criteria
1. The plugin SHALL provide a settings page under Settings → Languages & Frameworks → XS with a game-folder text field and a mod list.
2. The mod list SHALL support adding and removing entries with + / - buttons.
3. The settings page SHALL provide an Auto-detect button that scans the project for directories named exactly `game`.
4. Auto-detect SHALL stop recursion at each `game/` boundary and SHALL add the parent of each found `game/` directory as a mod root.
5. Auto-detect SHALL run only when the mod list is empty.
6. Auto-detect SHALL warn if no `game/` folders are found.
7. On IDE startup, `StartupActivity.DumbAware` SHALL spawn the Rust LSP binary with `--game-path <game-folder>`.
8. After spawning, the plugin SHALL send `workspace/didChangeWorkspaceFolders` with the configured mod URIs.
9. The plugin SHALL register `didChangeWatchedFiles` watchers for `<game-folder>/game/**/*.xs`.
10. Adding or removing a mod folder SHALL re-send `workspace/didChangeWorkspaceFolders`.
11. Changing the game folder SHALL restart the LSP server with the new `--game-path`.
12. The plugin SHALL display `window/showMessage` notifications from the server without blocking the user.
13. The plugin SHALL bundle `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`; it SHALL NOT ship or commit the `extracted/xs.vsix` binary.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `xs-language-server` | 2026-06-24 | PASS WITH DEVIATIONS | Initial spec; IntelliJ platform fixture tests were rewritten as plain JUnit to avoid headless hangs. |
