# Idle Auto-Repair (Age of Mythology: Retold mod)

Idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at the normal resource cost. Player commands always take priority. See the in-game mod listing for full behavior, edge cases, and limitations.

## Layout

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
