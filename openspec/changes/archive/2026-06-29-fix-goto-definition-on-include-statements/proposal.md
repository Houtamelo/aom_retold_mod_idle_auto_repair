# Proposal: fix-goto-definition-on-include-statements

## Status
Draft

## Background
Issue #4 reports that go-to-definition fails on every `include "..."` directive in `.xs` files, regardless of trigger (Ctrl+Click, keybind, right-click → Go to → Definition). The LSP server's `textDocument/definition` handler at `tools/xs-language-server/src/server.rs:638-696` first calls `word::identifier_at_cursor`, which only scans `[A-Za-z0-9_]`. Because an include path is a `string_literal` child of an `include_directive`, the identifier lookup returns `None` and the handler exits before examining the directive. The plugin's `XsGotoDeclarationHandler` (added for Issue #1) forwards the LSP result unchanged, so when the server returns `null`, all three triggers do nothing.

## Approach
**Option A — Pure LSP fix (chosen).**

Extend `textDocument/definition` so that, when the cursor sits inside an `include_directive` path node, the server extracts the quoted target, resolves it through the existing `Workspace` include-resolution pipeline (mod overlay → vanilla game fallback), and returns the target file URI. The plugin's `XsGotoDeclarationHandler` will forward that `Location` for free.

**Option B — Pure plugin fix (rejected).** Would require reimplementing `Workspace::resolve_include` (include-root detection, mod overlay precedence, vanilla fallback) in Kotlin, creating a second source of truth that must be kept in sync with the server.

**Option C — Combined LSP + plugin fallback (rejected).** Overkill: the LSP already owns include semantics and has no known resolution gaps for normal includes.

## User-facing contract
- **GIVEN** an open `.xs` file with `include "ai/core/debug.xs"`.
- **WHEN** the user invokes any go-to trigger on the include path token `"ai/core/debug.xs"` (Ctrl+Click, go-to-definition keybind, or right-click → Go to → Definition).
- **THEN** the editor opens the resolved file at line 0, column 0.
- Multi-line `include` directives behave the same way on any token inside the path.
- Mod overlays take precedence: if `mod/spire_ai/game/ai/core/debug.xs` exists, navigation lands on the overlay copy, not the vanilla one.

## Scope

### In scope
- `tools/xs-language-server/src/server.rs`: add an include-aware branch in `goto_definition` before the existing identifier-resolution branch.
- `tools/xs-language-server/src/parser.rs`: add a small helper (or extend `extract_include_directives`) to detect whether the cursor lies inside an `include_directive` path and to return the unquoted target.
- `tools/xs-language-server/src/workspace.rs`: reuse existing `Workspace::resolve_include`; expose a relative-path helper only if `server.rs` cannot already derive the include root from the current file's URI (existing `Workspace::game_relative_path` likely covers this).
- Add LSP regression tests, most likely in a new `tools/xs-language-server/tests/goto_include_repro.rs` covering vanilla and mod-overlay resolution.
- Add at least one plugin regression test in `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` proving the handler delegates an include-path element and returns the resolver's targets.
- `tools/intellij-xs-plugin/gradle.properties`: bump `pluginVersion` 0.3.0 → 0.4.0 (Minor per AGENTS.md "new LSP feature").

### Out of scope
- Issues 1, 2, 3 from `docs/issues/2026-06-29-runtime-issues.md`.
- Range information beyond `(0,0)-(0,0)`; landing inside the target file on a specific symbol is a future enhancement.
- Predictive include completion.
- Multi-include / include-substitution language features.
- Changes to any XS mod script under `mod/`.

### Audit overlap
R3-F-04 (engine-API refs vs merged-view walk) touches the same include-resolution area, but it concerns engine-symbol reference sets, not `textDocument/definition` on include directives. **Split:** no co-fix; this change may reuse helpers, but R3-F-04 stays its own SDD change.

## Impact
- **Files changed:**
  - `tools/xs-language-server/src/server.rs` — add include branch to `goto_definition` (+20-40 LOC).
  - `tools/xs-language-server/src/parser.rs` — add include-path-at-position helper (+10-20 LOC).
  - `tools/xs-language-server/src/workspace.rs` — probably no change (existing APIs sufficient); optional small relative-path helper if needed (+0-5 LOC).
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` — add include-delegation regression test (+10-20 LOC).
  - `tools/intellij-xs-plugin/gradle.properties` — bump `pluginVersion` to 0.4.0 (+1/-1 LOC).
- **New test files:** `tools/xs-language-server/tests/goto_include_repro.rs`.
- **LOC delta:** +~50 / -~5 (excluding tests).
- **Risk:** low.
- **Effort:** 2–3 hours.
- **Compatibility:** No breaking change. Regular go-to-definition on identifiers remains unchanged. Clients that do not send workspace folders still get include resolution against the vanilla game folder.
- **pluginVersion bump:** 0.3.0 → 0.4.0 (MINOR per AGENTS.md "new LSP feature").

## Success criteria
- [ ] Opening an `.xs` file containing `include "ai/core/debug.xs"` and Ctrl+Clicking the path opens `ai/core/debug.xs` at line 0, column 0.
- [ ] The keybind and right-hand menu triggers behave identically.
- [ ] When the file is overridden in the mod overlay, navigation lands on the overlay copy.
- [ ] Regular identifier go-to-definition (Issue #1) shows no regression.
- [ ] All 215+ existing `cargo test` cases still pass.
- [ ] New LSP regression tests (`goto_include_repro.rs`) pass.
- [ ] New plugin regression test in `XsGotoDeclarationHandlerTest.kt` passes.

## Risks and unknowns
- Cursor-position calculation must target the path content, not bleed into the surrounding `include` keyword or semicolon. The helper should accept positions anywhere on the `string_literal` node.
- Returning a `(0,0)` target range is acceptable for "jump to file" but may feel imprecise if users expect the cursor to land on the first declaration; deferred to a follow-up.
- Plugin clickable underline depends on `GotoDeclarationHandler` returning a non-empty array when the caret is on the string literal. Existing Issue #1 handler only filters by `.xs` file extension and delegates to the LSP, so it is expected to work, but should be verified in a real plugin build.

## Open questions
- Should the path helper return the full quoted range (useful for future hover on include paths) or just the inner content range? Content range is enough for this change.
- Should unresolved include targets produce a diagnostic/null response or a clear LSP-level `null`? Resolution semantics already live in `Workspace`; the definition handler will return `Ok(None)` when resolution fails, matching current behavior.

## Next step
The spec phase should formalize the `textDocument/definition` include-path scenario as Given/When/Then requirements and define the exact cursor-position acceptance rules for single-line and multi-line `include` directives.
