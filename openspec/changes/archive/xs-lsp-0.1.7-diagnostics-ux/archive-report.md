# Archive Report: xs-lsp-0.1.7-diagnostics-ux

## Change

| Field | Value |
|-------|-------|
| Change ID | `xs-lsp-0.1.7-diagnostics-ux` |
| Branch | `xs-lsp-0.1.7-diagnostics-ux` |
| Final commit | `462516a` |
| Archive date | 2026-06-28 |

## Intent & Scope

Fix four user-reported LSP server bugs so diagnostics clear reliably after edits, parse errors read as actionable sentences, and engine-API symbols (`kbUnitCount`, `aiEcho`, etc.) behave correctly in **Find References** and **Go to Definition**. Only the Rust server under `tools/xs-language-server/` was changed; IDE-side work stayed out of scope.

### In Scope

- Clear stale diagnostics by publishing an empty array when a file becomes clean.
- Rewrite parse-error messages for `MISSING`/`ERROR` tree-sitter nodes into actionable text.
- Return real use-site locations for references on engine-API symbols.
- Stop returning non-navigable `xs-stub://engine/<name>` URIs for engine-API definitions.
- Bump `pluginVersion` 0.1.6 → 0.1.7 and add tests under strict TDD.

### Out of Scope

- TextMate language-ID alignment / syntax highlighting.
- ColorScheme / `<colorSettingsPage>` registration.
- Kotlin or plugin XML changes beyond the version bump.

## Fixes Implemented

| # | Fix | Key files | Evidence |
|---|-----|-----------|----------|
| 1 | **Stale diagnostic clearing** | `tools/xs-language-server/src/diagnostics.rs`, `src/server.rs` | `collect_all` now inserts an empty entry for `current_uri`; roundtrip asserts `"diagnostics":[]` after last fix. |
| 2 | **Actionable parse-error messages** | `tools/xs-language-server/src/diagnostics.rs` | `MISSING` → `Missing '<token>'`; `ERROR` → `Unexpected <kind> '<text>'`; blacklists `MISSING`/`ERROR`/`node`/`column(s)`. |
| 3 | **Engine-symbol references** | `tools/xs-language-server/src/server.rs`, `src/references.rs` | Workspace scan via `visible_files` + `find_identifier_uses`; returns 2 `aiEcho` locations in 52 ms (≤500 ms). |
| 4 | **Engine-symbol definition** | `tools/xs-language-server/src/server.rs`, `src/bin/lsp_roundtrip_test.rs` | Engine-only symbols return `Ok(None)`; roundtrip asserts null for `aiEcho` and no `xs-stub://` URI. |

## Commits

Branch `xs-lsp-0.1.7-diagnostics-ux` @ `462516a` contains **13 meaningful implementation commits**. The branch log also shows 2 accidental plugin commits (`fix(xs-plugin): lowercase XsLanguage id...` and its RED test) plus their reverts; these cancel out and have no net effect on the codebase or archive content.

Implementation commits (most recent first):

1. `462516a` — chore(xs-lsp): align server.rs capability comments with new behavior
2. `d7e37d5` — chore(xs-lsp): pin strict TDD in openspec config + ignore .worktrees/
3. `d18bbdf` — fix(xs-lsp): action parse-error formatter; do not leak ERROR or grammar non-terminals
4. `7b211d8` — test(xs-lsp): assert parse-error formatter does not leak ERROR or grammar non-terminals
5. `3504b32` — chore(xs-lsp): bump pluginVersion to 0.1.7 (Phase 5.1)
6. `7a51290` — fix(xs-lsp): scan workspace files for engine-symbol references with time budgets
7. `4bd0c7f` — test(xs-lsp): assert engine-symbol aiEcho references return >=2 workspace locations
8. `bbd6a19` — fix(xs-lsp): actionable parse-error messages for MISSING and ERROR nodes
9. `fc87ab9` — test(xs-lsp): add parse-error message shape tests
10. `556e194` — fix(xs-lsp): always publish empty diagnostics for clean files; update comments
11. `be47594` — test(xs-lsp): assert empty publishDiagnostics after fixing last parse error
12. `b4f7ef1` — fix(xs-lsp): return null for engine-API symbol definitions
13. `cd5a8de` — test(xs-lsp): expect null for engine-symbol definition

Cosmetic / self-canceled commits:

- `595bb0b` → `fd6b80f` → `4765dfb` → `6e037f1` (test + fix + revert + revert) touched only plugin language-id scaffolding and were fully reverted before merge.

## Test Results

| Suite | Result |
|-------|--------|
| Library unit tests (`cargo test --lib`) | **178 passed, 0 failed** |
| Full `cargo test` | **178 lib + 9 integration passed, 0 failed** |
| Game-folder integration (`--test game_folder_parse`) | **9 passed, 0 failed** |
| LSP roundtrip binary | **44 PASS / 2 FAIL** |

The 2 roundtrip failures (`float_to_int_loss`, `extern_collision`) are **pre-existing semantic fixture failures** present on `master` and untouched by this change. They are out of scope for `0.1.7`.

## Version Bump

`tools/intellij-xs-plugin/gradle.properties`: `pluginVersion` moved from `0.1.6` to `0.1.7`.

## Specs Synced

All four delta specs from `openspec/changes/xs-lsp-0.1.7-diagnostics-ux/specs/` were merged into the source-of-truth specs directory:

- `openspec/specs/spec-stale-diagnostic-clearing.md`
- `openspec/specs/spec-actionable-parse-errors.md`
- `openspec/specs/spec-engine-symbol-references.md`
- `openspec/specs/spec-engine-symbol-definition.md`

No prior specs with these names existed, so each was copied directly.

## TDD Compliance

Strict TDD was observed:

- Every behavior change was preceded by a failing test (RED).
- Implementation commits (GREEN) made the new tests pass.
- Refactor commits updated comments and aligned the config without adding untested behavior.
- 6 parse-error unit tests cover distinct error shapes; integration tests exercise `aiEcho` definition, references, and clean-file diagnostic clearing.

## Notes for Audit

- **Pre-existing noise**: The branch contains accidental plugin-language-id commits and their reverts; they have zero net diff and should not appear in a final merge review.
- **Out-of-scope failures**: The roundtrip semantic fixture failures are tracked separately and not addressed here.
- The source files under `mod/` and plugin Kotlin code were not modified.

## Rollback

Reverting the Rust-only commits restores the previous `cargo test` baseline and prior plugin runtime behavior. No deployed XS scripts or game files are affected.
