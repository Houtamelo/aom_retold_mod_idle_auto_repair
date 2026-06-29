# Archive Report: fix-lsp-false-positive-forward-decl

## Status
Archived

## Date
2026-06-29

## Artifacts
- Spec (NEW): `openspec/specs/spec-lsp-forward-decl-honesty.md`
- Spec (synced from prior): N/A
- Design: `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/design.md`
- Tasks: `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/tasks.md`
- Apply progress: see Engram topic_key `sdd/fix-lsp-false-positive-forward-decl/apply-progress`
- Verify report: `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/verify-report.md`
- CHANGELOG: `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/CHANGELOG.md`
- RED evidence: `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/red-evidence.md`

## Outcome
This change resolves Issue #2 by making the LSP forward-declaration line-order check use the current-file include line (`effective_line`) instead of the intermediate file's raw `include_line`. `VisibilityProvenance::DirectInclude` and `TransitiveInclude` now store `effective_line: u32`, populated during `MergedView::build` from the earliest current-file `include` directive that reaches each symbol. The three line-order check sites in `semantic.rs` (`resolve_callee`, the `effective_line` helper, and `forward_callable_merged`) plus `MergedView::visibility_line` now compare against `effective_line()`, so unmodified vanilla `game/ai/` files opened through a mod overlay produce zero false-positive "used before declaration" diagnostics while real same-file forward-declaration errors and `mutable`-redefinition ordering continue to be enforced correctly.

The implementation adds six strict-TDD regression tests in `tools/xs-language-server/tests/forward_decl_repro.rs` covering the Issue #2 transitive-include layout, real same-file errors, `mutable` redefinition across a transitive include, cyclic include termination, multiple-root include edge ordering, and own-file symbol line selection. All 215 tests pass (209 baseline + 6 new); the `AOMR_GAME_PATH` integration test was skipped in the verification environment because the game is not installed there.

## Test count trajectory
- Before: 209
- After: 215 (+6 from forward_decl_repro.rs)

## Cross-references
- User issue: `docs/issues/2026-06-29-runtime-issues.md` Issue 2 (now marked Resolved)
- Audit: `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` (no audit findings overlap)
- Related archived change: `openspec/changes/archive/2026-06-29-fix-lsp-test-honesty/` (Batch D — similar verification rigor)
- Related archived change: `openspec/changes/archive/2026-06-29-fix-lsp-symbols-cleanup/` (Batch B — strict-TDD pattern)

## Commit guidance
The orchestrator will commit the change with the following Conventional Commits message:
```
fix(xs-lsp): use current-file include line for forward-decl ordering (Issue #2)

Fixes false-positive "used before declaration" diagnostics on unmodified vanilla game files opened through a mod overlay by storing the earliest current-file include line in `VisibilityProvenance` and using it in the three `semantic.rs` line-order checks. Adds six strict-TDD regression tests in `tests/forward_decl_repro.rs`.

Closes Issue #2 from docs/issues/2026-06-29-runtime-issues.md.
```

## Plugin version
Not bumped (LSP-only change; no plugin artifact change).
