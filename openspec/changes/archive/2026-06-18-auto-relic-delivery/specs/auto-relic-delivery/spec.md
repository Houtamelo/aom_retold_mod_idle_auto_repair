# Auto-relic-delivery Specification

## Purpose

Passive auto-relic-delivery for the `Human Assist Improvements` mod: when a human player's hero picks up a relic, task it once to the nearest player-owned temple with available space, without overriding manual orders.

## Requirements

| ID | Requirement |
|---|---|
| R1 | Register a `cXSRelicPickedUpHandler` in `autoRelicDelivery_register()` called from `human_assist.xs::main()`; do NOT poll all heroes each tick. |
| R2 | Track `(heroID, relicID)` pairs in append-only arrays as the active runtime path. The engine exposes `kbUnitGetContainedUnitByIndex(heroID, 0)` for the carried relic ID, `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic)` for the type-safe carrying check, and `kbRelicGetTechID(relicID)` to correlate the relic with the `cXSRelicPickedUpHandler` `techID` payload. A per-hero state-machine fallback is NOT used. Re-trigger for a new `(heroID, relicID)` pair. |
| R3 | Only issue when `kbUnitGetActionType(heroID) == cActionTypeIdle`; 1–2 s retry re-tests idle state and marks triggered on skip. |
| R4 | Pick nearest temple with `kbUnitGetNumberContained(templeID) < kbPlayerGetProtoStatInt(...)`; no-op if none has space. |
| R5 | Deliver with `aiTaskWorkUnit(heroID, templeID)`. |
| R6 | Treat `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) > 0` as the carrying signal; verify `kbUnitGetContainedUnitByIndex(heroID, 0)` returns the relic unit ID in-game. |
| R7 | Filter by `cUnitTypeHero`; culture-specific carriers out of scope. |
| R8 | Guard all paths with `kbPlayerIsHuman(cMyID)`. |
| R9 | Document singleton handler caveat in README and startup banner. |
| R10 | Intentionally break bootstrap D1 byte-identity of fourth mod's `human_assist.xs`. |
| R11 | Rollback removes the file, include/registration call, README additions, and deploy line. |

## Scenarios

All scenarios are MANUAL (deploy + `aiEcho`).

### B1 — Happy path

- **GIVEN** an idle hero with the mod enabled
- **WHEN** the hero picks up a relic
- **THEN** it is tasked once to the nearest non-full temple and `aiEcho` logs trigger

### B2 — Player overrides

- **GIVEN** a hero just picked up relic R
- **WHEN** the player issues a manual order before delivery
- **THEN** feature does not override and the pair is marked triggered

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

### B6 — Idle-check edge

- **GIVEN** a hero is mid-pickup-animation at event fire
- **WHEN** the scan and 1–2 s retry run
- **THEN** scan skips; retry delivers if idle or skips permanently on manual order

### B7 — Carrying-relic API verification

- **GIVEN** a hero cycled through pick-up → carry → deposit
- **WHEN** `aiEcho` logs `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic)` and `kbUnitGetContainedUnitByIndex(heroID, 0)` each phase
- **THEN** `kbUnitGetNumberContainedOfType` is `> 0` while carrying and `== 0` after deposit, and `kbUnitGetContainedUnitByIndex` returns a valid relic unit ID while carrying whose `kbRelicGetTechID` matches the event payload

### B8 — Handler singleton documentation

- **GIVEN** `README.md` and the startup banner
- **WHEN** a maintainer checks handler compatibility
- **THEN** both state the singleton registration policy

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
