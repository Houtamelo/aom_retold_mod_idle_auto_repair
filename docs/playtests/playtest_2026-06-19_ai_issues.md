# Playtest 2026-06-19: AI Issues Catalog

**Purpose:** Catalog of AI behavior issues found in the 2026-06-19 playtest, organized for parallel-session investigation. Each issue is self-contained — a fresh agent session with no prior context should be able to pick up ISSUE-XX and start investigating.

## Playtest context

- **Map**: midgard (water + fish)
- **AI player**: Player 2, Norse, personality **Balanced** (not Attacker), Legendary difficulty
- **Human player**: Player 1 (you), Loki, full Hersir + random myth units composition
- **Mod in use**: `Extra Ai + AoModAi` (post-wall-port, commit `3765cf7` + 3 fix-up commits)
- **Outcome**: AI resigned at 00:21:00 (healthScore 11.20). 236 units + 23 buildings lost.
  - AutoBase 11 was overrun at ~00:19:04 by ~135 enemy power (AI had 9 power).
  - AI never recovered from `gDefensivelyOverrun = true`.
- **Resources**: Standard starting resources (NOT infinite, despite the AI's repeated false message).
- **AI log file** (raw, UTF-16LE): `.tmp/MythRetoldAIOutputPlayer2.txt`
- **AI log file** (UTF-8): `.tmp/MythRetoldAIOutputPlayer2.utf8.txt` — use this one for `grep`.
- **Pre-split chunks** (8 chunks of 2768 lines each): `.tmp/chunk_00` through `.tmp/chunk_07`.

## Project principle (applies to every issue below)

**Every AI behavior issue is the mod's responsibility, even if the bug originates in the base Retold AI code.** The mod's purpose is "smarter AI". Never deflect with "this is a base AI bug, not mod-related" — if the AI does something dumb in playtests, fix it in our overlay.

## Mod's current overlay scope (22 files)

When investigating an issue, first check whether the implicated file is already overlaid. If yes, the fix edits the existing overlay. If no, the fix adds a new overlay file (the `deploy_tree()` helper in `scripts/deploy-mods.sh` handles arbitrary new files automatically).

```
mod/Extra Ai + AoModAi/game/ai/
├── core/atlantean/atlantean_archaic.xs
├── core/bo_system/bo_system.xs
├── core/bo_system/bo_system_internal.xs
├── core/buildings/buildings.xs
├── core/buildings/buildings_economic.xs
├── core/buildings/dropsite_placement.xs
├── core/economy/economic_units.xs
├── core/economy/resource_breakdown_system.xs
├── core/exploration.xs
├── core/godpowers/godpowers.xs
├── core/godpowers/godpowers_chinese.xs
├── core/godpowers/godpowers_japanese.xs
├── core/military/military_attack.xs
├── core/military/military_units.xs
├── core/military/naval_military.xs
├── core/shared/heroic/heroic_default_strategy.xs
├── core/shared/heroic/heroic_turtler_strategy.xs
├── core/shared/mythic/mythic_default_strategy.xs
├── core/shared/mythic/mythic_turtler_strategy.xs
├── core/shared/wonder/wonder_default_strategy.xs
├── core/startup/game_settings_analysis.xs
└── core/techs.xs
```

Files NOT overlaid (behavior here is purely base Retold): `core/military/military_defend.xs`, `core/military/naval_defend.xs`, `core/strategy/strategy_internal.xs`, `core/utilities/utilities.xs`, `core/godpowers/godpowers_utility.xs`, `core/godpowers/godpowers_egyptian.xs`, `core/godpowers/godpowers_greek.xs`, `core/godpowers/godpowers_norse.xs`, `core/godpowers/godpowers_aztec.xs`, `core/buildings/buildings_economic.xs` (overlaid), `core/economy/economy.xs`, all `core/shared/classical/*`, all `core/scenario/*`, all `core/campaign/*`, all `core/setup/*` (except `game_settings_analysis.xs`), `core/chats.xs`, etc.

**Important context**: `core/techs.xs` (overlaid) has been verified to have `haveForcedMilitaryTechnologyToResearch` byte-identical to the base. Most of our 22 files are stale versions of the base AI from Garbhus's classic AoM mod 314139 — Retold has patched the base since, so diffs against current base are likely to reveal the cause of research-related and other regressions.

## Index of issues

| ID       | Title                                                          | Severity        | Likely cause location                    |
| -------- | -------------------------------------------------------------- | --------------- | ---------------------------------------- |
| ISSUE-01 | Research never works (forced-tech trap) — **FIXED** in `1512b3b` | HIGH            | `core/techs.xs::haveForcedEconomicTechnologyToResearch` (15 broken force-research branches writing to `gMilitaryResearchPlanID` instead of the `ref int techID` output param) |
| ISSUE-02 | Second-ring wall rule produces zero output                    | HIGH (for wall feature) | `buildings.xs` (rule's gate diagnostic)  |
| ISSUE-03 | GreatHunt and resource god powers skipped on standard games — **FIXED** in `bbb83b7` | MEDIUM          | `godpowers.xs::isUnneededGodPowerDueToResources` (inverted `==` on `cStartingResourcesInfinite` flipped to `!=`) |
| ISSUE-04 | WalkingWoods never used defensively                           | MEDIUM          | Likely in `godpowers_norse.xs` (NOT overlaid) — would need new overlay |
| ISSUE-05 | AI makes suicide attacks (into unwinnable positions)          | HIGH (tactical) | `military_attack.xs` (overlaid) or `military_defend.xs` (NOT overlaid) — needs retreat logic |
| ISSUE-06 | AI attacks while leaving home undefended                        | HIGH (tactical) | `military_attack.xs` (overlaid) and/or `military_defend.xs` (NOT overlaid) |
| ISSUE-07 | Building placement failures cascade (dropsites, farms, towers, docks) | LOW-MEDIUM   | Various placement rules in `buildings*.xs` (overlaid) and base-game placement helpers |
| ISSUE-08 | Ox Cart pathing permanently stuck                              | LOW-MEDIUM      | Likely in `economy.xs` or `dropsite_placement.xs` (overlaid) |
| ISSUE-09 | `gDefensivelyOverrun` lock never clears                         | HIGH            | `military_defend.xs` (NOT overlaid) — needs recovery logic |
| ISSUE-10 | Wrong-civ myth unit fallback (Mountain Giant for Norse)        | LOW             | `military_units.xs` (overlaid) or `godpowers_japanese.xs` (overlaid) — `mythMilitaryTraining` rule |
| ISSUE-11 | Trade economy collapses on Market destruction                  | LOW             | `economy.xs` (NOT overlaid) — needs Market-loss resilience |

---

## ISSUE-01: Research never works (forced-tech trap)

**Status: ✅ FIXED** in commit `1512b3b` (2026-06-19). Fix removes 15 broken force-research branches from `haveForcedEconomicTechnologyToResearch` in `core/techs.xs` (mod lines 1373-1462). User will verify via next playtest.

**Severity**: HIGH — the AI fights every game with base-tech units. No Ballistics, no WatchTower, no StoneWall, no FortifiedTownCenter, no Armory upgrades. This is a massive military handicap.

### Symptom (with log evidence)

Every 30 seconds from 00:05:42 to the end of the game (00:21:00), the log shows:

```
00:06:12  (372282): --- Running Rule militaryUpgradeManager. ---
00:06:12  (372282): We had a research plan that didn't go into state research for 2 minutes, destroying it. Plan name: 38: Research Plan: Ballistics.
00:06:12  (372282): Created a Research Plan for: Ballistics with plan number: 59, priority: 50.
00:06:12  (372282): Not running through the default logic because we have a forced technology to research.
```

The pattern repeats for the entire game (~30 destroy/recreate cycles for Ballistics alone). No research plan ever reaches "researching" state. Zero matches in the log for `Research.*complet` or `complet.*research`.

### Verified facts

- **Armory WAS built** (around 00:08:12). The 4-min Classical timer gated it, but after that the Armory existed.
- **WatchTower plan #85** was created at 00:08:12 (priority 51), then reported "We have a functional plan in: 85: WatchTower" at 00:09:12 — but **NEVER enters researching state** and never completes. The misleading message "functional" just means "plan exists, I'm going to quit."
- **Ballistics plan** was created with priority 50 (lower than WatchTower's 51) — it loses the Armory's research slot every cycle.
- **Base Retold AI (no mod) researches fine** — user confirmed.
- **Our mod's commit `412a53c` (pre-wall-port) ALREADY had this bug** — user playtested and reported.
- **Wall port (commit `fa561e9`+) only modified `buildings.xs`** — NOT the cause.

### Root cause (found 2026-06-19)

The bug is in `core/techs.xs::haveForcedEconomicTechnologyToResearch` (mod lines 1373-1462). Our overlay from Garbhus's classic AoM mod 314139 added **15 broken "force-research" branches** like:

```xs
if (kbTechGetStatus(cTechBallistics) == cTechStatusObtainable)
{
   gMilitaryResearchPlanID = researchSimpleTech(cTechBallistics);  // WRONG: writes to MILITARY var
   return true;                                                     // WRONG: doesn't set techID output param
}
```

Two defects in every branch:

1. **Wrong global variable**: writes to `gMilitaryResearchPlanID` (the MILITARY manager's tracked plan ID), but this is the ECONOMIC picker function. The function's contract says: set `techID` (output `ref` parameter) and return `true` — let the caller `economyUpgradeManager` call `researchSimpleTech(techID)` and store the result in `gEconomyResearchPlanID`.
2. **Bypasses the caller's tracking logic**: the caller also attaches an event handler (`aiPlanSetEventHandler(gEconomyResearchPlanID, cPlanEventStateChange, "resetEconomyResearchPlan")`) which is what clears the `gAreResearchingForcedEconomicUpgrade` flag (and the equivalent military path) when research completes/fails. By writing directly to `gMilitaryResearchPlanID`, the broken branches create an untracked plan — `resetMilitaryResearchPlan` never fires for it.

**How this causes the infinite loop**:

1. `economyUpgradeManager` runs every 30s → calls `haveForcedEconomicTechnologyToResearch(techID)`.
2. Mod's broken branch fires for `cTechBallistics` (always obtainable once Armory exists) → creates Ballistics plan, writes plan ID to `gMilitaryResearchPlanID`, returns `true` WITHOUT setting `techID`.
3. Caller does `gEconomyResearchPlanID = researchSimpleTech(techID=-1)` — creates an INVALID plan for tech ID -1.
4. Caller sets `gAreResearchingForcedEconomicUpgrade = true` (stuck forever — no event handler fires for the bogus plan).
5. On the MILITARY side (different rule, also runs every 30s): sees `gMilitaryResearchPlanID` is valid, finds it NOT in `cPlanStateResearch`, hits the 2-min safety net (`currentTime > researchStartTime + 120`, where `researchStartTime = -1` stays uninitialized in the forced-tech path → `currentTime > 119` is always true after ~2 sec).
6. Military safety net DESTROYS the plan, sets `gMilitaryResearchPlanID = -1`, falls through to `haveForcedMilitaryTechnologyToResearch` → re-creates a NEW Ballistics plan via the (byte-identical to base) military branch.
7. Next economic tick → replaces `gMilitaryResearchPlanID` again with a fresh plan, abandoning the previous one.
8. Repeat forever. The engine sees plans being created and destroyed in rapid cycles; none ever reach `cPlanStateResearch`.

**Why militaryUpgradeManager is byte-identical to base yet the loop still fires**: the military rule body itself is unchanged, but its INPUT (the `gMilitaryResearchPlanID` value) is being polluted by the economic picker via the broken branches. Base doesn't pollute the military var, so the military rule works normally.

### Fix

**Commit `1512b3b`** (`2026-06-19`): removed all 15 broken branches. The function now matches base behavior — only returns `true` for the 11 truly-economic forced techs (Shaduf, Plow, Hunting Equipment, Divine Blood, Golden Apples, Necropolis, Perception, PropheticSight, SonsOfTheSun, Abundance, ChasingTheSun). The 15 removed techs (WatchTower, CopperWeapons, ..., CrossbowTower) are still researched via:

- `haveForcedMilitaryTechnologyToResearch` last-resort Ballistics branch (byte-identical to base).
- The scored evaluation in `militaryUpgradeManager` (the `minimumScoreNeeded` thresholds we already tuned).
- Dedicated monitors: `towerOffensiveUpgradeMonitor`, `wallUpgradeMonitor`, `townCenterUpgradeMonitor`, `siegeUpgradeMonitor`, `advancedFortificationsMonitor`.

**Verification**: `deploy-mods.sh` deployed cleanly (22 files, new `techs.xs` MD5 `59c592bca55a9974fede9f088b80b3af`). User must verify via next playtest — research plans should now progress to `cPlanStateResearch` and complete normally instead of looping on every 30s tick.

### Hypothesis (corrected from earlier wrong analysis)

One of our 22 overlaid files (likely from Garbhus's classic AoM mod 314139 port) has stale research-advancement logic. The base Retold AI's `haveForcedMilitaryTechnologyToResearch` in `techs.xs` is byte-identical to our overlay's version — so the trap mechanism itself isn't broken. The TRIGGER or the CLEARED state (which lives in another file) is what's broken.

A "forced technology" flag gets set during the Classical→Heroic transition (by the BO system). When this flag is set, `militaryUpgradeManager` skips its normal logic — which is what advances research plans from "created" to "researching". The flag never clears because the research never completes (it can't, because normal logic is skipped). Circular dependency.

### Suggested investigation

1. **Diff all 22 overlaid files against current base Retold versions.** Focus the diff on logic related to research advancement and forced-tech flag management. The 5 most likely culprits:
   - `core/bo_system/bo_system.xs` (BO system sets the "forced technology" flag during age-up)
   - `core/bo_system/bo_system_internal.xs` (BO system's state machine)
   - `core/techs.xs` (already verified for `haveForcedMilitaryTechnologyToResearch` — look at OTHER functions)
   - `core/military/military_attack.xs` (military state crossings)
   - `core/economy/economic_units.xs` (economy flow interactions)
   - `core/startup/game_settings_analysis.xs` (initialization — might initialize the flag wrong)

2. **Check the BO system's flag-clearing logic** — when does the "forced technology" flag get cleared in the base Retold AI? Find the file/function that clears it. Compare to our mod's overlay version.

3. **Tooling hint for the parallel-session agent**: Use the `delegate` tool to dispatch a research-focused sub-agent that greps the base AI for "forced tech" clearing logic, then diffs each candidate file in our 22-file overlay against base.

### Suggested fix approach (once root cause identified)

Patch the relevant overlaid file(s) to match the current Retold base's research advancement logic, OR add a guard that clears the "forced technology" flag after 30 seconds if the corresponding research plan hasn't entered "researching" state.

### Cross-references

- `engram` observation #933 "Forced-tech research trap is in OUR mod's 22-file overlay" — full investigation log

---

## ISSUE-02: Second-ring wall rule produces zero output

**Status**: ✅ Fix applied 2026-06-19 (`buildings.xs` rule body, commit pending). Added top-of-rule canary echo + per-gate diagnostic echoes, and replaced the suspect gate #6 (`aiPlanGetNumberByType(cPlanBuildWall) == 0`) with a `kbUnitQuery`-based wall-unit count near the main base. Awaiting user re-playtest to confirm the rule now fires visibly and (if gate #6 was the culprit) creates a 2nd-ring plan after the 1st ring builds. Engram traceability: observation #1047, topic_key `issue-02/second-ring-wall-rule-fix`.

**Severity**: HIGH for the wall-port feature (we just implemented this and it doesn't work). MEDIUM for the AI overall (the AI still has its existing 1st-ring walls).

### Symptom

Across all 22,144 lines of the AI log, **zero** `debugSecondRing` messages appear. The `secondRingWallPlanMonitor` rule produces no output — no "plan created", no "rusher delay", no "destroyed", no plan ID changes.

### Verified facts

- The rule compiled fine (after 4 fix-up commits).
- The 1st-ring wall plans WERE created (plan 111 at 00:09:49 for AutoBase 0, plans 263/269 at 00:17:42 for AutoBase 0/11). Verified via resource distribution logs.
- All gating conditions should pass after Classical Age: `cStrategyFlagBuildWalls: true` from 00:05:12, `kbPlayerGetAge(cMyID) >= cAge2` from 00:04:58, `xsGetTime() >= 8 * 60 * 1000` from 00:08:00, villager count >= 10 throughout, gold >= 150 on Legendary.
- Personality is Balanced, not Attacker — so `mRusher = false`, rusher delay doesn't apply.

### Hypothesis

The rule has no "running" echo at the top, AND the 1st-ring detection check `aiPlanGetNumberByType(cPlanBuildWall) == 0` may return `true` even when walls exist. Once the 1st-ring wall plan transitions to `cPlanStateDone` (wall completed), `aiPlanGetNumberByType` might only count ACTIVE plans, not completed ones. The rule would silently early-return every tick.

### Suggested investigation

1. **Add diagnostic echoes** at the very top of the rule and at every gate-exit point in `buildings.xs`:
   - Top of rule: `debugSecondRing("--- Running secondRingWallPlanMonitor ---")`
   - After each `return;`: `debugSecondRing("early return: <reason>")`
2. **Verify the 1st-ring detection** — search the base Retold AI for patterns that detect COMPLETED walls (e.g. unit queries for `cUnitTypeWall` or similar). Compare to `aiPlanGetNumberByType(cPlanBuildWall)`.
3. **Re-playtest** with diagnostic echoes — the next log will pinpoint the early-return gate.

### Suggested fix

- Add `debugSecondRing("--- Running secondRingWallPlanMonitor ---")` at the top + echoes at every gate-exit.
- Replace `aiPlanGetNumberByType(cPlanBuildWall) == 0` with a `kbUnitCount`-based check for wall units near the main base.

### Cross-references

- `openspec/changes/aom-retold-aomodai-port/` — the full SDD change for the wall port (proposal, spec, design, tasks, apply-progress)
- `mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs` lines 23-30 (state globals), 36-44 (init rule), 1085-1154 (helpers + main rule), 1156-1275 (`secondRingWallPlanMonitor` rule body)

---

## ISSUE-03: GreatHunt and resource god powers skipped on standard games

**Status: ✅ FIXED** in commit `bbb83b7` (`2026-06-19`). Root cause confirmed: `isUnneededGodPowerDueToResources` in `core/godpowers/godpowers.xs` is **byte-identical between our overlay and the base Retold AI** — the inverted operator is a long-standing base bug that our inherited classic-AoM overlay carried verbatim. Per the mod principle (any AI behavior issue is a mod issue even if it originates in base), the fix is applied in our overlay.

Fix flips the operator on `cStartingResourcesCurrent` (line 27 of our overlay today, originally line 13) from `==` to `!=` so the function matches its name:

- **Finite** `(cStartingResourcesCurrent != cStartingResourcesInfinite)` → early-return `false` ("not unneeded") → caller proceeds with `setUpGodPowerPlan` → resource powers ARE cast. ✓
- **Infinite** → fall through to the resource-power list → return `true` ("unneeded") → early-return at caller; resource powers are correctly skipped because they're worthless. ✓

This also fixes the same defect at the **second caller** — `godpowers_aztec.xs:288` (Copycat deciding whether to steal an enemy god power) — because `isUnneededGodPowerDueToResources` is a single shared function defined in `godpowers.xs` (which is in our 22-file overlay; `godpowers_aztec.xs` itself is NOT overlaid, but it calls our fixed function at runtime).

The pre-existing debug message at the caller (`"We're on infinite resources and X provides resources, have no use for it, exiting early."`) was lying on standard games pre-fix because it fired ~120 times across the 2026-06-19 playtest even though resources were NOT infinite. After the fix this branch only fires when resources ARE infinite — so the message is now accurate and needs no separate wording change. Rationale comments + a "do NOT revert to `==`" warning were added so future maintainers do not silently reintroduce ISSUE-03.

Verified the other 9 `cStartingResources` uses across the overlay (`economic_units.xs` × 7, `techs.xs` × 2) use the SAME `== cStartingResourcesInfinite` operator but with the OPPOSITE intent (`if (infinite) { disableRule; return; }` — bypass the rule on infinite-resource games where gathering/research is pointless). Those are CORRECT and out of scope for ISSUE-03.

**Deploy**: `scripts/deploy-mods.sh` deployed cleanly (22 files, new `godpowers.xs` MD5 `a1f1550eabe554dbae65968a8241e780`). `techs.xs` MD5 `59c592bca55a9974fede9f088b80b3af` intact from ISSUE-01's fix `1512b3b`. User must verify via next playtest — on a standard-resource Norse game, `useUnusedGodPowersMonitor` (every 10s) should now actually create and trigger god-power plans for GreatHunt / Lure / Rain / Prosperity / DwarvenMine / GaiaForest / PlentyVault / PeachBlossomSpring / ProsperousSeeds; the spurious "We're on infinite resources" debug spam will be absent.

**Severity**: MEDIUM — Norse AI on any standard-difficulty game never uses GreatHunt (which would boost its economy). Same for Lure, Rain, Prosperity, DwarvenMine, GaiaForest, PlentyVault, PeachBlossomSpring, ProsperousSeeds.

### Symptom (with log evidence)

Every 10 seconds from `useUnusedGodPowersMonitor`:

```
00:07:12  (432282): We have 1 GreatHunt charges.
00:07:12  (432282): We have an unhandled charge of GreatHunt, using it now.
00:07:12  (432282): We're on infinite resources and GreatHunt provides resources, have no use for it, exiting early.
```

The message repeats ~120 times across the game. The AI never uses GreatHunt. User confirms: resources were NOT infinite; it was a standard game.

### Verified facts

- The check lives in `core/godpowers/godpowers.xs` lines 24-43 (`isUnneededGodPowerDueToResources` function).
- Our mod **overlays** `godpowers.xs` — verify whether our overlay has the same inverted logic or corrects it.
- The logic is inverted:
  ```xs
  if (cStartingResourcesCurrent == cStartingResourcesInfinite)
  {
     return false;   // On INFINITE: don't skip — uses resource powers (useless)
  }
  // On STANDARD games:
  if (protoPowerID == cProtoPowerGreatHunt || ...) return true;  // Skips useful powers
  ```
  The `==` should be `!=`. The debug message also lies about the game state.

### Suggested investigation

1. **Read our overlay's version of `godpowers.xs` lines 24-43** — confirm whether our overlay has the same inverted logic. (Likely yes — Garbhus's port is from classic AoM, this bug has likely been in Retold since launch.)
2. **Decide on the fix scope**: should the mod fix just this function (`isUnneededGodPowerDueToResources`), or also fix related resource-skip checks?

### Suggested fix

Flip the `==` to `!=` in `isUnneededGodPowerDueToResources`. Fix the debug message to accurately describe the state. Our mod already overlays `godpowers.xs` — just edit that file in the mod folder.

### Cross-references

- `core/godpowers/godpowers.xs` (Retold base)
- `mod/Extra Ai + AoModAi/game/ai/core/godpowers/godpowers.xs` (our overlay)

---

## ISSUE-04: WalkingWoods never used defensively

**Severity**: MEDIUM — Freya AI can't use WalkingWoods for defense. The user noted "Walking woods would definitely have helped it defend itself in the final moments" though trees are Myth units and would be slaughtered by Hersirs (heroes).

### Symptom (with log evidence)

Every 10 seconds from `walkingWoodsMonitor`:

```
00:09:48  (588112): --- Running Rule walkingWoodsMonitor. ---
00:09:48  (588112): Found 0 attack/explore plans in attack state to analyze.
```

The rule only considers **active attack/scout plans** for casting WalkingWoods (the trees walk alongside an attacking army). It never considers defensive use. When the AI was being overrun at 00:19:04 and desperately needed defensive trees, the monitor still found 0 attack plans and did nothing.

The WalkingWoods plan WAS created at 00:09:48 ("Created a god power plan for: WalkingWoods"). 1 charge was available throughout. The AI cast ForestFire at 00:15:27 — proving trees existed near the enemy base. But WalkingWoods was never cast.

### Verified facts

- `walkingWoodsMonitor` is a polling rule, runs every 10 seconds.
- It only checks for `cPlanAttack` and `cPlanExplore` plans in attack state.
- It does NOT consider: base under attack, `gDefensivelyOverrun` flag, `isBaseUnderSustainedAttack()` (our new helper from the wall port), or any defensive trigger.
- The rule's file location is NOT in our 22 overlaid files list (likely `core/godpowers/godpowers_norse.xs` — verify by grep).

### Suggested investigation

1. **Locate `walkingWoodsMonitor`** in the Retold base (`grep -rn "walkingWoodsMonitor" /home/houtamelo/.steam/.../game/ai/`).
2. **Check if our mod overlays that file**. If it doesn't, fixing this requires adding a new overlay.
3. **Read the monitor's logic** to understand how WalkingWoods plans are activated and what gating prevents defensive casting.

### Suggested fix

Extend `walkingWoodsMonitor` to also cast when:
- `gDefensivelyOverrun == true` (AI is in defensive panic), OR
- `isBaseUnderSustainedAttack(mainBaseID) == true` (our new wall-port helper — can be reused here)

The cast target should be a defensive position (e.g. center of own main base) when in defensive mode, not an enemy base.

### Cross-references

- Our `isBaseUnderSustainedAttack` helper in `buildings.xs` (read `gEnemyPowerInBases[gDefendTCBases.find(baseID)]`) can be reused here
- Defensive-trigger pattern: same flag (`gDefensivelyOverrun`) used by `military_defend.xs`, `military_attack.xs` (`cStateForcedCantAttack`)

---

## ISSUE-05: AI makes suicide attacks (into unwinnable positions)

**Severity**: HIGH (tactical). The AI loses entire armies by attacking into clearly unfavorable positions. This is one of the most visible AI deficiencies.

### Symptom (with log evidence)

**Attack #77** (launched 00:07:42 against AutoBase 5, Player 1):

```
00:07:42  (462332): ***** LAUNCHING ATTACK on player: 1, base: AutoBase 5 Player 1
00:07:42  (462332): 36 military pop / 26.94 siege power
00:09:17  (557762): 77: Attack Player 1 Base AutoBase 5 Player 1 : Completed: we have no more units and also can't get more.
```

16 units lost. 0 buildings destroyed by the AI. The AI attacked INTO the player's main base — Player 1's TC + towers + Healing Spring fountain all fired on the Norse army while Player 1's Hersirs held the line.

**Attack #161** (launched 00:12:33 against AutoBase 10, Player 1):

```
00:12:33 (754212): ***** LAUNCHING ATTACK on player: 1, base: AutoBase 10 Player 1
00:14:21 (859862): 161: Attack Player 1 Base AutoBase 10 Player 1 : Completed: we have no more units and also can't get more.
```

30 more units lost. 0 buildings destroyed. Per the user: the AI was drastically outnumbered and resource-starved (Player 1 controlled all gold mines). The AI's units were mostly cavalry and could have outrun Player 1's units — they could have retreated.

### Verified facts

- The AI launches attacks based on `metRequirementsToAttack` and timing gates (next-attack timer)
- The AI does NOT appear to evaluate relative army strength vs. enemy position before committing
- The AI has no tactical retreat logic in `attackManager` — once an attack is launched, it walks until either destroying the target or being wiped out
- The AoModAi source we explored had a smarter "createLandAttack" rule that considered enemy titan presence, base-under-attack, wonder-defense-in-progress — we explicitly DEFERRED porting this behavior in the proposal (it was on the rejected list)
- The user's correction: the AI's cavalry units could have outrun Player 1's units — they could have retreated, but didn't try

### Hypothesis

`military_attack.xs` (which we DO overlay) likely has the `attackManager` rule with logic for when to launch an attack. It probably lacks:
- A "should I retreat?" check once the attack is mid-air (vs. enemy strength at target)
- A consideration of enemy TC/tower firepower at the target location
- A "we're outnumbered 2:1, withdraw" abort

### Suggested investigation

1. **Read `core/military/military_attack.xs` (both base and our overlay)** — find the rule that manages active attack plans, look for where retreat/abort decisions would live.
2. **Check the AoModAi source (`extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs`)** for the equivalent logic — AoModAi reportedly has smarter "cancel attack when outmatched" logic (per the user, this was one of AoModAi's celebrated behaviors).
3. **Cross-reference with the exploration doc** — `openspec/changes/aom-retold-aomodai-port/exploration.md` section 2.2 covers AoModAi's "smarter military decisions" — that section lists what AoModAi does that Retold's default doesn't.

### Suggested fix

Port the AoModAi "monitor attack plans and retreat when outmatched" logic into our overlay of `military_attack.xs`. Per the original exploration:
- AoModAi's `monitorAttPlans` rule (at `AoModAIMil.xs:395`) re-prioritizes, **retreats damaged plans**, and destroys empty ones
- Triggers: enemy titans near main/def base, own base under heavy attack, wonder-defense in progress, sustained stall (3-5 min no progress)

This was on our proposal's rejected list (point 5: "Smarter target picking / massing") because Retold's `attackManager` was assumed to cover it. Given the playtest evidence, it should be UN-rejected and added back to the proposal.

### Cross-references

- `openspec/changes/aom-retold-aomodai-port/exploration.md` section 2.2 — AoModAi's smarter military decisions
- `openspec/changes/aom-retold-aomodai-port/proposal.md` row 5 — was rejected, should be un-rejected
- `core/military/military_attack.xs` (both versions: base + our overlay)

---

## ISSUE-06: AI attacks while leaving home undefended

**Severity**: HIGH (tactical). The AI launched an attack at 00:18:43 that stripped home defense to 5 power. 21 seconds later, AutoBase 11 was overrun by 135 enemy power. Classic "attack while undefended" failure — directly caused the AI's loss.

### Symptom (with log evidence)

```
00:18:43 (112282): ***** LAUNCHING ATTACK on player: 1, base: AutoBase 20 Player 1
[home defense stripped to 5 power]
00:19:04 (1152410): AutoBase 11 Player 2 enemy power climbs to 112.865135
00:19:04: gDefensivelyOverrun = true
00:20:30: healthScore drops to 27.06
00:21:00: AI resigns
```

### Verified facts

- The attack at 00:18:43 took 92 military pop away from base defense, leaving only 5 power at home
- AutoBase 11 fell to a 135-power enemy attack ~21 seconds later
- The AI's `attackManager` doesn't appear to coordinate with `defendManager` — attacks launch on a timer without checking if home is currently defended
- `military_attack.xs` (overlaid) and `military_defend.xs` (NOT overlaid) both participate in this decision

### Hypothesis

The "should I launch an attack?" gate in `military_attack.xs` considers:
- Attack timer (next-attack scheduled time)
- Available military pop
- Difficulty multiplier

But does NOT consider:
- Current enemy threat to own home base (the `gEnemyPowerInBases` array we use for the wall rule)
- Whether own base defense is currently in `gDefensivelyOverrun`
- Whether enemy forces are visibly massing near own territory

### Suggested investigation

1. **Read `military_attack.xs` (both versions)** — find the launch-attack decision logic.
2. **Look for `gDefensivelyOverrun` checks** — does the base AI check this before launching? (The `cStateForcedCantAttack` state may already do this — investigate WHY it didn't engage earlier.)
3. **Cross-reference AoModAi** — its `createLandAttack` reportedly checks "wonder-defense in progress" and "own base under heavy attack" before launching. We should port that.

### Suggested fix

Add a pre-launch gate in `attackManager` (in our `military_attack.xs` overlay):
- If `gEnemyPowerInBases[mainBaseIndex] > some threshold` → don't launch, transfer units to primary defend plan instead
- If `gDefensivelyOverrun == true` → already blocked by `cStateForcedCantAttack` per the log; verify this is working as expected

### Cross-references

- `gEnemyPowerInBases` array (we already understand this from the wall port — declared in `core/globals.xs:163-164`, indexed parallel to `gDefendTCBases`)
- AoModAi's `createLandAttack` (per `exploration.md` section 2.2)

---

## ISSUE-07: Building placement failures cascade

**Severity**: LOW-MEDIUM — wastes some build plans, but the AI's economy survives. The failures cluster at the late game when the base footprint is crowded.

### Symptom (with log evidence)

Multiple distinct sub-issues:

**Dropsite placements** (plans 143, 144, 333, 334, 340):
```
00:11:24: 143: Build Dropsite: parent 1: another building placement claimed a spot too close to our lot, failing.
00:20:34: BP - 333: Build Dropsite: parent 149: failing because we found 0 suitable spots to build on.
```

**Farm placements** (plans 140, 176, 186, 199):
```
00:12:02: 140: Build Plan for 2 Farm : Failed checkPlacement, something must be blocking the spot, restarting plan.
00:12:23: 140: Build Plan for 2 Farm : Failed checkPlacement [same plan retried, same spot, still blocked]
00:14:13: 186: Build Plan for 2 Farm : Failed checkPlacement
00:14:53: 199: Build Plan for 2 Farm: our best position is no longer unobstructed
```

**SentryTower placement** (plan 145):
```
00:15:43 → 00:15:51: 5 retries, all fail with "another building placement claimed a spot too close to our lot"
00:15:51: SentryTower plan permanently killed
```

**Dock placements** (plans 212, 213):
```
00:15:54: BP - 212: Build Plan for 1 Dock: another building placement claimed a spot too close to our lot
00:15:55: BP - 213: 3 retries left
```

### Verified facts

- All these placement failures cluster in the late game (post-Mythic)
- Farm plan 140 retried the SAME blocked spot twice — no fallback logic to try a different location
- Tower plan 145 exhausted all 5 retries then died — final AI base had no additional towers after this

### Hypothesis

The placement logic tries a "best" spot, fails on obstruction (other buildings already there), retries the SAME spot, fails again. Lacks a fallback to "try the next best spot" or "expand the search radius" after a failure.

### Suggested investigation

1. Find the placement helper functions — likely in `core/buildings/buildings.xs` (overlaid) or `core/buildings/dropsite_placement.xs` (overlaid) or `core/utilities/utilities.xs` (NOT overlaid).
2. Read the retry logic — does it actually alternate positions, or just re-evaluate the same "best" position?
3. Cross-reference AoModAi's `buildings_economic.xs` for "find alternative placement" patterns.

### Suggested fix

After a failed placement: rather than retrying the same spot, decrement the priority/radius and search again. Implement a "next-best fallback" pattern. Most affected call sites are in files we already overlay (`buildings.xs`, `dropsite_placement.xs`).

---

## ISSUE-08: Ox Cart pathing permanently stuck

**Severity**: LOW-MEDIUM — one gather plan permanently failed to assign an Ox Cart, but the AI had other dedicated gatherers on the same resource.

### Symptom (with log evidence)

```
00:16:23: Ox Cart(525717) can't path to 227: AutoGPGoldEasy.
00:16:23: 227: AutoGPGoldEasy can't create a build plan because the plan has no units in it.
00:16:53: Ox Cart(525717) can't path to 243: AutoGPGoldEasy.
```

The same Ox Cart unit (525717) failed to path to multiple AutoGPGoldEasy plans across at least 2 separate attempts, 30 seconds apart. The gather plans never recovered.

### Verified facts

- This is a pathing failure — the Ox Cart exists but can't reach the resource
- The gather plan was renamed/reassigned (227 → 243) between failures — the AI's economic_units system tried alternative plans but the SAME cart kept being tapped
- No overlay of `core/economy/economy.xs` (where Ox Cart routing likely lives) — would need to add it

### Suggested investigation

1. Find where `Ox Cart ... can't path to` is logged — likely in `economy.xs` or `dropsite_placement.xs` (overlaid).
2. Understand why the cart keeps being selected — is there a per-cart capability check before assignment?
3. Consider: should the AI abandon the gold gather plan and redirect villagers to a different gold source when one is permanently unreachable?

### Suggested fix

Add a per-cart "last failed path time" record. If a cart can't path to a target twice in a row, blacklist it from that target for the rest of the game (or until the target moves).

### Cross-references

- `dropsite_placement.xs` (overlaid) — likely the assigner

---

## ISSUE-09: `gDefensivelyOverrun` lock never clears

**Severity**: HIGH — once the AI is overrun, it STAYS overrun for the rest of the game. The AI never recovers, retreats, or counter-attacks.

### Symptom (with log evidence)

```
00:19:04: gDefensivelyOverrun = true
[rest of game]
00:19:14: defendManager sets gDefensivelyOverrun = true [set again]
[repeats at 00:19:24, 00:19:34, 00:19:44, ...]
00:21:00: AI resigns
```

The `gDefensivelyOverrun = true` flag is set, repeated on every `defendManager` cycle, and never clears. While in this state:
- `attackManager` enters `cStateForcedCantAttack` — no attacks possible
- `navalMilitaryManager` cancels all naval military training ("We're defensively overrun on the land, not training any naval military now")
- `maintainTraps` suspends ("We're currently defensively overrun, use all army to fight and not build")
- `tcExpansionMonitor` stops claiming new settlements
- `militaryManager` cancels siege training ("defense panic")

### Verified facts

- `gDefensivelyOverrun` is a global state flag, not a per-base flag — once true, ALL offensive/strategic actions are blocked
- After AutoBase 11 fell at ~00:20:36, the AI's only remaining TC (TC 1) was at AutoBase 0 — but the overrun flag never cleared because the threat was still nearby
- There's no "the threat has passed, clear the overrun state and try to recover" logic observed in the log

### Hypothesis

`military_defend.xs` (NOT currently overlaid) likely manages `gDefensivelyOverrun`. The flag is set when enemy power > some multiple of own power at the main base. It's likely cleared when enemy power drops below the threshold — but if the enemy player keeps units near the main base (which Player 1 did, since they were attacking), the flag never clears.

### Suggested investigation

1. **Locate `gDefensivelyOverrun` setter/clearer** in the Retold base (`grep -rn "gDefensivelyOverrun" /home/houtamelo/.steam/.../game/ai/`).
2. **Read the clear condition** — what would make it false? If the threshold is too aggressive (must reduce enemy power to 0), the AI is stuck as long as the player sieges.
3. **Consider partial recovery** — even if enemy forces are near, the AI should be able to train new units and assemble a counter-force. Maybe `gDefensivelyOverrun` should only block NEW attacks, not training.

### Suggested fix

Overlay `core/military/military_defend.xs` (new file in the mod — currently NOT overlaid). Add recovery logic: if `gDefensivelyOverrun == true` for more than 60 seconds AND own military pop is rebuilding past 50% threshold, clear the flag.

---

## ISSUE-10: Wrong-civ myth unit fallback (Mountain Giant for Norse)

**Severity**: LOW — Norse AI sometimes queues Mountain Giant (an Atlantean myth unit), which is weird but doesn't break the game.

### Symptom (with log evidence)

```
00:17:18 (1038310): mythMilitaryTraining - analyzing mythPUID: MountainGiant. mythMilitaryTraining - currentMythPop: 0.
00:17:21 (1040360): currentMythPop: 9 [was Valkyries]
00:17:21 (1040360): [trains a Mountain Giant to bring myth pop to 13]
```

The Norse AI trained Mountain Giant — a unit type that doesn't belong to Norse at all. It's an Atlantean myth unit.

### Verified facts

- The AI is Norse (Freyja minor god) — its myth units should be Valkyrie, Troll, Frost Giant, etc.
- The `mythMilitaryTraining` rule was analyzing `MountainGiant` as a candidate
- Valkyries were trained successfully earlier in the game
- The Mountain Giant appears in `godpowers_aztec.xs` context elsewhere in the logs (curious cross-reference — might be a leak of Aztec units into Norse context)

### Hypothesis

`godpowers_japanese.xs` (which we DO overlay) might have a myth-unit selector that has Mountain Giant as a fallback when the proper Norse myth unit isn't available or is at capacity.

### Suggested investigation

1. Find the myth-unit selector — `grep -rn "MountainGiant" /home/houtamelo/.steam/.../game/ai/` to see which files reference this unit type.
2. Find the `mythMilitaryTraining` rule — check whether our overlay of `godpowers_japanese.xs` or `military_units.xs` has a too-broad fallback list.
3. Note: this is a curiosum — confirm with more playtests before fixing.

### Suggested fix

Constrain the Norse myth-unit selector to Norse-only myth units. Drop Mountain Giant from the candidate list when `cMyCulture == cCultureNorse`.

---

## ISSUE-11: Trade economy collapses briefly on Market destruction

**Severity**: LOW — Market was destroyed at 00:18:02 and rebuilt by 00:18:40 (38 seconds of trade disruption). Not game-deciding, but worth noting.

### Symptom (with log evidence)

```
00:18:02 (1080210): Our Market has been destroyed! We need to perform all the construction logic again.
00:18:02 (1080210): [Caravans drop from 15 to 0]
00:18:08 (1086210): [tradeMonitor route analysis: still protected by TC1/TC11]
00:18:08 (1086210): Created Market build plan 268 at area 334
00:18:40 (1118410): Built our Market ID: 1312422, in areaID: 314
00:18:40 (1118410): Adjusting CaravanNorse Maintain plan to maintain: 15
```

### Verified facts

- Market destroyed by enemy at 00:18:02
- All caravans reduced to 0 immediately (the trade route was instantly invalidated)
- AI rebuilt the Market in 38 seconds (fast)
- Caravans restored to 15 within seconds of Market completion
- Trade income lost for ~38 seconds — not game-deciding at game time 00:18, but could be decisive earlier

### Suggested investigation

1. Find where the caravan-count is reset to 0 — likely in `core/economy/economy.xs` (NOT overlaid) or somewhere in `bo_system.xs`/`bo_system_internal.xs` (both overlaid).
2. Check whether queued Caravans (training, not yet built) survive the Market destruction. If they don't, the AI loses queued Caravan training time on top of the trade income loss.

### Suggested fix (optional, low priority)

- Keep the caravan maintain plan at its prior count when Market is destroyed (just let it pause), then resume instantly when the new Market is built.
- Pre-queue the Market rebuild rather than waiting for the next `tradeMonitor` cycle.

---

## Appendix: Distribution of issues by file involvement

| File                                   | Related issues              | Our mod overlays it? |
| -------------------------------------- | --------------------------- | -------------------- |
| `core/buildings/buildings.xs`         | ISSUE-02, ISSUE-07          | YES                  |
| `core/buildings/dropsite_placement.xs` | ISSUE-07, ISSUE-08          | YES                  |
| `core/buildings/buildings_economic.xs` | ISSUE-07                    | YES                  |
| `core/godpowers/godpowers.xs`         | ISSUE-03                    | YES                  |
| `core/godpowers/godpowers_japanese.xs` | ISSUE-10                    | YES                  |
| `core/military/military_attack.xs`    | ISSUE-05, ISSUE-06          | YES                  |
| `core/military/military_units.xs`     | ISSUE-10                    | YES                  |
| `core/techs.xs`                        | ISSUE-01 (**FIXED** `1512b3b`) | YES                  |
| `core/bo_system/bo_system.xs`         | ISSUE-01 (suspect #3 — cleared, not the cause) | YES                  |
| `core/bo_system/bo_system_internal.xs`| ISSUE-01 (cleared, not the cause), ISSUE-11 | YES                  |
| `core/economy/economic_units.xs`      | ISSUE-08                    | YES                  |
| `core/economy/resource_breakdown_system.xs` | ISSUE-07-area               | YES                  |
| `core/military/military_defend.xs`    | ISSUE-05, ISSUE-06, ISSUE-09 | NO (would need new overlay) |
| `core/godpowers/godpowers_norse.xs`   | ISSUE-04                    | NO (would need new overlay) |
| `core/economy/economy.xs`             | ISSUE-08, ISSUE-11          | NO (would need new overlay) |
| `core/utilities/utilities.xs`         | ISSUE-07 (placement helpers)| NO                   |
| `core/strategy/strategy_internal.xs`   | ISSUE-09 (overrun state)    | NO                   |

## Workflow for parallel sessions

Each issue above is structured to be self-contained: a fresh agent session can pick up ISSUE-XX, do the investigation (reading the listed files), implement a fix, and validate. Sessions can run in parallel without coordination, since each addresses a different aspect of the AI.

Conventions for parallel sessions:
- **Don't touch the wall-port code** (`core/buildings/buildings.xs` audit-marker blocks) unless your issue is ISSUE-02.
- **Don't modify the `aom-retold-aomodai-port` openspec artifacts** unless your issue is ISSUE-02 (they're tied to that specific change).
- **Each session creates its own openspec change** under `openspec/changes/<issue-id>-<short-name>/` for its spec/design/tasks. Suggested IDs: `01-research-trap`, `02-wall-rule-diagnostic`, `03-god-powers-resources`, `04-walking-woods-defensive`, `05-suicide-attacks`, `06-attack-while-undefended`, `07-placement-fallback`, `08-oxcart-pathing`, `09-defensive-overrun-recovery`, `10-myth-unit-fallback`, `11-market-loss-resilience`.
- **Commit author**: use `houtamelo <antoniopedrogf@hotmail.com>` (the user's preferred identity, not the `SDD Apply` generic one).
- **Do not push** without explicit user request.
- **Playtest cadence**: the user runs playtests manually. Don't block on playtest verification — the user will playtest fixes when they see fit.
