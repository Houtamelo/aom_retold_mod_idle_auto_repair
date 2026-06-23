# Design: Engine-based area explore migration

**Change id:** `engine-explore-migration`  
**Status:** designed (explore + propose + spec complete; tasks next)  
**Author:** sdd-design sub-agent  
**Date:** 2026-06-23  
**Topic key:** `sdd/engine-explore-migration/design`

---

## 1. Architecture overview

The mod becomes a **area selector + lifecycle manager** on top of the engine's `cPlanExplore`. The engine owns pathfinding and waypoint sequencing; the mod decides which areas are worth scouting, filters danger, and parks/resumes the plan for herd divert, flee override, and Oracle LOS pauses.

```
+---------------------------------------+
|  Player toggle → enableAutoScouting   |
|  (human_assist.xs:95-113)             |
+-----------+---------------------------+
            |
            v
+---------------------------------------+
|  Engine cPlanExplore                  |
|  consumes cExplorePlanExploreAreaIDs  |
+-----------+---------------------------+
            |
            v
+---------------------------------------+
|  Mod area-selection layer             |
|  • BFS scoring (auto_scout.xs:2101)   |
|  • Per-area claims / self-scouted     |
|  • Heat-map filter                    |
+-----------+---------------------------+
            |
            v
+---------------------------------------+
|  Lifecycle overrides                  |
|  • Herd divert park + manual move     |
|  • Flee override park + flee move     |
|  • Oracle LOS monitor pause/resume    |
+---------------------------------------+
```

**Key changes from current mod**
- `autoScout_register` no longer parks the plan (`auto_scout.xs:2898-2936`).
- Manual `aiTaskMoveUnit` for normal movement is removed (`auto_scout.xs:2342-2873`).
- Frontier-walk (`auto_scout.xs:2260-2310`), corridor chains, and waypoint memory are deleted.
- Heat-map is retained but only filters the area list before it is written to `cExplorePlanExploreAreaIDs`.
- A new `cAutoScout_PlanState*` enum tracks why the engine plan is parked.

---

## 2. State machine (`auto-scout-plan-lifecycle`)

```
                    register / un-park
                           |
                           v
                       +--------+
                       | active |
                       +--------+
                          ^ ^ ^
            herd converted| | |cooldown over
            re-assign     | | |re-assign
                          | | |
       +------------------+ | +------------------+
       |                    |                    |
       v                    v                    v
+-------------+     +-------------+     +-------------+
| parked-     |     | parked-     |     | paused-     |
| divert      |     | flee        |     | oracle      |
+-------------+     +-------------+     +-------------+
       ^                                          |
       |                                          |
       +-------------- herd eligible -------------+
```

- **active**: engine `cPlanExplore` is in `cPlanStateExplore` and consumes the assigned area list.
- **parked-divert**: plan is `cPlanStateIdle`; scout is under manual `aiTaskMoveUnit` toward a herd.
- **parked-flee**: plan is `cPlanStateIdle`; scout is fleeing danger.
- **paused-oracle**: plan is `cPlanStateIdle`; Oracle LOS is below 50 % of max.

Plan-level state storage is orthogonal to the existing unit-behavior enum (`cAutoScoutState_*`). A new `gAutoScout_planState[]` array holds the mode.

| Transition | Trigger | Action | Log message |
|---|---|---|---|
| → active | `autoScout_register` finishes setup | Set mode active, run BFS pipeline, write area IDs | `autoScout: registered unit N plan P mode active` |
| active → parked-divert | `autoScout_tryDivert` succeeds | `aiPlanSetState(planID, cPlanStateIdle)`, `aiTaskMoveUnit` to herd | `autoScout: parking for divert unit N` |
| parked-divert → active | Herd converted / arrived / stuck | Run BFS pipeline, `aiPlanSetState(planID, cPlanStateExplore)` | `autoScout: un-park after divert unit N` |
| active → parked-flee | Current/target/hop area dangerous | `aiPlanSetState(planID, cPlanStateIdle)`, issue flee move | `autoScout: parking for flee unit N` |
| parked-flee → active | `xsGetTimeMS() >= fleeUntilMs` | Run BFS pipeline, `aiPlanSetState(planID, cPlanStateExplore)` | `autoScout: un-park after flee unit N` |
| active → paused-oracle | Oracle `currentLOS < 0.5 * maxLOS` | `aiPlanSetState(planID, cPlanStateIdle)`, `aiTaskStopUnit` | `autoScout: parking oracle unit N ratio R` |
| paused-oracle → active | Oracle `currentLOS / maxLOS >= 1.0` | Run BFS pipeline, `aiPlanSetState(planID, cPlanStateExplore)` | `autoScout: resuming oracle unit N ratio R` |
| any → destroyed | Unit dies | `autoScout_dropFromPool`, `cleanupLingeringExplorePlans` destroys empty plan | `autoScout: dropFromPool slot S unit N state M` |

---

## 3. BFS pipeline

Data flow:

```
autoScout_findNextArea(unitID)
        |
        +--> gAutoScout_bfsResultPath (start → ... → target)
        |
        v
autoScout_buildAreaListFromPath()
        |
        v
autoScout_removeExploredAreas()
        |
        v
autoScout_filterDangerousAreas()   // heat-map + claims + self-scouted
        |
        v
autoScout_claimAndAssignAreas(planID, filteredList)
        |
        v
aiPlanSetNumberVariableValues(...)
aiPlanSetVariableInt(...) into cExplorePlanExploreAreaIDs
```

- `autoScout_findNextArea` (`auto_scout.xs:2101-2254`) stays largely unchanged. It produces a scored target area and reconstructs the predecessor path.
- `autoScout_buildAreaListFromPath` is a new helper that copies the path to a local `int[]`. If the path is empty, it falls back to the current area plus its border areas (`kbAreaGetNumberBorderAreas` / `kbAreaGetBorderAreaID`) guarded by `kbCanPath`.
- `autoScout_filterDangerousAreas` removes areas where `autoScout_areaIsDangerous(areaID) == true`, areas already claimed by another scout, and `gAutoScout_areaSelfScouted` areas.
- `autoScout_claimAndAssignAreas` claims each surviving area for the scout, logs the final list, then calls `autoScout_explorePlanAssignAreas`.
- Tick cadence: the full pipeline runs every `cAutoScout_AssignmentInterval` ticks (default 5) to reduce script load. On un-park transitions it runs immediately.

---

## 4. Plan lifecycle implementation

### 4.1 New constants and storage

```xs
const int cAutoScout_PlanStateActive       = 0;
const int cAutoScout_PlanStateParkedDivert = 1;
const int cAutoScout_PlanStateParkedFlee   = 2;
const int cAutoScout_PlanStatePausedOracle = 3;
// Existing alias kept only for aiPlanSetState calls.
const int cAutoScout_PlanStateIdle = 23;  // == cPlanStateIdle

extern int[] gAutoScout_planState = default;
```

### 4.2 `autoScout_register` changes

Current code (`auto_scout.xs:2898-2936`) unconditionally calls `aiPlanSetState(planID, cAutoScout_PlanStateIdle)`. New behavior:

1. Guard `kbPlayerIsHuman(cMyID)` unchanged.
2. Add the slot arrays, including `gAutoScout_planState.add(cAutoScout_PlanStateActive)`.
3. Do **not** call `aiPlanSetState(planID, cPlanStateIdle)`.
4. Call `autoScout_unparkAndReassignAreas(planID, unitID)` so the engine has an immediate area list.
5. (Optional) keep an immediate first-tick advance, but the reassign call already seeds the plan.

### 4.3 Park / un-park helpers

```xs
void autoScout_parkPlan(int planID = -1)
{
   aiPlanSetState(planID, cPlanStateIdle);
}

void autoScout_unparkPlan(int planID = -1, int unitID = -1)
{
   autoScout_unparkAndReassignAreas(planID, unitID);
   aiPlanSetState(planID, cPlanStateExplore);
}

void autoScout_unparkAndReassignAreas(int planID = -1, int unitID = -1)
{
   int[] ranked = autoScout_buildRankedAreaList(unitID);
   autoScout_removeExploredAreas(ranked);
   autoScout_filterDangerousAreas(ranked);
   autoScout_claimAndAssignAreas(planID, unitID, ranked);
}
```

`autoScout_unparkAndReassignAreas` is the single point every active transition calls. This centralizes the R8 HIGH mitigation.

### 4.4 Herd divert wiring

In `autoScout_tryDivert` (`auto_scout.xs:1522-1542`):
- Before issuing `aiTaskMoveUnit`, call `autoScout_parkPlan(gAutoScout_planID[slot])`.
- Set `gAutoScout_planState[slot] = cAutoScout_PlanStateParkedDivert`.

In `autoScout_tickDivertingState` (`auto_scout.xs:2424-2456`):
- On completion (herd converted / arrived / stuck), before `autoScout_setStateIdle(slot)`, call `autoScout_unparkPlan(gAutoScout_planID[slot], unitID)` and set `gAutoScout_planState[slot] = cAutoScout_PlanStateActive`.

### 4.5 Flee override wiring

In `autoScout_enterFleeing` (`auto_scout.xs:1336-1372`):
- Park the plan, set `cAutoScout_PlanStateParkedFlee`.

When the flee timer expires (`autoScout_tickUnit` Fleeing branch, `auto_scout.xs:2881-2889`):
- Call `autoScout_unparkPlan` and set mode active before clearing `gAutoScout_fleeFromArea`.

### 4.6 Oracle pause/resume wiring

New rule `autoScout_tickOracleLOS` (see §5) parks/un-parks directly. When it transitions:
- park → set `cAutoScout_PlanStatePausedOracle`.
- resume → call `autoScout_unparkPlan(planID, unitID)` and set active.

---

## 5. Oracle LOS monitor

New rule:

```xs
rule autoScout_tickOracleLOS
minInterval 1
active
{
   xsSetContextPlayer(cMyID);
   int slotCount = gAutoScout_unitID.size();
   for (int slot = 0; slot < slotCount; slot = slot + 1)
   {
      int unitID = gAutoScout_unitID[slot];
      int planID = gAutoScout_planID[slot];
      if (kbUnitGetIsIDValid(unitID) == false) { continue; }
      if (kbUnitIsType(unitID, cUnitTypeAbstractOracle) == false) { continue; }
      if (gAutoScout_planState[slot] == cAutoScout_PlanStateParkedDivert) { continue; }

      float currentLOS = kbUnitGetStatFloat(unitID, cUnitStatLOS);
      float maxLOS     = autoScout_getOracleMaxLOS(unitID);
      float ratio      = currentLOS / maxLOS;

      if (ratio < 0.5 && gAutoScout_planState[slot] == cAutoScout_PlanStateActive)
      {
         aiEcho("autoScout: parking oracle unit " + unitID + " ratio " + ratio);
         aiPlanSetState(planID, cPlanStateIdle);
         aiTaskStopUnit(unitID);
         gAutoScout_planState[slot] = cAutoScout_PlanStatePausedOracle;
      }
      else if (ratio >= 1.0 && gAutoScout_planState[slot] == cAutoScout_PlanStatePausedOracle)
      {
         aiEcho("autoScout: resuming oracle unit " + unitID + " ratio " + ratio);
         autoScout_unparkAndReassignAreas(planID, unitID);
         aiPlanSetState(planID, cPlanStateExplore);
         gAutoScout_planState[slot] = cAutoScout_PlanStateActive;
      }
   }
   xsSetContextPlayer(-1);
}
```

### Max-LOS cache

```xs
extern float[] gAutoScout_oracleMaxLOS = default;

float autoScout_getOracleMaxLOS(int unitID = -1)
{
   int proto = kbUnitGetProtoUnitID(unitID);
   while (gAutoScout_oracleMaxLOS.size() <= proto)
   {
      gAutoScout_oracleMaxLOS.add(0.0);
   }
   if (gAutoScout_oracleMaxLOS[proto] <= 0.0)
   {
      float base = kbPlayerGetProtoStatInt(cMyID, proto, cProtoStatLOS);
      if (base <= 0.0) { base = 30.0; }
      gAutoScout_oracleMaxLOS[proto] = base;
   }
   return(gAutoScout_oracleMaxLOS[proto]);
}
```

- Cache is per-proto, lazy-initialized, and never decreases.
- Fallback 30.0 matches the expected Oracle cap.

---

## 6. POC helpers integration

Ported from `mod/auto_scout_test/game/ai/human_assist/human_assist.xs:53-245`, prefixed with `autoScout_`.

| Helper | Source lines | Purpose |
|---|---|---|
| `autoScout_explorePlanAssignAreas(planID, ref int[] areaIDs)` | POC:53-59 | Resize and write `cExplorePlanExploreAreaIDs`. |
| `autoScout_removeExploredAreas(ref int[] areas)` | POC:191-202 | Drop areas where `kbAreaGetPercentExplored >= 1.0`. |
| `autoScout_helperExploreStartingSurroundings(planID)` | POC:67-112 | Assign start-position area ring; used as fallback if BFS has no candidates. |
| `autoScout_helperExploreFarAreas(planID, scoutPos)` | POC:115-142 | Assign entire reachable area group; used as deep fallback. |
| `autoScout_exploreCurrentArea(unitID, planID)` | POC:205-216 | Assign only the scout's current area if unexplored. |
| `autoScout_exploreNeighborAreas(unitID, planID)` | POC:219-245 | Assign current area + direct border areas, filtered by explored. |

### Constants / arrays used

- `cExplorePlanExploreAreaIDs` (writable array, value 21, `docs/MythTRConstants.txt:3352`).
- `cExplorePlanExploreAreaIDsCurrentIndex` (read-only, value 22, `docs/MythTRConstants.txt:3353`).
- `cPlanStateIdle` = 23, `cPlanStateExplore` = 6 (`docs/MythTRConstants.txt:3149,3166`).
- `cUnitStatLOS` = 7, `cProtoStatLOS` = 6.
- Reuse existing `gAutoScout_areaClaim`, `gAutoScout_areaSelfScouted`, and `gAutoScout_heat` arrays.

---

## 7. Modified `enableAutoScouting` wrapper

Current `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:95-113`:

```xs
void enableAutoScouting(int unitID = -1)
{
   ...
   int planID = aiPlanCreate("...", cPlanExplore);
   aiPlanAddUnitType(...);
   aiPlanAddUnit(...);
   if (kbUnitIsType(unitID, cUnitTypeAbstractOracle) == true)
   {
      aiPlanSetVariableBool(planID, cExplorePlanDoLoops, 0, false);
      aiPlanSetVariableFloat(planID, cExplorePlanStopLOSPercentage, 0, 0.2);
   }
   autoScout_register(planID, unitID);
   ...
}
```

New wrapper:

```xs
void enableAutoScouting(int unitID = -1)
{
   ...
   int planID = aiPlanCreate("Autoscout with unit: " + unitID, cPlanExplore);
   aiPlanAddUnitType(planID, cUnitTypeUnit, 1, 1, 1);
   aiPlanAddUnit(planID, unitID);

   // Engine knobs: no looping for anyone; no AvoidingAttackedAreas (inert for player scouts).
   aiPlanSetVariableBool(planID, cExplorePlanDoLoops, 0, false);

   if (kbUnitIsType(unitID, cUnitTypeAbstractOracle) == true)
   {
      aiPlanSetVariableFloat(planID, cExplorePlanStopLOSPercentage, 0, 0.8);
   }

   autoScout_register(planID, unitID); // now leaves plan active
   aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true);
   aiPlanSetFlag(planID, cPlanFlagRequiresAllNeedUnits, true);
   ...
}
```

**Order of operations:** create plan → add unit → set engine knobs (`DoLoops=false`, Oracle `StopLOSPercentage=0.8`) → register → set flags. No `cExplorePlanAvoidingAttackedAreas` per user decision.

---

## 8. File-by-file implementation outline

### 8.1 `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`

| Section | Change | Approx lines | Status |
|---|---|---|---|
| Top constants | Add `cAutoScout_PlanState*` enum, `cPlanStateExplore` alias, `gAutoScout_planState[]`, `gAutoScout_oracleMaxLOS[]` | +40 | New |
| POC helpers | Add 6 assignment helpers (`autoScout_explorePlanAssignAreas`, `autoScout_removeExploredAreas`, etc.) | +200 | New |
| Area-list pipeline | Add `autoScout_buildAreaListFromPath`, `autoScout_filterDangerousAreas`, `autoScout_claimAndAssignAreas` | +80 | New |
| Lifecycle helpers | Add `autoScout_parkPlan`, `autoScout_unparkPlan`, `autoScout_unparkAndReassignAreas` | +60 | New |
| `autoScout_register` (`auto_scout.xs:2898-2936`) | Remove `aiPlanSetState(..., cPlanStateIdle)`; add active mode + immediate reassign | -10 / +25 | Modified |
| Heat-map block (`auto_scout.xs:600-1372`) | Keep data structures and updates; remove manual-move consumers | -80 | Retained |
| Herd block (`auto_scout.xs:1375-1542`, `auto_scout.xs:2940-2999`) | Keep; add park/un-park calls around divert and home-move scan | +40 | Retained |
| Oracle block (`auto_scout.xs:1549-1693`, `auto_scout.xs:2467-2627`) | Replace action-37 state machine with LOS monitor; keep overlap heuristics | -180 / +40 | Modified |
| BFS block (`auto_scout.xs:1699-2254`) | Keep `autoScout_findNextArea`; remove corridor consumers | -40 | Retained |
| Frontier-walk (`auto_scout.xs:2260-2310`) | Delete `autoScout_findFrontierWaypoint` and constants | -200 | Deleted |
| Corridor (`auto_scout.xs:2330-2396`) | Delete `autoScout_issueCorridor`, `autoScout_corridorFirstDangerousHop`, `gAutoScout_corridorAreas/Len`, `gAutoScout_bfsResult*` consumers | -180 | Deleted |
| Waypoint memory (`auto_scout.xs:367-376`, `auto_scout.xs:438-473`) | Delete `gAutoScout_workVisited*` and helpers | -80 | Deleted |
| `autoScout_tickUnit` (`auto_scout.xs:2632-2892`) | Simplify to danger/flee/herd checks and engine-plan mode tracking; remove WALKING/WORKING manual-move logic | -350 / +80 | Modified |
| New rule `autoScout_tickOracleLOS` | New rule + `autoScout_getOracleMaxLOS` | +70 | New |
| `autoScout_tickHeavy` (`auto_scout.xs:3020-3029`) | Keep heat-map + homeMove; remove frontier call | -20 | Modified |
| `autoScout_tickFast` (`auto_scout.xs:3031-3053`) | Add Oracle rule invocation; assignment interval gate | +30 | Modified |
| **Net estimate** | | **~+600 / ~-1180** | |

**What survives, what goes**

| Survives | Lines | Deleted | Lines |
|---|---|---|---|
| State/behaviour enums + pool management | 15-563 | Frontier-walk + constants | 186-205, 2260-2310 |
| Heat-map structures + update | 600-1372 | Corridor state + issuance | 220-255, 2330-2396 |
| Herd queries + divert + home-move | 1375-1542, 2940-2999 | `autoScout_issueCorridor` | 2342-2373 |
| Oracle identification + overlap heuristics | 1549-1693 | `autoScout_corridorFirstDangerousHop` | 2381-2396 |
| BFS scoring + candidate logic | 1699-2254 | Oracle action-37 state machine | 2467-2627 |
| `cleanupLingeringExplorePlans` | via human_assist.xs | Waypoint visited memory | 367-376, 438-473 |

### 8.2 `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`

- Lines 95-113: rewrite `enableAutoScouting` as shown in §7.
- Add engine knob `cExplorePlanDoLoops=false` for all scouts.
- Set Oracle `cExplorePlanStopLOSPercentage=0.8`.
- Remove any `cExplorePlanAvoidingAttackedAreas` usage.
- `autoScout_register(planID, unitID)` call retained.

### 8.3 `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`

- Verify `include "human_assist/auto_scout.xs"` remains at L15.
- Apply the same `enableAutoScouting` changes as 8.2.

### 8.4 `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs`

- Verify `include "human_assist/auto_scout.xs"` remains at L15.
- Apply the same `enableAutoScouting` changes as 8.2 (auto_relic_delivery include unaffected).

### 8.5 `scripts/deploy-mods.sh`

- Confirm `auto_scout.xs` copied to:
  - Intelligent Auto-Scout (`deploy` at L79)
  - Intelligent Auto-Repair and Scout (`deploy` at L84)
  - Human Assist Improvements (`deploy` at L89)
- No changes to the script.

---

## 9. Tick rules and performance

| Rule | minInterval | Work per tick | Cost after migration |
|---|---|---|---|
| `autoScout_tickHeavy` | 1 | Rebuild heat-map, homeMove scan | Unchanged (heat-map is the heavy work) |
| `autoScout_tickFast` | 1 | Iterate slots, run danger/flee/herd checks, track engine-plan modes | Reduced: no frontier sampling, no corridor math, no repeated `aiTaskMoveUnit` issuance |
| `autoScout_tickOracleLOS` | 1 | For each Oracle, read LOS, compare ratio, park/resume | New, but bounded by small Oracle count |
| `cleanupLingeringExplorePlans` | 15 | Destroy empty `cPlanExplore` plans | Unchanged |

**Performance targets**
- `autoScout_tickFast` cost reduced by ~50 % by deleting frontier-walk, corridor chains, and waypoint-memory bookkeeping.
- `autoScout_tickHeavy` cost unchanged; heat-map rebuild dominates.
- BFS + area-list assignment gated to every 5 ticks by default (adjustable constant `cAutoScout_AssignmentInterval`).

---

## 10. Risk-specific mitigations

| Risk | Mitigation in design |
|---|---|
| **R8 HIGH** — stale area list after un-park | `autoScout_unparkAndReassignAreas` is the single reactivation path. It runs BFS → heat-map filter → claim → assign BEFORE calling `aiPlanSetState(planID, cPlanStateExplore)` for every transition out of `parked-divert`, `parked-flee`, and `paused-oracle`. If the engine rejects the reassignment (area list appears consumed), the helper logs, leaves the slot in parked state, and sets a retry tick counter. |
| R5 / RP3 — `StopLOSPercentage=0.8` semantics | Documented as an engine hint; the explicit monitor owns the `< 0.5` pause and `1.0` resume. If in-game testing shows the engine auto-pauses at 0.8, the monitor thresholds can be widened without changing the plan-mode logic. |
| R1 — accidental parking | Explicit `gAutoScout_planState` enum and helper functions `autoScout_parkPlan` / `autoScout_unparkPlan` isolate state transitions. `autoScout_register` logs the active mode on creation. |
| R3 — `kbAreaGetIDByPosition` mismatch | `autoScout_buildAreaListFromPath` adds `kbCanPath` guards before border-area fallback. Mismatches are logged with unit ID and current/border area IDs. |
| R6 — filter ordering | Hard-coded order: `findNextArea` first, then `removeExploredAreas`, then `filterDangerousAreas`, then `claimAndAssignAreas`. |
| R7 — `cPlanStateIdle` value | Keep local alias `cAutoScout_PlanStateIdle = 23` and add a source comment referencing `docs/MythTRConstants.txt:3166`. Re-check after every game patch. |
| RP4 — engine area consumption | Un-park reassign assumes fresh area list works. If `cExplorePlanExploreAreaIDsCurrentIndex` persists and the engine ignores the new list, fall back to `aiPlanDestroy` + recreate plan + re-register on resume (destructive but acceptable for edge case). |

---

## 11. Verification matrix

| Capability | Scenario | Steps | Expected `aiEcho` | Pass criteria |
|---|---|---|---|---|
| `engine-area-explore` | Scout in fresh area | Toggle Auto-Scout on regular scout | Area IDs assigned; no `aiTaskMoveUnit` for normal movement | Scout reaches neighbor area |
| `engine-area-explore` | Multiple scouts | Toggle 2 scouts in same area group | Distinct area claims | No duplicate first-area assignment |
| `engine-area-explore` | All areas explored | Let scout run until BFS returns -1 | "no areas to scout" | Plan stays alive but empty list |
| `oracle-los-monitor` | Oracle fresh area | Toggle Oracle Auto-Scout | `StopLOSPercentage=0.8` set | Engine moves Oracle |
| `oracle-los-monitor` | Oracle LOS < 0.5 × max | Wait until LOS drops | `Parking oracle unit N ratio R` | Plan `cPlanStateIdle`, unit stopped |
| `oracle-los-monitor` | Oracle LOS = 1.0 | Wait until saturation | `Resuming oracle unit N ratio 1.0` | Plan active, unit moves |
| `intelligent-auto-scout-movement` | Scout near tower | Toggle near enemy tower | Heat-map filter log; engine drives around danger | Scout does not walk into tower |
| `intelligent-auto-scout-movement` | Scout divert | Scout near herd | Park log, manual move, un-park log, area list re-assigned | Herd converted + home-move |
| `auto-scout-plan-lifecycle` | Scout killed | Kill scout during any mode | `dropFromPool` + `cleanupLingeringExplorePlans` | Plan destroyed |
| Mod variants | Deploy all 3 | Run `scripts/deploy-mods.sh` | `auto_scout.xs` copied to all 3 variants | Each variant toggles Auto-Scout |

---

## 12. Open research (carry-over from spec)

- **RP3**: Exact engine semantics of `cExplorePlanStopLOSPercentage` for Oracles — verify during in-game testing whether 0.8 is a routing hint or a hard pause.
- **RP4**: Whether `cExplorePlanExploreAreaIDsCurrentIndex` resets or persists across `cPlanStateIdle` → `cPlanStateExplore` transitions; if it persists, the fallback in §10 is used.
- **RP2**: Whether `kbPlayerGetProtoStatInt(..., cProtoStatLOS)` already includes AutoLOS buffs; compare cache against observed saturation LOS.

---

## 13. Next phase

**next_recommended: `tasks`**

Design is complete. The task breakdown should produce a sequenced implementation plan covering:
1. Add new constants, arrays, and lifecycle helpers.
2. Port POC assignment helpers and build the BFS → filter → assign pipeline.
3. Modify `autoScout_register`, `autoScout_tryDivert`, flee handling, and Oracle LOS monitor.
4. Delete frontier-walk, corridor, and waypoint-memory code.
5. Update `enableAutoScouting` in all three mod variants.
6. Manual verification per §11.
