# Intelligent Auto-Scout — Herd Diversion (deferred)

**Date:** 2026-05-04
**Status:** Designed, not yet implemented. Resume when scouting core is stable.

## Goal

Make auto-scouting units divert briefly to claim nearby Gaia or enemy-owned herdables (cows, goats, pigs, etc.) before continuing their assigned scouting target. AoMR converts herdable ownership when a player's eligible unit gets close to it; we exploit that automatic conversion to give the player free food without manual micro. After conversion (whether caused by an intentional divert or by a scout incidentally walking past), the herdable is automatically issued a move-to-nearest-TC order so it lands somewhere useful.

## State machine extension

One new state and one new per-scout field:

```xs
const int cAutoScoutState_Diverting = 3;

extern int[] gAutoScout_targetHerdID = default;  // herd being diverted to
```

`gAutoScout_targetHerdID` is appended/removed alongside the existing pool arrays in `autoScout_register` and `autoScout_dropFromPool`.

**No saved-state arrays.** On divert completion the scout releases its area claim and transitions to `Idle`; the next tick's BFS picks the best next target from the scout's *new* position. This is a strict simplification over saving and restoring the pre-divert state — the post-divert position may already be closer to a different candidate area, and the original target may have been claimed by another scout in the meantime.

## Per-herd globals

Two parallel ID-list arrays, append-only, never unmarked. Linear scan to filter (small set, bounded by herd count on the map):

```xs
extern int[] gAutoScout_attemptedHerdIDs  = default;  // any herd ever selected for divert
extern int[] gAutoScout_redirectedHerdIDs = default;  // any herd ever auto-home-moved
```

`attemptedHerdIDs`: a herd is appended the moment any scout selects it for diversion. Subsumes the "two scouts targeting the same herd" coordination — the second scout filters it out. Also handles the "stuck/unreachable herd" case: if the first scout fails (timeout, herd dies, etc.), the herd stays marked and no other scout retries.

`redirectedHerdIDs`: a herd is appended the moment the home-move scan issues `aiTaskMoveUnit` for it. The single-issue guarantee prevents fighting with player command overrides.

Trivial linear-scan helpers (definitions implied):

```xs
bool autoScout_isHerdAttempted(int herdID)   { /* return gAutoScout_attemptedHerdIDs contains herdID */ }
bool autoScout_isHerdRedirected(int herdID)  { /* return gAutoScout_redirectedHerdIDs contains herdID */ }
```

## Eligibility gate

Before any divert detection runs for a scout:

```xs
if (kbProtoUnitIsType(cMyID, kbUnitGetProtoUnitID(unitID),
                      cUnitTypeLogicalTypeConvertsHerds) == false) { /* skip divert */ }
```

`LogicalTypeConvertsHerds` is the data-driven ground truth. Verified in AoMR's `proto.xml`:

- Has it: Kataskopos, Berserk, Priest, Oracle, SkyLantern (surprisingly — air, but data says it converts), and ~531 other units.
- Does not have it: Pegasus, Raven (air scouts that don't convert).

This is preferable to a negative check on `cUnitTypeAbstractFlyingUnit`, which would mis-handle SkyLantern.

## Detection

A single global unit query, created lazily on first use:

```xs
extern int gAutoScout_herdQuery = -1;

void autoScout_initHerdQuery()
{
   if (gAutoScout_herdQuery >= 0) { return; }
   gAutoScout_herdQuery = kbUnitQueryCreate("autoScout_herds");
   kbUnitQuerySetPlayerRelation(gAutoScout_herdQuery, cPlayerRelationEnemyOrNeutral);
   kbUnitQuerySetUnitType(gAutoScout_herdQuery, cUnitTypeHerdable);
   kbUnitQuerySetState(gAutoScout_herdQuery, cUnitStateAlive);
   kbUnitQuerySetAscendingSort(gAutoScout_herdQuery, true);  // closest first
}
```

Reused per-call by parameterizing position + max distance:

```xs
int autoScout_findVisibleHerd(int scoutUnitID, float los)
{
   autoScout_initHerdQuery();
   vector pos = kbUnitGetPosition(scoutUnitID);
   kbUnitQuerySetPosition(gAutoScout_herdQuery, pos);
   kbUnitQuerySetMaximumDistance(gAutoScout_herdQuery, los);
   kbUnitQueryResetResults(gAutoScout_herdQuery);
   int n = kbUnitQueryExecute(gAutoScout_herdQuery);
   int unitProto = kbUnitGetProtoUnitID(scoutUnitID);
   for (int i = 0; i < n; i++)
   {
      int herdID = kbUnitQueryGetResult(gAutoScout_herdQuery, i);
      if (herdID < 0) { continue; }
      if (autoScout_isHerdAttempted(herdID) == true) { continue; }
      vector herdPos = kbUnitGetPosition(herdID);
      if (kbCanPath(pos, herdPos, unitProto, 1.0, herdID) == false) { continue; }
      return(herdID);
   }
   return(-1);
}
```

Detection radius defaults to scout's full LOS. Tightening to LOS/2 is a possible tuning later if scouts wander too far off-path.

**Movement-type compatibility:** all herdables in AoMR's `proto.xml` are `<movementtype>land</movementtype>` (Cow, Goat, Pig, Capybara, Turkey). `kbCanPath` derives the scout's movement type from its proto, so a land scout's `kbCanPath` correctly fails for a water-island herd. No explicit movement-type compare is needed — the existing pathing gate covers compatibility.

## Tick logic

Insert at the top of WALKING and WORKING handlers in `autoScout_tickUnit`, **before the arrival check**, and only if the scout passes the eligibility gate:

```xs
if (kbProtoUnitIsType(cMyID, kbUnitGetProtoUnitID(unitID),
                      cUnitTypeLogicalTypeConvertsHerds) == true)
{
   int herdID = autoScout_findVisibleHerd(unitID, los);
   if (herdID >= 0 && kbUnitGetIsIDValid(herdID) == true)
   {
      gAutoScout_attemptedHerdIDs.add(herdID);
      gAutoScout_targetHerdID[slot]   = herdID;
      gAutoScout_state[slot]          = cAutoScoutState_Diverting;
      gAutoScout_targetWaypoint[slot] = kbUnitGetPosition(herdID);
      gAutoScout_stuckTicks[slot]     = 0;
      aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
      return(true);
   }
}
```

DIVERTING handler:

```xs
if (state == cAutoScoutState_Diverting)
{
   int herdID = gAutoScout_targetHerdID[slot];
   bool done = false;
   if (kbUnitGetIsIDValid(herdID) == false) { done = true; }
   else if (kbUnitGetPlayerID(herdID) == cMyID) { done = true; }
   else if (autoScout_arrived(unitID, gAutoScout_targetWaypoint[slot],
                              gAutoScout_stuckTicks[slot]) == true) { done = true; }

   if (done == true || gAutoScout_stuckTicks[slot] >= cAutoScout_StuckTickLimit)
   {
      autoScout_releaseClaim(slot);
      autoScout_setStateIdle(slot);
      gAutoScout_targetHerdID[slot] = -1;
      return(true);
   }

   gAutoScout_stuckTicks[slot] = gAutoScout_stuckTicks[slot] + 1;
   gAutoScout_targetWaypoint[slot] = kbUnitGetPosition(herdID);  // re-target if herd wandered
   aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
   return(false);
}
```

Completion conditions (any one suffices):
1. Herd no longer valid (died).
2. Herd's `kbUnitGetPlayerID == cMyID` (any cause — our divert, another of our scouts walking past, anything).
3. Scout arrived at the herd's position (engine didn't trigger conversion within reach — give up, the herd is already in `attemptedHerdIDs` from selection so it won't be retried).
4. Stuck timeout (`cAutoScout_StuckTickLimit` ticks) — same outcome: release claim, go Idle.

The home-move command is **not** issued from the DIVERTING handler — it is issued by the home-move scan below, which catches conversions from any cause uniformly.

## Home-move scan

Runs once per firing of `autoScout_tick` (the same `minInterval 1` rule that ticks scouts), after the scout loop:

```xs
extern int gAutoScout_ownedHerdQuery = -1;

void autoScout_initOwnedHerdQuery()
{
   if (gAutoScout_ownedHerdQuery >= 0) { return; }
   gAutoScout_ownedHerdQuery = kbUnitQueryCreate("autoScout_ownedHerds");
   kbUnitQuerySetPlayerRelation(gAutoScout_ownedHerdQuery, cPlayerRelationSelf);
   kbUnitQuerySetUnitType(gAutoScout_ownedHerdQuery, cUnitTypeHerdable);
   kbUnitQuerySetState(gAutoScout_ownedHerdQuery, cUnitStateAlive);
}

void autoScout_homeMoveScan()
{
   autoScout_initOwnedHerdQuery();
   kbUnitQueryResetResults(gAutoScout_ownedHerdQuery);
   int n = kbUnitQueryExecute(gAutoScout_ownedHerdQuery);
   for (int i = 0; i < n; i++)
   {
      int herdID = kbUnitQueryGetResult(gAutoScout_ownedHerdQuery, i);
      if (herdID < 0) { continue; }
      if (autoScout_isHerdRedirected(herdID) == true) { continue; }
      vector herdPos = kbUnitGetPosition(herdID);
      vector tcPos = autoScout_findNearestTC(herdPos);
      if (tcPos == cInvalidVector) { continue; }
      aiTaskMoveUnit(herdID, tcPos, false, false);
      gAutoScout_redirectedHerdIDs.add(herdID);
   }
}
```

`autoScout_findNearestTC` enumerates our TownCenters (via a unit query for `cMyID` + `cUnitTypeTownCenter`) and returns the position of the closest one to `herdPos`. If no TC exists, returns `cInvalidVector` and the herd is left in place (will be redirected the next tick a TC exists).

The scan covers conversions from **any cause** uniformly: intentional divert AND organic walk-past during normal scouting both end with the herd's playerID flipped to ours, and the scan picks it up on the next tick. Once issued, the herd is added to `redirectedHerdIDs` and never re-issued, so subsequent player overrides stick.

## Filtering decisions

- **Convertible herds only:** `cUnitTypeHerdable` (excludes `cUnitTypeNonConvertableHerdable`).
- **Faction:** `cPlayerRelationEnemyOrNeutral` — Gaia and enemy players, excludes our own and ally herds.
- **Reachable only:** `kbCanPath` gate inside `findVisibleHerd` skips herds in disconnected area-groups.
- **Eligible converters only:** `kbProtoUnitIsType(... cUnitTypeLogicalTypeConvertsHerds)` — filters out air scouts (Pegasus, Raven) without false negatives (SkyLantern).
- **Already attempted skipped:** the `attemptedHerdIDs` filter prevents wasted retries on stuck/unreachable herdables.

## Coordination tradeoffs

- **No per-herd claim array.** The `attemptedHerdIDs` permanent mark already prevents double-divert: once scout A selects a herd, scout B skips it on the next query. Single coordination mechanism, no separate "claim" bookkeeping.
- **Re-trigger guard is automatic.** Once a herd's playerID flips to ours, the divert query (relation = enemy-or-neutral) returns nothing for it. No infinite-divert loop.
- **Home-move single-issue.** `redirectedHerdIDs` permanent mark guarantees the move-to-TC is issued exactly once per herd, so player command overrides aren't fought.
- **Visibility-gated detection.** Scout queries herds within its current LOS at the moment of the check. AoMR's default unit-query visibility honors fog-of-war, so we won't divert toward herds the player has never seen.

## Performance

Per `autoScout_tick` firing:
- Per scout: 1 herd query + small linear scan of `attemptedHerdIDs` (bounded by herd count).
- Once: 1 owned-herd query + small linear scan of `redirectedHerdIDs`.

With ~5 active scouts and minInterval 1, this is ~6 queries/sec. Negligible against the per-AI ~5ms-per-tick budget.

## Verify at implementation time

- `cPlayerRelationEnemyOrNeutral` constant exists in AoMR's XS exports. If not, fall back to `cPlayerRelationAny` with manual `kbUnitGetPlayerID` filtering against `cMyID` and ally relations.
- `cUnitTypeLogicalTypeConvertsHerds` constant exists (or whatever the conventional naming is for `LogicalTypeConvertsHerds`). The data tag is confirmed in `proto.xml`; the XS-side constant naming convention should match.
- TC enumeration: a single `cUnitTypeTownCenter` + `cPlayerRelationSelf` unit query is the most direct approach. Verify the type constant.

## Out of scope (deferred refinements)

- **Chained herding (multiple herdables in one trip).** On DIVERTING completion, immediately re-scan for ANOTHER nearby herd before going Idle. Defer until v1 ships and we see whether one-at-a-time is adequate.
- **Detection radius tuning.** Defaults to full LOS; tighten to LOS/2 if scouts wander too far off-path.
- **Stuck-detection refinement.** Currently a fixed tick count; could be made distance-based (no progress over N ticks) for more robustness against slow units.
