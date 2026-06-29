# Spec: LSP forward-declaration honesty

> **Added/updated by change:** `fix-lsp-false-positive-forward-decl`

## Capability summary

The LSP server SHALL report a "used before declaration" diagnostic only when a call site in the analyzed file appears before the earliest `include` line in that file that makes the callee visible. For symbols reached through transitive includes, the comparison SHALL use the effective line in the currently analyzed file, not the line of an intermediate file's `include` directive. Real forward-declaration errors in the same file and `mutable`-redefinition ordering SHALL continue to be enforced.

## Rationale

Issue #2 (see `docs/issues/2026-06-29-runtime-issues.md` lines 97-180) reproduces on a clean copy of the vanilla AoM:R `game/ai/` tree placed in a mod overlay. The merged view resolves symbols such as `setupDebugCategories` correctly from transitively-included files, but `semantic.rs` compares the call-site line against `VisibilityProvenance::include_line()`, which for transitive includes records the line inside the intermediate includer, not the line inside `main.xs` where the include chain starts. Calls that appear after the direct include but before the intermediate-file line are therefore flagged incorrectly.

Approach (a) — precomputing an `effective_line` field on `VisibilityProvenance` at merged-view build time — fixes the comparison without rewinding the `IncludeGraph` on every keystroke and keeps the line-order invariant in one place.

## XS-engine constraint

AoM:R's `include "..."` directive is modeled as a textual paste. A symbol becomes visible in the analyzed file at the first `include` line in that file that reaches the symbol, whether directly or transitively. The engine's exact textual-paste semantics are otherwise undocumented; this spec assumes the conservative model "visible at the earliest current-file include line that reaches it."

## Scenarios

### Scenario: R1 — Open vanilla `main.xs` in a mod overlay

- GIVEN the vanilla AoM:R `game/ai/` tree has been copied into `mod/spire_ai/game/ai/`
- AND the user opens `mod/spire_ai/game/ai/core/main.xs`
- WHEN the LSP server publishes diagnostics for that file
- THEN zero diagnostics of the form `'X' used at line N before declaration; add forward declaration or mark 'mutable'` SHALL be produced
- AND the same zero-diagnostic guarantee SHALL hold for any other unmodified vanilla file opened from a mod overlay as long as it relies only on its own include chain.

### Scenario: R2 — Real same-file forward-declaration error remains detected

- GIVEN a file contains a call to a user-defined function before any definition, declaration, or include that makes it visible
- AND `updateBreakdown` in `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs` is the canonical example (see `docs/post-lsp-migration-issues.md` Category D, lines 122-143)
- WHEN the LSP server publishes diagnostics for that file
- THEN the call site SHALL still produce a `'updateBreakdown' used at line N before declaration; add forward declaration or mark 'mutable'` diagnostic.

### Scenario: R3 — Mutable redefinition ordering is preserved across a transitive include

- GIVEN a `mutable` function is defined in an included file
- AND the analyzed file includes that file and calls the function before any later redefinition of the same function
- WHEN the LSP server checks forward declarations
- THEN no `used before declaration` diagnostic SHALL be emitted for that call
- BECAUSE `mutable` is the explicit XS exception that makes a function forward-callable (see `AGENTS.md` line 43).

### Scenario: R4 — Cyclic include chain does not infinite-loop

- GIVEN `a.xs` includes `b.xs` and `b.xs` includes `a.xs`
- WHEN the merged view is built for `a.xs`
- THEN the traversal SHALL terminate in bounded time
- AND effective-line computation SHALL stop at the already-visited cycle edge.

### Scenario: R5 — Multiple root include edges take the first reaching line

- GIVEN `main.xs` reaches file `A` through an `include` directive on line 5
- AND `main.xs` also reaches file `A` through a different `include` directive on line 10
- WHEN the effective line for symbols from `A` is computed
- THEN the effective line SHALL be 5, the first reaching edge encountered during merged-view construction.

### Scenario: R6 — Existing integration test still passes

- GIVEN the game folder is installed and `AOMR_GAME_PATH` is set
- WHEN `tests/game_folder_parse.rs` runs after the fix
- THEN all tracked counts SHALL remain zero:
  - `duplicate_extern_count`
  - `wrong_diagnostic_uri_count`
  - `unresolved_symbol_count`
  - `wrong_arg_count_count`
  - `rule_call_unresolved_count`
  - `total_diagnostic_count`.

### Scenario: R7 — Strict-TDD regression test added for Issue #2

- GIVEN a new test fixture exercising the Issue #2 transitive-include layout (for example, `tools/xs-language-server/tests/issue2_repro.rs` or an equivalent inline test)
- AND the fixture mirrors the layout: `main.xs` includes `core/core.xs`, `core/core.xs` includes `helper.xs` at a late line, and `main.xs` calls the helper shortly after its own `include "core/core.xs"`
- WHEN the test runs
- THEN on the unfixed code it SHALL fail by producing at least one false-positive `used before declaration` diagnostic
- AND on the fixed code it SHALL pass with zero such diagnostics.

## Cross-references

- **User issue:** `docs/issues/2026-06-29-runtime-issues.md` Issue 2 (lines 97-180).
- **Audit overlap:** `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` finding **R3-F-04** (`server.rs:699-764`) touches the merged-view / include-resolution area but concerns reference-set scope, not line ordering. This change does NOT address R3-F-04 and SHALL be tracked as a separate SDD change.
- **Language rule:** `AGENTS.md` line 43: "Forward declarations are required. XS does NOT support implicit forward declarations like C/C++. A function must be either defined before it is called, OR declared (signature only) with a trailing semicolon earlier in the file. A function marked `mutable` is the only exception — it can be redefined later and is forward-callable." This spec does not relax that rule; it only makes the LSP's line-order check compare against the correct current-file line.
- **Explore report:** `openspec/changes/fix-lsp-false-positive-forward-decl/explore.md` documents the root cause in `semantic.rs:820-846`, `semantic.rs:367`, and `semantic.rs:207`.

## Out of scope

- Issues 1, 3, and 4 from `docs/issues/2026-06-29-runtime-issues.md`.
- Duplicate-`extern` diagnostics observed while reproducing the Issue #2 project layout.
- Grammar, parser, or tree-sitter changes.
- IntelliJ plugin / Kotlin changes.
- R3-F-04 engine-API references scope.
- Any change to XS mod packages under `mod/`.

## Verification approach

- Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml`; the full suite SHALL remain green.
- Run the Issue #2 reproduction test and confirm zero false-positive `used before declaration` diagnostics on the vanilla `main.xs` overlay layout.
- Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test game_folder_parse -- --nocapture` with `AOMR_GAME_PATH` set and confirm the counts in Scenario R6 are all zero.
- Confirm a real same-file forward-declaration fixture (e.g., `updateBreakdown`) still produces the expected diagnostic.

## Acceptance criteria

1. `VisibilityProvenance::DirectInclude` / `TransitiveInclude` SHALL carry an `effective_line` field (or accessor) representing the current-file line at which the symbol becomes visible.
2. `merged_view.rs` SHALL populate `effective_line` during `MergedView::build` / `walk_includes` from the outermost include line in the analyzed file.
3. The three `semantic.rs` line-order check sites (`resolve_callee`, `effective_line`, and `forward_callable_merged` per `explore.md`) SHALL compare against the effective current-file line.
4. Opening a mod-overlay copy of `game/ai/core/main.xs` SHALL produce zero false-positive "used before declaration" diagnostics.
5. A real same-file `updateBreakdown` forward-declaration error SHALL continue to produce its diagnostic.
6. A `mutable` function called before its redefinition via a transitive include SHALL produce no false-positive diagnostic.
7. A cyclic include chain SHALL terminate without infinite loops.
8. When the same file is reachable through multiple root include lines, the first reaching edge encountered during build SHALL determine `effective_line`.
9. `tests/game_folder_parse.rs` SHALL continue to pass with all tracked counts at zero.
10. A new strict-TDD regression test for Issue #2 SHALL be added and pass.

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `fix-lsp-false-positive-forward-decl` | 2026-06-29 | Spec phase | New spec formalizing the effective-line rule for forward-declaration diagnostics. |
