# Proposal: Auto-relic-delivery re-investigation (poll-based trigger)

## Status

| Field | Value |
|---|---|
| Change id | `auto-relic-delivery-reinvestigation` |
| Phase | `sdd-propose` |
| Status | draft |
| Date | 2026-06-19 |
| Author | SDD propose executor |

## Intent

Replace the current event-driven relic-delivery trigger with a **poll-based ground-state-delta detector**. The engine's `cXSRelicPickedUpHandler` does not fire for human players, so the deployed implementation silently falls back to a 5-second hero poll that delivers for *any* idle hero carrying *any* relic. We will instead scan ground relics every 2 seconds, detect when a known relic disappears, and deliver only the specific hero that was near the relic and is now carrying that exact relic unit ID while idle and plan-free.

## Context and motivation

The previous cycle (`openspec/changes/archive/2026-06-18-auto-relic-delivery/`) assumed the only reliable trigger was `cXSRelicPickedUpHandler` and used `kbUnitGetNumberContained(heroID)` to guess that a hero was carrying a relic. Playtest showed the handler never fires for human players — the vanilla engine registers it only for civ AI players (`core/setup.xs:181`), and the player-side `human_assist.xs::main()` around line 954 deliberately does not register it.

A follow-up exploration (`openspec/changes/auto-relic-delivery-reinvestigation/exploration.md`) proved the engine exposes the missing link:

- `kbUnitGetContainedUnitByIndex(unitID, index)` returns the unit ID inside a carrier (direct hero → relic mapping).
- `kbUnitGetContainer(unitID)` returns the carrier of a unit (inverse mapping).
- `kbUnitGetNumberContainedOfType(unitID, unitTypeID)` gives a type-safe carrying guard.
- `kbRelicGetTechID(unitID)` maps a relic unit ID back to the handler's `techID` payload.

These APIs make the ideal `(heroID, relicID)` pair tracker possible, but the *trigger* itself must be polling, not the event handler.

The deployed file `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` currently wires `aiSetHandler(..., cXSRelicPickedUpHandler)` plus a 5-second `autoRelicDelivery_pollFallback` rule that scans idle heroes carrying any relic. The fallback has a suboptimal false-positive profile because it does not correlate a disappearance with a specific relic.

## Scope

### In scope

- Rewrite `auto_relic_delivery.xs` to remove the `cXSRelicPickedUpHandler` registration and the 5-second hero-poll fallback.
- Implement a 2-second `autoRelicDelivery_scanRelics` rule that tracks all ground relics and reacts to disappearances.
- Preserve the `(heroID, relicID)` pair tracker, shifting its meaning to "pairs delivered by the new poll algorithm."
- Keep the nearest-temple-with-space lookup and `aiTaskWorkUnit(heroID, templeID)` issuance unchanged.
- Verify with in-game `aiEcho` logs and `scripts/extract-ai-logs.sh` UTF-8 conversion.

### Out of scope

- AI players (vanilla behavior is preserved via `kbPlayerIsHuman(cMyID)` guard).
- Non-relic pickup events (e.g., herdables, collectibles).
- Map-editor / scenario-specific triggers.
- A configuration UI or XS variable for the 10 m radius.
- Culture-specific relic carriers beyond `cUnitTypeHero`.

## Capabilities

> This is a **modified capability**: the existing `auto-relic-delivery` spec changes its trigger and idle-guard requirements.

### New capabilities

- None.

### Modified capabilities

- `auto-relic-delivery`: change trigger from `cXSRelicPickedUpHandler` event to 2-second ground-relic poll; replace "idle hero carrying any relic" with "idle/plan-free hero carrying the specific relic that disappeared within 10 m"; remove singleton-handler registration caveat because the handler is no longer registered.

## Approach

The new algorithm is **relic-first, not hero-first**.

```text
Every 2 seconds (rule minInterval 2):
1. Query all alive ground relics (cUnitTypeRelic, cUnitStateAlive).
2. Compare current relic IDs/positions with the previous tick's snapshot.
3. For each relic ID that disappeared:
   a. Query player-owned alive heroes within 10 m of the relic's last-seen position.
   b. For each nearby hero:
      i.   Guard: kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) > 0.
      ii.  Retrieve the carried relic: kbUnitGetContainedUnitByIndex(heroID, 0)
           (defensive loop over kbUnitGetNumberContained slots as fallback).
      iii. If carried relic ID equals the disappeared relic ID:
           - Require kbUnitGetActionType(heroID) == cActionTypeIdle.
           - Require kbUnitGetPlanID(heroID) == cInvalidID (no active AI plan).
           - If both, find nearest non-full temple and aiTaskWorkUnit(heroID, templeID).
           - Add (heroID, relicID) to the tracker.
4. Store the current snapshot for the next tick.
```

### Why this direction

- It is the only trigger that observably works for human players (the event does not fire).
- It correlates a *specific* relic disappearance with a *specific* hero and a *specific* relic unit ID, eliminating the old "any idle hero carrying any relic" false-positive.
- The pair tracker still gives idempotency across ticks, so a single pickup cannot be double-delivered.

### Retry-rule disposition

The separate 2-second one-shot `autoRelicDelivery_tickRetry` rule is **removed**. The 2-second main scan loop naturally retries the same disappeared relic until the hero becomes idle or the player overrides. Keeping a parallel one-shot rule would duplicate the same work and add state-machine complexity without adding coverage.

## Affected areas

| Area | Impact | Description |
|---|---|---|
| `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` | Major rewrite | Remove handler registration; add relic-snapshot state and 2-second scan rule. |
| `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` | Minor | `autoRelicDelivery_register()` call remains, but its body changes; no new include. |
| `mod/human_assist_improvements/README.md` | Minor | Remove singleton-handler caveat; document poll-based behavior. |
| `scripts/deploy-mods.sh` | None | File path unchanged. |

## Decisions log

| # | Decision | Rationale | Alternatives considered |
|---|---|---|---|
| 1 | Poll-based ground-state-delta detection (relic disappearance). | `cXSRelicPickedUpHandler` does not fire for human players. | Event-driven handler only; 5-second hero poll only. |
| 2 | Polling interval fixed at 2 seconds. | Responsive enough for pickup-animation settling; low CPU cost. | 1 s (too frequent), 5 s (too sluggish), adaptive. |
| 3 | Hero proximity radius fixed at 10 meters. | Covers pickup interaction distance and small animation drift; keeps query tight. | Configurable radius; 5 m; 15 m. |
| 4 | Match hero to relic via `kbUnitGetContainedUnitByIndex(heroID, 0)`. | Direct unit-ID correlation prevents false positives. | `kbUnitGetNumberContained` only; `kbUnitGetContainer` inverse walk. |
| 5 | Idle guard = `kbUnitGetActionType == cActionTypeIdle` **and** `kbUnitGetPlanID == cInvalidID`. | Maximizes "do not interrupt the player"; plan check is the conservative signal. | Action-type idle only; query-level idle filter. |
| 6 | Remove `aiSetHandler(..., cXSRelicPickedUpHandler)` registration. | Dead code for human players; simplifies ownership. | Keep for future engine changes. |
| 7 | Retain `(heroID, relicID)` pair tracker with shifted semantics. | Still needed for idempotency across ticks and after manual deposits. | Per-relic boolean; timestamp-based dedupe. |
| 8 | Remove the separate one-shot retry rule; retry happens in the main 2-second loop. | The poll already retries; a parallel rule adds no value. | Keep one-shot rule for explicit event-to-retry mapping. |
| 9 | Temple selection unchanged: nearest player-owned temple with relic space. | Avoids scope creep; existing behavior is proven. | Always nearest regardless of space; build new temple. |

## Open questions

1. What does `kbUnitGetPlanID(heroID)` return for a human hero under manual control? The check uses `cInvalidID` (docs/MythRMConstants.txt:16); an in-game smoke test should confirm.
2. Does `kbUnitGetContainedUnitByIndex(heroID, 0)` return the relic immediately after the ground relic disappears, or do we need the defensive loop over all contained slots? The design includes the loop as a fallback, but smoke testing will tell us whether index 0 is sufficient.
3. Is `kbUnitQuerySetMaximumDistance(..., 10.0)` interpreted as 10 meters in this query context? Expected, but should be logged and verified.

## Risks

| Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|
| 2-second polling × O(relics) becomes costly on huge maps | Low | Low | Relic counts are typically < 20; query cost is low. |
| 10 m radius misses a pickup where the hero was farther from the relic's last position | Low | Medium | Radius is generous; if logs show misses, increase or add `kbUnitGetContainer` inverse check on the previous tick's relic list. |
| False delivery when a relic disappears for a non-pickup reason (e.g., scenario script removes it) | Low | Medium | Filtered by specific relic-ID match and idle+no-plan guard; tracker prevents repeat. |
| Player order interleaving: delivery order issued just after player orders hero elsewhere | Med | Med | One-shot delivery; player retains control on the next order. Idle+plan guard reduces chance. |
| Idle semantics edge cases (garrisoned, dead, transforming) | Med | Low | Query uses `cUnitStateAlive`; add explicit guard checks during verification. |
| Temple fills between scan and `aiTaskWorkUnit` | Low | Low | Hero may walk to full temple and wait; acceptable. |

## Rollback plan

1. Revert `auto_relic_delivery.xs` to the archived version in `openspec/changes/archive/2026-06-18-auto-relic-delivery/`.
2. Keep `human_assist.xs` unchanged — `autoRelicDelivery_register()` call path is preserved.
3. Revert README changes.

## Dependencies

- None beyond the existing mod build/deploy pipeline.

## Success criteria

- [ ] `cXSRelicPickedUpHandler` registration is removed from `auto_relic_delivery.xs`.
- [ ] A 2-second rule scans ground relics and logs `aiEcho` on disappearance detection.
- [ ] Pickup → delivery to nearest non-full temple fires exactly once per `(hero, relic)` pair.
- [ ] Player-issued orders are not overridden (idle + no-plan guard verified).
- [ ] AI players are unaffected.
- [ ] Manual log extraction via `scripts/extract-ai-logs.sh` confirms the above.

## References

- `openspec/changes/auto-relic-delivery-reinvestigation/exploration.md` — API verification findings and open questions.
- `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` — currently deployed event + hero-poll implementation.
- `openspec/changes/archive/2026-06-18-auto-relic-delivery/proposal.md` — prior event-driven proposal (for comparison).
- `openspec/specs/auto-relic-delivery/spec.md` — main spec being modified.
- `extracted/doxygen/kbfuncs_8cpp.html:229` — `kbUnitGetContainer`.
- `extracted/doxygen/kbfuncs_8cpp.html:235` — `kbUnitGetNumberContainedOfType`.
- `extracted/doxygen/kbfuncs_8cpp.html:237` — `kbUnitGetContainedUnitByIndex`.
- `extracted/doxygen/kbfuncs_8cpp.html:189` — `kbUnitGetPlanID`.
- `extracted/doxygen/kbfuncs_8cpp.html:926` — `kbRelicGetTechID`.
- `docs/MythRMConstants.txt:16` — `cInvalidID = -1`.
- `docs/MythRMConstants.txt:768` / `docs/MythTRConstants.txt:747` — `cUnitTypeRelic`.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/setup.xs:181` — AI-only relic handler registration.
