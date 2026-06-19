# Tasks: Port AoModAi layered walls into Extra Ai + AoModAi

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated total changed lines | ~150–250 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR (state globals → init rule → helpers → monitor → markers) |
| Delivery strategy | auto-chain |
| Chain strategy | n/a (single PR) |
| Decision needed before apply | No |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

## Phase 1: State globals + audit markers

- [x] 1.1 In `mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs`, add the state globals after `int startTimeNoExtraBuildingsNeeded = cMaxInt;` (line 22): `bool mRusher = false;`, `int gSecondRingWallPlanID = -1;`, `int gSecondRingWallStartTime = -1;`, `int gSecondRingWallLastDestroyedTime = -1;`, `int gSecondRingAttackStartTime = -1;`, and `bool gDebugSecondRing = true;`.
Why: these hold the rusher flag, the active second-ring plan, its lifetime timestamps, the sustained-attack timestamp (used by `isBaseUnderSustainedAttack`), and a debug toggle.
- [x] 1.2 Wrap the globals added in 1.1 in `// === AoModAi: layered walls state begin ===` and `// === AoModAi: layered walls state end ===` markers.

## Phase 2: Init rule

- [x] 2.1 Add `rule secondRingWallInit` (group `defaultArchaicRules`, inactive, `minInterval 1`) immediately before `militaryBuildingManager` at line 26. It sets `mRusher = (cPersonalityCurrent == cPersonalityAttacker);` and calls `xsDisableRule("secondRingWallInit");`.
Why: AoModAi uses `mRusher` to delay wall-building for rushers; this rule captures the personality once at startup.

## Phase 3: Helper functions

- [x] 3.1 Insert `int createSecondRingWallPlan(int baseID)` after the `wallManager` rule (end line 1058). It creates a `cPlanBuildWall` plan of type `cBuildWallPlanWallTypeRing`, centers it on `kbBaseGetLocation(baseID)`, sets radius `50`, adds the correct culture builder unit type, sets priority 51, and records `gSecondRingWallStartTime`.
Why: `wallManager` skips bases that already have a wall plan, so the second ring must be created by a separate helper.
- [x] 3.2 Insert `void destroySecondRingWallPlan(string reason)` next to the creator. If `gSecondRingWallPlanID` is valid, destroy the plan, reset the plan/start globals, and record `gSecondRingWallLastDestroyedTime`.
- [x] 3.3 Insert `bool isBaseUnderSustainedAttack(int baseID)` that reads `gDefendTCBases.find(baseID)` and `gEnemyPowerInBases[index]`, and tracks `gSecondRingAttackStartTime` as a static timestamp. Returns true when `gEnemyPowerInBases[index] > 0` continuously for more than 25 s. Returns false (and resets the timestamp) when no attack, when the base is not in `gDefendTCBases`, or when the index lookup fails.
Why: reuses Retold's pre-computed enemy-power signal (the same one used by `military_defend.xs`, `godpowers_*.xs`, `chats.xs`) instead of replicating the unit-count query. Avoids the need for `getEnemyUnitsNearBase` and `getOwnAlliedUnitsNearBase` helpers.

## Phase 4: Main polled rule

- [x] 4.1 Add `rule secondRingWallPlanMonitor` (group `defaultClassicalRules`, inactive, `minInterval 10`) right after the helper functions. Early-return and destroy any existing second-ring plan when `checkStrategyFlag(cStrategyFlagBuildWalls)` is false.
- [x] 4.2 Implement rusher gating: if `mRusher == true`, skip new plan creation unless `kbGetAge() >= cAge3` and `xsGetTime() >= 15 * 60 * 1000`.
- [x] 4.3 Implement creation gating: if `gSecondRingWallPlanID` is invalid, only create a plan when a main-base wall plan exists, age is at least 2, time is at least 8 min, villagers are at least 10, and gold is at least 150. Otherwise destroy the plan if one exists and gating fails.
- [x] 4.4 Implement attack cancellation: if a plan exists and `isBaseUnderSustainedAttack(mainBaseID)` is true **AND** `xsGetTime() > 19*60*1000` (the AoModAi early-game exemption — before 19 min, walls keep building even under attack, otherwise a minor early push would prevent the wall from ever being built), call `destroySecondRingWallPlan("sustained attack")`. Recreate once the base is safe and creation gating still holds.
- [x] 4.5 Implement 12-minute lifetime: if `gSecondRingWallStartTime != -1` and `xsGetTime() - gSecondRingWallStartTime > 12 * 60 * 1000`, call `destroySecondRingWallPlan()`.

## Phase 5: Audit markers + CRLF preservation

- [x] 5.1 Wrap the `secondRingWallInit` rule, helper functions, and `secondRingWallPlanMonitor` rule inside `// === AoModAi: layered walls begin ===` and `// === AoModAi: layered walls end ===` markers.
- [x] 5.2 Save `buildings.xs` with CRLF line endings and verify the inserted block does not contain stray LF-only lines (check a hex dump or a Python line-ending script).

## Phase 6: Deployment + verification

- [x] 6.1 Run `bash scripts/deploy-mods.sh` to deploy the updated `Extra Ai + AoModAi` mod file.
- [ ] 6.2 Start a match and inspect the AI log for XS parse errors; confirm `secondRingWallPlanMonitor` is active.
- [ ] 6.3 USER: play and validate spec scenarios B1–B12 manually in-game.

## Open questions

| Question | Default | How to override |
|----------|---------|-----------------|
| Gating thresholds for the 2nd ring | Age >= 2, time >= 8 min, villagers >= 10, gold >= 150 | Edit literals in `secondRingWallPlanMonitor` (Phase 4.3). |
| 2nd-ring radius | 50 | Edit the radius literal in `createSecondRingWallPlan` (Phase 3.1). |
| Personality mapping for `mRusher` | `cPersonalityCurrent == cPersonalityAttacker` | Change the expression in `secondRingWallInit` (Phase 2.1), e.g. also include `cPersonalityConqueror`. |

## Rollback checklist

- [ ] Delete the `// === AoModAi: layered walls state ... ===` block and the `// === AoModAi: layered walls begin ... end ===` block from `buildings.xs`.
- [ ] Remove the modified file from the deployed mod directory if it was already copied.
