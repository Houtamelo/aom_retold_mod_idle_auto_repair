# Human Assist Improvements (Age of Mythology: Retold mod)

Bundled superset of the Auto-Repair and Intelligent Auto-Scout mods, and the canonical home for future human-assist features.

- **Idle Auto-Repair** — idle units capable of repairing automatically walk to nearby damaged friendly buildings and repair them at normal resource cost. Covers villagers across every civilization and Norse soldier-builders.
- **Intelligent Auto-Scout** — replaces the engine's auto-scout button on land scouts and Atlantean Oracles with a smarter exploration system: scouts spread across the map, divert to wild herdables, send claimed herds home, and stay away from enemy threats.
- **Auto-relic-delivery** — when a human player's hero picks up a relic, the mod detects the ground relic's disappearance on a 2-second poll, finds player-owned heroes within 10 meters of the relic's last position, and tasks the first idle, plan-free hero carrying that exact relic unit ID once to the nearest player-owned temple with available relic space. The three cases for a missing relic (other-player pickup, our-pickup-but-busy, our-pickup-and-idle) are exhaustive — stale carries and player overrides are not retried; if the player wants to keep the relic, an active order prevents delivery. The canonical architecture is documented in `openspec/changes/auto-relic-delivery-reinvestigation/design.md`.

The individual mods' READMEs (see [Idle Auto-Repair](../idle_auto_repair/README.md) and [Intelligent Auto-Scout](../intelligent_auto_scout/README.md)) document the per-feature behaviour in full.

## ⚠️ Do not install alongside the standalone or combined mods

This mod, `Idle Auto-Repair`, `Intelligent Auto-Scout`, and `Intelligent Auto-Repair and Scout` all overlay the same vanilla file: `game/ai/human_assist/human_assist.xs`. AoM:R loads mods as whole-file replacements, not additive layers, so only one of these four mods can be active at a time. Enable only **Human Assist Improvements** (recommended as the single-mod choice going forward), or one of the three older mods if you want only that single feature. Loading two or more at once will cause one overlay to win unpredictably.

## Mod layout

- `game/ai/human_assist/human_assist.xs` — copy of vanilla `human_assist.xs` with the three `include` lines and the scout hook, plus room for future feature includes.
- `game/ai/human_assist/auto_relic_delivery.xs` — native feature logic for auto-relic-delivery.
- `game/ai/human_assist/auto_<future>.xs` — future features will live here.

The existing feature files `auto_repair.xs` and `auto_scout.xs` are **not duplicated** in this folder. `scripts/deploy-mods.sh` copies them from the canonical sibling directories at deploy time, so the shared source of truth is preserved.

## Thumbnail

`thumbnail_human-assist-improvements.png` is currently a placeholder copied from `Intelligent Auto-Repair and Scout`. A dedicated asset is planned for a future update.

## Local testing and install

Run `scripts/deploy-mods.sh` from the repository root with Age of Mythology: Retold closed:

```bash
./scripts/deploy-mods.sh
```

This copies the required files into your AoM:R `mods/local/Human Assist Improvements/` folder. Verify the folder contains:

```text
Human Assist Improvements/
└── game/ai/human_assist/
    ├── auto_repair.xs
    ├── auto_scout.xs
    ├── auto_relic_delivery.xs
    └── human_assist.xs
```

Launch the game, enable only **Human Assist Improvements**, and start a match.

## Manual verification checklist

- [ ] **Deploy dry-run.** Run `scripts/deploy-mods.sh` and confirm `mods/local/Human Assist Improvements/game/ai/human_assist/` contains `auto_repair.xs`, `auto_scout.xs`, `auto_relic_delivery.xs`, and `human_assist.xs`. The first three mods' deploy blocks should still print normally.
- [ ] **No regression to existing mods.** Verify the script still creates `Idle Auto-Repair/`, `Intelligent Auto-Scout/`, and `Intelligent Auto-Repair and Scout/` with the same files and unchanged content as before this mod was added.
- [ ] **In-game first load (USER step).** Launch AoM:R with only **Human Assist Improvements** enabled, start a match as a human player, and confirm the mod loads without XS syntax errors.
- [ ] **Feature parity walkthrough (USER step).** Idle a repair-capable villager near a damaged friendly building and toggle the auto-scout button on a land scout; inspect `aiEcho` output for both repair and scout state-machine logs. Repeat as an AI player to confirm both features stay silent.
- [ ] **Auto-relic-delivery walkthrough (USER step).** With a hero and a temple, pick up a relic; verify the hero walks once to the nearest temple with space and deposits it. Issue a manual order right after pickup and confirm no second delivery order is issued. Fill the nearest temple and confirm a farther one is chosen.
- [ ] **Rollback check.** If you need to revert this feature, delete `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs`, remove its include and `autoRelicDelivery_register()` call from the 4th mod's `human_assist.xs`, remove its deploy line in `scripts/deploy-mods.sh`, and revert the README edits.

## Patch maintenance

`human_assist.xs` is a whole-file vanilla overlay. After any Age of Mythology Retold patch that touches that file:

1. Replace `game/ai/human_assist/human_assist.xs` with a fresh copy of the new vanilla file.
2. Re-add the three include lines right after the existing `include` directives:
   ```xs
   include "human_assist/auto_repair.xs"; // Idle Auto-Repair mod
   include "human_assist/auto_scout.xs";  // Intelligent Auto-Scout mod
   include "human_assist/auto_relic_delivery.xs"; // Auto-relic-delivery mod (D1 divergence from intelligent_auto_repair_and_scout — see openspec/specs/auto-relic-delivery/spec.md)
   ```
3. Re-add the scout hook inside `enableAutoScouting`, immediately after the Oracle block and before `aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true)`:
   ```xs
   autoScout_register(planID, unitID); // Intelligent Auto-Scout mod (no-op for AI players)
   ```
4. Re-add the auto-relic-delivery registration call at the end of `main()`, after `disableVillagerAssist()`:
   ```xs
   autoRelicDelivery_register(); // Auto-relic-delivery mod (no-op for AI players)
   ```
5. Re-upload to the Age of Mythology Retold mod platform.

`auto_repair.xs` and `auto_scout.xs` continue to be maintained in their canonical sibling directories; `auto_relic_delivery.xs` is native to this mod.

## Changelog

**Auto-relic-delivery**
- Rewrote `auto_relic_delivery.xs` to use a 2-second ground-relic poll instead of `cXSRelicPickedUpHandler`. Detects relic disappearances, queries player-owned heroes within 10 meters of the relic's last position, and delivers only when a hero carries the exact relic unit ID while idle and has no active AI plan.
- No cross-tick state beyond the diff snapshot. The three cases for a missing relic are exhaustive, so a `(heroID, relicID)` pair tracker and a pending-retry window are not needed; player orders prevent delivery for as long as they keep the hero busy.
- The `autoRelicDelivery_register()` call in the 4th mod's `human_assist.xs` is unchanged; only the internals of `auto_relic_delivery.xs` changed.
- Documented poll-based behavior and patch-maintenance steps.

**Bootstrap — bundled Auto-Repair + Intelligent Auto-Scout**
- Added `human_assist.xs` overlay that includes both feature files and registers the scout hook.
- Reused `auto_repair.xs` from `mod/idle_auto_repair/` and `auto_scout.xs` from `mod/intelligent_auto_scout/` at deploy time.
- Added deploy block, placeholder thumbnail, and conflict documentation.

## License

MIT. See `LICENSE` in the repository root.
