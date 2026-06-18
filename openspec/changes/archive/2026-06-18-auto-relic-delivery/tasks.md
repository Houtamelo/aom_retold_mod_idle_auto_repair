# Tasks: Auto-relic-delivery (event-driven)

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated total changed lines | ~380–580 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR (module → wiring → docs) |
| Delivery strategy | auto-chain |
| Chain strategy | pending |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

## Phase 1: Module skeleton

- [x] 1.1 Create `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` with a header block matching `auto_repair.xs`.
- [x] 1.2 Add constants `cAutoRelic_MaxTrackerPairs`, `cAutoRelic_RetryInterval`, and per-hero state constants.
- [x] 1.3 Add globals: parallel arrays `gAutoRelic_heroID[]`/`relicID[]`, state array, retry flag/counter, and lazy query handles.
- [x] 1.4 Add stub `autoRelicDelivery_register()` with human-only guard.

## Phase 2: Registration + tracker

- [x] 2.1 Implement `autoRelicDelivery_register()`: early-return non-human and register `aiSetHandler("autoRelicDelivery_onPickedUp", cXSRelicPickedUpHandler)`.
- [x] 2.2 Implement `autoRelicDelivery_hasPair(heroID, relicID)` via linear scan of the parallel arrays.
- [x] 2.3 Implement `autoRelicDelivery_addPair(heroID, relicID)` with size guard and overflow `aiEcho` warning.

## Phase 3: Handler body + retry

- [x] 3.1 Implement `autoRelicDelivery_onPickedUp(int techID = -1)` with human guard and entry `aiEcho`.
- [x] 3.2 Lazily create the idle-hero query (`cUnitTypeHero`, `cActionTypeIdle`) and execute it inside the handler.
- [x] 3.3 Iterate candidates: skip non-idle heroes; if `kbUnitGetNumberContained(heroID) > 0` and pair not tracked, attempt delivery.
- [x] 3.4 Add `rule autoRelicDelivery_tickRetry minInterval 2 inactive` that fires once when `gAutoRelic_retryPending` is true.

## Phase 4: Temple lookup + delivery

- [x] 4.1 Lazily create the temple query (`cUnitTypeTemple`, alive, ascending sort by distance from hero).
- [x] 4.2 Implement `autoRelicDelivery_findNearestTempleWithSpace(heroID)`: return first temple with `kbUnitGetNumberContained(templeID) < kbPlayerGetProtoStatInt(...)`, else `-1` and `aiEcho` "no temple with space".
- [x] 4.3 Implement `autoRelicDelivery_issueDelivery(heroID, relicID)`: select temple, call `aiTaskWorkUnit(heroID, templeID)`, add pair to tracker, and set state.
- [x] 4.4 Add the design state-machine transitions; fall back to per-hero tracking if relic ID unavailable. (Superseded by remediation: real pair-tracker is now active.)

## Phase 5: Wiring + deploy

- [x] 5.1 Add `include "human_assist/auto_relic_delivery.xs";` after the two existing includes in `human_assist_improvements/.../human_assist.xs`.
- [x] 5.2 Call `autoRelicDelivery_register();` at the end of `human_assist.xs::main()` after `disableVillagerAssist()`.
- [x] 5.3 Add maintainer comments marking the D1 divergence from the combined-mod `human_assist.xs`.
- [x] 5.4 Add one deploy line in `scripts/deploy-mods.sh` for `$HUMAN_SRC/auto_relic_delivery.xs` under the 4th-mod block.

## Phase 6: Documentation + verification

- [x] 6.1 Update `mod/human_assist_improvements/README.md` with feature bullet, singleton-handler caveat, and patch-maintenance note.
- [x] 6.2 Update top-level `README.md` to list Auto-relic-delivery and repeat the conflict warning.
- [x] 6.3 Structural checks: `bash -n scripts/deploy-mods.sh`; `git diff --stat` limited to expected files; `cmp` shows expected `human_assist.xs` divergence from combined mod.
- [x] 6.4 Manual in-game verification (USER): deploy, launch with only Human Assist Improvements, and run spec scenarios B1–B11.

## Phase 7: Remediation — wire real pair tracker

- [x] 7.1 Replace per-hero state machine with `(heroID, relicID)` pair tracker as the active runtime path.
- [x] 7.2 Use `kbUnitGetContainedUnitByIndex(heroID, 0)` to read the carried relic unit ID.
- [x] 7.3 Use `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic)` as the type-safe carrying check.
- [x] 7.4 Use `kbRelicGetTechID(relicID)` for defensive correlation against the `cXSRelicPickedUpHandler` `techID` payload.
- [x] 7.5 Remove dead `cAutoRelic_RelicIDUnknown` sentinel, per-hero state arrays/helpers, and `gAutoRelic_retryCounter`.
- [x] 7.6 Revert cosmetic whitespace realignment in the 4th mod's `human_assist.xs` so the diff is purely additive.
- [x] 7.7 Amend spec, design, archive-report, and README to document the real APIs and removal of the fallback.
- [x] 7.8 Re-run structural checks and deploy dry-run; leave in-game verification for `sdd-verify`.

## Rollback checklist

- [x] Delete `auto_relic_delivery.xs`; revert include/registration edits in the 4th mod's `human_assist.xs`.
- [x] Revert the deploy line and both README edits.
