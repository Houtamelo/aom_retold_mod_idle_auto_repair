# AoM: Retold Auto-Repair Mod (WIP)

QoL mod adding a per-unit auto-repair toggle to villagers and Norse infantry.
When enabled and the unit is idle, the unit auto-tasks to repair nearby damaged
friendly buildings. Off by default.

## Layout
- `mod/` — the mod source files (mirrors the in-game mod folder layout).
- `docs/superpowers/plans/` — implementation plans.
- `extracted/` (gitignored) — vanilla game data extracted with CryBar.Cli for
  reference. Re-extract on demand; not committed.
- `tools/` (gitignored) — CryBar.Cli download for working with .bar/.XMB files.

## Status
Currently in path 1: testing whether `<type>AutoRepair</type>` is a working
ProtoAction type via a minimal proto_mods.xml. See
`docs/superpowers/plans/2026-05-01-aom-autorepair-path1.md`.

## Deployment (deferred — game must be closed)
Once the mod source is ready, deploy to AoM:R by symlinking
`mod/aom_autorepair_test` into the game's local mods folder:

```
ln -s /home/houtamelo/Documents/projects/aom_retold_mod/mod/aom_autorepair_test \
  "/home/houtamelo/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local/aom_autorepair_test"
```

This step must NOT be performed while AoM:R is running.
