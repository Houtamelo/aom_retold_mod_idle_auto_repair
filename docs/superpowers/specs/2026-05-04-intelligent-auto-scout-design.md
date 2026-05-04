# Intelligent Auto-Scout — Design

**Date:** 2026-05-04
**Mod:** `intelligent_auto_scout` (separate from `idle_auto_repair`; combo mod planned later)

## Goal

Replace the engine's player-triggered `cPlanExplore` behavior for non-Oracle `AbstractScout` units with a frontier-based exploration system that:

1. Prioritizes nearby unexplored areas via BFS over the area-adjacency graph.
2. Coordinates multiple scouts via per-area claims (no two scouts go to the same area).
3. Adapts to map shape — handles peninsulas, corridors, islands — by leveraging the engine's area graph + `kbCanPath` reachability.
4. Auto-stops (via `disableAutoScouting`) when no candidate areas remain.

Rationale: previous spiral-based design (loop primitives in `cPlanExplore`) produced a fixed octagonal sweep around the TC that left "always-skipped" pie slices and degraded badly on out-of-bounds and corridor maps. The frontier approach addresses the underlying spec ("prioritize nearby unexplored areas") directly using fog data the engine already exposes.

## Scope

**In:**
- All `AbstractScout` units except Oracle.
- Test priority units: Kataskopos (Greek), Berserk (Norse), Priest (Egyptian).
- Land + naval + air movement types via single per-unit movement-type filter.

**Out (deferred):**
- Oracle: keeps vanilla `StopLOSPercentage=0.2` scry-in-place behavior.
- Herd diversion (next iteration).
- Pre-emptive enemy avoidance (next iteration).
- AI-player scouts (chairon's planning — untouched).

## Architecture

### File layout
- `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` — all logic.
- `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs` — vanilla copy + 1 include + 1 function call inside `enableAutoScouting` (already wired from prior iterations).

### Hook into vanilla `enableAutoScouting`
Existing `autoScout_configurePlan(planID, unitID)` call is renamed `autoScout_register(planID, unitID)`. It:
- Skips Oracle units (vanilla scry-in-place branch already handles).
- For non-Oracle scouts: registers the unit in our scout pool, makes the engine plan inert by setting `cExplorePlanNumberOfLoops=0` (plan still exists as housekeeping marker — its existence == "auto-scout is on for this unit"; engine destroys it on `disableAutoScouting`).

### Per-scout state (parallel arrays, indexed by pool slot)
```
gAutoScout_unitID[]        // unit ID
gAutoScout_planID[]        // associated cPlanExplore ID (housekeeping marker)
gAutoScout_state[]         // cAutoScoutState_Idle | _Walking | _Working
gAutoScout_targetAreaID[]  // area currently claimed
gAutoScout_targetWaypoint[] // current move destination (vector)
gAutoScout_ringIdx[]       // current mini-ring index in WORKING state
gAutoScout_pointIdx[]      // current waypoint index within ring
```

### Global per-area state
```
gAutoScout_areaClaim[]  // sized to kbAreaGetNumber() at first use; value = scout unit ID, 0 = unclaimed
```

### Per-tick rule (`autoScout_tick`, `minInterval 2`)
For each scout in the pool, validate then advance state:

1. **Validate.** Drop from pool (releasing area claim) if any of:
   - Unit no longer valid: `kbUnitGetIsIDValid(unitID) == false` (covers death + removal).
   - Unit no longer ours: `kbUnitGetPlayerID(unitID) != cMyID`.
   - Stored plan no longer valid: `aiPlanGetIsIDValid(planID) == false` (means vanilla `disableAutoScouting` ran, or engine cleanup destroyed plan after player override).
   - State-stuck timeout: stayed in WALKING or WORKING for > N consecutive ticks without progress (default N = 30 ticks ≈ 60s with `minInterval 2`). Releases claim, returns to IDLE so BFS picks something else.

2. **State machine.**
   - **IDLE** → BFS from scout's current area. If candidate found: claim it, set targetWaypoint = `kbAreaGetCenter(areaID)`, transition to WALKING, issue `aiTaskUnitMove`. If no candidate: call `disableAutoScouting(unitID)`, drop from pool.
   - **WALKING** → arrival = `kbUnitGetDistanceToPoint(unitID, target) <= LOS * cAutoScout_ArrivalSlackMultiplier` OR `kbUnitGetActionType(unitID) == cActionTypeIdle` (engine pathed as close as it could):
     - If target area is "small" (`sqrt(kbAreaGetNumberTiles(area)) <= LOS`) OR scout never actually entered the area (`kbAreaGetIDByPosition(unitPos) != targetAreaID` — non-convex area, centroid was outside): release claim, transition to IDLE.
     - Else: transition to WORKING with `ringIdx=0, pointIdx=0`, compute first mini-ring waypoint and issue move.
     - If not arrived: re-issue `aiTaskUnitMove` with current waypoint (defensive, in case engine plan stole the task).
   - **WORKING** → if scout reached current mini-ring waypoint:
     - Advance `pointIdx`. If beyond ring's point count: advance `ringIdx`, reset `pointIdx`. If beyond outermost ring: release claim, transition to IDLE.
     - Compute next waypoint position. Skip if (a) outside target area (`kbAreaGetIDByPosition(wp) != targetAreaID`) or (b) `kbCanPath` fails.
     - Issue `aiTaskUnitMove`.

### BFS algorithm
```
int autoScout_findNextArea(int scoutUnitID) {
   int startArea = kbAreaGetIDByPosition(kbUnitGetPosition(scoutUnitID));
   if (startArea < 0) return -1;

   int[] queue = ...;     // ring buffer with head/tail indices
   int[] visited = ...;   // sized to kbAreaGetNumber(); 1 if enqueued
   enqueue(queue, startArea); visited[startArea] = 1;

   while (queue not empty) {
      int areaID = dequeue(queue);
      if (autoScout_areaIsCandidate(areaID, scoutUnitID)) return areaID;
      int n = kbAreaGetNumberBorderAreas(areaID);
      for (int i = 0; i < n; i++) {
         int next = kbAreaGetBorderAreaID(areaID, i);
         if (next < 0 || visited[next] == 1) continue;
         visited[next] = 1;
         enqueue(queue, next);
      }
   }
   return -1;
}
```

### Candidate criteria
```
bool autoScout_areaIsCandidate(int areaID, int scoutUnitID) {
   if (gAutoScout_areaClaim[areaID] != 0) return false;
   int totalTiles = kbAreaGetNumberTiles(areaID);
   if (totalTiles <= 0) return false;
   int blackTiles = kbAreaGetNumberBlackTiles(areaID);
   if (blackTiles * 100 / totalTiles < 10) return false;
   vector areaPos = kbAreaGetCenter(areaID);
   vector unitPos = kbUnitGetPosition(scoutUnitID);
   int unitProto = kbUnitGetProtoUnitID(scoutUnitID);
   if (!kbCanPath(unitPos, areaPos, unitProto, 1.0, -1)) return false;
   return true;
}
```

### Reachability is `kbCanPath`-only
No area-type filter. Engine area types (`cAreaTypePassableLand`, `cAreaTypeWater`, `cAreaTypeAmphibious`, `cAreaTypeForest`, `cAreaTypeGold`, `cAreaTypeSettlement`) are categorical/overlapping (an area can be both Settlement and PassableLand), so a simple type-equality filter would either miss valid scouting targets (skipping Settlements) or include unwalkable ones. `kbCanPath` is authoritative for "can this unit walk there", which is exactly the question we need answered. For air/naval units the engine still answers correctly via the same call.

### Mini-ring waypoint generation (within a "large" area)
```
ringRadius(r)  = (r + 1) * LOS   // r = 0..numRings-1
numRings       = ceil(sqrt(tileCount) / (2 * LOS))
pointsInRing(r) = max(8, ceil(2 * pi * ringRadius(r) / (2 * LOS)))

waypoint(r, p) = areaCentroid + (cos(angle) * radius, 0, sin(angle) * radius)
   where angle = (2 * pi / pointsInRing(r)) * p
```
Per-waypoint guard: skip if outside target area (`kbAreaGetIDByPosition(wp) != areaID`) or unreachable.

### Player override detection
The vanilla `cleanupLingeringExplorePlans` rule (every 15s) destroys plans with no units. Engine handles player-override unit removal from the plan. Combined with our `aiPlanGetIsIDValid(storedPlanID)` check at validation step → drops within 15s of override at worst, immediately on next tick if engine destroys faster.

## Failure modes and handling

| Scenario | Detection | Response |
|---|---|---|
| Scout dies | Validation step | Drop from pool, release claim |
| Player redirects scout | Plan destroyed by engine cleanup | Drop within ≤15s, release claim |
| Player clicks AutoScoutCancel | Vanilla `disableAutoScouting` destroys plan | Same as above |
| Map fully explored | BFS returns -1 | Call `disableAutoScouting`, drop, release claim |
| Area centroid outside the area (non-convex area) | `kbAreaGetIDByPosition(centroid) != areaID` after walking — detected when checking arrival | Treat as "small area visit done", release claim, transition to IDLE |
| Area becomes claimed mid-BFS (race within a tick) | Cannot happen — scouts processed sequentially | N/A |

## Constants
```
const int cAutoScoutState_Idle    = 0;
const int cAutoScoutState_Walking = 1;
const int cAutoScoutState_Working = 2;

const int   cAutoScout_BlackTilesPercentMin   = 10;  // areas with <10% black tiles considered done
const float cAutoScout_ArrivalSlackMultiplier = 1.0; // arrival = within 1*LOS of target
const int   cAutoScout_StuckTickLimit         = 30;  // ~60s with minInterval 2; release claim if no progress
```

## Test plan

1. Greek game, single Kataskopos. Click AutoScout. Expect: BFS-driven sweep, prioritizing nearby unexplored areas; scout walks from area to area; toggle button reverts to AutoScout once map fully covered.
2. Norse game, three Berserks. AutoScout all three. Expect: each picks a different area (no overlap); BFS prioritizes proximity for each.
3. Egyptian game, Priest scouting. Expect: same behavior as Kataskopos (same code path).
4. Awkward map (corridor / peninsula). Expect: BFS gracefully follows reachable terrain; no waste on water/cliff areas.

## Out-of-scope notes

- **Herd diversion:** would require periodic herd-proximity scan during WALKING/WORKING states. Out for v1.
- **Enemy avoidance:** would require checking `kbAreaGetDangerLevel` in the BFS criteria + replanning when threats appear near scout. Out for v1.
- **Save/reload:** untested; assumption is XS globals persist across saves like vanilla AI globals do. Worst case: pool repopulates over next few ticks once auto-scout is re-clicked.
