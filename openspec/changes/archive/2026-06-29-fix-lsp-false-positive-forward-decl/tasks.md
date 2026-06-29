# Tasks: fix-lsp-false-positive-forward-decl — Use current-file include line for forward-decl ordering

> ⚠️ **DECISION NEEDED BEFORE APPLY**
>
> `AGENTS.md` requires a patch bump of `tools/intellij-xs-plugin/gradle.properties` for any change to `tools/xs-language-server/` Rust sources, because the bundled LSP artifact changes. The change design also instructs bumping `pluginVersion` from `0.2.2` to `0.2.3`. However, the task directive for this change says **NO plugin version bump** because no Kotlin/plugin-side files change. Resolve this conflict before the apply phase.
>
> Decision needed before apply: Yes
> Chained PRs recommended: No
> Chain strategy: pending
> 400-line budget risk: Low

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~220–300 (+80/−30 production, +150 tests/docs) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Chain strategy | pending |

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Add `effective_line` to `VisibilityProvenance`, propagate it during merged-view build, and update `MergedView::visibility_line` | PR 1 | Against `main`; includes new regression tests |
| 2 | Switch the three `semantic.rs` line-order checks to use `effective_line` and reinterpret any broken existing test | PR 1 | Same PR, same work unit because the semantic check sites depend on MergedSymbol::effective_line |

## Phase 0: Setup

### Task 0.1: Confirm branch state
**Effort:** 2 min **Risk:** low  
**Depends on:** none
- [x] Run `git status --short` and `git log --oneline -5`.
- [x] Ensure no unrelated, unstaged work will leak into the commit.
- **Verification:** A clean understanding of working-tree state. If unexpected dirty files exist, stage/revert them before RED tests.

### Task 0.2: Confirm `cargo build --tests` is clean
**Effort:** 5–10 min **Risk:** low  
**Depends on:** 0.1
- [x] Run `cargo build --tests --manifest-path tools/xs-language-server/Cargo.toml`.
- [x] Confirm exit code `0` and no new compiler errors/warnings (pre-existing warnings are OK per Batch D precedent).
- **Verification:** Build succeeds.

### Task 0.3: Confirm test count baseline
**Effort:** 2 min **Risk:** low  
**Depends on:** 0.2
- [x] Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --list` and record the baseline count (expected 209 per `AGENTS.md`).
- **Verification:** Baseline recorded; after adding the new test file the expected total is 215.

## Phase 1: RED — failing tests first

### Task 1.1: Write `tools/xs-language-server/tests/forward_decl_repro.rs`
**Effort:** 45 min **Risk:** medium  
**Depends on:** 0.3
- [x] Create `tools/xs-language-server/tests/forward_decl_repro.rs`.
- [x] Add a small helper that writes inline fixture strings to a temporary `game/ai/` tree, builds a `semantic::VirtualProject`, a `Workspace`, and a `MergedView`, and runs the forward-declaration check publicly exported from `xs_language_server::semantic`.
- [x] Add these six strict-TDD tests:
  - `test_vanilla_main_xs_in_mod_overlay_produces_zero_false_positives` — mirrors the Issue #2 layout: `main.xs` includes `core/core.xs` early and calls a function defined in a transitively-included file; assert zero `"before declaration"` diagnostics.
  - `test_real_same_file_forward_decl_error_still_detected` — `void foo() { bar(); }` before `void bar() {}`; assert one `"before declaration"` diagnostic for `bar`.
  - `test_mutable_function_called_before_redefinition_does_not_emit` — included file has `mutable void helper() {}` then `void helper() {}`; analysed file includes it and calls `helper()` after the include; assert zero diagnostics.
  - `test_cyclic_includes_do_not_infinite_loop` — `a.xs` includes `b.xs`, `b.xs` includes `a.xs`; build merged view and assert `graph().is_cyclic()` within a short timeout.
  - `test_multiple_root_includes_takes_minimum_effective_line` — analysed file includes the same target on line 5 and line 10; assert the symbol's `effective_line` is 5.
  - `test_definition_in_current_file_uses_definition_line` — own-file symbol is visible; assert `MergedSymbol::effective_line()` equals `symbol.selection_range.start.line`.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test forward_decl_repro` compiles the test file.

### Task 1.2: Run the six new tests and confirm RED
**Effort:** 5 min **Risk:** low  
**Depends on:** 1.1
- [x] Run the six tests against the unfixed code.
- **Verification:** All six tests FAIL. At minimum, the Issue #2 mirror test must fail with at least one `"before declaration"` diagnostic; the own-file/effective-line tests must fail because `effective_line()` does not yet exist.

### Task 1.3: Record failure evidence
**Effort:** 5 min **Risk:** low  
**Depends on:** 1.2
- [x] Copy the failing test names and key assertion output into a scratch note in the change folder.
- **Verification:** Evidence file (e.g. `openspec/changes/fix-lsp-false-positive-forward-decl/red-evidence.md`) exists and contains the Issue #2 false-positive diagnostic output.

## Phase 2: GREEN — minimal fix in `merged_view.rs`

### Task 2.1: Add `effective_line: u32` to the include variants of `VisibilityProvenance`
**Effort:** 10 min **Risk:** low  
**Depends on:** 1.3
- [x] In `tools/xs-language-server/src/merged_view.rs`, add `effective_line: u32` to `VisibilityProvenance::DirectInclude` and `VisibilityProvenance::TransitiveInclude`.
- **Verification:** `cargo build --manifest-path tools/xs-language-server/Cargo.toml` still compiles (field unused at this point).

### Task 2.2: Initialize `effective_line` to a safe default
**Effort:** 10 min **Risk:** low  
**Depends on:** 2.1
- [x] Initialize the new field with `effective_line: u32::MAX` in every constructor site, so a missing propagation path fails loudly by treating the symbol as invisible.
- **Verification:** Build still compiles.

### Task 2.3: Thread `effective_line` through `build` and the DFS walk
**Effort:** 20 min **Risk:** medium  
**Depends on:** 2.2
- [x] Update `walk_includes` signature to accept `effective_line: u32`.
- [x] In `MergedView::build`, pass the analysed-file include line `L` as `effective_line` to `add_included_symbols` and `walk_includes`.
- [x] In `walk_includes`, keep the inherited `effective_line` when recurring; `include_line` still records the intermediate-file line.
- [x] Update `add_included_symbols` to accept and record `effective_line` in both `DirectInclude` and `TransitiveInclude`.
- **Verification:** `cargo build --manifest-path tools/xs-language-server/Cargo.toml` compiles.

### Task 2.4: Confirm cycle handling via the existing `visited` guard
**Effort:** 5 min **Risk:** low  
**Depends on:** 2.3
- [x] Verify that `visited` is checked before recursing and before calling `add_included_symbols` a second time for the same file, so cycle-closing edges do not overwrite `effective_line`.
- **Verification:** The cyclic-include unit test is still loadable; no code change is required if the guard is already in place.

### Task 2.5: Confirm first-source-order root edge wins
**Effort:** 10 min **Risk:** low  
**Depends on:** 2.3
- [x] Verify that `parser::extract_include_directives` returns directives in source order and that `MergedView::build` walks them in that order.
- [x] Verify that because files are added to `visited` before their children are walked, the first reaching root include sets `effective_line` and later root includes hit `visited` and skip.
- **Verification:** No code change required; behavior follows from existing DFS + visited guard.

### Task 2.6: Add `VisibilityProvenance::effective_line()` and `MergedSymbol::effective_line()`
**Effort:** 15 min **Risk:** low  
**Depends on:** 2.3
- [x] Add `pub fn effective_line(&self) -> u32` on `VisibilityProvenance` that returns the field for include variants and `0` for `OwnFile`.
- [x] Add `pub fn effective_line(&self) -> u32` on `MergedSymbol`:
  - `OwnFile` → `self.symbol.selection_range.start.line`
  - include variants → `self.provenance.effective_line()`
- **Verification:** `cargo build --manifest-path tools/xs-language-server/Cargo.toml` compiles.

### Task 2.7: Update `MergedView::visibility_line` to return `effective_line`
**Effort:** 5 min **Risk:** low  
**Depends on:** 2.6
- [x] Change `tools/xs-language-server/src/merged_view.rs:435` from `ms.provenance.include_line()` to `ms.effective_line()`.
- **Verification:** Build compiles and any existing merged-view tests still pass.

### Task 2.8: Run the six RED tests and confirm GREEN
**Effort:** 5 min **Risk:** medium  
**Depends on:** 2.7
- [x] Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test forward_decl_repro`.
- **Verification:** All six new tests PASS.

### Task 2.9: Run the full `cargo test` suite and confirm 215 tests
**Effort:** 10 min **Risk:** low  
**Depends on:** 2.8
- [x] Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml`.
- **Verification:** 215 tests pass (209 baseline + 6 new).

## Phase 3: GREEN — update `semantic.rs` line-order checks

### Task 3.1: Update `resolve_callee` to use `effective_line`
**Effort:** 10 min **Risk:** medium  
**Depends on:** 2.9
- [x] In `tools/xs-language-server/src/semantic.rs` (`resolve_callee`), replace the combined `include_line()` + `OwnFile` special case with `ms.effective_line() < call_line`.
- **Verification:** The existing `resolve_callee` unit tests still pass.

### Task 3.2: Update the `effective_line` helper in `semantic.rs`
**Effort:** 5 min **Risk:** low  
**Depends on:** 2.9
- [x] Replace the body of `fn effective_line(ms: &MergedSymbol)` with a single call to `ms.effective_line()`.
- **Verification:** Build compiles.

### Task 3.3: Update `forward_callable_merged`
**Effort:** 10 min **Risk:** medium  
**Depends on:** 2.9
- [x] In `tools/xs-language-server/src/semantic.rs` (`forward_callable_merged`), replace the `def_line` match with `let def_line = ms.effective_line();`.
- **Verification:** The Issue #2 reproducer test remains green; the same-file forward-decl test still fails.

### Task 3.4: Run the full test suite after `semantic.rs` changes
**Effort:** 10 min **Risk:** medium  
**Depends on:** 3.1, 3.2, 3.3
- [x] Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml`.
- **Verification:** The six new tests and the 209 baseline tests all pass.

## Phase 4: GREEN — reinterpret existing tests

### Task 4.1: Identify any broken existing tests
**Effort:** 10 min **Risk:** low  
**Depends on:** 3.4
- [x] Run the full test suite. If any existing test fails, capture:
  - test name,
  - file and line,
  - assertion output.
- **Verification:** List of broken tests (may be empty).

### Task 4.2: For each broken test, decide bug-vs-regression
**Effort:** 15 min **Risk:** medium  
**Depends on:** 4.1
- [x] For each failing test:
  - (a) If the test was asserting the old bug (raw `include_line()` ordering), rewrite the assertion to use correct semantics (`effective_line` for ordering, `include_line` only if the test explicitly checks the intermediate-file edge).
  - (b) If the test was asserting correct behavior, diagnose the production-code regression and fix it in the relevant Phase 2/3 task.
- **Verification:** Each broken test is either updated with a justified change or traced to a production fix.

### Task 4.3: Run the integration test `tests/game_folder_parse.rs`
**Effort:** 5 min (skip if no game) / 30 min (if game installed) **Risk:** low  
**Depends on:** 4.2
- [x] If `AOMR_GAME_PATH` is set, run `AOMR_GAME_PATH=... cargo test --manifest-path tools/xs-language-server/Cargo.toml --test game_folder_parse -- --nocapture`.
- [x] Confirm the counts in Scenario R6 remain zero (especially `total_diagnostic_count` and `unresolved_symbol_count`).
- **Verification:** Integration test passes or skips cleanly with recorded reason.

## Phase 5: REFACTOR — cleanup and documentation

### Task 5.1: Doc-comment `VisibilityProvenance.effective_line`
**Effort:** 5 min **Risk:** low  
**Depends on:** 3.4
- [x] Add a rustdoc comment in `tools/xs-language-server/src/merged_view.rs` explaining that `effective_line` is the 0-indexed line of the earliest `include` directive in the analysed file that reaches this symbol, and that it is stable for the lifetime of the `MergedView`.
- **Verification:** `cargo doc --manifest-path tools/xs-language-server/Cargo.toml --no-deps` builds without new warnings.

### Task 5.2: Doc-comment the include-line propagation algorithm
**Effort:** 5 min **Risk:** low  
**Depends on:** 5.1
- [x] Add/update module-level or function-level docs in `merged_view.rs` describing the DFS propagation rule, the `visited` guard for cycles, and the first-source-order edge wins rule.
- **Verification:** Doc build clean.

### Task 5.3: Doc-comment `semantic.rs::resolve_callee` on `effective_line`
**Effort:** 5 min **Risk:** low  
**Depends on:** 5.2
- [x] Update the `fn effective_line` helper comment and/or `resolve_callee` note to state that ordering uses the current-file include line, not the intermediate file's include line.
- **Verification:** Doc build clean; new comment accurately describes behavior.

## Phase 6: VERIFY

### Task 6.1: Full Rust test suite with `--no-fail-fast`
**Effort:** 10 min **Risk:** low  
**Depends on:** 4.3, 5.3
- [x] Run `cargo test --no-fail-fast --manifest-path tools/xs-language-server/Cargo.toml`.
- **Verification:** All 215+ tests pass.

### Task 6.2: Clean `cargo build --tests`
**Effort:** 5 min **Risk:** low  
**Depends on:** 6.1
- [x] Run `cargo build --tests --manifest-path tools/xs-language-server/Cargo.toml`.
- **Verification:** Exit code `0`.

### Task 6.3: Run `cargo clippy --all-targets`
**Effort:** 5 min **Risk:** low  
**Depends on:** 6.1
- [x] Run `cargo clippy --all-targets --manifest-path tools/xs-language-server/Cargo.toml`.
- [x] Note any new warnings introduced by this change; pre-existing warnings are acceptable.
- **Verification:** Clippy runs; new warnings recorded (ideally zero).

### Task 6.4: LSP roundtrip smoke test
**Effort:** 10 min **Risk:** low  
**Depends on:** 6.2
- [x] Run `cargo run --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test`.
- **Verification:** Exit code `0`.

### Task 6.5: Game-folder integration test
**Effort:** 5 min (skip) / 30 min (run) **Risk:** low  
**Depends on:** 6.4
- [x] If `AOMR_GAME_PATH` is set, run the integration test from Task 4.3 again; otherwise explicitly skip and note the skip.
- **Verification:** Test passes or skip recorded.

### Task 6.6: Write/upate the verify report
**Effort:** 10 min **Risk:** low  
**Depends on:** 6.5
- [x] Create `openspec/changes/fix-lsp-false-positive-forward-decl/verify-report.md` with the required verify summary (exact content to be filled by `sdd-verify`).
- **Verification:** File exists with at minimum a header, run commands, pass/fail summary, and any skipped steps.

## Phase 7: DOCUMENT

### Task 7.1: Update audit/code-review documentation
**Effort:** 5 min **Risk:** low  
**Depends on:** 6.6
- [x] Check `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` for an audit finding that corresponds to Issue #2 (false-positive "used before declaration" on vanilla files).
- [x] If a finding exists, add `[Resolved 2026-06-29]` and reference `openspec/changes/fix-lsp-false-positive-forward-decl/`.
- [x] If no corresponding finding exists (expected), add a one-line note to a new or existing `openspec/changes/fix-lsp-false-positive-forward-decl/CHANGELOG.md` instead.
- **Verification:** Either the code-review file contains a resolved marker, or the change folder contains a CHANGELOG note.

### Task 7.2: Update Issue 2 in `docs/issues/2026-06-29-runtime-issues.md`
**Effort:** 5 min **Risk:** low  
**Depends on:** 7.1
- [x] In `docs/issues/2026-06-29-runtime-issues.md`, in the Issue 2 section, add: "Resolved 2026-06-29 by `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/`."
- **Verification:** The Issue 2 section contains the resolved marker.

## Phase 8: COMMIT

### Task 8.1: Commit with a Conventional Commits message
**Effort:** 5 min **Risk:** low  
**Depends on:** 7.2
- [ ] Stage only the intended files:
  - `tools/xs-language-server/src/merged_view.rs`
  - `tools/xs-language-server/src/semantic.rs`
  - `tools/xs-language-server/tests/forward_decl_repro.rs`
  - doc updates from Phase 7
  - the verify report
- [ ] Commit with message: `fix(xs-lsp): use current-file include line for forward-decl ordering (Issue #2)`.
- **Verification:** `git log --oneline -1` shows the commit and `git diff --stat` is limited to the intended scope.

### Task 8.2: Do NOT bump `pluginVersion`
**Effort:** 2 min **Risk:** low  
**Depends on:** the pluginVersion decision resolved at the top of this file
- [x] Do not modify `tools/intellij-xs-plugin/gradle.properties` for this change.
- [x] Record the resolved decision (bump vs. no-bump) in the verify report so release notes stay consistent.
- **Verification:** `git diff -- tools/intellij-xs-plugin/gradle.properties` is empty.

## Review Workload Forecast

- Files changed: `tools/xs-language-server/src/merged_view.rs`, `tools/xs-language-server/src/semantic.rs`, documentation files, plus the new `tools/xs-language-server/tests/forward_decl_repro.rs`.
- New test files: `tools/xs-language-server/tests/forward_decl_repro.rs`.
- Total changed lines: Medium (~220–300).
- Chained PRs recommended: No.
- 400-line budget risk: Low.
- Decision needed before apply: Yes — whether to follow the `AGENTS.md` pluginVersion bump rule or the explicit no-bump directive for this change.
