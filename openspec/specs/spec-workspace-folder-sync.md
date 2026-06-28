# Spec: Workspace folder sync (mod list)

This spec defines the contract for how the plugin synchronizes the user's configured mod list (`XsSettings.modPaths`) with the LSP server as workspace folders.

## Initial workspace folders

When the LSP server starts (via the platform's `LspServerManager`), the plugin MUST send the configured mod list as initial workspace folders.

### Contract

The LSP server receives workspace folders via the `initialize` request's `workspaceFolders` parameter and via the `workspace/didChangeWorkspaceFolders` notification. The plugin MUST:

1. Include the configured mod paths as workspace folders in the `initialize` request
2. Send `workspace/didChangeWorkspaceFolders` notifications when the mod list changes after init

### Given/When/Then scenarios

- **Given** the user has configured mods at `/home/user/proj/mod/intelligent_auto_scout` and `/home/user/proj/mod/idle_auto_repair`
  **When** the LSP server starts
  **Then** the LSP `initialize` request SHALL include `workspaceFolders: [{uri: "file:/home/user/proj/mod/intelligent_auto_scout", name: "intelligent_auto_scout"}, {uri: "file:/home/user/proj/mod/idle_auto_repair", name: "idle_auto_repair"}]`.

- **Given** the user has no configured mods
  **When** the LSP server starts
  **Then** the LSP `initialize` request SHALL include `workspaceFolders: []` (empty list, NOT null).

## Workspace folder change

When the mod list changes, the plugin MUST notify the LSP server via `workspace/didChangeWorkspaceFolders` (or restart the server as a fallback).

### Given/When/Then scenarios

- **Given** the LSP server is running and the current mod list is `[/mod/A, /mod/B]`
  **When** the user adds `/mod/C` to the mod list (via settings or auto-detection)
  **Then** the plugin SHALL send `workspace/didChangeWorkspaceFolders` with `event: {added: [{uri: "file:/mod/C", name: "C"}], removed: []}`.

- **Given** the LSP server is running and the current mod list is `[/mod/A, /mod/B]`
  **When** the user removes `/mod/A` from the mod list
  **Then** the plugin SHALL send `workspace/didChangeWorkspaceFolders` with `event: {added: [], removed: [{uri: "file:/mod/A", name: "A"}]}`.

- **Given** the LSP server is running and the current mod list is `[/mod/A]`
  **When** the user replaces the entire mod list with `[/mod/X, /mod/Y]`
  **Then** the plugin SHALL send `workspace/didChangeWorkspaceFolders` with `event: {added: [{X}, {Y}], removed: [{A}]}` (delta computed as set difference, NOT replace).

## URI format

Workspace folder URIs MUST be encoded as `file:<absolute-path>` (single-slash form per Java's `File(path).toURI().toString()`). The LSP server's `lookup_mod(file_uri)` performs `Path::starts_with(&mod_path)` on the longest-prefix match; using the wrong form (e.g., `file:///path` with three slashes) would break the prefix match.

- **Given** a mod path `/home/user/proj/mod/foo bar` (contains a space)
  **When** the plugin encodes it as a workspace folder URI
  **Then** the URI SHALL be `file:/home/user/proj/mod/foo%20bar` (single slash, percent-encoded space). The URI MUST NOT be double-encoded (e.g., `file:/home/user/proj/mod/foo%2520bar`).

## Settings change listener

The plugin MUST listen for changes to `XsSettings.state.modPaths` and trigger a workspace folder sync. The listener MUST be a per-project service that:

1. Is created when the LSP server is needed for the project (lazily, on first `.xs` file open)
2. Stays alive for the lifetime of the project
3. Is disposed when the project is closed

### Given/When/Then scenarios

- **Given** the user opens the XS settings page and adds a mod path
  **When** they click "OK" to save
  **Then** the plugin SHALL persist the new mod list to `XsSettings.state.modPaths` and SHALL trigger a workspace folder sync via `workspace/didChangeWorkspaceFolders` (or server restart as fallback).

- **Given** the user has not changed the mod list
  **When** the plugin's settings listener fires for an unrelated reason (e.g., another component modifying the state object)
  **Then** the plugin SHOULD debounce or no-op to avoid spurious workspace folder updates.

## Fallback: server restart

If the platform API does not expose a hook to send `workspace/didChangeWorkspaceFolders` directly, the plugin MUST fall back to restarting the LSP server when the mod list changes.

- **Given** the LSP server is running and the mod list changes
  **When** no platform hook is available for sending `workspace/didChangeWorkspaceFolders`
  **Then** the plugin SHALL call `LspClientManager.stopAndRestartClientsIfNeeded(XsLspSupportProvider::class.java)` to restart the server with the new mod list.

- **Given** the LSP server restart takes more than 30 seconds
  **When** the user changes the mod list
  **Then** the plugin SHALL show a notification indicating that the server is restarting and SHALL log the restart duration to the LSP log file.

## Auto-detected mods

When the plugin's `XsModAutoDetector` finds mod folders on first project open, the plugin MUST persist the detected paths and trigger a workspace folder sync as if the user had added them manually.

- **Given** the project is opened for the first time and `XsSettings.state.modPaths` is empty
  **When** `XsModAutoDetector.scan(projectRoot)` finds two mod folders
  **Then** the plugin SHALL save both paths to `XsSettings.state.modPaths` and SHALL send a `workspace/didChangeWorkspaceFolders` notification with both as added folders.

## Backward compatibility

The migration MUST NOT change the LSP server's contract for workspace folders. The server already expects:

- `initialize.workspaceFolders` as a `List<WorkspaceFolder>` (URI + name)
- `workspace/didChangeWorkspaceFolders` with added/removed deltas

The migration only changes how the plugin constructs and sends these messages — not the protocol.

## Patch maintenance

This spec MUST NOT modify any `.xs` source files. The plugin-side workspace folder sync is purely a client concern.
