## Bug status

CONFIRMED

## Current state

Go-to-definition fails on every `include "..."` directive, across all three triggers (Ctrl+Click, keybind, right-click → Go to → Definition). The LSP server only resolves identifiers that match XS identifier rules; the `string_literal` path inside an `include_directive` is not an identifier, so `textDocument/definition` returns `null`. The existing plugin `XsGotoDeclarationHandler` (added for Issue #1) forwards the LSP result unchanged, so it also returns no target.

## Code-path map

### LSP server

- `tools/xs-language-server/src/server.rs:638-696` — `goto_definition` handler.
  - Line 649: calls `word::identifier_at_cursor` on the request position.
  - Because the path token of `include "core.xs";` is a `string_literal`, `identifier_at_cursor` returns `None` at line 649 and the handler returns `Ok(None)` (line 651).
  - The include case is never reached.
- `tools/xs-language-server/src/word.rs:22-56` — `identifier_at_cursor` only scans `[A-Za-z0-9_]`, so it cannot extract a quoted path.
- `tools/xs-language-server/src/parser.rs:21-41` — `extract_include_directives` already walks `include_directive` nodes and returns the target path with quotes stripped and the directive range. It can be extended (or a new helper added) to return the path node range/position.
- `tools/xs-language-server/src/grammar.js:113-117` — `include_directive: $ => seq('include', field('path', $.string_literal), ';')`; the path is a named field under `include_directive`.

### Plugin

- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandler.kt:23-47` — forwards every `.xs` go-to request to `XsDefinitionResolver`.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsDefinitionResolver.kt:39-58` — builds `textDocument/definition` params, converts returned `Location`/`LocationLink` into `PsiElement` via `VirtualFileManager`, and returns the list. If the LSP ever returns a file URI for an include target, this code opens the file.
- No existing include-specific handler, reference contributor, or reference provider exists in the plugin (grep for `IncludeReference`, `IncludeHandler`, etc. returned no matches).

## Fix-site analysis

| Option | Description | Pros | Cons | Effort |
|--------|-------------|------|------|--------|
| A — Pure LSP fix | Extend `textDocument/definition` to detect a cursor inside an `include_directive` path, resolve via `Workspace`, return the target URI. | Single source of truth; reuses existing include resolution; plugin gets the feature for free for all three triggers; tests can be `cargo test` integration. | Need a small AST-position helper; need to compute the current file's relative path. | Low |
| B — Pure plugin fix | Add plugin-side detection of include tokens and resolve the path with `VirtualFileManager`/game folder. | Does not touch Rust server. | Duplicates `Workspace::resolve_include` (include roots, mod overlay, vanilla fallback) in Kotlin; must keep plugin and server resolution in sync. | Medium |
| C — Combined | LSP returns target when resolved; plugin adds fallback for unresolved paths. | Robust if LSP resolution has gaps. | More complex than A; no evidence LSP gaps exist for normal includes. | Medium |

**Recommendation: Option A.** The LSP already owns include semantics (overlay + root + fallback), and the existing plugin handler already converts LSP locations into navigable PSI elements. A server-side fix propagates automatically to Ctrl+Click, keybind, and right-click without any plugin navigation logic change.

## Include resolution in LSP

The resolution stack we can reuse:

1. `Workspace::resolve_include` (`tools/xs-language-server/src/workspace.rs:249-262`) takes the current file's relative path under `game/`, the quoted target, and a `VirtualProject`. It detects the include root (`ai/`, `data/trigger/`, `random_maps/`), prepends it, and calls `resolve_file`.
2. `Workspace::resolve_file` (`workspace.rs:227-241`) checks mod overlays first (`project.file_overrides`), then falls back to `<game_path>/game/<rel>`, and rejects non-`.xs`/unreadable files.
3. `Workspace::resolve_include_edge` (`workspace.rs:266-290`) wraps the above and returns both the target path and an `IncludeEdge` record.
4. `MergedView::build` (`merged_view.rs:271-399`) already uses this pipeline to build the include graph.

For `goto_definition`, the cleanest path is:

- Parse the current document with `parser::parse`.
- Find an `include_directive` whose `path` (string_literal) range contains the cursor.
- Extract the unquoted target.
- Build a `VirtualProject` for the owning mod (or default for unowned files) the same way `get_or_build_merged_view` does (`server.rs:195-203`).
- Compute the current file's relative path under `game/` (mirroring `merged_view::relative_path_for` or exposing it from `Workspace`).
- Call `Workspace::resolve_include` and return `Location { uri: Url::from_file_path(target), range: Range::new(Position::new(0,0), Position::new(0,0)) }`.

If no include matches, fall through to the existing identifier resolution.

## Affected files (potential)

- `tools/xs-language-server/src/server.rs:638-696` — add include detection before identifier detection; return include target `Location`.
- `tools/xs-language-server/src/parser.rs` — add `include_path_at_position` helper (or extend `extract_include_directives` to expose path node range).
- `tools/xs-language-server/src/workspace.rs` — consider making `relative_path_for` logic public or adding `Workspace::relative_path_for_file(project, abs_path)` so `server.rs` does not duplicate it.
- `tools/intellij-xs-plugin/gradle.properties:12` — bump `pluginVersion` 0.3.0 → 0.4.0 (new navigation capability = Minor per AGENTS.md table).
- Optional: `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` — add a small test confirming the handler delegates a string-literal element and returns the resolver's targets (mostly documentation, since the real work is server-side).

## Test infrastructure

### LSP

- Existing unit tests live under `tools/xs-language-server/src/*/tests` and `tools/xs-language-server/tests/`.
- Pattern for spawn-based integration tests: `tools/xs-language-server/tests/r5_test_honesty_repro.rs` launches the server binary and drives it via JSON-RPC.
- A new file `tools/xs-language-server/tests/goto_include_repro.rs` can create a temp `game/` layout + mod overlay, spawn `xs-language-server`, send `initialize` + workspace folder + `textDocument/didOpen`, then `textDocument/definition` on the include path and assert the response URI.
- `Workspace::resolve_include` and the include-root tests already in `workspace.rs` give fast unit coverage of the resolution half.

### Plugin

- Existing handler tests: `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` using `BasePlatformTestCase` and a stub `XsDefinitionResolver`.
- Add one test that configures an `.xs` buffer containing `include "b.xs";`, positions the caret inside the string literal, injects a target `PsiElement`, and asserts the handler returns it. This proves the plugin path works once the server starts returning a location.

## pluginVersion

- Current (post-Issue #3): `0.3.0` in `tools/intellij-xs-plugin/gradle.properties:12`.
- Planned bump: **0.4.0** (Minor). Adding go-to-definition on include statements is a new navigation capability / new LSP feature covered by the AGENTS.md Minor row.

## Audit cross-reference

- No direct overlap with the 141 audit findings in `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md`.
- Issue #2 work (`fix-lsp-false-positive-forward-decl`) touched the same include-resolution pipeline in `merged_view.rs` and `workspace.rs`, but only for ordering/diagnostics; nothing there exposes includes to `textDocument/definition`.

## Scope recommendation for proposal phase

- **Files to change:**
  - `tools/xs-language-server/src/server.rs`
  - `tools/xs-language-server/src/parser.rs` (new helper)
  - `tools/xs-language-server/src/workspace.rs` (optional path helper)
  - `tools/intellij-xs-plugin/gradle.properties`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt`
- **New test files:**
  - `tools/xs-language-server/tests/goto_include_repro.rs`
- **LOC delta:** +~120 / -~5 (mostly new include-path helper + handler branch + tests)
- **Risk:** low
- **Effort:** ~3-4 hours
- **Audit overlap:** None identified

## Risks / unknowns

1. **Cursor-position granularity.** Need to decide whether the path node range includes the quotes or only the content. Returning a definition for the quotes is harmless UX over-trigger; keeping it to the inner content is cleaner. A helper should accept positions on the path string itself.
2. **Current-file relative-path computation.** For mod overlay files this is a reverse lookup from absolute path to `game/<rel>`. The `MergedView` helper `relative_path_for` handles this but is private; exposing it avoids duplication.
3. **Target file range.** Returning `(0,0)` opens the file at the top. This matches user expectation (jump to file), but if the project wants the cursor placed on the first substantive symbol, scope can be expanded later.
4. **Plugin underline for string literal.** Assumes `GotoDeclarationHandler` returning targets makes the `sourceElement` under cursor clickable. This is standard IntelliJ Platform behavior, but should be verified with the real plugin build.

## Next step

Proposal phase should spec Option A: add an include-aware branch to `textDocument/definition` that parses the `include_directive` path node, reuses `Workspace::resolve_include`, returns the resolved file URI, and leaves the plugin handler unchanged except for a version bump and a delegation smoke test.
