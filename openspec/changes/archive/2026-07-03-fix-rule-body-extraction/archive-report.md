# Archive Report: 2026-07-03-fix-rule-body-extraction

**Change**: 2026-07-03-fix-rule-body-extraction  
**Archived on**: 2026-07-03  
**Verify status**: PASS — 176/176 tests pass (see `verify-report.md`)  
**Archive mode**: OpenSpec (filesystem) + Engram closure  

## Specs Synced

| Domain | Action | Details |
|--------|--------|---------|
| `fix-rule-body-extraction` | Created | Copied `spec-rule-body-extraction.md` (S-RBE-01..09) to `openspec/specs/spec-rule-body-extraction.md` |
| `typed-ast` | Updated | Added `RuleDefinition` body support section to `openspec/specs/spec-typed-ast.md` noting non-empty rule bodies now work via `TypeTable`-gated predicate dispatch |

## Archive Contents

All original artifacts preserved:

- `proposal.md` ✅
- `design.md` ✅
- `tasks.md` ✅ (9/9 tasks complete)
- `apply-progress.md` ✅ (2 deviations logged)
- `verify-report.md` ✅ (PASS)
- `specs/spec-rule-body-extraction.md` ✅
- `archive-report.md` ✅ (this file)

## Source of Truth Updated

- `openspec/specs/spec-rule-body-extraction.md`
- `openspec/specs/spec-typed-ast.md`

## Deviations Preserved

The deviations documented in `apply-progress.md` are preserved in the archive:

1. **Grammar structure**: Actual implementation uses a `block_declaration_or_definition` helper rule instead of direct ordered choice inside `block_item^`, due to lelwel 0.10.4 predicate/ordered-choice restrictions.
2. **S-RBE-07 / S-RBE-08 expectations**: Tests assert that no `Rule::Declaration` descendant is produced when `TypeTable` rejects the type; the statement fallback parses with errors because `int x;` / `MyClass x;` are not valid expression statements.

## Verification Summary

- `cargo build` in `tools/xs-language-server/lelwel-xs/` passed.
- 176 unit tests passed; 0 failed.
- All 9 spec scenarios (S-RBE-01..09) covered and compliant.
- Parent workspace failures were environmental (missing `doxygen_retail.7z` path and `rustdoc` binary), not introduced by this change.

## SDD Cycle Complete

The change has been fully proposed, specified, designed, tasked, implemented, verified, and archived.
