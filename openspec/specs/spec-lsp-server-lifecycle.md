# Spec: LSP server lifecycle and binary resolution

This spec defines the contract for the LSP server's process lifecycle, the binary resolution strategies, and how the plugin reacts to game folder changes.

## Binary Resolution

The plugin MUST resolve the path to the `xs-language-server` binary by trying the following strategies in order. The first strategy that yields a usable path wins.

1. **Java system property** `-Dxs.lsp.path=<path>`. MUST be honored if set and non-blank. Used for debugging and CI.
2. **Environment variable** `XS_LSP_PATH=<path>`. MUST be honored if set and non-blank. Used for CI and sandboxed environments.
3. **Bundled binary** at `bin/xs-language-server` (resource extracted from the plugin JAR). MUST be extracted to a temp file on first use; the extracted path MUST be cached for the lifetime of the JVM. The temp file MUST be deleted on JVM exit. This is the default strategy when the plugin is installed from disk.
4. **Project-local binary** at `<project>/tools/xs-language-server/target/release/xs-language-server` or `tools/xs-language-server/target/release/xs-language-server` (relative to CWD). MUST be honored if the file exists and is executable. Convenience for plugin development.
5. **PATH lookup** `xs-language-server`. MUST be the fallback. The plugin SHOULD log a warning when this strategy is used (the user is expected to set up the binary via strategies 1-3).

### Given/When/Then scenarios

- **Given** the `-Dxs.lsp.path` system property is set to `/custom/path/xs-language-server`
  **When** the plugin needs to start the LSP server
  **Then** the plugin SHALL use `/custom/path/xs-language-server` and SHALL NOT attempt other strategies.

- **Given** the `-Dxs.lsp.path` system property is not set, and `XS_LSP_PATH=/env/path/xs-language-server`
  **When** the plugin needs to start the LSP server
  **Then** the plugin SHALL use `/env/path/xs-language-server` and SHALL log a message indicating the env var was used.

- **Given** neither system property nor env var is set, and the bundled binary is present in the plugin JAR
  **When** the plugin needs to start the LSP server
  **Then** the plugin SHALL extract the bundled binary to a temp file, mark it executable, cache the extracted path, and use it. Subsequent starts in the same JVM SHALL reuse the cached path.

- **Given** neither system property nor env var is set, no bundled binary is present, and the project-local binary exists at `tools/xs-language-server/target/release/xs-language-server` and is executable
  **When** the plugin needs to start the LSP server
  **Then** the plugin SHALL use the project-local binary.

- **Given** none of the above strategies yield a usable binary
  **When** the plugin needs to start the LSP server
  **Then** the plugin SHALL attempt `xs-language-server` on the `PATH` and SHALL log a warning instructing the user to set `XS_LSP_PATH` or install the plugin from disk.

## Server Lifecycle

The LSP server's process MUST be managed by IntelliJ Platform's `LspServerManager`. The plugin MUST NOT spawn, manage, or kill the process directly.

### When the server starts

The platform starts the LSP server lazily on the first `textDocument/didOpen` (or other LSP request) for a supported file. The `XsLspSupportProvider.fileOpened()` callback SHALL be the entry point:

- **Given** the user opens a file with extension `.xs` in the IDE
  **When** `XsLspSupportProvider.fileOpened()` is called
  **Then** the plugin SHALL call `serverStarter.ensureServerStarted(XsLspServerDescriptor)`, which causes the platform to spawn the binary and perform the LSP `initialize` handshake.

### LSP initialize arguments

The plugin MUST pass the game folder to the LSP server at startup. Per the current LSP server contract, this is done via the `--game-path <path>` CLI argument. The plugin SHALL construct the `GeneralCommandLine` with this argument.

- **Given** the configured game folder is `/home/user/.steam/steamapps/common/Age of Mythology Retold` and that path contains a `game/` directory and a `doxygen_retail.7z` file
  **When** the LSP server is started
  **Then** the spawned process SHALL receive `--game-path /home/user/.steam/steamapps/common/Age\ of\ Mythology\ Retold` as its first argument.

- **Given** the configured game folder is blank or does not contain a `game/` directory
  **When** the LSP server is needed
  **Then** the plugin SHALL NOT start the server. The plugin SHOULD show a notification pointing the user to the XS settings page.

### Game folder change

The plugin MUST restart the LSP server when the game folder changes.

- **Given** the LSP server is running with game folder `/path/A`
  **When** the user changes the configured game folder to `/path/B` via the XS settings page
  **Then** the plugin SHALL stop the current server and start a new one with game folder `/path/B`.

### Server stderr → log file

The LSP server writes diagnostic output to stderr (via `tracing_subscriber`). The plugin MUST redirect stderr to a log file at `<project>/.idea/xs-lsp.log`. The file MUST be appended across sessions. The file MUST NOT be redirected to stdout (the LSP JSON-RPC stream) because the platform's LSP message reader rejects anything that is not `Content-Length:` headers + JSON bodies.

- **Given** the project's base path is `/home/user/project`
  **When** the LSP server starts
  **Then** the plugin SHALL redirect stderr to `/home/user/project/.idea/xs-lsp.log`, creating parent directories if needed. The file MUST be appended, not truncated.

- **Given** the project has no base path (rare edge case, e.g. detached project)
  **When** the LSP server starts
  **Then** the plugin SHALL redirect stderr to `<java.io.tmpdir>/xs-lsp.log` as a fallback.

### RUST_LOG environment passthrough

The plugin MUST forward the user's `RUST_LOG` environment variable to the LSP server's process environment. This allows users to opt into debug-level logging without code changes.

- **Given** the user's shell has `RUST_LOG=debug` set
  **When** the LSP server starts
  **Then** the spawned process environment SHALL include `RUST_LOG=debug`.

- **Given** the user's shell has no `RUST_LOG` set
  **When** the LSP server starts
  **Then** the spawned process environment SHALL NOT include `RUST_LOG`, allowing the LSP server to use its own default (info level).

## File watching

The plugin MUST NOT install a custom `VirtualFileListener` for forwarding `workspace/didChangeWatchedFiles` events to the LSP server. The platform handles this automatically since 2023.3.2 (per IntelliJ Platform docs).

- **Given** the user modifies a `.xs` file in the project's `<game>/` directory
  **When** the file system reports the change to IntelliJ
  **Then** the platform SHALL forward the change to the LSP server via `workspace/didChangeWatchedFiles` without any plugin-side wiring.

- **Given** the user deletes a `.xs` file from the project's `<game>/` directory
  **When** the file system reports the deletion to IntelliJ
  **Then** the platform SHALL forward the deletion to the LSP server.

## Plugin dependencies

The plugin.xml MUST declare the following module dependencies:

- `<depends>com.intellij.modules.lsp</depends>` — required for the LSP API package
- `<depends>com.intellij.modules.ultimate</depends>` — required for some LSP features (e.g., rename refactoring)

These dependencies MUST be present at plugin load time. If they are not, the plugin SHALL fail to load.
