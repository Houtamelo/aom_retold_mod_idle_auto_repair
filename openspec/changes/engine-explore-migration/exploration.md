# Exploration: Engine-based area exploration migration

## Change id
`engine-explore-migration`

## Status
- [x] Phase: explore
- [ ] Next: propose

## Goal recap

The `intelligent_auto_scout` mod currently replaces the engine's `cPlanExplore` behavior with a hand-rolled state machine: it parks the engine plan in `cPlanStateIdle` and then issues manual `aiTaskMoveUnit` commands through a custom BFS, heat-map, frontier-walk, and Oracle-saturation detector. A proof of concept in `mod/auto_scout_test/` shows that the engine's own `cPlanExplore` can be driven by area IDs through `cExplorePlanExploreAreaIDs`, letting the engine handle pathfinding, area sequencing, danger-aware area skipping, and looping internally. The migration would move the mod from manual movement back to an engine-owned explore plan, which should reduce edge cases (stuck scouts, pathing corners, oscillation) and shrink the per-tick script load, while preserving the mod's higher-level features such as herd delivery and Oracle coordination.

## 1. POC architecture summary

Source: `mod/auto_scout_test/game/ai/human_assist/human_assist.xs`.

- **Plan creation**: `enableAutoScouting(unitID)` creates one `cPlanExplore` per scout and assigns exactly that unit (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:250-284`).
- **Area assignment helper**: `pocExplorePlanAssignAreas(planID, areaIDs)` resizes `cExplorePlanExploreAreaIDs` with `aiPlanSetNumberVariableValues` and fills it with `aiPlanSetVariableInt` (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:53-59`).
- **Engine knobs set at creation**:
  - `cExplorePlanAvoidingAttackedAreas = true` for all scouts (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:270`).
  - `cExplorePlanDoLoops = false` for all scouts (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:259`, `277`).
  - For Oracles: `cExplorePlanStopLOSPercentage = 0.5` (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:279`).
  - Vanilla `human_assist.xs` only sets `cExplorePlanDoLoops=false` and `cExplorePlanStopLOSPercentage=0.2` for Oracles and nothing else (`~/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/human_assist/human_assist.xs:102-107`).
- **Area discovery helpers** (defined in the POC but not invoked from the current `enableAutoScouting` body):
  - `autoScout_helperExploreStartingSurroundings(planID)` queries areas within 130 range of the starting position using `kbAreaGetIDsByPositionAndRange` with types `PassableLand`, `Gold`, `Settlement` and `cPassabilityLand`, removes already-explored areas, and assigns the rest (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:67-112`).
  - `autoScout_helperExploreFarAreas(planID, scoutPos)` falls back to the scout's entire area group via `kbAreaGroupGetIDByPosition` / `kbAreaGroupGetNumberAreas` / `kbAreaGroupGetAreaID` (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:115-142`).
  - `pocExploreCurrentArea(unitID, planID)` and `pocExploreNeighborAreas(unitID, planID)` refresh the plan with the current area and its borders (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:205-245`).
- **Explored-area pruning**: `pocRemoveExploredAreas(ref int[] areas)` drops any area where `kbAreaGetPercentExplored(areaID) >= 1.0` (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:191-202`).
- **Cleanup**: `cleanupLingeringExplorePlans` destroys every `cPlanExplore` with zero units (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:290-303`).

> **Note**: The POC's initial creation in `enableAutoScouting` assigns the *entire area group* of the scout's current position, not the starting-surroundings helper. The starting-surroundings and neighbor-refill helpers are implemented and referenced by name but are not wired into the visible call graph in this file.

## 2. Current mod architecture summary

Sources: `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs` and `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`.

- **Plan creation / hook**: `enableAutoScouting` still creates one `cPlanExplore` per unit, then immediately calls `autoScout_register(planID, unitID)` (`mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:95-113`).
- **Plan parking**: `autoScout_register` sets `aiPlanSetState(planID, cAutoScout_PlanStateIdle)` where `cAutoScout_PlanStateIdle = 23 == cPlanStateIdle` (`auto_scout.xs:80`, `2910`; `docs/MythTRConstants.txt:3166`). This keeps the plan alive as a UI marker but tells the engine not to drive the unit.
- **State machine**: Each registered scout slot has a state from the `cAutoScoutState_*` enum (`auto_scout.xs:15-20`):
  - `Idle`, `Walking`, `Working`, `Diverting`, `Stationed`, `Fleeing`.
- **Tick rules**:
  - `autoScout_tickHeavy` rebuilds the heat-map and runs herd home-move (`auto_scout.xs:3020-3029`).
  - `autoScout_tickFast` advances the state machine for every registered scout (`auto_scout.xs:3031-3053`).
- **Coverage algorithm**: custom layered BFS over the area-adjacency graph with scoring by TC proximity, scout proximity, friendly-scout density, effective danger, and Oracle overlap (`auto_scout.xs:1699-2254`). After reaching an area, regular scouts enter a `WORKING` frontier-walk that samples 5 radii × 8 angles for in-area, out-of-LOS waypoints (`auto_scout.xs:2260-2310`).
- **No engine area IDs**: the mod never writes to `cExplorePlanExploreAreaIDs`; movement is entirely via `aiTaskMoveUnit`.
- **Features layered on top of the parked `cPlanExplore`**:
  - Danger avoidance: per-tick heat-map from visible enemy threats + damage/death event bumps + flee state + 90-second blacklists (`auto_scout.xs:600-1372`).
  - Herd delivery: `Diverting` state converts eligible herds and `autoScout_homeMoveScan` routes owned herds to the nearest TC (`auto_scout.xs:1375-1542`, `2940-2999`).
  - Oracle coordination: separate `autoScout_tickOracleUnit` state machine waits for action type 37 (saturated LOS) and uses overlap penalties to spread Oracle LOS rings (`auto_scout.xs:2467-2627`, `1596-1693`).
  - Per-area claims and self-scouted flags to prevent duplicate assignment (`auto_scout.xs:244-255`, `479-501`).

> **Design-doc note**: The files `docs/superpowers/specs/2026-05-04-intelligent-auto-scout-design.md`, `docs/superpowers/plans/2026-05-06-oracle-scouting-implementation.md`, and `docs/superpowers/specs/2026-05-13-scout-danger-avoidance-design.md` were requested but do not exist in the current repository. The feature intent above is taken from the `auto_scout.xs` header comments and implementation.

## 3. Side-by-side comparison

| Concern | POC behavior | Current mod behavior | Migration implication |
|---|---|---|---|
| **Scout creation** | One `cPlanExplore` per scout, unit added directly (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:250-284`). | One `cPlanExplore` per scout, then parked in `cPlanStateIdle` and hijacked by the mod (`mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:95-113`, `auto_scout.xs:2898-2910`). | Keep the creation wrapper; stop parking the plan so the engine drives it. |
| **Area discovery** | Starting surroundings via `kbAreaGetIDsByPositionAndRange` within 130 range of start; fallback to full area group; current/neighbor area refill helpers exist (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:67-142`, `205-245`). | BFS from scout position over area adjacency, collecting candidates batch-by-batch (`auto_scout.xs:2101-2254`). | Replace the custom BFS candidate search with POC-style area-group / neighbor area assignment into `cExplorePlanExploreAreaIDs`. |
| **Frontier expansion** | Engine sequences through assigned area IDs; `pocExploreNeighborAreas` can re-fill the array when the scout reaches borders (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:219-245`). | Manual frontier-walk with 5×8 ring sampling inside the current area (`auto_scout.xs:2260-2310`). | Delete manual frontier-walk; rely on engine area sequencing plus neighbor refresh. |
| **Danger handling** | Sets `cExplorePlanAvoidingAttackedAreas=true` and trusts the engine (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:270`). [unverified] exact engine semantics. | Hand-built heat-map, hard-skip thresholds, flee state, and blacklists (`auto_scout.xs:600-1372`). | Decide whether to keep a light heat-map override or fully trust `cExplorePlanAvoidingAttackedAreas`. |
| **Herd handling** | None; incidental conversion only (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs` has no herd logic). | Explicit `Diverting` state and automatic home-move to TC (`auto_scout.xs:1375-1542`, `2940-2999`). | Keep herd divert + home-move as a layer above the engine plan. |
| **Oracle coordination** | Oracles get `cExplorePlanStopLOSPercentage=0.5`; no multi-Oracle overlap logic (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:275-280`). | Separate state machine waits for action 37 (saturated LOS) and applies overlap penalties (`auto_scout.xs:2467-2627`, `1596-1693`). | Either drop the custom Oracle machine and rely on `StopLOSPercentage=0.5`, or keep overlap logic and re-evaluate whether the engine plan conflicts with parking. |
| **Oracle LOS wait** | Engine-driven via `cExplorePlanStopLOSPercentage=0.5` (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:279`). | Manual wait for `kbUnitGetActionType == cAutoScout_OracleSaturatedActionType` (37) (`auto_scout.xs:2594-2601`). | If the engine wait is sufficient, remove action-37 detection; otherwise keep it as an auxiliary trigger. |
| **Looping** | `cExplorePlanDoLoops=false` for everyone (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:259`, `277`). | No explicit loop; state machine re-picks areas until exhausted. | Continue using `DoLoops=false`; engine area list is consumed once. |
| **Area reassignment on unit death** | `cleanupLingeringExplorePlans` destroys plans with zero units every 15 s (`mod/auto_scout_test/game/ai/human_assist/human_assist.xs:290-303`). | `autoScout_dropFromPool` removes the slot and emits a death heat event (`auto_scout.xs:502-563`). | Keep `cleanupLingeringExplorePlans`; decide if death heat event still matters without the mod danger layer. |
| **Performance** | Two small rules; engine does pathing/sequencing. | Two `minInterval 1` rules, per-tick heat-map rebuild, BFS, frontier sampling, threat profiling (`auto_scout.xs:1010-1121`, `2101-2254`, `2260-2310`). | Significant script-load reduction if heat-map/BFS/frontier-walk are removed or throttled. |
| **Complexity** | ~6 small helpers + vanilla `human_assist.xs` body. | ~3,054 lines of state machines, heat-map, queries, corridor logic, Oracle overlap. | Large deletion surface; must preserve herd/oracle/danger decisions that survive. |
| **Edge cases** | Relies on engine for stuck/pathing; neighbor re-fill assumes `kbAreaGetIDByPosition` matches engine current-area view. [unverified] | Custom stuck detection, blacklists, corridor danger checks, visited-waypoint memory, centroid-short-circuit (`auto_scout.xs:2407-2417`, `2728-2808`). | Some custom guards may still be needed on top of the engine plan. |

## 4. Migration plan

### Step 1 — Stop parking the engine plan
- **Where**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:2898-2936`.
- **What**: Remove `aiPlanSetState(planID, cAutoScout_PlanStateIdle)` from `autoScout_register`, or make it conditional. The plan must remain in an engine-active state so `cExplorePlanExploreAreaIDs` drives the unit.
- **Keep from mod**: Human-player guard (`kbPlayerIsHuman(cMyID)`), pool bookkeeping for herd/danger/Oracle features.
- **Replace with**: vanilla-style plan setup plus POC area assignment.
- **Verify**: Deploy; toggle Auto-Scout; confirm in `aiEcho` that the unit moves without the mod issuing `aiTaskMoveUnit`.

### Step 2 — Add area-assignment helpers to `auto_scout.xs`
- **Where**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` (new helpers near the top).
- **What**: Port `pocExplorePlanAssignAreas`, `pocRemoveExploredAreas`, `autoScout_helperExploreStartingSurroundings`, `autoScout_helperExploreFarAreas`, `pocExploreCurrentArea`, and `pocExploreNeighborAreas` from the POC.
- **Keep from mod**: Initialize the existing area arrays (`gAutoScout_areaClaim`, `gAutoScout_areaSelfScouted`) before use.
- **Replace with**: `kbAreaGetIDsByPositionAndRange` + `kbAreaGroupGet*` based area lists instead of the custom BFS.
- **Verify**: Deploy; watch `aiEcho` for the assigned area IDs and confirm `cExplorePlanExploreAreaIDs` increases.

### Step 3 — Set engine knobs in `enableAutoScouting`
- **Where**: `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:95-113`.
- **What**: Add `aiPlanSetVariableBool(planID, cExplorePlanAvoidingAttackedAreas, 0, true)` for all scouts; keep `cExplorePlanDoLoops=false`; set `cExplorePlanStopLOSPercentage=0.5` for Oracles (POC value) instead of the current 0.2.
- **Keep from mod**: `autoScout_register` call as the feature hook point.
- **Replace with**: POC engine-knob configuration.
- **Verify**: Deploy with Oracles; check that they park at ~50% unexplored surrounding tiles, not 20%.

### Step 4 — Drive the plan with areas instead of issuing `aiTaskMoveUnit`
- **Where**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:2101-3054`.
- **What**: Replace `autoScout_findNextArea` / `autoScout_issueCorridor` / `autoScout_findFrontierWaypoint` logic with calls to assign area IDs to `cExplorePlanExploreAreaIDs`. Keep a lightweight rule that re-fills neighbor areas as the scout moves.
- **Keep from mod**: Diverting/herd delivery, danger flee decision, Oracle saturation wait if still needed, death-event cleanup.
- **Replace with**: POC `pocExploreNeighborAreas` or similar re-fill plus engine movement.
- **Verify**: Deploy on a revealed map; use `aiEcho` to confirm the scout visits different area IDs and does not oscillate.

### Step 5 — Decide danger layer fate
- **Where**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:600-1372`.
- **What**: Either delete the heat-map and flee state (trust `cExplorePlanAvoidingAttackedAreas`) or keep a minimal version that overrides area assignment by removing dangerous areas from `cExplorePlanExploreAreaIDs`.
- **Keep from mod**: Death-event heat bump if danger layer is retained.
- **Replace with**: POC's simple danger flag or a filtered area list.
- **Verify**: Deploy against an enemy base; confirm scouts do not walk into towers unless the engine overrides the avoidance flag.

### Step 6 — Preserve herd delivery
- **Where**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:1375-1542`, `2940-2999`.
- **What**: Keep `autoScout_tryDivert`, `autoScout_tickDivertingState`, and `autoScout_homeMoveScan`. If the engine plan is active, briefly redirect the unit with `aiTaskMoveUnit` toward the herd; after conversion, the engine plan resumes exploring.
- **Keep from mod**: Attempt-once herd tracking and home-move-to-TC behavior.
- **Replace with**: Nothing; this feature has no POC equivalent.
- **Verify**: Deploy with a scout that converts herds; confirm converted herds walk toward a TC.

### Step 7 — Preserve Oracle overlap / saturation logic (optional)
- **Where**: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:2467-2627`, `1596-1693`.
- **What**: If `cExplorePlanStopLOSPercentage=0.5` alone does not spread Oracles well, keep the overlap penalty and use it to filter the area list before assignment, instead of issuing manual moves.
- **Keep from mod**: `gAutoScout_maxOracleLOS` cache and oracle query.
- **Replace with**: Area-list filtering rather than `aiTaskMoveUnit` parking.
- **Verify**: Deploy with 2+ Oracles; confirm their final positions do not heavily overlap.

### Step 8 — Cleanup and unify variants
- **Where**: `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`, `mod/intelligent_auto_repair_and_scout/...`, `mod/human_assist_improvements/...`.
- **What**: Apply the same `enableAutoScouting` changes to all variants; ensure `scripts/deploy-mods.sh` still copies the shared `auto_scout.xs` correctly.
- **Keep from mod**: Existing mod-package structure and README conflict warnings.
- **Replace with**: Updated `human_assist.xs` knob setup in each variant.
- **Verify**: Deploy each variant standalone; confirm toggling Auto-Scout works.

## 5. Risk register

| Risk | Severity | Mitigation |
|---|---|---|
| **Mod's `cAutoScout_PlanStateIdle` parking trick is incompatible with an engine-driven `cPlanExplore`.** If the plan stays in state 23, the engine will not consume `cExplorePlanExploreAreaIDs` and the unit will not move. | High | Remove or conditionalize the `aiPlanSetState(..., 23)` call in `autoScout_register` (verified at `auto_scout.xs:2910`; `cPlanStateIdle=23` at `docs/MythTRConstants.txt:3166`). |
| **`cExplorePlanAvoidingAttackedAreas=true` may mean "skip areas that were attacked" rather than "actively flee while inside a dangerous area".** The engine-level semantics are not documented beyond the variable name. | High | Test scouts near enemy towers; if they die inside attacked areas, keep a minimal flee override or raise the engine danger threshold via `aiSetExploreDangerThreshold`. |
| **`kbAreaGetIDByPosition` for the scout's position may not match the engine's internal "current area" used by `cPlanExplore`, causing `pocExploreNeighborAreas` to assign areas the engine thinks are unreachable.** | Medium | Add `kbCanPath` guards before adding border areas, and log mismatches between `kbAreaGetIDByPosition` and the plan's `cExplorePlanExploreAreaIDsCurrentIndex`. |
| **Multiple scouts in the same area group each receive the full area list, so the engine may send several scouts to the same first area.** | Medium | Either reintroduce a light per-area claim system that removes claimed areas from other scouts' lists, or rely on density logic from Step 7. |
| **`pocExploreNeighborAreas` adds the current area plus borders; if all neighbors are already explored, the plan can stall until the rule refires.** | Low | Run the re-fill rule at `minInterval 1` while any `cPlanExplore` is active, and ensure `cleanupLingeringExplorePlans` destroys finished empty plans. |
| **Oracle `cExplorePlanStopLOSPercentage=0.5` may not replicate the mod's action-37 saturated-LOS wait, causing Oracles to repick earlier/later than intended.** | Medium | A/B test 0.5 vs 0.2 against the existing action-37 detector; keep the detector as a fallback if timing diverges. |
| **Herd divert briefly overrides the engine plan; if the engine plan reasserts before conversion completes, the scout may abandon the herd.** | Low | Set the plan to `cPlanStateIdle` only during the divert, or issue the herd move with `queue=false` and rely on the next tick to reassign areas. |
| **Removing the heat-map deletes the death-event danger inheritance, so a scout dying in an enemy base will not warn subsequent scouts.** | Medium | If danger layer is removed entirely, accept this behavioral change or keep a small event system that only blacklists the death area for a short duration. |
| **Patch-maintenance risk**: `human_assist.xs` is a vanilla overlay; adding more knobs to it increases rebase surface after game updates. | Low | Document changed lines in the mod README and keep the diff minimal. |

## 6. Open questions for the user

1. **Danger strategy**: Do we keep the mod's hand-rolled danger heat-map and feed it into area-list filtering (or `aiSetExploreDangerThreshold`), or trust the engine's native `cExplorePlanAvoidingAttackedAreas`?
2. **Oracle saturation wait**: Do we keep the action-37 detector on top of `cExplorePlanStopLOSPercentage=0.5`, or replace it entirely with the engine percentage?
3. **Herd delivery**: Is herd auto-home-move a must-have for the migrated build, and should divert still preempt area exploration?
4. **Per-area claim/lock system**: Should we retain the per-area claim lock to avoid sending multiple scouts to the same area, or rely on the engine's natural sequencing?
5. **Multi-variant rollout**: Should the migration land in `intelligent_auto_scout`, `intelligent_auto_repair_and_scout`, and `human_assist_improvements` in lockstep, or validate in one variant first?
6. **Performance target**: Is the primary goal to reduce script load (delete heat-map/BFS/frontier), or to preserve current behavior while using the engine for movement?
7. **Military-unit scope**: The previous exploration (`base-ai-military-scout-comparison`) found the civ AI uses military units in `cPlanExplore`. Should this migration stay scoped to `AbstractScout`/Oracle units only?
8. **Testing maps**: Which map types should be used for manual verification? (The POC's area-group fallback matters most on single-landmass vs. island maps.)

## 7. Recommended next phase

`next_recommended: propose`
