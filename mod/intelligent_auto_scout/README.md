# Intelligent Auto-Scout (Age of Mythology: Retold mod)

Replaces the engine's auto-scout button on land scouts with an engine-driven area-exploration system. The mod selects, scores, and filters land areas; the game's own `cPlanExplore` consumes the area list and handles pathing and waypoint sequencing. It still prioritizes coverage near your town center, coordinates across multiple scouts, and routes any herdable a scout passes near to your nearest town center.

When you press the auto-scout button on a scout, the scout is added to a managed pool. On every tick, the mod builds a ranked list of areas for each scout using a layered breadth-first search over the map's area-adjacency graph: the scout's current area takes top priority while it still has unexplored tiles, then nearby reachable areas are scored by closeness to your main town center, closeness to the picking scout, and a density penalty that discounts areas where another scout is already heading. Dangerous areas (from the heat-map), areas claimed by another scout, and already-visited areas are filtered out before the list is handed to the engine. The engine then drives the scout through the surviving area IDs. Areas claimed by one scout are excluded from other scouts' candidate sets, so multiple scouts naturally fan out across the map instead of stacking on the same direction.

Herdables are handled along the way. A scout near a Gaia or enemy herdable will divert to claim it by walking close enough to flip ownership, then resume scouting. Each herdable is attempted at most once globally, so multiple scouts will not pile onto the same target. Any herdable that flips to your control — whether by intentional divert or by walking past it during normal scouting — is automatically routed to the nearest of your town centers via the engine's herd-on-TC delivery, the same behavior as right-clicking a herd on a town center. Town center variants are recognized by the `AbstractTownCenter` unit type, including Citadel Centers spawned by the Egyptian Citadel god power and Atlantean Village Centers.

Player commands always take priority. Pressing the cancel button on the auto-scout, or issuing any move/attack/garrison order to the scout, removes it from the pool and reverts to vanilla behavior. The button is the standard auto-scout UI button — there is no separate toggle.

The mod applies to all `AbstractScout` units. Atlantean Oracles use the same engine-driven area list as regular scouts, with an additional discount for areas already covered by another Oracle's growing LOS. The engine is given `StopLOSPercentage=0.8` as a hint, and an explicit LOS monitor parks the Oracle and stops it when current LOS drops below 50% of its maximum, then resumes exploration when LOS reaches 100%. Multiple Oracles coordinate so they spread their saturation footprints across the map instead of stacking.

Scouts actively avoid enemy threats. Visible enemy military units, town centers, and towers radiate danger into surrounding areas via a heat-map flood-fill scaled by each threat's DPS and attack range; areas with heat above a hard-skip threshold are refused as candidates and as propagation steps. Because area lists are filtered before being handed to the engine, the path the engine chooses is also constrained to safe areas. Damage and death events leave persistent heat bumps so subsequent scouts steer wide of where one was attacked. A scout that finds itself in a now-dangerous area enters a flee state, walks to the lowest-danger neighbour, and holds for a few seconds before picking a fresh target.

The mod adds a single include line plus one function call to `game/ai/human_assist/human_assist.xs` and ships an `auto_scout.xs` file alongside it; that is the entire footprint.

## Mod layout

- `game/ai/human_assist/human_assist.xs` — copy of vanilla `human_assist.xs` with one extra `include` line and one call to `autoScout_register` inside `enableAutoScouting`.
- `game/ai/human_assist/auto_scout.xs` — all of the mod's logic, in a self-contained file. Read this first.

Design specs for the BFS + frontier-walk core and the herd-diversion extension live in the repository root under `docs/superpowers/specs/`.

## Local testing

Place the contents of this folder under `mods/local/intelligent_auto_scout/` in your Age of Mythology Retold user data directory. The portable way to find that directory: launch the game, open `Mods → Mod Manager`, select any installed mod, and click `Open Directory`; navigate up one level to the `local/` folder.

If you also use the Idle Auto-Repair mod, both mods overlay the same `human_assist.xs` and cannot be deployed as separate local mods. Use the combined deploy under `mod/intelligent_auto_repair_and_scout/` in the repository root instead, which contains a single `human_assist.xs` that includes both feature scripts.

## Patch maintenance

The mod overlays vanilla `human_assist.xs`. After any Age of Mythology Retold patch that touches that file:

1. Replace `game/ai/human_assist/human_assist.xs` with a fresh copy of the new vanilla file.
2. Re-add the include line right after the existing `include` directives:
   ```xs
   include "human_assist/auto_scout.xs"; // Intelligent Auto-Scout mod
   ```
3. Re-add the registration call inside `enableAutoScouting`, immediately after the Oracle-specific block and before `aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true)`:
   ```xs
   autoScout_register(planID, unitID); // Intelligent Auto-Scout mod
   ```
4. Re-upload to the Age of Mythology Retold mod platform.

`auto_scout.xs` itself is independent of vanilla and is generally unaffected by game patches.

## Known limitations

- The engine still picks exact waypoints inside an area, so a scout occasionally clips a thin sliver of an otherwise-blocked area. The heat-map filter still removes truly dangerous areas from the search graph.
- Oracles rely on the engine respecting `StopLOSPercentage`; the explicit LOS monitor handles the case where that hint is not honoured by issuing a hard stop/park.

## Changelog

**2026-06-23 — Engine-driven area exploration**
- Replaced the manual frontier-walk state machine with the engine's built-in `cPlanExplore` area mode. The mod now builds, scores, filters, and claims land areas, then hands the ranked list to `cExplorePlanExploreAreaIDs` for the engine to path and sweep.
- Heat-map danger areas remain filtered out before the list is sent, keeping scouts on safe exploration routes.
- Oracle movement migrated to the same engine plan, using `StopLOSPercentage=0.8` plus a hard LOS monitor that parks when under 50% of max LOS and resumes at 100%.
- Park/unpark coordination is now explicit during herd diversion, flee recovery, and routine Oracle LOS pausing, so scouts release their area claims and reclaim fresh areas after every pause.
- Removed the legacy frontier-walk, corridor, visited-waypoint, and hand-written Walking/Idle/WORKING state arrays.

**2026-05-14 — Scouts now avoid enemy danger**
- Scouts actively keep away from enemy military units, town centers, and towers. Tiles around visible threats become no-go zones; the more dangerous a building (more damage, longer range, more projectiles), the wider the no-go ring around it.
- Path-aware walking: when a scout's destination is on the far side of an enemy threat, the scout detours around the danger area instead of cutting through it on the way.
- Retreat behaviour: a scout that ends up in a freshly-discovered danger zone walks to a safer nearby area and pauses for five seconds before picking a new destination. Areas it fled from are remembered for a minute and a half.
- Combat memory: tiles where a scout was attacked or killed stay marked dangerous for ten seconds, so the next scouts steer wide of the same hot spot.
- Oracle re-positioning hops are shorter (about a quarter of an Oracle's maximum sight range), so each move reveals more new map without wasting LOS on already-seen ground.
- Oracles whose claim circles touch can now share a little overlap instead of being hard-blocked, so they can still cover the map when there's no perfectly-clean alternative.

**2026-05-13 — Oracle support**
- Atlantean Oracles now use the auto-scout button: walk to a chosen spot, park, wait for line-of-sight to grow to its maximum (the meditation animation plays), then move on to a fresh location.
- Multiple Oracles coordinate so they spread across the map instead of stacking on the same area. Regular scouts also discount areas already covered by an Oracle's growing LOS.

**2026-05-04 — Herd diversion**
- Scouts that pass near a wild or enemy herdable briefly detour to claim it by walking close enough to flip ownership.
- Any herdable that ends up under your control (whether by intentional divert or by walking past it during normal scouting) is automatically sent to the nearest of your town centers — Town Centers, Citadel Centers, and Atlantean Village Centers all qualify.
- Each herd is attempted at most once across all your scouts, so they don't pile onto the same target.

**Initial release — Frontier-based exploration**
- Replaces the engine's auto-scout button on land scouts. The scout pool spreads across the map, prioritizes areas near your town center, and avoids stacking on the same direction.
- Each scout sweeps its chosen area until coverage is reached, then picks a new one.
- Player commands and the auto-scout cancel button always take priority.

## License

MIT. See `LICENSE` in the repository root.
