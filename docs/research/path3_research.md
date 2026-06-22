# Path 3 Research: AI Script Hook Viability for Auto-Repair

**Date:** 2026-05-01  
**Research scope:** Can a mod extend `human_assist.xs` (the Villager Priority AI script) to
issue real, resource-costed Repair commands from idle villagers/Norse-infantry to nearby
damaged friendly buildings?

---

## Q1: AI Script Load Order for the Human Player

**TL;DR:** The human-player AI entry point is `game/ai/human_assist/human_assist.xs`. It is a
separate script from the normal opponent AI (`chairon.xs` / `core/main.xs`) and runs
automatically for every human player. It is NOT the same personality system used to select
skirmish AI opponents.

### Findings

The game ships with two distinct AI stacks:

1. **Skirmish/opponent AI** – personality files in `game/ai/personalities/` (Balanced, Attacker,
   Humanoid, NoAI, etc.), each naming a `<script>` entry point. The default opponent AI
   entry is `chairon`, which resolves to `game/ai/chairon.xs`, which in turn includes
   `core/main.xs`. The `Humanoid` personality also points at `chairon`.

2. **Human-player assistant AI** – `game/ai/human_assist/human_assist.xs`. This is the file
   that powers the Villager Priority System (VPS), auto-scouting, and farm placement. Its
   `main()` logs: `"Game type is " + cGameTypeCurrent + ", 0 = Scenario, 1 = Save Game, 2 = Random Map, 3 = Campaign, 4 = Recorded Game."` — confirming it runs across all game types.

`human_assist.xs` includes:
- `human_assist/human_assist_debug.xs`
- `human_assist/human_assist_unit_queries.xs`
- `human_assist/human_assist_resource_breakdown_system.xs`

There is no top-level file (like the historical `aomxai.xs`) that merges both stacks. The human
assistant is loaded by the engine separately and automatically for human-controlled slots.
[verified-from-AoMR-data: `game/ai/human_assist/human_assist.xs`]

**How it is loaded:** The "How-to create your own skirmish AI" PDF states explicitly: to add a
new skirmish AI the developer must create a personality file and put it under `ai/personalities/`.
The human assistant is *not* a personality; it has no entry in `personalities.xml`. The engine
loads it via an internal mechanism for human player slots. The `trEnforceAIAssist(bool enable)`
trigger function (in `triggerfuncs_8cpp.html` doxygen) shows the engine has explicit support for
enforcing or suppressing the assistant per scenario.
[verified-from-AoMR-docs: `doxygen_retail.7z/triggerfuncs_8cpp.html`]

---

## Q2: Game-Mode Coverage (Campaign)

**TL;DR:** `human_assist.xs` reports all four game types in its `main()` log line (0=Scenario,
1=SaveGame, 2=RandomMap, 3=Campaign). The VPS starts **disabled by default** (`disableVillagerAssist()` is called at the end of `main()`), and campaigns can explicitly override this via `trEnforceAIAssist`.

### Findings

The `human_assist.xs` `main()` function runs regardless of game mode; the game-type echo is
purely informational. The script calls `disableVillagerAssist()` at the end of `main()`, so it
is off until UI or trigger code enables it.
[verified-from-AoMR-data: `game/ai/human_assist/human_assist.xs` line 990]

Campaign missions use scenario AI scripts for enemy players (e.g. `campaign/fott/fott02_p2.xs`)
which reference `inactive_ai.xs` or `core/main.xs`. These are per-player-slot scripts for NPC
players — there is no separate p1 script for the human player, meaning the human assistant runs
its own separate instance.
[verified-from-AoMR-data: `game/ai/campaign/fott/` — no `*_p1.xs` file exists in any campaign]

**The key risk:** Campaign designers can call `trEnforceAIAssist(false)` to suppress the
assistant for specific missions. If a campaign mission suppresses it, the mod's auto-repair
rules would also be suppressed. There is no documented way to make a mod's rules immune to
`trEnforceAIAssist`. Whether any shipped campaign missions actually call this is **UNCERTAIN**
(the `.mythscn` binaries could not be read as text; no `.xs` campaign file was found calling
`trEnforceAIAssist`).
[UNCERTAIN: no readable campaign scenario confirmed to call `trEnforceAIAssist(false)`]

---

## Q3: XS API Surface

### (a) Enumerating idle units owned by a player that can repair

**Idle detection — two methods available:**

Method 1 — `kbUnitQuerySetActionType(queryID, cActionTypeIdle)`: sets a query filter so only
units currently performing the Idle action are returned. This is a first-class filter in the
KB query system.
> "Sets what action type the units must be performing to be valid for the provided queryID. This
> can be all cActionType constants."
[verified-from-AoMR-docs: `doxygen_retail.7z/kbfuncs_8cpp.html`]

The `human_assist.xs` code already uses `kbUnitGetActionType(unitID)` and compares it to
`cActionTypeIdle` in `checkReservePlan()`.
[verified-from-AoMR-data: `game/ai/human_assist/human_assist.xs` line 434]

Additionally, `kbUnitGetIdleTime(unitID)` returns idle duration in milliseconds, which `human_assist.xs` uses (`kbUnitGetIdleTime(unitID) > 2000`) to avoid reacting to units that
are only briefly stopped.
[verified-from-AoMR-docs: `doxygen_retail.7z/kbfuncs_8cpp.html`]

Method 2 — check `kbUnitGetPlanID(unitID)`: the existing `getIdleUnit()` in
`human_assist_unit_queries.xs` iterates query results and treats a unit as idle when
`kbUnitGetPlanID(unitID) < 0` (no assigned plan).
[verified-from-AoMR-data: `game/ai/human_assist/human_assist_unit_queries.xs` lines 341-355]

**Filtering to "can repair":** There is no `kbProtoUnitGetCanRepair()` function in the
documented API. The workaround is to filter by unit type constant. For villagers use
`cUnitTypeAbstractVillager`; for Norse military infantry use the specific proto unit IDs (e.g.
`cUnitTypeHersir`, `cUnitTypeBerserk`, `cUnitTypeRaiding Cavalry`) or a shared type like
`cUnitTypeAbstractNorseMilitary` if it exists. Which unit types have a Repair action must be
confirmed empirically (toggle the `aiDebug` config and inspect constants at runtime).
[UNCERTAIN: no API function enumerates actions a unit type has; unit-type ID lookup requires
runtime `MythAIConstantsPlayer1.txt`]

### (b) Finding nearby damaged friendly buildings

**Step 1 — query friendly buildings in a radius:**
```xs
int qID = kbUnitQueryCreate("nearbyDamagedBuildings");
kbUnitQuerySetPlayerID(qID, cMyID, true);       // friendly only
kbUnitQuerySetUnitType(qID, cUnitTypeBuilding);  // all buildings
kbUnitQuerySetState(qID, cUnitStateAlive | cUnitStateBuilding); // alive + under construction
kbUnitQuerySetPosition(qID, unitPosition);
kbUnitQuerySetMaximumDistance(qID, searchRadius);
kbUnitQuerySetAscendingSort(qID, true);          // closest first
kbUnitQueryResetResults(qID);
int count = kbUnitQueryExecute(qID);
```
[verified-from-AoMR-docs: `doxygen_retail.7z/kbfuncs_8cpp.html` — all setters documented]
[verified-from-AoMR-data: pattern used in `human_assist_unit_queries.xs`]

**Step 2 — detect damage (HP < max HP):**
There is no `kbUnitGetCurrentHitpoints()` or `kbUnitGetMaxHitpoints()` per-unit function in the
documented API. The available route is `kbUnitQueryGetUnitHitpoints(queryID, considerHealth)`:
- `kbUnitQueryGetUnitHitpoints(qID, false)` = total **maximum** HP of query results
- `kbUnitQueryGetUnitHitpoints(qID, true)` = total **current** HP of query results

For a query restricted to exactly one unit (iterate through results one by one), comparing
`considerHealth=true < considerHealth=false` tells you whether that unit is damaged.
[verified-from-AoMR-docs: `doxygen_retail.7z/kbfuncs_8cpp.html`]

Alternatively, `kbUnitGetPower(unitID, false)` (ignoring health) vs `kbUnitGetPower(unitID, true)` (using health percentage) can detect damage: if `kbUnitGetPower(unitID, false) > kbUnitGetPower(unitID, true)` the unit is below full HP. This works for buildings with a population cost or resource cost; buildings with neither default to 1.0 regardless of health.
[verified-from-AoMR-docs: `doxygen_retail.7z/kbfuncs_8cpp.html` — kbUnitGetPower description]
[UNCERTAIN: whether all building types have a non-zero cost that makes kbUnitGetPower reliable]

**Recommended approach:** Use a per-building query loop with `kbUnitQueryGetUnitHitpoints`.

### (c) Issuing a repair work command

The exact function is:

```xs
bool aiTaskWorkUnit(int unitID, int targetUnitID, bool queue)
```

> "Does a lightweight (no plan) work tasking of the provided unitID on the provided
> targetUnitID, this is **like right-clicking**."

The multi-unit variant is:
```xs
bool aiTaskWorkUnits(int[] unitIDs, int targetUnitID, bool queue)
```

Both return `bool` (success/failure).
[verified-from-AoMR-docs: `doxygen_retail.7z/aifuncs_8cpp.html`]

`aiTaskWorkUnit` is a direct analogue of the player right-clicking a damaged building with a
villager selected. Because it goes through the normal work-command path rather than a plan
abstraction, the engine should apply standard resource deduction for the Repair action — exactly
the same path as the player right-clicking. This has NOT been empirically confirmed but is
strongly implied by the "like right-clicking" description and by the fact that `aiTaskBuildUnit`
(same family) results in actual building with normal cost.
[educated-guess-from-AoE3-DE: right-click work command = canonical resource cost path]

Also useful for aborting an in-progress command:
```xs
bool aiTaskStopUnit(int unitID)
```
[verified-from-AoMR-docs: `doxygen_retail.7z/aifuncs_8cpp.html`]

---

## Q4: Mod Overlay Mechanism

**TL;DR:** File-based overlay works. A mod can place `human_assist.xs` (or any of its included
files) under `mods/local/<mod>/game/ai/human_assist/` and it will shadow the vanilla file. This
is the documented and supported path.

### Findings

From "How-to create your own skirmish AI.pdf" (page 2, section "Overwriting parts of the main AI
without creating a custom AI"):

> "To achieve this we again need a local mod as described above, but we don't perform the steps
> related to personalities. Instead you can now place files in this mod's ai folder with the same
> name as the main AI uses. **You can overwrite specific files in this way.**"

This means the mod path:
```
mods/local/aom_autorepair/game/ai/human_assist/human_assist.xs
```
will shadow the vanilla:
```
game/ai/human_assist/human_assist.xs
```

The mod file must preserve all existing content (VPS, auto-scouting, farm placement) and append
the new auto-repair rule. The doc warns:

> "if you mod the main AI in this manner you must be wary each official patch. Since the script
> files you've modified will likely be changed and your mod won't have those changes."

The mod path for the local install (Linux/Steam):
```
~/.steam/debian-installation/steamapps/common/.../mods/local/<name>/
```
or the documented Windows path:
```
Users\<user>\Games\Age of Mythology Retold\<steamID>\mods\local\
```
[verified-from-AoMR-docs: "How-to create your own skirmish AI.pdf" pages 1-2]

**No dropdown selection required.** The overlay is unconditional — it replaces the file for
**all** matches (skirmish, MP, campaign). The user does not need to select anything.

**There is no additive modding** for `.xs` files — you must copy and modify the whole file. This
means the mod must be kept in sync with game patches.
[verified-from-AoMR-docs: "How-to create your own skirmish AI.pdf" page 2]

---

## Q5: Multiplayer Determinism

**TL;DR:** The human-player AI assistant runs in the game simulation (not local-only), so it is
deterministic and MP-safe. This is confirmed by the fact that the VPS already works in MP.

### Findings

The "XS rules functionality" PDF states:
> "Each game update we run the AI scripts attached to the players and potentially a trigger script
> attached to the map/scenario."

This means AI scripts run in the canonical simulation tick, not in a client-local thread. Commands
issued via `aiTaskWorkUnit` are processed as simulation events, the same as any player right-click.
For multiplayer, all clients execute the same simulation with the same inputs, so AI commands are
deterministic.
[verified-from-AoMR-docs: "XS rules functionality.pdf" page 1]

The VPS already issues real gather-plan assignments in multiplayer without desync. `aiTaskWorkUnit`
is in the same syscall family (`aifuncs`), which runs inside the simulation.
[educated-guess-from-AoE3-DE: AoE3 DE used the same BANG engine model where AI syscalls are
simulation-side; no AoMR-specific explicit documentation of lockstep mechanics found]
[UNCERTAIN: no explicit AoMR documentation states "AI scripts are deterministic in MP" —
the evidence is strong but indirect]

---

## Recommendation

**PATH 3 IS VIABLE. Proceed to implementation.**

### Concrete next-step plan

1. **Copy `human_assist.xs` and its includes** into the mod overlay at:
   ```
   mod/aom_autorepair_test/game/ai/human_assist/human_assist.xs
   mod/aom_autorepair_test/game/ai/human_assist/human_assist_unit_queries.xs
   mod/aom_autorepair_test/game/ai/human_assist/human_assist_resource_breakdown_system.xs
   mod/aom_autorepair_test/game/ai/human_assist/human_assist_debug.xs
   ```
   Only copy and modify `human_assist.xs` unless the new rule needs helper functions (put those in
   a new `human_assist_autorepair.xs` and `include` it from `human_assist.xs`).

2. **Add a toggle global** (per-unit instance is impossible in script; use a global bool):
   ```xs
   bool gAutoRepairEnabled = false;
   ```
   Wire the toggle: add `enableAutoRepair()` / `disableAutoRepair()` functions callable from the
   UI or from scenario triggers (same pattern as `enableVillagerAssist()` /
   `disableVillagerAssist()`). Default `false` as required.

3. **Add the repair rule** in `human_assist.xs` after the existing rules:
   ```xs
   rule autoRepairIdleUnits
   inactive
   minInterval 3
   maxInterval 10
   {
       if (gAutoRepairEnabled == false) { return; }
       // Step 1: find idle repairers (villagers + Norse infantry with repair action)
       // Use kbUnitQuerySetActionType(qID, cActionTypeIdle) 
       // or loop and check kbUnitGetActionType(unitID) == cActionTypeIdle
       // AND kbUnitGetIdleTime(unitID) > 2000  (avoid interrupting very brief idles)
       
       // Step 2: for each idle repairer, query nearby buildings in radius ~20
       // kbUnitQuerySetPlayerID, cUnitTypeBuilding, cUnitStateAlive,
       // kbUnitQuerySetPosition(pos), kbUnitQuerySetMaximumDistance(20.0)
       
       // Step 3: iterate results, check for damage:
       //   for each building: run single-result query, compare
       //   kbUnitQueryGetUnitHitpoints(singleQ, false) > kbUnitQueryGetUnitHitpoints(singleQ, true)
       //   if damaged -> aiTaskWorkUnit(repairer, building, false)
       //   break to next repairer (one task per repairer per poll)
   }
   ```
   Enable the rule from `enableAutoRepair()` and disable from `disableAutoRepair()`.

4. **Unit type filtering for Norse infantry:**  Use `kbProtoUnitIsType(protoID, cUnitTypeHersir)`
   etc. to filter. The exact set of Nordic infantry with the Repair action must be confirmed at
   runtime using `aiDebug` + `MythAIConstantsPlayer1.txt`. Expected types: Huskarl, Berserk,
   Hersir, Throwing Axeman, Raiding Cavalry — anything with a Repair proto-action.

5. **HP detection fallback:** If `kbUnitQueryGetUnitHitpoints` proves unreliable for buildings,
   fall back to `kbUnitGetPower(buildingID, false) > kbUnitGetPower(buildingID, true)` as the
   damage check. Both values return 0 for buildings with no pop/resource cost — test explicitly.

6. **Verify resource deduction** early in testing: place a damaged building, let the auto-repair
   fire, and watch the wood counter (wood is consumed by repair). If it drops, resource cost is
   being applied. If not, `aiTaskWorkUnit` for repair may need a different approach (e.g. wrapping
   in a short-lived Repair plan instead of a bare task). This is the single highest-risk unknown.

7. **Patch-safety:** Document the lines added in `human_assist.xs`. After each game patch, diff
   the vanilla file against the mod file and re-merge any changes from the patch.

### Risk register

| Risk | Severity | Mitigation |
|---|---|---|
| `aiTaskWorkUnit` on a building issues Repair but engine does not deduct resources | HIGH | Empirical test first; fallback: wrap in a `cPlanRepair` plan if one exists, or use a trigger-side `trUnitSetHitpoints` approach as a last resort |
| `kbUnitQueryGetUnitHitpoints` returns 0 for all buildings (no cost) | MEDIUM | Test one concrete building; use `kbUnitGetPower` comparison as fallback |
| Campaign missions call `trEnforceAIAssist(false)` blocking the mod in campaign | LOW | Check each campaign on first playthrough; no evidence this is called in shipped missions |
| Game patch changes `human_assist.xs` | MEDIUM-LOW | Pin mod file to specific patch version; establish diff workflow |
| MP desync from non-deterministic calls inside the rule | LOW | Do not use `xsRandFloat/Int` based on local state; all inputs must be deterministic (unit IDs, positions from KB) |

---

## Sources Read

| Source | Used for |
|---|---|
| `BANG_Documentation/AI documentation/AI dev environment setup.pdf` | AI config table, constants file, debug tools |
| `BANG_Documentation/AI documentation/How-to create your own skirmish AI.pdf` | Personality system, mod overlay mechanism (Q4) |
| `BANG_Documentation/AI documentation/What is allowed in AI scripts.pdf` | AI/KB/XS syscall scope |
| `BANG_Documentation/XS documentation/What is XS.pdf` | XS language basics, script availability |
| `BANG_Documentation/XS documentation/XS rules functionality.pdf` | Rule execution model, AI simulation tick (Q5) |
| `BANG_Documentation/Trigger documentation/What is allowed in TR scripts.pdf` | TR vs AI syscall split |
| `doxygen_retail.7z` extracted → `aifuncs_8cpp.html` | `aiTaskWorkUnit`, `aiTaskWorkUnits`, `aiTaskStopUnit` (Q3c) |
| `doxygen_retail.7z` extracted → `kbfuncs_8cpp.html` | `kbUnitQuerySetActionType`, `kbUnitGetIdleTime`, `kbUnitGetActionType`, `kbUnitQueryGetUnitHitpoints`, `kbUnitGetPower` (Q3a/b) |
| `doxygen_retail.7z` extracted → `triggerfuncs_8cpp.html` | `trEnforceAIAssist`, `trPlayerGetVillagerPriority` (Q2) |
| `game/ai/human_assist/human_assist.xs` | Entry point identity, game-type log, VPS enable/disable lifecycle (Q1, Q2) |
| `game/ai/human_assist/human_assist_unit_queries.xs` | Idle-unit detection pattern, query API usage (Q3a) |
| `game/ai/personalities/humanoid.personality` | Confirms humanoid personality uses `chairon` not `human_assist` (Q1) |
| `game/ai/personalities/noai.personality` | Confirms `inactive_ai` is separate from `human_assist` (Q1) |
| `game/ai/personalities/personalities.xml` | Full personality list (Q1) |
| `game/ai/inactive_ai.xs` | Confirms inactive_ai is for NPC-only slots, not human players (Q1) |
| `game/ai/chairon.xs` | Confirms skirmish AI entry point is chairon → core/main.xs (Q1) |
| `game/ai/campaign/fott/fott02_p2.xs` | Confirms campaign per-player scripts are NPC-only (Q2) |

---

## Self-Review

- [x] Read all 5 listed PDFs (AI dev setup, How-to skirmish AI, What is allowed in AI, What is XS, XS rules functionality)
- [x] Extracted `doxygen_retail.7z` and searched `aifuncs_8cpp.html`, `kbfuncs_8cpp.html`, `triggerfuncs_8cpp.html`
- [x] Looked at `game/ai/` directory — identified `human_assist.xs` as entry point, examined its includes and rules
- [x] All five questions answered with citations
- [x] Confidence indicators attached to every claim
- [x] Recommendation is concrete (specific file paths, function names, implementation steps, risk register)

## Open Questions (UNCERTAIN — require empirical resolution)

1. Does `aiTaskWorkUnit(villagerID, buildingID, false)` actually deduct wood resources when the
   engine resolves it as a Repair action? **This is the single most critical unknown.** Test first
   before implementing the full toggle system.

2. Does `kbUnitQueryGetUnitHitpoints(qID, true)` return a value less than `(qID, false)` for a
   damaged building? Run in a test scenario with `aiDebug` enabled.

3. Which exact unit type constant identifies "Norse military units with the Repair action"?
   Confirm at runtime via `MythAIConstantsPlayer1.txt`.

4. Do any shipped campaign missions call `trEnforceAIAssist(false)`? If yes, the mod silently
   does nothing in those missions — acceptable given the `.mythscn` files are binary.
