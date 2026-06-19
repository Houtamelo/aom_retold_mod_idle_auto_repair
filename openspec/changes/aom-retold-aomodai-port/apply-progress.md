# Apply Progress: aom-retold-aomodai-port

## Status

**Partial** — all code insertion, deployment, and structural verification are complete. The two in-game verification tasks remain for the user (this phase intentionally does not run the game).

## Completed Tasks

- [x] Phase 1.1 — Added state globals (`mRusher`, `gSecondRingWallPlanID`, `gSecondRingWallStartTime`, `gSecondRingWallLastDestroyedTime`, `gSecondRingAttackStartTime`, `gDebugSecondRing`) after `startTimeNoExtraBuildingsNeeded`.
- [x] Phase 1.2 — Wrapped the globals in `// === AoModAi: layered walls state begin/end ===` markers.
- [x] Phase 2.1 — Added `rule secondRingWallInit` in `defaultArchaicRules` that sets `mRusher = (cPersonalityCurrent == cPersonalityAttacker)` and disables itself.
- [x] Phase 3.1 — Added `int createSecondRingWallPlan(int baseID)` with ring type, radius `50.0`, culture-aware builder, priority 51, and start-time recording.
- [x] Phase 3.2 — Added `void destroySecondRingWallPlan(string reason)` for plan cleanup, timestamp reset, and cooldown recording.
- [x] Phase 3.3 — Added `bool isBaseUnderSustainedAttack(int baseID)` using `gDefendTCBases.find()` and `gEnemyPowerInBases`, plus the 19-minute early-game exemption.
- [x] Phase 4.1 — Added `rule secondRingWallPlanMonitor` in `defaultClassicalRules` with `minInterval 10`; early-destroys plan when `cStrategyFlagBuildWalls` is off.
- [x] Phase 4.2 — Implemented rusher delay (Age 3 + 15 min).
- [x] Phase 4.3 — Implemented creation gating: existing 1st-ring wall plan, Age 2+, 8 min, ≥ 10 villagers, ≥ 150 gold; destroys plan if gating drops while active.
- [x] Phase 4.4 — Implemented sustained-attack cancellation (only after 19 min, > 25 s of enemy power in base) with 60-second re-creation cooldown.
- [x] Phase 4.5 — Implemented 12-minute lifetime cap.
- [x] Phase 5.1 — Wrapped the new rule, helpers, and monitor in `// === AoModAi: layered walls begin/end ===` markers.
- [x] Phase 5.2 — Verified the file is 100% CRLF (no orphan CR/LF).
- [x] Phase 6.1 — Ran `scripts/deploy-mods.sh`; source and deployed `buildings.xs` are byte-identical (`cmp_exit=0`).

## Incomplete Tasks

- [ ] Phase 6.2 — Start a match and inspect the AI log for XS parse errors; confirm `secondRingWallPlanMonitor` is active. **Reason:** User explicitly requested no game launch; in-game verification is manual.
- [ ] Phase 6.3 — Validate spec scenarios B1–B12 manually in-game. **Reason:** Playtest is the user's responsibility per the constraints.

## Files Changed

| File | Action | Notes |
|------|--------|-------|
| `mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs` | Modified | Added state globals, `secondRingWallInit`, helpers, and `secondRingWallPlanMonitor`. All CRLF. |
| `openspec/changes/aom-retold-aomodai-port/tasks.md` | Modified | Marked completed tasks `[x]`. |
| `openspec/changes/aom-retold-aomodai-port/apply-progress.md` | Created | This file. |

## Git Status

```text
warning: in the working copy of 'mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs', CRLF will be replaced by LF the next time Git touches it
 mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs | 205 +++++++++++++++++++++
 1 file changed, 205 insertions(+)
```

Only the expected file is modified in the working tree (plus the two `openspec/...` markdown artifacts above). No accidental changes to other mod files.

## Deployment Status

- `scripts/deploy-mods.sh` completed without errors.
- Source and deployed `buildings.xs` are byte-identical (`cmp -l` returned exit 0, zero differences).

## Open Issues for the User

1. **B1–B12 require in-game manual verification.** The code matches the spec and design, but only a real match can confirm XS loads cleanly and the visual 2nd ring/rusher delay/attack-cancellation behaviors fire.
2. **CRLF Git warning.** Git warns that CRLF will be replaced by LF the next time Git touches the file. The on-disk file is still CRLF as required; ensure any future checkout preserves CRLF or run the same line-ending conversion before deployment.
3. **Rule enabling is implicit.** Both new rules use Retold's rule groups (`defaultArchaicRules` for the init rule, `defaultClassicalRules` for the monitor). `core/setup.xs` enables these groups automatically, so no explicit `xsEnableRule` is required.

## Implementation Notes

- The `kbUnitCount` argument order was corrected to the project's convention (`protoUnit, playerID, state`) for villager checks.
- The attack signal uses Retold's pre-computed `gEnemyPowerInBases[gDefendTCBases.find(baseID)]`; no custom unit query was added.
- The deferred 2× overwhelmed power-ratio check is documented in `design.md` but, per the open-question default, left for a future enhancement.
