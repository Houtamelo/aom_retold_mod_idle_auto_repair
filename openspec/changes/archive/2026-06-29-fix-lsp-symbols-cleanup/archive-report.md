# Archive Report: `fix-lsp-symbols-cleanup`

**Date:** 2026-06-29
**Change Name:** `fix-lsp-symbols-cleanup`
**Status:** Complete (PASS WITH WARNINGS in verify)
**Mode:** Strict TDD (user override of project-level `strict_tdd: false`)

## Change Summary

Single-file cleanup in `tools/xs-language-server/src/symbols.rs`. Deleted the dead `Param::render` method and its now-empty `impl Param` block, deleted the `is_false` serde helper and inlined it via `std::ops::Not::not`, and renamed the misleading test `recovers_function_definition_from_error_node_with_body` to `function_definition_with_inner_error_in_default_value_still_extracts`. The R1-F-02 forward-declaration grammar claim was refuted: direct parser inspection confirmed function forward declarations currently produce `ERROR` nodes in `tree-sitter-xs`, so the `extract_error_*` helpers remain required and were left untouched. Six strict-TDD audit-grade tests in `tools/xs-language-server/tests/symbols_cleanup_repro.rs` assert the post-fix state.

## Specs Synced

| Domain | Action | Details |
|--------|--------|---------|
| spec-lsp-symbol-honesty | Created | New spec promoted to `openspec/specs/`; no existing main spec to merge against |

## Source of Truth Updated

The following spec now reflects the new behavior:
- `openspec/specs/spec-lsp-symbol-honesty.md` — LSP symbol-extractor module honesty (dead methods and misleading test labels must be removed/renamed when surfaced).

## Archive Contents

- proposal.md ✅
- specs/spec-lsp-symbol-honesty.md (delta) ✅
- design.md ✅
- tasks.md ✅ (all tasks complete)
- verify-report.md ✅ (PASS WITH WARNINGS)
- archive-report.md ✅ (this file)

## File Changes (Production)

| File | Action | Lines Changed |
|------|--------|---------------|
| `tools/xs-language-server/src/symbols.rs` | Modified | ~-25/+5 |
| `tools/xs-language-server/tests/symbols_cleanup_repro.rs` | Created/Modified | 6 strict-TDD tests (+6 test count) |
| `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` | Modified | R1-F-01, R1-F-02, R1-F-03 markers added |
| `openspec/specs/spec-lsp-symbol-honesty.md` | Created | promoted from change folder |

## Review Markdown Updates

The canonical adversarial-review document `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` was updated with resolution markers for the three R1 findings addressed by this change:

- `Line 32` — top concerns list: `R1-F-01` marked `[Resolved 2026-06-29]` via `openspec/changes/archive/2026-06-29-fix-lsp-symbols-cleanup/` (dead code deleted).
- `Line 33` — top concerns list: `R1-F-03` marked `[Resolved 2026-06-29]` via the same archive path (test renamed).
- `Line 117` — per-finding section `R1-F-01` marked `[Resolved 2026-06-29]`.
- `Line 139` — per-finding section `R1-F-02` marked `[Refuted 2026-06-29 — direct parser inspection showed forward declarations DO produce ERROR nodes; the extract_error_* helpers are needed]` (no code change; separate grammar/parser change recommended).
- `Line 156` — per-finding section `R1-F-03` marked `[Resolved 2026-06-29]`.
- `Line 909` — last-verified footer: added entry that `R1-F-01` and `R1-F-03` are resolved while `R1-F-02` is refuted.

## Project Context Updates

`openspec/sdd-init/aom_retold_mod_idle_auto_repair.md` does not track per-tooling test counts; no update was required. The canonical test-count reference is `AGENTS.md` and the verify report.

## Deviations from Design

The apply phase substituted runtime source-grep contracts for the originally planned compile-fail contracts for `Param::render` absence. A compile-fail test for a deleted method would cause the entire test crate to fail to compile, breaking the rest of the strict-TDD suite. The runtime contract `param_render_no_callers_in_production_source` asserts the same post-fix state. This substitution is documented in:

- `tasks.md` § "Apply-Phase Deviations from Original Tasks"
- `verify-report.md` § "Deviations"

The verification result is equivalent for this dead-code cleanup.

## Verification Outcome

- **209 tests** passing (was 203 before Batch B; +6 new `symbols_cleanup_repro` tests).
- 6 new `symbols_cleanup_repro.rs` tests cover `Param::render` caller absence, `is_false` absence, and the renamed test.
- All spec scenarios covered by passing tests.
- All design decisions matched in code.
- No new clippy warnings introduced; pre-existing warning baseline unchanged.

## Review Workload Forecast

- Estimated changed lines: ~35 (well under 400-line budget)
- Chained PRs: No
- Chain strategy: single-pr
- Decision needed before apply: No

## SDD Cycle Status

The change has been fully:
1. **Planned** (proposal + spec + design + tasks) ✅
2. **Implemented** (apply, with strict TDD) ✅
3. **Verified** (verify, PASS WITH WARNINGS) ✅
4. **Archived** (this file) ✅

Ready for the next change.

## Open Follow-ups (out of scope for this change)

- The R1-F-02 refutation uncovered a real `tree-sitter-xs` grammar/parser bug: function forward declarations currently produce `ERROR` nodes. Recommend scheduling a dedicated `fix-tree-sitter-xs-forward-declarations` change as a Batch F candidate.
- The remaining review findings in `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` and the companion CSV remain open and can be addressed by future SDD changes.
- Pre-existing clippy warnings remain unchanged; a future global clippy-cleanup change could address them.
