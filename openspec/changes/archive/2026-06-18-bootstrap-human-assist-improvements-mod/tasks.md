# Tasks: Bootstrap Human Assist Improvements bundled mod

## Review Workload Forecast

| Field | Value |
|---|---|
| Estimated changed lines | ~1050 |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR 1: mod skeleton (verbatim `human_assist.xs` copy) ~993 lines, needs size-exception; PR 2: deploy + thumbnail + docs ~120 lines |
| Delivery strategy | auto-forecast |
| Chain strategy | stacked-to-main |

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|---|---|---|---|
| 1 | Add `mod/human_assist_improvements/` skeleton with `human_assist.xs` | PR 1 | Verbatim copy of combined overlay; single-file exception due to engine overlay size |
| 2 | Wire deploy script, thumbnail, and READMEs | PR 2 | Depends on PR 1 landing or can target PR 1 branch if stacked |

## Phase 1: Mod skeleton

- [x] 1.1 Create directory `mod/human_assist_improvements/game/ai/human_assist/`.
- [x] 1.2 Copy `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` to `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs`; verify it includes `human_assist/auto_repair.xs`, `human_assist/auto_scout.xs`, and calls `autoScout_register(planID, unitID)` inside `enableAutoScouting`.
- [x] 1.3 Run `cmp`/`diff` to confirm the new file is byte-identical to the combined-mod source (or note any deliberate future-extensibility markers as separate follow-up).

## Phase 2: Deploy script

- [x] 2.1 Add `HUMAN_SRC="$SRC_ROOT/human_assist_improvements/game/ai/human_assist"` after `COMBINED_SRC` in `scripts/deploy-mods.sh`.
- [x] 2.2 Append a fourth `=== Human Assist Improvements ===` block that copies `auto_repair.xs` from `$REPAIR_SRC`, `auto_scout.xs` from `$SCOUT_SRC`, and `human_assist.xs` from `$HUMAN_SRC` into `$DEPLOY_ROOT/Human Assist Improvements/game/ai/human_assist/`, without editing the first three blocks.
- [x] 2.3 Update the script header comment to list all four mods.
- [x] 2.4 Run `bash -n scripts/deploy-mods.sh`; then run a dry-run against a temp `AOMR_LOCAL_MODS` directory and confirm all four target trees are created with the expected files.

## Phase 3: Thumbnail

- [x] 3.1 Copy `thumbnail_idle-auto-repair-and-intelligent-auto-scout.png` to `thumbnail_human-assist-improvements.png`; verify byte-identity with `cmp`.

## Phase 4: Documentation

- [x] 4.1 Create `mod/human_assist_improvements/README.md`: bundle description, install steps, conflict warning versus `Idle Auto-Repair`, `Intelligent Auto-Scout`, and `Intelligent Auto-Repair and Scout`, plus a patch-maintenance note for the vanilla `human_assist.xs` overlay.
- [x] 4.2 Update top-level `README.md`: add the fourth mod to the mod list and note that all four mods shadow the same `human_assist.xs` so only one may be enabled.

## Phase 5: Verification (manual)

- [x] 5.1 Deploy dry-run (MANUAL): close AoM:R, run `scripts/deploy-mods.sh`, and confirm `mods/local/Human Assist Improvements/game/ai/human_assist/` contains `auto_repair.xs`, `auto_scout.xs`, and `human_assist.xs` at the expected paths.
- [x] 5.2 Regression check (MANUAL): verify the three existing mods still deploy to their original paths with unchanged content; compare md5/path output against a pre-change run if available.
- [x] 5.3 In-game first load (MANUAL): launch AoM:R with only `Human Assist Improvements` enabled, start a match as a human player, and confirm the mod loads without XS syntax errors.
- [x] 5.4 Feature parity walkthrough (MANUAL): idle a repair-capable villager near a damaged friendly building and toggle the auto-scout button on a land scout; inspect `aiEcho` output for both repair and scout state-machine logs; repeat as an AI player to confirm both features remain silent.
