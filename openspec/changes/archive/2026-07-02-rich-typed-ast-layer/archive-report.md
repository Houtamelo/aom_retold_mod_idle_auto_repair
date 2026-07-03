# Archive Report: `2026-07-02-rich-typed-ast-layer`

**Archive date:** 2026-07-03  
**Verified by:** `verify-report.md` (PASS, 159/159 tests, build clean)  
**Archive location:** `openspec/changes/archive/2026-07-02-rich-typed-ast-layer/`  
**Source-code location:** `tools/xs-language-server/lelwel-xs/src/ast/` (gitignored; not merged by this archive)

---

## Executive Summary

The SDD change `2026-07-02-rich-typed-ast-layer` has completed all phases (1–3), passed verification, and is now archived. The delta specifications (`spec-typed-ast.md` and `spec-format-preservation.md`) have been promoted to the main `openspec/specs/` source of truth. The original change artifacts have been moved to the archive directory as an audit trail. No source code was modified by the archive step; the `lelwel-xs` scratch implementation remains in its gitignored location for the user to integrate manually.

---

## Specs Synced to Main Source of Truth

| Domain | Action | Details |
|--------|--------|---------|
| `typed-ast` | Created | Copied `specs/spec-typed-ast.md` → `openspec/specs/spec-typed-ast.md` (full spec, 7 scenarios + NFRs) |
| `format-preservation` | Created | Copied `specs/spec-format-preservation.md` → `openspec/specs/spec-format-preservation.md` (full spec, 5 scenarios) |

Both domains were new; no existing main spec files were overwritten.

---

## Cleanup Applied Before Archive

Per the verify report's recommendations:

- `tasks.md` acceptance checkboxes were updated to `[x]` for all completed tasks (T8, T10 deferred criterion satisfied by T15, T11–T15).
- Phase 2 and Phase 3 headers were marked `✅ COMPLETE` to match verified status.
- No source-code changes were made.

---

## Moved Artifacts

All artifacts were moved from `openspec/changes/2026-07-02-rich-typed-ast-layer/` to `openspec/changes/archive/2026-07-02-rich-typed-ast-layer/`:

- `proposal.md` ✅
- `design.md` ✅
- `tasks.md` ✅ (15/15 tasks complete per verify report)
- `apply-progress.md` ✅ (Phase 1 + 2 + 3 records)
- `verify-report.md` ✅ (PASS for all 3 phases)
- `specs/spec-typed-ast.md` ✅
- `specs/spec-format-preservation.md` ✅
- `archive-report.md` ✅ (this file)

---

## Cross-references Preserved

- `openspec/changes/2026-07-03-fix-class-specifier-no-trailing-semi/` — referenced in `verify-report.md` and `apply-progress.md` as the separate change that resolved the `class_specifier` trailing-`;` issue. This archive operation did **not** modify that change's artifacts.
- `docs/plans/2026-07-02-typed-ast-design.md` — canonical design catalogue referenced by `design.md`.

---

## Post-archive State

- `openspec/changes/2026-07-02-rich-typed-ast-layer/` — removed.
- `openspec/changes/archive/2026-07-02-rich-typed-ast-layer/` — contains the full artifact trail.
- `openspec/specs/spec-typed-ast.md` and `openspec/specs/spec-format-preservation.md` — main specs now reflect the merged requirements.

---

## Outstanding Items (Out of Scope for This Change)

- Integration of the gitignored `lelwel-xs/src/ast/` source into the main `tools/xs-language-server/` tree (manual commit).
- Phase 4 LSP wiring: symbol extraction, hover, completion, definition, rename handlers using the typed AST.
- Removal of the `tree_sitter` dependency once LSP handlers are migrated.
- Production formatter with indentation, comment attachment, and policy support.

---

## SDD Cycle Status

**Complete.** The change has been proposed, specified, designed, tasked, applied, verified, and archived.
