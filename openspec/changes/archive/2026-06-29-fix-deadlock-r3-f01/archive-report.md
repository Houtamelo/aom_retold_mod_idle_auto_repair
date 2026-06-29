# Archive Report: `fix-deadlock-r3-f01`

**Date:** 2026-06-29
**Change Name:** `fix-deadlock-r3-f01`
**Status:** Complete (PASS WITH WARNINGS in verify)
**Mode:** Strict TDD (user override of project-level `strict_tdd: false`)

## Change Summary

Defensive lock-order refactor in `tools/xs-language-server/src/server.rs`'s `did_close` handler. The cyclic lock-acquisition pattern in source (which was statically unreachable but latent) was eliminated by extracting `did_close_lock_pattern` with scoped-block mutex holds, adding a contributor-facing doc-comment on the `XsLanguageServer` struct, and writing 4 new strict-TDD tests that prove the no-simultaneous-mutex-hold invariant.

## Specs Synced
| Domain | Action | Details |
|--------|--------|---------|
| spec-lsp-server-lifecycle | Updated | 1 added, 0 modified, 0 removed |

## Source of Truth Updated
The following spec now reflects the new behavior:
- `openspec/specs/spec-lsp-server-lifecycle.md` — appended `### Requirement: Single-Mutex Hold Across Await Points` with two scenarios.

## Archive Contents
- proposal.md ✅
- specs/spec-lsp-server-lifecycle.md (delta) ✅
- design.md ✅
- tasks.md ✅ (11/11 tasks complete)
- verify-report.md ✅ (PASS WITH WARNINGS)
- archive-report.md ✅ (this file)

## File Changes (Production)
| File | Action | Lines Changed |
|------|--------|---------------|
| `tools/xs-language-server/src/server.rs` | Modified | +82/-5 |
| `tools/xs-language-server/tests/r3_f01_deadlock_repro.rs` | Modified | +289/-19 |
| `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` | Modified | +3 (R3-F-01 resolution markers) |

## Verification Outcome
- **198 tests** passing (182 unit + 9 game-folder integration + 7 deadlock-repro integration).
- 4 new tests cover the strict-TDD invariant: counter check, regression-detection, production invariant, triangulation.
- All spec scenarios covered by passing tests.
- All design decisions matched in code.

## Review Workload Forecast
- Estimated changed lines: ~100 (well under 400-line budget)
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

## Open Follow-ups (out of scope for this change)
- The `cargo clippy --all-targets -- -D warnings` failure is due to **pre-existing** warnings in unrelated files (`typecheck.rs`, `semantic.rs`, `cache.rs`). The R3-F-01 fix introduces no new warnings. A future clippy-cleanup SDD change could address these.
- The remaining 144 findings from the 2026-06-29 code review are documented in `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` and `docs/code-reviews/2026-06-29-lsp-and-plugin-findings.csv`. Each can be addressed by an SDD change following the same workflow as this one.
