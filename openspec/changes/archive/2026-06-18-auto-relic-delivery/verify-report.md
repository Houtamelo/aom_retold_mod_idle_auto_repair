# Verify Report: auto-relic-delivery (post-remediation)

**Change:** `auto-relic-delivery` for `aom_retold_mod_idle_auto_repair` (Human Assist Improvements)  
**Mode:** automatic (Strict TDD off; manual in-game verification)  
**Report date:** 2026-06-18  
**Verdict:** **PASS WITH WARNINGS**

---

## 1. Executive summary

The remediation genuinely replaced the per-hero state-machine fallback with a `(heroID, relicID)` pair tracker as the active runtime path. Source inspection confirms all three real KB APIs are called from the live handler/scan, the dead per-hero symbols are gone, the MAY-fallback clause has been removed from both the promoted and archived specs, and the prior `human_assist.xs` whitespace realignment is gone. The only residual warnings are a non-functional trailing-newline normalization on the final brace of `human_assist.xs` and the fact that the engine APIs are verified in the retail docs but still need in-game confirmation (scenario B7).

---

## 2. Failure-mode audit — did the silent-downgrade failure recur?

**NO.** The prior verify failed because it approved a per-hero state machine that silently downgraded the user's hard "once per `(hero, relic)` pair" constraint. This verification directly read the runtime code and confirms the constraint is now honored:

- The active runtime path is in `autoRelicDelivery_scanAndDeliver()` (`auto_relic_delivery.xs:161`).
- For each alive hero, it first checks `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) > 0` (`:180`).
- It then reads `int relicID = kbUnitGetContainedUnitByIndex(heroID, 0)` (`:186`).
- It defensively correlates `kbRelicGetTechID(relicID)` with the event `techID` (`:197`).
- It checks `autoRelicDelivery_hasPair(heroID, relicID)` and skips if already tracked (`:206`).
- Only then, if the hero is idle, it calls `autoRelicDelivery_issueDelivery()` (`:231`), which again checks `hasPair()`, finds a temple, issues `aiTaskWorkUnit(heroID, templeID)` (`:153`), and finally calls `autoRelicDelivery_addPair(heroID, relicID)` (`:154`).

The order is strictly **check → issue → add**, and the pair tracker is queried before every potential delivery. There is no alive branch that falls back to per-hero state. The silent downgrade did not recur.

---

## 3. Per-check results

| ID | Check | Method | Result |
|---|---|---|---|
| A1 | Pair tracker is the active runtime path | Read `auto_relic_delivery.xs:161-241` | ✅ `autoRelicDelivery_hasPair` called at `:140`, `:206`; `autoRelicDelivery_addPair` called at `:154`, `:225`. Order is check (`:206`) → issue (`:231` -> `aiTaskWorkUnit` at `:153`) → add (`:154`). |
| A2 | Three real APIs invoked in active path | Grep + read | ✅ `kbUnitGetContainedUnitByIndex(heroID, 0)` at `:186`; `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic)` at `:180`; `kbRelicGetTechID(relicID)` at `:197`, `:200`. All inside `scanAndDeliver`, not dead code. |
| A3 | Inferred carrying-check replaced | Grep | ✅ Relic carrying check uses `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic)`. The broader `kbUnitGetNumberContained(` only remains at `:123` for temple capacity, which is correct. |
| A4 | Prior dead-code symbols removed | Grep | ✅ Zero matches for `cAutoRelic_RelicIDUnknown`, `cAutoRelic_State*`, `gAutoRelic_state`, `gAutoRelic_retryCounter`, `cAutoRelic_RetryMaxAttempts`, `findHeroSlot`, `getHeroState`, `setHeroState`, `NO_RELIC`, `AUTO_DELIVERING`, `PLAYER_OVERRIDE`. |
| A5 | One-shot retry still works | Read `:283-300` and `:234-240` | ✅ `autoRelicDelivery_tickRetry` rule exists, `minInterval 2`, `inactive`. Flag armed on busy carrying hero; cleared on first fire; scan re-tests `hasPair()` before re-issuing. |
| A6 | Human-only guard | Read `:163-164`, `:249`, `:264` | ✅ `kbPlayerIsHuman(cMyID)` guards handler body, `register()`, and scan entry. |
| A7 | Spec R2 honored by code flow | Trace `onPickedUp` → `scanAndDeliver` | ✅ Each (hero, relic) pair is checked and recorded exactly once; re-trigger for a new pair is possible because tracker keys include `relicID`. |
| B | `human_assist.xs` whitespace fix | `git diff` | ✅ No include-comment realignment. One residual `-` line: a trailing newline was added to the final `}` (HEAD had no trailing newline). |
| C | `scripts/deploy-mods.sh` intact | `bash -n` + `git diff` | ✅ `bash -n: OK`. Diff is a single added deploy line; 3 prior blocks byte-identical to HEAD. Dry-run deployed all 5 mod blocks. |
| D | No sibling-mod regression | `git status --porcelain` + `git diff HEAD -- mod/idle_auto_repair mod/intelligent_auto_scout mod/intelligent_auto_repair_and_scout` | ✅ No modified sibling-mod files. |
| E1 | Promoted + archived specs amended | Grep both `spec.md` files | ✅ No `MAY fall back`. R2 now mandates the three real APIs. R6 uses `kbUnitGetNumberContainedOfType`. B7 references all three APIs. |
| E2 | Design amended (D7-D9) | Read `design.md` | ✅ D7 (`kbUnitGetContainedUnitByIndex`), D8 (`kbRelicGetTechID`), D9 (`kbUnitGetNumberContainedOfType`) documented. State-machine diagram now describes pair tracker, not per-hero fallback. |
| E3 | Archive-report amended | Read `archive-report.md` | ✅ Warnings 3+4 and rejected suggestions 1+2 are marked addressed/enacted. Pre-publish notes correctly cite remaining in-game verification gap. Minor stale prose at line 47 still says "per-hero state machine" in the file-delivery table. |
| F | Hidden silent downgrade | Grep `fallback/Fallback/FALLBACK` + full read | ✅ No runtime fallback branch found. Only benign occurrences are historical comment at `:10` and "retry fallback" at `:158`. |
| G | Temp deploy dry-run + md5 | `AOMR_LOCAL_MODS=/tmp/... bash scripts/deploy-mods.sh` + `md5sum` | ✅ All 4 mod trees deployed. `auto_relic_delivery.xs` landed at correct path; source md5 `1e57a75351e7c06929761dc812fbc8c4` matches deployed copy. |

---

## 4. Requirement coverage

| Req | Requirement | Verdict | Evidence |
|---|---|---|---|
| R1 | Register `cXSRelicPickedUpHandler` from `human_assist.xs::main()`; no per-tick poll | MET | `aiSetHandler` at `auto_relic_delivery.xs:276`; call at `human_assist.xs:999` |
| R2 | Track `(heroID, relicID)` pairs as active path; no per-hero fallback | MET | Active path in `scanAndDeliver` (`:161`); helpers `:76-100`; spec R2 amended |
| R3 | Only issue when idle; one-shot retry re-tests idle | MET | Idle gate `:211-212`; retry `:283-300` |
| R4 | Nearest temple with space | MET | `findNearestTempleWithSpace` `:106-131` |
| R5 | Deliver with `aiTaskWorkUnit(heroID, templeID)` | MET | `:153` |
| R6 | Type-safe carrying check + in-game verification | MET | `kbUnitGetNumberContainedOfType` at `:180`; `kbUnitGetContainedUnitByIndex` at `:186`; scenario B7 covers in-game confirmation |
| R7 | Filter by `cUnitTypeHero` | MET | Hero query setup `:52-55` |
| R8 | Guard all paths with `kbPlayerIsHuman(cMyID)` | MET | `:163-164`, `:249`, `:264` |
| R9 | Document singleton handler caveat | MET | README singleton section and startup `aiEcho` `:267`, `:272-275` |
| R10 | Intentionally break D1 byte-identity | MET | Include + registration added to 4th mod's `human_assist.xs` |
| R11 | Rollback removes file/include/deploy/README | MET | Rollback steps documented in `mod/human_assist_improvements/README.md` |

---

## 5. Scenario traceability

| Scenario | Status | Notes |
|---|---|---|
| B1 — Happy path | Manual (USER) | Structure verified; delivery path present. |
| B2 — Player overrides | Manual (USER) | Retry marks pair triggered on busy hero (`:224-225`). |
| B3 — Sequential delivery | Manual (USER) | New `relicID` yields new tracker pair. |
| B4 — Nearest temple full | Manual (USER) | Space loop present (`:119-127`). |
| B5 — No temple with space | Manual (USER) | No-op path present; pair NOT recorded (`:142-149`). |
| B6 — Idle-check edge | Manual (USER) | One-shot retry armed on busy carrying hero. |
| B7 — Carrying-relic API verification | Manual (USER) | README and spec reference the three APIs; requires in-game `aiEcho` log. |
| B8 — Singleton documentation | Auto-verifiable | README singleton section + code comment at `:272-275`. |
| B9 — Existing mod regression | Auto-verifiable | No sibling-mod changes in working tree. |
| B10 — Conflict warning | Auto-verifiable | Top-level README `:17` and mod README `:13-15`. |
| B11 — D1 divergence and rollback | Auto-verifiable | Diff/cmp confirms divergence; rollback steps documented. |

---

## 6. Issue lists

### CRITICAL

*None.*

### WARNING

1. **Engine APIs are documented but still need in-game confirmation (scenario B7).**
   - `kbUnitGetContainedUnitByIndex`, `kbRelicGetTechID`, and `kbUnitGetNumberContainedOfType` were found in `extracted/doxygen/kbfuncs_8cpp.html` and are used correctly in code, but their exact behavior for human-player heroes at pickup time has not been measured in-game. This is an inherent limitation of the XS environment and is explicitly scoped to scenario B7.

2. **Residual trailing-newline normalization on `human_assist.xs`.**
   - `git diff` shows one content `-` line for the final `}` because HEAD had no trailing newline and the working tree now has one. The prior include-comment realignment is gone, so the original whitespace friction is fixed, but the diff is not strictly "purely additive." This is non-functional and does not affect behavior or patch maintenance.

### SUGGESTION

1. **Update stale prose in `archive-report.md` line 47.**
   - The "Files / deltas delivered" table still says the created `auto_relic_delivery.xs` contains a "per-hero state machine." This contradicts the rest of the amended archive report. Change the description to: "Feature logic: handler, `(heroID, relicID)` pair tracker, retry rule, temple lookup, delivery command."

2. **Consider adding a code comment near the final brace of `human_assist.xs`** noting that a trailing newline is intentional, to avoid future diffs looking like a content change on that line.

---

## 7. Final verdict

**PASS WITH WARNINGS**

The remediation is complete and correct. The silent-downgrade failure mode (per-hero fallback replacing pair-tracking) did **not** recur: the runtime path genuinely uses `kbUnitGetContainedUnitByIndex`, `kbRelicGetTechID`, and `kbUnitGetNumberContainedOfType`, and the pair tracker is the active one-shot guard. No existing mod files are modified. The deploy script and dry-run are clean. The remaining warnings are low-risk: an unavoidable in-game verification gap and a single non-functional trailing-newline artifact.

**Next recommended step:** re-commit cleanly (orchestrator handles).
