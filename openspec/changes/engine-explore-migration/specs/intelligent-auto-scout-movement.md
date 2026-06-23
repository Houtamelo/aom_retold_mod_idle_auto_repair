# intelligent-auto-scout-movement

## Overview

Replace the mod's hand-issued `aiTaskMoveUnit` movement with engine-driven area exploration for player-controlled scouts. BFS scoring, per-area claims, herd divert, and the danger heat-map remain; frontier-walk, corridor chains, and waypoint sampling are removed.

## Requirements

### R1 Remove frontier-walk

The system MUST delete the `autoScout_findFrontierWaypoint` implementation and the `WORKING`-state frontier-walk loop currently at `auto_scout.xs:2260-2310`.

### R2 Remove corridor logic

The system MUST delete `autoScout_issueCorridor`, `autoScout_corridorFirstDangerousHop`, the `gAutoScout_corridorAreas` flat array, and all corridor-related state fields.

### R3 Remove manual move issuance for normal exploration

The system MUST NOT issue `aiTaskMoveUnit(unitID, ...)` for ordinary area-to-area movement. Manual moves remain permitted only for herd divert and flee override.

### R4 Keep BFS scoring and claims

The system MUST keep `autoScout_findNextArea` (`auto_scout.xs:2101-2254`), `autoScout_areaScore`, `autoScout_areaIsCandidate`, `autoScout_releaseClaim`, and the `gAutoScout_areaClaim` / `gAutoScout_areaSelfScouted` arrays.

### R5 BFS-first, filter-second ordering

The system MUST first run `autoScout_findNextArea` to produce a ranked candidate area and its predecessor chain, then pass that ranked list through the heat-map filter, and only then write the surviving area IDs to `cExplorePlanExploreAreaIDs`. Ranking is computed before danger filtering so a slightly hotter but much closer area does not outrank a safer distant one.

### R6 Feed the engine via area IDs

The system MUST call `autoScout_explorePlanAssignAreas(planID, filteredAreaIDs)` to hand the filtered list to the engine.

### R7 Conditional parking

The system SHALL park the engine plan (`aiPlanSetState(planID, cPlanStateIdle)`) only during herd divert and flee override. The default registration state leaves the plan active.

### R8 Re-assign on resume

The system SHALL recompute and reassign `cExplorePlanExploreAreaIDs` immediately when un-parking after divert or flee.

### R9 PERFORMANCE CADDENCE (optional)

The system MAY add a per-slot tick counter so that the full BFS/filter/assignment pipeline runs only every N ticks (`N >= 1`) instead of every tick.

## Scenarios

### S1 Normal scouting via engine areas

- GIVEN `autoScout_findNextArea` returns area 17 with border areas 18 and 19
- WHEN the heat-map filter marks 18 as dangerous
- THEN `autoScout_explorePlanAssignAreas` writes `{17, 19}` to `cExplorePlanExploreAreaIDs`
- AND no `aiTaskMoveUnit` is issued by the mod for that movement.

### S2 Danger filters the top candidate

- GIVEN `autoScout_findNextArea` returns area 31 with the highest score
- WHEN `autoScout_areaIsDangerous(31) == true`
- THEN 31 is removed from the list and the next safe ranked area is assigned instead.

### S3 Herd divert parks and resumes

- GIVEN a scout is active and `autoScout_tryDivert` finds an eligible herd
- WHEN the slot enters `cAutoScoutState_Diverting`
- THEN the plan is parked, `aiTaskMoveUnit(unitID, herdPos, false, false)` is issued
- AND when `autoScout_tickDivertingState` finishes, the plan is un-parked and the area list is recomputed.

### S4 Flee override parks and resumes

- GIVEN a scout is active and its current area becomes dangerous
- WHEN the slot enters fleeing
- THEN the plan is parked, a flee move is issued
- AND when the flee hold expires, the plan is un-parked and the area list is recomputed.

## XS API constraints

| API | Signature | Source |
| --- | --- | --- |
| `autoScout_findNextArea` | `int (int scoutUnitID)` | `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:2101-2254` |
| `autoScout_tryDivert` | `bool (int slot, int unitID, float los)` | `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:1522-1542` |
| `autoScout_tickDivertingState` | `bool (int slot, int unitID)` | `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:2424-2456` |
| `autoScout_explorePlanAssignAreas` | `void (int planID, ref int[] areaIDs)` | derived from `mod/auto_scout_test/game/ai/human_assist/human_assist.xs:53-59` |
| `aiPlanSetState` | `void (int planID, int planState)` | `docs/doxygen_retail/aifuncs_8cpp.html:4139` |
| `aiTaskMoveUnit` | `bool (int unitID, vector position, bool attackMove, bool queue)` | `docs/doxygen_retail/aifuncs_8cpp.html:5739` |

## Out of scope

- Restoring frontier-walk as a fallback.
- Military-unit movement handled by the civ AI.

## Open research

- **[RESEARCH-DEFERRED: RP4]** Reassignment after un-park assumes the engine will consume the freshly written `cExplorePlanExploreAreaIDs` list. Whether `cExplorePlanExploreAreaIDsCurrentIndex` resets or persists across `aiPlanSetState(cPlanStateIdle)` → `cPlanStateExplore` transitions must be verified manually.
