# Tasks: Engine-based area explore migration

**Change id:** `engine-explore-migration`  
**Status:** applied (2026-06-23). All implementation phases (1-9) completed; verification is manual in-game.  
**Author:** sdd-tasks sub-agent  
**Date:** 2026-06-23  
**Topic key:** `sdd/engine-explore-migration/tasks`

---

## 0. Goal and constraints

Turn `intelligent_auto_scout` from a hand-issued-movement mod into an **area selector + lifecycle manager** on top of the engine's `cPlanExplore`. The engine will consume `cExplorePlanExploreAreaIDs`; the mod keeps BFS scoring, heat-map filtering, per-area claims, herd divert, Oracle LOS pausing, and flee override.

User decisions are locked and must not be revisited in implementation:

1. Danger: keep the hand-rolled heat-map as an area-list filter.
2. Oracle: use `StopLOSPercentage=0.8` plus an explicit LOS monitor.
3. Herd delivery: keep divert + park/un-park around manual moves.
4. Area selection: keep BFS + scoring + claims.
5. Single source of truth: `auto_scout.xs` is shared.
6. Performance: delete heaviest pieces (frontier-walk, corridor, waypoint memory), keep the rest.
7. Military scope: out.
8. Testing maps: user's call.
9. No runtime kill-switch flag.
10. R8 is HIGH — un-park reassign is mandatory.

---

## Phase 1 — Foundation (port POC helpers + add new constants)

### Phase 1.1 — Add plan-state constants and shared arrays

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +40 / 0
**Prerequisites**: none
**Description**: Declare the lifecycle-mode constants and arrays that every later phase depends on, at the top of `auto_scout.xs` near the existing pool declarations.

**Concrete changes**:
- Add `cAutoScout_PlanStateActive`, `cAutoScout_PlanStateParkedDivert`, `cAutoScout_PlanStateParkedFlee`, `cAutoScout_PlanStatePausedOracle`.
- Keep `cAutoScout_PlanStateIdle = 23` as a documented alias for `cPlanStateIdle`.
- Add `const int cAutoScout_AssignmentInterval = 5` and a per-slot tick counter array `gAutoScout_assignmentTick[]` if needed.
- Add `extern int[] gAutoScout_planState = default`.
- Add `extern float[] gAutoScout_oracleMaxLOS = default`.
- Add comments pointing to `docs/MythTRConstants.txt:3149/3166` for `cPlanStateExplore` / `cPlanStateIdle`.

**Manual verification**:
1. Run `./scripts/deploy-mods.sh`.
2. Start a match with the Intelligent Auto-Scout mod.
3. Load the `aiEcho` log and confirm there is no XS compile error at script load.

**Manual verify takes**: ~5 minutes
**Risks**: R7 (`cPlanStateIdle` value) — documented in code; RP6 patch-stability.

---

### Phase 1.2 — Port `pocExplorePlanAssignAreas`

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +15 / 0
**Prerequisites**: Phase 1.1
**Description**: Add the low-level helper that resizes and writes land area IDs to `cExplorePlanExploreAreaIDs`. All assignment paths go through this helper.

**Concrete changes**:
- Create `void autoScout_explorePlanAssignAreas(int planID = -1, ref int[] areaIDs)`.
- Use `aiPlanSetNumberVariableValues(planID, cExplorePlanExploreAreaIDs, areaCount)`.
- Loop with `aiPlanSetVariableInt(planID, cExplorePlanExploreAreaIDs, i, areaIDs[i])`.
- Log the final list via `aiEcho`.

**Manual verification**:
1. Hard-code a test call in a temporary rule that writes `{12, 13}` to a plan and log `cExplorePlanExploreAreaIDsCurrentIndex`.
2. Deploy and load a match.
3. Confirm the index moves from `0` to `1` as the engine consumes area 12.

**Manual verify takes**: ~10 minutes
**Risks**: R3 (`kbAreaGetIDByPosition` mismatch) surfaced later; verifies the array-write API.

---

### Phase 1.3 — Port `pocRemoveExploredAreas`

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +20 / 0
**Prerequisites**: Phase 1.2
**Description**: Add an in-place filter that drops any area where `kbAreaGetPercentExplored(areaID) >= 1.0` before the area list is assigned.

**Concrete changes**:
- Create `void autoScout_removeExploredAreas(ref int[] areas)`.
- Iterate in reverse to keep removals cheap.
- Log each dropped area and each retained area.

**Manual verification**:
1. Start a match on a revealed map with some areas already explored.
2. Watch `aiEcho`: dropped areas are logged with `"already fully explored"`, retained areas with `"Added areaID"`.

**Manual verify takes**: ~10 minutes
**Risks**: engine-area-explore S3/S4.

---

### Phase 1.4 — Port starting-surroundings helper

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +55 / 0
**Prerequisites**: Phase 1.2, Phase 1.3
**Description**: Port `autoScout_helperExploreStartingSurroundings` from the POC. This is a fallback that assigns the start-position area ring when the BFS has no candidates.

**Concrete changes**:
- Add `static int[] gAutoScout_startingSurroundings = default` and `static bool gAutoScout_startingSurroundingsDone = false`.
- Initialize the ring once with `kbAreaGetIDsByPositionAndRange(startingPos, 130.0, areaLandScoutTypes, true, cPassabilityLand)`.
- Call `autoScout_removeExploredAreas` and `autoScout_explorePlanAssignAreas`.

**Manual verification**:
1. Disable BFS temporarily (do not call it) and force this helper to run on register.
2. Deploy; confirm the scout gets a starting-ring area list and moves.
3. Re-enable BFS integration in the next phase.

**Manual verify takes**: ~10 minutes
**Risks**: Fallback coverage for blank BFS results.

---

### Phase 1.5 — Port far-areas / area-group fallback helper

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +35 / 0
**Prerequisites**: Phase 1.2, Phase 1.3
**Description**: Port `autoScout_helperExploreFarAreas` so the scout never stalls when the starting surroundings and current-neighbor lists are exhausted.

**Concrete changes**:
- Create `bool autoScout_helperExploreFarAreas(int planID = -1, vector scoutPos = cInvalidVector)`.
- Use `kbAreaGroupGetIDByPosition` / `kbAreaGroupGetNumberAreas` / `kbAreaGroupGetAreaID`.
- Filter explored areas, then assign.

**Manual verification**:
1. Force the helper by clearing BFS results and starting surroundings.
2. Confirm `aiEcho` logs `"helperExploreFarAreas for plan ... at center ..."` and the scout receives all unvisited areas in its group.

**Manual verify takes**: ~10 minutes
**Risks**: Deep fallback against empty area lists.

---

### Phase 1.6 — Port current-area and neighbor-area refresh helpers

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +70 / 0
**Prerequisites**: Phase 1.2, Phase 1.3
**Description**: Port `pocExploreCurrentArea` and `pocExploreNeighborAreas` as `autoScout_exploreCurrentArea` and `autoScout_exploreNeighborAreas`. These become the fast-refill path when the engine reaches the edge of the assigned list.

**Concrete changes**:
- Create `bool autoScout_exploreCurrentArea(int unitID = -1, int planID = -1)`.
- Create `int autoScout_exploreNeighborAreas(int unitID = -1, int planID = -1)`.
- In the neighbor helper, add the current area plus `kbAreaGetNumberBorderAreas` / `kbAreaGetBorderAreaID`, then call `autoScout_removeExploredAreas`.
- Log each border area that is added.

**Manual verification**:
1. Deploy and stand a scout near a border between areas.
2. Confirm the log lists area 12 plus its border areas.
3. Confirm the engine moves the scout toward the first unexplored neighbor.

**Manual verify takes**: ~10 minutes
**Risks**: R3 area mismatch; verifies neighbor expansion before BFS wiring.

---

### Phase 1.7 — Add heat-map + claims area-list filter

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +45 / 0
**Prerequisites**: Phase 1.3
**Description**: Build the glue required by user decision #1: after `autoScout_findNextArea` produces a ranked list, remove dangerous, claimed, and self-scouted areas before writing to the engine plan.

**Concrete changes**:
- Create `void autoScout_filterDangerousAreas(ref int[] areas)`.
- Drop areas where `autoScout_areaIsDangerous(areaID) == true`.
- Drop areas where `gAutoScout_areaClaim[slotForArea] != -1` and the claim owner is not the current unit.
- Drop areas where `gAutoScout_areaSelfScouted[areaID] == true`.
- Iterate in reverse.
- Log dropped areas with reason.

**Manual verification**:
1. Place a scout near an enemy tower.
2. Confirm the heat-map marks the area dangerous and it is removed from the assigned list.
3. Confirm the engine routes to the next safe area instead.

**Manual verify takes**: ~10 minutes
**Risks**: R1, R6 (filter ordering); user decision #1.

---

### Phase 1.8 — Add `autoScout_unparkAndReassignAreas`

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +40 / 0
**Prerequisites**: Phase 1.2, Phase 1.4-1.7
**Description**: Create the single reactivation helper mandated by R8 HIGH. It recomputes and reassigns the area list before the plan is set back to `cPlanStateExplore`.

**Concrete changes**:
- Create `void autoScout_unparkAndReassignAreas(int planID = -1, int unitID = -1)`.
- Build ranked list (BFS first, then neighbor/current fallback).
- Call `autoScout_removeExploredAreas`.
- Call `autoScout_filterDangerousAreas`.
- Claim surviving areas and call `autoScout_explorePlanAssignAreas`.
- If the resulting list is empty, log `"no areas to scout"` and do not transition the plan state.

**Manual verification**:
1. Add a temporary test path that parks and immediately un-parks a registered plan.
2. Confirm `aiEcho` logs a fresh area list before `"un-parking"` is logged.
3. Confirm `cPlanStateExplore` is only set after the list is written.

**Manual verify takes**: ~10 minutes
**Risks**: R8 HIGH; central mitigation for stale consumed areas.

### Phase 1 manual verify wrap-up

- Deploy the mod with all Phase-1 helpers present but without calling them from `autoScout_register` yet.
- Confirm no compile errors and no behavioral change (plan is still parked by existing registration).

---

## Phase 2 — Lifecycle modifications (conditionalize parking)

### Phase 2.1 — Modify `autoScout_register`

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: -10 / +25
**Prerequisites**: Phase 1.1, Phase 1.8
**Description**: Remove the unconditional `aiPlanSetState(planID, cPlanStateIdle)` call and make registration leave the engine plan active.

**Concrete changes**:
- Keep the `kbPlayerIsHuman(cMyID)` guard unchanged.
- Add `gAutoScout_planState.add(cAutoScout_PlanStateActive)` to slot initialization.
- Remove the `aiPlanSetState(..., cAutoScout_PlanStateIdle)` line.
- After all slot arrays are initialized, call `autoScout_unparkAndReassignAreas(planID, unitID)` so the engine has an immediate area list.
- Add an `aiEcho` log with slot, unit ID, and mode `active`.

**Manual verification**:
1. Toggle Auto-Scout on a regular scout.
2. Use `aiEcho`/`aiPlanGetState` or a temporary log to confirm `cPlanStateExplore` is set right after registration.
3. Confirm the scout begins moving toward the first assigned area.

**Manual verify takes**: ~10 minutes
**Risks**: R1 HIGH; accidental parking regression.

---

### Phase 2.2 — Add `autoScout_parkPlan`

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +8 / 0
**Prerequisites**: Phase 1.1
**Description**: Create a single helper that parks the engine plan and logs why.

**Concrete changes**:
- Create `void autoScout_parkPlan(int planID = -1)`.
- Body: `aiPlanSetState(planID, cPlanStateIdle)`.
- Log `"autoScout: parking plan P"`.

**Manual verification**:
1. Add a temporary rule that calls `autoScout_parkPlan` on all registered scouts.
2. Confirm `aiPlanGetState` returns 23.
3. Remove the temporary rule.

**Manual verify takes**: ~5 minutes
**Risks**: R1; isolates parking.

---

### Phase 2.3 — Add `autoScout_unparkPlan`

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +12 / 0
**Prerequisites**: Phase 1.8, Phase 2.2
**Description**: Create the inverse helper: reassign areas then set the plan to explore.

**Concrete changes**:
- Create `void autoScout_unparkPlan(int planID = -1, int unitID = -1)`.
- Call `autoScout_unparkAndReassignAreas(planID, unitID)`.
- Then call `aiPlanSetState(planID, cPlanStateExplore)`.
- Log `"autoScout: un-parking plan P unit U"`.

**Manual verification**:
1. Park a registered plan manually, then call `autoScout_unparkPlan`.
2. Confirm a fresh area list is logged before the plan state changes to `cPlanStateExplore`.
3. Confirm the scout resumes movement.

**Manual verify takes**: ~5 minutes
**Risks**: R8 HIGH; confirms ordering of reassign before explore.

---

### Phase 2.4 — Keep unit-behavior enum orthogonal to plan-mode enum

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +5 / 0
**Description**: Verify that the existing `cAutoScoutState_*` enum (`Idle`, `Walking`, `Working`, `Diverting`, `Stationed`, `Fleeing`) is left untouched and that `gAutoScout_planState[]` is the only storage for plan lifecycle mode.

**Concrete changes**:
- Add a code comment above `gAutoScout_planState[]` explaining the split.
- Ensure `autoScout_register` adds exactly one new array slot for `gAutoScout_planState` without renaming the behavior-state array.

**Manual verification**:
1. Grep `cAutoScoutState_` usages; confirm none are repurposed for plan parking.
2. Grep `gAutoScout_planState`; confirm it is referenced by every lifecycle transition.

**Manual verify takes**: ~5 minutes
**Risks**: Spec R2 — prevents overloading the unit-behavior enum.

### Phase 2 manual verify wrap-up

- Deploy; toggle Auto-Scout; confirm the plan starts in `cPlanStateExplore`.
- Confirm the scout moves without any `aiTaskMoveUnit` issued by the mod.

---

## Phase 3 — BFS pipeline refactor (replace movement with area-ID assignment)

### Phase 3.1 — Delete frontier-walk and waypoint sampling

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: 0 / -280
**Prerequisites**: Phase 1.2
**Description**: Remove the hand-rolled `autoScout_findFrontierWaypoint` implementation, the `WORKING`-state frontier-walk loop, and related constants/arrays.

**Concrete changes**:
- Delete `autoScout_findFrontierWaypoint` and its supporting constants (angles/radii).
- Delete `gAutoScout_workVisited*` arrays and helpers.
- Delete any `aiTaskMoveUnit` calls issued purely for normal in-area exploration.

**Manual verification**:
1. Deploy and toggle Auto-Scout.
2. Grep the log: there must be no `aiTaskMoveUnit` entries attributed to normal exploration.
3. Confirm the scout still reaches new areas via the engine plan.

**Manual verify takes**: ~10 minutes
**Risks**: intelligent-auto-scout-movement R1/R3; large deletion surface.

---

### Phase 3.2 — Delete corridor-chain family

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: 0 / -180
**Prerequisites**: Phase 3.1
**Description**: Remove `autoScout_issueCorridor`, `autoScout_corridorFirstDangerousHop`, and all corridor-related state fields.

**Concrete changes**:
- Delete `autoScout_issueCorridor` and `autoScout_corridorFirstDangerousHop`.
- Delete `gAutoScout_corridorAreas`, `gAutoScout_corridorLen` and any related flat arrays.
- Remove corridor consumers from `autoScout_tickUnit`.

**Manual verification**:
1. Grep for `corridor` in `auto_scout.xs`; expect zero hits except historical comments.
2. Run a deploy and confirm no syntax errors.

**Manual verify takes**: ~5 minutes
**Risks**: intelligent-auto-scout-movement R2.

---

### Phase 3.3 — Refactor `autoScout_findNextArea` to return ranked area IDs

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: -40 / +15
**Prerequisites**: Phase 1.6
**Description**: Keep the BFS scoring logic but change the public contract so it fills a ranked `int[]` of area IDs (the predecessor path) instead of returning only a single target area.

**Concrete changes**:
- Modify `autoScout_findNextArea` signature to `void autoScout_findNextArea(int scoutUnitID = -1, ref int[] resultPath)` (or equivalent).
- Reconstruct the predecessor path into `resultPath`.
- Preserve the existing scoring, claim check, and self-scouted guards.
- If BFS returns no candidate, leave `resultPath` empty.

**Manual verification**:
1. Toggle Auto-Scout and read the `aiEcho` log.
2. Confirm the logged path is a sequence of area IDs from start to target.
3. Confirm the first ID in the path is the scout's current area when applicable.

**Manual verify takes**: ~10 minutes
**Risks**: intelligent-auto-scout-movement R4/R5.

---

### Phase 3.4 — Wire BFS → filter → assign in `autoScout_tickFast`

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +25 / -30
**Prerequisites**: Phase 1.7, Phase 1.8, Phase 3.3
**Description**: Replace the old manual-move driver with the new pipeline: run BFS, filter explored/dangerous/claimed areas, then write surviving IDs to `cExplorePlanExploreAreaIDs`.

**Concrete changes**:
- In `autoScout_tickFast`, gate the pipeline with `cAutoScout_AssignmentInterval` (default every 5 ticks).
- For each active slot call `autoScout_findNextArea` to build a ranked path.
- Call `autoScout_removeExploredAreas`, `autoScout_filterDangerousAreas`, claim surviving areas, and `autoScout_explorePlanAssignAreas`.
- Remove any remaining `WORKING`/`WALKING` manual-move code path.

**Manual verification**:
1. Toggle Auto-Scout; watch the log for assigned area IDs every 5 ticks.
2. Confirm no `aiTaskMoveUnit` is issued for ordinary movement.
3. Confirm the scout reaches areas in the order logged.

**Manual verify takes**: ~15 minutes
**Risks**: intelligent-auto-scout-movement R6; engine-area-explore R1-R4.

---

### Phase 3.5 — Adjust `autoScout_tickHeavy`

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: 0 / -20
**Prerequisites**: Phase 3.1
**Description**: The heavy rule still rebuilds the heat-map and runs the herd home-move scan; remove only the frontier-walk call.

**Concrete changes**:
- Remove any call to `autoScout_findFrontierWaypoint` or frontier sampling inside `autoScout_tickHeavy`.
- Keep all heat-map rebuild logic intact.

**Manual verification**:
1. Toggle Auto-Scout near an enemy tower.
2. Confirm the heat-map log still appears and the scout avoids the dangerous area.

**Manual verify takes**: ~10 minutes
**Risks**: Heat-map behavior regression.

---

### Phase 3.6 — Set `autoScout_tickFast` assignment cadence

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +5 / -5
**Prerequisites**: Phase 3.4
**Description**: Reduce the per-tick cost of the full BFS/filter/assignment pipeline by running it only every `cAutoScout_AssignmentInterval` ticks (default 5).

**Concrete changes**:
- Keep `minInterval 1` on `autoScout_tickFast`.
- Inside the rule, use a per-slot tick counter (or the global tick counter) to skip the expensive pipeline.
- Always allow lifecycle state checks (danger/flee/herd/oracle) to run every tick.

**Manual verification**:
1. Toggle Auto-Scout.
2. Confirm area-list assignment logs appear at ~5-tick intervals, while danger/flee/herd still react every tick.

**Manual verify takes**: ~10 minutes
**Risks**: Performance per design §9.

### Phase 3 manual verify wrap-up

- Deploy; toggle Auto-Scout.
- Confirm area IDs are assigned, the scout reaches them via engine movement, and no mod-issued `aiTaskMoveUnit` is logged for normal exploration.

---

## Phase 4 — Oracle LOS monitor

### Phase 4.1 — Initialize Oracle max-LOS cache

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`, `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`
**Estimated diff**: +12 / 0
**Prerequisites**: Phase 1.1
**Description**: Create the per-proto max-LOS cache and set the high `StopLOSPercentage` engine knob for every Oracle plan.

**Concrete changes**:
- Initialize `gAutoScout_oracleMaxLOS` to empty.
- In each variant's `enableAutoScouting`, set `aiPlanSetVariableFloat(planID, cExplorePlanStopLOSPercentage, 0, 0.8)` for Oracles.

**Manual verification**:
1. Toggle Auto-Scout on an Oracle.
2. Use a temporary `aiEcho` of `aiPlanGetVariableFloat(planID, cExplorePlanStopLOSPercentage, 0)` to confirm it reads `0.8`.

**Manual verify takes**: ~5 minutes
**Risks**: oracle-los-monitor R2; RP3 semantics.

---

### Phase 4.2 — Implement `autoScout_getOracleMaxLOS`

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +20 / 0
**Prerequisites**: Phase 4.1
**Description**: Implement lazy per-proto max-LOS lookup with fallback.

**Concrete changes**:
- Create `float autoScout_getOracleMaxLOS(int unitID = -1)`.
- Get proto via `kbUnitGetProtoUnitID(unitID)`.
- Expand cache size as needed.
- On first use read `kbPlayerGetProtoStatInt(cMyID, proto, cProtoStatLOS)`, cast to float, and fall back to `30.0` if `<= 0.0`.

**Manual verification**:
1. Add a temporary log of `autoScout_getOracleMaxLOS(oracleID)`.
2. Confirm a positive value (around 22-32 for a typical Oracle) and that repeated calls reuse the cache.

**Manual verify takes**: ~5 minutes
**Risks**: RP2 (AutoLOS buff included or not).

---

### Phase 4.3 — Add `autoScout_tickOracleLOS` rule

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +35 / 0
**Prerequisites**: Phase 2.2, Phase 4.1, Phase 4.2
**Description**: Implement a per-tick rule that polls every Oracle slot, computes the LOS ratio, and parks or resumes the plan.

**Concrete changes**:
- Add `rule autoScout_tickOracleLOS minInterval 1 active`.
- Iterate all slots; skip non-Oracle, dead, or diverting slots.
- Read `currentLOS = kbUnitGetStatFloat(unitID, cUnitStatLOS)` and `maxLOS = autoScout_getOracleMaxLOS(unitID)`.
- Compute `ratio = currentLOS / maxLOS`.
- If `ratio < 0.5` and plan is active, call `autoScout_parkPlan(planID)`, `aiTaskStopUnit(unitID)`, set mode `cAutoScout_PlanStatePausedOracle`, log.
- If `ratio >= 1.0` and mode is paused, call `autoScout_unparkPlan(planID, unitID)`, set mode active, log.

**Manual verification**:
1. Toggle an Oracle Auto-Scout.
2. Wait until LOS is low; confirm `"parking oracle unit N ratio R"` log and plan state 23.
3. Wait until LOS is full; confirm `"resuming oracle unit N ratio 1.0"` log and plan state 6.

**Manual verify takes**: ~15 minutes
**Risks**: oracle-los-monitor R3/R4; R5 herd precedence handled next.

---

### Phase 4.4 — Add dedicated Oracle park/resume helpers

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +15 / 0
**Prerequisites**: Phase 4.3
**Description**: Keep the rule body readable by extracting `autoScout_oracleParkAndStop` and `autoScout_oracleResume` helpers.

**Concrete changes**:
- Create `void autoScout_oracleParkAndStop(int planID = -1, int unitID = -1)`.
- Create `void autoScout_oracleResume(int planID = -1, int unitID = -1)`.
- Move the corresponding rule body blocks into these helpers.

**Manual verification**:
1. Repeat the Phase 4.3 manual test.
2. Confirm the same logs and state transitions.

**Manual verify takes**: ~10 minutes
**Risks**: None new; improves maintainability.

---

### Phase 4.5 — Add herd-precedence guard to Oracle monitor

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +5 / 0
**Prerequisites**: Phase 4.3, Phase 5.1
**Description**: Ensure an Oracle that is diverting to a herd is not paused by the LOS monitor, per oracle-los-monitor R5.

**Concrete changes**:
- In `autoScout_tickOracleLOS`, skip slots where `gAutoScout_planState[slot] == cAutoScout_PlanStateParkedDivert` (or the older unit-behavior state is `Diverting`).
- Log skipped Oracle because of herd.

**Manual verification**:
1. Put an Oracle near a herd and trigger divert.
2. Manually lower LOS (e.g., via debug scenario or wait).
3. Confirm no pause is issued while the Oracle is moving toward the herd.

**Manual verify takes**: ~10 minutes
**Risks**: oracle-los-monitor R5; interaction with herd divert.

---

### Phase 4.6 — Update all three mod variants' `enableAutoScouting` for Oracle knob

**File(s)**:
- `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`
- `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`
- `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs`
**Estimated diff**: +12 / -12 (total across 3 files)
**Prerequisites**: Phase 4.1
**Description**: Apply the 0.8 Oracle threshold in every wrapper. Remove the old 0.2 branch text.

**Concrete changes**:
- For each `human_assist.xs` variant:
  - Set `aiPlanSetVariableFloat(planID, cExplorePlanStopLOSPercentage, 0, 0.8)` inside the Oracle block.
  - Update the comment to say `"Stand still if less or equal than 80% of our surrounding tiles are explored (engine hint; explicit monitor owns <50% pause / 100% resume)"`.

**Manual verification**:
1. Deploy each variant.
2. Toggle Oracle Auto-Scout in each variant.
3. Confirm the threshold is set to 0.8 in the log.

**Manual verify takes**: ~15 minutes
**Risks**: RP3 semantics; must verify the engine hint does not conflict with the explicit monitor.

---

### Phase 4.7 — Remove action-37 Oracle state machine

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: 0 / -180
**Prerequisites**: Phase 4.3
**Description**: Delete the old `autoScout_tickOracleUnit` action-37 detector and overlap heuristics that are now superseded by the LOS monitor.

**Concrete changes**:
- Delete `autoScout_tickOracleUnit` rule.
- Delete `cAutoScout_OracleSaturatedActionType` and related overlap counters.
- Keep any generalized Oracle-overlap penalty if still used for BFS scoring; only remove the action-37 path.

**Manual verification**:
1. Grep `action 37`, `OracleSaturated`, and `tickOracleUnit`; expect no hits.
2. Run Oracle scenario from Phase 4.3; confirm pause/resume still works.

**Manual verify takes**: ~10 minutes
**Risks**: Removing fallback; mitigated by explicit monitor.

### Phase 4 manual verify wrap-up

- Deploy with an Oracle; confirm pause at `<0.5` LOS, stop unit, resume at `1.0` LOS.
- Confirm herd divert still overrides the monitor.

---

## Phase 5 — Herd divert + flee integration

### Phase 5.1 — Wrap `autoScout_tryDivert` with park/un-park

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +20 / -5
**Prerequisites**: Phase 2.2, Phase 2.3, Phase 3.4
**Description**: When the mod takes control to convert a herd, park the engine plan, issue the manual move, set mode `ParkedDivert`, and un-park on completion.

**Concrete changes**:
- At the start of `autoScout_tryDivert`:
  - Call `autoScout_parkPlan(planID)`.
  - Set `gAutoScout_planState[slot] = cAutoScout_PlanStateParkedDivert`.
- Issue the existing `aiTaskMoveUnit` toward the herd.
- In `autoScout_tickDivertingState`, on completion (herd converted / arrived / stuck):
  - Call `autoScout_unparkPlan(planID, unitID)`.
  - Set `gAutoScout_planState[slot] = cAutoScout_PlanStateActive`.

**Manual verification**:
1. Toggle Auto-Scout near a herd.
2. Confirm `aiEcho` shows `"parking for divert unit N"`, the manual move, then `"un-park after divert unit N"` with a fresh area list.
3. Confirm the converted herd walks toward a TC.

**Manual verify takes**: ~15 minutes
**Risks**: auto-scout-plan-lifecycle S2; intelligent-auto-scout-movement R7/R8; R8 HIGH.

---

### Phase 5.2 — Wrap `autoScout_homeMoveScan` park/un-park

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +15 / -5
**Prerequisites**: Phase 5.1
**Description**: If `autoScout_homeMoveScan` issues a manual move after herd conversion, ensure the engine plan remains parked during that move and un-parks only after the move is issued.

**Concrete changes**:
- In `autoScout_homeMoveScan`, before any `aiTaskMoveUnit` call, confirm `gAutoScout_planState[slot] == cAutoScout_PlanStateParkedDivert` (already parked).
- If a manual home move is issued, keep the parked mode; do not call `autoScout_unparkPlan` until the home move is finished.
- Add a log `"autoScout: homeMove while parked unit N"`.

**Manual verification**:
1. Convert a herd and watch the home-move phase.
2. Confirm no area reassignment happens while the unit is walking toward the TC.
3. Confirm un-park and area reassignment happen after the TC is reached or the move is done.

**Manual verify takes**: ~10 minutes
**Risks**: R8 HIGH; premature un-park could interrupt herd delivery.

---

### Phase 5.3 — Wrap flee path with park/un-park

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: +15 / -5
**Prerequisites**: Phase 2.2, Phase 2.3
**Description**: Update the flee override to park the plan while issuing a flee move, then reassign areas and un-park when the flee hold expires.

**Concrete changes**:
- In `autoScout_enterFleeing`:
  - Call `autoScout_parkPlan(planID)`.
  - Set `gAutoScout_planState[slot] = cAutoScout_PlanStateParkedFlee`.
- In the flee timer-expiry path of `autoScout_tickUnit`:
  - Call `autoScout_unparkPlan(planID, unitID)`.
  - Set `gAutoScout_planState[slot] = cAutoScout_PlanStateActive`.
  - Clear `gAutoScout_fleeFromArea` after un-park.

**Manual verification**:
1. Trigger danger near the scout (e.g., enemy tower).
2. Confirm `"parking for flee unit N"` log, the flee move, then `"un-park after flee unit N"` with a fresh area list.

**Manual verify takes**: ~10 minutes
**Risks**: intelligent-auto-scout-movement S4; auto-scout-plan-lifecycle S2.

### Phase 5 manual verify wrap-up

- Deploy with herds and enemy towers.
- Confirm divert + home-move preserve delivery.
- Confirm flee override does not strand the scout.

---

## Phase 6 — `enableAutoScouting` wrapper modifications

### Phase 6.1 — Update `enableAutoScouting` in the primary variant

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`
**Estimated diff**: +8 / -4
**Prerequisites**: Phase 4.6
**Description**: Align the wrapper with the new engine mode: always set `DoLoops=false`, never set `AvoidingAttackedAreas`, and set Oracle threshold to 0.8.

**Concrete changes**:
- Add `aiPlanSetVariableBool(planID, cExplorePlanDoLoops, 0, false)` for all scouts (not just Oracles).
- Remove any `cExplorePlanAvoidingAttackedAreas` calls.
- Keep the Oracle `StopLOSPercentage=0.8` set in Phase 4.6.
- Preserve the plan flags and `autoScout_register(planID, unitID)` call.

**Manual verification**:
1. Toggle Auto-Scout on a non-Oracle.
2. Confirm via temporary log that `cExplorePlanDoLoops` is false.
3. Confirm no `AvoidingAttackedAreas` log appears.

**Manual verify takes**: ~10 minutes
**Risks**: Proposal R2 (`AvoidingAttackedAreas` inert); user decision #1.

---

### Phase 6.2 — Confirm no missing engine knobs

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`
**Estimated diff**: +2 / 0
**Prerequisites**: Phase 6.1
**Description**: Audit the wrapper for any remaining engine knobs that could conflict with engine-driven area exploration.

**Concrete changes**:
- Add code comments listing the active knobs: `DoLoops=false`, `StopLOSPercentage=0.8` for Oracles, no `AvoidingAttackedAreas`.
- No code changes beyond comments if the audit is clean.

**Manual verification**:
1. Read the final wrapper body.
2. Confirm it matches design §7 and the spec for `engine-area-explore` / `oracle-los-monitor`.

**Manual verify takes**: ~5 minutes
**Risks**: Spec-design drift.

### Phase 6 manual verify wrap-up

- Deploy; toggle Auto-Scout with vanilla settings.
- Confirm the engine plan runs and the unit moves.

---

## Phase 7 — Mod variant verification

### Phase 7.1 — Verify combined mod include path

**File(s)**: `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`
**Estimated diff**: 0 / 0
**Prerequisites**: Phase 1.1
**Description**: Confirm that the combined mod still includes the shared `auto_scout.xs` and that `auto_repair.xs` is loaded before it.

**Concrete changes**:
- Read line 14-15; ensure order is `auto_repair.xs` then `auto_scout.xs`.
- No edits unless the include is broken.

**Manual verification**:
1. Open the file; confirm `include "human_assist/auto_scout.xs"; // Intelligent Auto-Scout mod` is present.

**Manual verify takes**: ~2 minutes
**Risks**: Single-source-of-truth user decision #5.

---

### Phase 7.2 — Verify improvements mod include path

**File(s)**: `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs`
**Estimated diff**: 0 / 0
**Prerequisites**: Phase 1.1
**Description**: Same check for the superset variant; auto-relic-delivery include is independent.

**Concrete changes**:
- Confirm `include "human_assist/auto_scout.xs";` at line 15.
- Confirm `auto_relic_delivery.xs` include does not interfere with scouting.

**Manual verification**:
1. Open the file; confirm the include line.

**Manual verify takes**: ~2 minutes
**Risks**: User decision #5.

---

### Phase 7.3 — Verify deploy script copies `auto_scout.xs`

**File(s)**: `scripts/deploy-mods.sh`
**Estimated diff**: 0 / 0
**Description**: Confirm the shared file is copied into the Intelligent Auto-Scout, Intelligent Auto-Repair and Scout, and Human Assist Improvements packages.

**Concrete changes**:
- Read lines 79, 84, 89; no edits.

**Manual verification**:
1. Open `scripts/deploy-mods.sh` and read the deploy calls.
2. Confirm `auto_scout.xs` is copied three times.

**Manual verify takes**: ~2 minutes
**Risks**: Deploy inconsistency.

---

### Phase 7.4 — Deploy and smoke-test each variant

**File(s)**: all three mod packages
**Estimated diff**: 0 / 0
**Prerequisites**: Phase 7.1-7.3
**Description**: Run the deploy script and verify every variant loads and toggles Auto-Scout without compile errors.

**Concrete changes**:
- No source edits; this is a verification task.

**Manual verification**:
1. Run `./scripts/deploy-mods.sh`.
2. Start a match with each variant standalone:
   - Intelligent Auto-Scout
   - Intelligent Auto-Repair and Scout
   - Human Assist Improvements
3. In each match, toggle Auto-Scout on a regular scout and on an Oracle if available.
4. Confirm no XS errors and that the unit moves.

**Manual verify takes**: ~20 minutes
**Risks**: Variant-specific interactions (auto_repair + auto_scout, auto_relic_delivery).

### Phase 7 manual verify wrap-up

- All three variants deploy correctly.
- No duplicate-first-area bug appears with multiple scouts in the combined variants.

---

## Phase 8 — Cleanup and verification matrix execution

### Phase 8.1 — Remove remaining dead code

**File(s)**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
**Estimated diff**: 0 / -80
**Prerequisites**: Phase 3, Phase 4.7, Phase 5
**Description**: Final cleanup pass for any leftover constants, arrays, or helper stubs from frontier-walk, corridor, and waypoint-memory logic.

**Concrete changes**:
- Grep for `frontier`, `corridor`, `workVisited`, `OracleSaturated`, `issueCorridor`.
- Delete orphan declarations and unused constants.
- Tidy comments that still describe removed behavior.

**Manual verification**:
1. Grep results are empty for removed concepts.
2. Deploy and toggle Auto-Scout; confirm no compile errors.

**Manual verify takes**: ~10 minutes
**Risks**: Accidental deletion of still-live code; use grep carefully.

---

### Phase 8.2 — Execute verification matrix end-to-end

**File(s)**: `openspec/changes/engine-explore-migration/design.md` §11, all mod source
**Estimated diff**: 0 / 0
**Prerequisites**: All prior phases
**Description**: Play through every row of design §11 with the user, collecting pass/fail/observations.

**Concrete changes**:
- No code edits; this is a manual testing task.
- Record results in the task artifact or a follow-up note.

**Manual verification**:
1. Use the matrix in the Verification matrix section below.
2. For each row:
   - Set up the scenario (fresh area, multiple scouts, Oracle LOS dip, enemy tower, herd, death, variant deploy).
   - Read `aiEcho` logs.
   - Record expected vs observed behavior.
3. Mark all rows as PASS before closing Phase 8.

**Manual verify takes**: ~40-60 minutes
**Risks**: RP3, RP4 Deferred; in-game semantics may differ from spec.

---

### Phase 8.3 — Verify deferred research points RP3 and RP4

**File(s)**: `openspec/changes/engine-explore-migration/specs/oracle-los-monitor.md`, `openspec/changes/engine-explore-migration/specs/engine-area-explore.md`
**Estimated diff**: 0 / 0
**Prerequisites**: Phase 4, Phase 8.2
**Description**: Document the in-game behavior of the two deferred research points.

**Concrete changes**:
- No code edits.
- Add a short note to the final docs or task artifact:
  - RP3: whether `StopLOSPercentage=0.8` auto-halted the Oracle in addition to the explicit `<0.5` monitor.
  - RP4: whether reassigning `cExplorePlanExploreAreaIDs` after un-park reset the read-only current index or the index persisted.

**Manual verification**:
1. For RP3: watch an Oracle at ~80% LOS. Note whether the engine already pauses movement before the monitor fires.
2. For RP4: after herd divert, log `cExplorePlanExploreAreaIDsCurrentIndex` before and after un-park. Note if it resets.
3. Decide whether a fallback (`aiPlanDestroy` + recreate) is required based on observations.

**Manual verify takes**: ~15 minutes
**Risks**: RP3/RP4 may force a follow-up hotfix if the engine semantics differ from assumptions.

### Phase 8 manual verify wrap-up

- All verification matrix rows pass.
- RP3 and RP4 observations recorded.
- No blocking regressions remain.

---

## Phase 9 — Documentation and changelog

### Phase 9.1 — Update `mod/intelligent_auto_scout/README.md`

**File(s)**: `mod/intelligent_auto_scout/README.md`
**Estimated diff**: +10 / -2
**Prerequisites**: Phase 8
**Description**: Add an "Engine explore migration" note explaining the new behavior (engine drives movement, mod selects areas, herd/oracle/flee unchanged).

**Concrete changes**:
- Add a short section under the changelog.
- List the removed manual-movement pieces (frontier-walk, corridor, waypoint memory).
- Mention added Oracle LOS monitor and plan-lifecycle helpers.

**Manual verification**:
1. Read the README.
2. Confirm the migration is described accurately for players.

**Manual verify takes**: ~5 minutes
**Risks**: Patch-maintenance note; user decision #9 (no kill-switch).

---

### Phase 9.2 — Update top-level `README.md` compatibility matrix

**File(s)**: `README.md`
**Estimated diff**: +5 / 0
**Prerequisites**: Phase 9.1
**Description**: If patch notes exist at repo root, add this change to the compatibility/changelog matrix.

**Concrete changes**:
- Add a row or bullet for `engine-explore-migration`.
- Mention affected mods: Intelligent Auto-Scout, Intelligent Auto-Repair and Scout, Human Assist Improvements.

**Manual verification**:
1. Read the top-level README.
2. Confirm the change is reflected.

**Manual verify takes**: ~5 minutes
**Risks**: None.

---

### Phase 9.3 — Add Known limitations section

**File(s)**: `mod/intelligent_auto_scout/README.md`
**Estimated diff**: +8 / 0
**Prerequisites**: Phase 8.3
**Description**: Document the two deferred research points and the manual-verify conditions.

**Concrete changes**:
- Add a "Known limitations" subsection.
- List RP3 (`StopLOSPercentage=0.8` exact engine pause semantics) and RP4 (area-index persistence across un-park).
- State: if RP4 shows index persistence, the fallback is `aiPlanDestroy` + recreate + re-register on resume.

**Manual verification**:
1. Read the new section.
2. Confirm it accurately reflects observations from Phase 8.3.

**Manual verify takes**: ~5 minutes
**Risks**: Transparency for players; avoids surprise regressions.

### Phase 9 manual verify wrap-up

- All docs updated.
- Final deploy script run.
- No source behavior changes in this phase.

---

## Verification matrix (cross-reference)

| Capability | Scenario | Implementing phase(s) | Manual verify phase | Expected `aiEcho` / outcome |
|---|---|---|---|---|
| `engine-area-explore` | Scout in fresh area | 1, 3 | 3.4, 8.2 | Area IDs assigned; no `aiTaskMoveUnit` for normal movement; scout reaches neighbor area. |
| `engine-area-explore` | Multiple scouts | 1.7, 3.4 | 7.4, 8.2 | Distinct area claims; no duplicate first-area assignment. |
| `engine-area-explore` | All areas explored | 1.3, 1.8, 3.4 | 8.2 | `"no areas to scout"`; plan stays alive with empty list. |
| `oracle-los-monitor` | Oracle fresh area | 4.1, 4.6 | 4.1, 8.2 | `StopLOSPercentage=0.8` set; engine moves Oracle. |
| `oracle-los-monitor` | Oracle LOS < 0.5 × max | 4.3, 4.4 | 4.3, 8.2 | `"parking oracle unit N ratio R"`; plan `cPlanStateIdle`, unit stopped. |
| `oracle-los-monitor` | Oracle LOS = 1.0 | 4.3, 4.4 | 4.3, 8.2 | `"resuming oracle unit N ratio 1.0"`; plan active, unit moves. |
| `intelligent-auto-scout-movement` | Scout near tower | 1.7, 3.5 | 1.7, 8.2 | Heat-map filter log; engine drives around danger; scout does not walk into tower. |
| `intelligent-auto-scout-movement` | Scout divert | 5.1, 5.2, 5.3 | 5.1, 8.2 | Park log, manual move, un-park log, area list reassigned; herd converted + home-move. |
| `auto-scout-plan-lifecycle` | Scout killed | 2.1, 2.4 | 8.2 | `dropFromPool` + `cleanupLingeringExplorePlans`; plan destroyed. |
| Mod variants | Deploy all 3 | 7.1-7.4 | 7.4, 8.2 | `auto_scout.xs` copied to all 3 variants; each toggles Auto-Scout. |

---

## Commit strategy (work-unit commits)

If applying via chained PRs, split the implementation into reviewable commits that preserve logical ordering and keep each PR near or below the 400-line soft limit:

1. `Port POC helpers + add new constants` — Phase 1.
2. `Conditionalize plan parking + add lifecycle helpers` — Phase 2.
3. `Replace manual movement with engine area assignment` — Phase 3.
4. `Add Oracle LOS pause/resume monitor` — Phase 4.
5. `Integrate herd divert + flee park/un-park` — Phase 5.
6. `Update enableAutoScouting wrapper across all variants` — Phases 6 and 7 combined (deployment verification in commit message).
7. `Cleanup, docs, and verification matrix` — Phases 8 and 9.

Commit 3 is the largest deletions; because most of it is removal, the reviewable "net new logic" is small, but the changed-line count will still exceed 400. The Review Workload Forecast below recommends chaining so this commit does not block review of the others.

---

## Review Workload Forecast

**Total estimated diff**: ~665 lines added, ~1,160 lines removed, net ~-495 lines across 5 source files (plus documentation)
**Files affected**:

| File | Lines added | Lines removed | Net change |
|---|---|---|---|
| `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` | ~650 | ~1,140 | ~-490 |
| `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs` | ~8 | ~4 | ~+4 |
| `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` | ~8 | ~4 | ~+4 |
| `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` | ~8 | ~4 | ~+4 |
| `mod/intelligent_auto_scout/README.md` | ~10 | ~2 | ~+8 |
| `README.md` | ~5 | ~0 | ~+5 |
| `scripts/deploy-mods.sh` | 0 | 0 | 0 (verification only) |

**400-line budget risk**: High — `auto_scout.xs` alone adds 650 new lines and removes 1,140. Even though many removals are mechanical deletions, the changed-line count is well above 400.

**800-line budget risk**: High — total added + removed is approximately 1,825 lines, more than double the 800-line review budget.

**Chained PRs recommended**: Yes. The work naturally splits into seven logical, sequentially dependent units (commit strategy above). Chaining lets reviewers focus on the lifecycle contract (Phase 2), the BFS pipeline (Phase 3), and the Oracle monitor (Phase 4) separately.

**Decision needed before apply**:
- Yes — confirm whether to land the work as a single stacked chain into `main` or validate Phase 1-3 in the `intelligent_auto_scout` variant first before touching the combined variants.
- The recommended default is the seven-commit chained chain, starting with Phase 1, because `auto_scout.xs` is shared by all three variants; there is no clean way to validate one variant without impacting the others via `scripts/deploy-mods.sh`.

**Reasoning**:
- The line-count forecast is driven by the design §8.1 file-by-file table, which calls for large deletions of frontier-walk, corridor, waypoint-memory, and action-37 manual-movement code.
- New code is concentrated in small, testable helpers (POC ports, filter, monitor, lifecycle helpers). Review density is highest where old and new logic intersect: `autoScout_register`, `autoScout_tickFast`, `autoScout_tickUnit`, and the new Oracle rule.
- Chaining isolates the riskiest interaction (R8: un-park reassign) in Phase 2/5, the movement refactor in Phase 3, and the Oracle semantics in Phase 4. A problem in one phase can be reverted without losing the whole migration.
- There is no runtime kill-switch flag; the rollback path is `git revert` of the relevant commits. Keeping commits small and thematic makes that rollback safer.
