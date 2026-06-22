# AOTG Chaos Events Reference

Source: `game/data/Data.bar :: gameplay/aotg_chaosevents.xml` (extracted via CryBar).

Engine codename: Gauntlet. Player-facing name: Arena of the Gods. Internal pool of 15 events; the engine picks one via `NextChaosEvent`, the rest is data-driven from this file.

| name | powerxmls | action | chaosplacementmode | unit spawned | unit count (min ~ max) | playercount (min ~ max) | minimumprecedingevents | triggeranotherchaoseventaftertime |
|---|---|---|---|---|---|---|---|---|
| GodPowerMeteor [^mp] | GauntletChaosMeteor | ActivateGodPower | AllTownCentersPerformanceLimited | — | — | — ~ 6 | — | — |
| GodPowerTornado | GauntletChaosTornado | ActivateGodPower | AllTownCentersPerformanceLimited | — | — | — ~ 6 | — | — |
| GodPowerEarthquake [^mp] | GauntletChaosEarthquake | ActivateGodPower | AllTownCentersPerformanceLimited | — | — | — ~ 6 | — | — |
| GodPowerThunderBurst | GauntletChaosThunderBurst | ActivateGodPower | AllTownCenters | — | — | — | — | — |
| GodPowerNatureNidhogg | GauntletChaosNidhogg | ActivateGodPower | SingleMiddleOfMap | — | — | 3 ~ — | — | — |
| GodPowerLightningStorm | GauntletChaosLightningStorm | ActivateGodPower | AllTownCenters | — | — | — ~ 6 | 2 | — |
| EnemyIncursion | — | SpawnIncursion | DistributeOnMap | see [^incursion] | tiered, see [^incursion] | — | 2 | — |
| GodPowerPestilence | GauntletChaosPestilence | ActivateGodPower | AllTownCenters | — | — | — | — | — |
| GodPowerSwampLand | GauntletChaosSwampland | ActivateGodPower | DistributeOnMap | — | — | — | — | — |
| CluckCluckBoom | CluckCluckBoom, VFXCluckCluckBoom, VFXHeavenLight | SpawnCluckCluckBoom | DistributeOnMap | — | — | — | — | — |
| Conscription | — | Conscription | AllTownCenters | — | — | — | — | — |
| ThatsNoMoon | GauntletChaosEclipse, GauntletChaosShinobiWanKenobi | ThatsNoMoon | DistributeOnMap | — | — | — | — | — |
| HadesMonkeys | — | SpawnIncursion | DistributeOnMap | GauntletChaosFlamingMonkey | default (1) | — | — | — |
| RebelliousHumans | — | SpawnIncursion | DistributeOnMap | Hoplite, Spearman, Berserk, Murmillo, DaoSwordsman, YariSpearman, TlamanihSpearman | 1 ~ 3 each | — | — | — |
| GodPowerCeaseFire | GauntletChaosCeaseFire | ActivateGodPower | SingleMiddleOfMap | — | — | — | — | 16000 ms |

[^mp]: `<disablein>Multiplayer</disablein>` — only fires in single-player runs.

[^incursion]: EnemyIncursion's spawn table has four tiers based on per-entry attributes.

  **Tier 1** (`mincount=1 maxcount=3 duplicate=2`, can spawn the same proto twice per event): Centaur, Minotaur, Cyclops, Sphinx, Wadjet, Anubite, Valkyrie, Troll, Einheri, Draugr, Promethean, Automaton, Qilin, Qiongqi, Yazi, Kamaitachi, Wanyudo, Jorogumo, LykaonWolf, CentzonTotochtin, Maquizcoatl, Chaneque.

  **Tier 2** (`mincount=1 maxcount=2`, no duplicates): Hydra, Manticore, NemeanLion, Petsuchos, Scarab, ScorpionMan, BattleBoar, RockGiant, Behemoth, Satyr, Taowu, Taotie, Baihu, StymphalianBird, Tengu, Raiju, Oni, Hamadryad, Ayotochtli, Tzitzimitl, ObsidianButterfly.

  **Tier 3** (no attributes — defaults to a single spawn): FrostGiant, MountainGiant, Medusa, Colossus, Chimera, Mummy, Avenger, FireGiant, FenrisWolfBrood, Fafnir, Centimanus, Argus, Lampades, Hundun, Phoenix, QingLong, ZhuQue, Asura, Onmoraki, Shinigami, HarpyMyth, Siren, Tunkuluchu, Ahuizotl, SoulGuide.

  **Tier 4** (`listofprotos=1 requiredifficulty=5` — gated behind difficulty 5, picks one from the list): TitanCerberus, TitanBird, TitanYmir, TitanAtlantean, TitanXingTian, TitanYamataNoOrochi.

## Field semantics

- **name** — event identifier (`<name>`). Internal handle; the engine selects by this. Distinct from the user-facing notification string.
- **powerxmls** — `<powername>` content. One or more power names from `gauntlet.xml` / `aotg.godpowers` / cross-culture `*.godpowers`. Comma-separated when multiple. All `GauntletChaos*` powers are dedicated chaos variants, isolated from player-castable powers.
- **action** — engine-side dispatch. Closed set of five values: `ActivateGodPower`, `SpawnIncursion`, `SpawnCluckCluckBoom`, `Conscription`, `ThatsNoMoon`. Each action consumes `<powername>` and/or `<protospawns>` differently.
- **chaosplacementmode** — where the event fires. Closed set: `AllTownCenters`, `AllTownCentersPerformanceLimited`, `SingleMiddleOfMap`, `DistributeOnMap`.
- **unit spawned** — `<protospawns>/<chaosspawn>` proto names. Only meaningful for `SpawnIncursion` (and indirectly the actions whose engine code references protos by name).
- **unit count (min ~ max)** — `mincount` / `maxcount` per `<chaosspawn>`. When absent the engine defaults to a single spawn. `duplicate="N"` allows the same proto to appear up to N times in one event; `listofprotos="1"` flags the comma-separated value as a pick-one list rather than a single proto.
- **playercount (min ~ max)** — `<minimumplayercount>` / `<maximumplayercount>`. Both optional; `—` on a side means no bound.
- **minimumprecedingevents** — event isn't eligible until at least N other chaos events have fired this match.
- **triggeranotherchaoseventaftertime** — milliseconds after this event fires before another is queued. Only Calm Before the Storm uses it (16 s), creating the eponymous setup.
