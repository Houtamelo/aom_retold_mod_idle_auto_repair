# Tasks: Auto-relic-delivery re-investigation (poll-based trigger)

## Status

| Field | Value |
|---|---|
| Change id | `auto-relic-delivery-reinvestigation` |
| Phase | `sdd-tasks` |
| Status | draft |
| Date | 2026-06-19 |

## Review Workload Forecast

| Field | Value |
|---|---|
| Estimated total changed lines | ~270–395 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR (code + docs in one commit; archive move as follow-up) |
| Delivery strategy | auto-forecast |
| Chain strategy | pending |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

## Summary

Rewrite `auto_relic_delivery.xs` so the feature triggers on a 2-second ground-relic poll instead of the non-functional `cXSRelicPickedUpHandler`. The new scan detects relic disappearances, correlates each with the nearby hero carrying the exact relic unit ID, and delivers only when the hero is idle and has no AI plan. The diff snapshot is the only state retained across ticks; no `(heroID, relicID)` pair tracker and no pending-retry window are needed because the three cases for a missing relic are exhaustive. The nearest-temple and human-player guard patterns are preserved.

## Scope estimate

| File | Expected changed lines | Note |
|---|---|---|
| `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` | 250–350 | Full rewrite (new relic query, snapshot state, pending lists, scan rule, helper functions). |
| `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` | 0–5 | Verify registration call; adjust line only if needed. |
| `mod/human_assist_improvements/README.md` | 20–40 | Update feature description and remove singleton-handler caveat. |
| `AGENTS.md` | 0 | No file exists; skip. |
| `scripts/deploy-mods.sh` | 0 | No script change. |
| `scripts/extract-ai-logs.sh` | 0 | No script change; produces `.tmp/` output. |
| Archive move | ~10 | Git rename of change folder to `openspec/changes/archive/YYYY-MM-DD-auto-relic-delivery-reinvestigation/`. |
| **Total** | **~270–395** | Well under the 400-line budget. |

## Task list

### T1 — Rewrite `auto_relic_delivery.xs`

- [x] **Description:** Replace the current implementation with the poll-based design. Add `gAutoRelic_relicQuery` and snapshot arrays (`gAutoRelic_prevRelicIDs[]`, `gAutoRelic_prevRelicPositions[]`). Implement `autoRelicDelivery_setupRelicQuery`, `autoRelicDelivery_setupHeroProximityQuery`, `autoRelicDelivery_findHeroesInRange`, `autoRelicDelivery_heroCarriesRelic`, `autoRelicDelivery_heroIsDeliverable`, `autoRelicDelivery_handleDisappearance`. Preserve `autoRelicDelivery_findNearestTempleWithSpace`. Remove the pair tracker (`hasPair`, `addPair`, `gAutoRelic_heroID`, `gAutoRelic_relicID`), the pending-disappearance arrays, and the one-shot retry rule. Add the `autoRelicDelivery_scanRelics` rule (`minInterval 2`) which performs the snapshot, diff, and per-disappearance handling inline. Rewrite `autoRelicDelivery_register()` to enable only the scan rule, keeping the `xsSetContextPlayer(cMyID)` / `xsSetContextPlayer(-1)` defensive wrapper and human-player guard. Include the diagnostic `aiEcho` banners listed in `design.md`.
- **Acceptance criteria:** The file compiles/loads (deploy script copies it without errors) and in-game `aiEcho` shows the registration banner plus tick-start banner.
- **Files affected:** `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs`
- **Dependencies:** None.
- **Commit suggestion:** `feat(auto-relic): poll-based ground-relic delivery` (with T2 and T3 if clean).
- **Estimated diff lines:** 250–350.

### T2 — Verify `human_assist.xs` registration site

- [x] **Description:** Confirm that `autoRelicDelivery_register()` is called at the end of `human_assist.xs::main()` after `disableVillagerAssist()` and that the surrounding `xsSetContextPlayer(cMyID)` restoration is intact. No signature change is expected, so this task is normally a read-only verification; only adjust the call site if the new register body requires it.
- **Acceptance criteria:** `human_assist.xs` still compiles/loads after the change and the registration banner appears in the AI log.
- **Files affected:** `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs`
- **Dependencies:** T1.
- **Commit suggestion:** Same commit as T1 (registration call is unchanged).
- **Estimated diff lines:** 0–5.

### T3 — Update `mod/human_assist_improvements/README.md`

- [x] **Description:** Refresh the auto-relic-delivery bullet to describe the poll-based behavior, the 2-second ground-relic scan, the 10 m hero proximity radius, and the idle + no-plan guard. Remove or rewrite the **Singleton-handler caveat** section because `cXSRelicPickedUpHandler` is no longer registered. Update the changelog entry to match the new trigger and retry model.
- **Acceptance criteria:** README no longer claims the handler is registered, accurately describes the poll mechanism, and still documents the conflict warning and patch-maintenance steps.
- **Files affected:** `mod/human_assist_improvements/README.md`
- **Dependencies:** T1 (so the description matches the implemented code).
- **Commit suggestion:** Same commit as T1 (docs belong with the code change they explain).
- **Estimated diff lines:** 20–40.

### T4 — Deploy via `scripts/deploy-mods.sh`

- [x] **Description:** Run `scripts/deploy-mods.sh` with Age of Mythology: Retold closed. Confirm the `Human Assist Improvements` local-mod folder contains the new `auto_relic_delivery.xs`, `human_assist.xs`, `auto_repair.xs`, and `auto_scout.xs`.
- **Acceptance criteria:** Deploy script exits 0 and prints MD5s for all expected files.
- **Files affected:** None (copies to Steam local-mods folder).
- **Dependencies:** T1, T2, T3.
- **Commit suggestion:** No repo commit (deployment is a local file copy).
- **Estimated diff lines:** 0.

### T5 — In-game playtest, log extraction, and banner verification

- **Description:** Launch AoM:R with only **Human Assist Improvements** enabled, start a match as a human player, pick up relics with a hero, and verify: (a) tick-start banner fires, (b) disappearance banner fires only for relics that left the world, (c) candidate-hero banner lists the nearby heroes, (d) successful delivery banner names the chosen temple, (e) busy / plan-busy heroes do NOT trigger a delivery (player commands take precedence; the relic stays with the hero and is not retried), (f) AI players stay silent. Run `scripts/extract-ai-logs.sh` afterwards and grep the UTF-8 bundle for `autoRelicDelivery`.
- **Acceptance criteria:** Extracted `playerN.log` contains the expected `aiEcho` banners from the design doc and no repeated delivery for the same `(hero, relic)` pair.
- **Files affected:** Produces `.tmp/human_assist_improvements_YYYYMMDD-HHMMSS_NNN.log/` only.
- **Dependencies:** T4.
- **Commit suggestion:** No repo commit (logs stay in `.tmp/`).
- **Estimated diff lines:** 0.

### T6 — Update `AGENTS.md` if relevant

- **Description:** Read `AGENTS.md` at the repository root. If it mentions the prior auto-relic-delivery handler architecture, update or remove that section. If no file exists (current state), skip this task and record the skip.
- **Acceptance criteria:** `AGENTS.md` is either absent/unchanged or correctly reflects the poll-based architecture.
- **Files affected:** `AGENTS.md` (if it exists).
- **Dependencies:** T1.
- **Commit suggestion:** If changed, include in the code/docs commit.
- **Estimated diff lines:** 0.

### T7 — Archive the change

- **Description:** Move the completed change folder to `openspec/changes/archive/YYYY-MM-DD-auto-relic-delivery-reinvestigation/` using today's date. This is normally done by the `sdd-archive` phase after verification.
- **Acceptance criteria:** The change folder exists under `openspec/changes/archive/` and no longer exists under `openspec/changes/`.
- **Files affected:** `openspec/changes/auto-relic-delivery-reinvestigation/` → `openspec/changes/archive/YYYY-MM-DD-auto-relic-delivery-reinvestigation/`.
- **Dependencies:** T5.
- **Commit suggestion:** `chore(openspec): archive auto-relic-delivery-reinvestigation` (separate commit from the implementation).
- **Estimated diff lines:** ~10 (rename).

## Commit strategy

**Recommended: single commit for the code/docs change, then a separate archive commit.**

- `feat(auto-relic): poll-based ground-relic delivery` — bundles T1, T2, and T3. The implementation and its README explanation are one coherent work unit, and the total diff is comfortably below the 400-line budget.
- `chore(openspec): archive auto-relic-delivery-reinvestigation` — T7 only, after playtest passes.

T4 and T5 do not produce repo commits because they are local deploy/run steps. T6 is a no-op unless `AGENTS.md` exists.

Justification: the new design is a single logical replacement of one file. Splitting it into per-task commits would create tiny, non-functional intermediate states (e.g., a rewritten module with an outdated README). A single commit tells a complete story; the archive move is independent and should stay separate so the implementation PR diff stays focused.

## Risks

- `kbUnitGetPlanID(heroID)` semantics for manually controlled human heroes are unverified; if it returns something other than `cInvalidID`, deliveries may be suppressed.
- `kbUnitGetContainedUnitByIndex(heroID, 0)` may not return the relic immediately on the disappearance tick, requiring the pending-retry list to age out correctly.
- `kbUnitQuerySetMaximumDistance(..., 10.0)` radius is assumed to be in meters; if the engine interprets distance differently, the proximity query may miss carriers.
- If `auto_relic_delivery.xs` is loaded by a different mod that also overlays `human_assist.xs`, the registration path could differ; this change is scoped to `Human Assist Improvements` only.
- README maintenance: removing the singleton-handler caveat is safe only if no other feature in the mod still registers `cXSRelicPickedUpHandler`.

## References

- `openspec/changes/auto-relic-delivery-reinvestigation/exploration.md` — API verification findings.
- `openspec/changes/auto-relic-delivery-reinvestigation/proposal.md` — architecture decisions and decisions log.
- `openspec/changes/auto-relic-delivery-reinvestigation/specs/auto-relic-delivery/spec.md` — delta spec with SHALL statements and scenarios.
- `openspec/changes/auto-relic-delivery-reinvestigation/design.md` — functions, state tables, diagnostic banners, and code-migration map.
- `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` — current deployed implementation being replaced.
- `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` — `autoRelicDelivery_register()` call site.
- `mod/human_assist_improvements/README.md` — mod-level documentation to update.
- `scripts/deploy-mods.sh` — deployment script.
- `scripts/extract-ai-logs.sh` — AI log extraction script.
- `openspec/changes/archive/2026-06-18-auto-relic-delivery/tasks.md` — prior task breakdown (structural reference only).
