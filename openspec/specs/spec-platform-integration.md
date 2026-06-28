# Spec: Platform integration (diagnostics, completion, hover, etc.)

This spec defines the contract for how LSP server responses surface as IntelliJ Platform features. After migration, the platform's built-in LSP API auto-handles all of these via its extension points; this spec is more of a regression-prevention contract than an implementation contract.

## Bug fix contracts

The three known bugs that motivated this migration MUST be fixed by the platform API integration.

### Bug #1: Diagnostics no longer logged-only

**Before migration**: `XsLanguageClient.publishDiagnostics()` only logs diagnostic parameters to the IntelliJ log file. Diagnostics never reach the editor or Problems window.

**After migration**: The platform's `Lsp4jClient` (created automatically by `LspServerSupportProvider`) routes `publishDiagnostics` into the platform's diagnostic pipeline, which surfaces them in:
- Inline gutter markers + red/yellow underlines in the editor
- The Problems window (View → Tool Windows → Problems)
- The status bar indicator
- Build output (if applicable)

### Bug #2: Completion contributor registered

**Before migration**: No `CompletionContributor` extension registered for `XS` language. Ctrl+Space shows nothing.

**After migration**: The platform's `LspServerSupportProvider` automatically registers a completion contributor that queries `textDocument/completion` on the LSP server and presents the results in the standard IntelliJ completion popup.

### Bug #3: Diagnostic integration wired

**Before migration**: Even if `publishDiagnostics` returned IntelliJ objects, no `ExternalAnnotator` or `ProblemRequestor` was registered to translate them into editor highlights.

**After migration**: The platform's `LspServerSupportProvider` registers the necessary extension points automatically. Diagnostics flow from LSP → platform → editor with no plugin code.

## Given/When/Then scenarios

### Diagnostics

- **Given** the LSP server returns `publishDiagnostics` for `file:///home/user/proj/mod/foo.xs` with one error at line 10, column 5
  **When** the user opens `foo.xs` in the editor
  **Then** the editor SHALL display a red underline at line 10, column 5. The Problems window SHALL list "error: <message>" for `foo.xs`.

- **Given** the LSP server clears diagnostics for `file:///home/user/proj/mod/foo.xs` (empty array)
  **When** the user edits `foo.xs` and the LSP re-analyzes
  **Then** the editor SHALL remove any previously shown underlines. The Problems window SHALL remove the entry for `foo.xs`.

### Completion

- **Given** the user has `foo.xs` open with the cursor positioned after `aiPlan` and types `(` to trigger parameter info
  **When** the user presses Ctrl+Space
  **Then** the platform SHALL send `textDocument/completion` to the LSP server and display the returned completions in a popup. Pressing Enter SHALL insert the selected completion.

- **Given** the LSP server returns an empty completion list for a position
  **When** the user presses Ctrl+Space
  **Then** the platform SHALL display "No suggestions" in the completion popup (NOT a blank popup with no message).

### Hover

- **Given** the user hovers the mouse over a symbol in `foo.xs`
  **When** the LSP server returns a `Hover` response with documentation
  **Then** the platform SHALL display the documentation in a tooltip within 500ms.

### Go to declaration

- **Given** the user has `foo.xs` open and places the cursor on a symbol that is defined in `bar.xs` (via `include`)
  **When** the user presses Ctrl+B (or selects Navigate → Declaration)
  **Then** the platform SHALL send `textDocument/definition` to the LSP server and navigate to the definition location in `bar.xs`.

### Find usages

- **Given** the user places the cursor on a function definition in `foo.xs`
  **When** the user presses Alt+F7 (or selects Edit → Find Usages)
  **Then** the platform SHALL send `textDocument/references` to the LSP server and display the results in the Find Usages tool window.

### Semantic highlighting

- **Given** the LSP server returns semantic tokens for a file
  **When** the file is opened in the editor
  **Then** the platform SHALL apply the semantic token styles to the editor (e.g., distinguishing types, functions, variables by color/italic). This is automatic since 2024.2.

### Inlay hints

- **Given** the LSP server returns inlay hints for a file
  **When** the file is opened in the editor
  **Then** the platform SHALL display the inlay hints (e.g., parameter names, return types) inline in the editor. This is automatic since 2025.2.2.

## Negative scenarios (what MUST NOT happen)

- **MUST NOT**: The plugin's `XsLanguageClient` (or any custom LanguageClient) intercepts `publishDiagnostics` and only logs them. The plugin SHALL NOT have any class implementing `org.eclipse.lsp4j.services.LanguageClient`.
- **MUST NOT**: The plugin re-implements diagnostic-to-editor wiring via custom `ExternalAnnotator` or `ProblemRequestor`. The platform's automatic wiring is the only acceptable path.
- **MUST NOT**: The plugin re-implements completion via custom `CompletionContributor`. The platform's automatic wiring is the only acceptable path.
- **MUST NOT**: The plugin adds `LspCustomization` opt-outs that disable features the LSP server provides (e.g., `LspCompletionDisabled`, `LspHoverDisabled`). XS has no native PSI; default behavior = correct.

## Stub PSI compatibility

The existing stub PSI implementation (`XsParserDefinition.createElement()` throws `UnsupportedOperationException("Full XS PSI factory is not implemented until P2")`) MAY remain unchanged. The platform's LSP integration does not depend on the PSI working. The stub PSI exists only to satisfy the `lang.parserDefinition` extension point declaration and is not called during normal LSP-driven features.

- **Given** the stub PSI throws `UnsupportedOperationException` when its `createElement()` is called
  **When** the user opens an `.xs` file
  **Then** the editor SHALL still display syntax highlighting (via TextMate), diagnostics (via LSP), and completion (via LSP). The platform SHALL NOT propagate the stub PSI's exception to the editor.

## Performance expectations

The migration MUST NOT regress performance. Specifically:

- `didOpen`/`didChange`/`didClose` events SHALL be sent to the LSP server within 100ms of the corresponding IDE event.
- The completion popup SHALL appear within 200ms of Ctrl+Space on a typical `.xs` file (~500 lines).
- Diagnostics SHALL appear within 500ms of opening a file (LSP server's first analysis).
- Hover tooltips SHALL appear within 300ms.

## Plugin version

The migration MUST bump `pluginVersion` 0.1.4 → 0.1.5 in `tools/intellij-xs-plugin/gradle.properties`. Per the project's AGENTS.md plugin version bump policy, this is a bug fix (not a feature), so a patch bump is correct.

## Plugin.xml contract

The plugin.xml MUST declare the following extensions and dependencies:

```xml
<idea-plugin>
    <!-- ... -->
    <depends>com.intellij.modules.platform</depends>
    <depends>com.intellij.modules.lang</depends>
    <depends>com.intellij.modules.lsp</depends>
    <depends>com.intellij.modules.ultimate</depends>
    <depends>org.jetbrains.plugins.textmate</depends>
    <depends optional="true">com.intellij.modules.rider</depends>
    
    <extensions defaultExtensionNs="com.intellij">
        <platform.lsp.serverSupportProvider 
            implementation="com.aomr.xs.lsp.XsLspSupportProvider"/>
        <!-- ... existing extensions (fileTypeFactory, textmate.bundleProvider, 
             lang.ast.factory, lang.parserDefinition, lang.braceMatcher, 
             lang.quoteHandler, lang.commenter, notificationGroup,
             projectConfigurable) ... -->
    </extensions>
</idea-plugin>
```

The plugin MUST NOT declare obsolete extensions that the platform now provides:
- `<lang.completion.contributor language="XS" .../>` (provided by platform API)
- `<externalAnnotator language="XS" .../>` (provided by platform API)
- `<problemRequestor .../>` (provided by platform API)
- `<postStartupActivity implementation="com.aomr.xs.startup.XsStartupActivity"/>` (replaced by `platform.lsp.serverSupportProvider`)

The `XsStartupActivity` itself remains as a separate component (registered via `<postStartupActivity>`) to handle first-run UX (game-path-missing notification, mod auto-detection, settings save). Its job is reduced to setting up persistent state; the platform takes over from there.
