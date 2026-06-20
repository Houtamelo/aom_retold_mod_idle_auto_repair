# Auto-relic-delivery Specification

> Source of truth updated by delta `auto-relic-delivery-reinvestigation` on 2026-06-19.

## Purpose

Passive auto-relic-delivery for the `Human Assist Improvements` mod: when a human player's hero picks up a relic, task it once to the nearest player-owned temple with available space, without overriding manual orders.

## Requirements

### R1 Trigger mechanism

SHALL enable a `rule ... minInterval 2` named `autoRelicDelivery_scanRelics` for ground-relic scans and SHALL NOT register `cXSRelicPickedUpHandler`.

#### Scenario: Poll detects a pickup

- GIVEN an idle hero near a ground relic
- WHEN the relic unit ID disappears between two 2-second ticks
- THEN `aiEcho` logs the disappeared relic unit ID

#### Scenario: No handler registered

- GIVEN `autoRelicDelivery_register()` runs for a human player
- WHEN the body executes
- THEN `aiSetHandler(..., cXSRelicPickedUpHandler)` is absent and the scan rule is active

### R3 Delivery guard

SHALL issue a delivery order only when `kbUnitGetActionType(heroID) == cActionTypeIdle` and `kbUnitGetPlanID(heroID) == -1`.

#### Scenario: Idle and plan-free hero delivers

- GIVEN a hero carries the disappeared relic within 10 meters
- WHEN both checks pass
- THEN the hero is tasked once to the nearest non-full temple

#### Scenario: Player override suppresses delivery

- GIVEN a hero has just picked up a relic
- WHEN the player issues a manual order before the next tick
- THEN no delivery order is issued on this or any subsequent tick; the relic stays with the hero

### R4 Nearest temple with space

SHALL pick the nearest player-owned temple where `kbUnitGetNumberContained(templeID) < kbPlayerGetProtoStatInt(...)`; no-op if none has space.

### R5 Delivery order

SHALL deliver with `aiTaskWorkUnit(heroID, templeID)`.

### R6 Relic identification

SHALL confirm carrying with `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) > 0`, retrieve `kbUnitGetContainedUnitByIndex(heroID, 0)`, and match it to the disappeared relic ID. It MAY scan slots up to `kbUnitGetNumberContained(heroID)` when slot 0 does not match.

#### Scenario: Specific relic match delivers

- GIVEN relic R disappeared and hero H is within 10 meters
- WHEN `kbUnitGetContainedUnitByIndex(H, 0)` returns R
- THEN delivery is issued once

#### Scenario: Different relic skips delivery

- GIVEN relic R disappeared and hero H carries relic S
- WHEN the contained unit ID is S ≠ R
- THEN no delivery order is issued for R

### R7 Hero type filter

SHALL filter by `cUnitTypeHero`; culture-specific carriers are out of scope.

### R8 Human-player guard

SHALL guard all paths with `kbPlayerIsHuman(cMyID)`.

### R10 Bootstrap D1 divergence

SHALL intentionally break bootstrap D1 byte-identity of the fourth mod's `human_assist.xs` by adding the include and registration call.

### R11 Rollback

SHALL remove the file, include/registration call, README additions, and deploy line on rollback.

### R12 Ground-relic disappearance scan

SHALL snapshot alive ground relic IDs and detect disappearances by comparing consecutive tick results.

#### Scenario: Disappearance banner

- GIVEN three relics exist on the ground
- WHEN one unit ID is present in the prior tick but absent in the current tick
- THEN `aiEcho` logs the disappeared unit ID and the remaining relics do not produce extra banners

### R13 Hero proximity query

SHALL query alive player heroes within 10 meters of a disappeared relic's last position using `kbUnitQuerySetMaximumDistance(..., 10.0)`. SHALL use two merged (deduplicated) queries — one filtering on `cUnitTypeLogicalTypeHealable` (the proto unittype shared by 154/159 hero units, including Miko, Atlantean villager-heroes, and most of the Chinese roster) and one filtering on `cUnitTypeHero` (the 5 unhealable military heroes that lack the healable tag: SonOfOsiris, Regent, QianKunQuan, Shogun, BloodMasterQianKunQuan) — so that all 159 hero unit-types are reachable by the scan.

#### Scenario: Proximity radius filters heroes

- GIVEN a disappeared relic's last position
- WHEN one hero is 8 meters away and another is 15 meters away
- THEN the 8-meter hero appears in results and the 15-meter hero does not

## Cross-cutting scenarios

All scenarios are MANUAL (deploy + `aiEcho`).

### B1 — Happy path

- **GIVEN** an idle hero with the mod enabled
- **WHEN** the hero picks up a relic
- **THEN** it is tasked once to the nearest non-full temple and `aiEcho` logs the trigger

### B2 — Player overrides

- **GIVEN** a hero just picked up relic R
- **WHEN** the player issues a manual order before delivery
- **THEN** the feature does not override the order and the relic stays with the hero

### B3 — Sequential relic delivery

- **GIVEN** a hero deposited relic R
- **WHEN** the same hero picks up relic S
- **THEN** delivery triggers once for S

### B4 — Nearest temple full

- **GIVEN** the nearest temple is full and a farther one has space
- **WHEN** a hero picks up a relic near the full temple
- **THEN** delivery targets the nearest temple with available space

### B5 — No temple with space

- **GIVEN** all temples are full
- **WHEN** a hero picks up a relic
- **THEN** no order is issued and `aiEcho` logs "no temple with space"

### B7 — Carrying-relic API verification

- **GIVEN** a hero cycled through pick-up → carry → deposit
- **WHEN** `aiEcho` logs `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic)` and `kbUnitGetContainedUnitByIndex(heroID, 0)` each phase
- **THEN** `kbUnitGetNumberContainedOfType` is `> 0` while carrying and `== 0` after deposit, and `kbUnitGetContainedUnitByIndex` returns the specific relic unit ID while carrying

### B9 — Existing mod regression

- **GIVEN** only the fourth mod is deployed
- **WHEN** repair/scout are used for a human and an AI is present
- **THEN** repair/scout work for the human; no relic activity appears for AI

### B10 — Conflict handling

- **GIVEN** the fourth mod is loaded alongside an existing mod
- **WHEN** AoM:R resolves overlapping `human_assist.xs` overlays
- **THEN** one mod wins and README warns not to load them together

### B11 — D1 divergence and rollback

- **GIVEN** the combined mod source tree
- **WHEN** the fourth mod's `human_assist.xs` is compared before/after rollback
- **THEN** after apply it differs by include/registration call; after rollback it is byte-identical

## Out of scope

- Culture-specific carriers beyond `cUnitTypeHero`.
- Vanilla deposit behavior for non-human players / AI.
- Multiple concurrent `cXSRelicPickedUpHandler` registrations.
- Editing `auto_repair.xs`, `auto_scout.xs`, or the three existing mods.
- AoM:R platform manifest / packaging.
