# Verify Report: fix-lsp-false-positive-forward-decl

## Verdict

PASS WITH WARNINGS

The implementation fixes Issue #2 correctly, all 215 tests pass, and every spec
scenario has a passing strict-TDD test. However, the working tree contains many
unrelated modified files that must be cleaned before any commit.

## Test results

| Command | Result | Notes |
|---------|--------|-------|
| `cargo test --no-fail-fast` | PASS | 215/215 tests pass (209 baseline + 6 new from `forward_decl_repro.rs`) |
| `cargo build --tests` | PASS | Exit 0 with pre-existing warnings only |
| `cargo clippy --all-targets` | PASS with warnings | 0 new warnings introduced by this change; only pre-existing warnings remain |
| `cargo run --bin lsp_roundtrip_test` | PASS | Exit 0; all LSP messages and semantic fixtures behave correctly |
| `AOMR_GAME_PATH=... cargo test --test game_folder_parse` | SKIPPED | `AOMR_GAME_PATH` is not set in this environment; the 9 tests skip cleanly |

- Total tests pass: 215
- New tests pass: 6 (`tests/forward_decl_repro.rs`)
- Build: clean (exit 0)
- Clippy: warnings are all pre-existing; none introduced by this change
- lsp_roundtrip_test: pass
- AOMR_GAME_PATH integration: skipped

## Spec coverage

| Scenario | Test | Status |
| -------- | ---- | ------ |
| R1 — Open vanilla `main.xs` in a mod overlay | `test_vanilla_main_xs_in_mod_overlay_produces_zero_false_positives` | PASS |
| R2 — Real same-file forward-declaration error remains detected | `test_real_same_file_forward_decl_error_still_detected` | PASS |
| R3 — Mutable redefinition ordering is preserved across a transitive include | `test_mutable_function_called_before_redefinition_does_not_emit` | PASS |
| R4 — Cyclic include chain does not infinite-loop | `test_cyclic_includes_do_not_infinite_loop` | PASS |
| R5 — Multiple root include edges take the first reaching line | `test_multiple_root_includes_takes_minimum_effective_line` | PASS |
| R6 — Existing integration test still passes | `tests/game_folder_parse.rs` (9 tests, skipped without `AOMR_GAME_PATH`) | PASS / SKIPPED |
| R7 — Strict-TDD regression test added for Issue #2 | `test_vanilla_main_xs_in_mod_overlay_produces_zero_false_positives` (covers the Issue #2 layout) | PASS |

## TDD compliance

| Check | Result | Details |
|-------|--------|---------|
| TDD Evidence reported | WARNING | Apply-progress Engram observation describes the work but does **not** contain a formal "TDD Cycle Evidence" table as required by strict-TDD verify protocol. `red-evidence.md` does capture RED-phase output. |
| All tasks have tests | PASS | 6/6 new tests exist in `tests/forward_decl_repro.rs`. |
| RED confirmed (tests exist) | PASS | `red-evidence.md` exists and records the failing `test_vanilla_main_xs_in_mod_overlay_produces_zero_false_positives` before the fix, plus compile-time RED for `effective_line` accessors. |
| GREEN confirmed (tests pass) | PASS | All 6 new tests pass on the fixed code. |
| Triangulation adequate | PASS | Issue #2 scenario is covered by a dedicated test; edge cases (same-file, mutable, cycles, multi-root, own-file) each have separate tests. |
| Safety Net for modified files | PASS | Full 209-baseline test suite passed before / after the change; no existing tests were broken. |

**Assertion quality**: All assertions verify real behavior (diagnostic counts, provenance variants, `effective_line` values). No tautologies or vacuous checks were found.

## Adversarial findings

1. **Cycle handling — PASS.** `test_cyclic_includes_do_not_infinite_loop` runs in `std::thread::scope` and completes in ~0.03 s. The code uses a `HashSet<PathBuf> visited` guard; files are inserted into `visited` before recursing, so re-entry terminates immediately. A cycle is recorded in `graph.cyclic`.

2. **Multiple-root handling — PASS.** `test_multiple_root_includes_takes_minimum_effective_line` verifies that when the same target is included on lines 5 and 10, the symbol's `effective_line` is 5. Static review confirms this because `MergedView::build` walks `extract_include_directives` in source order and the `visited` guard skips the later root edge, leaving the first (earliest) `effective_line` in place.

3. **Mutable redefinition — PASS.** `test_mutable_function_called_before_redefinition_does_not_emit` passes. In `semantic.rs` both `resolve_callee` and `forward_callable_merged` check `ms.symbol.is_mutable` before the line-order check, so a `mutable` declaration remains forward-callable regardless of `effective_line`.

4. **Same-file definitions — PASS.** `test_definition_in_current_file_uses_definition_line` passes. `MergedSymbol::effective_line()` returns `symbol.selection_range.start.line` for `VisibilityProvenance::OwnFile`, so own-file ordering is unchanged.

5. **Backward compatibility — PASS.** The full 215-test suite is green. The existing same-file forward-decl tests (`semantic::tests::missing_forward_declaration_is_error`, `semantic::tests::call_before_include_is_error`) still produce the expected diagnostics, and no existing test had to be reinterpreted or suppressed.

6. **Working tree cleanliness — STAGING BUG.** `git diff --stat HEAD` and `git status --short` show many unrelated modified and untracked files beyond the intended scope of this change:
   - Unstaged modifications: `scripts/deploy-mods.sh`, `tools/xs-language-server/src/bin/{day1_probe,dump_top_level,inspect_tree}.rs`, `cache.rs`, `completion.rs`, `definition_check.rs`, `diagnostics.rs`, `doxygen.rs`, `engine_api.rs`, `lib.rs`, `main.rs`, `parser.rs`, `references.rs`, `typecheck.rs`, `word.rs`, `workspace.rs`, `tests/game_folder_parse.rs`.
   - Unrelated untracked files: `docs/code-reviews/2026-06-29-lsp-and-plugin-findings.csv`, `docs/research/map_awareness_research.md`, `mod/spire_ai/`, `mod/test_targeting/`, `scripts/encode_tactic_xmb.py`, `scripts/find_units_with_tag.py`.
   - Some intended change artifacts are also untracked and should be staged: `design.md`, `explore.md`, `proposal.md`, and `specs/` under `openspec/changes/fix-lsp-false-positive-forward-decl/`.

   This is a severe staging problem. **Do not commit as-is**; the unrelated changes must be reverted or moved to their own changes, and the intended change files must be carefully staged.

7. **Issue #2 resolved note path is incorrect.** `docs/issues/2026-06-29-runtime-issues.md` line 100 says resolved by `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/`, but the actual change folder is `openspec/changes/fix-lsp-false-positive-forward-decl/`. The note should be corrected.

## Working tree state

- Diff vs HEAD includes intended files:
  - `tools/xs-language-server/src/merged_view.rs`
  - `tools/xs-language-server/src/semantic.rs`
  - `tools/xs-language-server/tests/forward_decl_repro.rs`
  - `docs/issues/2026-06-29-runtime-issues.md`
  - `openspec/changes/fix-lsp-false-positive-forward-decl/{CHANGELOG.md,red-evidence.md,tasks.md,verify-report.md}` (staged)
  - `openspec/changes/fix-lsp-false-positive-forward-decl/{design.md,explore.md,proposal.md,specs/}` (untracked but intended)
- Staging: **has-unrelated-files** (listed above)
- `pluginVersion`: not bumped — `git diff HEAD -- tools/intellij-xs-plugin/gradle.properties` is empty
- `tools/intellij-xs-plugin/`: no changes

## Deviations from design

None in the production code. The implementation matches the design:
- `effective_line: u32` added to both include variants of `VisibilityProvenance`.
- `walk_includes` and `add_included_symbols` propagate `effective_line` from the first root include.
- `VisibilityProvenance::effective_line()` and `MergedSymbol::effective_line()` added.
- The three `semantic.rs` line-order sites use `ms.effective_line()`.
- `MergedView::visibility_line` returns `effective_line`.

Documentation deviations:
- Issue #2 resolved note points to a non-existent `archive/` path.
- Apply-progress artifact lacks the formal strict-TDD cycle table.

## Risks

1. **Staging risk (HIGH).** Committing now would include many unrelated changes. The working tree must be cleaned before this change is committed.
2. **Unresolved game-folder integration.** `AOMR_GAME_PATH` is unavailable here, so the R6 zero-count guarantees were not exercised on real game files. They should be run in an environment with the game installed.
3. **`effective_line` wins on first DFS edge.** The design explicitly accepts this; if the include graph is later reordered (e.g. BFS), the R5 guarantee would change.

## Recommendation

Do **not** commit from the current working tree. Instead:

1. Revert or stash all unrelated changes (`scripts/deploy-mods.sh`, all `tools/xs-language-server/src/` files except `merged_view.rs` and `semantic.rs`, `tests/game_folder_parse.rs`, and the untracked out-of-scope files).
2. Correct the Issue #2 resolved note path in `docs/issues/2026-06-29-runtime-issues.md`.
3. Stage the intended files only:
   - `tools/xs-language-server/src/merged_view.rs`
   - `tools/xs-language-server/src/semantic.rs`
   - `tools/xs-language-server/tests/forward_decl_repro.rs`
   - `docs/issues/2026-06-29-runtime-issues.md`
   - `openspec/changes/fix-lsp-false-positive-forward-decl/{CHANGELOG.md,design.md,explore.md,proposal.md,red-evidence.md,specs/,tasks.md,verify-report.md}`
4. Commit with the agreed message: `fix(xs-lsp): use current-file include line for forward-decl ordering (Issue #2)`.
5. When a game installation is available, re-run the game-folder integration test with `AOMR_GAME_PATH` set.
