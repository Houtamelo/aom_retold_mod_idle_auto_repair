# houtamelo's Age of Mythology: Retold mods

Source for the AoM:R mods I publish on the Age of Mythology Retold mod platform, plus their development history.

## Mods

- [`mod/idle_auto_repair/`](mod/idle_auto_repair/) — **Idle Auto-Repair**. Idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at normal resource cost. Covers villagers for all civs and Norse soldier-builders. Player commands always take priority. See the mod's [README](mod/idle_auto_repair/README.md) for full details.

- [`mod/intelligent_auto_scout/`](mod/intelligent_auto_scout/) — **Intelligent Auto-Scout**. Replaces the engine's auto-scout button on land scouts with a frontier-based exploration system that prioritizes coverage near your town center, coordinates across multiple scouts, and routes any herdable a scout passes near to your nearest town center. Oracles keep their vanilla auto-scout behavior. See the mod's [README](mod/intelligent_auto_scout/README.md) for full details.

- [`mod/intelligent_auto_repair_and_scout/`](mod/intelligent_auto_repair_and_scout/) — Combined-deploy variant of the two mods above. Both mods overlay the same `game/ai/human_assist/human_assist.xs` and so cannot be installed as separate local mods at the same time; this folder ships a single unified overlay so you can run both features together. See the combined mod's [README](mod/intelligent_auto_repair_and_scout/README.md) for the player-facing changelog spanning both features.

- [`mod/human_assist_improvements/`](mod/human_assist_improvements/) — **Human Assist Improvements**. Bundled superset of Idle Auto-Repair and Intelligent Auto-Scout, now also with Auto-relic-delivery, and the canonical home for future human-assist features. Use this if you want all features in a single mod.

- [`mod/Extra Ai + AoModAi/`](mod/Extra%20Ai%20%2B%20AoModAi/) — **Extra Ai + AoModAi**. A curated port of the AI improvements from Garbhus's published mod 314139 "extra ai", distilled to the 22 files that contain intentional modder edits. Covers island/amphibious play, max-military economy, reduced resource bonus at higher difficulties, fewer suicide-wave attacks, and significant God Powers / upgrade logic fixes. The 5 vanilla-stale files that the source mod accidentally included are intentionally excluded so future base-game patches can reach those files normally. See the mod's [README](mod/Extra%20Ai%20%2B%20AoModAi/README.md) for the full file-by-file map.

> **Compatibility:** The first four mods overlay the same `game/ai/human_assist/human_assist.xs` — enable only one of them at a time. `Human Assist Improvements` is the recommended single-mod choice going forward; the three existing mods remain available for players who want only one feature. `Extra Ai + AoModAi` overlays 22 different files under `game/ai/core/...` and is compatible with all four `human_assist_*` mods (no file overlap). It conflicts with the subscribed mod 314139 "extra ai" (same 22 files — enable only one of the two at a time).

## Other contents

- `mod/aom_autorepair_test/` — historical proof-of-concept iterations from the auto-repair research; kept for reference, not deployed.
- `docs/` — development notes (proto_mods.xml syntax reference, AI-script-hook viability research) and the design specs / implementation plans that drove each mod under `docs/superpowers/`.
- `extracted/` — extracted reference files from the game install (proto.xml, doxygen, etc.) used during research.

## Changelog

### Extra Ai + AoModAi

**2026-06-18 — Initial release**
- Curated port of Garbhus's published mod 314139 "extra ai", distilled to the 22 files with intentional modder edits. 5 vanilla-stale files the source mod shipped are intentionally excluded so future base-game patches can reach those files normally.
- 22 files covering all five of Garbhus's stated themes: island/amphibious play, max-military economy, reduced resource bonus at higher difficulties, fewer suicide-wave attacks, and significant God Powers / upgrade logic fixes.
- See the mod's [README](mod/Extra%20Ai%20%2B%20AoModAi/README.md) for the full file-by-file map and compatibility matrix.

### Intelligent Auto-Scout

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

### Idle Auto-Repair

**2026-05-03 — Norse Farms fix**
- Norse villagers under Thor, Odin, and Loki now correctly skip Farms. Previously they would try to repair Farms (which they cannot, under those gods) and get stuck.

**Initial release**
- Idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at normal resource cost.
- Coverage: villagers for Greek, Egyptian, Atlantean, Chinese, Japanese, and Aztec; Norse soldier-builders (Berserk, Throwing Axeman, Hersir, Hirdman, Huskarl, Godi, Heroes of Ragnarok) for any building; Norse villagers for the building types their major god allows.
- Up to ten builders can converge on a single damaged building.
- Buildings inside active warzones are skipped so workers don't get sent into combat.
- Buildings the game marks as non-repairable are also skipped.
- Player commands always take priority — issue any order to a repairing unit and it leaves auto-repair immediately.

## License

MIT. See `LICENSE`.
