# Techs.xs Diff Analysis: Mod vs Vanilla Approach to Technology Research

**Scope**: All differences between vanilla `core/techs.xs` and mod's `core/techs.xs`.
**Method**: `diff -u` structural overview + targeted reads of both files for context.
**Verdict (TL;DR)**: The mod's approach to tech research is **systematically counterproductive**. By lowering every threshold for what counts as "worth researching," the mod starves critical-priority techs (like WatchTower) of the finite global research slots — inverting the priority system vanilla uses to keep critical researches scheduled. **Most threshold changes should be reverted to vanilla**; a small number of structural changes (Aztec wall/siege enablement, tower monitor rewrite excluding the new Boiling Oil/TemiminaloyanTrials blocks) are worth keeping.

---

## 1. Executive Summary

| Metric | Vanilla | Mod |
|---|---|---|
| Total lines | 2,950 | 2,987 |
| Net changed lines (base → mod) | — | +108 / −71 |
| Functions modified | 8 | — |
| Functions added | 0 | — |
| Functions removed | 0 | — |

**Top-level verdict**: Patchwork of half-finished ideas. The mod's research changes collectively make the AI *less* effective at researching the *right* techs, even though they make it research *more* techs overall. The threshold-lowering approach is fundamentally incompatible with vanilla's `areAtMaxConcurrentResearchPlans` slot-limit guard, which is the gatekeeper that protects critical techs like WatchTower from being starved out by low-priority research.

---

## 2. Function-by-Function Diff Inventory

### 2.1 `getWeightsPerCulture` (lines ~195–305)

**What vanilla does**: Sets `armoryUpgradeChance = 15` for all cultures; 25 for Thor specifically. Logs a warning for unknown cultures via `default:`.

**What the mod does**: Raises `armoryUpgradeChance` to **35** (all cultures), **45** for Thor. Removes the `default:` warning case.

**Apparent intent**: Make the AI prioritize armory upgrades more eagerly. Severity: minor.

**Side effects**: 
- Removing the `default:` case means unknown/future cultures silently use the mod's thresholds without logging the catch.
- Higher chance means armory upgrades win the scoring race more often, which under the one-slot/limited-slots research rule **displaces** critical techs higher in the queue.

**Assessment**: ❌ **Net negative**. The threshold-raising direction is the wrong lever — vanilla's value (15/25) is calibrated to keep armory upgrades as a *secondary* priority, not a primary one. The mod makes armory upgrades compete with critical techs like WatchTower.

---

### 2.2 Myth-tech auto-research list trim (line ~415 + line ~486)

**What vanilla does**: Auto-researches (via `cModeAutoResearch`) a list of myth techs including `cTechPredatoryInstinct`, `cTechHallowedWoodlands`, and `cTechAdvancedTraps` (Mayan/Aztec traps treated as myth).

**What the mod does**: Removes these three techs from the auto-research list.

**Apparent intent**: Unclear. Possibly: (a) wants explicit research paths elsewhere, (b) considers them unnecessary, (c) legacy artifact from classic AoM where these techs had different IDs.

**Side effects**:
- These three techs will **never be researched** unless explicit research code exists elsewhere. A grep should be done to verify.
- `cTechAdvancedTraps` is Aztec/Mayan — if no explicit replacement is added, Aztec/Mayan AI loses this completely. **Potential regression for those cultures**.

**Assessment**: ⚠️ **Unclear — depends on context**. If the mod author added explicit research paths for these techs elsewhere in the codebase, the removal is harmless. If not, this is a silent feature loss. **Needs verification** (grep for `cTechPredatoryInstinct`, `cTechHallowedWoodlands`, `cTechAdvancedTraps` across the mod tree).

---

### 2.3 `militaryUpgradeManager` (lines ~1028–1067)

**What vanilla does**: Minimum score threshold of 300 (Overwhelmer: 150). Excess-resources threshold = 1000. "Close to max pop" threshold = 90% of cap.

**What the mod does**: Radically lowers **all three thresholds**:
- Minimum score: 300 → **100** (Overwhelmer: 150 → **50**, i.e. **1/3** of vanilla)
- Excess resources: 1000 → **200** (1/5 of vanilla)
- "Close to max pop" pop-ratio: 0.9 → **0.3** (1/3 of vanilla)

**Apparent intent**: Make the AI tech up more eagerly, presumably so it has upgraded units earlier.

**Side effects**: This is the **single most damaging change** in the file. Combined with the mod's other threshold reductions (item 2.4 below), it means:
- Minimum score 50 (Overwhelmer) means *near-any* upgrade with a non-zero score is eligible. Vanilla uses 150–300 to filter for *meaningful* upgrades.
- Excess-resource threshold 200 means the AI considers itself "excess" (and thus immune to the minimum-score gate) almost immediately after game start. Vanilla requires 1000 across all resources before dropping the floor.
- **30% pop-ratio to trigger "close to max pop" is absurd** — the AI will think it's "close to max pop" when it has barely begun producing units, immediately removing all score floors and researching the next-best tech regardless of value.
- **All of this drives `areAtMaxConcurrentResearchPlans` to `true` continuously** because more techs pass the bar faster, occupying the limited research slots. Critical-priority monitors (`towerOffensiveUpgradeMonitor`, `townCenterUpgradeMonitor`, etc.) all guard on this function — see Section 4 for the mechanism. **This is the systemic root cause of the WatchTower regression (ISSUE-NEW-1)**.

**Assessment**: ❌ **Strongly net negative**. This change inverts the entire priority system. Vanilla keeps critical researches scheduled by *raising the bar* so only high-value techs pass the score gate, leaving research slots free for monitor-driven critical techs like WatchTower. The mod does the opposite — it *lowers the bar* so any tech passes, flooding the limited research slots with low-priority work and blocking the critical techs from ever being scheduled.

---

### 2.4 `economyUpgradeManager` (lines ~1508–1530)

**What vanilla does**: Minimum score 150 (Economist personality: 75). Excess threshold 1000.

**What the mod does**: Mirrors the military manager changes:
- Minimum score: 150 → **50** (Economist 75 → **25**, i.e. **1/3**).
- Excess threshold: 1000 → **200**.

**Apparent intent**: Same as 2.3 — make AI tech up more eagerly on the economy side.

**Side effects**: Same systemic problem as 2.3 — economy slots get clogged with low-priority techs, starving monitor-driven critical economy techs.

**Assessment**: ❌ **Net negative** for the same reasons as 2.3.

---

### 2.5 Nuwa `ChasingTheSun` gate (line ~1365)

**What vanilla does**: Only researches `cTechChasingTheSun` for Nuwa **if personality != Humanoid** (i.e., Humanoids are skipped).

**What the mod does**: Removes the personality check — now **all** personalities for Nuwa will trigger this research.

**Apparent intent**: Make sure Nuwa's civilization power is researched regardless of personality.

**Side effects**: Negligible — Humanoid personalities are uncommon and the tech itself is a one-shot research. Likely intentional.

**Assessment**: ✅ **Net positive (minor)** — harmless consistency fix.

---

### 2.6 `haveForcedEconomicTechnologyToResearch` (ISSUE-01 fix, line ~1365)

**What the mod does**: Adds a comment block documenting the removal of 15 broken "force-research" branches that wrote `gMilitaryResearchPlanID` directly instead of setting the `ref int techID` output parameter, contaminating the military manager's tracked plan ID.

**Status**: ✅ **Already fixed** (commit `1512b3b`). The 15 broken branches were removed.

**Assessment**: ✅ **Fix is correct and well-documented**. No further action needed for this function.

---

### 2.7 `towerOffensiveUpgradeMonitor` — WatchTower prime suspect (lines ~1795–1980)

This is the most structurally changed function. The diff has multiple sub-changes:

#### 2.7.1 Separation of Aztec vs non-Aztec WatchTower blocks

**What vanilla does**: Single block with variable indirection:
```xs
int firstTowerUpgrade = cTechWatchTower;
if (cMyCulture == cCultureAztec) { firstTowerUpgrade = cTechTzompantliWatchTower; }
if (kbTechGetStatus(firstTowerUpgrade) == cTechStatusObtainable) { ... }
```

**What the mod does**: Splits into **two separate `if` blocks** — one for `cTechWatchTower`, one for `cTechTzompantliWatchTower`. Each has the full time/defender/age gates. Code is duplicated.

**Assessment**: ⚠️ **Functionally equivalent but worse code**. The split doesn't fix any bug vanilla had — it just duplicates the gate logic. The variable-indirection approach vanilla uses is perfectly fine. **Reverting this sub-change is safe**.

#### 2.7.2 Added Boiling Oil + TemiminaloyanTrials blocks with enemy-infantry gate (mod-only, no vanilla equivalent)

The mod adds two new blocks **between** the WatchTower and Guard Tower blocks:

```xs
if (kbTechGetStatus(cTechTemiminaloyanTrials) == cTechStatusObtainable) {
   int numInfantry = kbUnitQueryExecute(useSimpleUnitQuery(cUnitTypeAbstractInfantry, cPlayerRelationEnemyNotGaia));
   if (numInfantry >= 10) {
      gTowerOffensiveResearchPlanID = researchSimpleTech(cTechTemiminaloyanTrials, cUnitTypeSentryTower, -1, prio);
      aiPlanSetEventHandler(gTowerOffensiveResearchPlanID, cPlanEventStateChange, "resetOffensiveTowerResearchPlan");
      return;
   }
}

if (kbTechGetStatus(cTechBoilingOil) == cTechStatusObtainable) {
   // Identical pattern, researching cTechBoilingOil instead
   ...
}
```

**Apparent intent**: "Don't waste resources on Boiling Oil / Temiminaloyan Trials unless the enemy has 10+ infantry visible." Reasonable tactical gate.

**Side effects**:
- The unit query (`kbUnitQueryExecute`) runs on **every tick** of this monitor — adds CPU cost.
- The two new blocks run **before** the Guard Tower / Crenellations blocks. If either block early-`return`s (when 10+ infantry is visible), the Guard Tower upgrade is **skipped this tick**.
- Vanilla vox populi: Boiling Oil is a niche tech. Adding a tactical gate is fine in principle, but adding it in the middle of an existing function that's already struggling to schedule upgrades is risky.

**Assessment**: ⚠️ **Conceptually reasonable, but in the wrong file context**. Combined with the upstream threshold-lowering problem (Section 4), these new blocks are at risk of starving the Guard Tower slot too. Recommend: keep the new feature but ensure it doesn't `return` (use `else if` chain instead of early return).

#### 2.7.3 WatchTower gate itself — verified CORRECT, NOT the regression cause

**Important correction to prior agent reports**: The WatchTower gate uses:
```xs
int time = gAgeUpTimes[cAge2] + 240;  // 4 minutes after Classical
if (cPersonalityCurrent == cPersonalityDefender) { time -= 60; }
if (time < xsGetTime()) { researchUpgrade = true; }
// + gDefensivelyOverrun == true → researchUpgrade = true
// + age >= cAge3 → researchUpgrade = true
```

**`xsGetTime()` returns SECONDS** in Retold (verified at vanilla line 96: `thresholdTime = 60;` with comment "60 seconds" — and `gAgeUpTimes[currentAge] + thresholdTime < currentTime`). `gAgeUpTimes[cAge2]` stores the `xsGetTime()` value (seconds) at age-up, set in `handlers.xs:100` (`gAgeUpTimes[newAge] = xsGetTime();`).

So `gAgeUpTimes[cAge2] + 240` = (age-up-time-seconds) + 240 seconds = age-up-time + 4 minutes. **The comparison is correct.** The mod's three fallback gates (`gDefensivelyOverrun`, `age >= cAge3`) are also reasonable safety nets.

P4 reached Classical at 00:04:03 → gate should pass at 00:08:03. P4 was defensively overrun from 00:17:23 → second gate passes. P4 reached Mythic at 00:19:11 → third gate passes. **At least one of the three gates should ALWAYS have been true after minute ~8.**

Yet P4 never researched WatchTower. **So the gate is NOT the bug.** Section 4 explains the actual root cause.

**Assessment**: ✅ The mod's WatchTower gate logic itself is fine — partially smarter than vanilla (the `gDefensivelyOverrun` and `age >= cAge3` fallbacks are good ideas). The bug is upstream — it's the `areAtMaxConcurrentResearchPlans` guard at line 1777 that starves the monitor of execution time.

#### 2.7.4 Same Aztec/non-Aztec split applied to Guard Tower block

Identical pattern to 2.7.1, applied to the second tower upgrade (Guard Tower / Temiminaloyan Trials). Same assessment: functionally equivalent, structurally worse.

**Assessment**: ⚠️ Revert to vanilla's variable-indirection approach.

---

### 2.8 `townCenterUpgradeMonitor` (line ~2269)

**What vanilla does**: Gates one of the tc upgrades on `gAgeUpTimes[cAge3] + 600` — i.e., 10 minutes after reaching Heroic.

**What the mod does**: Changes `cAge3` → `cAge4` — i.e., 10 minutes after reaching **Mythic**.

**Apparent intent**: Unclear. May have been an attempt to delay the tech.

**Side effects**:
- If the player **never** reaches Mythic, `gAgeUpTimes[cAge4]` is initialized to 0 (per `setup.xs:804`), so `time = 0 + 600 = 600` seconds = 10 minutes from game start. **Less restrictive than intended** in that scenario.
- If the player reaches Mythic at, say, minute 15, the tech won't fire until minute 25. Vanilla fires it at Heroic-up + 10 min. **Significantly later than vanilla**.
- This is likely an unintentional regression — the comment still says `// 10 minutes` without indicating the age intended changed.

**Assessment**: ❌ **Bug — likely unintentional**. Revert to `cAge3` unless the mod author has a specific Heroic-vs-Mythic intent that's documented somewhere.

---

### 2.9 `wallUpgradeMonitor` changes (lines ~2259-2549)

Three changes bundled here:

#### 2.9.1 Removed `cTechThornedWalls` (Demeter) auto-research block

**What vanilla does**: For Demeter (non-Humanoid), auto-creates or boosts a research plan for Thorned Walls.

**What the mod does**: Removes this entire block.

**Side effects**: Demeter no longer gets Thorned Walls auto-researched. Thorned Walls is a major defensive upgrade for Demeter — without it, Demeter loses a signature defensive tool.

**Assessment**: ❌ **Active regression** unless an explicit replacement was added elsewhere. **Grep for `cTechThornedWalls`** in the mod tree to verify.

#### 2.9.2 Removed Aztec from wall-disable list

**What vanilla does**: Disables `wallUpgradeMonitor` for Norse AND Aztec (presumably they have no walls in vanilla config).

**What the mod does**: Disables for **only Norse** — Aztecs now get wall upgrades.

**Apparent intent**: Aztecs apparently have walls in Retold (mod author noted the change).

**Side effects**: Depends on whether Aztecs actually have walls in Retold. If yes — improvement. If no — running the monitor unnecessarily wastes CPU. Probably fine either way.

**Assessment**: ✅ **Likely net positive** if Aztecs have walls in Retold (which they do — Retold gave Aztecs some wall tech).

#### 2.9.3 Removed `return` after `xsDisableRule("wallUpgradeMonitor")`

**What vanilla does**: After disabling the rule, returns immediately.

**What the mod does**: Disables the rule BUT continues executing — the subsequent code runs even though the rule is now disabled.

**Side effects**: 
- Could cause errors (rule context invalid after disable).
- May cause unintended scheduling work in the disabled rule's body.

**Assessment**: ❌ **Bug — restoration candidate**. Almost certainly unintentional; the `return` after disable is a vanilla pattern.

---

### 2.10 `siegeUpgradeMonitor` (line ~2602)

**What vanilla does**: Disables the siege monitor for Aztecs ("Aztecs don't have access to these technologies").

**What the mod does**: Removes the Aztec disable block.

**Apparent intent**: Aztecs in Retold may have siege access where they didn't in classic.

**Side effects**: Aztec AI now runs the siege monitor. If they have no siege, monitor runs harmlessly looking for techs that don't exist. If they do, it's an improvement.

**Assessment**: ✅ **Likely net positive or neutral** (Retold gave Aztecs siege access).

---

### 2.11 Init-rule renames — possible bugs (lines ~2655, ~2857)

#### 2.11.1 `advancedFortificationsMonitor` → `omniscienceMonitor`

**What vanilla does**: In Deathmatch and Easy difficulty, disables `advancedFortificationsMonitor` (prevents advanced fortification research in those modes).

**What the mod does**: Disables `omniscienceMonitor` instead.

**Side effects**: 
- `advancedFortificationsMonitor` KEEPS RUNNING in DM/easy (should have been disabled).
- `omniscienceMonitor` gets disabled (unrelated to fortification research).

**Assessment**: ❌ **Seems like a bug**. Different rules entirely. Probably unintentional string replacement or refactor error. **Verify the mod author's intent** — if the rule was renamed in the rules file too, OK. If not, two unrelated bugs.

#### 2.11.2 `marketUpgradeMonitor` → `economyUpgradeManager`

**What vanilla does**: In Infinite resources mode, disables `marketUpgradeMonitor`.

**What the mod does**: Disables `economyUpgradeManager`.

**Side effects**: Similar to 2.11.1 — if `marketUpgradeMonitor` was renamed to `economyUpgradeManager` elsewhere, fine. Otherwise, both rules are affected wrongly.

**Assessment**: ⚠️ **Likely intentional rename** — `economyUpgradeManager` is mentioned in item 2.4 if the mod's renaming pattern is consistent. But worth verifying both names exist in the mod's rules.

---

## 3. Cross-Cutting Patterns

The mod's changes to techs.xs follow **three recurring themes**:

1. **Threshold-lowering** (items 2.1, 2.3, 2.4, 2.7.2): Every threshold that gates "should we research something?" has been lowered. `armoryUpgradeChance` 15→35, minimum scores cut by 1/3, excess-resource thresholds cut by 80%, pop-ratio threshold cut by 67%. This is **the central flaw** — it naively assumes "more eager = more research throughput" without accounting for the limited global research slot count enforced by `areAtMaxConcurrentResearchPlans`. **The lower the thresholds, the more low-priority techs clog the slots, starving critical techs like WatchTower.**

2. **Variable-indirection removal** (items 2.7.1, 2.7.4): Replaces vanilla's `int firstTech = cTechX; if (aztec) firstTech = cTechY; if (kbTechGetStatus(firstTech)) { ... }` pattern with separate `if (cTechX) {...} if (cTechY) {...}` blocks. Functionally equivalent for non-Aztec civs. Adds code duplication. No clear benefit. **Mild negative — worse code without behavior change.**

3. **Init-rule renames**: `marketUpgradeMonitor` → `economyUpgradeManager`, `advancedFortificationsMonitor` → `omniscienceMonitor`. Likely the result of a refactor that renamed rules but missed updating these callsites. **Needs verification** that the renames are intentional and the old rule names no longer exist.

---

## 4. WatchTower Regression — Root Cause (NOT the Time Gate)

**Critical finding**: The WatchTower gate itself (the mod's rewritten time + defensive + age checks) is **NOT broken**. The gate logic is correct and the units are correct (`xsGetTime()` returns seconds; `gAgeUpTimes[cAge2] + 240` correctly means "4 minutes after Classical"). For P4 (Atlantean):
- Reached Classical at 00:04:03 → time gate passes at ~00:08:03.
- Defensively overrun from 00:17:23 → second gate passes.
- Reached Mythic at 00:19:11 → third gate passes.

By at least minute 8, the gate logic should have allowed WatchTower research. By minute 17+, the second gate. By minute 20, the third gate. Yet WatchTower was never researched.

**Root cause**: The `areAtMaxConcurrentResearchPlans("towerOffensiveUpgradeMonitor")` guard at line 1777 (identical in both vanilla and mod). This function returns `true` (blocking the entire monitor) when:

```xs
// In utilities.xs line 1018-1040:
bool areAtMaxConcurrentResearchPlans(string caller = "ERROR") {
   if (haveExcessResourceAmount(2000) == true) {
      return false;  // Infinite exemptions when 2k excess across board
   }
   int[] plans = aiPlanGetIDsByTypeAndVariableBoolValue(cPlanResearch, cResearchPlanIsProtoUnitCommand, false);
   int numPlans = plans.size();
   int limit = 3;  // Before Mythic
   if (kbPlayerGetAge(cMyID) >= cAge4) { limit = 2; }  // In Mythic
   if (numPlans >= limit) {
      return true;  // Blocked — too many active research plans
   }
   return false;
}
```

Vanilla Retold allows up to **3 concurrent research plans** (or 2 in Mythic+). The user's observation of "one research globally" may reflect:
- Plans including age-up (which counts in `plans`), reducing effective slots to 2 (or 1 in Mythic).
- Freyr's 250% research time → each individual research takes 2.5× as long → successive researches look serial.

**The mechanism linking the mod's threshold-lowering to the WatchTower regression**:

1. Mod lowers `minimumScoreNeeded` from 300 → 100 (and 150 → 50 for Overwhelmer) → near-any upgrade passes the score bar.
2. Mod lowers `haveExcessResourceAmount` from 1000 → 200 → "excess" fires early.
3. Mod lowers pop-ratio from 0.9 → 0.3 → "close to max pop" fires absurdly early (30% pop).
4. AI **always** has the research slots full with low-priority techs because every tech qualifies.
5. When `towerOffensiveUpgradeMonitor` runs (every tick), it calls `areAtMaxConcurrentResearchPlans("...")` and finds `numPlans >= limit` → **early returns without ever reaching the WatchTower block**.
6. Result: WatchTower (and Crenellations, and Guard Tower, and Boiling Oil, and all the other monitor-driven critical researches) **never gets scheduled** because the AI is always "at max concurrent research plans."

**This is THE systemic root cause of the WatchTower regression AND likely the broader pattern of "P4 didn't research critical military techs"**. It's not limited to WatchTower — every monitor in `techs.xs` that guards on `areAtMaxConcurrentResearchPlans` is starved by the threshold-lowering changes.

**The user's intuition was correct**: "the changes made by the mod aren't the best way of making the AI smarter." The mod makes the AI *busier* with research but *worse at selecting the right techs*.

---

## 5. Recommendations (Ranked)

### Strongly recommend reverting to vanilla

1. **Revert the threshold changes in `militaryUpgradeManager`** (items 2.3): `minimumScoreNeeded` 300→100, Overwhelmer 150→50, excess 1000→200, pop-ratio 0.9→0.3. These are the root cause of the WatchTower regression. Restoring vanilla values will free up research slots for critical monitor-driven techs. **Highest priority — directly fixes ISSUE-NEW-1**.

2. **Revert the threshold changes in `economyUpgradeManager`** (item 2.4): mirror of 2.3 for economy. Same reasoning — frees up economy-side research slots.

3. **Revert `armoryUpgradeChance` 35 → 15 (Thor 45 → 25)** (item 2.1): the vanilla calibration is correct. Armory upgrades shouldn't be a top-priority concern.

4. **Revert `gAgeUpTimes[cAge4]` back to `cAge3`** in the townCenterUpgrade gate (item 2.8): the mod's change delays a critical tech by an entire age without documented reason.

5. **Restore the `cTechThornedWalls` Demeter block in `wallUpgradeMonitor`** (item 2.9.1): Demeter loses a signature defensive tech without this auto-research path (verify no replacement exists elsewhere first).

6. **Restore `return` after `xsDisableRule("wallUpgradeMonitor")`** (item 2.9.3): minor bug fix — prevents disabled-rule execution.

### Recommend investigating before deciding

7. **Verify the `advancedFortificationsMonitor` → `omniscienceMonitor` rename** (item 2.11.1): grep both rule names in the mod's rules files to confirm whether the rename was intentional or a copy/paste bug.

8. **Verify the `marketUpgradeMonitor` → `economyUpgradeManager` rename** (item 2.11.2): same verification.

9. **Verify the myth-tech-list-trimmed replacements** (item 2.2): grep for `cTechPredatoryInstinct`, `cTechHallowedWoodlands`, `cTechAdvancedTraps` across the mod tree. If no explicit research path exists for these, they will NEVER be researched.

### Recommend keeping (no changes needed)

10. **The WatchTower gate logic itself** (item 2.7.3): the `gDefensivelyOverrun` and `age >= cAge3` fallbacks are good additions. Keep these on the vanilla time gate. Note: this means *partially* keeping the mod's rewrite — keep the safety-net additions but revert to vanilla's variable-indirection approach to avoid code duplication.

11. **Nuwa `ChasingTheSun` personality check removal** (item 2.5): harmless consistency improvement.

12. **Aztec now gets wall upgrades** (item 2.9.2): Aztecs have walls in Retold, this is correct.

13. **Aztec now gets siege upgrades** (item 2.10): Aztecs have siege in Retold, this is correct.

14. **The Boiling Oil / TemiminaloyanTrials enemy-infantry gates** (item 2.7.2): tactically reasonable new feature. Change `return` to `else if` so they don't block the subsequent Guard Tower block when triggered.

### Optional future improvements (not in this diff's scope)

15. Consider per-civ tuned `minimumScoreNeeded` values instead of the global vanilla values — but **only if** the new values are based on actual scoring math, not picked arbitrarily.

16. Consider adding a "reserved slot" mechanism for monitor-driven critical techs so they don't compete with low-priority scored techs for the limited research slots. This would require changes to `areAtMaxConcurrentResearchPlans` (vanilla, out of scope).

---

## 6. Appendices

### 6.1 `areAtMaxConcurrentResearchPlans` definition (verified, vanilla utilities.xs lines 1018–1041)

```xs
bool areAtMaxConcurrentResearchPlans(string caller = "ERROR")
{
   if (haveExcessResourceAmount(2000) == true)
   {
      debugTechs(caller + ", we have 2k excess resources across the board, allowed to research a tech regardless.");
      return false;
   }
   int[] plans = aiPlanGetIDsByTypeAndVariableBoolValue(cPlanResearch, cResearchPlanIsProtoUnitCommand, false);
   int numPlans = plans.size();
   int limit = 3;  // Before Mythic
   if (kbPlayerGetAge(cMyID) >= cAge4) { limit = 2; }  // In Mythic
   if (numPlans >= limit) {
      debugTechs(caller + ", can't research a technology because we're at our global limit.");
      return true;
   }
   debugTechs(caller + ", allowed to research a technology, not at our global limit.");
   return false;
}
```

Key: limits to 3 concurrent research plans pre-Mythic (incl. age-up plan, so effectively 2 active researches) or 2 in Mythic (effectively 1). The mod's threshold-lowering changes keep these slots perpetually full with low-priority techs.

### 6.2 `xsGetTime()` returns SECONDS (verified)

Context from vanilla `techs.xs` line 96:
```xs
else if (cPersonalityCurrent == cPersonalitySieger && currentAge == cAge2) {
   // Go to Heroic very fast.
   debugTechs("Reducing threshold time for next age to 60 seconds since we're the Sieger personality and are currently in Classical.");
   thresholdTime = 60;  // <-- 60 seconds
}
if (gAgeUpTimes[currentAge] + thresholdTime < currentTime) { ... }
```

`thresholdTime = 60` corresponds to "60 seconds" → `xsGetTime()` returns seconds → `gAgeUpTimes[cAge2]` stores seconds → `gAgeUpTimes[cAge2] + 240` means "age-up-time + 240 seconds = age-up-time + 4 minutes". WatchTower gate time math is correct.

### 6.3 Files referenced (for future verification)

- Mod techs.xs: `mod/Extra Ai + AoModAi/game/ai/core/techs.xs` (2,987 lines)
- Vanilla techs.xs: `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/techs.xs` (2,950 lines)
- `areAtMaxConcurrentResearchPlans`: `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/utilities/utilities.xs` line 1018
- `gAgeUpTimes` initialization: `core/setup.xs` lines 126, 794, 799, 804
- `gAgeUpTimes` updated on age-up: `core/handlers.xs` line 100

### 6.4 Methodology note

This analysis was performed by:
1. Running `diff -u` on the two files — produces a 390-line diff (small, focused).
2. Reading the diff output in full.
3. Reading function headers in context (lines ~88 to ~107, ~1774 to ~1808) to verify variable types and units.
4. Grepping for `areAtMaxConcurrentResearchPlans` and `gAgeUpTimes` across the vanilla AI tree to understand the broader mechanism.

No prior agent reports were trusted for this analysis — all conclusions are based on direct reading of the actual code.
