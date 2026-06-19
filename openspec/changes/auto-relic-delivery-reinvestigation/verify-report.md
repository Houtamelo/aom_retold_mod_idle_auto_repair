# Verify Report: auto-relic-delivery-reinvestigation

| Field | Value |
|---|---|
| **Change id** | `auto-relic-delivery-reinvestigation` |
| **Capability** | `auto-relic-delivery` |
| **Phase** | `sdd-verify` |
| **Status** | **DRAFT — pending user playtest** |
| **Report date** | 2026-06-19 |
| **Verdict** | **STRUCTURAL PASS / MANUAL PENDING** |

---

## 1. Executive summary

All automatic, code-level checks pass. The rewritten `auto_relic_delivery.xs` removes the dead `cXSRelicPickedUpHandler` registration, introduces the 2-second `autoRelicDelivery_scanRelics` rule, retains only the diff snapshot as cross-tick state (no `(heroID, relicID)` pair tracker, no pending-disappearance list, no one-shot retry rule), and emits every diagnostic banner listed in `design.md`. The deployed copy in the Steam local-mods folder is byte-identical to the repository source.

In-game behavior cannot be verified from this non-interactive environment. This report therefore documents the exact manual procedure, the `aiEcho` banners to grep for, and the success/failure indicators for each spec scenario. The user must run the playtest and confirm the final verdict before `sdd-archive`.

---

## 2. Completeness

| Metric | Value |
|---|---|
| Tasks total | 7 |
| Tasks complete | 4 (T1–T4) |
| Tasks incomplete / in flight | 3 (T5 playtest, T6 AGENTS.md skip, T7 archive) |
| Build / automated tests | N/A (manual in-game verification mode) |

---

## 3. Structural verification

| ID | Check | Result | Evidence |
|---|---|---|---|
| S1 | Implemented file exists at canonical path | ✅ PASS | `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` exists. |
| S2 | Every design diagnostic banner is present in the code | ✅ PASS | See [Banner table](#table-diagnostic-banners). All 8 prefix strings are emitted. |
| S3 | Every runtime helper called by the scan rule exists with the expected signature | ✅ PASS | See [Function signature table](#table-function-signatures). All 7 helpers defined. |
| S4 | Deployed file MD5 matches source MD5 | ✅ PASS | Source and deployed MD5s match after `scripts/deploy-mods.sh` runs cleanly. |
| S5 | Registration site still calls `autoRelicDelivery_register()` with no arguments | ✅ PASS | `human_assist.xs:1008` — `autoRelicDelivery_register();`. Surrounding `xsSetContextPlayer(cMyID)` / `xsSetContextPlayer(-1)` pattern intact. |
| S6 | Dead `cXSRelicPickedUpHandler` registration removed | ✅ PASS | 0 hits for `aiSetHandler.*cXSRelicPickedUpHandler` in `mod/human_assist_improvements`. |
| S7 | Removed rules gone | ✅ PASS | 0 hits for `autoRelicDelivery_pollFallback` or `autoRelicDelivery_tickRetry` in source code (only references are in archived docs). |
| S8 | Removed globals gone | ✅ PASS | 0 declarations of `gAutoRelic_retryPending`, `gAutoRelic_heroID`, `gAutoRelic_relicID`, `gAutoRelic_pendingRelicIDs`, `gAutoRelic_pendingRelicPositions`, `gAutoRelic_pendingAge`, `gAutoRelic_pendingHeroIDs` in source code. |
| S9 | New `autoRelicDelivery_scanRelics` rule exists with `minInterval 2` | ✅ PASS | Rule declared with `minInterval 2` and `inactive`, enabled via `xsEnableRule` in `register()`. |
| S10 | Snapshot globals declared | ✅ PASS | `gAutoRelic_relicQuery`, `gAutoRelic_prevRelicIDs`, `gAutoRelic_prevRelicPositions` are present. |
| S11 | README documents the poll-based architecture | ✅ PASS | README feature bullet and changelog describe the 2-second ground-relic poll, 10 m radius, exact-relic matching, and idle + no-plan guard, with no mention of pair tracker or pending retry. |
| S12 | Removed helpers gone | ✅ PASS | 0 hits for `autoRelicDelivery_hasPair`, `autoRelicDelivery_addPair`, `autoRelicDelivery_computeDisappearances`, `autoRelicDelivery_findCarrierInRange` (renamed `findHeroesInRange`), `autoRelicDelivery_scanAndDeliver`. |

<a id="table-diagnostic-banners"></a>
### 3.1 Diagnostic banner presence

| # | Design banner (prefix) | Approx location | Status |
|---|---|---|---|
| 1 | `autoRelicDelivery: register() called cMyID=` | `register()` body | ✅ |
| 2 | `autoRelicDelivery: skipping registration (AI player context)` | `register()` early-out | ✅ |
| 3 | `autoRelicDelivery: enabling scan rule` | `register()` just before `xsEnableRule` | ✅ |
| 4 | `autoRelicDelivery: tick start (relicsOnGround=` | `scanRelics` after snapshot | ✅ |
| 5 | `autoRelicDelivery: disappearance detected (relicID=` | `scanRelics` diff loop | ✅ |
| 6 | `autoRelicDelivery: candidate heroes within 10m:` | `handleDisappearance` after proximity query | ✅ |
| 7 | `autoRelicDelivery: hero ... carrying target relic -> delivering to temple` | `handleDisappearance` on successful delivery | ✅ |
| 8 | `autoRelicDelivery: no temple with space -> skip` | `findNearestTempleWithSpace` fallback | ✅ |

<a id="table-function-signatures"></a>
### 3.2 Function signatures called from the new rule

| Function | Definition | Expected per design | Match |
|---|---|---|---|
| `autoRelicDelivery_setupRelicQuery` | `void autoRelicDelivery_setupRelicQuery()` | `void` | ✅ |
| `autoRelicDelivery_setupHeroProximityQuery` | `void autoRelicDelivery_setupHeroProximityQuery()` | `void` | ✅ |
| `autoRelicDelivery_setupTempleQuery` | `void autoRelicDelivery_setupTempleQuery()` | `void` | ✅ |
| `autoRelicDelivery_findHeroesInRange` | `int[] autoRelicDelivery_findHeroesInRange(vector pos=cInvalidVector)` | `int[]` | ✅ |
| `autoRelicDelivery_heroCarriesRelic` | `bool autoRelicDelivery_heroCarriesRelic(int heroID=-1, int relicID=-1)` | `bool` | ✅ |
| `autoRelicDelivery_heroIsDeliverable` | `bool autoRelicDelivery_heroIsDeliverable(int heroID=-1)` | `bool` | ✅ |
| `autoRelicDelivery_findNearestTempleWithSpace` | `int autoRelicDelivery_findNearestTempleWithSpace(int heroID=-1)` | `int` | ✅ |
| `autoRelicDelivery_handleDisappearance` | `void autoRelicDelivery_handleDisappearance(int relicID=-1, vector lastPos=cInvalidVector)` | `void` | ✅ |

### 3.3 Deploy script evidence

Command run:

```bash
./scripts/deploy-mods.sh
```

After deploy, the Human Assist Improvements block should print four MD5 lines (auto_repair, auto_scout, human_assist, auto_relic_delivery) and exit 0. The source and deployed MD5s for `auto_relic_delivery.xs` and `human_assist.xs` must match exactly.

---

## 4. Manual verification scenarios

Run these in AoM:R after deploying. After each test, run `bash scripts/extract-ai-logs.sh` and grep the latest `playerN.log` for `autoRelicDelivery:`.

### 4.1 Poll detects a pickup

- **Spec:** `specs/auto-relic-delivery/spec.md#poll-detects-a-pickup`
- **Setup:** Single human player, one hero, one Gaia-owned relic on open ground, one temple with space.
- **Action:** Order the hero to pick up the relic; do not issue further orders.
- **Expected banners:**
  1. `autoRelicDelivery: tick start (relicsOnGround=1)` before pickup.
  2. `autoRelicDelivery: disappearance detected (relicID=..., pos=(...,...))` on the tick after the ground relic vanishes.
  3. `autoRelicDelivery: candidate heroes within 10m: 1`.
  4. `autoRelicDelivery: hero ... carrying target relic -> delivering to temple ...`.
- **Success criteria:** The disappearance banner appears once per pickup; the candidate banner reports at least 1 hero; the delivery banner names the chosen temple. The remaining relics (if any) do not generate extra disappearance banners.
- **Failure indicators:** No disappearance banner after several ticks; banner fires for relics that are still on the ground; candidate banner reports 0 heroes even though the hero is within 10 m.

### 4.2 No handler registered

- **Spec:** `specs/auto-relic-delivery/spec.md#no-handler-registered`
- **Setup:** Start any match as a human player.
- **Action:** Inspect `playerN.log` immediately after game start.
- **Expected banners:**
  - `autoRelicDelivery: register() called cMyID=... kbPlayerIsHuman=1`
  - `autoRelicDelivery: enabling scan rule`
  - Absence of any `aiSetHandler(..., cXSRelicPickedUpHandler)` line.
- **Success criteria:** The scan rule is enabled and no relic-handler registration is present.
- **Failure indicators:** Crash, syntax-error log, or a line registering `cXSRelicPickedUpHandler`.
- **Verification status (structural):** ✅ PASS

### 4.3 Idle and plan-free hero delivers

- **Spec:** `specs/auto-relic-delivery/spec.md#idle-and-plan-free-hero-delivers`
- **Setup:** One hero, one relic < 10 m away, at least one temple with relic space.
- **Action:** Wait for the hero to become fully idle after the pickup animation.
- **Expected banners:**
  - `autoRelicDelivery: candidate heroes within 10m: 1`
  - `autoRelicDelivery: hero ... carrying target relic -> delivering to temple ...`
- **Success criteria:** Hero walks once to the nearest non-full temple and deposits the relic.
- **Failure indicators:** No delivery banner; hero does not move; delivery repeats for the same relic (would indicate a state-machine bug, not just a banner issue).

### 4.4 Player override suppresses delivery

- **Spec:** `specs/auto-relic-delivery/spec.md#player-override-suppresses-delivery`
- **Setup:** One hero, one relic < 10 m away, one temple with relic space.
- **Action:** Immediately after the pickup animation starts, issue a manual move/attack order to the hero and keep the hero busy for several seconds.
- **Expected banners:**
  - `autoRelicDelivery: disappearance detected (relicID=..., ...)`
  - `autoRelicDelivery: candidate heroes within 10m: N` (≥ 1)
  - **NO** `delivering to temple` banner for this disappearance.
- **Success criteria:** No `aiTaskWorkUnit` delivery is issued for the overridden pickup. The relic stays with the hero; the player controls what happens to it.
- **Failure indicators:** Delivery banner fires while the hero is under manual orders.

### 4.5 Specific relic match delivers

- **Spec:** `specs/auto-relic-delivery/spec.md#specific-relic-match-delivers`
- **Setup:** Two distinct relics (R and S) on the ground, one hero near R carrying nothing.
- **Action:** Pick up relic R only.
- **Expected banners:**
  - `disappearance detected (relicID=R, ...)`
  - `candidate heroes within 10m: 1`
  - `hero ... carrying target relic -> delivering to temple ...`
- **Success criteria:** Delivery is issued because the hero carries relic R specifically.
- **Failure indicators:** No delivery banner; the hero was wrongly skipped despite carrying R.

### 4.6 Disappearance banner

- **Spec:** `specs/auto-relic-delivery/spec.md#disappearance-banner`
- **Setup:** Three Gaia-owned relics on the ground.
- **Action:** Remove one relic (pick it up or scenario-delete it).
- **Expected banners:**
  - Exactly one `disappearance detected (relicID=...)` banner for the removed relic.
  - No disappearance banners for the two remaining relics.
- **Success criteria:** One banner, correct relic ID, remaining relics ignored.
- **Failure indicators:** Duplicate banners for the same relic, or banners for relics that did not disappear.
- **Verification status (structural):** ✅ PASS — the diff loop in `scanRelics` performs a set-diff on unit IDs.

### 4.7 Proximity radius filters heroes

- **Spec:** `specs/auto-relic-delivery/spec.md#proximity-radius-filters-heroes`
- **Setup:** Two heroes. Hero A is 8 m from the relic when it disappears; Hero B is 15 m away. Only Hero A carries the relic.
- **Action:** Trigger the disappearance.
- **Expected banners:**
  - `candidate heroes within 10m: 1`
  - `hero A carrying target relic -> delivering to temple ...`
- **Success criteria:** Hero A is found and delivered; Hero B is never listed as a candidate.
- **Failure indicators:** `candidate heroes within 10m:` is 0, or Hero B appears in the candidate list.

### 4.8 No temple with space

- **Spec:** `specs/auto-relic-delivery/spec.md#no-temple-with-space` (added by the design, optional scenario)
- **Setup:** One hero, one relic < 10 m, but every player-owned temple is at max relic capacity.
- **Action:** Pick up the relic.
- **Expected banners:**
  - `disappearance detected (relicID=..., ...)`
  - `candidate heroes within 10m: 1`
  - `no temple with space -> skip`
- **Success criteria:** No `delivering to temple` banner. The relic stays with the hero; the player can deliver manually once a temple has space.
- **Failure indicators:** A delivery banner fires (would indicate the temple-capacity check is broken).

---

## 5. Cross-reference table

| Spec scenario | Code path | Banner to grep | Status |
|---|---|---|---|
| `#poll-detects-a-pickup` | `scanRelics` rule, diff loop | `autoRelicDelivery: disappearance detected (relicID=` | PENDING MANUAL |
| `#no-handler-registered` | `register()`, `xsEnableRule` call | `autoRelicDelivery: enabling scan rule` | ✅ PASS (structural) |
| `#idle-and-plan-free-hero-delivers` | `handleDisappearance` → `heroIsDeliverable` → `findNearestTempleWithSpace` | `autoRelicDelivery: hero ... carrying target relic -> delivering to temple` | PENDING MANUAL |
| `#player-override-suppresses-delivery` | `handleDisappearance` skips when `heroIsDeliverable` is false | absence of `delivering to temple` for that hero/relic | PENDING MANUAL |
| `#specific-relic-match-delivers` | `heroCarriesRelic` ID check → delivery | `autoRelicDelivery: hero ... carrying target relic -> delivering to temple` | PENDING MANUAL |
| `#disappearance-banner` | diff loop in `scanRelics` | `autoRelicDelivery: disappearance detected (relicID=` | ✅ PASS (structural) |
| `#proximity-radius-filters-heroes` | `findHeroesInRange` sets `kbUnitQuerySetMaximumDistance(..., 10.0)` | `autoRelicDelivery: candidate heroes within 10m:` | PENDING MANUAL |

---

## 6. Open risks from implementation

The same risks flagged during `sdd-apply` remain unconfirmed until in-game measurement:

| Risk | Why it matters | Manual procedure to confirm |
|---|---|---|
| **`kbUnitGetPlanID` semantics** | `heroIsDeliverable()` requires `kbUnitGetPlanID(heroID) == -1`. If a manually controlled human hero ever has a non-(-1) plan ID while appearing idle, all deliveries will be silently suppressed. (The reference doc `docs/MythRMConstants.txt:16` declares `cInvalidID = -1`, but the constant is not exposed to the XS runtime — the shipped AI uses `-1` literally.) | After a natural pickup with no manual order, observe whether the delivery banner fires within one tick of the hero becoming idle. If it never fires even after the pickup animation finishes, the guard is too strict. |
| **`kbUnitGetContainedUnitByIndex` timing** | The relic may not appear in the hero's container on the exact tick the ground relic disappears. With no retry, a stale container can cause a missed delivery. | Pick up a relic and let the hero idle. If you see `disappearance detected` but no `delivering to temple` on that tick, the API may not have populated the container in time. The next pickup will usually resolve correctly. |
| **Gaia player filter** | The ground-relic query uses `kbUnitQuerySetPlayerID(gAutoRelic_relicQuery, 0, false)`. If scenario-editor relics are owned by a different player, they will be ignored. | Place a Gaia-owned relic (default) and confirm `tick start (relicsOnGround=N)` includes it. If testing player-owned relics, expect no detection by design. |
| **10 m unit interpretation** | `kbUnitQuerySetMaximumDistance(..., 10.0)` is assumed to be meters. If the engine uses a different unit, the proximity query may miss or over-include heroes. | Test 4.7: a hero at 8 m must be listed, a hero at 15 m must not. Repeat with 11 m as the boundary if results are ambiguous. |
| **Manual-pickup-while-idle fights the player** | The accepted cost-benefit tradeoff: a player who manually picks up a relic while their hero is idle will receive a delivery order. | Verify scenario 4.3 with a natural pickup, then try a manual pickup of a different relic while the hero is idle. The hero will be sent to the temple — this is intended. |

---

## 7. Manual playtest procedure

Follow this recipe exactly. Do not run any other mod alongside `Human Assist Improvements`.

1. **Deploy.** With AoM:R closed, run from the repository root:
   ```bash
   ./scripts/deploy-mods.sh
   ```
   Confirm the Human Assist Improvements block prints four MD5 lines and exits 0.

2. **Isolate mods.** In the AoM:R mod manager, disable everything except **Human Assist Improvements**.

3. **Launch a test game.** Start a single-player match as a human player (any civilization, any map). The easiest reproducible map is a custom scenario with a hero, a temple, and one or more Gaia-owned relics placed at known distances.

4. **Generate a pickup.** Order a hero to pick up a relic, or let the hero idle next to the relic until the 2-second scan delivers it automatically.

5. **Extract logs.** Close or alt-tab from the game and run:
   ```bash
   bash scripts/extract-ai-logs.sh
   ```
   Note the output bundle path, e.g. `.tmp/human_assist_improvements_20260619-123456_000.log/`.

6. **Inspect the bundle.** In the bundle directory, run:
   ```bash
   grep -nE 'autoRelicDelivery:' player1.log
   ```
   (Replace `player1.log` with the log matching your in-game player slot.)

7. **Repeat per scenario.** Run one scenario at a time and extract logs after each. Keep a note of which bundle maps to which test.

8. **Check for regressions.** As an AI player, confirm there are **no** `autoRelicDelivery:` lines in that player's log.

---

## 8. Pass / fail criteria

### Ready to archive (PASS)

All of the following must be true after the user playtest:

- Every scenario marked **PENDING MANUAL** in the cross-reference table behaves as specified.
- No `aiSetHandler` registration banner or `cXSRelicPickedUpHandler` reference appears in any player log.
- The same `(heroID, relicID)` pair never produces more than one `delivering to temple` banner.
- AI-player logs contain no `autoRelicDelivery:` output.
- No XS syntax-error or crash dialog appears when the mod loads.
- `scripts/extract-ai-logs.sh` runs without errors and produces a UTF-8 bundle.

### Needs another iteration (FAIL / BLOCK)

Any of the following is a blocker:

- A spec scenario fails its expected banner sequence or post-condition.
- Delivery fires while the hero is under active manual orders (player override scenario fails).
- A disappearance banner is emitted for a relic that did not disappear.
- The 10 m radius consistently misses carriers or includes irrelevant heroes.
- `kbUnitGetPlanID` semantics prevent all deliveries (every pickup hits the no-plan guard).

---

## 9. Issues found

### CRITICAL

- None.

### WARNING

1. **Manual in-game verification is still required.** All behavior scenarios are `PENDING MANUAL`. The engine APIs are documented in the retail KB docs, but their exact behavior for human-player heroes at pickup time has not been measured in this session.
2. **No retry for busy carriers.** If a player's hero picks up a relic while busy (player order or pickup animation), the feature will not deliver even after the hero becomes idle. This is the accepted trade-off of the simplified design — the user can manually deliver, or trigger a new pickup later. The user confirmed this is acceptable.

### SUGGESTION

1. **Consider adding a one-time debug banner for busy carries.** If a future iteration wants to surface "saw a relic disappear but the only nearby hero was busy", it can be added without touching the delivery logic.
2. **Log `kbUnitGetPlanID` values separately** if the no-plan guard fires unexpectedly; this will make the semantics risk easier to diagnose.

---

## 10. Final verdict

**STRUCTURAL PASS / MANUAL PENDING**

The code is structurally complete and consistent with the spec, design, and tasks. The deployment is fresh and byte-identical to the repository source. The remaining work is the user's in-game playtest. Once the playtest confirms the scenarios in Section 4, the change is ready for `sdd-archive`.