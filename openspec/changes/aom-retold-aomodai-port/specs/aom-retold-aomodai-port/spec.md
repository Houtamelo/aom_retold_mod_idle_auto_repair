# Layered Walls (AoModAi port) Specification

## Purpose

Port AoModAi layered walls into `Extra Ai + AoModAi`: default AIs add a second main-base ring, rushers delay walls until Age 3 + 15 min, and the second ring cancels under heavy attack.

## Requirements

| ID | Requirement | Notes |
|---|---|---|
| R1 | Add polled rule `secondRingWallPlanMonitor` in `core/buildings/buildings.xs` and enable it. | `xsEnableRule`. |
| R2 | Confirm a main-base wall plan exists before creating the 2nd ring. | Avoid standalone outer ring; `kbBaseGetMainID`, `aiPlanGetNumber`, `aiPlanGetBaseID`. |
| R3 | Create a 2nd `cPlanBuildWall` plan (`cBuildWallPlanWallTypeRing`, center `kbBaseGetLocation(mainBaseID)`, radius **50**) when Age >= 2, time >= 8 min, villagers >= 10, gold >= 150. | Mirrors AoModAi defaults; `aiPlanCreate`, `kbGetAge`, `xsGetTime`, `kbUnitCount`, `kbResourceGet`. |
| R4 | When `mRusher` is true, suppress new wall plans until `kbGetAge() >= cAge3` and `xsGetTime() >= 15 * 60 * 1000`. | `mRusher` (define if absent), `kbGetAge`, `xsGetTime`. |
| R5 | Destroy 2nd-ring plan when the main base has been under attack for more than 25 s. Attack detection reads Retold's `gEnemyPowerInBases[gDefendTCBases.find(mainBaseID)]` (extern arrays, populated by `military_defend.xs` every defense frame); a static `gSecondRingAttackStartTime` tracks the sustained duration. | No new unit query needed; reuses Retold's pre-computed enemy power. The 2x-overwhelmed check from AoModAi is deferred (would need a power-ratio idiom, future enhancement). |
| R6 | Destroy 2nd-ring plan alive > 12 minutes. | Static timestamp, `xsGetTime`. |
| R7 | Recreate 2nd-ring plan after cancellation/timeout when gating conditions hold. | Polled re-evaluation. |
| R8 | Leave 1st-ring unchanged (`mWallCircleAmount = 1`; `wallManager` at `core/buildings/buildings.xs:933`). | No strategy-file edits. |
| R9 | Wrap inserted code in `// === AoModAi: layered walls begin ===` and `// === AoModAi: layered walls end ===`. | Rollback markers. |
| R10 | Preserve CRLF line endings in `buildings.xs`. | Save with CRLF. |

> **OPEN:** Gating thresholds use AoModAi defaults (Age 2+, 8 min, >= 10 villagers, >= 150 gold). May be overridden for shorter playtests.
>
> **OPEN:** 2nd-ring radius uses 50, matching Retold's natural radius. May be overridden.

## Scenarios

- **B1 — Default AI 2nd ring** **MANUAL**: GIVEN non-rusher AI with 1st ring; WHEN Age >= 2, time >= 8 min, villagers >= 10, gold >= 150; THEN second ring at radius 50.
- **B2 — 2nd ring destroyed under sustained attack** **MANUAL**: GIVEN 2nd-ring plan AND game time > 19 min; WHEN `gEnemyPowerInBases[mainBaseIndex] > 0` continuously for > 25 s; THEN plan destroyed. (Before 19 min, walls keep building even under attack — AoModAi's early-game exemption. We also no longer check the 2x enemy-vs-own ratio — that was the unit-count version; Retold's power-based signal is a single value.)
- **B3 — 2nd ring recreated after attack** **MANUAL**: GIVEN 2nd-ring plan destroyed; WHEN base safe and gating holds; THEN new plan created.
- **B4 — Rusher AI delays walls** **MANUAL**: GIVEN AI with `mRusher` true; WHEN time < 15 min or age < cAge3; THEN no new wall rings.
- **B5 — Non-rusher AI builds walls normally** **MANUAL**: GIVEN AI with `mRusher` false; WHEN gating holds; THEN 1st and 2nd rings build.
- **B6 — 12-minute lifetime cap** **MANUAL**: GIVEN 2nd-ring plan existed 12 minutes; WHEN rule detects timeout; THEN plan destroyed.
- **B7 — No 2nd ring without 1st ring** **MANUAL**: GIVEN no main-base wall plan; WHEN 2nd-ring rule runs; THEN no plan created.
- **B8 — Civ-specific gating works** **MANUAL**: GIVEN Norse/Atlantean AI; WHEN builder counts evaluated; THEN walls match civ builder logic.
- **B9 — 2nd ring delayed until 8 minutes** **MANUAL**: GIVEN non-rusher AI meets other gating; WHEN game time < 8 min; THEN no 2nd-ring plan.
- **B10 — Gold drop cancels plan** **MANUAL**: GIVEN 2nd-ring plan with gold >= 150; WHEN gold drops below threshold; THEN plan destroyed.
- **B11 — No 1st-ring regression** **MANUAL**: GIVEN default strategy `mWallCircleAmount = 1`; WHEN only 1st ring expected; THEN radius, priority, gates, timing unchanged.
- **B12 — XS loads cleanly** **MANUAL**: GIVEN modified `buildings.xs` deployed; WHEN match starts; THEN no parse errors, new rule active.

## Out of scope

- Forward firebases / siege support.
- GP combo.
- Tower/fort recycling.
- Per-secondary-base ring walls.
- Ally base wall rings.
- Modifying 1st-ring parameters (`mWallCircleAmount = 1`).
- Playtest cadence / test plan.
