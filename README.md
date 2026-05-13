# houtamelo's Age of Mythology: Retold mods

Source for the AoM:R mods I publish on the Age of Mythology Retold mod platform, plus their development history.

## Mods

- [`mod/idle_auto_repair/`](mod/idle_auto_repair/) — **Idle Auto-Repair**. Idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at normal resource cost. Covers villagers for all civs and Norse soldier-builders. Player commands always take priority. See the mod's [README](mod/idle_auto_repair/README.md) for full details.

- [`mod/intelligent_auto_scout/`](mod/intelligent_auto_scout/) — **Intelligent Auto-Scout**. Replaces the engine's auto-scout button on land scouts with a frontier-based exploration system that prioritizes coverage near your town center, coordinates across multiple scouts, and routes any herdable a scout passes near to your nearest town center. Oracles keep their vanilla auto-scout behavior. See the mod's [README](mod/intelligent_auto_scout/README.md) for full details.

- [`mod/intelligent_auto_repair_and_scout/`](mod/intelligent_auto_repair_and_scout/) — Combined-deploy variant of the two mods above. Both mods overlay the same `game/ai/human_assist/human_assist.xs` and so cannot be installed as separate local mods at the same time; this folder ships a single unified overlay so you can run both features together.

## Other contents

- `mod/aom_autorepair_test/` — historical proof-of-concept iterations from the auto-repair research; kept for reference, not deployed.
- `docs/` — development notes (proto_mods.xml syntax reference, AI-script-hook viability research) and the design specs / implementation plans that drove each mod under `docs/superpowers/`.
- `extracted/` — extracted reference files from the game install (proto.xml, doxygen, etc.) used during research.

## Changelog

### Intelligent Auto-Scout

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

### Idle Auto-Repair

**2026-05-03 — Norse Farms fix**
- Norse villagers under non-Freyr gods now correctly skip Farms (the vanilla `<rate type="House">` constraint means they can only repair Houses; previously they would attempt Farms and get stuck).

**Initial release**
- Idle repair-capable units automatically walk to nearby damaged friendly buildings and repair them at normal resource cost.
- Coverage: villagers for Greek, Egyptian, Atlantean, Chinese, Japanese, and Aztec civilizations; Norse soldier-builders (Berserk, Throwing Axeman, Hersir, Hirdman, Huskarl, Godi, Heroes of Ragnarok) for any building type; Norse villagers for the building types they can repair under the current major god.
- Up to 10 units can converge on a single damaged building.
- Buildings inside engine-flagged warzones are skipped so workers don't get sent into combat.
- Buildings flagged non-repairable in the game's data are also skipped.
- Player commands always take priority and remove the unit from its auto-repair plan immediately.

## License

MIT. See `LICENSE`.
