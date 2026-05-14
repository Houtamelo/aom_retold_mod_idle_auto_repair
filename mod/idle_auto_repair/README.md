# Idle Auto-Repair (Age of Mythology: Retold mod)

Idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at normal resource cost.

When a unit capable of repairing is idle and a damaged friendly building is within its line of sight, the unit walks to the building and repairs it. The repair uses the engine's normal Repair action, so the same resources you would have spent on a manual repair are deducted from your stockpile as the building heals. Up to ten builders can converge on a single damaged building, and each idle unit targets the closest visible damaged building first.

The behavior applies to villagers for Greek, Egyptian, Atlantean, Chinese, Japanese, and Aztec civilizations. For Norse it applies to soldier-builders for any building type: Berserk, Throwing Axeman, Hersir, Hirdman, Huskarl, Godi, and the Heroes of Ragnarok. Norse villagers also participate, but only for the building types they can repair under your current major god — Houses only under Thor, Odin, and Loki, and any building type under Freyr.

Player commands always take priority. As soon as you give a unit any other order, the script removes it from its auto-repair plan immediately, and the unit follows your command without interference. The unit becomes eligible again only when it next returns to idle.

There is no per-unit toggle; the behavior is on for every idle repair-capable unit you own. Units do not interrupt repair to defend themselves when attacked, so a villager mid-repair will continue repairing through hostile fire rather than running away, and a military unit will not switch to fighting back. Buildings inside warzones (areas the engine flags as high-danger) are skipped so workers are not sent into combat. Buildings flagged non-repairable in the game's data are also skipped.

The mod adds a single include line to `game/ai/human_assist/human_assist.xs` and ships an `auto_repair.xs` file alongside it; that is the entire footprint.

## Mod layout

- `game/ai/human_assist/human_assist.xs` — copy of vanilla `human_assist.xs` with one extra line added (an `include` of `auto_repair.xs`).
- `game/ai/human_assist/auto_repair.xs` — all of the mod's logic, in a self-contained file. Read this first.

Development notes (a `proto_mods.xml` syntax reference, AI-script-hook viability research, the implementation plan that drove the work) and the historical proof-of-concept iterations live in the repository root under `docs/` and `mod/aom_autorepair_test/`.

## Local testing

Place the contents of this folder under `mods/local/idle_auto_repair/` in your Age of Mythology Retold user data directory. The portable way to find that directory: launch the game, open `Mods → Mod Manager`, select any installed mod, and click `Open Directory`; navigate up one level to the `local/` folder.

If you also use the Intelligent Auto-Scout mod, both mods overlay the same `human_assist.xs` and cannot be deployed as separate local mods. Use the combined deploy under `mod/intelligent_auto_repair_and_scout/` in the repository root instead.

## Patch maintenance

The mod overlays vanilla `human_assist.xs`. After any Age of Mythology Retold patch that touches that file:

1. Replace `game/ai/human_assist/human_assist.xs` with a fresh copy of the new vanilla file.
2. Re-add the include line right after the existing `include` directives:
   ```xs
   include "human_assist/auto_repair.xs"; // Idle Auto-Repair mod
   ```
3. Re-upload to the Age of Mythology Retold mod platform.

`auto_repair.xs` itself is independent of vanilla and is generally unaffected by game patches.

## Changelog

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

MIT. See `LICENSE` in the repository root.
