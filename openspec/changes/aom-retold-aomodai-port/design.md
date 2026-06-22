# Design: AoModAi layered walls port into Extra Ai + AoModAi

## 1. Architecture

A single polled rule `secondRingWallPlanMonitor` is added to `mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs`, placed immediately after the existing `wallManager` rule (after line 1058) and before `templeMonitor`. It re-uses the same `cPlanBuildWall` plan type that `wallManager` uses, but creates the second ring explicitly because Retold's `wallManager` skips any base that already has a wall plan.

The rule has three implicit states:
- **WAITING** — no active second-ring plan; it checks for a first-ring plan and gating conditions.
- **ACTIVE** — a second-ring plan exists and is monitored for cancellation triggers.
- **DESTROYED** — plan cancelled by attack, gold drop, or 12-minute lifetime; a re-creation cooldown is recorded and the rule re-enters WAITING.

The rule runs in `group defaultClassicalRules` with `minInterval 5` for responsiveness, mirroring existing building managers. It observes `cStrategyFlagBuildWalls` and exits immediately when walls are disabled.

## 2. File changes

| File                                                       | Action | Location                                                     | Description                                                                                                                             |
| ---------------------------------------------------------- | ------ | ------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| `mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs` | Modify | Top of file after `startTimeNoExtraBuildingsNeeded` (line ~22) | Add state globals and `mRusher`.                                                                                                          |
| `mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs` | Modify | After `wallManager` (line ~1058)                               | Insert one-shot init rule `secondRingWallInit` and polled rule `secondRingWallPlanMonitor` with helper functions, wrapped in audit markers. |
| `scripts/deploy-mods.sh`                                     | None   | —                                                            | Already deploys the whole `Extra Ai + AoModAi/game/ai` tree via `deploy_tree`; no script change needed.                                     |

All inserted code is wrapped in:

```xs
// === AoModAi: layered walls begin ===
...
// === AoModAi: layered walls end ===
```

The file is saved with CRLF line endings to match the existing file convention.

## 3. State variables

Declared near the existing globals at the top of `buildings.xs`:

```xs
// === AoModAi: layered walls state begin ===
extern bool mRusher = false;                      // True for rusher personalities.
extern int  gSecondRingWallPlanID = -1;           // Active 2nd-ring plan, -1 if none.
extern int  gSecondRingWallStartTime = -1;        // xsGetTime() when plan was created.
extern int  gSecondRingWallLastDestroyedTime = -1;// Cooldown marker for re-creation.
extern int  gSecondRingAttackStartTime = -1;      // xsGetTime() when sustained attack started.
// === AoModAi: layered walls state end ===
```

`mRusher` is initialized once by the `secondRingWallInit` rule. `gSecondRingAttackStartTime` is set/reset by `isBaseUnderSustainedAttack()`.

## 4. mRusher decision

**Chosen option: A** — introduce a dedicated global `bool mRusher` and set it in a one-shot init rule.

| Option | Choice                                              |
| ------ | --------------------------------------------------- |
| A      | Introduce `mRusher` global + init rule.               |
| B      | Read `cPersonalityCurrent` inline in every wall rule. |
| C      | Defer rusher detection.                             |

**Rationale:** Option A matches AoModAi's pattern, keeps the personality check in one place, and lets future wall rules read the same flag without repeating the mapping. The init rule is a single rule that disables itself after setting the flag.

**Default mapping:**

```xs
rule secondRingWallInit inactive group defaultArchaicRules minInterval 1
{
   // AoModAi Personality 4 = Aggressive Rusher. Retold's semantic equivalent is Attacker.
   mRusher = (cPersonalityCurrent == cPersonalityAttacker);
   xsDisableRule("secondRingWallInit");
}
```

> **OPEN:** The personality-to-rusher mapping may be overridden. Default: `cPersonalityAttacker`. Alternative: include `cPersonalityConqueror` or other aggressive personalities.

## 5. Rule logic

```xs
rule secondRingWallPlanMonitor inactive group defaultClassicalRules minInterval 5
{
   if (checkStrategyFlag(cStrategyFlagBuildWalls) == false) return;

   int mainBaseID = kbBaseGetMainID(cMyID);
   if (kbBaseGetIsIDValid(cMyID, mainBaseID) == false) return;
   if (mainBase on unreachable area group) return;

   // Rusher delay gates the second ring.
   if (mRusher == true &&
       (kbGetAge() < cAge3 || xsGetTime() < 15*60*1000))
   {
      debugSecondRing("rusher delay: waiting until Age 3 + 15 min");
      return;
   }

   if (gSecondRingWallPlanID != -1)
   {
      maintainSecondRingPlan(mainBaseID);
      return;
   }

   // WAITING: need a first-ring plan before creating the second ring.
   if (aiPlanGetNumber(cPlanBuildWall, -1, true) == 0) return;

   // Gating defaults (mirroring AoModAi).
   if (kbGetAge() < cAge2) return;
   if (xsGetTime() < 8*60*1000) return;
   if (kbUnitCount(cMyID, cUnitTypeAbstractVillager, cUnitStateAlive) < 10) return;
   if (kbResourceGet(cResourceGold) < 150.0) return;

   // Re-creation delay after a destruction.
   if (gSecondRingWallLastDestroyedTime != -1 &&
       xsGetTime() < gSecondRingWallLastDestroyedTime + 60*1000) return;

   gSecondRingWallPlanID = createSecondRingWallPlan(mainBaseID);
   if (gSecondRingWallPlanID != -1)
      gSecondRingWallStartTime = xsGetTime();
}
```

## 6. Helpers

| Helper                                          | Purpose                                                                                                                                                    |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `int createSecondRingWallPlan(int baseID)`        | Creates a `cPlanBuildWall` ring plan at radius **50**, centered on `kbBaseGetLocation(cMyID, baseID)`, with Norse/non-Norse builder logic copied from `wallManager`. |
| `void destroySecondRingWallPlan(string reason)`   | Destroys the active plan, resets the plan ID/start time, records the destruction time for cooldown, and echoes the reason.                                 |
| `bool isBaseUnderSustainedAttack(int baseID)`     | Returns true when the base has been under attack (per `gEnemyPowerInBases`) for more than 25 s.                                                          |

**Note on the attack signal:** AoModAi uses `kbBaseGetTimeUnderAttack(cMyID, baseID)` which returns the number of seconds the base has been under attack. **This API does not exist in Retold** (no `kbBase*UnderAttack*` function is present in Retold's catalog — the closest is `kbBaseGetDefenseRating` which measures base fortification, not active attack). However, Retold exposes the same information via the extern arrays `gDefendTCBases` and `gEnemyPowerInBases` (declared in `core/globals.xs:163-164`, populated by `military_defend.xs` every defense frame). The helper does:

```xs
bool isBaseUnderSustainedAttack(int baseID)
{
   int baseIndex = gDefendTCBases.find(baseID);
   if (baseIndex == -1) return false;        // base not tracked
   if (gEnemyPowerInBases[baseIndex] == 0) {
      gSecondRingAttackStartTime = -1;        // reset timestamp
      return false;
   }
   if (gSecondRingAttackStartTime == -1)
      gSecondRingAttackStartTime = xsGetTime();
   // AoModAi's early-game exemption: don't trigger cancellation before 19 min,
   // otherwise a minor early push would prevent the wall from ever being built.
   if (xsGetTime() <= 19 * 60 * 1000) return false;
   return (xsGetTime() - gSecondRingAttackStartTime) > 25 * 1000;
}
```

**Why this is cleaner than the original helper:** the original design proposed `getEnemyUnitsNearBase` + `getOwnAlliedUnitsNearBase` + `isBaseUnderHeavyAttack` with our own unit-count query and ratio check. Using `gEnemyPowerInBases` reuses Retold's pre-computed enemy-power value (same one used by `military_defend.xs`, `godpowers_*.xs`, `chats.xs`, etc.), removes two helpers, and reduces the risk of subtle bugs from replicating Retold's enemy-power calculation. The sustained-attack timestamp still needs to be tracked locally (Retold doesn't expose it directly), but that's a one-line static.

**What was simplified:** the original design also had a "2× enemy-vs-own-and-ally units" check (AoModAi's "overwhelmed" threshold). With `gEnemyPowerInBases` we have a single power value, not a unit count, so a unit-ratio check is awkward. For the first version, the design uses only the sustained-attack check (25 s). The "overwhelmed" check can be added later as `gEnemyPowerInBases[baseIndex] > availablePowerToDefendWith * gWinningArmyPercentage` (the same idiom Retold uses in `military_defend.xs:731`).

## 7. Edge cases

| Case                              | Handling                                                                |
| --------------------------------- | ----------------------------------------------------------------------- |
| No main base / nomad              | Early-return if `kbBaseGetMainID` is invalid.                             |
| Island / unreachable base         | Same area-group check used by `wallManager`; skip if not connected.       |
| `wallManager` disabled              | Exit on `checkStrategyFlag(cStrategyFlagBuildWalls) == false`.            |
| First ring destroyed              | `aiPlanGetNumber(cPlanBuildWall,...)` returns 0 → no second ring created. |
| 12-minute lifetime                | Destroy plan and set re-creation timer.                                 |
| Gold drops below 150              | Destroy plan and set re-creation timer.                                 |
| Heavy attack > 25 s               | Destroy plan; re-creation only after base is safe and gates hold.       |
| Plan state becomes `cPlanStateDone` | Reset `gSecondRingWallPlanID` to -1 on next tick.                         |
| Coop human allies                 | Ally unit count is included in the denominator of the attack check.     |

## 8. Debug

All second-ring diagnostics are gated by a dedicated flag to avoid chat spam in release:

```xs
extern bool gDebugSecondRing = false;   // Toggle manually.
#define debugSecondRing(msg) if (gDebugSecondRing == true) { aiEcho(msg); }
```

Echoed events: plan created, plan destroyed with reason, rusher delay active, cancellation triggered. Alternatively, reuse the existing `debugMilitaryBuildings()` helper if the mod already provides it, but keep `gDebugSecondRing` as an independent kill-switch.

## 9. Rollback

- Delete the `// === AoModAi: layered walls begin ===` … `end ===` block.
- Remove the state globals block.
- Remove the `secondRingWallInit` rule.
- The remaining `buildings.xs` is byte-identical to its pre-port state except for any unrelated changes.

## 10. Open questions

> **OPEN:** Gating thresholds default to Age 2+, 8 min, ≥ 10 villagers, ≥ 150 gold (AoModAi defaults). Alternative: lower thresholds for shorter playtest matches.

> **OPEN:** 2nd-ring radius defaults to **50** (Retold natural: `30.0 + 1 * 20.0`). Alternative: 45 to match AoModAi's `gMainBaseAreaWallRadius`, or another value.

> **OPEN:** `mRusher` default mapping is `cPersonalityAttacker` (semantic match for AoModAi's Aggressive Rusher / Personality 4). Alternative: include `cPersonalityConqueror`, or switch the default entirely.
