# AOTG World Twists Reference

Source: `game/data/Data.bar :: gameplay/aotg_effects.xml` (extracted via CryBar). World twists are `<effect>` entries whose `<titleid>` starts with `STR_AOTG_RULE_*`. Implementation lives in `trigger/aotg/{minor_wt,seasons,timevortex,remnants}.xs` (XS rules) and/or in `<tech>` entries inside `aotg_techtree`.

Each world twist may have multiple `<effect>` variants — one per difficulty tier (0..9) for the tiered ones (Seasons / Remnants / Time Vortex), plus optional `*Noop` variants representing a deliberate no-op at the base tier. Each variant binds a specific `xsrule` for that escalation step. Rows below collapse the variants into one logical world twist.

| in-game title | effect name(s) | xsrule(s) | tech granted | difficulty | applyflags | applytime (ms) | description |
|---|---|---|---|---|---|---|---|
| Meteor Hit | `MeteorHit` | `aotgMeteorHit` | — | 0 | Global | 0 | A meteor drops on a random unit every minute (including Nature units). |
| Idle Bolt | `IdleBolt` | `aotgIdleBolt` | — | 0 | Global | 0 | Any villagers idle for more than 15 seconds get bolted. |
| Giant Units | `GiantUnits` | — | `AOTGGiantUnits` | 0 | Global | 0 | Every unit is twice as big and has twice the hitpoints. |
| Low Gravity | `LowGravity` | — | `AOTGLowGravity` | 0 | Global | 0 | All projectiles fly at quarter speed. |
| Instant Techs | `InstantTechs` | — | `AOTGInstantTechs` | 0 | Global | 0 | All technologies (except age-ups) are researched nearly instantly. |
| Lost Souls | `LostSouls` | — | `AOTGLostSouls` | 0 | Global | 0 | Every dead unit respawns as an aggressive Gaia shade. |
| Heavenly Haste | `FastRun` | — | `AOTGFastRun` | 0 | Global | 0 | All Units run twice as fast. |
| Double Minor God Techs | `DoubleGodTech` | — | `AOTGDoubleGodTech` | 0 | Global | 0 | All God Techs (purple-border technologies) grant their effect twice. |
| Reincarnation | `Zombie` | — | `AOTGZombie` | 0 | Global | 0 | All living units respawn as aggressive Nature variants of themselves 30 seconds after dying. |
| Gaia's Remnants of Atlantis | `RemnantsOfAtlantis`, `RemnantsNoop`, `RemnantsD0`..`RemnantsD9` | `aotgRemnants_d0`..`aotgRemnants_d9` | `AOTGRemnantsOfAtlantis` | 0 ~ 9 | Global | 0 | Gaia spawns reinforcements across the map. Complete objectives to unfreeze them from Kronos' grasp. |
| Kronos' Time Vortex | `TimeVortexNoop`, `TimeVortexD0`..`TimeVortexD9` | `aotgTimeVortex_d0`..`aotgTimeVortex_d9` | — | 0 ~ 9 | Global | 0 | Kronos' allies from the past rally and attack any gods and their followers trying to stop his plans. |
| Freyr's Harsh Weather [^seasons] | `SeasonsNoop`, `SeasonsD0`..`SeasonsD5` | `aotgSeasons_d0`..`aotgSeasons_d5` | — | 0 ~ 5 | Global | 0 | Freyr's influence over the seasons as the weather gets harsher and harsher. |
| Loki's Harsh Weather [^seasons] | `SeasonsD6`..`SeasonsD9` | `aotgSeasons_d6`..`aotgSeasons_d9` | — | 6 ~ 9 | Global | 0 | Loki has taken over Freyr's seasons. It is getting colder and darker for longer. |
| Hel's Mythic Frenzy | `HelMinorWT` | `aotgMinorHel` | — | 1 | Global | 0 | Hel gives all myth units double hitpoints and removes favor cost, but they are trained at half speed. |
| Freyja's Second Ride | `FreyjaMinorWT` | `aotgMinorFreyja` | — | 1 | Global | 0 | Freyja makes infantry units spawn when cavalry falls in battle, but makes them more expensive to train. |
| Athena's Vanguard Heroism [^athena] | `AthenaMinorWT` | `aotgMinorAthena` | — | 1 | Global | 0 | Athena removes favor cost from heroes and makes them inspire allies upon death. |
| Aphrodite's Rite of Flourishing | `AphroditeMinorWT` | `aotgMinorAphrodite` | — | 1 | Global | 0 | Aphrodite slows down villager train time and increases cost, but multiplies all villagers after a while. |
| Oceanus' Driftwood Empire | `OceanusMinorWT` | `aotgMinorOceanus` | — | 1 | Global | 0 | Oceanus grants human players a huge stockpile of wood, but remove the ability to gather any more. |
| Prometheus' Secret Knowledge | `PrometheusMinorWT` | `aotgMinorPrometheus` | — | 1 | Global | 0 | Prometheus grants early access to a slowly constructing titan gate, but villagers are much less effective. |
| Thoth's Divine Wisdom | `ThothMinorWT` | `aotgMinorThoth` | — | 1 | Global | 0 | Thoth doubles the effect of all god technologies. |
| Horus' Ancestral Protection | `HorusMinorWT` | `aotgMinorHorus` | — | 1 | Global | 0 | Horus resurrects all units once from death after a while, but they have reduced hitpoints. |
| Wandering Resources [^wandering] | — (no `<effect>` entry) | — | — | — | — | — | Trees, Berries and Goldmines can wander across the map. |

[^seasons]: Seasons is one continuous escalation that swaps its display title mid-run. Difficulty tiers 0–5 read as "Freyr's Harsh Weather" (`STR_AOTG_RULE_SEASONS`); tiers 6–9 switch to "Loki's Harsh Weather" (`STR_AOTG_RULE_SEASONS_PHASE2`). The underlying XS rules form a single sequence (`aotgSeasons_d0`..`d9`) plus auxiliary `aotgSeasons_fimbulwinter` / `_fire` / `_freeze` / `_regrow` rules in `trigger/aotg/seasons.xs`.

[^athena]: The string table contains both `STR_AOTG_RULE_ATHENA_MINOR_DESC` ("Athena removes favor cost from heroes and gives them increased health, but make them lose health in battle.") and `STR_AOTG_RULE_ATHENA_MINOR_ALT_DESC` (the row text above). The active effect uses the `_ALT` titleid — the original "lose-health-in-battle" variant exists only as an unused string.

[^wandering]: `STR_AOTG_RULE_WANDERING_RESOURCES` exists in the string table but `aotg_effects.xml` contains no matching `<effect>` entry, no XS rule named for it appears in `trigger/aotg/*.xs`, and no `AOTGWandering*` tech is referenced. Listed for completeness — likely a future or removed feature.

## Field semantics

- **in-game title** — `<titleid>` resolved through `strings/English/string_table.txt`. What the player sees on the node card and the World Twists panel.
- **effect name(s)** — `<effect><name>` identifiers. Multiple variants exist for tiered twists (one per difficulty step). `*Noop` variants are deliberate no-ops bound to `aotgNoop`, used to slot the twist into a node without applying its effect.
- **xsrule(s)** — `<xsrule>` content. Names the XS rule the engine enables when the effect is activated. Implemented in `trigger/aotg/*.xs` (extracted to `extracted/aotg/trigger/aotg/`).
- **tech granted** — `<tech>` content. Names a tech in `aotg_techtree.techtree.XMB` whose stat-modifier `<effect>` blocks are applied to players. Tech-based twists hand the modification to the standard tech effect pipeline rather than running custom XS each tick.
- **difficulty** — `<difficulty>` value, or range for tiered twists. The engine selects a variant whose difficulty matches the node's tier.
- **applyflags** — `<applyflags>`. All world twists observed are `Global` (apply to every human player simultaneously); blessing entries use other values like per-player.
- **applytime (ms)** — `<applytime>`. Delay between activation request and effect application. All world twists use 0; some blessings use 1500 or 3500.
- **description** — `<descriptionid>` resolved through the string table. Player-facing flavour text.

## Mechanism summary

Two implementation routes coexist:

1. **Tech-based twists** (Giant Units, Low Gravity, Instant Techs, Lost Souls, Heavenly Haste, Double Minor God Techs, Reincarnation, Remnants of Atlantis base entry). The `<tech>` field names an `AOTG*` tech inside `aotg_techtree`. Activating the twist triggers the tech, whose effect blocks modify unit stats / tech costs / projectile parameters via the same machinery that handles regular god techs. Tunable purely through tech XML edits.

2. **XS-rule-based twists** (Meteor Hit, Idle Bolt, all `*MinorWT`, all tiered twists Seasons/Remnants/Time Vortex). The `<xsrule>` field names a rule defined in one of `trigger/aotg/{minor_wt,seasons,remnants,timevortex}.xs`. Activating the twist calls `xsEnableRule()` on it. Tunable by overlaying the XS file in a local mod at `mods/local/<name>/trigger/aotg/<file>.xs` (path needs empirical confirmation since the vanilla file lives inside `Data.bar` rather than loose on disk).

Tiered twists pick a different variant per node difficulty — the `_d0`..`_d9` XS rules are escalation steps, with `_d0` being the lightest application. `RemnantsOfAtlantis` and `*Noop` entries provide opt-out paths at the base tier.
