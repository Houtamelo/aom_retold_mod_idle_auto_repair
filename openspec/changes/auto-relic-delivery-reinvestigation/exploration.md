# Re-investigation: Does AoM:R expose the carried relic unit ID?

## Status

- **Change id:** auto-relic-delivery-reinvestigation
- **Phase:** explore
- **Question:** Does the AoM:R XS engine expose any API to map a hero unit ID to the relic unit ID it is carrying, or a relic unit ID to its carrier?
- **Verdict:** **API EXISTS** — `kbUnitGetContainedUnitByIndex(unitID, index)` returns the contained unit ID; `kbUnitGetContainer(unitID)` returns the container. Combined with `kbRelicGetTechID(unitID)`, the ideal `(heroID, relicID)` pair tracker is possible.

## Goal

The archived `auto-relic-delivery` implementation fell back to a per-hero state machine because the prior exploration claimed no API could retrieve the relic unit ID a hero was carrying. The user pushed back, arguing that relics and heroes are first-class objects with IDs and the engine should expose them. This re-investigation must either find that API or prove its absence.

## Method

1. **Primary source — `extra ai` mod:** read the entire relic collection implementation in `mod/Extra Ai + AoModAi/game/ai/core/exploration.xs` (lines 1767–2455) and catalogue every call touching relics, containers, plans, or unit IDs.
2. **Broad substring search over shipped AoM:R AI source** (`/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/`) for any function call containing: `Contain`, `Garrison`, `Inventory`, `Carry`, `Carrier`, `Held`, `Holder`, `Slot`, `Item`, `Transport`, `Relic`, `Passenger`, `Rider`, `Load`, `Cargo`, `Stored`, `Inside`, `Within`.
3. **Exact container/cargo API search:** manually enumerate every `kbUnit*Contain*`, `kbUnit*Garrison*`, `kbUnit*Cargo*`, `kbUnit*Passenger*`, `kbUnit*Holder*`, `kbUnit*Carrier*` call site.
4. **Retail engine API docs:** search `extracted/doxygen/kbfuncs_8cpp.html` (the KB/XS function reference from the retail build) for the same terms and for `Relic`.
5. **Constants files:** search `docs/MythRMConstants.txt` and `docs/MythTRConstants.txt` for `Relic`, `Contain`, `Garrison`, `Inventory`, `Slot`, `Item`, `Held`, `Holder`, `Cargo`, `Passenger`.
6. **Trigger system check:** search the AI source and API docs for richer relic trigger events (`cTriggerEvent*`, `kbTrigger*`, `xsTrigger*`, `RelicTrigger`, `cEventRelic*`).
7. **Plan-content check:** enumerate `aiPlanGet*` / `kbPlanGet*` functions that might expose units-in-plan as a workaround.
8. **Existing mod precedent:** compare with `auto_scout.xs` herdable tracking and `auto_repair.xs` query patterns.

### Search counts

| Search | Scope | Result count / note |
|--------|-------|--------------------|
| `Contain\|Garrison\|Inventory` | shipped `game/ai/*.xs` | 14 matches |
| `Carry\|Carrier\|Held\|Holder\|Slot\|Item` | shipped `game/ai/*.xs` | 40 matches |
| `Transport\|Passenger\|Rider\|Load\|Cargo\|Stored\|Inside\|Within` | shipped `game/ai/*.xs` | 100+ matches (truncated) |
| `kbUnitGet*Contained*` family | shipped `game/ai/*.xs` | **3 call sites, all `kbUnitGetNumberContained`** |
| `Relic` | `extra ai` mod `game/ai/core/exploration.xs` | 100+ matches (full relic section read) |
| Plan-content APIs (`aiPlanGetNumberUnits`, `aiPlanGetUnitIDByIndex`, `aiPlanGetUnits`) | shipped `game/ai/*.xs` | 72 / 48 / 42 usages |
| `Relic\|Contain\|Garrison\|Inventory\|Slot\|Item\|Held\|Holder\|Cargo\|Passenger` | `extracted/doxygen/kbfuncs_8cpp.html` | **6 relevant KB APIs including `kbUnitGetContainer` and `kbUnitGetContainedUnitByIndex`** |
| Trigger-system patterns | shipped `game/ai/*.xs` | No `cTriggerEvent*`, `kbTrigger*`, or `xsTrigger*` symbols found |

## 1. The smoking gun — how `extra ai` identifies relics

Garbhus's mod does **not** ask the engine "what relic is this hero carrying?". It already knows both IDs because it chose the hero, found a relic in the world, and ordered the pickup.

Key pieces from `mod/Extra Ai + AoModAi/game/ai/core/exploration.xs`:

- **Relics are first-class units.** They are enumerated with a standard KB unit query using the constant `cUnitTypeRelic`:
  - `int queryID = useSimpleUnitQuery(cUnitTypeRelic, 0, cUnitStateAlive, searchPosition, cMaxFloat);` at **line 2328**.
  - The result is an `int[] relics = kbUnitQueryGetResults(queryID);` (line 2336).
  - The chosen relic's unit ID is stored globally: `gRelicID = relics[i]; gRelicPosition = kbUnitGetPosition(gRelicID);` (lines 2351–2352).
- **The carrier is chosen from a plan.** `int[] units = aiPlanGetUnits(gPrimaryLandDefendPlan, searchUnitType, false, excludeTypes);` (line 2310); `heroID = units[0]` (line 2315); `heroProtoUnitID = kbUnitGetProtoUnitID(heroID)` (line 2316).
- **Island variant tracks the hero explicitly.** `gRelicHeroID = heroID; gRelicHeroProtoUnitID = heroProtoUnitID;` in `createIslandRelicPlan` (lines 1942–1943).
- **Pickup order uses the known IDs.** `aiTaskWorkUnit(heroID, gRelicID)` at line 2024, 2037, 2229, etc.
- **Deposit uses the known hero ID.** `aiTaskWorkUnit(heroID, gRelicTempleID)` (lines 2082, 2269, etc.).
- **Container API usage:** the only container API the mod uses is `kbUnitGetNumberContained(gRelicTempleID)` to check temple capacity (line 1887 and 2065).

**Crucial implication:** the mod never had a reason to discover a "hero → carried relic" API because it owns the whole pickup pipeline. Its code therefore does **not** prove the API is absent; it just proves the mod did not need it.

## 2. API candidate table

### 2.1 Direct container APIs (from retail KB docs)

These were found in `extracted/doxygen/kbfuncs_8cpp.html`. They are **not used anywhere** in the shipped AI source or the `extra ai` mod, which is why the previous search missed them.

| Function | Signature | Returns | Could answer our question? |
|----------|-----------|---------|---------------------------|
| `kbUnitGetContainer` | `(int unitID) -> int` | The container unit ID of the provided unit | **YES (inverse)** — call with a relic unit ID; if it is carried, returns the hero. |
| `kbUnitIsContainedBy` | `(int unitID, int unitTypeID) -> bool` | Whether `unitID` is inside a container of `unitTypeID` | Maybe — `kbUnitIsContainedBy(relicID, cUnitTypeHero)` could test the relationship, but does not return the carrier. |
| `kbUnitGetNumberContained` | `(int unitID) -> int` | Count of units inside `unitID` | NO — only a count. Shipped source uses this for temples and (speculatively) heroes. |
| `kbUnitGetNumberContainedOfType` | `(int unitID, int unitTypeID) -> int` | Count of contained units of a specific type | Useful guard — `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) > 0` before iterating. |
| `kbUnitGetContainedUnitByIndex` | `(int unitID, int index) -> int` | The unit ID contained at `index` | **YES (direct)** — `kbUnitGetContainedUnitByIndex(heroID, 0)` returns the carried relic ID. |
| `kbRelicGetTechID` | `(int unitID) -> int` | The tech ID of a relic unit, or -1 | **YES (cross-check)** — maps a relic unit ID to the `techID` payload of `cXSRelicPickedUpHandler`. |

### 2.2 Other substring matches (not usable for relic/carrier mapping)

| Function | First source | What it actually does | Verdict |
|----------|--------------|----------------------|---------|
| `kbGodPowerGetIDInSlot` | `core/godpowers/godpowers.xs:259` | Returns the god-power proto in a UI slot | Not related to units. |
| `kbGetPopulationSlotsByUnitTypeID` | `core/military/naval_military_units.xs:161` | Population-slot accounting | Not related. |
| `kbUnitQueryGetPopulationSlots` | `core/military/military_units.xs:403` | Population-slot accounting from query | Not related. |
| `kbUnitGetCarryCapacity` | `kbfuncs_8cpp.html:225` | Resource carry capacity (food/wood/gold) | Not unit-inventory. |
| `aiPlanGetNumberUnits` / `aiPlanGetUnitIDByIndex` / `aiPlanGetUnits` | many | Enumerate units assigned to an AI plan | Could enumerate the hero in a plan, but not units inside a hero. |

## 3. Relic-as-unit hypothesis

- **Relics have a unit type constant:** `cUnitTypeRelic = 598` in both `docs/MythRMConstants.txt:768` and `docs/MythTRConstants.txt:747`.
- **Relics have unit IDs in the world:** confirmed by `extra ai`'s query and `kbRelicGetTechID(unitID)` (retail docs), which requires a relic unit ID.
- **Can we query "relics carried by hero X" with `kbUnitQueryCreate`?** No. The KB query filters are: player, unit type, state, action type, position/distance, area/group, visible state, exclude types. There is **no** "container" or "inside unit X" filter in `kbfuncs_8cpp.html`. `cUnitState*` constants (`cUnitStateAlive`, etc.) do not include a carried/contained state.
- **Can we find the carrier of a known relic?** Yes, via `kbUnitGetContainer(relicID)`. This is the inverse of the original question. It requires knowing the relic's unit ID before pickup (e.g., from a prior world query or from scanning the hero after pickup).

## 4. Trigger-system finding

- The only relic-related XS event handler constants are `cXSRelicPickedUpHandler = 5` and `cXSRelicGarrisonedHandler = 6` (`docs/MythTRConstants.txt:2964–2965`).
- The shipped AI handler signatures take only `int techID` (vanilla `core/exploration.xs:1989` and `:2002`).
- No richer trigger event API (`cTriggerEvent*`, `kbTrigger*`, `xsTrigger*`, `RelicTrigger`, `cEventRelic*`) exists in the AI script tree; trigger-based events live in scenario files, not in the AI source exposed here.
- Because `kbRelicGetTechID(relicID)` returns the same tech ID the handler receives, we can still correlate the event with a specific hero/relic pair without richer trigger payload.

## 5. Existing mod precedent

- `auto_repair.xs` uses `kbUnitQueryCreate` + per-result inspection; it does not need contained-unit IDs.
- `auto_scout.xs` identifies individual herdable unit IDs through `kbUnitQueryGetResult(queryID, i)` (`auto_scout.xs:2983`) and tracks them in `gAutoScout_redirectedHerdIDs[]`. This is the closest analog: each "object" (herdable/relic) has a queryable unit ID. The new finding generalizes this to units contained inside another unit.

## 6. Final verdict

**API EXISTS.**

The exact functions are:

1. **Hero → relic:** `int relicID = kbUnitGetContainedUnitByIndex(heroID, 0);`
2. **Relic → hero (inverse):** `int heroID = kbUnitGetContainer(relicID);`
3. **Relic → event techID:** `int techID = kbRelicGetTechID(relicID);`
4. **Safe guard before indexing:** `if (kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) > 0) { ... }`

### Suggested algorithm to replace the per-hero fallback

Inside the `cXSRelicPickedUpHandler` event handler:

```xs
void autoRelicDelivery_onPickedUp(int eventTechID = -1)
{
   if (kbPlayerIsHuman(cMyID) == false) { return; }
   xsSetContextPlayer(cMyID);

   // 1. Enumerate idle heroes (or all heroes if the pickup animation matters).
   int heroQuery = kbUnitQueryCreate("autoRelic_heroesOnPickup");
   kbUnitQuerySetPlayerID(heroQuery, cMyID, false);
   kbUnitQuerySetUnitType(heroQuery, cUnitTypeHero);
   kbUnitQuerySetState(heroQuery, cUnitStateAlive);
   int heroCount = kbUnitQueryExecute(heroQuery);

   for (int i = 0; i < heroCount; i++)
   {
      int heroID = kbUnitQueryGetResult(heroQuery, i);
      if (heroID < 0) { continue; }

      // 2. Skip heroes that are not carrying a relic.
      if (kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) <= 0)
      {
         // kbUnitGetNumberContained(heroID) > 0 still works as a fallback.
         continue;
      }

      // 3. Retrieve the actual relic unit ID.
      int relicID = kbUnitGetContainedUnitByIndex(heroID, 0);
      if (relicID < 0 || kbUnitGetIsIDValid(relicID) == false) { continue; }

      // 4. Optional: correlate with the event payload.
      if (kbRelicGetTechID(relicID) != eventTechID)
      {
         // The event may fire for an ally/enemy or the scan caught a different carrier.
         continue;
      }

      // 5. One-shot delivery, tracked by (heroID, relicID).
      if (autoRelicDelivery_hasPair(heroID, relicID) == false)
      {
         autoRelicDelivery_issueDelivery(heroID, relicID);
      }
   }

   xsSetContextPlayer(-1);
}
```

**Open points that need an in-game smoke test before re-applying:**

1. Does `kbUnitGetContainedUnitByIndex(heroID, 0)` return a valid relic ID immediately after pickup, or only after the pickup animation completes? The `cXSRelicPickedUpHandler` plus the existing one-shot retry should cover any one-frame delay.
2. Is the contained index always `0` for a carried relic? Hero containers should only hold one relic, but a defensive loop over `kbUnitGetNumberContained(heroID)` filtering by `kbUnitIsType(..., cUnitTypeRelic)` is safer.
3. Does `kbUnitGetContainer(relicID)` work for a relic ID captured before pickup? It is the cleanest inverse, but its behavior when the relic is in the world (container = -1?) should be verified.
4. Is `kbRelicGetTechID(relicID)` stable across the pickup event? Compare against `eventTechID` to avoid acting on unrelated pickups.

## 7. Recommendation

- **Re-apply `auto-relic-delivery`** to replace the `cAutoRelic_RelicIDUnknown` fallback with the pair tracker using `kbUnitGetContainedUnitByIndex(heroID, 0)`.
- Keep the per-hero state machine as a defensive fallback inside `issueDelivery`/`scanAndDeliver` until in-game verification confirms the container API works for human-player heroes.
- Document the container-API usage in the module header and README so future maintainers do not repeat the prior "no API exists" assumption.

## Artifacts cited

- `mod/Extra Ai + AoModAi/game/ai/core/exploration.xs:1767–2455` — `extra ai` relic collection logic.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/exploration.xs:1767–2032` — vanilla relic collection logic.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/utilities/unit_queries.xs:495–543` — `getSuitableTempleIDForRelic`.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/setup.xs:180–181` — relic handler registration.
- `extracted/doxygen/kbfuncs_8cpp.html:229` — `kbUnitGetContainer`.
- `extracted/doxygen/kbfuncs_8cpp.html:237` — `kbUnitGetContainedUnitByIndex`.
- `extracted/doxygen/kbfuncs_8cpp.html:235` — `kbUnitGetNumberContainedOfType`.
- `extracted/doxygen/kbfuncs_8cpp.html:926` — `kbRelicGetTechID`.
- `docs/MythTRConstants.txt:747` / `docs/MythRMConstants.txt:768` — `cUnitTypeRelic`.
- `docs/MythTRConstants.txt:2964` — `cXSRelicPickedUpHandler`.
- `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:2952–2998` — per-herd unit-ID tracking precedent.
- `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` — current fallback implementation.
