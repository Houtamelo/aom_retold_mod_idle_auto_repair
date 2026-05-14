# Intelligent Auto-Repair and Scout (Age of Mythology: Retold mod)

Combined deploy of two mods in one package:

- **Idle Auto-Repair** — idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at normal resource cost. Covers villagers across every civilization and Norse soldier-builders.
- **Intelligent Auto-Scout** — replaces the engine's auto-scout button on land scouts and Atlantean Oracles with a smarter exploration system: scouts spread across the map, divert to wild herdables, send claimed herds home, and stay away from enemy threats.

Both features overlay the same vanilla `human_assist.xs` and cannot be installed side-by-side as separate local mods, so this package ships a single unified overlay you can run together. The individual mods' READMEs (see [Intelligent Auto-Scout](../intelligent_auto_scout/README.md) and [Idle Auto-Repair](../idle_auto_repair/README.md)) document the per-feature behaviour in full.

## Local testing

Place the contents of this folder under `mods/local/intelligent_auto_repair_and_scout/` in your Age of Mythology Retold user data directory. The portable way to find that directory: launch the game, open `Mods → Mod Manager`, select any installed mod, and click `Open Directory`; navigate up one level to the `local/` folder.

## Changelog

**2026-05-14 — Scouts now avoid enemy danger** *(Intelligent Auto-Scout)*
- Scouts actively keep away from enemy military units, town centers, and towers. Tiles around visible threats become no-go zones; the more dangerous a building (more damage, longer range, more projectiles), the wider the no-go ring around it.
- Path-aware walking: when a scout's destination is on the far side of an enemy threat, the scout detours around the danger area instead of cutting through it on the way.
- Retreat behaviour: a scout that ends up in a freshly-discovered danger zone walks to a safer nearby area and pauses for five seconds before picking a new destination. Areas it fled from are remembered for a minute and a half.
- Combat memory: tiles where a scout was attacked or killed stay marked dangerous for ten seconds, so the next scouts steer wide of the same hot spot.
- Oracle re-positioning hops are shorter (about a quarter of an Oracle's maximum sight range), so each move reveals more new map without wasting LOS on already-seen ground.
- Oracles whose claim circles touch can now share a little overlap instead of being hard-blocked, so they can still cover the map when there's no perfectly-clean alternative.

**2026-05-13 — Oracle support** *(Intelligent Auto-Scout)*
- Atlantean Oracles now use the auto-scout button: walk to a chosen spot, park, wait for line-of-sight to grow to its maximum (the meditation animation plays), then move on to a fresh location.
- Multiple Oracles coordinate so they spread across the map instead of stacking on the same area.

**2026-05-04 — Herd diversion** *(Intelligent Auto-Scout)*
- Scouts that pass near a wild or enemy herdable briefly detour to claim it by walking close enough to flip ownership.
- Any herdable that ends up under your control (whether by intentional divert or by walking past it during normal scouting) is automatically sent to the nearest of your town centers — Town Centers, Citadel Centers, and Atlantean Village Centers all qualify.
- Each herd is attempted at most once across all your scouts, so they don't pile onto the same target.

**2026-05-03 — Norse Farms fix** *(Idle Auto-Repair)*
- Norse villagers under Thor, Odin, and Loki now correctly skip Farms. Previously they would try to repair Farms (which they cannot, under those gods) and get stuck.

**Initial release**
- **Intelligent Auto-Scout** — replaces the engine's auto-scout button on land scouts. The scout pool spreads across the map, prioritizes areas near your town center, and avoids stacking on the same direction. The scout sweeps the chosen area until coverage is reached, then picks a new one. Player commands and the auto-scout cancel button always take priority.
- **Idle Auto-Repair** — idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at normal resource cost. Coverage: villagers for Greek, Egyptian, Atlantean, Chinese, Japanese, and Aztec; Norse soldier-builders (Berserk, Throwing Axeman, Hersir, Hirdman, Huskarl, Godi, Heroes of Ragnarok) for any building; Norse villagers for the building types their major god allows. Up to ten builders can converge on a single damaged building. Buildings inside engine-flagged warzones are skipped so workers don't get sent into combat. Player commands always take priority.

## License

MIT. See `LICENSE` in the repository root.
