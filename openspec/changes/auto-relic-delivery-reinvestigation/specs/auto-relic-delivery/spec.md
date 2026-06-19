# Delta Spec for auto-relic-delivery

- **Change id:** `auto-relic-delivery-reinvestigation`
- **Capability:** `auto-relic-delivery`
- **Status:** draft
- **Date:** 2026-06-19
- **Prior delta:** `openspec/changes/archive/2026-06-18-auto-relic-delivery/specs/auto-relic-delivery/spec.md`

## Why this delta

`cXSRelicPickedUpHandler` does not fire for human players; vanilla registers it only for civ-AI. The prior delta scanned idle heroes carrying any relic, so it could not match a disappearance to a specific relic. This delta uses a 2-second ground-relic scan and delivers only the hero within 10 meters that now carries the exact relic ID while idle and plan-free. No cross-tick state is retained beyond the diff snapshot.

## MODIFIED Requirements

### Requirement: R1 Trigger mechanism

SHALL enable a `rule ... minInterval 2` named `autoRelicDelivery_scanRelics` for ground-relic scans and SHALL NOT register `cXSRelicPickedUpHandler`.
(Previously: required registering the handler and prohibited polling heroes each tick.)

#### Scenario: Poll detects a pickup

- GIVEN an idle hero near a ground relic
- WHEN the relic unit ID disappears between two 2-second ticks
- THEN `aiEcho` logs the disappeared relic unit ID

#### Scenario: No handler registered

- GIVEN `autoRelicDelivery_register()` runs for a human player
- WHEN the body executes
- THEN `aiSetHandler(..., cXSRelicPickedUpHandler)` is absent and the scan rule is active

### Requirement: R3 Delivery guard

SHALL issue a delivery order only when `kbUnitGetActionType(heroID) == cActionTypeIdle` and `kbUnitGetPlanID(heroID) == -1`.
(Previously: only required action-type idle and used a separate one-shot retry rule; retry rule is now removed.)

#### Scenario: Idle and plan-free hero delivers

- GIVEN a hero carries the disappeared relic within 10 meters
- WHEN both checks pass
- THEN the hero is tasked once to the nearest non-full temple

#### Scenario: Player override suppresses delivery

- GIVEN a hero has just picked up a relic
- WHEN the player issues a manual order before the next tick
- THEN no delivery order is issued on this or any subsequent tick; the relic stays with the hero

### Requirement: R6 Relic identification

SHALL confirm carrying with `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) > 0`, retrieve `kbUnitGetContainedUnitByIndex(heroID, 0)`, and match it to the disappeared relic ID. It MAY scan slots up to `kbUnitGetNumberContained(heroID)` when slot 0 does not match.
(Previously: only required the carrying signal and verification that the API returned a valid relic unit ID.)

#### Scenario: Specific relic match delivers

- GIVEN relic R disappeared and hero H is within 10 meters
- WHEN `kbUnitGetContainedUnitByIndex(H, 0)` returns R
- THEN delivery is issued once

#### Scenario: Different relic skips delivery

- GIVEN relic R disappeared and hero H carries relic S
- WHEN the contained unit ID is S ≠ R
- THEN no delivery order is issued for R

## REMOVED Requirements

### Requirement: R1 Event-handler registration clause

(Reason: `cXSRelicPickedUpHandler` does not fire for human players.)

### Requirement: R3 One-shot retry rule

(Reason: the 2-second scan loop is the canonical cadence; no separate retry rule.)

### Requirement: R3b Pending-disappearance retry

(Reason: stale carries and player overrides are not retried; the three exhaustive cases for a missing relic are handled by the main scan alone.)

### Requirement: R6 (prior) (heroID, relicID) pair tracker

(Reason: the disappearance-detection algorithm cannot produce a second disappearance for the same relic, so the pair tracker prevents a scenario that cannot exist.)

### Requirement: R9 Singleton handler caveat

(Reason: handler no longer registered.)

## ADDED Requirements

### Requirement: R12 Ground-relic disappearance scan

SHALL snapshot alive ground relic IDs and detect disappearances by comparing consecutive tick results.

#### Scenario: Disappearance banner

- GIVEN three relics exist on the ground
- WHEN one unit ID is present in the prior tick but absent in the current tick
- THEN `aiEcho` logs the disappeared unit ID and the remaining relics do not produce extra banners

### Requirement: R13 Hero proximity query

SHALL query alive player heroes within 10 meters of a disappeared relic's last position using `kbUnitQuerySetMaximumDistance(..., 10.0)`.

#### Scenario: Proximity radius filters heroes

- GIVEN a disappeared relic's last position
- WHEN one hero is 8 meters away and another is 15 meters away
- THEN the 8-meter hero appears in results and the 15-meter hero does not

## Cross-references

- Proposal: `openspec/changes/auto-relic-delivery-reinvestigation/proposal.md`
- Exploration: `openspec/changes/auto-relic-delivery-reinvestigation/exploration.md`
- Main spec: `openspec/specs/auto-relic-delivery/spec.md`
- Prior delta: `openspec/changes/archive/2026-06-18-auto-relic-delivery/specs/auto-relic-delivery/spec.md`
- Deployed file: `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs`
