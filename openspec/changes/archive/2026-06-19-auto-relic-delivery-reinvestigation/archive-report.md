# Archive Report: auto-relic-delivery-reinvestigation

| Field | Value |
|---|---|
| Change id | `auto-relic-delivery-reinvestigation` |
| Capability | `auto-relic-delivery` |
| Archive date | 2026-06-19 |
| Verdict | **PASS** |

## Summary

The `auto-relic-delivery-reinvestigation` change is archived. The final implementation replaces the non-functional `cXSRelicPickedUpHandler` event path with a **2-second ground-relic poll**. On each tick the scan snapshots all alive Gaia-owned relics, computes a set-diff against the previous tick to detect disappearances, queries player-owned heroes within 10 meters of the relic's last known position, and issues `aiTaskWorkUnit` only when a nearby hero is carrying the exact disappeared relic unit ID, is idle, and has no active AI plan. No cross-tick state is retained beyond the previous-tick snapshot; the `(heroID, relicID)` pair tracker, pending-disappearance retry list, and singleton-handler caveat were removed. The user confirmed the in-game playtest works flawlessly after the `cInvalidID` and local-array initialization fixes.

## Final state

### Commits on master

| Hash | Message |
|---|---|
| `c0e4c1e` | feat(auto-relic): rewrite auto_relic_delivery.xs to poll-based disappearance detection |
| `281ef24` | docs(auto-relic): update registration site and README for poll-based delivery |
| `739ef52` | refactor(auto-relic): drop pair tracker and pending retry; track SDD artifacts |
| `ec74edc` | fix(auto-relic): replace cInvalidID with -1 in no-plan guard |
| `4e24de1` | fix(auto-relic): use new int(0,0)/new vector(0,cInvalidVector) for local arrays |

### Deployed file MD5s

| File | Source path | Deployed path | MD5 |
|---|---|---|---|
| `auto_relic_delivery.xs` | `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` | `$AOMR_LOCAL_MODS/Human Assist Improvements/game/ai/human_assist/auto_relic_delivery.xs` | `77ac5d0cf6f6da2804ba9fa193194c7d` |
| `human_assist.xs` | `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` | `$AOMR_LOCAL_MODS/Human Assist Improvements/game/ai/human_assist/human_assist.xs` | `2897889cbd65f97b0c67e1b3d2f58b7a` |

MD5s were captured after the most recent `./scripts/deploy-mods.sh` run; source and deployed copies are byte-identical.

## Spec deltas applied

Source of truth: `openspec/specs/auto-relic-delivery/spec.md`

### MODIFIED

- **R1 Trigger mechanism** — replace event-handler registration with a 2-second `autoRelicDelivery_scanRelics` rule; no `cXSRelicPickedUpHandler` registration.
- **R3 Delivery guard** — require both `kbUnitGetActionType(heroID) == cActionTypeIdle` and `kbUnitGetPlanID(heroID) == -1`.
- **R6 Relic identification** — use `kbUnitGetContainedUnitByIndex(heroID, 0)` (with defensive slot scan) and match the carried relic ID to the disappeared relic ID.

### REMOVED

- R1 Event-handler registration clause (superseded by poll-based R1).
- R3 One-shot retry rule.
- R3b Pending-disappearance retry.
- R6 (prior) `(heroID, relicID)` pair tracker.
- R9 Singleton handler caveat.

### ADDED

- **R12 Ground-relic disappearance scan** — snapshot alive ground relic IDs and detect disappearances by comparing consecutive tick results.
- **R13 Hero proximity query** — query alive player heroes within 10 meters of a disappeared relic's last position.

> **Note on R2:** The previous main-spec row R2 (`Track (heroID, relicID) pairs in append-only arrays as the active runtime path`) was effectively removed because the final design retains only the diff snapshot; the row is no longer present in the source-of-truth spec.

## Open issues / follow-ups

none

## Archive contents

- `exploration.md` ✅
- `proposal.md` ✅
- `specs/auto-relic-delivery/spec.md` ✅
- `design.md` ✅
- `tasks.md` ✅ (T1–T4 complete; T5 passed by user playtest; T6 skipped; T7 archive complete)
- `verify-report.md` ✅ (verdict PASS)
- `archive-report.md` ✅

## References

- Exploration: `openspec/changes/archive/2026-06-19-auto-relic-delivery-reinvestigation/exploration.md`
- Proposal: `openspec/changes/archive/2026-06-19-auto-relic-delivery-reinvestigation/proposal.md`
- Delta spec: `openspec/changes/archive/2026-06-19-auto-relic-delivery-reinvestigation/specs/auto-relic-delivery/spec.md`
- Design: `openspec/changes/archive/2026-06-19-auto-relic-delivery-reinvestigation/design.md`
- Tasks: `openspec/changes/archive/2026-06-19-auto-relic-delivery-reinvestigation/tasks.md`
- Verify report: `openspec/changes/archive/2026-06-19-auto-relic-delivery-reinvestigation/verify-report.md`
- Main spec: `openspec/specs/auto-relic-delivery/spec.md`
- Implemented file: `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs`
- Registration site: `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs`
- Deploy script: `scripts/deploy-mods.sh`
- Log extraction script: `scripts/extract-ai-logs.sh`
