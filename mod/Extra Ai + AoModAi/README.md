# Extra Ai + AoModAi (Age of Mythology: Retold mod)

A curated port of the AI improvements from the published mod **314139 "extra ai"** by Garbhus, distilled to just the files that contain intentional modder edits. The mod overlays 22 vanilla AI files and ships nothing else — every overlay was verified modder-intentional (not stale base-game content), so each one carries real improvements and no others block future base-game updates.

The classification that produced this mod's file list is documented in `extracted/mod_analysis/subscribed_mod_314139_comparison.md` §7 and §8. A git-timeline review confirmed zero files have overlapping modder + developer edits, so every overlay here ports 1:1 with zero merge risk.

## What this mod does

Garbhus's published "extra ai" mod states five themes. The 22 files in this mod implement them as follows (each file lists the theme(s) it satisfies):

### Theme 1 — Island / amphibious / walls / relics / town centers

| File | Notes |
|------|-------|
| `core/shared/wonder/wonder_default_strategy.xs` | Adds a defensive wall ring (`mWallCircleAmount = 1`) to the Wonder strategy |
| `core/buildings/buildings.xs` | Land → Amphibious passability; widens settlement query |
| `core/buildings/buildings_economic.xs` | Land → Amphibious, looser eco-pop targets, gold-dropsite fallback radius 15 → 24 |
| `core/buildings/dropsite_placement.xs` | Rewrites gold-dropsite placement for amphibious terrain |
| `core/economy/resource_breakdown_system.xs` | Adds nearby-gold + remote-gold fallback paths |
| `core/exploration.xs` | Adds ~375 lines of island-relic collection (`relicIslandCollectionMonitor`, `relicTargetNeedsTransport`, etc.) |
| `core/military/naval_military.xs` | Lowers naval ratios, 1000 → 500 excess |
| `core/bo_system/bo_system.xs` | Heroize hooks (also theme 5) |
| `core/bo_system/bo_system_internal.xs` | Adds `findBOGoldKBResource` for amphibious gold mining |
| `core/shared/heroic/heroic_default_strategy.xs` | Longer attack intervals + wall ring (also theme 4) |
| `core/shared/mythic/mythic_default_strategy.xs` | Longer attack intervals + wall ring (also theme 4) |

### Theme 2 — Max military

| File | Notes |
|------|-------|
| `core/military/military_units.xs` | 3× hero pop cap, loosens military-pop unlock (Age 4 / 90% → Age 2 / 30%, 1000 → 500) |
| `core/military/military_attack.xs` | Doubles `mMinimumAttackSize` across all 5 attack branches (also theme 4) |
| `core/military/naval_military.xs` | (also theme 1) |
| `core/economy/economic_units.xs` | (also theme 3) |

### Theme 3 — Reduce resource bonus at high levels

| File | Notes |
|------|-------|
| `core/startup/game_settings_analysis.xs` | Lowers lobby handicap (Extreme 0.25 → 0.10, Legendary 0.50 → 0.20) |
| `core/techs.xs` | `armoryUpgradeChance` 15 → 35; loosens research gates (also theme 5) |
| `core/economy/economic_units.xs` | Aggressive `deleteExcessGatherers` (active, minInterval 240, always prune idles); loosens military-pop cap (Hard → Easy) |

### Theme 4 — Attack frequency / fewer suicide waves

| File | Notes |
|------|-------|
| `core/military/military_attack.xs` | (also theme 2) |
| `core/shared/heroic/heroic_default_strategy.xs` | (also theme 1) |
| `core/shared/heroic/heroic_turtler_strategy.xs` | Longer turtler Heroic intervals (18,14,10,8,6,6) → (18,14,12,10,8,8) |
| `core/shared/mythic/mythic_default_strategy.xs` | (also theme 1) |
| `core/shared/mythic/mythic_turtler_strategy.xs` | Longer turtler Mythic intervals |

### Theme 5 — God powers + upgrade logic

| File | Notes |
|------|-------|
| `core/godpowers/godpowers.xs` | Removes `isUnsupportedGodPower()` whitelist (unblocks EarthWall, Vanish, SmitingGust) |
| `core/godpowers/godpowers_chinese.xs` | Implements `cProtoPowerEarthWall` + `cProtoPowerVanish` + vanishMonitor |
| `core/godpowers/godpowers_japanese.xs` | Implements `cProtoPowerSmitingGust` |
| `core/atlantean/atlantean_archaic.xs` | Hooks `HeroizeStartingOracles` / `HeroizeStartingMurmillo` into both archaic paths |
| `core/bo_system/bo_system.xs` | Adds `HeroizeStartingOracles` + `HeroizeStartingMurmillo` rules |
| `core/techs.xs` | (also theme 3) |

## Mod layout

This mod is a set of 22 whole-file overlays under `game/ai/core/...`. The directory structure mirrors the base game exactly so the mod engine can replace the vanilla files at those paths:

```text
Extra Ai + AoModAi/
└── game/ai/
    └── core/
        ├── atlantean/
        │   └── atlantean_archaic.xs
        ├── bo_system/
        │   ├── bo_system.xs
        │   └── bo_system_internal.xs
        ├── buildings/
        │   ├── buildings.xs
        │   ├── buildings_economic.xs
        │   └── dropsite_placement.xs
        ├── economy/
        │   ├── economic_units.xs
        │   └── resource_breakdown_system.xs
        ├── exploration.xs
        ├── godpowers/
        │   ├── godpowers.xs
        │   ├── godpowers_chinese.xs
        │   └── godpowers_japanese.xs
        ├── military/
        │   ├── military_attack.xs
        │   ├── military_units.xs
        │   └── naval_military.xs
        ├── shared/
        │   ├── heroic/
        │   │   ├── heroic_default_strategy.xs
        │   │   └── heroic_turtler_strategy.xs
        │   ├── mythic/
        │   │   ├── mythic_default_strategy.xs
        │   │   └── mythic_turtler_strategy.xs
        │   └── wonder/
        │       └── wonder_default_strategy.xs
        ├── startup/
        │   └── game_settings_analysis.xs
        └── techs.xs
```

22 files. The 5 files that the source mod contained as stale base-game snapshots — `core/shared/archaic/build_order_strategy.xs`, `core/shared/archaic/nomad_strategy.xs`, `campaign/om/om05_p2.xs`, `campaign/om/om08_p2.xs`, `campaign/om/om08_p3.xs` — are intentionally **not** included. Including them would force the mod to lock in pre-Aztec-DLC content and prevent official developer updates to those files from reaching players.

## Compatibility

| Mod | Compatible? | Reason |
|-----|-------------|--------|
| `Idle Auto-Repair`            | ✅ Yes | Overlays `human_assist.xs`, not any of the 22 files here |
| `Intelligent Auto-Scout`      | ✅ Yes | Same as above |
| `Intelligent Auto-Repair and Scout` | ✅ Yes | Same as above |
| `Human Assist Improvements`   | ✅ Yes | Same as above |
| Mod 314139 "extra ai" (subscribed) | ❌ No  | Ships the same 22 files. Enable only one at a time. |

## Local testing and install

With Age of Mythology: Retold closed, copy the contents of this folder to your local mods directory:

```bash
# Default Linux/Proton path:
DEST="$HOME/.steam/steam/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local/Extra Ai + AoModAi"

mkdir -p "$DEST"
cp -r game "$DEST/"
```

To find the right path on your install, launch the game, open `Mods → Mod Manager`, select any installed mod, click `Open Directory`, and navigate up one level to the `local/` folder. The new mod's folder name should be exactly `Extra Ai + AoModAi` (or whatever you set when uploading it to the mod platform).

Launch the game, enable only **Extra Ai + AoModAi** (and optionally any of the compatible `human_assist_*` mods above), and start a match.

## Manual verification checklist

- [ ] **Deploy dry-run.** Confirm `mods/local/Extra Ai + AoModAi/game/ai/core/` contains all 22 files, organised in the seven subdirectories above. The 5 do-not-port files (under `core/shared/archaic/` and `campaign/om/`) must NOT be present.
- [ ] **No conflict with source mod.** Disable the subscribed mod 314139 "extra ai" before enabling this one — both overlay the same 22 files and only one can win.
- [ ] **No conflict with human_assist mods.** If combining with any of the four `human_assist_*` mods, the only shared concern is the deploy order; the file paths don't overlap, so they coexist.
- [ ] **In-game first load (USER step).** Launch AoM:R with only **Extra Ai + AoModAi** enabled, start a match, and confirm the mod loads without XS syntax errors. (If the base game has been patched since this mod was authored, one or more files may need a re-port — see Patch maintenance below.)
- [ ] **Theme spot-check (USER step).** Verify the listed behaviours: AI walls Wonder, larger attack waves, gold-dropsites on amphibious tiles, island relics collected, EarthWall + Vanish + SmitingGust cast by AI, armory upgrade chance visibly higher.

## Patch maintenance

Each of the 22 files in this mod is a whole-file overlay of a vanilla file. When an Age of Mythology: Retold patch touches any of them, the mod will keep using the old version and lose the patch's fixes/balances for that file.

**Maintenance workflow** (after every Steam update that changes any of the 22 files):

1. Identify which of the 22 files the devs touched. The fastest way: re-run the depot download (`scripts/fetch_aom_ai_part1.sh` and `scripts/fetch_aom_ai_part2.sh`) and diff the new HEAD against the previous one inside `./extracted/aom_depot_ai_diffs/`. Any file with changes in the new commit needs attention.
2. For each affected file, fetch the new vanilla version (the new depot's content) and the new modded version (re-download mod 314139 — the modder will likely have updated their mod for the patch).
3. Re-apply the per-file classification:
   - If the file is still content-equal to a historical depot with no later dev work → re-port: copy the new modded version over the file in this folder.
   - If the devs' changes overlap with the modder's → manual hunk merge (this has not happened yet, per the §8 adversarial review, but could on a future patch).
4. Run a smoke test in-game: enable only this mod, start a match, confirm no XS errors.

To remove the mod entirely, delete `mod/Extra Ai + AoModAi/` and remove any deploy entry from `scripts/deploy-mods.sh`.

## Provenance

- **Source mod**: 314139 "extra ai" by Garbhus, last updated 2026-05-26 (v16.8), 2,208 subscribers.
- **Classification method**: see `extracted/mod_analysis/subscribed_mod_314139_comparison.md` §7 (raw depot diff) and §8 (git-timeline review, 0 MIXED files confirmed).
- **Steam depot history**: see `extracted/aom_depot_ai_diffs/` (git repo with 6 Steam depots as commits, oldest first) and `extracted/steam_depot_manifests_since_2025_jan.md`.

## Changelog

**2026-06-18 — Initial release: cherry-pick from mod 314139 "extra ai"**
- 22 files ported, covering all five of Garbhus's stated themes.
- 5 do-not-port files (`core/shared/archaic/build_order_strategy.xs`, `core/shared/archaic/nomad_strategy.xs`, `campaign/om/om05_p2.xs`, `campaign/om/om08_p2.xs`, `campaign/om/om08_p3.xs`) intentionally excluded — they would lock in pre-Aztec-DLC content.

## License

MIT. See `LICENSE` in the repository root.
