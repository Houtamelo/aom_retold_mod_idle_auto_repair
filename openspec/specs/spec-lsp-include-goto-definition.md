# Delta Spec: LSP include go-to-definition

**Change:** `fix-goto-definition-on-include-statements`

## Capability summary

The XS LSP server SHALL navigate from any include directive's path token to the resolved target file, so users can traverse the include graph via Ctrl+Click, keybind, or right-click.

## Rationale

Issue #4 (`docs/issues/2026-06-29-runtime-issues.md` lines 291-359) reports that all go-to-definition triggers fail on `include "..."` directives because the server only extracts XS identifier tokens. Since the include path is a `string_literal`, the handler returns `null` before locating the directive. Reusing the existing `Workspace::resolve_include` pipeline (mod overlay → vanilla fallback) fixes this without duplicating resolution logic.

## ADDED Requirements

### Requirement: Direct include navigation

The LSP server SHALL resolve a `textDocument/definition` request when the cursor lies inside the `string_literal` path of an `include_directive`.

#### Scenario: R1 — Single-line include path

- GIVEN an `.xs` file contains `include "ai/core/debug.xs"` on line N
- WHEN the cursor is on any character inside `"ai/core/debug.xs"`
- THEN the LSP SHALL return a `Location` whose URI resolves to the target file
- AND the target range SHALL be `(0,0)-(0,0)`

#### Scenario: R1b — Multi-line include path

- GIVEN an `include` directive whose path token spans multiple lines
- WHEN the cursor is on any character within the path token
- THEN the server SHALL resolve the target and return a `(0,0)` location

### Requirement: Mod-overlay resolution

The LSP server SHALL resolve include targets against the mod overlay before the vanilla game folder.

#### Scenario: R2 — Overlay precedence

- GIVEN `mod/spire_ai/game/ai/core/debug.xs` overlays the vanilla `game/ai/core/debug.xs`
- WHEN the user invokes go-to-definition on `include "ai/core/debug.xs"` in a mod file
- THEN the URI SHALL point to the mod overlay copy

### Requirement: Vanilla fallback

The LSP server SHALL fall back to the vanilla game folder when the mod does not override the target.

#### Scenario: R3 — Unmodded target

- GIVEN the active mod does not override `ai/core/debug.xs`
- AND the vanilla game contains `game/ai/core/debug.xs`
- WHEN the user invokes go-to-definition on that include path
- THEN the URI SHALL point to `<AOMR>/game/ai/core/debug.xs`

### Requirement: Not-found include returns null

The LSP server SHALL return an empty response when the include target cannot be resolved.

#### Scenario: R4 — Missing target

- GIVEN an `.xs` file contains `include "does/not/exist.xs"`
- WHEN the user invokes go-to-definition on the path token
- THEN the LSP SHALL return an empty list

### Requirement: Cursor outside the path token

The LSP server SHALL preserve existing identifier-based go-to-definition behavior when the cursor is not inside the include path token.

#### Scenario: R5 — Fallback behavior

- GIVEN an include directive at line 2, columns 0-30
- WHEN the cursor is on the `include` keyword or outside the quoted path
- THEN the LSP SHALL fall through to identifier resolution
- AND it SHALL return the symbol definition or `null`, unchanged from current behavior

### Requirement: Plugin forwarding

The plugin SHALL forward an include-path `Location` from the LSP to the editor.

#### Scenario: R6 — Handler opens resolved include

- GIVEN the LSP returns a `Location` for an include path token
- WHEN `XsGotoDeclarationHandler` receives it via Issue #1's resolver
- THEN the plugin SHALL open the resolved file at line 0, column 0

### Requirement: Strict-TDD regression tests

The change SHALL include automated tests proving the new behavior.

#### Scenario: R7 — LSP regression coverage

- GIVEN new tests in `tools/xs-language-server/tests/goto_include_repro.rs`
- WHEN they run under `cargo test`
- THEN they SHALL cover direct navigation, mod overlay, vanilla fallback, and not-found

#### Scenario: R7b — Plugin regression coverage

- GIVEN an added test in `XsGotoDeclarationHandlerTest.kt`
- WHEN it runs under `./gradlew :test`
- THEN it SHALL assert the handler forwards an include-path element to the resolver

## Cross-references

- **User issue:** `docs/issues/2026-06-29-runtime-issues.md` Issue 4.
- **Plugin handler:** Issue #1's `XsGotoDeclarationHandler` (`spec-xs-goto-declaration-handler.md`) forwards LSP locations.
- **Include resolver:** Issue #2's `Workspace::resolve_include` pipeline (`spec-lsp-forward-decl-honesty.md`) is reused.
- **Version:** `AGENTS.md` lines 93-111; bump `pluginVersion` from `0.3.0` to `0.4.0` (MINOR — new LSP feature).

## Out of scope

- Issues 1, 2, and 3 from `docs/issues/2026-06-29-runtime-issues.md`.
- Identifier go-to-definition changes beyond pre-existing fallback.
- Target-range precision beyond `(0,0)-(0,0)`.
- Include completion, hover, or multi-include syntax.

## Verification approach

- Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml`; all 215+ existing tests plus the new include-goto tests SHALL pass.
- Run `./gradlew :test`; the new plugin regression test SHALL pass.
- Manual check in Rider: Ctrl+Click an include path in a mod overlay and verify the overlay file opens at `(0,0)`.

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | `textDocument/definition` detects an include path token at the cursor. |
| 2 | Single-line and multi-line include paths resolve through `Workspace::resolve_include`. |
| 3 | Mod overlays take precedence over vanilla files. |
| 4 | Unmodded paths fall back to vanilla `game/`. |
| 5 | Unresolvable paths return an empty list. |
| 6 | Positions outside the path token fall through to identifier resolution. |
| 7 | `XsGotoDeclarationHandler` forwards include-path locations. |
| 8 | 4+ LSP regression tests and 1+ plugin regression test are added and pass. |
| 9 | `pluginVersion` in `gradle.properties` reads `0.4.0`. |

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `fix-goto-definition-on-include-statements` | 2026-06-29 | Spec phase | Delta spec formalizing include-path go-to-definition. |
