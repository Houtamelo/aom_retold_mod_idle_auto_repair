# Exploration: Base-game AI military-unit scouting vs. intelligent_auto_scout mod (refined)

## Change id
base-ai-military-scout-comparison

## Status
- [x] Phase: explore (refined with vanilla source on 2026-06-18)
- [ ] Next: propose

## Goal
Understand how the enemy civilization AI in Age of Mythology: Retold uses military units (units without the AutoScout ability) to scout the map, and compare that behavior to the player-side `intelligent_auto_scout` mod. Identify the triggers, movement logic, coordination, and edge-case handling on both sides so the team can decide whether the mod should mirror, replace, or ignore the base-game military scouting approach.

## Refinement notes
- Previous report (replaced) used AoModAI as a proxy for the shipped civilization AI. This version reads the actual vanilla files at `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/` directly.
- All claims below cite the real file path and line range.

## Method
- Read the civilization-AI entry point: `chairon.xs` and `core/main.xs`.
- Read the main scouting implementation: `core/exploration.xs` (full file, 2,032 lines), plus helper `core/utilities/utilities.xs:setDefaultExplorePlanTargetUnitTypes`.
- Read rule-activation plumbing: `core/setup.xs:init()`, `core/handlers.xs:ageUpEventHandler`, and `core/bo_system/bo_system_internal_steps.xs:onBOSystemEnd`.
- Read the vanilla player AutoScout source: `human_assist/human_assist.xs`.
- Read the unchanged mod source for comparison: `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` and `human_assist.xs`.
- Skimmed `personalities/*.personality` XML files for scout-related overrides.
- Searched per-culture subdirectories under `core/` for additional `cPlanExplore` logic.

## Findings

### 1. Vanilla civilization AI scouting — what the shipped code actually does

#### Entry / trigger
- `chairon.xs:4` includes `core/main.xs`. `core/main.xs:11-53` runs during the loading screen, then calls `prepareForInit()` which moves control into `core/setup.xs`.
- `core/setup.xs:780-810` (`init()`) enables rule groups by current age:
  - `defaultArchaicRules` for units/scouts available from the start.
  - `defaultClassicalRules` once the AI reaches Classical.
- `core/bo_system/bo_system_internal_steps.xs:581-588` enables the early scouts after the build order ends:
  - `scoutingMonitor` for default land scouting.
  - `kataskoposManager` if Greek.
  - `quimichinSpyScoutingMonitor` if Aztec.
- `core/handlers.xs:107-124` enables `defaultClassicalRules` on age-up, which activates the military and naval scout rules defined in `core/exploration.xs`.
- Global danger threshold is set once in `core/main.xs:44`: `aiSetExploreDangerThreshold(100.0)`.

#### Plan creation
The vanilla civ AI creates `cPlanExplore` plans in multiple specialised rules inside `core/exploration.xs`:

| Rule | Lines | Unit type(s) | Notes |
|------|-------|--------------|-------|
| `scoutingMonitor` | 101-334 | Priest (Egypt); `cUnitTypeHumanSoldier` + Pioneer/Kitsune fallback | Default early land scout plan(s). |
| `armyScoutingMonitor` | 339-454 | Units from `gPrimaryLandDefendPlan`, excluding siege | Classical military-unit scouting groups. |
| `navalScoutingMonitor` | 459-570 | `cUnitTypeLogicalTypeNavalMilitary` | Naval version; aggressive personalities transfer the whole defend plan. |
| `transportScoutingMonitor` | 576-661 | Transport ship | Archaic transport scout; recalled at Age 2. |
| `kataskoposManager` | 666-731 | Kataskopos | Greek starting scout. |
| `hippocampusManager` | 736-785 | Hippocampus | Poseidon naval scout. |
| `pegasusScoutingMonitor` | 790-874 | Pegasus variants | Greek air scouts; one plan per idle Pegasus. |
| `pegasusMaintainMonitor` | 885-934 | — | Maintain/train 1 Pegasus. |
| `ravenManager` | 939-999 | Raven | Odin scout. |
| `quimichinSpyScoutingMonitor` | 1062-1128 | Quimichin Spy | Aztec scout. |
| `startupOracleScoutingMonitor` | 1181-1246 | Oracle | Atlantean Archaic Oracle plan. |
| `oracleMonitor` | 1449-1645 | Oracle | Atlantean Classical+ Oracle placement; direct move commands. |

All of these call `aiPlanCreate(..., cPlanExplore, -1, gExplorationCategoryID)` with priority 50 (or 1 for the transport) and add the relevant unit type(s) explicitly.

#### Target selection
- Area-based targeting is done by setting `cExplorePlanExploreAreaIDs` (local starting surroundings) and/or `cExplorePlanExploreAreaGroupIDs` (other landmasses).
  - `helperExploreStartingSurroundings` (`core/exploration.xs:31-96`) populates the start-area list.
  - `helperExploreOtherIslands` (`core/exploration.xs:13-23`) populates area-group lists on island maps.
- The default explore plans (e.g. `scoutingMonitor`) do **not** call `cExplorePlanDoLoops`; they rely on the engine's area-based exploration.
- Build-order explore steps (`core/bo_system/bo_system_internal_steps.xs:645-688`) **do** enable looping with `cExplorePlanDoLoops=true`, `cExplorePlanNumberOfLoops=3`, and a loop start point at the main TC.
- Army/navy scout plans set:
  - `cExplorePlanAggressiveScouts=true` (`core/exploration.xs:408`, `542`)
  - `cExplorePlanAvoidingAttackedAreas=false` (`core/exploration.xs:409`, `547`)
  - target unit types to enemy soldiers via `setDefaultExplorePlanTargetUnitTypes` (`core/exploration.xs:410`, `545`; implementation at `core/utilities/utilities.xs:715-725`).

#### Coordination
- Multiple plans are allowed only where explicitly coded:
  - `scoutingMonitor` normally maintains 1 plan, but `cPersonalityRetaliator` maintains 3 (`core/exploration.xs:155-159`).
  - `armyScoutingMonitor` creates many small groups (up to `selectByDifficulty(1, 2, 4, 8, 10, 12)` groups) (`core/exploration.xs:398`).
- There is **no per-area claim system**. Plans can independently target the same region.
- Army scout plans use `cExplorePlanMasterExploreAreasPlan` (`core/exploration.xs:380`, `426`) to link sub-plans to a master plan. The exact engine semantics are not exposed, but it appears intended to share area progress rather than to claim areas.
- `cPlanFlagNoMoreUnits` is set true on most plans, so units are manually assigned and not stolen/backfilled automatically.
- Some myth/flying scouts set `cPlanFlagCantBeStolenFrom=true` (`core/exploration.xs:716`, `849`, `1105`).

#### Military-unit fallback (the user's original question)
**YES.** The shipped civilization AI explicitly puts non-AutoScout military units into `cPlanExplore` plans.
- `scoutingMonitor` (`core/exploration.xs:209-279`) pulls `cUnitTypeHumanSoldier` units out of the primary land defend plan and assigns them to the default explore plan, filtering by cost (`kbAICostGetProtoUnitCost(protoUnitID) < 120.0`) at line 265.
- `armyScoutingMonitor` (`core/exploration.xs:339-454`) is dedicated to this: whenever `gAttackManager.mScoutingState == cScoutingForEnemies`, it creates groups of `cPlanExplore` plans from the primary land defend plan, excluding siege weapons (`core/exploration.xs:393-395`).
- `navalScoutingMonitor` (`core/exploration.xs:459-570`) does the same for naval military units.
- This is not a manual `aiTaskMoveUnit` fallback; it is regular `cPlanExplore` plan creation staffed with ordinary military units.

#### Edge cases
- **Danger**: default explore plans inherit the global `aiSetExploreDangerThreshold(100.0)` set in `core/main.xs:44`; army/naval scouts explicitly disable area-danger avoidance (`cExplorePlanAvoidingAttackedAreas=false`) so they scout aggressively. `oracleMonitor` checks `kbAreaGetDangerLevel(areaID, false) > 100.0` before placing or repositioning Oracles (`core/exploration.xs:1563`, `1588`).
- **Stuck handling**: very limited. The transport scout is explicitly moved home at Age 2 with `aiTaskMoveUnit` (`core/exploration.xs:621-625`). Oracles are re-tasked with `aiTaskMoveUnit` if they drift from their assigned area (`core/exploration.xs:1617-1644`). There is no generic stuck handler for the default land/air scouts.
- **Herdables**: no explicit logic. Conversion happens only if a scout happens to walk past a herd.
- **LOS outposts**: Egyptian default explore plans set `cExplorePlanCanBuildOutpost=true` and `cExplorePlanOutpostPUID=cUnitTypeObelisk` (`core/exploration.xs:302-303`).
- **Naval vs land vs air split**: land rules use no naval flag; naval rules set `cExplorePlanNaval=true` and `cExplorePlanWaterAreaGroupID`; flying scouts (Pegasus, Ravens) rely on no area restrictions.
- **KOTH**: if the AI is not holding King of the Hill, army scouting is bypassed and all army scouts are sent to attack the KOTH point (`core/exploration.xs:363-367`).

#### Per-personality
- The `personalities/*.personality` files are metadata only (name strings, cities, image paths) and contain no scout parameters.
- Personality-specific effects are hard-coded in `core/exploration.xs`:
  - `cPersonalityRetaliator`: 3 default land scout plans instead of 1 (`core/exploration.xs:155-159`). Army scouting is disabled for Retaliator (`core/exploration.xs:356-360`).
  - `cPersonalityPassive`: army scouting disabled (`core/exploration.xs:356-360`).
  - `cPersonalityHumanoid`: most myth/unit scouts disabled (`core/exploration.xs:109`, `742`, `795`, etc.) and `oracleMonitor` exits immediately (`core/exploration.xs:1454`).

#### Per-civ
- Culture-specific scouting is all inside `core/exploration.xs` (not in per-civ subdirectories):
  - Greek: Kataskopos + Pegasus air scout + Hippocampus for Poseidon.
  - Egyptian: Priest scout + Obelisk build option.
  - Norse/Odin: Ravens.
  - Atlantean: Oracle startup plan + Oracle favor-placement monitor.
  - Aztec: Quimichin Spy.
  - Chinese: Pioneer fallback, Sky Lantern power (`castSkyLantern`, `core/exploration.xs:1004-1057`).
  - Japanese: Kitsune fallback.
- No additional `cPlanExplore` logic was found in the `core/<culture>/` directories.

### 2. Vanilla player-side AutoScout — `human_assist/human_assist.xs`

This is the file the mod overlays.

- **Hook / trigger**: `enableAutoScouting(int unitID)` is called directly by the UI AutoScout button, and `disableAutoScusting(int unitID)` is called by the Cancel button (`human_assist/human_assist.xs:77-111`).
- **Plan / command surface**: vanilla `enableAutoScouting` creates one `cPlanExplore` per unit:
  - `aiPlanCreate("Autoscout with unit: " + unitID, cPlanExplore)` (`human_assist/human_assist.xs:99`)
  - `aiPlanAddUnitType(planID, cUnitTypeUnit, 1,1,1)` and `aiPlanAddUnit(planID, unitID)` (`human_assist/human_assist.xs:100-101`)
  - `cPlanFlagNoMoreUnits` and `cPlanFlagRequiresAllNeedUnits` are set (`human_assist/human_assist.xs:108-109`).
- **Target selection**: the vanilla player AutoScout delegates entirely to the engine. It sets:
  - For Oracles only: `cExplorePlanDoLoops=false` and `cExplorePlanStopLOSPercentage=0.2` (`human_assist/human_assist.xs:103-107`).
  - No other `cExplorePlan*` variables are configured.
- **Coordination**: none. Each enabled unit gets its own independent `cPlanExplore`; there is no area-claim, no multi-scout spread logic, and no herd delivery.
- **Danger handling**: none. `human_assist.xs` never calls `aiSetExploreDangerThreshold` and does not set `cExplorePlanAvoidingAttackedAreas`.
- **Edge cases**: `cleanupLingeringExplorePlans` (`human_assist/human_assist.xs:117-130`) destroys empty plans every 15 seconds. Player override works because destroying the plan drop-kicks the unit from AutoScout; the mod piggybacks on this same behaviour.

### 3. intelligent_auto_scout mod recap

The mod behaviour is unchanged from the previous report; it now contrasts with **both** the civ AI and the vanilla player AutoScout.

- **Hook point**: `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:109` calls `autoScout_register(planID, unitID)` from the vanilla `enableAutoScouting` overlay.
- **Register**: `autoScout_register` (`auto_scout.xs:2898-2936`) immediately parks the `cPlanExplore` in state `cPlanStateIdle` (23) so the engine plan becomes a UI/housekeeping marker (`auto_scout.xs:2910`). It only runs for `kbPlayerIsHuman(cMyID)` (`auto_scout.xs:2902`).
- **Tick cadence**:
  - `rule autoScout_tickHeavy minInterval 1` (`auto_scout.xs:3020-3029`) rebuilds the heat-map and runs `autoScout_homeMoveScan`.
  - `rule autoScout_tickFast minInterval 1` (`auto_scout.xs:3031-3049`) advances each registered scout through its state machine.
- **Coverage algorithm**: custom BFS over the area-adjacency graph, scored by TC proximity, scout proximity, friendly-scout density, danger, and Oracle-overlap. Once in an area, it samples 5 radii × 8 angles for frontier waypoints.
- **Coordination**: per-area claims (`gAutoScout_areaClaim`), self-scouted flags, density scoring, and Oracle-overlap penalties.
- **Danger**: hand-built per-tick heat-map from visible enemy military units and buildings, with persistent damage/death events. Scouts flee to safe areas and black-list dangerous areas for 90 seconds.
- **Herdables**: explicit divert state (one attempt per herd globally) and `autoScout_homeMoveScan` routes newly-owned herdables to the nearest TC (`auto_scout.xs:2967-2999`).
- **Oracle handling**: separate state machine that waits for action type 37 (saturated LOS) before repicking.
- **Player override**: plan destruction or any manual command drops the scout from the pool on the next tick.

### 4. Side-by-side comparison (three columns)

| Dimension | Vanilla civ AI | Vanilla player AutoScout | intelligent_auto_scout mod |
| --- | --- | --- | --- |
| Trigger | Rule groups enabled in `setup.xs:init()` and `handlers.xs:ageUpEventHandler`; early scouts enabled after BO in `bo_system_internal_steps.xs:581-588`. | UI button calls `enableAutoScouting(unitID)` in `human_assist/human_assist.xs:94-111`. | Same UI hook as vanilla player AutoScout, then `autoScout_register` parks the plan (`auto_scout.xs:2898-2910`). |
| Target selection | Engine plan driven by area IDs / area-group IDs; BO steps use spiral loops. Army/naval scouts are set aggressive with hand/ranged target types. | Pure engine default; only Oracles get `DoLoops=false` and `StopLOSPercentage=0.2`. | Custom BFS + scoring + in-area frontier walk. |
| Movement command | Plan-driven; Oracles and transport also receive direct `aiTaskMoveUnit`. | Plan-driven. | Explicit `aiTaskMoveUnit` along BFS-safe corridors and to frontier/herd/flee waypoints. |
| Coordination | One plan per unit; no area claims; army plans link to a master explore plan; Retaliator runs 3 default plans. | None — one independent plan per toggled unit. | Per-area claims, density scoring, Oracle overlap, self-scouted flags. |
| Danger handling | Global `aiSetExploreDangerThreshold(100.0)` (`main.xs:44`); army/naval scouts disable danger avoidance. | None. | Custom heat-map + flee state + 90 s blacklists. |
| Herdables | Incidental walk-past conversion only. | Incidental walk-past conversion only. | Explicit divert and automatic home-move to nearest TC. |
| Oracle vs regular scout | Oracles get `StopLOSPercentage=0.2` and a separate Classical `oracleMonitor` that parks them in safe areas. | Oracles get `StopLOSPercentage=0.2`; no safe-area logic. | Separate Oracle state machine waits for saturated action (37), coordinates LOS rings. |
| Player override | N/A (AI). | Destroying the plan stops the unit. | Same + state-machine drops scout from pool on next tick. |
| Perf overhead | One rule run per 10 s for each scout rule; otherwise plan is engine-internal. | One plan per unit; engine handles movement. | Two `minInterval 1` rules, heat-map rebuild, BFS, and frontier sampling per scout. |
| Military reuse | **Yes** — `scoutingMonitor`, `armyScoutingMonitor`, and `navalScoutingMonitor` all staff `cPlanExplore` with ordinary military units. | N/A (player-only; only the selected unit). | N/A — scoped to `AbstractScout`/Oracle units only. |

### 5. Concrete behavioral deltas

- **Coverage shape**: vanilla engine plans can overlap and wander; the mod deliberately spreads scouts across the frontier.
- **Scout safety**: the mod actively models DPS, range, multi-projectile buildings, and path-aware danger; the civ AI relies on a coarser global danger threshold and army scouts ignore danger entirely.
- **Herd economy**: the mod converts and delivers herdables automatically; vanilla systems rely on chance contact.
- **Military scouting**: the enemy AI visibly reroutes idle soldiers to scouting; the player mod does not (and vanilla player AutoScout cannot).
- **Oracle precision**: the mod detects LOS saturation empirically; vanilla systems use a fixed 20 % stop-LOS and do not coordinate multiple Oracles.
- **Responsiveness to player commands**: both vanilla systems hand the unit back when the plan is destroyed; the mod adds one-tick pool cleanup so the unit stops moving faster after manual override.

### 6. Recommendations (revised)

- **Should the mod extend to idle military units as scouts?**
  - **No by default.** The shipped civ AI does this (`scoutingMonitor`, `armyScoutingMonitor`, `navalScoutingMonitor` all put non-AutoScout military units into `cPlanExplore`), so the behavioral precedent is real. However, a player-assist mod that automatically grabs idle soldiers and turns them into scouts would interfere with combat micro, retreats, and garrisoning. Keep the mod scoped to `AbstractScout`/Oracle units.
  - If the team wants to offer an opt-in **"idle military auto-scout"** feature later, the civ-AI logic shows the viable pattern: create `cPlanExplore` plans, add `cUnitTypeHumanSoldier`/naval military units from the defend/idle pool, set `cExplorePlanAggressiveScouts=true` and `cExplorePlanAvoidingAttackedAreas=false`, and link sub-plans to a master plan. That work should be a separate proposal with its own UX toggle and safety rules.
- **Mirror-worthy civ-AI behaviors for the existing mod**:
  - Egyptian Obelisk/outpost building (vanilla `cExplorePlanCanBuildOutpost` + `cExplorePlanOutpostPUID`) could inspire an optional stationary-scout placement for Oracles or priests, but the mod's Oracle LOS-ring coordination already covers most of the same value.
  - The civ AI's "starting surroundings" area list (`helperExploreStartingSurroundings`) is conceptually similar to the mod's TC-proximity BFS; no change needed.
- **Keep the current mod architecture**: parking the engine plan in `cPlanStateIdle` is still the cleanest way to keep the UI state while replacing movement logic.

## Open questions
- What does the engine do with `cExplorePlanMasterExploreAreasPlan` in practice? The variable links army scout sub-plans to a master, but the exact sharing/avoidance semantics are opaque.
- Does the engine's `cPlanExplore` area-based mode also honor `aiSetExploreDangerThreshold(100.0)` when `cExplorePlanAvoidingAttackedAreas` is left at its default? If so, how does it compare quantitatively to the mod's hand-tuned threshold?
- How long can an army scout plan stay in `cPlanStateAttack`/`cPlanStateTransport` before `armyScoutingMonitor` cleans it up? This affects whether "scouting" military units can get stuck fighting instead of exploring.
- Would extending the mod to idle military units require scanning the reserve/defend plans, and what would the per-tick cost be on large armies?

## Artifacts cited (vanilla)
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/chairon.xs` — entry point (`chairon.xs:4`).
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/main.xs` — `aiSetExploreDangerThreshold(100.0)` at `main.xs:44` and global init flow.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/setup.xs` — `init()` enabling rule groups (`setup.xs:787-810`).
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/handlers.xs` — age-up rule-group activation (`handlers.xs:107-124`).
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/bo_system/bo_system_internal_steps.xs` — enabling early scout rules after the BO (`bo_system_internal_steps.xs:581-588`).
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/exploration.xs` — all main scouting rules and logic.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/utilities/utilities.xs` — `setDefaultExplorePlanTargetUnitTypes` (`utilities.xs:715-725`).
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/human_assist/human_assist.xs` — vanilla player AutoScout.
- `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs` — mod logic.
- `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs` — mod overlay hook.

## Risks / unknowns
- Some `cExplorePlan*` variables (especially `cExplorePlanMasterExploreAreasPlan`) are set by the shipped AI but their internal engine behaviour is not fully documented.
- Oracle placement in the shipped AI uses `kbAreaGetDangerLevel(..., false) > 100.0`; the mod uses its own heat-map threshold, so direct numeric comparisons are approximate.
- The "army scouts ignore danger" behaviour of the civ AI is intentional aggression; copying it for a player-assist feature could frustrate players if their units are killed while auto-scouting.
- No automated tests exist (`strict_tdd: false`); any future decision to mirror civ-AI military scouting would need manual in-game verification.
