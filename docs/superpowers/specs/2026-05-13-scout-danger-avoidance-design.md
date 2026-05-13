# Intelligent Auto-Scout — Pre-emptive Danger Avoidance

**Date:** 2026-05-13
**Status:** Designed, not yet implemented. Continues the deferred work noted in `project_aom_retold_mod` after the Oracle support shipped.

## Goal

Stop auto-scouts (regular and Oracle) from walking into enemy towers, town centers, and military, where they get killed before completing their sweep. The mod currently picks BFS candidates with no awareness of enemy presence and runs commit-and-die when an aggressive map puts a defended area on the scout's route.

The fix is **hybrid + hard-skip**: penalize dangerous areas at BFS pick-time as a fourth weighted subscore, hard-skip any area whose danger exceeds a threshold, and add a per-tick re-check on the scout's current and target areas that aborts the current goal and runs the scout away from the threat for a forced minimum duration.

V1 uses **only** `kbAreaGetDangerLevel(areaID, true)` as the danger source. No own enemy query. Once empirically validated (or proven insufficient), we can swap in a per-tick `kbUnitQuery` for enemy military + defensive buildings — but the design contract stays the same.

## Architecture

One danger source: the engine's per-area heuristic `kbAreaGetDangerLevel(areaID, true)`. `averageInBorderAreas=true` averages each area's rating with its one-area neighbors, giving cheap one-hop lookahead without a graph walk.

One mutable per-player state: a danger blacklist of area IDs with per-entry expiry timestamps. Areas enter the blacklist when a scout aborts because of them; they're excluded from BFS picking until their expiry passes.

Three integration points:

1. **BFS pick** (`autoScout_findBestArea` / `autoScout_scoreArea`): hard-skip dangerous and blacklisted areas; otherwise discount their score via a fourth weighted subscore.
2. **Per-tick checks** at the top of each non-Idle state handler in `autoScout_tickUnit` and `autoScout_tickOracleUnit`: re-check the scout's current area and its target area; if either has crossed the hard-skip threshold, blacklist the offending area and transition to a new FLEEING state.
3. **FLEEING state**: issue one `aiTaskMoveUnit` in the direction opposite the danger area's center; hold the scout in FLEEING for a minimum duration regardless of arrival; on timer expiry transition to Idle, BFS picks fresh (the now-blacklisted area is excluded).

No new query objects. No own enemy enumeration in v1.

## State machine extension

One new state added alongside the existing five:

```xs
const int cAutoScoutState_Fleeing = 5;
```

Two new per-scout fields, appended/removed alongside the existing pool arrays in `autoScout_register` and `autoScout_dropFromPool`:

```xs
extern int[]  gAutoScout_fleeUntilMs = default;  // xsGetTime() of earliest Idle transition
extern int[]  gAutoScout_fleeFromArea = default; // area we fled FROM; for diagnostics
```

State transitions added to the existing graph:

```
WALKING / WORKING / STATIONED / DIVERTING ──danger──> FLEEING
FLEEING ──timer expired──> IDLE
```

Manual player override of a FLEEING scout drops it from the pool via the existing plan-invalid check; no special handling required.

## Blacklist storage

Two parallel append-only int arrays. Bounded by the number of distinct dangerous areas encountered, which is small for typical maps.

```xs
extern int[] gAutoScout_blacklistedAreaIDs   = default;
extern int[] gAutoScout_blacklistedExpiryMs  = default;
```

API (signatures only; bodies straightforward):

```xs
void autoScout_blacklistArea(int areaID = -1)
{
   // If areaID already present, bump expiry to xsGetTime() + cAutoScout_BlacklistDurationMs.
   // Else append (areaID, xsGetTime() + cAutoScout_BlacklistDurationMs).
}

bool autoScout_isAreaBlacklisted(int areaID = -1)
{
   // Linear scan. If found, return (expiry > xsGetTime()).
   // Do NOT remove expired entries inline; let them get bumped on re-blacklist.
}
```

Why not remove expired entries? Removal during linear scan is O(n) per remove and complicates indexing. The array grows only when a *new* area is blacklisted; expired entries are cheap to skip. If memory pressure becomes an issue, add a periodic compaction pass.

## BFS pick-time changes

Inside the candidate loop of `autoScout_findBestArea` (and any oracle-specific variant), before scoring:

```xs
float danger = kbAreaGetDangerLevel(candidateAreaID, true);
if (danger > cAutoScout_DangerHardSkip)                    { continue; }
if (autoScout_isAreaBlacklisted(candidateAreaID) == true)  { continue; }
```

In `autoScout_scoreArea`, the existing three-weight blend (TC / scout / density at 0.4 / 0.4 / 0.2) is rebalanced to four weights summing to 1.0:

| Subscore | Old weight | New weight |
|---|---|---|
| TC distance | 0.4 | 0.35 |
| Scout distance | 0.4 | 0.35 |
| Density (other scouts) | 0.2 | 0.15 |
| **Danger (new)** | — | **0.15** |

Danger subscore:

```xs
float dangerRatio = danger / cAutoScout_DangerHardSkip;
if (dangerRatio < 0.0) { dangerRatio = 0.0; }
if (dangerRatio > 1.0) { dangerRatio = 1.0; }
float dangerScore = 1.0 - dangerRatio;
```

Zero-danger areas score 1.0; areas at the hard-skip threshold score 0.0. Areas above threshold are excluded before scoring runs, so no overshoot.

The same scoring path is used for both regular-scout source-picking and oracle source-picking. The oracle-specific overlap rules (`autoScout_anyOracleNear` / `autoScout_oraclePenalty`) layer on top of the rebalanced base score.

## Per-tick danger check

At the top of each non-Idle state handler in both `autoScout_tickUnit` and `autoScout_tickOracleUnit`:

```xs
int currentArea = kbAreaGetIDByPosition(kbUnitGetPosition(unitID));
int targetArea  = gAutoScout_targetAreaID[slot];

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
```

Where `autoScout_areaIsDangerous(areaID)` returns `kbAreaGetDangerLevel(areaID, true) > cAutoScout_DangerHardSkip`.

Two calls per scout per tick at most (current + target). Border-averaging on each gives implicit 1-hop lookahead at both endpoints. Areas strictly between current and target are **not** sampled in v1; we trust border-averaging to surface immediate threats. If playtest shows scouts dying mid-route through a third area, line-sample 1-3 points between scoutPos and targetCentroid as a follow-up.

Diverting handler (`autoScout_handleDiverting`) uses the same check. On flee from Diverting, additionally clear `gAutoScout_targetHerdID[slot]`; do NOT add to `gAutoScout_attemptedHerdIDs` (it's already there) — the cost of the aborted divert is one lost attempt under the "attempt once globally" rule.

## FLEEING transition

```xs
void autoScout_enterFleeing(int slot = -1, int unitID = -1, int dangerAreaID = -1)
{
   vector scoutPos = kbUnitGetPosition(unitID);
   vector dangerCenter = kbAreaGetCenter(dangerAreaID);
   vector fleeDir = xsVectorNormalize(scoutPos - dangerCenter);
   vector dest = scoutPos + fleeDir * cAutoScout_FleeDistance;

   // Sanity: dest must be on a valid area and reachable. Uses the existing
   // kbCanPath signature (pos, dest, unitProto, pathRadius, excludeUnitID)
   // matching the herd-and-frontier path checks elsewhere in this file.
   int destArea = kbAreaGetIDByPosition(dest);
   int unitProto = kbUnitGetProtoUnitID(unitID);
   if (destArea >= 0 && kbCanPath(scoutPos, dest, unitProto, 1.0, -1) == true)
   {
      aiTaskMoveUnit(unitID, dest, false, false);
   }
   // If dest is invalid/unreachable, skip the move; the 5s timer still holds
   // the scout in FLEEING in place. BFS will re-pick safely after.

   // Release area claim inline (no shared helper exists; pattern from
   // autoScout_dropFromPool).
   int prevArea = gAutoScout_targetAreaID[slot];
   if (prevArea >= 0 && prevArea < gAutoScout_areaClaim.size())
   {
      gAutoScout_areaClaim[prevArea] = 0;
   }
   gAutoScout_targetAreaID[slot]   = -1;
   gAutoScout_state[slot]        = cAutoScoutState_Fleeing;
   gAutoScout_fleeFromArea[slot] = dangerAreaID;
   gAutoScout_fleeUntilMs[slot]  = xsGetTime() + cAutoScout_FleeMinDurationMs;
}
```

FLEEING handler:

```xs
if (state == cAutoScoutState_Fleeing)
{
   if (xsGetTime() < gAutoScout_fleeUntilMs[slot]) { return(false); }  // hold
   gAutoScout_state[slot] = cAutoScoutState_Idle;
   return(true);
}
```

The 5-second timer is enforced regardless of whether the move command completes. Three reasons:

1. The scout may arrive before 5s if the flee destination was close; holding in FLEEING prevents an instant re-pick that would put it back into the same danger.
2. The scout may still be in flight after 5s; transitioning to Idle is harmless because `cPlanStateIdle` keeps the engine from re-tasking, and the very next tick's BFS picks a new target away from the blacklisted area.
3. If `dest` was invalid and no move was issued, the scout stands still for 5s — still better than picking the same dangerous area again immediately.

## Oracle handling

Oracles use the same checks and the same FLEEING transition. Behavioural specifics:

- A Stationed oracle that becomes dangerous mid-saturation aborts. The lost saturation progress is the cost of avoiding death.
- After flee, the re-picked area is BFS-scored normally with the oracle overlap penalty (`autoScout_oraclePenalty`) layered on top. The now-blacklisted dangerous area is excluded.
- No special "go home and wait" behavior for oracles. They re-engage as soon as the 5s timer expires.

## Player override

The existing override-detection (plan-invalid check in `autoScout_tick`) handles FLEEING with no additions. If the player gives a FLEEING scout any command, the underlying `cPlanExplore` plan is cancelled by the engine, our pool-prune step removes the scout, and the manually-commanded path proceeds normally. The flee state, blacklist, and any in-flight `aiTaskMoveUnit` are all dropped on pool-removal.

## Tunables

```xs
const float cAutoScout_DangerHardSkip       = 5.0;      // playtest-tune
const float cAutoScout_DangerWeight         = 0.15;     // BFS subscore weight
const float cAutoScout_FleeDistance         = 25.0;     // tiles
const int   cAutoScout_FleeMinDurationMs    = 5000;
const int   cAutoScout_BlacklistDurationMs  = 90000;
```

Rationale:

- `cAutoScout_DangerHardSkip = 5.0`: placeholder. `kbAreaGetDangerLevel`'s range is undocumented and we have no calibration data; first playtest will surface observed values. Diagnostic `aiEcho` on every flee transition prints the observed danger so we can re-tune.
- `cAutoScout_FleeDistance = 25.0` tiles: matches typical AoMR tower attack range (~20 tiles) with a buffer.
- `cAutoScout_FleeMinDurationMs = 5000`: at scout walk speed (~5 tiles/sec), 5s covers `cAutoScout_FleeDistance`. Long enough to physically clear the threat's range, short enough not to feel sticky if the threat was transient.
- `cAutoScout_BlacklistDurationMs = 90000` (90s): long enough to discourage immediate ping-pong; short enough that if the enemy retreats or the tower is destroyed, the area is re-scoutable in a reasonable time.

## Diagnostic logs

Existing log filter `autoScout: ` prefix:

- On flee entry: `"autoScout: flee slot=N from area=X danger=Y dest=Z"`
- On BFS hard-skip: `"autoScout: skip area=X danger=Y (over hard-skip)"` (per candidate, gated by a verbose flag to avoid log spam)
- On blacklist expiry: no log (silent).

## Open questions to resolve in playtest, not spec

1. `cAutoScout_DangerHardSkip` calibration. The first playtest's `aiEcho` traces will tell us the real value range of `kbAreaGetDangerLevel`. Expect to retune.
2. Whether `averageInBorderAreas=true` is sufficient lookahead, or whether line-sampling (Section 3 deferred path) becomes necessary.
3. Whether 5s flee duration clears a tower's range in practice, or whether a 7-8s value is more reliable.
4. Whether the engine's danger heuristic over-rates wandering enemy hunters (early game, when AI scouts roam Gaia areas). If so, we may need to swap in our own enemy-buildings-only query rather than trusting the engine.

## Out of scope for v1

- Own enemy query (deferred to v2 if v1 proves insufficient).
- Line-sampling between current and target area (deferred per Section 3).
- Threat-direction inference from multiple nearby dangerous areas; we flee away from the single area that triggered the abort.
- HP-based flee triggers (scout HP dropping fast → flee even if area danger is low).
- "Memory" of dead scouts — currently a scout that dies just disappears from the pool with no marking of the killing area beyond whatever danger rating the area had.

## Deployment

Same as other auto-scout changes: deploy via `scripts/deploy-mods.sh` after editing `auto_scout.xs` in the source mod folders. The change is self-contained to `auto_scout.xs`; no `human_assist.xs` edits, no new files.
