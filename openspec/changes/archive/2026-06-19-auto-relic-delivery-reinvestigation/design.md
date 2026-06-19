# Design: Auto-relic-delivery re-investigation (poll-based trigger)

| Field | Value |
|---|---|
| Change id | `auto-relic-delivery-reinvestigation` |
| Phase | `sdd-design` |
| Status | draft |
| Date | 2026-06-19 |
| Capability | `auto-relic-delivery` |

## 1. Architecture overview

Replace the event-driven `cXSRelicPickedUpHandler` path with a **2-second ground-relic poll**: every tick we snapshot all alive relics on the ground, compare the unit IDs with the previous snapshot, and treat each missing ID as a probable pickup. For each disappeared relic we query player-owned heroes within a 10-meter radius of its last-seen position, then deliver only if one of those heroes is carrying the exact relic unit ID, is idle, and has no active AI plan.

**No state is retained across ticks except the diff snapshot.** Each tick is independent. Stale carries and player overrides are not retried. This decision was made after the user walked through every case for a "missing relic" path and showed that the three outcomes (other-player pickup, our-pickup-but-busy, our-pickup-and-idle) are handled by the main scan alone — there is no scenario that requires a pending-retry window or a pair tracker.

## 2. Data flow

```text
rule autoRelicDelivery_scanRelics (minInterval 2)
   │
   ▼
kbUnitQueryExecute(gAutoRelic_relicQuery)
   │
   ▼
currentRelicIDs[] / currentRelicPositions[]
   │
   ▼
diff against gAutoRelic_prevRelicIDs (set difference)
   │
   ├─ none disappeared ──► update previous cache, exit
   │
   ▼
for each disappeared relic R at lastPos:
   │
   ▼
heroes = findHeroesInRange(lastPos, 10.0)
   │
   ▼
for each candidate hero H:
   │  ├─ heroCarriesRelic(H, R) == false ──► continue
   │  ├─ heroIsDeliverable(H) == false ──► continue
   │  └─ all guards pass ──► issueDelivery(H, R); return
   │
   ▼
update gAutoRelic_prevRelicIDs/Positions from current snapshot
```

A disappearance is computed by a set difference on unit IDs between consecutive ticks. The previous tick's positions are used as the search origin because the relic no longer exists as a ground unit at the time the heroes are queried.

## 3. Module structure

`mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs`:

| Function | Responsibility |
|---|---|
| `autoRelicDelivery_setupRelicQuery()` | Creates the persistent `relicsOnGround` query (`cUnitTypeRelic`, `cUnitStateAlive`, player 0 / gaia). |
| `autoRelicDelivery_setupHeroProximityQuery()` | Creates a reusable hero query (`cUnitTypeHero`, `cUnitStateAlive`, `cMyID`); position and radius are reset per call. |
| `autoRelicDelivery_setupTempleQuery()` | Preserved; ascending-distance temple query. |
| `autoRelicDelivery_findHeroesInRange(vector pos)` | Resets the hero query to `pos` + 10 m, executes, and returns matching hero unit IDs as an `int[]`. |
| `autoRelicDelivery_heroCarriesRelic(int heroID, int relicID)` | True if any contained unit of type `cUnitTypeRelic` equals `relicID`. Slot 0 fast path with defensive scan over `kbUnitGetNumberContained(heroID)` slots. |
| `autoRelicDelivery_heroIsDeliverable(int heroID)` | True when `kbUnitGetActionType(heroID) == cActionTypeIdle` **and** `kbUnitGetPlanID(heroID) == -1`. |
| `autoRelicDelivery_findNearestTempleWithSpace(int heroID)` | Nearest player-owned temple with `kbUnitGetNumberContained < cProtoStatMaxContained`, or -1. Emits `no temple with space -> skip` if none qualify. |
| `autoRelicDelivery_handleDisappearance(int relicID, vector lastPos)` | For one disappeared relic: find nearby heroes, deliver to the first valid carrier. |
| `autoRelicDelivery_register()` | Enables `autoRelicDelivery_scanRelics` only; preserves `xsSetContextPlayer(cMyID)` / `xsSetContextPlayer(-1)` defensive wrapper and the human-player guard. |
| `autoRelicDelivery_scanRelics` | New 2-second rule: snapshot ground relics, diff against previous snapshot, handle each disappearance, update snapshot. |

## 4. State management

| Name | Type | Purpose |
|---|---|---|
| `cAutoRelic_ProximityRadius` | `const float = 10.0` | Hero proximity radius in meters. Used by `findHeroesInRange`. |
| `gAutoRelic_heroQuery` | `int` | Reusable player-owned hero proximity query. |
| `gAutoRelic_templeQuery` | `int` | Player-owned temple query (preserved). |
| `gAutoRelic_relicQuery` | `int` | Alive ground relics (player 0 / gaia). |
| `gAutoRelic_prevRelicIDs` | `int[]` | Previous tick's ground relic unit IDs. |
| `gAutoRelic_prevRelicPositions` | `vector[]` | Parallel last-known positions for the previous tick's relics. |

**No other state.** There is no `(heroID, relicID)` pair tracker, no pending-disappearance list, no retry counter. Each tick is independent. Globals reset on game load because the script is reloaded.

## 5. Edge cases and behavior decisions

### The three exhaustive cases for a disappeared relic

1. **Picked up by another player.** The hero query filters by `cMyID`; the carrier is not a candidate. No action. Correct — we cannot deliver a relic that another player holds.
2. **Picked up by our hero but the hero is busy.** The idle guard (or no-plan guard) rejects; no delivery. Correct — player commands take precedence.
3. **Picked up by our hero and the hero is idle.** Deliver to the nearest temple with space.

These three cover every "relic disappeared" scenario. No retry logic is needed because:
- If our hero is busy, the player intends something else; do not interrupt. If the player later wants delivery, they can issue a deposit order.
- If another player picked it up, we cannot deliver.
- If our hero is idle, we deliver.

### New ground relics appearing

A new relic entering the world (temple destroyed, hero died and dropped a relic) becomes a new entry in `gAutoRelic_prevRelicIDs` on the next tick. We do not act on new entries. If the relic later disappears, the missing-relic path handles it normally.

### Multiple heroes within 10 m

Candidate heroes are checked in query order. The first hero that is carrying the target relic and passes both idle and no-plan guards wins; `handleDisappearance` returns after issuing the delivery. Subsequent candidates are ignored for that relic.

### Hero carries a different relic

`heroCarriesRelic` filters by `cUnitTypeRelic` and compares unit IDs. If the carried ID differs from the disappeared relic ID, the hero is skipped.

### No temple with space

`findNearestTempleWithSpace` returns -1 and emits a `no temple with space -> skip` banner; `handleDisappearance` returns without issuing any delivery. The relic stays with the hero; the player can deliver manually or wait for a temple with space.

### Garrisoned / transforming / dead heroes

`cUnitStateAlive` excludes dead heroes from the proximity query. Garrisoned heroes typically fail the idle guard. `heroIsDeliverable` adds the no-plan guard for further safety.

### Two relics disappear in the same tick

Each disappearance is processed independently inside the same rule fire.

### Manual player pickup and walk-back

If the player manually picks up a relic with an idle/plan-free hero, the next 2-second tick detects the disappearance, finds the carrying hero, and issues `aiTaskWorkUnit`. **This is the accepted cost-benefit tradeoff:** an idle hero after a manual pickup looks identical to an idle hero waiting for orders. The verifier must treat this as expected behavior, not a bug.

### Same relic reappears and disappears again later

If a relic reappears on the ground (temple destroyed, manual drop) and then disappears again at a later tick, that is a fresh disappearance event. The missing-relic path handles it independently. There is no global "I already saw this relic" memory — and none is needed, because each disappearance is a fresh real-world event.

## 6. Diagnostic banners

| Banner | Condition |
|---|---|
| `autoRelicDelivery: register() called cMyID=X kbPlayerIsHuman=Y` | Once per registration. |
| `autoRelicDelivery: skipping registration (AI player context)` | Once per registration if the active player is AI. |
| `autoRelicDelivery: enabling scan rule` | Once per registration, just before `xsEnableRule`. |
| `autoRelicDelivery: tick start (relicsOnGround=N)` | Every 2-second tick. |
| `autoRelicDelivery: disappearance detected (relicID=X, pos=(x,z))` | Per disappeared relic. |
| `autoRelicDelivery: candidate heroes within 10m: N` | Per disappearance, immediately after the proximity query. |
| `autoRelicDelivery: hero H carrying target relic -> delivering to temple T` | Successful delivery. |
| `autoRelicDelivery: no temple with space -> skip` | Temple-capacity guard rejected. |

## 7. Code-migration map

| Old artifact | New artifact | Notes |
|---|---|---|
| `autoRelicDelivery_register()` registers handler | enables `autoRelicDelivery_scanRelics` only | `aiSetHandler(..., cXSRelicPickedUpHandler)` removed. |
| `autoRelicDelivery_onPickedUp` | removed | Event path deleted; never fired for human players. |
| `autoRelicDelivery_pollFallback` (5 s) | `autoRelicDelivery_scanRelics` (2 s) | Renamed, simplified, made canonical. |
| `autoRelicDelivery_tickRetry` | removed | The 2 s scan loop is the canonical cadence; no separate retry rule. |
| `autoRelicDelivery_scanAndDeliver` | replaced by `handleDisappearance` + helpers | Single-purpose per-disappearance logic. |
| `autoRelicDelivery_computeDisappearances` | inlined into `scanRelics` | Diff is short enough to inline; removes a 30-line helper. |
| `autoRelicDelivery_findCarrierInRange` | renamed `findHeroesInRange` | Same semantics; clearer name. |
| `autoRelicDelivery_hasPair`, `autoRelicDelivery_addPair` | removed | Pair tracker no longer exists. |
| `gAutoRelic_heroID[]`, `gAutoRelic_relicID[]` | removed | Pair tracker globals no longer exist. |
| `gAutoRelic_pendingRelicIDs[]`, `gAutoRelic_pendingRelicPositions[]`, `gAutoRelic_pendingAge[]`, `gAutoRelic_pendingHeroIDs[]` | removed | Pending-retry state no longer exists. |
| `gAutoRelic_retryPending` | removed | Global one-shot retry flag no longer needed. |

## 8. Performance notes

Per tick cost:

- One ground-relic query: `O(R)` where `R` = alive relics in the world.
- Diff against previous snapshot: `O(R²)` worst case (linear scan per prev relic). For typical `R ≤ 10`, this is negligible (≤ 100 comparisons per tick).
- Per disappearance: one hero proximity query (`O(H_r)` where `H_r` = heroes within 10 m, expected ≤ 5).
- Per successful delivery: one temple query (`O(T)` where `T` = player temples, expected ≤ 30).

Worst-case complexity is `O(R² + D · H_r + S · T)`. For typical matches, this is far below the 2-second tick budget. Maps with many scenario-scripted relics could raise `R`, but the cost is still linear in the number of relics present at the time of the query.

## 9. Risks and tradeoffs

- **No retry for busy carriers.** If our hero picks up a relic while busy (pickup animation, mid-order), the feature will not deliver even after the hero becomes idle later. The player must manually deliver or trigger a new disappearance. This is the accepted cost of the simpler design.
- **10 m radius may miss distant pickups.** A hero could pick up a relic from just outside 10 m or move away before the tick fires. Accepted cost of not scanning all heroes globally.
- **2-second latency.** Delivery can occur up to 2 seconds after pickup. The user accepted this cadence.
- **Manual override ambiguity.** An idle hero after a manual pickup looks identical to an idle hero after the pickup animation. The feature will deliver in both cases; this is the intended tradeoff.
- **Heavy relic maps.** Scenario-heavy maps could raise `R`; if logs show tick spikes, consider tuning.
- **`kbUnitGetPlanID` semantics.** The no-plan guard compares against `-1`, the value `kbUnitGetPlanID` returns when the unit has no plan (this is the convention used throughout the shipped AoM:R AI source). The reference doc `docs/MythRMConstants.txt:16` declares `cInvalidID = -1`, but the constant is not exposed to the XS runtime; use `-1` literally. This needs in-game verification to confirm that a manually controlled human hero does not get assigned a plan ID while appearing idle.

## 10. References

- `openspec/changes/auto-relic-delivery-reinvestigation/exploration.md` — API verification findings.
- `openspec/changes/auto-relic-delivery-reinvestigation/proposal.md` — architecture decisions.
- `openspec/changes/auto-relic-delivery-reinvestigation/specs/auto-relic-delivery/spec.md` — delta spec with SHALL statements.
- `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` — implemented file.
- `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs:996-1009` — registration call site and context-player restoration pattern.
- `openspec/specs/auto-relic-delivery/spec.md` — canonical capability spec.
- `extracted/doxygen/kbfuncs_8cpp.html:229` — `kbUnitGetContainer`.
- `extracted/doxygen/kbfuncs_8cpp.html:235` — `kbUnitGetNumberContainedOfType`.
- `extracted/doxygen/kbfuncs_8cpp.html:237` — `kbUnitGetContainedUnitByIndex`.
- `extracted/doxygen/kbfuncs_8cpp.html:189` — `kbUnitGetPlanID`.
- `extracted/doxygen/kbfuncs_8cpp.html:926` — `kbRelicGetTechID`.