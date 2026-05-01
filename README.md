# Idle Auto-Repair (Age of Mythology: Retold mod)

A mod that auto-tasks idle repair-capable units (villagers for most civs, infantry for Norse) to repair nearby damaged friendly buildings via the engine's regular Repair action — costed normally, not free.

Player commands always win: a high-frequency watchdog detects when a unit is redirected by the player and removes it from the auto-repair plan within one frame.

## Repo layout

- `mod/idle_auto_repair/` — release-ready mod source.
  - `game/ai/human_assist/human_assist.xs` — copy of vanilla `human_assist.xs` with **one** added line (an `include` of `auto_repair.xs`). Patch-friendly.
  - `game/ai/human_assist/auto_repair.xs` — all the mod's logic.
- `mod/aom_autorepair_test/` — historical POC source (kept for reference; do not deploy).
- `docs/` — research notes, syntax reference, implementation plan.
- `extracted/` (gitignored) — vanilla data extracted with CryBar.Cli for reference. Re-extract on demand.
- `tools/` (gitignored) — CryBar.Cli download.

## Patch maintenance

The mod overlays `human_assist.xs`. After every game update that touches that file:

1. Copy the new vanilla file from `<install>/game/ai/human_assist/human_assist.xs` over `mod/idle_auto_repair/game/ai/human_assist/human_assist.xs`.
2. Re-add the single include line right after the existing includes:
   ```xs
   include "human_assist/auto_repair.xs"; // Idle Auto-Repair mod
   ```
3. Redeploy.

`auto_repair.xs` itself is independent and is unaffected by vanilla updates.

## Deploy (game must be closed)

```bash
DEPLOY="$HOME/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local/idle_auto_repair"
rm -rf "$DEPLOY"
cp -r mod/idle_auto_repair "$DEPLOY"
```

## Publishing (Age of Mythology Retold mod platform)

In-game flow:
1. `Mods → Mod Manager`
2. Select `idle_auto_repair` from the local mods list
3. Click `view` (bottom right)
4. Find the upload / publish action in the description view.

Web upload alternative: <https://www.ageofempires.com/mods/create/>

## Known limitations / future improvements

- **No per-unit toggle UI.** Auto-repair is on for all idle repair-capable units the player owns. Adding a per-unit toggle would require either UI surgery (`UIResources.bar` command-card buttons) or hotkey-driven selection toggling.
- **Units don't fight back when attacked while repairing.** A military unit doing auto-repair currently keeps repairing even if attacked; a villager keeps repairing even if attacked. Spec for the future: military units should fight back, villagers should run.
- **Multiplayer-ranked is not supported by AoMR's mod system in general** (anti-cheat / integrity-check).
