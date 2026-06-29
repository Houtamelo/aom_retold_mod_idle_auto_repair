# Archive Report: `fix-lsp-test-honesty`

**Date:** 2026-06-29
**Change Name:** `fix-lsp-test-honesty`
**Status:** Complete (PASS WITH WARNINGS in verify)
**Mode:** Strict TDD (user override of project-level `strict_tdd: false`)

## Change Summary

The LSP end-to-end gate `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` now parses JSON-RPC frames instead of scanning raw stdout byte substrings. An inline `wire_helpers` module exposes Content-Length-framed parsing and typed message queries; the three dishonest assertions tracked as R5-F-01 (completion counting), R5-F-02 (references timing), and R5-F-03 (diagnostics flags) were rewritten against parsed messages. Five strict-TDD contract tests in `tests/r5_test_honesty_repro.rs` exercise each scenario and the timing window was narrowed to exclude the post-response 50 ms sleep, shutdown/exit roundtrip, and `child.wait()` cleanup.

## Specs Synced

| Domain | Action | Details |
|--------|--------|---------|
| `spec-lsp-roundtrip-test-honesty` | Created | New capability spec with 3 scenarios and 4 acceptance criteria; applies `spec-game-folder-test-coverage.md:61` classifier rule to the roundtrip gate |

## Source of Truth Updated

The following spec now reflects the new behavior:

- `openspec/specs/spec-lsp-roundtrip-test-honesty.md` — new canonical spec promoted from `openspec/changes/fix-lsp-test-honesty/specs/`. It references the no-string-matching principle in `openspec/specs/spec-game-folder-test-coverage.md:61` (spec line 8).

## Archive Contents

- proposal.md ✅
- specs/spec-lsp-roundtrip-test-honesty.md (full spec) ✅
- design.md ✅
- tasks.md ✅ (1/1 work unit complete; all Phase 1–4 tasks done)
- verify-report.md ✅ (PASS WITH WARNINGS)
- archive-report.md ✅ (this file)

## File Changes (Production)

| File | Action | Lines Changed |
|------|--------|---------------|
| `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` | Modified | +438/-160 |
| `tools/xs-language-server/tests/r5_test_honesty_repro.rs` | Modified | +80/-20 (approx.; transformed 4 reproductions + 1 new live integration test + 1 removed audit test) |
| `openspec/specs/spec-lsp-roundtrip-test-honesty.md` | Created | 70 lines |
| `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` | Modified | +4 resolution markers + 1 last-verified line |

## Synced Delta Spec Details

Step 1 of archive copied `openspec/changes/fix-lsp-test-honesty/specs/spec-lsp-roundtrip-test-honesty.md` to `openspec/specs/spec-lsp-roundtrip-test-honesty.md`. The copy was verified byte-identical with `cmp` (5204 bytes). The new spec is a full capability document, not a delta, so no requirements were merged into an existing file. It cross-references the classifier rule at `openspec/specs/spec-game-folder-test-coverage.md:61`.

## Review Markdown Updates

Step 2 of archive updated `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md`:

- **Top concerns list, line 28:** Added `[Resolved 2026-06-29]` marker after `**R5-F-01, R5-F-03**`, tied to this archive folder.
- **HIGH findings heading, line 453:** `### R5-F-01 · test · lsp_roundtrip_test.rs:401-411` marked `[Resolved 2026-06-29]`.
- **HIGH findings heading, line 470:** `### R5-F-02 · test · lsp_roundtrip_test.rs:1659-1676` marked `[Resolved 2026-06-29]`.
- **HIGH findings heading, line 488:** `### R5-F-03 · test · lsp_roundtrip_test.rs:362-380` marked `[Resolved 2026-06-29]`.
- **End of document, line 908:** Added a second `Last verified` line tying R5-F-01/02/03 to this archive folder and the 203/203 PASS WITH WARNINGS result.

The companion CSV (`docs/code-reviews/2026-06-29-lsp-and-plugin-findings.csv`) has no status column, so no CSV edits were required.

## Project Context Updates

Step 3 was skipped: `openspec/sdd-init/aom_retold_mod_idle_auto_repair.md` documents the original XS-modding project context and contains no Rust LSP test-count line. The new count is recorded in this archive report instead.

## Deviations from Design

All six ADRs were honoured in substance. Two minor deviations were already documented in `verify-report.md` §4 and are reproduced here for the audit trail:

1. **ADR-5 timer placement (R5-F-02)** — Design placed `let start = Instant::now()` immediately *after* the references `stdin.flush()`. The implementation places it one line *before* the references `write_all`/`flush` (`src/bin/lsp_roundtrip_test.rs:1919-1921`). The captured interval still excludes the post-write 50 ms sleep, shutdown/exit roundtrip, and `child.wait()` cleanup, and the live R5-F-02 contract test passes the 5 ms tolerance. This is a documented harmless ordering nuance with no semantic impact.
2. **Test naming suffix (ADR-4)** — Design table used `_derives_from_parsed_notification` for the two R5-F-03 tests; implementation uses `_asserts_parsed_notification`. Semantic content is identical.
3. **Line-count inflation (Phase 3)** — Binary diff was +438/-160 instead of the forecasted ~+180/-25 because Phase 3 included cosmetic `eprintln!` reformatting. No behavior change or scope creep.

## Verification Outcome

- **203 tests** passing (182 unit + 9 game-folder integration + 7 R3-F-01 + 5 R5).
- 5 new/transformed tests cover the strict-TDD contract: 2 completion-count tests, 2 diagnostics tests, and 1 live references timing test.
- The roundtrip binary exits 0 and prints the corrected label: `responded {} aiEcho location(s) in {} ms (request to response)`.
- All spec scenarios are covered by passing tests.
- All design decisions matched in code, subject to the documented deviations above.

## Review Workload Forecast

- Estimated changed lines: ~180 forecasted; ~438/+160 actual due to Phase 3 cosmetic reformatting (still under 400-line effective behavior budget).
- Chained PRs: No
- Chain strategy: pending (no chains needed)
- Decision needed before apply: No

## SDD Cycle Status

The change has been fully:

1. **Planned** (proposal + spec + design + tasks) ✅
2. **Implemented** (apply, with strict TDD) ✅
3. **Verified** (verify, PASS WITH WARNINGS) ✅
4. **Archived** (this file) ✅

Ready for the next change.

## Engram References

- Archive report observation: `#1422` (`sdd/fix-lsp-test-honesty/archive-report`)
- Apply-progress / verification recovery observation: `#1420` (captured by orchestrator-side recovery; confirms RED→GREEN transitions and the 203/203 test result)

## Open Follow-ups (out of scope for this change)

- This archive clears Batch D of the user's per-finding workflow: **R5-F-01, R5-F-02, R5-F-03**.
- **142 findings remain** from the 2026-06-29 review (145 total minus R3-F-01, R5-F-01/02/03). The next recommended batch is the remaining 3 CRITICAL findings:
  - **R7-F-02** — race on `XsBinaryResolver.cachedBundledPath` (double-checked locking without a lock).
  - **R7-F-13** — `XsStartupActivity.runActivity` scans the project tree on the EDT and can freeze the IDE.
  - If those are deferred, the next HIGH batch could address **R3-F-09** (misleading "internal error: failed to install XS language" message surfaced on user typos) or **R1-F-04** (UTF-16 vs byte offset in `identifier_at_cursor`) — both are small, user-visible, and self-contained.
- One new test-side clippy warning remains at `tests/r5_test_honesty_repro.rs:298` (`collapsible_if`). It mirrors a pre-existing warning in the production helper and is cosmetic; it can be folded into a future clippy-cleanup change.
