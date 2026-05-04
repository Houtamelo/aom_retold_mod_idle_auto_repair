# Intelligent Auto-Scout — Herd Diversion (deferred)

**Date:** 2026-05-04
**Status:** Designed, not yet implemented. Resume when scouting core is stable.

## Goal

Make auto-scouting units divert briefly to claim nearby Gaia/nature-owned herdables (cows, goats, pigs, etc.) before continuing their assigned scouting target. AoMR converts herdable ownership when a player's unit gets close to it; we exploit that automatic conversion to give the player free food without manual micro.

## Detection

A single global unit query is created lazily on first use:

```xs
extern int gAutoScout_herdQuery = -1;

void autoScout_initHerdQuery() {
   if (gAutoScout_herdQuery >= 0) { return; }
   gAutoScout_herdQuery = kbUnitQueryCreate("autoScout_herds");
   kbUnitQuerySetPlayerRelation(gAutoScout_herdQuery, cPlayerRelationGaia);
   kbUnitQuerySetUnitType(gAutoScout_herdQuery, cUnitTypeHerdable);
   kbUnitQuerySetState(gAutoScout_herdQuery, cUnitStateAlive);
   kbUnitQuerySetAscendingSort(gAutoScout_herdQuery, true); // closest first
}
```

**Open question at implementation time:** verify `cPlayerRelationGaia` exists. If not, fall back to passing player ID `0` (Gaia is conventionally player 0 in AoE-engine games) via `kbUnitQuerySetPlayerID(query, 0, false)`.

The query is parameterized per-call (position + maxDistance) so it can be reused across all scouts:

```xs
int autoScout_findVisibleHerd(int scoutUnitID, float los) {
   autoScout_initHerdQuery();
   vector pos = kbUnitGetPosition(scoutUnitID);
   kbUnitQuerySetPosition(gAutoScout_herdQuery, pos);
   kbUnitQuerySetMaximumDistance(gAutoScout_herdQuery, los);
   kbUnitQueryResetResults(gAutoScout_herdQuery);
   int n = kbUnitQueryExecute(gAutoScout_herdQuery);
   for (int i = 0; i < n; i++) {
      int herdID = kbUnitQueryGetResult(gAutoScout_herdQuery, i);
      if (herdID < 0) { continue; }
      vector herdPos = kbUnitGetPosition(herdID);
      int unitProto = kbUnitGetProtoUnitID(scoutUnitID);
      if (kbCanPath(pos, herdPos, unitProto, 1.0, herdID) == false) { continue; }
      return(herdID);
   }
   return(-1);
}
```

Detection radius defaults to scout's full LOS. Tightening to LOS/2 is a possible tuning later if scouts wander too far off-path.

## State machine extension

One new state plus three new parallel-array fields:

```xs
const int cAutoScoutState_Diverting = 3;

extern int[]    gAutoScout_savedState     = default; // state to resume after divert
extern vector[] gAutoScout_savedWaypoint  = default; // waypoint to resume after divert
extern int[]    gAutoScout_targetHerdID   = default; // herd being claimed (for completion check)
```

These three arrays are appended/removed alongside the existing pool arrays in `autoScout_register` and `autoScout_dropFromPool`.

## Tick logic

Insert at the top of WALKING and WORKING handlers, **before the arrival check**:

```xs
int herdID = autoScout_findVisibleHerd(unitID, los);
if (herdID >= 0 && kbUnitGetIsIDValid(herdID) == true) {
   gAutoScout_savedState[slot]    = state;
   gAutoScout_savedWaypoint[slot] = gAutoScout_targetWaypoint[slot];
   gAutoScout_targetHerdID[slot]  = herdID;
   gAutoScout_state[slot]         = cAutoScoutState_Diverting;
   gAutoScout_targetWaypoint[slot] = kbUnitGetPosition(herdID);
   gAutoScout_stuckTicks[slot]    = 0;
   aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
   return(true);
}
```

New DIVERTING handler:

```xs
if (state == cAutoScoutState_Diverting) {
   int herdID = gAutoScout_targetHerdID[slot];
   bool done = false;
   if (kbUnitGetIsIDValid(herdID) == false) { done = true; }
   else if (kbUnitGetPlayerID(herdID) == cMyID) { done = true; }
   else if (autoScout_arrived(unitID, gAutoScout_targetWaypoint[slot],
                              gAutoScout_stuckTicks[slot]) == true) { done = true; }

   if (done == true || gAutoScout_stuckTicks[slot] >= cAutoScout_StuckTickLimit) {
      gAutoScout_state[slot]         = gAutoScout_savedState[slot];
      gAutoScout_targetWaypoint[slot] = gAutoScout_savedWaypoint[slot];
      gAutoScout_stuckTicks[slot]    = 0;
      aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
      return(true);
   }

   gAutoScout_stuckTicks[slot] = gAutoScout_stuckTicks[slot] + 1;
   aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
   return(false);
}
```

Completion conditions (any one suffices):
1. Herd no longer valid (died).
2. Herd's `kbUnitGetPlayerID == cMyID` (someone, possibly us, claimed it — happens automatically on proximity).
3. Scout arrived at the herd's position.
4. Stuck timeout (60s) — give up, restore.

## Coordination tradeoffs

- **No herd-claim arrays.** Two scouts seeing the same herd will both divert. The first arrival flips ownership; the second's tick sees `playerID == cMyID` and instantly restores. Wasted detour for the second scout, but small (LOS distance) and rare. Adding a per-herd claim array doubles the bookkeeping and isn't worth it.
- **Re-trigger guard is automatic.** Once the herd's playerID flips to ours, the next `autoScout_findVisibleHerd` query returns nothing (filtered by `cPlayerRelationGaia`). No infinite-divert loop.
- **Visibility-gated.** Scout only diverts for herds within its LOS at the moment of the check. Avoids ghost-divert from herds the scout discovered earlier but currently can't see.

## Performance

One extra unit query per scout per tick during WALKING/WORKING/DIVERTING. With `minInterval 1` and ~5 active scouts, ~5 queries/sec. Negligible against the per-AI ~5ms-per-tick budget.

## Filtering decisions (locked in)

- **Convertible only.** `cUnitTypeHerdable` (excludes `cUnitTypeNonConvertableHerdable` since those can't be claimed).
- **Gaia-owned only.** Already-claimed herdables (any non-Gaia owner including ours) skipped naturally by the player-relation filter.
- **Reachable only.** `kbCanPath` gate inside `findVisibleHerd` skips herds in disconnected area-groups (e.g., across water without a transport).

## Out of scope

- **Herding multiple herdables in one trip.** Possible refinement: while in DIVERTING, on completion check, immediately re-scan for ANOTHER nearby herd before restoring the saved waypoint. Defer until v1 ships.
- **Combat units (Berserk, etc.) attacking the herd instead of just walking past.** AoMR's auto-attack might trigger if the scout's tactic includes Hunting. We don't override the tactic; if the engine's auto-attack-on-Hunting path kicks in, the scout will eat the herd instead of converting. Acceptable side effect — player still gets food. Worst case it's slower than conversion. If the user reports problems, gate diversion to non-combat scouts (Pegasus, Priest, Kataskopos) only.
- **Air scouts (Pegasus, Raven, SkyLantern) "claiming" herds.** Air units don't trigger ground-based proximity conversion in AoMR. They'd divert pointlessly. At implementation time, gate diversion behind `kbUnitIsType(scoutID, cUnitTypeAbstractFlyingUnit) == false`.
