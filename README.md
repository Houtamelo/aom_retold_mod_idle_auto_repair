# Idle Auto-Repair (Age of Mythology: Retold mod)

Idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at normal resource cost.

When a unit capable of repairing is idle and a damaged friendly building is within its line of sight, the unit walks to the building and repairs it. The repair uses the engine's normal Repair action, so the same resources you would have spent on a manual repair are deducted from your stockpile as the building heals. Up to ten builders can converge on a single damaged building, and each idle unit targets the closest visible damaged building first.

The behavior applies to villagers for Greek, Egyptian, Atlantean, Chinese, Japanese, and Aztec civilizations. For Norse it applies to soldier-builders for any building type: Berserk, Throwing Axeman, Hersir, Hirdman, Huskarl, Godi, and the Heroes of Ragnarok. Norse villagers also participate, but only for the building types they can repair under your current major god — Houses and Farms under Thor, Odin, and Loki, and any building type under Freyr.

Player commands always take priority. As soon as you give a unit any other order, the script removes it from its auto-repair plan immediately, and the unit follows your command without interference. The unit becomes eligible again only when it next returns to idle.

There is no per-unit toggle; the behavior is on for every idle repair-capable unit you own. Units do not interrupt repair to defend themselves when attacked, so a villager mid-repair will continue repairing through hostile fire rather than running away, and a military unit will not switch to fighting back. Buildings inside warzones (areas the engine flags as high-danger) are skipped so workers are not sent into combat. Buildings flagged non-repairable in the game's data are also skipped.

The mod adds a single include line to `game/ai/human_assist/human_assist.xs` and ships an `auto_repair.xs` file alongside it; that is the entire footprint.

## Known issues

1. Repairers will endlessly attempt to path to damaged buildings that can't be reached (common for towers surrounded by houses). I plan to address this in a future release. For now, you must identify such cases yourself and manually give the unit another order.

## Repository layout

- `mod/idle_auto_repair/game/ai/human_assist/human_assist.xs` — copy of vanilla `human_assist.xs` with one extra line added (an `include` of `auto_repair.xs`).
- `mod/idle_auto_repair/game/ai/human_assist/auto_repair.xs` — all of the mod's logic, in a self-contained file. Read this first.
- `mod/aom_autorepair_test/` — historical proof-of-concept iterations. Kept for reference; not deployed.
- `docs/` — development notes: a `proto_mods.xml` syntax reference, AI-script-hook viability research, and the implementation plan that drove the work.

## Local testing

Place the contents of `mod/idle_auto_repair/` under `mods/local/idle_auto_repair/` in your Age of Mythology Retold user data directory. The portable way to find that directory: launch the game, open `Mods → Mod Manager`, select any installed mod, and click `Open Directory`; navigate up one level to the `local/` folder.

## Patch maintenance

The mod overlays vanilla `human_assist.xs`. After any Age of Mythology Retold patch that touches that file:

1. Replace `mod/idle_auto_repair/game/ai/human_assist/human_assist.xs` with a fresh copy of the new vanilla file.
2. Re-add the include line right after the existing `include` directives:
   ```xs
   include "human_assist/auto_repair.xs"; // Idle Auto-Repair mod
   ```
3. Re-upload to the Age of Mythology Retold mod platform.

`auto_repair.xs` itself is independent of vanilla and is generally unaffected by game patches.

## License

MIT. See `LICENSE`.
