# Scout Danger Avoidance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add pre-emptive danger avoidance to the intelligent_auto_scout mod. Hard-skip and discount dangerous areas at BFS pick-time, abort and flee opposite-direction-then-Idle when a scout's current or target area becomes dangerous in flight. Blacklist aborted areas for 90s.

**Architecture:** One danger source (`kbAreaGetDangerLevel(areaID, true)`); one mutable blacklist of `(areaID, expiryMs)` pairs; one new state `cAutoScoutState_Fleeing`. Three integration points: BFS pick (`autoScout_scoreArea` + `autoScout_areaIsCandidate`), per-tick check at top of non-Idle state handlers, and a new FLEEING handler that issues an opposite-direction `aiTaskMoveUnit` and holds the scout for 5 seconds.

**Tech Stack:** XS (Age of Mythology Retold AI scripting). Single source file modified: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`. The combined mod (`intelligent_auto_repair_and_scout`) inherits this file at deploy time via `scripts/deploy-mods.sh` — no second source copy exists in `mod/intelligent_auto_repair_and_scout/` (only the unified `human_assist.xs` lives there). No tests (XS has no harness); verification is parse-by-deploy + in-game `aiEcho` log inspection.

**Spec:** `docs/superpowers/specs/2026-05-13-scout-danger-avoidance-design.md`.

**Important context for implementers:**
- **Hard rule:** never write to the AoM:R game folder while the game is running. The deploy script (`scripts/deploy-mods.sh`) is invoked manually by the user after each task batch when they're not playing.
- **Only one source copy of `auto_scout.xs` exists.** `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/` only contains `human_assist.xs`; `auto_scout.xs` is pulled from `mod/intelligent_auto_scout/` at deploy time by `scripts/deploy-mods.sh`.
- **XS quirks** (project memory):
  - Locals use `new <type>(size, value)`, NOT `= default`. `default` is for `extern` globals only.
  - Don't combine `&&` + integer math + `>=` in one `if` — split into intermediate variables. (Pure boolean `&&` chains without int-math are fine.)
  - Within a single file, forward function references work. The danger-avoidance code lives entirely in `auto_scout.xs`, so forward refs are fine.
- **Existing helpers to reuse:** `autoScout_releaseClaim(slot)`, `autoScout_setStateIdle(slot)`, `autoScout_isOracle(unitID)`. Don't re-implement.
- **Existing handler is named `autoScout_tickDivertingState`** (not `autoScout_handleDiverting` as Section 4 of the spec called it).

---

## File Structure

Single file modified:
1. `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (~1594 lines today, +~120 after this plan).

Edits land in five existing sections + one new function block, in source order:
1. Constants block (top of file, near line 15) — add `cAutoScoutState_Fleeing` + the 5 tunable constants.
2. Globals block (near line 120) — add blacklist arrays + per-scout flee fields.
3. Pool bookkeeping (`autoScout_register` and `autoScout_dropFromPool`) — add/remove new per-scout arrays.
4. Helper functions (new block after pool management) — `autoScout_areaIsDangerous`, `autoScout_blacklistArea`, `autoScout_isAreaBlacklisted`, `autoScout_enterFleeing`.
5. BFS scoring (`autoScout_areaIsCandidate` + `autoScout_scoreArea`) — hard-skip blacklisted, rebalance weights, add danger subscore.
6. State handlers (`autoScout_tickOracleUnit`, `autoScout_tickUnit`, `autoScout_tickDivertingState`) — top-of-handler danger check + new FLEEING handler block.

---

## Task 1: Add constants and the FLEEING state value

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (constants block near line 15)

- [ ] **Step 1: Add new state value after `cAutoScoutState_Stationed = 4`**

Find this block:

```xs
const int cAutoScoutState_Idle      = 0;
const int cAutoScoutState_Walking   = 1;
const int cAutoScoutState_Working   = 2;
const int cAutoScoutState_Diverting = 3;
const int cAutoScoutState_Stationed = 4;
```

Append:

```xs
const int cAutoScoutState_Fleeing   = 5;
```

- [ ] **Step 2: Add tunables in the existing tunables block (near line 86)**

After the existing `cAutoScout_Weight*` constants:

```xs
// Danger avoidance (2026-05-13). Hard-skip threshold is a playtest-tune
// placeholder; the engine's kbAreaGetDangerLevel range is undocumented and
// the first playtest's aiEcho traces will surface the actual values seen.
const float cAutoScout_DangerHardSkip      = 5.0;
const float cAutoScout_DangerWeight        = 0.15;
const float cAutoScout_FleeDistance        = 25.0;
const int   cAutoScout_FleeMinDurationMs   = 5000;
const int   cAutoScout_BlacklistDurationMs = 90000;
```

- [ ] **Step 3: Rebalance the three existing weights to make room for the danger weight**

Replace:

```xs
const float cAutoScout_WeightTC      = 0.4;
const float cAutoScout_WeightScout   = 0.4;
const float cAutoScout_WeightDensity = 0.2;
```

With:

```xs
const float cAutoScout_WeightTC      = 0.35;
const float cAutoScout_WeightScout   = 0.35;
const float cAutoScout_WeightDensity = 0.15;
```

(0.35 + 0.35 + 0.15 + 0.15 = 1.0)

- [ ] **Step 5: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): add danger-avoidance constants

cAutoScoutState_Fleeing=5, cAutoScout_DangerHardSkip=5.0,
cAutoScout_DangerWeight=0.15, cAutoScout_FleeDistance=25.0,
cAutoScout_FleeMinDurationMs=5000, cAutoScout_BlacklistDurationMs=90000.
Existing TC/scout/density weights rebalanced to 0.35/0.35/0.15 to make
room for the new 0.15 danger weight (sum stays 1.0)."
```

---

## Task 2: Add globals — blacklist storage and per-scout flee state

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (globals block near line 120)

- [ ] **Step 1: Add per-scout flee state arrays after `gAutoScout_targetHerdID`**

Find:

```xs
// Per-scout: herd currently being diverted to in DIVERTING state. -1 when not diverting.
extern int[] gAutoScout_targetHerdID = default;
```

Append:

```xs
// Per-scout danger-avoidance state.
//   fleeUntilMs[slot]:  xsGetTime() value at which the FLEEING hold expires.
//   fleeFromArea[slot]: area we fled from (-1 when not fleeing). Diagnostics only.
extern int[] gAutoScout_fleeUntilMs   = default;
extern int[] gAutoScout_fleeFromArea  = default;
```

- [ ] **Step 2: Add blacklist arrays at end of globals block (just before any `void` declarations)**

Insert after the per-scout block, before the next function definition:

```xs
// Danger blacklist. Two parallel append-only arrays keyed by areaID. Areas
// enter when a scout aborts because of them and remain excluded from BFS
// picking until expiryMs < xsGetTime(). Linear scan on lookup; bounded by
// the number of distinct dangerous areas seen, which is small in practice.
extern int[] gAutoScout_blacklistedAreaIDs  = default;
extern int[] gAutoScout_blacklistedExpiryMs = default;
```

- [ ] **Step 4: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): add globals for flee state + danger blacklist"
```

---

## Task 3: Wire per-scout arrays into autoScout_register and autoScout_dropFromPool

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (`autoScout_register` near line 1350, `autoScout_dropFromPool` near line 282)

The two new per-scout fields (`gAutoScout_fleeUntilMs`, `gAutoScout_fleeFromArea`) need to be appended in `autoScout_register` and `removeIndex`'d in `autoScout_dropFromPool` so slot indices stay aligned.

- [ ] **Step 1: Append default values in `autoScout_register`**

Find (near line 1359):

```xs
   gAutoScout_state.add(cAutoScoutState_Idle);
```

Locate the full block of `.add` calls for the per-scout fields (immediately after this line — there should be calls for state/targetAreaID/targetWaypoint/workSteps/stuckTicks/targetHerdID). After the last existing `.add` (likely `gAutoScout_targetHerdID.add(-1);`), insert:

```xs
   gAutoScout_fleeUntilMs.add(0);
   gAutoScout_fleeFromArea.add(-1);
```

- [ ] **Step 2: `removeIndex` in `autoScout_dropFromPool`**

Find (near line 302):

```xs
   gAutoScout_targetHerdID.removeIndex(slot);
```

Append:

```xs
   gAutoScout_fleeUntilMs.removeIndex(slot);
   gAutoScout_fleeFromArea.removeIndex(slot);
```

- [ ] **Step 4: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): track flee fields in pool register/drop"
```

---

## Task 4: Implement danger / blacklist helpers

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (insert new helper block after `autoScout_dropFromPool`, near line 310)

- [ ] **Step 1: Add the helper block**

Insert this entire block just before the `// Position helpers` section comment:

```xs
//------------------------------------------------------------------------------
// Danger / blacklist helpers (2026-05-13)
//------------------------------------------------------------------------------

// Engine's per-area danger heuristic, averaged with one-area-hop neighbors so
// a tower in an adjacent area surfaces as elevated danger here.
bool autoScout_areaIsDangerous(int areaID = -1)
{
   if (areaID < 0) { return(false); }
   if (kbAreaGetIsIDValid(areaID) == false) { return(false); }
   float danger = kbAreaGetDangerLevel(areaID, true);
   return(danger > cAutoScout_DangerHardSkip);
}

// Add areaID to the blacklist (or bump its expiry if already present).
void autoScout_blacklistArea(int areaID = -1)
{
   if (areaID < 0) { return; }
   int newExpiry = xsGetTime() + cAutoScout_BlacklistDurationMs;
   int n = gAutoScout_blacklistedAreaIDs.size();
   for (int i = 0; i < n; i++)
   {
      if (gAutoScout_blacklistedAreaIDs[i] == areaID)
      {
         gAutoScout_blacklistedExpiryMs[i] = newExpiry;
         return;
      }
   }
   gAutoScout_blacklistedAreaIDs.add(areaID);
   gAutoScout_blacklistedExpiryMs.add(newExpiry);
}

// True if areaID is blacklisted AND its expiry has not yet passed. Expired
// entries are left in place; they get bumped naturally on re-blacklist.
bool autoScout_isAreaBlacklisted(int areaID = -1)
{
   if (areaID < 0) { return(false); }
   int now = xsGetTime();
   int n = gAutoScout_blacklistedAreaIDs.size();
   for (int i = 0; i < n; i++)
   {
      if (gAutoScout_blacklistedAreaIDs[i] == areaID)
      {
         return(gAutoScout_blacklistedExpiryMs[i] > now);
      }
   }
   return(false);
}
```

- [ ] **Step 3: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): add danger + blacklist helpers"
```

---

## Task 5: Implement autoScout_enterFleeing

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (extend the helper block from Task 4)

- [ ] **Step 1: Append `autoScout_enterFleeing` after `autoScout_isAreaBlacklisted`**

```xs
// Transition slot/unit to FLEEING. Computes a flee target opposite the danger
// area's center, issues a single aiTaskMoveUnit if the target is on-map and
// reachable, releases area claim, sets a 5-second hold timer. The handler
// keeps the scout in FLEEING until the timer expires regardless of arrival.
void autoScout_enterFleeing(int slot = -1, int unitID = -1, int dangerAreaID = -1)
{
   if (slot < 0) { return; }
   if (unitID < 0) { return; }

   vector scoutPos    = kbUnitGetPosition(unitID);
   vector dangerCenter = kbAreaGetCenter(dangerAreaID);
   vector awayDir     = xsVectorNormalize(scoutPos - dangerCenter);
   vector dest        = scoutPos + awayDir * cAutoScout_FleeDistance;

   bool destOK = false;
   if (autoScout_isOnMap(dest) == true)
   {
      int destArea = kbAreaGetIDByPosition(dest);
      if (destArea >= 0)
      {
         int unitProto = kbUnitGetProtoUnitID(unitID);
         if (kbCanPath(scoutPos, dest, unitProto, 1.0, -1) == true)
         {
            destOK = true;
         }
      }
   }

   if (destOK == true)
   {
      aiTaskMoveUnit(unitID, dest, false, false);
   }

   autoScout_releaseClaim(slot);
   gAutoScout_state[slot]        = cAutoScoutState_Fleeing;
   gAutoScout_fleeFromArea[slot] = dangerAreaID;
   gAutoScout_fleeUntilMs[slot]  = xsGetTime() + cAutoScout_FleeMinDurationMs;
   gAutoScout_stuckTicks[slot]   = 0;

   aiEcho("autoScout: FLEE slot=" + slot + " unit=" + unitID
      + " from area=" + dangerAreaID + " dest=" + dest
      + " destOK=" + destOK);
}
```

- [ ] **Step 3: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): add autoScout_enterFleeing"
```

---

## Task 6: BFS pick-time integration

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (`autoScout_areaIsCandidate` near line 649; `autoScout_scoreArea` near line 700-750)

- [ ] **Step 1: Hard-skip in `autoScout_areaIsCandidate`**

Find the function (search for `bool autoScout_areaIsCandidate`). After the existing `gAutoScout_areaSelfScouted` and claim-related early returns, add two new early returns:

```xs
   if (autoScout_isAreaBlacklisted(areaID) == true) { return(false); }
   if (autoScout_areaIsDangerous(areaID) == true) { return(false); }
```

(Place them after the existing self-scouted / claim checks, so we still pay for those cheap checks first.)

- [ ] **Step 2: Danger subscore in `autoScout_scoreArea`**

Find the line:

```xs
   float baseScore = cAutoScout_WeightTC * tcScore
                   + cAutoScout_WeightScout * scoutScore
                   + cAutoScout_WeightDensity * densityScore;
```

Immediately before this line, compute the danger subscore:

```xs
   // Danger subscore: zero-danger areas score 1.0; areas at the hard-skip
   // threshold score 0.0. Areas above threshold are excluded by
   // autoScout_areaIsCandidate so no overshoot is possible here.
   float danger = kbAreaGetDangerLevel(areaID, true);
   float dangerRatio = danger / cAutoScout_DangerHardSkip;
   if (dangerRatio < 0.0) { dangerRatio = 0.0; }
   if (dangerRatio > 1.0) { dangerRatio = 1.0; }
   float dangerScore = 1.0 - dangerRatio;
```

Then replace the `baseScore` assignment with the four-weight blend:

```xs
   float baseScore = cAutoScout_WeightTC * tcScore
                   + cAutoScout_WeightScout * scoutScore
                   + cAutoScout_WeightDensity * densityScore
                   + cAutoScout_DangerWeight * dangerScore;
```

- [ ] **Step 4: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): danger-aware BFS pick

Hard-skip blacklisted + over-threshold-danger areas in
autoScout_areaIsCandidate; add 0.15-weighted danger subscore in
autoScout_scoreArea. dangerScore = 1.0 - clamp01(danger/threshold)."
```

---

## Task 7: Per-tick danger check in `autoScout_tickUnit` (regular scouts)

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (`autoScout_tickUnit`, near line 1128)

The check goes after the validity/oracle-routing block but before the Idle handler. This way Idle is unaffected (Idle just re-picks, which already respects hard-skip), and Walking/Working/Diverting all get covered by a single check.

- [ ] **Step 1: Add the danger check**

Find:

```xs
   int state = gAutoScout_state[slot];

   if (state == cAutoScoutState_Idle)
```

Immediately before `int state = ...`, insert:

```xs
   // Danger check: applies to all non-Idle states. Idle is exempt because
   // the next BFS pick respects the hard-skip and blacklist directly.
   int preState = gAutoScout_state[slot];
   if (preState != cAutoScoutState_Idle && preState != cAutoScoutState_Fleeing)
   {
      vector unitPos = kbUnitGetPosition(unitID);
      int currentArea = -1;
      if (autoScout_isOnMap(unitPos) == true)
      {
         currentArea = kbAreaGetIDByPosition(unitPos);
      }
      int targetArea = gAutoScout_targetAreaID[slot];

      if (currentArea >= 0 && autoScout_areaIsDangerous(currentArea) == true)
      {
         autoScout_blacklistArea(currentArea);
         autoScout_enterFleeing(slot, unitID, currentArea);
         return(true);
      }
      if (targetArea >= 0 && targetArea != currentArea
          && autoScout_areaIsDangerous(targetArea) == true)
      {
         autoScout_blacklistArea(targetArea);
         autoScout_enterFleeing(slot, unitID, targetArea);
         return(true);
      }
   }
```

- [ ] **Step 3: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): per-tick danger check in regular tickUnit"
```

---

## Task 8: Per-tick danger check in `autoScout_tickOracleUnit`

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (`autoScout_tickOracleUnit`, near line 1025)

- [ ] **Step 1: Add the same danger-check block as Task 7, placed after `autoScout_updateMaxOracleLOS` and before the Diverting branch**

Find:

```xs
   float los = kbUnitGetStatFloat(unitID, cUnitStatLOS);
   int   state = gAutoScout_state[slot];

   // Diverting: shared logic.
```

Just before `float los = ...`, insert:

```xs
   // Danger check: same logic as regular tickUnit. Applies to all non-Idle
   // oracle states (Walking / Stationed / Diverting).
   int preState = gAutoScout_state[slot];
   if (preState != cAutoScoutState_Idle && preState != cAutoScoutState_Fleeing)
   {
      vector unitPos = kbUnitGetPosition(unitID);
      int currentArea = -1;
      if (autoScout_isOnMap(unitPos) == true)
      {
         currentArea = kbAreaGetIDByPosition(unitPos);
      }
      int targetArea = gAutoScout_targetAreaID[slot];

      if (currentArea >= 0 && autoScout_areaIsDangerous(currentArea) == true)
      {
         autoScout_blacklistArea(currentArea);
         autoScout_enterFleeing(slot, unitID, currentArea);
         return(true);
      }
      if (targetArea >= 0 && targetArea != currentArea
          && autoScout_areaIsDangerous(targetArea) == true)
      {
         autoScout_blacklistArea(targetArea);
         autoScout_enterFleeing(slot, unitID, targetArea);
         return(true);
      }
   }
```

- [ ] **Step 3: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): per-tick danger check for oracles"
```

---

## Task 9: FLEEING state handler

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (both `autoScout_tickUnit` and `autoScout_tickOracleUnit`)

The FLEEING handler is identical for oracles and regular scouts: hold until timer, then transition to Idle. Add it to both tick functions.

- [ ] **Step 1: Add FLEEING handler in `autoScout_tickUnit`**

Find the last `if (state == ...)` block before the function returns (likely Working). After it, but before the final `return(false);`, add:

```xs
   if (state == cAutoScoutState_Fleeing)
   {
      if (xsGetTime() < gAutoScout_fleeUntilMs[slot]) { return(false); }
      aiEcho("autoScout: flee-hold expired slot=" + slot + " unit=" + unitID
         + ", returning to Idle");
      autoScout_setStateIdle(slot);
      gAutoScout_fleeFromArea[slot] = -1;
      return(true);
   }
```

- [ ] **Step 2: Add the identical block in `autoScout_tickOracleUnit`**

Locate the equivalent end-of-function area (after the defensive `Working → Stationed` block, before `return(false);`):

```xs
   if (state == cAutoScoutState_Fleeing)
   {
      if (xsGetTime() < gAutoScout_fleeUntilMs[slot]) { return(false); }
      aiEcho("autoScout: oracle flee-hold expired slot=" + slot + " unit=" + unitID
         + ", returning to Idle");
      autoScout_setStateIdle(slot);
      gAutoScout_fleeFromArea[slot] = -1;
      return(true);
   }
```

- [ ] **Step 4: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): FLEEING state handler

Hold scout in FLEEING until xsGetTime() > fleeUntilMs, then transition to
Idle. BFS picks fresh from the new position; blacklisted area is excluded."
```

---

## Task 10: Diverting-state danger check refinement

**Files:**
- Modify: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (`autoScout_tickDivertingState`, near line 963)

Diverting is covered by the top-of-tick check from Tasks 7 and 8 — but those checks only look at `gAutoScout_targetAreaID`, which Diverting may not have set (the scout is chasing a herd, not an area target). The current-area check still fires for Diverting and catches "scout walked into a tower while chasing a herd". This task just adds the herd-id cleanup on flee from Diverting.

- [ ] **Step 1: Clear targetHerdID at the start of `autoScout_enterFleeing` if the slot is Diverting**

Update `autoScout_enterFleeing` (added in Task 5). After the initial validity guards, before `vector scoutPos = ...`, insert:

```xs
   // Clean up Diverting bookkeeping if we flee mid-divert. attemptedHerdIDs
   // already records the herd; we don't re-attempt it, matching the
   // "attempt once globally" rule.
   if (gAutoScout_state[slot] == cAutoScoutState_Diverting)
   {
      gAutoScout_targetHerdID[slot] = -1;
   }
```

- [ ] **Step 3: Commit**

```bash
git add mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
git commit --no-gpg-sign -m "feat(scout): clear herd target when fleeing from Diverting"
```

---

## Task 11: Smoke check + parse verification

This is verification, not new code. No commit if everything passes.

- [ ] **Step 1: grep for typos / missing references**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
grep -nE "autoScout_(areaIsDangerous|blacklistArea|isAreaBlacklisted|enterFleeing)" \
   mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
grep -nE "cAutoScoutState_Fleeing|cAutoScout_(DangerHardSkip|DangerWeight|FleeDistance|FleeMinDurationMs|BlacklistDurationMs)" \
   mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs
```

Expected: each helper called from at least one place; each constant referenced in code (not only in its declaration).

- [ ] **Step 2: Confirm forward-reference safety**

`autoScout_enterFleeing` is called from `autoScout_tickUnit` and `autoScout_tickOracleUnit` (Tasks 7-8). It's defined in the helper block from Task 5, which is above both tick functions in source order. XS within-file forward refs work anyway, but this confirms there's no implicit ordering bug.

- [ ] **Step 3: (User-driven) Deploy and playtest**

User runs `scripts/deploy-mods.sh` when the game is closed. First playtest will surface the actual `kbAreaGetDangerLevel` value range via the `aiEcho` lines added in Task 5. Expect to retune `cAutoScout_DangerHardSkip` after seeing the first 1-3 abort events.

---

## Summary

Total: 10 implementation tasks + 1 verification task. ~10 commits. ~120 lines added across two source files. No new state arrays in the global `extern` block that interact with anything outside the danger-avoidance feature itself.

Spec-coverage check:

| Spec section | Implementing task |
|---|---|
| State machine extension | Tasks 1, 2 |
| Blacklist storage + helpers | Tasks 2, 4 |
| BFS pick-time changes | Task 6 |
| Per-tick danger check | Tasks 7, 8 |
| FLEEING transition (`autoScout_enterFleeing`) | Tasks 5, 10 |
| FLEEING handler | Task 9 |
| Oracle handling | Tasks 8, 9 |
| Player override | (no change; existing pool-prune covers it) |
| Tunables | Task 1 |
| Diagnostic logs | Tasks 5, 9 |
