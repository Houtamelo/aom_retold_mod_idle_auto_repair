# Archive Report: Bootstrap Human Assist Improvements Mod

The `bootstrap-human-assist-improvements-mod` change is fully archived. The Human Assist Improvements bundled mod is bootstrapped in source, its delta spec is promoted to the openspec main spec store, and all SDD phase artifacts are preserved below.

## Quick path

1. Main spec created at `openspec/specs/human-assist-improvements-mod-bootstrap/spec.md` (additive, no merge conflicts).
2. Change folder moved to `openspec/changes/archive/2026-06-18-bootstrap-human-assist-improvements-mod/`.
3. Verification verdict was **PASS** with 0 CRITICAL issues.

## Change summary

| Field | Value |
|---|---|
| Change name | `bootstrap-human-assist-improvements-mod` |
| Intent | Add a 4th AoM:R local mod, **Human Assist Improvements**, bundling Auto-Repair + Intelligent Auto-Scout as the single home for future human-assist features |
| Capability | `human-assist-improvements-mod-bootstrap` |
| Nature | Purely additive: new mod directory, deploy block, README entries, and placeholder thumbnail |
| Existing mod files modified | None |

## Phase roll-up

| Phase | Status |
|---|---|
| explore | done |
| propose | done |
| spec | done |
| design | done |
| tasks | done |
| apply | done |
| verify | **PASS** |

## Specs synced

| Domain | Action | Details |
|---|---|---|
| `human-assist-improvements-mod-bootstrap` | Created | 7 requirements + 7 scenarios promoted from the delta spec to `openspec/specs/human-assist-improvements-mod-bootstrap/spec.md`. No existing main spec existed; no destructive deltas were merged. |

## Source of truth updated

- `openspec/specs/human-assist-improvements-mod-bootstrap/spec.md`

## Files / deltas delivered

| File | Action | Description |
|---|---|---|
| `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` | Create | Vanilla overlay with both feature includes and scout hook; byte-identical to the combined mod at bootstrap |
| `mod/human_assist_improvements/README.md` | Create | Bundle description, install/run steps, conflict warning, patch-maintenance note, manual verification checklist |
| `scripts/deploy-mods.sh` | Modify | Additive 4th deploy block; copies `auto_repair.xs` and `auto_scout.xs` from sibling source dirs |
| `README.md` | Modify | 4th mod added to mod list; compatibility/conflict note added |
| `thumbnail_human-assist-improvements.png` | Create | Placeholder thumbnail copied from the combined-mod thumbnail |

## Verify verdict

- **Verdict:** PASS
- **CRITICAL:** 0
- **WARNING:** 1
- **SUGGESTION:** 1 (rejected)

### WARNING

`.gitignore` is modified in the working tree independently of this change. It is outside the SDD change boundary and should be reviewed/reverted before packaging the PR.

### SUGGESTION (rejected)

> *Add a sync-reminder comment in the new `human_assist.xs`.*

**Rejected by the orchestrator.** Adding a comment would break design decision **D1**: the 4th mod's `human_assist.xs` must remain byte-identical to the combined mod's `human_assist.xs` at bootstrap. The file itself carries no delta from the combined mod, so a maintainer comment would introduce an artificial difference. Revisit this suggestion when the 4th mod receives its first real feature — at that point byte-identity no longer holds and inline maintainer comments become appropriate.

## Archive contents

- `exploration.md` ✅
- `proposal.md` ✅
- `specs/human-assist-improvements-mod-bootstrap/spec.md` ✅
- `design.md` ✅
- `tasks.md` ✅ (14/14 tasks complete)
- `verify-report.md` ✅
- `archive-report.md` ✅

## Patch-maintenance notes

- The 4th mod's `human_assist.xs` is a whole-file vanilla overlay, carrying the same patch-rebase burden as the other three mods. Any game patch touching the original file requires manual rebasing of the include and hook edits.
- At bootstrap, `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` is byte-identical to `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`. Keep them in sync until the 4th mod intentionally diverges.
- Canonical feature sources:
  - `auto_repair.xs` → `mod/idle_auto_repair/game/ai/human_assist/auto_repair.xs`
  - `auto_scout.xs` → `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
- Future feature files should live directly in `mod/human_assist_improvements/game/ai/human_assist/` and be deployed from that directory, following the extensibility pattern in `design.md`.

## Rollback procedure

1. Delete `mod/human_assist_improvements/`.
2. Revert the `HUMAN_SRC` variable and the `=== Human Assist Improvements ===` deploy block in `scripts/deploy-mods.sh`.
3. Revert the 4th mod list entry and compatibility note in `README.md`.
4. Delete `thumbnail_human-assist-improvements.png`.

Rollback is clean because no existing mod source files were modified.

## Traceability

- Apply progress recorded in Engram observation `#958` (`sdd/bootstrap-human-assist-improvements-mod/apply-progress`).
- This archive report persisted in Engram under topic key `sdd/bootstrap-human-assist-improvements-mod/archive-report`.

## Pre-publish notes for orchestrator

- Address the `.gitignore` working-tree modification before packaging the PR.
- Do not add a maintainer comment to `human_assist.xs` at this time; revisit after the first real 4th-mod feature divergence.

## Next step

No further SDD phase is required. The change is ready for git publishing by the orchestrator/user.
