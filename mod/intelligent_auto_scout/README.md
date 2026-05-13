# Intelligent Auto-Scout (Age of Mythology: Retold mod)

Replaces the engine's auto-scout button on land scouts with a frontier-based exploration system that prioritizes coverage near your town center, coordinates across multiple scouts, and routes any herdable a scout passes near to your nearest town center.

When you press the auto-scout button on a scout, the scout is added to a managed pool. On every tick, each pooled scout is assigned a target area chosen by a layered breadth-first search over the map's area-adjacency graph: the scout's current area takes top priority while it still has unexplored tiles, then nearby reachable areas are scored by closeness to your main town center, closeness to the picking scout, and a density penalty that discounts areas where another scout is already heading. The scout walks to the chosen area's centroid and then sweeps the area via a frontier walk — sample candidate waypoints at five radii by eight angles around the scout, accept the first reachable in-area position outside its current line of sight, walk there, and repeat — until the area's unexplored-tile percentage drops below a threshold or the per-area step cap is hit. Areas claimed by one scout are excluded from other scouts' candidate sets, so multiple scouts naturally fan out across the map instead of stacking on the same direction.

Herdables are handled along the way. A scout near a Gaia or enemy herdable will divert to claim it by walking close enough to flip ownership, then resume scouting. Each herdable is attempted at most once globally, so multiple scouts will not pile onto the same target. Any herdable that flips to your control — whether by intentional divert or by walking past it during normal scouting — is automatically routed to the nearest of your town centers via the engine's herd-on-TC delivery, the same behavior as right-clicking a herd on a town center. Town center variants are recognized by the `AbstractTownCenter` unit type, including Citadel Centers spawned by the Egyptian Citadel god power and Atlantean Village Centers.

Player commands always take priority. Pressing the cancel button on the auto-scout, or issuing any move/attack/garrison order to the scout, removes it from the pool and reverts to vanilla behavior. The button is the standard auto-scout UI button — there is no separate toggle.

The mod applies to all `AbstractScout` units. Atlantean Oracles get their own state machine because their mechanic is fundamentally different from a frontier walk: they are picked a target area the same way as regular scouts (with an additional discount for areas already covered by another Oracle's growing LOS), walk there, then park and let their AutoLOS bonus saturate before re-picking. Multiple Oracles coordinate so they spread their saturation footprints across the map instead of stacking. Scouts do not currently avoid enemy threats, so a scout walking past an enemy tower or town center on an aggressive map can die before completing its sweep.

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

## Changelog

**2026-05-13 — Oracle support**
- Atlantean Oracles now have their own scouting behaviour: walk to a chosen area, park there, wait for LOS to saturate (engine action 37 / meditation animation), then re-pick a fresh target.
- Multi-oracle coordination: hard-skip for areas within 80% of MaxOracleLOS of another oracle; regular scouts get a multiplicative score discount for areas near oracles. Pool-tracked oracles' target waypoints are considered alongside their current positions so newly-toggled oracles diverge instead of converging on the same destination.
- Dynamic MaxOracleLOS cache adapts to god-tech upgrades and proto modifications.
- Engine `cPlanExplore` is parked in `cPlanStateIdle` so it no longer drives the unit; our state machine has full control while the auto-scout UI button stays functional.

**2026-05-04 — Herd diversion**
- Scouts passing near a Gaia or enemy herdable briefly divert to claim it by walking close enough to flip ownership.
- Any herdable that flips to your control (whether by intentional divert or by walking past it) is automatically routed to the nearest of your town centers via the engine's herd-on-TC delivery. Town Centers, Citadel Centers, and Village Centers all qualify.
- Each herdable is attempted at most once globally across all your scouts.

**Initial release — Frontier-based exploration**
- Replaces the engine's auto-scout for land scouts (Oracles got their own pass later, see above).
- Layered BFS over the map's area-adjacency graph picks targets scored by closeness to your main town center, closeness to the picking scout, and a density penalty against other scouts already heading to the same region.
- Frontier-walk inside the chosen area sweeps until coverage is reached or the per-area step cap is hit.
- Multi-scout coordination via per-area claims; scouts naturally fan out across the map.
- Player commands and the auto-scout cancel button always take priority.

## License

MIT. See `LICENSE` in the repository root.
