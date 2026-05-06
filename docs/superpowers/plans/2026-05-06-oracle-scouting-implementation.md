# Oracle Scouting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the intelligent_auto_scout state machine to handle Atlantean Oracles with a parking-based exploration model that maximises the LOS² × time integral (favor income scales with LOS area while standing still) instead of trying to walk through every tile like vanilla cPlanExplore does.

**Architecture:** Oracles share the existing scout pool but get a dedicated per-tick handler. State machine is `Idle → Walking (move to area centroid) → Stationed (park, grow LOS) → Idle (saturated, repick)`. The diverting state is shared with regular scouts via an extracted helper. A dynamic per-game `gAutoScout_maxOracleLOS` cache (cold-start 20.0) replaces hardcoded cap values; cache updates only when an oracle is observed in the saturated action state (action == 37), which makes it robust to god-tech upgrades and proto modifications. The heuristic gets two new asymmetric rules: oracle source units hard-skip area candidates within `0.8 × MaxOracleLOS` of any other oracle (toggled-on or not), and regular-scout area scores are multiplied by an oracle-discount factor that approaches 0 when areas overlap an oracle's claim circle.

**Tech Stack:** AoMR XS scripting (`auto_scout.xs`). Reuses existing pool/query/BFS infrastructure. No changes to `human_assist.xs`.

---

## Spec recap (locked-in design from brainstorming)

- **Identification:** Both `Oracle` and `OracleHero` carry `<unittype>AbstractOracle</unittype>` and `<unittype>AbstractScout</unittype>`. They keep `LogicalTypeConvertsHerds` so existing herd-divert logic applies unchanged.
- **Saturation signal:** `kbUnitGetActionType(unitID) == 37` (constant `cAutoScout_OracleSaturatedActionType` already declared). Empirically observed (2026-05-06): the engine flips action type from 7 (idle) to 37 (meditation animation) the moment current LOS hits the proto's `modifyratecap`. No documented `cActionType*` constant matches; hardcoded.
- **MaxOracleLOS cache:** single global float `gAutoScout_maxOracleLOS`. Cold-start 20.0 (so the 50% movement floor has a meaningful threshold from tick 1 — `0.5 × 20 = 10` is a sensible "keep walking" floor). Update gated on `action == 37`: only saturated readings can raise the cache. Never decreases.
- **Oracle state machine:** Idle → Walking → Stationed → Idle. `Stationed` is a new state (`cAutoScoutState_Stationed = 4`). No `Working` (frontier-walk) for oracles — their LOS is large enough to cover their reachable fog from a single parked spot. Diverting reuses the existing handler.
- **Heuristic asymmetry:**
  - Regular scout target (existing density penalty for nearby regular scouts/waypoints) PLUS a multiplicative oracle-discount: areas within `MaxOracleLOS` of any oracle get score scaled by `(1 - penalty)` where penalty is the linear overlap proportion summed across oracles, clamped to `[0, 1]`.
  - Oracle target: same density penalty for nearby regular scouts as today, PLUS a HARD-SKIP for areas whose centroid lies within `0.8 × MaxOracleLOS` of any other oracle (toggled-on AND not).
- **Non-toggled oracles:** included in the heuristic. Treated as having a claim radius of `MaxOracleLOS` if not currently moving (`action != cActionTypeMove`); if moving, their claim is just their current dynamic LOS (small, transient). The hard-skip uses raw distance vs `0.8 × MaxOracleLOS` so this distinction is irrelevant for the candidate check; the asymmetry only matters for the score-side multiplicative discount.
- **50% movement floor:** while Walking, if `currentLOS / MaxOracleLOS < 0.5` AND the oracle has not yet reached its target, stop in place and become Stationed. Suspended during Diverting (herd claim outranks LOS preservation).

## File structure

Single file modified: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`. No new files. No changes to `human_assist.xs` — `autoScout_register` is already invoked from `enableAutoScouting`.

Function-order constraint: XS resolves function references at parse time (no forward refs across includes; safer to assume same applies within a file). The Diverting helper extraction must precede both callers; new oracle helpers must precede `autoScout_areaIsCandidate` / `autoScout_areaScore` modifications.

## Important XS quirks to obey while editing

- All non-`ref` user-defined function parameters MUST have default values. `(int x, string y)` is a parse error; use `(int x = -1, string y = "")`.
- Don't use `default` for local arrays; that's extern-globals-only. Use `new int(0, 0)` etc.
- ASCII only inside string literals — em-dashes (`—`) inside `"..."` trigger Token Error 0008. Comments tolerate non-ASCII.
- Avoid compound `if` with `&&` + int math + `>=` — split into intermediate booleans to avoid the parser flake.

## Deployment policy for this plan

**The user is playing the game during execution.** No deploys until the user confirms the game is closed. All changes go to source-of-truth `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` only. The deploy step (copy to `local/Intelligent Auto-Scout/` and `local/Intelligent Auto-Repair and Scout/`) is deferred to after the user signals.

---

## Tasks

### Task 1: Scaffold Oracle constants, state, and globals

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:15-19` (states), constants block (after line 59), globals block (after line 136)

- [ ] **Step 1: Add `cAutoScoutState_Stationed = 4` to the state-constant block**

Existing block at lines 15-18 declares Idle/Walking/Working/Diverting. Insert after Diverting:

```xs
const int cAutoScoutState_Idle      = 0;
const int cAutoScoutState_Walking   = 1;
const int cAutoScoutState_Working   = 2;
const int cAutoScoutState_Diverting = 3;
const int cAutoScoutState_Stationed = 4;
```

- [ ] **Step 2: Add Oracle-specific constants below the existing `cAutoScout_OracleSaturatedActionType` constant**

Insert after the constant currently at line 59:

```xs
// Cold-start value for the dynamic gAutoScout_maxOracleLOS cache. Used until
// any oracle is observed in the saturated action state (action ==
// cAutoScout_OracleSaturatedActionType) with a higher current LOS. 20.0 is
// chosen so the 50% movement floor (cAutoScout_OracleMovementLOSFloor) has a
// meaningful threshold from tick 1, even before any oracle reaches its real cap.
const float cAutoScout_OracleColdCacheMaxLOS = 20.0;

// Movement-state LOS floor for Oracles, expressed as a ratio of MaxOracleLOS.
// While Walking, an Oracle whose currentLOS / MaxOracleLOS drops below this
// ratio stops in place and becomes Stationed, avoiding the vanilla failure
// mode of draining all the way to base LOS (where favor income is minimal).
// Suspended during Diverting -- herd claim outranks LOS preservation.
const float cAutoScout_OracleMovementLOSFloor = 0.5;

// Oracle-vs-oracle exclusion factor used by autoScout_areaIsCandidate when
// the source unit is an Oracle. Areas whose centroid lies within
// cAutoScout_OracleExclusionFactor * gAutoScout_maxOracleLOS of any other
// oracle (toggled-on or not) are skipped. 0.8 leaves a small buffer around
// each oracle's claim so oracles park with adjacent (not overlapping) circles.
const float cAutoScout_OracleExclusionFactor = 0.8;
```

- [ ] **Step 3: Add Oracle globals below the existing extern declarations**

Insert at the end of the globals block (after line 136 in the file as it stands):

```xs
// Dynamic cache: highest currentLOS ever observed on any of our oracles while
// in the saturated action state. Cold-started to cAutoScout_OracleColdCacheMaxLOS;
// climbs only on confirmed saturation events (never decreases). Used as the
// denominator for the 50% movement floor and as the claim radius for the
// oracle-overlap heuristic checks.
extern float gAutoScout_maxOracleLOS = cAutoScout_OracleColdCacheMaxLOS;

// Cached query handle for "all of cMyID's alive AbstractOracle units". Used
// by the heuristic to enumerate every oracle (toggled-on AND not), so player-
// controlled oracles still influence target-area selection.
extern int gAutoScout_oracleQuery = -1;
```

- [ ] **Step 4: Read the modified file to confirm placement and that nothing else was disturbed**

Use Read on lines 1-150.

- [ ] **Step 5: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit -m "feat(scout): scaffold oracle constants, state, max-LOS cache global"
```

---

### Task 2: Oracle helpers — identification, query, cache update, overlap predicates

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` — insert new helpers below the existing herd-diversion helpers section (around line 220, before `autoScout_areaIsCandidate`)

- [ ] **Step 1: Add helpers below the herd-diversion helpers section**

Place AFTER `autoScout_findVisibleHerd` and `autoScout_tryDivert` (those end around line 378), and BEFORE the `Candidate area criteria + BFS` section header:

```xs
//------------------------------------------------------------------------------
// Oracle helpers — identification, query setup, MaxOracleLOS cache update,
// and overlap predicates used by the BFS heuristic.
//------------------------------------------------------------------------------

bool autoScout_isOracle(int unitID = -1)
{
   if (unitID < 0) { return(false); }
   return(kbUnitIsType(unitID, cUnitTypeAbstractOracle));
}

void autoScout_initOracleQuery()
{
   if (gAutoScout_oracleQuery >= 0) { return; }
   gAutoScout_oracleQuery = kbUnitQueryCreate("autoScout_oracles");
   kbUnitQuerySetPlayerID(gAutoScout_oracleQuery, cMyID, false);
   kbUnitQuerySetUnitType(gAutoScout_oracleQuery, cUnitTypeAbstractOracle);
   kbUnitQuerySetState(gAutoScout_oracleQuery, cUnitStateAlive);
}

// Updates gAutoScout_maxOracleLOS only if the given oracle is currently
// saturated AND its current LOS exceeds the cached value. Saturation gating
// prevents mid-growth readings from polluting the cache.
void autoScout_updateMaxOracleLOS(int unitID = -1)
{
   if (unitID < 0) { return; }
   if (kbUnitGetActionType(unitID) != cAutoScout_OracleSaturatedActionType) { return; }
   float current = kbUnitGetStatFloat(unitID, cUnitStatLOS);
   if (current > gAutoScout_maxOracleLOS) { gAutoScout_maxOracleLOS = current; }
}

// Returns true if areaPos lies within (radiusFactor * gAutoScout_maxOracleLOS)
// of ANY of cMyID's alive oracles, excluding excludeUnitID. Used for the
// oracle-vs-oracle hard-skip in autoScout_areaIsCandidate (radiusFactor=0.8).
bool autoScout_anyOracleNear(
   vector areaPos = cInvalidVector, int excludeUnitID = -1, float radiusFactor = 0.8)
{
   autoScout_initOracleQuery();
   kbUnitQueryResetResults(gAutoScout_oracleQuery);
   int n = kbUnitQueryExecute(gAutoScout_oracleQuery);
   if (n <= 0) { return(false); }
   float threshold = radiusFactor * gAutoScout_maxOracleLOS;
   for (int i = 0; i < n; i = i + 1)
   {
      int oracleID = kbUnitQueryGetResult(gAutoScout_oracleQuery, i);
      if (oracleID < 0) { continue; }
      if (oracleID == excludeUnitID) { continue; }
      vector pos = kbUnitGetPosition(oracleID);
      if (xsVectorDistanceXZ(pos, areaPos) < threshold) { return(true); }
   }
   return(false);
}

// Returns a [0, 1] penalty representing summed overlap of areaPos with all
// our oracles' claim circles, excluding excludeUnitID. Penalty per oracle is
// linear in distance: 1.0 at zero distance, 0.0 at >= MaxOracleLOS. Sums then
// clamps to 1.0. Used as a multiplicative discount in autoScout_areaScore for
// non-oracle source units. Moving oracles use their current dynamic LOS as
// the claim radius (small, transient) instead of MaxOracleLOS.
float autoScout_oraclePenalty(vector areaPos = cInvalidVector, int excludeUnitID = -1)
{
   autoScout_initOracleQuery();
   kbUnitQueryResetResults(gAutoScout_oracleQuery);
   int n = kbUnitQueryExecute(gAutoScout_oracleQuery);
   if (n <= 0) { return(0.0); }
   float penalty = 0.0;
   for (int i = 0; i < n; i = i + 1)
   {
      int oracleID = kbUnitQueryGetResult(gAutoScout_oracleQuery, i);
      if (oracleID < 0) { continue; }
      if (oracleID == excludeUnitID) { continue; }
      vector pos = kbUnitGetPosition(oracleID);
      float d = xsVectorDistanceXZ(pos, areaPos);
      float radius = gAutoScout_maxOracleLOS;
      if (kbUnitGetActionType(oracleID) == cActionTypeMove)
      {
         radius = kbUnitGetStatFloat(oracleID, cUnitStatLOS);
      }
      if (radius < 0.001) { continue; }
      if (d < radius)
      {
         penalty = penalty + (radius - d) / radius;
      }
   }
   if (penalty > 1.0) { penalty = 1.0; }
   return(penalty);
}
```

- [ ] **Step 2: Verify by reading the modified region**

Read lines 380-500 to confirm the helpers are placed before the BFS section.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(scout): oracle helpers — query, cache update, overlap predicates"
```

---

### Task 3: Heuristic updates — oracle hard-skip + multiplicative oracle discount

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:autoScout_areaIsCandidate` (around line 380) and `autoScout_areaScore` (around line 412)

- [ ] **Step 1: Add the oracle hard-skip to `autoScout_areaIsCandidate`**

The existing function body ends with `return(true);` after the kbCanPath check. Add the oracle-source hard-skip immediately before that return:

```xs
   // Oracle source hard-skip: refuse candidates within
   // cAutoScout_OracleExclusionFactor * MaxOracleLOS of any other oracle.
   // Prevents oracle-on-oracle LOS overlap (which throttles favor income
   // for the overlapped pair) and redundant coverage.
   if (autoScout_isOracle(scoutUnitID) == true &&
       autoScout_anyOracleNear(areaPos, scoutUnitID, cAutoScout_OracleExclusionFactor) == true)
   {
      return(false);
   }

   return(true);
```

(Replace the existing standalone `return(true);` with the block above. The `areaPos` local was computed earlier in the function at the kbAreaGetCenter call.)

- [ ] **Step 2: Add the multiplicative oracle discount to `autoScout_areaScore`**

The existing function ends with:

```xs
   return(cAutoScout_WeightTC * tcScore
        + cAutoScout_WeightScout * scoutScore
        + cAutoScout_WeightDensity * densityScore);
```

Replace that block with:

```xs
   float baseScore = cAutoScout_WeightTC * tcScore
                   + cAutoScout_WeightScout * scoutScore
                   + cAutoScout_WeightDensity * densityScore;

   // Oracle-overlap discount applies only when the source scout is NOT an
   // oracle. Oracle sources already use the hard-skip in
   // autoScout_areaIsCandidate; double-applying a discount would over-penalise
   // oracle re-positioning. The discount is multiplicative so areas deeply
   // inside an oracle's claim approach zero score, while areas just barely
   // touching the claim circle keep most of their score.
   if (autoScout_isOracle(scoutUnitID) == false)
   {
      float oracleDiscount = 1.0 - autoScout_oraclePenalty(areaPos, scoutUnitID);
      baseScore = baseScore * oracleDiscount;
   }

   return(baseScore);
```

- [ ] **Step 3: Verify by reading both functions end-to-end**

Read lines 380-470.

- [ ] **Step 4: Commit**

```bash
git commit -am "feat(scout): area heuristics aware of oracle claims"
```

---

### Task 4: Extract Diverting handler into a shared helper

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` — extract the existing Diverting branch from `autoScout_tickUnit` into `autoScout_tickDivertingState`, and replace the original block with a call.

- [ ] **Step 1: Add the new helper above `autoScout_tickUnit`**

Insert immediately above `bool autoScout_tickUnit(int slot = -1)` (currently at line 656):

```xs
// Shared Diverting-state handler. Called from both autoScout_tickUnit (regular
// scouts) and autoScout_tickOracleUnit (oracles). Identical semantics: re-target
// the herd every tick (herds wander), complete on herd-invalid OR herd-flipped-
// to-us OR arrived OR stuck-timeout. On completion, releases area claim, sets
// Idle, clears targetHerdID. Returns true if the state transitioned (caller
// may re-tick in same firing).
bool autoScout_tickDivertingState(int slot = -1, int unitID = -1)
{
   if (slot < 0 || unitID < 0) { return(false); }

   int herdID = gAutoScout_targetHerdID[slot];
   bool herdInvalid = (kbUnitGetIsIDValid(herdID) == false);
   bool herdOurs = false;
   if (herdInvalid == false) { herdOurs = (kbUnitGetPlayerID(herdID) == cMyID); }

   bool arrived = false;
   if (herdInvalid == false && herdOurs == false)
   {
      arrived = autoScout_arrived(unitID, gAutoScout_targetWaypoint[slot], gAutoScout_stuckTicks[slot]);
   }

   bool stuck = (gAutoScout_stuckTicks[slot] >= cAutoScout_StuckTickLimit);

   if (herdInvalid == true || herdOurs == true || arrived == true || stuck == true)
   {
      aiEcho("autoScout: DIVERT done unit " + unitID + " herd " + herdID
         + " (invalid=" + herdInvalid + " ours=" + herdOurs
         + " arrived=" + arrived + " stuck=" + stuck + ")");
      autoScout_releaseClaim(slot);
      autoScout_setStateIdle(slot);
      gAutoScout_targetHerdID[slot] = -1;
      return(true);
   }

   gAutoScout_stuckTicks[slot] = gAutoScout_stuckTicks[slot] + 1;
   gAutoScout_targetWaypoint[slot] = kbUnitGetPosition(herdID);
   aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
   return(false);
}
```

- [ ] **Step 2: Replace the inline Diverting block in `autoScout_tickUnit` with a call**

Locate the existing block (around lines 812-842 in the current file — the `if (state == cAutoScoutState_Diverting) { ... }` block). Replace its body with:

```xs
   if (state == cAutoScoutState_Diverting)
   {
      return(autoScout_tickDivertingState(slot, unitID));
   }
```

- [ ] **Step 3: Verify by reading the modified region**

Read lines 656-880.

- [ ] **Step 4: Commit**

```bash
git commit -am "refactor(scout): extract Diverting state handler into shared helper"
```

---

### Task 5: Oracle tick handler

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` — add `autoScout_tickOracleUnit` immediately above `autoScout_tickUnit`

- [ ] **Step 1: Insert `autoScout_tickOracleUnit` above `autoScout_tickUnit`**

```xs
// Oracle-specific tick handler. State machine:
//   Idle -> Walking (issue move to chosen area centroid)
//   Walking -> Stationed (on arrival OR currentLOS/MaxOracleLOS < 0.5)
//   Stationed -> Idle (on saturation: action == cAutoScout_OracleSaturatedActionType)
//   Diverting -> handled by shared autoScout_tickDivertingState
// tryDivert is checked at the top of every non-Idle, non-Diverting state so
// herd claims always preempt, including suspending the 50% LOS floor.
//
// Returns true on a state transition this tick (caller may re-tick).
bool autoScout_tickOracleUnit(int slot = -1)
{
   int unitID = gAutoScout_unitID[slot];
   int planID = gAutoScout_planID[slot];

   if (kbUnitGetIsIDValid(unitID) == false ||
       kbUnitGetPlayerID(unitID) != cMyID ||
       aiPlanGetIsIDValid(planID) == false)
   {
      autoScout_dropFromPool(slot);
      return(true);
   }

   // Opportunistic cache update — harmless if not saturated (the helper is
   // gated). Done every tick so we capture the saturation moment regardless
   // of which state branch we hit.
   autoScout_updateMaxOracleLOS(unitID);

   float los = kbUnitGetStatFloat(unitID, cUnitStatLOS);
   int   state = gAutoScout_state[slot];

   // Diverting: shared logic. No tryDivert call here since we are already
   // committed to a herd target.
   if (state == cAutoScoutState_Diverting)
   {
      return(autoScout_tickDivertingState(slot, unitID));
   }

   // tryDivert outranks all other state transitions (incl. the 50% LOS floor).
   if (autoScout_tryDivert(slot, unitID, los) == true) { return(true); }

   if (state == cAutoScoutState_Idle)
   {
      int nextArea = autoScout_findNextArea(unitID);
      if (nextArea < 0)
      {
         // No candidate area available -- drop oracle from pool and let the
         // engine plan housekeeping revert the UI button (same as regular
         // scouts when BFS is exhausted).
         aiTaskStopUnit(unitID);
         aiPlanDestroy(planID);
         autoScout_dropFromPool(slot);
         return(true);
      }
      gAutoScout_areaClaim[nextArea] = unitID;
      gAutoScout_targetAreaID[slot] = nextArea;
      gAutoScout_targetWaypoint[slot] = kbAreaGetCenter(nextArea);
      gAutoScout_state[slot] = cAutoScoutState_Walking;
      gAutoScout_stuckTicks[slot] = 0;
      aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
      return(true);
   }

   if (state == cAutoScoutState_Walking)
   {
      vector waypoint = gAutoScout_targetWaypoint[slot];
      int areaID = gAutoScout_targetAreaID[slot];

      // 50% LOS floor: if our currentLOS bled below half MaxOracleLOS while
      // walking, commit to the current position. The vanilla failure mode is
      // letting LOS drain to base, where favor income is minimal.
      bool floorTrigger = false;
      if (gAutoScout_maxOracleLOS > 0.0001)
      {
         float pct = los / gAutoScout_maxOracleLOS;
         if (pct < cAutoScout_OracleMovementLOSFloor) { floorTrigger = true; }
      }
      if (floorTrigger == true)
      {
         aiTaskStopUnit(unitID);
         if (areaID >= 0 && areaID < gAutoScout_areaSelfScouted.size())
         {
            gAutoScout_areaSelfScouted[areaID] = 1;
         }
         aiEcho("autoScout: oracle " + unitID + " LOS-floor stop (los=" + los
            + " maxLOS=" + gAutoScout_maxOracleLOS + ")");
         gAutoScout_state[slot] = cAutoScoutState_Stationed;
         gAutoScout_stuckTicks[slot] = 0;
         return(true);
      }

      if (autoScout_arrived(unitID, waypoint, gAutoScout_stuckTicks[slot]) == true)
      {
         if (areaID >= 0 && areaID < gAutoScout_areaSelfScouted.size())
         {
            gAutoScout_areaSelfScouted[areaID] = 1;
         }
         aiEcho("autoScout: oracle " + unitID + " arrived; parking (los=" + los + ")");
         gAutoScout_state[slot] = cAutoScoutState_Stationed;
         gAutoScout_stuckTicks[slot] = 0;
         return(true);
      }

      gAutoScout_stuckTicks[slot] = gAutoScout_stuckTicks[slot] + 1;
      if (gAutoScout_stuckTicks[slot] >= cAutoScout_StuckTickLimit)
      {
         autoScout_releaseClaim(slot);
         autoScout_setStateIdle(slot);
         return(true);
      }
      aiTaskMoveUnit(unitID, waypoint, false, false);
      return(false);
   }

   if (state == cAutoScoutState_Stationed)
   {
      // Saturation == LOS hit cap. Action 37 is the engine-level signal
      // (the meditation animation plays at this point). Re-pick a new area.
      if (kbUnitGetActionType(unitID) == cAutoScout_OracleSaturatedActionType)
      {
         aiEcho("autoScout: oracle " + unitID + " saturated at los=" + los
            + " (maxLOS cache=" + gAutoScout_maxOracleLOS + "), repicking");
         autoScout_releaseClaim(slot);
         autoScout_setStateIdle(slot);
         return(true);
      }
      // Still growing -- stay parked, no action.
      return(false);
   }

   // Working state should not occur for oracles (they don't enter frontier-walk).
   // Defensive: promote to Stationed.
   if (state == cAutoScoutState_Working)
   {
      gAutoScout_state[slot] = cAutoScoutState_Stationed;
      return(true);
   }

   return(false);
}
```

- [ ] **Step 2: Verify by reading the inserted block**

Read the relevant lines.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(scout): oracle tick handler with parking state machine"
```

---

### Task 6: Wire oracle dispatch into the rule loop

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:autoScout_tickUnit`

- [ ] **Step 1: Add an oracle-route branch at the top of `autoScout_tickUnit`**

Right after the validation block (the existing `if (kbUnitGetIsIDValid(unitID) == false || ...) { autoScout_dropFromPool(slot); return(true); }` near the start of `autoScout_tickUnit`), insert:

```xs
   // Route oracles to their dedicated state machine. They share the pool and
   // the Diverting handler with regular scouts but have their own Idle/
   // Walking/Stationed flow (no Working / frontier-walk).
   if (autoScout_isOracle(unitID) == true)
   {
      return(autoScout_tickOracleUnit(slot));
   }
```

- [ ] **Step 2: Verify by reading the modified function start**

Read lines 656-690.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(scout): dispatch oracles to their own tick handler"
```

---

### Task 7: Allow oracles into the scout pool

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:autoScout_register` line 884

- [ ] **Step 1: Remove the oracle early-return in `autoScout_register`**

The current line 884 is:

```xs
   if (kbUnitIsType(unitID, cUnitTypeAbstractOracle) == true) { return; }
```

Delete that line entirely. Oracles will now be admitted to the pool just like regular scouts. The dispatch in `autoScout_tickUnit` (Task 6) routes them to the oracle handler.

- [ ] **Step 2: Verify by reading `autoScout_register` end-to-end**

Read lines 881-915.

- [ ] **Step 3: Commit**

```bash
git commit -am "feat(scout): admit oracles to the scout pool (registration)"
```

---

### Task 8: Self-review pass

**Files:**
- Read: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` end-to-end

- [ ] **Step 1: Re-read constants section**

Confirm `cAutoScoutState_Stationed = 4`, `cAutoScout_OracleColdCacheMaxLOS`, `cAutoScout_OracleMovementLOSFloor`, `cAutoScout_OracleExclusionFactor` are all present and reasonably commented.

- [ ] **Step 2: Re-read the helpers**

Confirm `autoScout_isOracle`, `autoScout_initOracleQuery`, `autoScout_updateMaxOracleLOS`, `autoScout_anyOracleNear`, `autoScout_oraclePenalty` are placed above the BFS / heuristic section.

- [ ] **Step 3: Re-read `autoScout_areaIsCandidate` and `autoScout_areaScore`**

Confirm hard-skip and multiplicative discount are correctly placed.

- [ ] **Step 4: Re-read `autoScout_tickDivertingState`, `autoScout_tickOracleUnit`, `autoScout_tickUnit`**

Confirm:
- The Diverting helper is defined before both callers.
- `autoScout_tickOracleUnit` is defined before `autoScout_tickUnit`.
- The oracle dispatch branch in `autoScout_tickUnit` is at the top.

- [ ] **Step 5: Re-read `autoScout_register`**

Confirm the oracle early-return is gone.

- [ ] **Step 6: Search for any leftover references to TODO/TBD/placeholder**

```bash
grep -nE "TODO|TBD|FIXME|XXX|placeholder" mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
```

Expected: no hits beyond pre-existing comments.

- [ ] **Step 7: Commit (only if any cleanup edits were needed; otherwise skip)**

---

### Task 9: Deploy (DEFERRED until user confirms game closed)

**Files:**
- Copy: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` → `mods/local/Intelligent Auto-Scout/game/ai/human_assist/auto_scout.xs`
- Copy: same source → `mods/local/Intelligent Auto-Repair and Scout/game/ai/human_assist/auto_scout.xs`

**This task is paused.** The user is currently playing. Resume only after the user confirms the game is closed (or after `pgrep -x AoMRT_s.exe` returns empty).

- [ ] **Step 1: Verify game closed**

```bash
pgrep -x AoMRT_s.exe; echo "exit=$?"
```

Expected: exit=1 (no output).

- [ ] **Step 2: Deploy to both mods**

```bash
SRC="/home/houtamelo/Documents/projects/aom_retold_mod/mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs"
DEST_BASE="/home/houtamelo/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local"
cp "$SRC" "$DEST_BASE/Intelligent Auto-Scout/game/ai/human_assist/auto_scout.xs"
cp "$SRC" "$DEST_BASE/Intelligent Auto-Repair and Scout/game/ai/human_assist/auto_scout.xs"
md5sum "$SRC" "$DEST_BASE/Intelligent Auto-Scout/game/ai/human_assist/auto_scout.xs" "$DEST_BASE/Intelligent Auto-Repair and Scout/game/ai/human_assist/auto_scout.xs"
```

Expected: all three MD5s match.

---

## Self-review of the plan

**Spec coverage:**
- Differentiate oracles ✓ Task 6 dispatch + Task 5 handler
- All scouts check other scouts' types ✓ existing pool already handles this; Task 3 adds oracle-aware sub-rules
- Oracles also account for non-toggled oracles ✓ Task 2 query + Task 3 hard-skip
- MaxOracleLOS cache ✓ Task 1 global + Task 2 update helper
- Heuristic 5.1.R/5.1.O (regular density) ✓ existing
- Heuristic 5.2.R (oracle penalty for regular scouts) ✓ Task 3 multiplicative discount
- Heuristic 5.2.O (oracle hard-skip) ✓ Task 3 candidate filter
- Oracle state machine Idle/Walking/Stationed ✓ Task 5
- Saturation = action 37 ✓ existing constant + Task 5 trigger
- 50% LOS floor while moving ✓ Task 5 Walking branch
- Herd divert outranks LOS floor ✓ Task 5 tryDivert called before floor check
- Cold cache 20.0 ✓ Task 1
- Centroid as target waypoint ✓ Task 5 Idle branch
- No Working state for oracles ✓ Task 5 defensive promotion to Stationed
- Non-toggled oracle moving check ✓ Task 2 oraclePenalty branches on cActionTypeMove

**Placeholder scan:** none.

**Type consistency:** All function signatures and global names match across tasks. State constant `cAutoScoutState_Stationed = 4` used consistently.

## Execution Handoff

User has explicitly said: "plan the implementation, then execute it. Don't deploy because I'm playing the game right now."

Will execute inline, skipping Task 9 (deploy) until the game is confirmed closed.
