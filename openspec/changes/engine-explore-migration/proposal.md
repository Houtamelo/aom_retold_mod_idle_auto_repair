# Proposal: Engine-based area explore migration

**Change id:** `engine-explore-migration`  
**Status:** proposed (exploration complete, awaiting spec + design)  
**Author:** sdd-propose sub-agent  
**Date:** 2026-06-23  
**Topic key:** `sdd/engine-explore-migration/proposal`

---

## 1. Intent

The `intelligent_auto_scout` mod currently parks the engine's `cPlanExplore` in `cPlanStateIdle` and drives the scout by hand with a ~3,054-line state machine: BFS area scoring, frontier-walk waypoint sampling, manual `aiTaskMoveUnit` issuance, corridor chains, and a custom danger heat-map. A proof-of-concept in `mod/auto_scout_test/` shows that the engine can consume a list of area IDs from `cExplorePlanExploreAreaIDs` and handle its own pathfinding/sequencing. This migration keeps the mod's high-level decisions (where to scout next, herd divert, Oracle pause, danger filter) but replaces the manual movement layer with engine-native area exploration, reducing stuck-scout edge cases and per-tick script load while preserving herd delivery and Oracle coordination.

---

## 2. User decisions (locked)

1. **Danger: keep hand-rolled heat-map.** `cExplorePlanAvoidingAttackedAreas=true` is empirically inert for player scouts; filter the area list with heat-map data before writing to `cExplorePlanExploreAreaIDs`. Keep flee + 90 s blacklist.
2. **Oracle: engine percentage + monitor.** Set `cExplorePlanStopLOSPercentage=0.8`; add a monitor that disables the plan and issues `aiTaskStopUnit` when current LOS < 0.5 × max LOS, then re-enables when current LOS = 1.0 (or `kbAreaGetPercentExplored(currentAreaID) >= 1.0`). The plan is disabled, not destroyed.
3. **Herd delivery: keep `Diverting` + `autoScout_homeMoveScan`.** During divert, park the engine plan (`aiPlanSetState(planID, cAutoScout_PlanStateIdle=23)`), issue manual `aiTaskMoveUnit`, then un-park before resuming.
4. **Area selection: keep BFS + scoring + per-area claims.** Only replace movement: remove frontier-walk, corridor logic, waypoint sampling, and manual `aiTaskMoveUnit` issuance. Let the engine pick waypoints via `cExplorePlanExploreAreaIDs`.
5. **Mod variants: single source of truth.** `auto_scout.xs` is shared across `intelligent_auto_scout`, `intelligent_auto_repair_and_scout`, and `human_assist_improvements`. Changes propagate automatically; only verify the include path in the combined mod `human_assist.xs`.
6. **Performance goal: mix.** Delete heaviest pieces (frontier-walk, corridor, waypoint sampling, manual `aiTaskMoveUnit`). Keep BFS, heat-map, herd, Oracle, claims.
7. **Military scope: player-triggered only.** Handle whatever units players give us; out of scope for this change.
8. **Testing maps:** user's call at verify time; proposal does not pre-commit to specific maps.

---

## 3. Approach

The mod transforms from **"engine plan + manual movement"** to **"engine plan + engine waypoints + mod-supplied area IDs"**. The engine's `cPlanExplore` consumes `cExplorePlanExploreAreaIDs` and picks waypoints natively; the mod becomes an **area selector and lifecycle manager**.

### 3.1 New responsibilities

| Layer | What it does |
|-------|--------------|
| `enableAutoScouting` wrapper | Creates `cPlanExplore`, adds unit, sets `cExplorePlanDoLoops=false`, Oracle `StopLOSPercentage=0.8`, then registers the slot. No `AvoidingAttackedAreas`. |
| `autoScout_register` | Bookkeeping only. Parking in `cPlanStateIdle` becomes **conditional** (herd divert and flee override only). |
| BFS + scoring (`autoScout_findNextArea` area, `auto_scout.xs:1699-2254`) | Kept intact. Decides the next target area. |
| Area-ID assignment (NEW helpers ported from POC) | Writes current/neighbor/starting-surroundings/far-area IDs into `cExplorePlanExploreAreaIDs`. |
| Heat-map filter (NEW glue) | Removes dangerous areas from the assigned list before writing to the plan. |
| Oracle monitor (NEW rule) | Pauses plan + unit at < 50 % LOS, resumes at 100 % LOS. |
| Herd divert | Parks plan, issues manual move, un-parks on resume. |
| Flee override | Parks plan, issues flee move, un-parks after hold. |
| `cleanupLingeringExplorePlans` | Destroy plans with zero units; unchanged. |

### 3.2 Flow diagram

```
Player toggle Auto-Scout
        |
        v
enableAutoScouting(unitID)
        |
        +--> create cPlanExplore
        +--> set cExplorePlanDoLoops=false
        +--> Oracle: set cExplorePlanStopLOSPercentage=0.8
        +--> autoScout_register(planID, unitID)
        |           +--> slot bookkeeping (NO unconditional parking)
        |
        v
Engine plan ACTIVE + cExplorePlanExploreAreaIDs assigned
        |
        +--> tick rule: BFS pick next area
        |       +--> heat-map filter removes dangerous areas
        |       +--> write area IDs to cExplorePlanExploreAreaIDs
        |
        +--> Oracle monitor
        |       +--> if currentLOS < 0.5 * maxLOS
        |       |         disable plan + aiTaskStopUnit(unitID)
        |       +--> elsif currentLOS == 1.0
        |                 re-enable plan
        |
        +--> herd divert
        |       +--> park plan (cPlanStateIdle)
        |       +--> aiTaskMoveUnit toward herd
        |       +--> un-park on resume
        |
        +--> flee override
                +--> park plan, flee, un-park
```

---

## 4. Scope

### 4.1 In scope

- Port POC helpers into `auto_scout.xs`: `pocExplorePlanAssignAreas`, `pocRemoveExploredAreas`, `autoScout_helperExploreStartingSurroundings`, `autoScout_helperExploreFarAreas`, `pocExploreCurrentArea`, `pocExploreNeighborAreas`.
- Conditionalize `autoScout_register` parking (`auto_scout.xs:2898-2936`).
- Add engine knobs in `enableAutoScouting` (`human_assist.xs:95-113`): `StopLOSPercentage=0.8` for Oracles; no `AvoidingAttackedAreas`.
- Replace manual movement (`autoScout_findNextArea`, `_issueCorridor`, frontier-walk, waypoint sampling) with area-ID assignment.
- Implement Oracle pause/resume monitor (NEW rule).
- Wire heat-map area-list filter (NEW glue between existing heat-map and `cExplorePlanExploreAreaIDs`).
- Verify the shared-file include path across all 3 mod variants.

### 4.2 Out of scope

- Military-unit scouting logic (handled by `base-ai-military-scout-comparison`).
- New mod variants.
- Map-specific tuning (user picks test maps at verify time).
- Performance benchmarking beyond script-load reduction observed in the diff.

---

## 5. Capabilities (contract with spec phase)

### New capabilities

- `engine-area-explore`: Assign area IDs to `cExplorePlanExploreAreaIDs` and let the engine drive waypoint selection.
- `oracle-los-monitor`: Pause/resume an Oracle's explore plan based on current vs. max LOS.

### Modified capabilities

- `intelligent-auto-scout-movement`: Replace manual `aiTaskMoveUnit` movement with engine-driven area exploration while keeping BFS/scoring/claims/heat-map/herd.
- `auto-scout-plan-lifecycle`: Change `autoScout_register` to keep the plan active by default and only park conditionally.

---

## 6. Files affected

| File | Change type | Approx lines added/removed/edited |
|------|-------------|-----------------------------------|
| `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` | Major rewrite | 500–1000 line diff (large deletions of frontier-walk/corridor/manual moves; modest additions for POC helpers + Oracle monitor + conditional parking). |
| `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs` | Minor tweak | Small diff in `enableAutoScouting` knob setup (`human_assist.xs:95-113`). |
| `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` | Verify include | Trivial or no change; verify `include "human_assist/auto_scout.xs"` preserved. |
| `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` | Verify include | Trivial or no change; verify `include "human_assist/auto_scout.xs"` preserved. |
| `scripts/deploy-mods.sh` | Verify copy | Confirm shared `auto_scout.xs` is copied to combined and improvements variants (lines 79, 84, 89). |

---

## 7. Risk register

| Risk | Severity | Mitigation | Owner |
|------|----------|------------|-------|
| R1: `cPlanStateIdle` parking incompatible with engine-driven `cPlanExplore` unless conditional. | High | Parking now only during divert/flee; default state leaves engine active. Define explicit state transitions in spec. | us |
| R2: `cExplorePlanAvoidingAttackedAreas=true` does not produce observable behavior for player scouts. | Resolved | Not used; rely on heat-map area-list filter instead. | N/A |
| R3: `kbAreaGetIDByPosition` mismatches engine's "current area" for `cPlanExplore`. | Medium | Add `kbCanPath` guards and log mismatches; spec must define fallback. | us |
| R4: Multiple scouts sent to same first area without per-area claims. | Resolved | Per-area claim system retained. | N/A |
| R5: Oracle pause/resume needs verified API for reading current LOS and max LOS. | Medium | Spec research: `kbUnitGetStatFloat(unitID, cUnitStatLOS)` and likely `kbUnitGetMaxLOS(unitID)`. | us |
| R6: Heat-map filter ordering could conflict with BFS scoring. | Low | Spec defines ordering: BFS produces ranked list, then heat-map removes dangerous entries before writing. | us |
| R7: `cAutoScout_PlanStateIdle` local constant (= 23) collides with engine's `cPlanStateIdle`; future patch could change it. | Low | Keep local constant equal to `cPlanStateIdle` and document dependency on `docs/MythTRConstants.txt:3166`. | us |
| R8: Re-assigning area list on un-park after herd divert — engine may have consumed/moved index. | High | Re-assign `cExplorePlanExploreAreaIDs` on un-park; spec MUST define ordering and verify the plan re-enters active state correctly after herd. | us |
| R9: Exact API to disable a plan without destroying it (`aiPlanSetActive` vs. `aiPlanSetState`). | Medium | Spec must resolve; fallback is `aiPlanDestroy` + `aiPlanCreate` if disable API unavailable. | us |
| Patch-maintenance: `human_assist.xs` overlay grows more engine-knob lines. | Low | Document changed lines in mod READMEs; keep diff minimal. | us |

---

## 8. Rollback plan

- The migration is concentrated in `auto_scout.xs` and one function family in `human_assist.xs`.
- Rollback = `git revert` the changed files and redeploy with `scripts/deploy-mods.sh`.
- **No runtime kill-switch flag** (`cAutoScout_useEngineAreas`) will be added. User decision: keeping both paths doubles maintenance surface; `git revert` is fast enough. If the migration regresses in production, revert the PR.

---

## 9. Verification strategy

Manual verification per `strict_tdd: false`:

- **Deploy baseline:** run `scripts/deploy-mods.sh` and load a match.
- **Basic movement:** toggle Auto-Scout on a regular scout; confirm via `aiEcho` that the unit moves and no manual `aiTaskMoveUnit` is logged for normal exploration.
- **Area IDs:** confirm `cExplorePlanExploreAreaIDs` is populated and consumed.
- **Oracle scenario:** deploy with an Oracle; confirm plan pauses + unit stops when LOS < 0.5 × max LOS, and resumes when LOS = 1.0.
- **Herd scenario:** deploy near herds; confirm scout diverts, parks plan, converts herd, and `autoScout_homeMoveScan` routes it to a TC.
- **Danger scenario:** deploy near enemy towers; confirm scout avoids via heat-map (not engine flag).
- **Shared-file deploy:** confirm `scripts/deploy-mods.sh` copies `auto_scout.xs` to all 3 mod variants.

---

## 10. Research points to resolve in spec phase

- RP1: Exact API to disable a plan without destroying it (`aiPlanSetActive`? `aiPlanSetState` with a new value?).
- RP2: Exact API to read unit current LOS and max LOS.
- RP3: Exact semantics of `cExplorePlanStopLOSPercentage` for Oracles — does it auto-pause the unit, or only affect plan routing?
- RP4: When the engine consumes an area ID from `cExplorePlanExploreAreaIDs`, can it be re-added? Does the engine mark areas as "done" persistently?
- RP5: Heat-map area-list filter ordering: BFS-then-filter, or filter-then-BFS?
- RP6: Confirm `cPlanStateIdle` value stability across AoM:R patches.

---

## 11. Recommended next phase

**next_recommended: `spec`**

The spec phase must produce:

- Requirements for the new Oracle pause/resume monitor (RP1, RP2, RP3).
- Heat-map → area-list filter contract (RP5).
- Plan lifecycle state machine (parked, active, paused).
- Modified `enableAutoScouting` engine-knob setup.

The design phase must produce:

- BFS pipeline refactor (how BFS result feeds area-ID assignment).
- Conditional parking implementation.

---

## 12. References

- Exploration report: `openspec/changes/engine-explore-migration/exploration.md`
- POC source: `mod/auto_scout_test/game/ai/human_assist/human_assist.xs:53-284`
- Target source: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
- Overlay: `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:95-113`
- Shared-include variants:
  - `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs:15`
  - `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs:15`
- Deploy script: `scripts/deploy-mods.sh:79,84,89`
- Engram decisions: observation #1202
