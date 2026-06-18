# Archive Report: Auto-relic-delivery

The `auto-relic-delivery` change is fully archived. The event-driven relic-delivery capability is promoted to the openspec main spec store, and all SDD phase artifacts are preserved below.

## Quick path

1. Main spec created at `openspec/specs/auto-relic-delivery/spec.md` (additive, no destructive merge).
2. Change folder moved to `openspec/changes/archive/2026-06-18-auto-relic-delivery/`.
3. Verification verdict was **PASS WITH WARNINGS** — 0 CRITICAL, 4 WARNINGs, 3 SUGGESTIONs.

## Change summary

| Field | Value |
|---|---|
| Change name | `auto-relic-delivery` |
| Intent | Add a passive human-assist feature to the **Human Assist Improvements** mod: when a human player's hero picks up a relic, task it once to the nearest player-owned temple with available space |
| Capability | `auto-relic-delivery` |
| Nature | Purely additive: new feature file, one include + one registration call in the 4th mod's `human_assist.xs`, one deploy line, README updates |
| Existing mod files modified | None (`auto_repair.xs` and `auto_scout.xs` shared sources untouched) |

## Phase roll-up

| Phase | Status |
|---|---|
| explore | done |
| propose | done |
| spec | done |
| design | done |
| tasks | done |
| apply | done |
| verify | **PASS WITH WARNINGS** |

## Specs synced

| Domain | Action | Details |
|---|---|---|
| `auto-relic-delivery` | Created | 11 requirements + 11 scenarios promoted from the delta spec to `openspec/specs/auto-relic-delivery/spec.md`. No existing main spec existed; no destructive deltas were merged. |

## Source of truth updated

- `openspec/specs/auto-relic-delivery/spec.md`

## Files / deltas delivered

| File | Action | Description |
|---|---|---|
| `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` | Create | Feature logic: event handler, `(heroID, relicID)` pair-tracker (active runtime path via `kbUnitGetContainedUnitByIndex` + `kbRelicGetTechID`), one-shot retry rule, temple-with-space lookup, `aiTaskWorkUnit` delivery (post-remediation, ~300 lines) |
| `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` | Modify | Add `include "human_assist/auto_relic_delivery.xs";` and call `autoRelicDelivery_register();` at end of `main()` |
| `scripts/deploy-mods.sh` | Modify | Additive deploy line for `$HUMAN_SRC/auto_relic_delivery.xs` in the 4th-mod block |
| `mod/human_assist_improvements/README.md` | Modify | Feature description, singleton-handler caveat, patch-maintenance note |
| `README.md` | Modify | Auto-relic-delivery added to mod list; conflict warning repeated |

## Design update: per-hero state machine removed

The original archive noted a per-hero state-machine fallback because the prior exploration believed AoM:R exposed no API to read the relic unit ID carried by a hero. A re-investigation of the engine KB docs found:
- `kbUnitGetContainedUnitByIndex(heroID, 0)` returns the carried relic unit ID (`kbfuncs_8cpp.html:237`).
- `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic)` returns the type-safe contained relic count (`kbfuncs_8cpp.html:235`).
- `kbRelicGetTechID(relicID)` correlates a relic unit ID with the `cXSRelicPickedUpHandler` `techID` payload (`kbfuncs_8cpp.html:926`).

The remediation replaces the per-hero state machine with the `(heroID, relicID)` pair tracker as the active runtime path. The `cAutoRelic_RelicIDUnknown` sentinel and `gAutoRelic_state[]` array were removed, and the `gAutoRelic_retryCounter` dead state was eliminated. The verify phase will be re-run to confirm the new runtime path works in-game.

## Verify verdict

- **Verdict:** PASS WITH WARNINGS (original verify, prior to remediation)
- **CRITICAL:** 0
- **WARNING:** 4
- **SUGGESTION:** 3

### Warnings (as originally reported)

1. **`human_assist.xs` diff was `+8/-2` rather than purely additive.** Two existing include-comment lines were whitespace-realigned. **Addressed by remediation:** the realignment was reverted and the file is now LF-only, producing a purely additive diff.
2. **CRLF→LF normalization occurred on `human_assist.xs`.** **Addressed by remediation:** the file is written with LF line endings consistent with HEAD.
3. **Per-hero fallback was the runtime path; ideal pair tracker was dead code at runtime.** **Addressed by remediation:** the active runtime path now uses `kbUnitGetContainedUnitByIndex(heroID, 0)` and `kbRelicGetTechID(relicID)`; the per-hero state machine was removed entirely.
4. **`gAutoRelic_retryCounter` was dead state.** **Addressed by remediation:** `gAutoRelic_retryCounter` and `cAutoRelic_RetryMaxAttempts` were removed; the retry is bounded by the flag-clear alone.

### Suggestions status

1. **Revert whitespace realignment on the two existing include lines.** **Enacted during remediation.**
2. **Remove `gAutoRelic_retryCounter` or wire it to enforce `cAutoRelic_RetryMaxAttempts`.** **Enacted during remediation** (removed entirely; Option A: flag-clear already bounds the retry to one attempt).
3. **Add a header note that `cUnitTypeHero` excludes culture-specific carriers.** Remains rejected; spec OUT-OF-SCOPE already covers this.

## Patch-maintenance notes

- The 4th mod's `human_assist.xs` now carries **two intentional sets of edits** that must be reapplied after any vanilla patch rebase:
  1. Bootstrap edits: includes for `auto_repair.xs` and `auto_scout.xs`, plus the `autoScout_register()` call inside `enableAutoScouting`.
  2. Auto-relic-delivery edits: include for `auto_relic_delivery.xs` and `autoRelicDelivery_register()` at end of `main()`.
- Expected post-rebase diff on the 4th mod's `human_assist.xs`: two feature includes near the top, one scout registration call inside `enableAutoScouting`, and one relic registration call at the end of `main()` (plus any maintainer comments documenting D1 divergence).
- Canonical feature sources:
  - `auto_repair.xs` → `mod/idle_auto_repair/game/ai/human_assist/auto_repair.xs`
  - `auto_scout.xs` → `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`
  - `auto_relic_delivery.xs` → `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` (native to the 4th mod)

## Rollback procedure

1. Delete `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs`.
2. Revert the `include "human_assist/auto_relic_delivery.xs";` line and `autoRelicDelivery_register();` call in the 4th mod's `human_assist.xs`. This restores bootstrap byte-identity with the combined mod's `human_assist.xs` (if the cosmetic whitespace realignment is also reverted).
3. Revert the new deploy line in `scripts/deploy-mods.sh`.
4. Revert the auto-relic-delivery additions in `mod/human_assist_improvements/README.md` and `README.md`.

No changes to `auto_repair.xs`, `auto_scout.xs`, or the three existing mods are needed.

## Deferred-item backlog

All backlog items from the original verify report were enacted in the remediation and are now complete:

1. ✅ Reverted the whitespace realignment on the two existing include-comment lines in `human_assist_improvements/.../human_assist.xs`; the diff is now purely additive.
2. ✅ Removed `gAutoRelic_retryCounter` (Option A: flag-clear already bounds the retry to one attempt).

No deferred items remain. The next phase is `sdd-verify` to confirm the new pair-tracker runtime path works in-game.

## Archive contents

- `exploration.md` ✅
- `proposal.md` ✅
- `specs/auto-relic-delivery/spec.md` ✅
- `design.md` ✅
- `tasks.md` ✅ (all tasks complete)
- `verify-report.md` ✅
- `archive-report.md` ✅

## Traceability

- Apply progress recorded in Engram observation `#989` (`sdd/auto-relic-delivery/apply-progress`).
- This archive report persisted in Engram under topic key `sdd/auto-relic-delivery/archive-report`.

## Pre-publish notes for orchestrator

The remediation addressed all four original verify warnings and enacted the two actionable suggestions. The remaining step is `sdd-verify` to produce a fresh verify-report against the pair-tracker runtime path. Do not publish before the fresh verify is complete; the engine APIs `kbUnitGetContainedUnitByIndex`, `kbRelicGetTechID`, and `kbUnitGetNumberContainedOfType` were found in the retail docs but still need in-game confirmation (scenario B7).

## Next step

`sdd-verify` (fresh-context verification of the remediated implementation).
