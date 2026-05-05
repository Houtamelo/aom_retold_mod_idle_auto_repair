# Herd Diversion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add in-flight herd diversion to the intelligent_auto_scout mod. Eligible scouts briefly detour to claim Gaia/enemy herdables they pass near; any herd flipped to our ownership (intentional divert OR organic walk-past) is auto-routed to the nearest TC, exactly once.

**Architecture:** Extend the existing per-scout state machine (Idle/Walking/Working) with a fourth `Diverting` state. Detection runs at the top of WALKING/WORKING handlers via an LOS-bounded unit query. A separate per-tick scan over our owned herdables issues a single `aiTaskMoveUnit` to the nearest TC per herd. Two append-only ID-list arrays (`attemptedHerdIDs`, `redirectedHerdIDs`) provide all coordination — no claim-per-herd bookkeeping.

**Tech Stack:** XS (Age of Mythology Retold AI scripting). Single-file change: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`. No tests (XS has no harness); verification is parse-by-deploy + in-game `aiEcho` log inspection.

**Spec:** `docs/superpowers/specs/2026-05-04-herd-diversion-design.md`.

**Important context for implementers:**
- **Hard rule:** never write to the AoM:R game folder while the game is running. Deploy step assumes the user has confirmed the game is closed.
- **XS quirks** (project memory):
  - Locals use `new <type>(size, value)`, NOT `= default`. `default` is for `extern` globals only.
  - Don't combine `&&` + integer math + `>=` in one `if` — split into intermediate variables. (Pure boolean `&&` chains without int-math are fine; vanilla uses them constantly.)
  - Within a single file, forward function references work. Across `include` boundaries they don't — irrelevant here since the entire feature lives in one file.

---

## File Structure

Single file modified: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (~680 lines today, +~150 after this plan).

Section additions in this file (in source order):
1. New constants (with the existing constants block at the top).
2. New globals (with the existing globals block).
3. Pool-array bookkeeping update (existing `autoScout_dropFromPool` + `autoScout_register`).
4. Helper section: `isHerdAttempted`, `isHerdRedirected`, `initHerdQuery`, `initOwnedHerdQuery`, `findNearestTC`.
5. Detection: `findVisibleHerd`.
6. Divert entry: `tryDivert`.
7. State-machine wiring: insertions in WALKING + WORKING handlers; new DIVERTING handler block.
8. Home-move scan: `homeMoveScan` + call from `autoScout_tick` rule body.

---

## Task 1: Verify required XS constants exist in AoMR exports

**Files:**
- Read-only: `extracted/doxygen/*.html`, `extracted/gameplay/proto.xml`

Some constants are referenced in the spec but not yet confirmed by name in AoMR's XS export. We confirm or pick a fallback BEFORE writing any code that depends on them.

- [ ] **Step 1: Grep doxygen + proto for each constant**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
grep -rohE 'cPlayerRelation[A-Za-z]+' extracted/doxygen/ | sort -u
grep -rohE 'cUnitType[A-Za-z]+' extracted/doxygen/ | sort -u
grep -rohE 'LogicalTypeConvertsHerds' extracted/gameplay/proto.xml | head -1
grep -rohE 'TownCenter' extracted/gameplay/proto.xml | sort -u | head -5
```

- [ ] **Step 2: Cross-check against the existing mod code**

```bash
grep -rohE 'cPlayerRelation[A-Za-z]+|cUnitType[A-Za-z]+' mod/ | sort -u
```

Existing usages give ground-truth names that compile in current AoMR.

- [ ] **Step 3: Decide naming**

Expected names (will be used by later tasks):
- `cPlayerRelationEnemyOrNeutral` — if absent, fall back to `cPlayerRelationAny` and filter `playerID == cMyID || kbGetPlayerRelation(cMyID, playerID) == cPlayerRelationAlly` in the result loop.
- `cUnitTypeLogicalTypeConvertsHerds` — convention is `cUnitType` + the proto unittype string verbatim. Doxygen shows `cUnitTypeAbstractVillager` and `cUnitTypeHouse` follow this rule. If `kbProtoUnitIsType` rejects the string-form constant, fall back to a name lookup via `kbGetProtoUnitID` against the string `"LogicalTypeConvertsHerds"` is NOT applicable here (kbProtoUnitIsType expects a unit-type ID). The constant should exist; if it doesn't, we'd fall back to `cUnitTypeAbstractFlyingUnit == false` as a less precise gate.
- `cUnitTypeHerdable` — already used in the existing spec context; expect to exist.
- `cUnitTypeTownCenter` — for the owned-TC enumeration.

- [ ] **Step 4: Note any required fallbacks for later tasks**

Update Task 4 (`findVisibleHerd`) and Task 5 (`tryDivert`) inline if any constant must be replaced.

- [ ] **Step 5: Commit (only if Step 4 produced changes — usually nothing to commit here)**

This task is research-only; if no plan edits are made, skip the commit.

---

## Task 2: Add constants, globals, and pool-array bookkeeping

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`

- [ ] **Step 1: Add new state constant + tuning constant**

In the constants block near the top of the file, after `const int cAutoScoutState_Working = 2;` (currently around line 17):

```xs
const int cAutoScoutState_Diverting = 3;
```

No additional tuning constants are needed — we reuse `cAutoScout_StuckTickLimit` for the divert timeout.

- [ ] **Step 2: Add new globals**

In the globals block (after the existing `gAutoScout_areaSelfScouted` declaration, around line 89):

```xs
// Per-scout: herd currently being diverted to in DIVERTING state. -1 when idle/not diverting.
extern int[] gAutoScout_targetHerdID = default;

// Per-herd, append-only, never unmarked. Any herd ever selected by any scout for divert.
// Filters subsequent divert candidates so each herd is attempted at most once globally.
extern int[] gAutoScout_attemptedHerdIDs = default;

// Per-herd, append-only, never unmarked. Any herd we've already issued a home-move for.
// Single-issue guarantee — prevents fighting with subsequent player command overrides.
extern int[] gAutoScout_redirectedHerdIDs = default;

// Cached unit-query handles, lazily initialized.
extern int gAutoScout_herdQuery = -1;
extern int gAutoScout_ownedHerdQuery = -1;
```

- [ ] **Step 3: Update `autoScout_dropFromPool` to remove the new per-scout array element**

Locate `autoScout_dropFromPool` (currently line 120). After `gAutoScout_stuckTicks.removeIndex(slot);` add:

```xs
   gAutoScout_targetHerdID.removeIndex(slot);
```

- [ ] **Step 4: Update `autoScout_register` to append to the new per-scout array**

Locate `autoScout_register` (currently around line 629). After `gAutoScout_stuckTicks.add(0);` add:

```xs
   gAutoScout_targetHerdID.add(-1);
```

- [ ] **Step 5: Verify the file still parses (read-back sanity check)**

```bash
grep -n 'gAutoScout_targetHerdID' mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
```

Expected: 4 hits (extern declaration, removeIndex, add, plus the eventual usage we'll add in later tasks — for now, 3 hits).

- [ ] **Step 6: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit -m "feat(scout): scaffold globals for herd diversion state"
```

---

## Task 3: Add helper functions (membership + query init + nearest-TC)

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`

Add a new section. Insert after the "Position helpers" section (after `autoScout_clampToMap`, currently around line 158), before the "Candidate area criteria + BFS" section:

- [ ] **Step 1: Add the membership helpers**

```xs
//------------------------------------------------------------------------------
// Herd diversion helpers
//------------------------------------------------------------------------------

bool autoScout_isHerdAttempted(int herdID = -1)
{
   if (herdID < 0) { return(false); }
   int n = gAutoScout_attemptedHerdIDs.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_attemptedHerdIDs[i] == herdID) { return(true); }
   }
   return(false);
}

bool autoScout_isHerdRedirected(int herdID = -1)
{
   if (herdID < 0) { return(false); }
   int n = gAutoScout_redirectedHerdIDs.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_redirectedHerdIDs[i] == herdID) { return(true); }
   }
   return(false);
}
```

- [ ] **Step 2: Add the lazy query initializers**

```xs
void autoScout_initHerdQuery()
{
   if (gAutoScout_herdQuery >= 0) { return; }
   gAutoScout_herdQuery = kbUnitQueryCreate("autoScout_herds");
   kbUnitQuerySetPlayerRelation(gAutoScout_herdQuery, cPlayerRelationEnemyOrNeutral);
   kbUnitQuerySetUnitType(gAutoScout_herdQuery, cUnitTypeHerdable);
   kbUnitQuerySetState(gAutoScout_herdQuery, cUnitStateAlive);
   kbUnitQuerySetAscendingSort(gAutoScout_herdQuery, true);
}

void autoScout_initOwnedHerdQuery()
{
   if (gAutoScout_ownedHerdQuery >= 0) { return; }
   gAutoScout_ownedHerdQuery = kbUnitQueryCreate("autoScout_ownedHerds");
   kbUnitQuerySetPlayerRelation(gAutoScout_ownedHerdQuery, cPlayerRelationSelf);
   kbUnitQuerySetUnitType(gAutoScout_ownedHerdQuery, cUnitTypeHerdable);
   kbUnitQuerySetState(gAutoScout_ownedHerdQuery, cUnitStateAlive);
}
```

- [ ] **Step 3: Add `findNearestTC`**

```xs
// Returns the position of our nearest alive TC to refPos, or cInvalidVector if we own no TC.
// Uses a fresh per-call query (cheap; called at most once per redirect).
vector autoScout_findNearestTC(vector refPos = cInvalidVector)
{
   int q = kbUnitQueryCreate("autoScout_nearestTC");
   kbUnitQuerySetPlayerRelation(q, cPlayerRelationSelf);
   kbUnitQuerySetUnitType(q, cUnitTypeTownCenter);
   kbUnitQuerySetState(q, cUnitStateAlive);
   kbUnitQueryResetResults(q);
   int n = kbUnitQueryExecute(q);
   if (n <= 0) { return(cInvalidVector); }

   vector best = cInvalidVector;
   float bestDist = 1.0e18;
   for (int i = 0; i < n; i = i + 1)
   {
      int tcID = kbUnitQueryGetResult(q, i);
      if (tcID < 0) { continue; }
      vector tcPos = kbUnitGetPosition(tcID);
      float d = xsVectorDistanceXZ(tcPos, refPos);
      if (d < bestDist) { bestDist = d; best = tcPos; }
   }
   return(best);
}
```

Note: we don't cache this query handle because it's executed at most once per converted herd, and caching would require holding a result-slot across calls (the cached query's results would be reset by the next redirect call anyway). Cheap enough to recreate.

- [ ] **Step 4: Read-back sanity**

```bash
grep -nE 'autoScout_isHerdAttempted|autoScout_isHerdRedirected|autoScout_initHerdQuery|autoScout_initOwnedHerdQuery|autoScout_findNearestTC' mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
```

Expected: each function name appears at least once (its definition).

- [ ] **Step 5: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit -m "feat(scout): herd-diversion helpers (membership, query init, nearest-TC)"
```

---

## Task 4: Implement findVisibleHerd

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`

Detection: takes a scout, returns the closest reachable not-yet-attempted herd within LOS, or `-1`.

- [ ] **Step 1: Add `findVisibleHerd` immediately after `autoScout_findNearestTC`**

```xs
// Returns ID of the closest reachable, not-yet-attempted, eligible herd within `los` of
// the scout, or -1 if none. Caller is responsible for checking the scout's
// LogicalTypeConvertsHerds eligibility before calling.
int autoScout_findVisibleHerd(int scoutUnitID = -1, float los = 18.0)
{
   if (scoutUnitID < 0 || los < 1.0) { return(-1); }
   autoScout_initHerdQuery();
   vector pos = kbUnitGetPosition(scoutUnitID);
   kbUnitQuerySetPosition(gAutoScout_herdQuery, pos);
   kbUnitQuerySetMaximumDistance(gAutoScout_herdQuery, los);
   kbUnitQueryResetResults(gAutoScout_herdQuery);
   int n = kbUnitQueryExecute(gAutoScout_herdQuery);
   int unitProto = kbUnitGetProtoUnitID(scoutUnitID);
   for (int i = 0; i < n; i = i + 1)
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

- [ ] **Step 2: Read-back sanity**

```bash
grep -n 'autoScout_findVisibleHerd' mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
```

Expected: 1 hit (the definition).

- [ ] **Step 3: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit -m "feat(scout): herd-diversion detection (findVisibleHerd)"
```

---

## Task 5: Implement tryDivert

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`

`tryDivert` is the entry-point used by both WALKING and WORKING handlers. It checks scout eligibility, queries for a herd, and (if found) transitions the slot to `Diverting`.

- [ ] **Step 1: Add `tryDivert` after `findVisibleHerd`**

```xs
// Tries to enter Diverting state for slot. Returns true if the scout transitioned to
// Diverting (caller should `return(true)` to short-circuit normal handler logic for this tick).
bool autoScout_tryDivert(int slot = -1, int unitID = -1, float los = 18.0)
{
   int unitProto = kbUnitGetProtoUnitID(unitID);
   if (kbProtoUnitIsType(cMyID, unitProto, cUnitTypeLogicalTypeConvertsHerds) == false) { return(false); }

   int herdID = autoScout_findVisibleHerd(unitID, los);
   if (herdID < 0) { return(false); }
   if (kbUnitGetIsIDValid(herdID) == false) { return(false); }

   gAutoScout_attemptedHerdIDs.add(herdID);
   gAutoScout_targetHerdID[slot]   = herdID;
   gAutoScout_state[slot]          = cAutoScoutState_Diverting;
   gAutoScout_targetWaypoint[slot] = kbUnitGetPosition(herdID);
   gAutoScout_stuckTicks[slot]     = 0;

   aiEcho("autoScout: DIVERTING unit " + unitID + " to herd " + herdID);
   aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
   return(true);
}
```

- [ ] **Step 2: Read-back sanity**

```bash
grep -n 'autoScout_tryDivert' mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
```

Expected: 1 hit (definition).

- [ ] **Step 3: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit -m "feat(scout): herd-diversion entry (tryDivert)"
```

---

## Task 6: Wire divert into WALKING + WORKING handlers and add DIVERTING handler

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`

This task contains the most-invasive edits. Three changes to `autoScout_tickUnit`.

- [ ] **Step 1: Insert divert check at the top of WALKING handler**

Locate `if (state == cAutoScoutState_Walking)` (currently line 480). Immediately after the opening `{`, before the existing `vector waypoint = ...` line, insert:

```xs
      if (autoScout_tryDivert(slot, unitID, los) == true) { return(true); }
```

The handler body is inside a 3-space indent following the existing style; match that indentation exactly.

- [ ] **Step 2: Insert divert check at the top of WORKING handler**

Locate `if (state == cAutoScoutState_Working)` (currently line 562). Immediately after the opening `{`, before the existing `vector waypoint = ...` line, insert the same line:

```xs
      if (autoScout_tryDivert(slot, unitID, los) == true) { return(true); }
```

- [ ] **Step 3: Add the DIVERTING state handler**

Locate the existing WORKING handler block. After its closing `}` and before the function's closing `return(false);` (currently line 622), insert the DIVERTING handler:

```xs
   if (state == cAutoScoutState_Diverting)
   {
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
            + " (invalid=" + herdInvalid + " ours=" + herdOurs + " arrived=" + arrived + " stuck=" + stuck + ")");
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

The booleans are split into intermediates instead of a compound `if (a || b || c || d)` because the XS parser has been observed to flake on certain compound boolean expressions (project memory: `reference_xs_compound_expr_quirk.md`). The split is also defensive against `kbUnitGetPlayerID(invalid)` being called.

- [ ] **Step 4: Read-back sanity**

```bash
grep -nE 'autoScout_tryDivert\(slot|cAutoScoutState_Diverting' mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
```

Expected: 4 hits — 2 wiring sites in WALKING/WORKING, the DIVERTING handler match, and the constant declaration.

- [ ] **Step 5: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit -m "feat(scout): wire divert detection + DIVERTING state handler"
```

---

## Task 7: Implement homeMoveScan and wire into the tick rule

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`

The home-move scan runs once per `autoScout_tick` firing, after the per-scout loop. It queries our owned herdables, and for each not yet in `redirectedHerdIDs` issues a single move-to-nearest-TC.

- [ ] **Step 1: Add `homeMoveScan` between the existing `autoScout_register` (line ~629) and the rule definition (line ~659)**

```xs
//------------------------------------------------------------------------------
// Home-move scan: route newly-converted herdables to the nearest TC, once.
//------------------------------------------------------------------------------

void autoScout_homeMoveScan()
{
   autoScout_initOwnedHerdQuery();
   kbUnitQueryResetResults(gAutoScout_ownedHerdQuery);
   int n = kbUnitQueryExecute(gAutoScout_ownedHerdQuery);
   for (int i = 0; i < n; i = i + 1)
   {
      int herdID = kbUnitQueryGetResult(gAutoScout_ownedHerdQuery, i);
      if (herdID < 0) { continue; }
      if (autoScout_isHerdRedirected(herdID) == true) { continue; }

      vector herdPos = kbUnitGetPosition(herdID);
      vector tcPos = autoScout_findNearestTC(herdPos);
      if (tcPos == cInvalidVector) { continue; }

      aiEcho("autoScout: home-move herd " + herdID + " -> TC at " + tcPos);
      aiTaskMoveUnit(herdID, tcPos, false, false);
      gAutoScout_redirectedHerdIDs.add(herdID);
   }
}
```

- [ ] **Step 2: Call `homeMoveScan` from the `autoScout_tick` rule body**

Locate the rule body (currently line 662). After the existing `for (int slot = ...)` loop closes (currently line 678) and before `xsSetContextPlayer(-1);`, insert:

```xs
   autoScout_homeMoveScan();
```

The full rule body should now look like:

```xs
rule autoScout_tick
minInterval 1
active
{
   xsSetContextPlayer(cMyID);
   autoScout_initAreaArrays();
   for (int slot = gAutoScout_unitID.size() - 1; slot >= 0; slot = slot - 1)
   {
      bool transitioned = true;
      int iter = 0;
      while (transitioned == true && iter < cAutoScout_MaxChainPerTick)
      {
         if (slot >= gAutoScout_unitID.size()) { break; }
         transitioned = autoScout_tickUnit(slot);
         iter = iter + 1;
      }
   }
   autoScout_homeMoveScan();
   xsSetContextPlayer(-1);
}
```

- [ ] **Step 3: Read-back sanity**

```bash
grep -n 'autoScout_homeMoveScan' mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
```

Expected: 2 hits (definition + call from rule).

- [ ] **Step 4: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit -m "feat(scout): home-move scan for converted herdables"
```

---

## Task 8: Deploy and verify in-game

**Files:**
- No source changes. Deploys the file under work to the AoM:R local-mod folder.

- [ ] **Step 1: Confirm the game is closed**

The hard rule: no writes to the AoM:R install or Proton user folder while the game is running. Ask the user to confirm before proceeding.

- [ ] **Step 2: Deploy auto_scout.xs**

```bash
cp /home/houtamelo/Documents/projects/aom_retold_mod/mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs \
   "/home/houtamelo/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs"
```

- [ ] **Step 3: User starts a game with the AI-assist enabled and at least one eligible auto-scout**

A Greek civ with Kataskopos is the easiest test case. Start a skirmish, queue a Kataskopos, and toggle auto-scout on it. Spawn a Cow nearby (cheat: `goatunheim` or via editor) for divert verification, OR rely on natural map herds.

Successful behavior to observe:
- `aiEcho` line `autoScout: DIVERTING unit <id> to herd <id>` when scout passes near a Gaia/enemy herd.
- `aiEcho` line `autoScout: DIVERT done unit <id> herd <id> (...)` on completion, with `ours=true` for the conversion case.
- `aiEcho` line `autoScout: home-move herd <id> -> TC at <pos>` exactly once per converted herd.
- After conversion, the herd walks toward the TC.

Failure modes to watch for:
- Parse errors in `aoegame.log` after game start — most likely cause is a constant name not exposed by AoMR's XS (Task 1 verification missed something).
- Scout diverts but never completes — likely missing eligibility check or `kbUnitGetPlayerID` not flipping. Inspect the scout's distance to the herd at "stuck" timeout.
- Home-move issued multiple times — `redirectedHerdIDs` not being checked or appended; cross-reference Task 7.

- [ ] **Step 4: Capture logs and report**

The user will share `aiEcho` log lines or screenshots. If any failure mode triggers, plan a fix and re-iterate.

- [ ] **Step 5: No commit (deploy + observation only)**

---

## Self-Review

**Spec coverage check** (each spec section → task):
- "Goal" — covered by overall Task 1–8 sequence.
- "State machine extension" — Task 2 (state constant + targetHerdID), Task 6 (handler insertion + DIVERTING state).
- "Per-herd globals" — Task 2 (declarations), Task 3 (membership helpers).
- "Eligibility gate" — Task 5 (inside `tryDivert`).
- "Detection" — Task 3 (initHerdQuery), Task 4 (findVisibleHerd).
- "Tick logic" — Task 6 (WALKING/WORKING wiring + DIVERTING handler).
- "Home-move scan" — Task 7.
- "Filtering decisions" — implicit across Tasks 4 and 5 (LogicalTypeConvertsHerds, EnemyOrNeutral, kbCanPath, attemptedHerdIDs).
- "Coordination tradeoffs" — no implementation needed; emerges from the design.
- "Performance" — no implementation needed.
- "Verify at impl time" — Task 1.
- "Out of scope" — explicitly deferred; no tasks.

**Placeholder scan:** no TBD/TODO; all code blocks contain complete XS.

**Type consistency:** function names match across tasks (`autoScout_tryDivert`, `autoScout_findVisibleHerd`, `autoScout_homeMoveScan`, `autoScout_isHerdAttempted`, `autoScout_isHerdRedirected`, `autoScout_initHerdQuery`, `autoScout_initOwnedHerdQuery`, `autoScout_findNearestTC`); state constant `cAutoScoutState_Diverting` consistent.
