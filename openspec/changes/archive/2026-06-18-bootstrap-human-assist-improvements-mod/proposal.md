# Proposal: Bootstrap the HumanAssistImprovements bundled mod

## Intent

The three existing mods overlay the same vanilla `human_assist.xs`. Without a canonical bundle, every new feature would multiply combined mods. Add a fourth mod, **Human Assist Improvements**, bundling Auto-Repair + Intelligent Auto-Scout as the single home for future human-assist features.

## Scope

### In Scope

- Create `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` mirroring the combined mod.
- Add `mod/human_assist_improvements/README.md` with bundle description, install steps, patch note, and conflict warning.
- Extend `scripts/deploy-mods.sh` for `Human Assist Improvements`, copying `auto_repair.xs` and `auto_scout.xs` from sibling dirs.
- Update top-level `README.md` mod list and conflict note.
- Add a placeholder thumbnail copied from the combined-mod thumbnail.

### Out of Scope

- New feature logic; deferred to later changes.
- Private copies of `auto_repair.xs` / `auto_scout.xs` (canonical in standalone dirs).
- Platform manifest or packaging metadata.

## Capabilities

### New Capabilities

- `human-assist-improvements-mod-bootstrap`: Skeleton wiring and deploy integration.

### Modified Capabilities

- None.

## Approach

Copy `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` as the 4th mod overlay so it includes both feature files and calls `autoScout_register` inside `enableAutoScouting`. Commit only that file; reuse `auto_repair.xs` and `auto_scout.xs` from `idle_auto_repair/` and `intelligent_auto_scout/` at deploy time. Future features will add own `auto_<feature>.xs` files in the new directory and include them.

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `mod/human_assist_improvements/` | New | Skeleton, overlay, README. |
| `scripts/deploy-mods.sh` | Modified | Fourth deploy block. |
| `README.md` | Modified | Mod list + conflict note. |
| `thumbnail_human-assist-improvements.png` | New | Placeholder thumbnail. |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Vanilla patch invalidates overlay | High | Same burden as existing mods; README documents it; clean rollback. |
| Player loads alongside existing mod | High | READMEs state conflict clearly. |
| No automated XS tests | High | Deploy dry-run + in-game `aiEcho`. |
| Thumbnail/platform requirements unknown | Med | Placeholder for bootstrap. |

## Rollback Plan

1. Delete `mod/human_assist_improvements/`.
2. Revert `scripts/deploy-mods.sh` additions.
3. Revert `README.md` additions.
4. Delete placeholder thumbnail.

No existing mod files are modified; rollback is clean.

## Dependencies

- None.

## Success Criteria

- [ ] New `human_assist.xs` with both includes and scout hook.
- [ ] Deploy script copies feature files from sibling dirs.
- [ ] Existing three mods deploy unchanged.
- [ ] READMEs conflict warning present.
- [ ] Dry-run lists 4th mod files.
