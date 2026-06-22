# AoModAi Behavioral Exploration

**Change:** `aom-retold-aomodai-port`  
**Date:** 2026-06-18  
**Source folder:** `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/`  
**User intent:** Map what AoModAi (classic AoM) does file-by-file so the user can later decide which behaviors to port into `mod/Extra Ai + AoModAi/` for AoM:Retold.  
**Scope reminder:** This document is **descriptive only**. No porting decisions are made here. Step 2 (`sdd-propose`) will own the selection.

---

## 1. File inventory

| File | Bytes | Lines | Responsibility | Behaviors it owns |
|------|-------|-------|----------------|-------------------|
| `AoModAI.xs` | 139,798 | 3,806 | Main entry point. Declares the global `extern` state, wires age handlers, picks target players, runs per-civ (`initGreek/Egyptian/Norse/Atlantean/Chinese`), enables rules for walls/military/econ, and handles wonders/relics/titans. | Overall orchestration; target-player selection (`updatePlayerToAttack` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:410`); age-up event binding; enables all subsystems. |
| `AoModAIBasics.xs` | 80,362 | 2,004 | Low-level helper library: unit queries, base creation, plan factories (`createDefOrAttackPlan`), `FindSaferTC`, distance utilities, garrison/eject helpers (mostly disabled). | Underpins migration (`FindSaferTC` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBasics.xs:1712`), attack/defend plan creation, and base management. |
| `AoModAIBuild.xs` | 148,288 | 3,663 | Every building rule: houses, settlements, temples, armories, farms, markets, wonders, towers, fortresses, siege camps, ring walls, and special map buildings. | Layered walls (`createCommonRingWallPlan` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:678`); forward siege support (`SupportUnits` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:3004`); base/ally fortification; build-limit management. |
| `AoModAIEcon.xs` | 104,938 | 2,706 | Economy core: resource breakdowns, farming, scouting, fishing, trade, herding, and base migration. | Migration (`relocateFarming` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIEcon.xs:878`, `changeMainBase` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIEcon.xs:740`); multi-base farm distribution; trade; map-adaptive nomad/Vinland handling. |
| `AoModAIGPs.xs` | 84,056 | 2,069 | God-power setup by age, town-defense GP, heavy-GP combos, and many per-power special cases. | GP combos (`SetSpecialGP` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIGPs.xs:1965`); town-defense GP (`findTownDefenseGP` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIGPs.xs:1535`); age-specific GP plans. |
| `AoModAIMapSpec.xs` | 28,762 | 714 | Map-type detection and special-map overrides (water, nomad, Vinlandsaga, King of the Hill, etc.). | Map adaptation; mainland migration for transport/nomad maps. |
| `AoModAIMil.xs` | 130,852 | 3,003 | Military coordination: attack plans, defend plans, allied-base defense, raiding, obelisk-clearing, tactical building/titan control. | Smarter attacks (`createLandAttack` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:2244`, `attackEnemySettlement` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:1494`); allied defense (`defendAlliedBase` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:2561`); base-under-attack response. |
| `AoModAINaval.xs` | 16,394 | 449 | Water rules: fishing, docks, transport planning, and naval attack goals. | Naval economy and transport-based map play. |
| `AoModAIPers.xs` | 2,740 | 88 | Interprets the numeric `Personality` global to set the rush/boom/offense/defense sliders (`cvRushBoomSlider`, `cvMilitaryEconSlider`, `cvOffenseDefenseSlider`). | Personality-to-strategy mapping. |
| `AoModAIProgr.xs` | 4,694 | 128 | Age progression plans and un-pause logic. | Paced age-ups (pauses Age 3 briefly for non-Egyptians). |
| `AoModAITechs.xs` | 66,927 | 2,225 | Technology research plan factory and civ-specific tech priorities. | Research priorities, including siege-related upgrades. |
| `AoModAITrain.xs` | 26,011 | 755 | Unit training maintenance rules, including siege-weapon training per civ. | Unit production, siege weapons. |
| `AoModAiExtra.xs` | 89,715 | 2,395 | Reth's extras: ally comms, resource donations, allied support, special map modes (KOTH), wonder race chats, extra micro, Vinland/nomad migration. | Allied donations (`MonitorAllies` + Donations group around `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAiExtra.xs:1685`); AI-to-AI coordination; migration failsafes. |
| `AoModAIAttacker.xs` | 49 | 2 | Stub entry: `extern int Personality = 4; include "AoModAI.xs";` | Personality wiring only. |
| `AoModAIDefault.xs` | 49 | 2 | Stub entry: `extern int Personality = 8; include "AoModAI.xs";` | Random personality (0–5). |
| `AoModAIDefender.xs` | 49 | 2 | Stub entry: `extern int Personality = 1; include "AoModAI.xs";` | Defensive rusher wiring. |
| `AoModAIEconomical.xs` | 50 | 2 | Stub entry: `extern int Personality = 10; include "AoModAI.xs";` | Random economic-ish personality subset. |

**Totals:** 17 `.xs` files, **923,734 bytes**, **24,008 lines** of XS code.  
(Four `.xml` personality files and `AoModAi_publishinfo.txt` are metadata/declarations, not logic.)

---

## 2. Per-behavior deep dives

### 2.1 Migration (base relocation when the original base is destroyed/overrun)

* **Player-observable effect:** When the AI's main farming base loses its settlement or is outnumbered, it shifts the main base and its economy to a safer owned settlement instead of trying to rebuild at the lost location. On Vinlandsaga-style maps it also migrates from the starting island to the mainland once a safe settlement is found.
* **Files + key functions:**
  * `AoModAIEcon.xs` `relocateFarming` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIEcon.xs:878`
  * `AoModAIEcon.xs` `changeMainBase` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIEcon.xs:740`
  * `AoModAIBasics.xs` `FindSaferTC` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBasics.xs:1712`
  * `AoModAiExtra.xs` `VinLandMBChange` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAiExtra.xs:2363`
  * `AoModAIMapSpec.xs` nomad/Vinland handling around `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMapSpec.xs:600`
* **Inputs/triggers:** `relocateFarming` runs every 26 seconds (accelerating to 7 seconds once a problem is detected). It checks whether the current farm base still has one of the AI's own settlements and whether own+ally military near it outnumbers the enemy. After three failed checks it looks for a safer TC using `FindSaferTC`.
* **Edge cases handled:**
  * If the AI has no settlements left, it stops farming (`gFarmBaseID = -1`).
  * On Vinlandsaga maps it avoids migrating until the mainland has been found (`VinOkToChange`).
  * Old defend plans and wall plans are destroyed so they do not anchor to the lost base.
* **How it works:** The rule scores every owned settlement by own buildings, military, villagers, walls, and nearby enemy strength, then picks the highest scorer. `changeMainBase` moves the KB main base pointer to that settlement, rebuilds the front vector and military gather point, removes old farm/favor breakdowns, recreates them at the new base, and clears stale defense/wall plans. This is a **definite** behavior: the user-described migration is directly implemented.

---

### 2.2 Smarter military decisions

* **Player-observable effect:** The AI picks a target player dynamically, masses before attacking, cancels attacks when the enemy has titans nearby or is storming its own base, and can switch targets if a vulnerable enemy settlement appears close to home. It also shares the current target with AoModAI allies.
* **Files + key functions:**
  * `AoModAI.xs` `updatePlayerToAttack` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:410`
  * `AoModAIMil.xs` `createLandAttack` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:2244`
  * `AoModAIMil.xs` `attackEnemySettlement` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:1494`
  * `AoModAIMil.xs` `monitorAttPlans` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:395`
  * `AoModAIBasics.xs` plan helpers (e.g. `createDefOrAttackPlan`)
* **Inputs/triggers:**
  * Target refreshes every 27 seconds, randomized every ~10–15 minutes.
  * `createLandAttack` runs every 53 seconds; `attackEnemySettlement` every 29 seconds.
  * Attacks are gated by `ReadyToAttack()`, `AvailableUnitsFromDefPlans()`, age, rush counts, and enemy-threat checks (titans near main/def base, own base under heavy attack, wonder-defense in progress).
* **Edge cases handled:**
  * Transport maps restrict attacks to reachable area groups.
  * Stalled attack plans are destroyed after 3–5 minutes of gathering.
  * Plans that lose most of their units retreat or are killed.
* **How it works:** `updatePlayerToAttack` either uses `cvPlayerToAttack` or calls the engine `aiCalculateMostHatedPlayerID`, then broadcasts the target to allies. Attack rules take units currently sitting in defense plans and form them into `cPlanAttack` plans aimed at the most hated player or the nearest enemy settlement. A continuous monitor re-prioritizes, retreats damaged plans, and destroys empty ones. This is **definite**.

---

### 2.3 Sieges (forward towers / fortresses + protection for ranged/siege units)

* **Player-observable effect:** The AI drops shooting buildings (towers, fortresses, mirror towers) near its forward military units in enemy territory, giving ranged attackers and siege a protected firebase. It also reserves at least one siege unit for settlement-attack plans.
* **Files + key functions:**
  * `AoModAIBuild.xs` `SupportUnits` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:3004`
  * `AoModAIBuild.xs` `buildFortress` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:1380`
  * `AoModAIMil.xs` `attackEnemySettlement` (`numSiegeInAttackPlan` handling) at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:1650`
  * `AoModAITrain.xs` siege-weapon training logic around `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAITrain.xs:477`
* **Inputs/triggers:** `SupportUnits` runs every 10 seconds in Age 3+. It looks for a live ranged-attack military unit (`cUnitStateAlive, cActionRangedAttack`) and checks that: enemy forces nearby are light, own+ally forces dominate, the site is not within 50 meters of the main base, there is no market already there, and there are fewer than 8 friendly shooters nearby.
* **Edge cases handled:**
  * Skips entirely on transport maps.
  * Caps concurrent plans at 3.
  * Deletes idle far-away towers/forts to stay under build limits.
* **How it works:** The rule creates a high-priority build plan centered on the live forward unit, weighted heavily toward nearby military and lightly penalized by existing shooters. The building type is chosen randomly among fortress, tower, and (for Helios Atlanteans) mirror towers. Whether the protected unit is specifically a *long-range siege weapon* or just any ranged attacker depends on what unit triggered the scan; the code deliberately uses `cUnitTypeMilitary`/`cUnitTypeBuildingsThatShoot`, so the building supports the forward force in general. The settlement-attack plan separately forces 1–4 siege weapons. This is **definite** for forward firebases; **probable** for the narrower "park long-range siege" phrasing.

---

### 2.4 Layered walls

* **Player-observable effect:** The AI builds a ring of cheap walls around its main base, then a second ring, plus smaller rings around each secondary settlement, and ring walls around human allies' bases.
* **Files + key functions:**
  * `AoModAIBuild.xs` `createCommonRingWallPlan` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:678`
  * `AoModAIBuild.xs` `mainBaseAreaWallTeam1` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:691`
  * `AoModAIBuild.xs` `mainBaseAreaWallTeam2` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:803`
  * `AoModAIBuild.xs` `otherBaseRingWallTeam1`/`otherBase1RingWallTeam`/etc. at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:885` and followings
  * `AoModAIBuild.xs` `WallAllyMB` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:2828`
* **Inputs/triggers:** Enabled in the Age 2 handler (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:2782`). Each wall rule polls on a few-second interval once active, but is gated by age, resources, villager count, and whether the base is currently being overrun.
* **Edge cases handled:**
  * Rushes / early game: skipped before 15 minutes if the personality is a rusher.
  * Wall plans are destroyed if the base is under heavy attack and outnumbered (so villagers are not trapped building walls).
  * Each plan has a hard 12-minute lifetime.
* **How it works:** `createCommonRingWallPlan` calls `aiWallRingAroundPoint(...)` centered on the base, with a radius of 45 for the main base and 21 for other bases. `mainBaseAreaWallTeam1` creates the first ring; `mainBaseAreaWallTeam2` creates the second ring once the first is live. Parallel rules do the same for `gOtherBase1ID` through `gOtherBase4ID`, and `WallAllyMB` does it for human allies. This is **definite**.

---

### 2.5 God-power combos

* **Player-observable effect:** The AI can chain an Age 1 reveal with a Mythic heavy-damage power (Meteor/Tornado) on the same high-value target. It also auto-casts town-defense powers on bases that are under sustained attack.
* **Files + key functions:**
  * `AoModAIGPs.xs` `SetSpecialGP` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIGPs.xs:1965`
  * `AoModAIGPs.xs` `findTownDefenseGP` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIGPs.xs:1535`
  * `AoModAIGPs.xs` `setupGodPowerPlan` / `rAge*FindGP` rules
  * `AoModAIMil.xs` `baseAttackTracker` triggers town-defense GP at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:2912`
* **Inputs/triggers:** `SetSpecialGP` runs every 14 seconds. It triggers only if the Age-4 god-power slot is `cTechMeteor` or `cTechTornado` and the Age-1 slot is `cTechVision`. The target is either an enemy Titan Gate or the enemy's main TC (only if the TC area has at least 5 enemy farms). It first casts `Vision` on the target; once the location is visible, it casts the heavy power.
* **Edge cases handled:**
  * After 75 minutes the AI stops targeting titan gates and settles on the enemy TC.
  * If casting fails five times it falls back to the normal `castHeavyGP` rule.
* **How it works:** The rule is an explicit two-step script: reveal a target with `aiCastGodPowerAtPosition(cTechVision, ...)` and then, once `kbLocationVisible` is true, cast the Age-4 nuke (`Meteor` or `Tornado`) at the same point. `findTownDefenseGP` similarly re-targets the best available defensive god power to a base currently being attacked. This is **definite**, with one clarification:
* **Clarification / possible conflict:** The concrete combo in the code is **`cTechVision` → `cTechMeteor/Tornado`**. The user mentioned "Set's Reveal in Archaic → Meteor in Mythic". Whether "Reveal" maps to `cTechVision` depends on the exact civ tables; the code only tests for `Vision`, not a generic Reveal power.

---

## 3. Significant behaviors not in the user's list

### 3.1 Allied base defense

* **Observable:** Sends defensive armies to an ally's main base when the ally is outnumbered there; can also train a unit at a secondary base that is under attack.
* **Files + functions:** `defendAlliedBase` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:2561`), `baseAttackTracker` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:2912`), `WallAllyMB` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:2828`), `MonitorAllies` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAiExtra.xs:1685`).
* **Confidence:** definite.

### 3.2 Resource donations to allies

* **Observable:** Spare food, wood, and gold are sent to allies. Big lump sums are sent to help allies age up; smaller aid is sent to allies that look like they are dying.
* **Files + functions:** Donations rule group inside `AoModAiExtra.xs` (around `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAiExtra.xs:1050`); `MonitorAllies` enables it.
* **How it works:** Uses `aiTribute` when resources exceed ~2k; if both players use AoModAI it uses comms messages (`MessagePlayer`) instead of tribute.
* **Confidence:** definite.

### 3.3 Multi-base economy / per-base farming

* **Observable:** Farms are spread across the main base and up to four secondary settlements; when a base is lost the farming breakdown is moved.
* **Files + functions:** `updateFoodBreakdown` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIEcon.xs:295`), `relocateFarming`, `otherBasesDefPlans` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:1155`).
* **Confidence:** definite.

### 3.4 Obelisk-clearing and raiding parties

* **Observable:** A dedicated small plan clears enemy obelisks, and a separate smaller force raids the enemy.
* **Files + functions:** `activateObeliskClearingPlan` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:1042`), `createRaidingParty` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIMil.xs:2017`).
* **Confidence:** definite.

### 3.5 Wonder / KOTH handling

* **Observable:** Detects wonders being built, builds its own wonder, defends allied wonders, attacks enemy wonders, and builds defensive towers/bunkers around the Plenty Vault on King of the Hill.
* **Files + functions:** `watchForFirstWonderStart/Done`, `watchForWonder`, `BunkerUpThatWonder`, KOTH rules in `AoModAiExtra.xs`.
* **Confidence:** definite.

### 3.6 Map auto-detection for nomad / migration

* **Observable:** Detects nomad and migration maps, pauses age-ups, builds an initial island base, sends the transport toward the mainland, and switches the main base after landing.
* **Files + functions:** `preInitMap` and related rules in `AoModAIMapSpec.xs`, `AutoDetectMap` block in `AoModAiExtra.xs` around `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAiExtra.xs:522`, `VinLandMBChange`.
* **Confidence:** definite.

### 3.7 Build-limit-aware tower/fort management

* **Observable:** Old, idle towers and fortresses far from the action are deleted to make room for new forward ones.
* **Files + functions:** `SupportUnits`, `buildFortress`.
* **Confidence:** definite.

---

## 4. Architectural notes

### Entry point

The executable entry point is `void main(void)` in `AoModAI.xs` at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:3733`. It seeds randomness, calculates map areas, calls `preInitMap()`, `persDecidePersonality()`, and then `init()`.

### File relationships

`AoModAI.xs` is the only file with a `main()`. It directly `include`s all other logic files in this order (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:365`):

```text
AoModAIBasics.xs
AoModAIBuild.xs
AoModAIEcon.xs
AoModAiExtra.xs
AoModAIGPs.xs
AoModAIMapSpec.xs
AoModAIMil.xs
AoModAINaval.xs
AoModAIPers.xs
AoModAIProgr.xs
AoModAITechs.xs
AoModAITrain.xs
```

The four stub files (`AoModAIAttacker.xs`, `AoModAIDefault.xs`, `AoModAIDefender.xs`, `AoModAIEconomical.xs`) reverse the dependency: they set the `Personality` integer and then `include "AoModAI.xs"`.

### How the personality XMLs wire in

The `.xml` files only select which stub script to load:

| XML file | `<script>` | `Personality` value set in stub |
|----------|-----------|--------------------------------|
| `aomxai001 AoModAI.xml` | `AoModAIDefault` | 8 (random 0–5) |
| `aomxai002 AoModAI Attacker.xml` | `AoModAIAttacker` | 4 (Aggressive Rusher) |
| `aomxai002 AoModAI Defender.xml` | `AoModAIDefender` | 1 (Defensive Rusher) |
| `aomxai002 AoModAI Economic.xml` | `AoModAIEconomical` | 10 (random among defensive boomer, economic boomer, aggressive boomer) |

`AoModAIPers.xs` then maps the integer `Personality` to the three sliders (`cvRushBoomSlider`, `cvMilitaryEconSlider`, `cvOffenseDefenseSlider`) at `/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAIPers.xs:12`.

### Event-driven vs polling

* **Polling:** Most activity is driven by `rule ... minInterval N` blocks that run repeatedly. Examples: `updatePlayerToAttack`, `buildFortress`, `SupportUnits`, `createLandAttack`, `relocateFarming`.
* **Event-driven:**
  * Age transitions call handlers registered with `aiSetAgeEventHandler` in `init()` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:2266`).
  * God-power casts trigger `gpHandler()` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:3753`).
  * Resign events trigger `resignHandler()` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:3433`).
  * As a result the design is a **hybrid**.

### Cross-civ vs per-civ split

`init()` contains a `switch (cMyCulture)` that calls dedicated setup functions:

* `initGreek()` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:1827`)
* `initEgyptian()` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:1872`)
* `initNorse()` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:1933`)
* `initAtlantean()` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:2004`)
* `initChinese()` (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:2074`)

Culture checks appear throughout the rules (e.g. Atlanteans use one-builder plans, Norse skip villager thresholds for walls, Egyptians have special dropsite rules, Chinese heroes are maintained separately).

### Config / flag patterns

The code uses two main families of globals:

* **`cv*`** "control variables" that the personality/map setup can tune:
  * `cvOkToBuildWalls` / `cvOkToUseAge*GodPower` / `cvRushBoomSlider` / `cvMilitaryEconSlider` / `cvOffenseDefenseSlider` / `cvSliderNoise` / `cvPlayerToAttack` / `cvRandomMapName` / `cvMapSubType`.
* **`g*`** runtime globals that track plan IDs and state:
  * `gWallPlanID`, `gRushGoalID`, `gLandAttackGoalID`, `gDefendPlanID`, `gTownDefenseGodPowerPlanID`, `gOtherBase1ID`…`gOtherBase4ID`, etc.

Many rules also use `static` local variables to keep per-rule state (timers, counts, once-only flags).

### Debug / diagnostics

Three echo flags drive diagnostic chat:

* `ShowAiEcho` — general AI status prints.
* `ShowAIDebug` / `ShowAIDebugEchoes` — detailed rule-level diagnostics.
* `ShowAIComms` — prints ally-communication events.

Heavy use of `aiEcho(...)` is present; these can be toggled off for "online-friendly" play.

---

## 5. XS language notes relevant to porting

* **Include model:** XS uses `include "filename.xs";` without `#` prefix (`/home/houtamelo/Documents/projects/aom_retold_mod/extracted/mods_AoModAi/AoModAi\ai2\AoModAI.xs:365`).
* **Global sharing:** Globals are declared `extern` once and reused across included files. Stubs set a few globals (`Personality`) before including the main file.
* **Rules:** `rule name minInterval N { active | inactive }` defines polling rules that get enabled/disabled at runtime.
* **Static locals:** `static int x = -1` inside a rule persists across rule invocations, which is used heavily for timers and latch flags.
* **Engine API calls:** The code relies on `kb*`, `ai*`, and `xs*` functions (`kbBaseSetMain`, `aiPlanCreate`, `aiCastGodPowerAtPosition`, `aiWallRingAroundPoint`, `xsEnableRule`, etc.).
* **Retold compatibility watch points:**
  * Tale-of-the-Dragon constants are used: `cUnitTypeSiegeCamp`, `cUnitTypeTowerMirror`, `cUnitTypeHeroChineseGeneral`, `cUnitTypeHeroChineseMonk`, `cPowerYearOfTheGoat`, `cPowerTsunami`, `cPowerRecreation`, etc. Retold may have renamed or removed some of these; each would need to be verified against Retold's `globals.xs`/`units.json`.
  * `aiWallRingAroundPoint` is a classic-era helper. Confirm it exists under the same name and with the same signature in Retold.
  * Map names such as `"river styx"`, `"anatolia"`, `"king of the hill"`, `"nomad"` may differ in Retold and need normalization.

---

## 6. What could not be determined from static reading

* Full per-power logic inside `setupGodPowerPlan` in `AoModAIGPs.xs`. It is a ~900-line `if/else` chain; only the top-level structure and the explicit `Vision → Meteor/Tornado` combo were traced.
* Exact behavior of helper functions that are defined elsewhere and used as guards: `ReadyToAttack()`, `ShouldIAgeUp()`, `AvailableUnitsFromDefPlans()`, `createDefOrAttackPlan()` internals, and `initRethlAge1()`.
* Runtime-only knowledge such as whether a given civ's Age-1 power truly equals `cTechVision` or `cTechReveal`, and whether `aiWallRingAroundPoint` behaves identically in Retold.
* The full meaning of several user-variable slots (e.g. `aiPlanSetUserVariableInt(..., 0, 250)`) and the `gSomeData` plan used as a key/value store; these are author-specific conventions.
* The intended semantics of the missing change-log file referenced in `AoModAi_publishinfo.txt`.
* Many map-specific branches (KOTH water variant, King of the Hill, Vinlandsaga, etc.) were identified but not exhaustively traced.

---

## Key takeaways for the next phase

* **AoModAi is a monolithic, classic-AoM AI** split into 12 logic modules plus 4 tiny entry stubs. Porting "a behavior" likely means extracting a rule + its helper functions from one or two files, not copying whole files.
* **Worth noting:** Some celebrated behaviors are tightly coupled. Migration depends on the `FindSaferTC` scorer and `changeMainBase`; layered walls depend on `aiWallRingAroundPoint`; the GP combo depends on exact engine power constants.
* **Potential friction for Retold:** TotD/Chinese-specific constants and map-name strings need cross-checking before any code is reused as-is.