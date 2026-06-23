# engine-area-explore

## Overview

Feed a ranked list of land area IDs into the engine's `cPlanExplore` via `cExplorePlanExploreAreaIDs` so the engine handles pathfinding and waypoint sequencing. The mod keeps the high-level decision layer (BFS scoring, heat-map filter, per-area claims) but delegates movement to the engine.

## Requirements

### R1 Port POC assignment helpers

The system MUST port the following helpers from `mod/auto_scout_test/game/ai/human_assist/human_assist.xs` into `auto_scout.xs` and prefix each with `autoScout_`:

- `autoScout_explorePlanAssignAreas(planID, ref int[] areaIDs)`
- `autoScout_removeExploredAreas(ref int[] areas)`
- `autoScout_helperExploreStartingSurroundings(planID)`
- `autoScout_helperExploreFarAreas(planID, scoutPos)`
- `autoScout_exploreCurrentArea(unitID, planID)`
- `autoScout_exploreNeighborAreas(unitID, planID)`

`autoScout_explorePlanAssignAreas` MUST resize `cExplorePlanExploreAreaIDs` with `aiPlanSetNumberVariableValues` and write each entry with `aiPlanSetVariableInt`.

### R2 Fresh area list on resume

The system MUST recompute the area list (BFS + heat-map filter + assignment) whenever a plan transitions out of a parked or paused state back to active, so stale consumed areas do not strand the scout.

### R3 Progress tracking

The system SHALL read `cExplorePlanExploreAreaIDsCurrentIndex` to know which area the engine is currently consuming, and SHALL log assigned area IDs via `aiEcho` for manual verification.

### R4 Assignment ordering

Area selection MUST follow the user-decided ordering: `autoScout_findNextArea` produces a ranked candidate first; the heat-map filter removes dangerous areas from that ranked list before it is written to `cExplorePlanExploreAreaIDs`.

### R5 Caching (optional)

The system MAY cache a filtered area list for up to one tick to reduce repeated `kbAreaGetPercentExplored` calls, but any cache MUST be invalidated on park/un-park, unit death, or heat-map update.

## Scenarios

### S1 Scout in an unexplored area

- GIVEN a registered scout in area 12 where `kbAreaGetPercentExplored(12) < 1.0`
- WHEN `autoScout_exploreNeighborAreas(unitID, planID)` runs
- THEN it assigns area 12 plus its reachable, unexplored border areas to `cExplorePlanExploreAreaIDs`
- AND the engine issues movement toward the first area ID.

### S2 Multiple scouts in the same area group

- GIVEN two registered scouts and area 17 is already claimed by scout A
- WHEN scout B's BFS scores area 17 highest
- THEN the per-area claim system excludes 17 from scout B's candidate list before writing `cExplorePlanExploreAreaIDs`.

### S3 Area becomes fully explored

- GIVEN `cExplorePlanExploreAreaIDs` contains area 17
- WHEN `kbAreaGetPercentExplored(17) >= 1.0` is detected before the next write
- THEN `autoScout_removeExploredAreas` removes 17 from the list
- AND the engine continues with the remaining area IDs.

### S4 All reachable areas explored

- GIVEN every candidate area returns `kbAreaGetPercentExplored >= 1.0`
- WHEN the assignment helper runs
- THEN it writes an empty area list and logs "no areas to scout".

## XS API constraints

| API | Signature | Source |
| --- | --- | --- |
| `aiPlanSetNumberVariableValues` | `void (int planID, int variableIndex, int numberValues, bool clearCurrentValues)` | `docs/doxygen_retail/aifuncs_8cpp.html:4001-4030` |
| `aiPlanSetVariableInt` | `void (int planID, int variableIndex, int valueIndex, int value)` | `docs/doxygen_retail/aifuncs_8cpp.html:4463-4469` |
| `cExplorePlanExploreAreaIDs` | `const int = 21`, writable array | `docs/MythTRConstants.txt:3352`, `docs/doxygen_retail/aiplans_8cpp.html:360` |
| `cExplorePlanExploreAreaIDsCurrentIndex` | `const int = 22`, read-only | `docs/MythTRConstants.txt:3353`, `docs/doxygen_retail/aiplans_8cpp.html:363` |
| `kbAreaGetPercentExplored` | `float (int areaID)` | `docs/doxygen_retail/kbfuncs_8cpp.html:1516-1522` |

## Out of scope

- Manual `aiTaskMoveUnit` issuance for normal exploration.
- Frontier-walk waypoint sampling.
- Corridor chain pathing.

## Open research

- **[RESEARCH-DEFERRED: RP4]** The engine advances `cExplorePlanExploreAreaIDsCurrentIndex` (RO = 1), so scripts cannot reset it. It is not confirmed whether reassigning `cExplorePlanExploreAreaIDs` resets the engine's consumed progress or whether the index continues from its previous value. The spec assumes a full recompute plus reassign on resume; verify in-game.
