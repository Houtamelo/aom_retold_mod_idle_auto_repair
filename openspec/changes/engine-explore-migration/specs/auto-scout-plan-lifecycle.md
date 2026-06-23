# auto-scout-plan-lifecycle

## Overview

Define a small lifecycle state machine for each registered `cPlanExplore`. The plan starts active, is briefly parked for herd divert or flee, and is paused for an Oracle LOS dip. Transitions back to active always recompute the area list.

## State machine

```
          +------------------+
          | register / un-park |
          v
      +--------+
      | active |<--------------------------+
      +--------+                           |
       |        ^                          |
       | herd   | herd converted           |
       v        | (re-assign areas)        |
  +-------------+                          |
  | parked-     |                          |
  | divert      |                          |
  +-------------+                          |
       | flee trigger                     |
       |                                  |
       v                                  |
  +-------------+                        |
  | parked-     |                        |
  | flee        |------------------------+
  +-------------+  (cooldown elapsed + re-assign)
       ^
       | Oracle LOS < 0.5*max
       |
  +-------------+
  | paused-     |
  | oracle      |
  +-------------+
       | Oracle LOS = 100 %
       +--------------------> active (re-assign areas)
```

## Requirements

### R1 Lifecycle constants

The system MUST define the following local constants in `auto_scout.xs`:

- `cAutoScout_PlanStateActive`
- `cAutoScout_PlanStateParkedDivert`
- `cAutoScout_PlanStateParkedFlee`
- `cAutoScout_PlanStatePausedOracle`

The existing `cAutoScout_PlanStateIdle = 23` MUST be kept as an alias for `cPlanStateIdle` and used only when the engine plan needs to be parked.

### R2 State storage

The system MUST store the per-slot plan lifecycle mode in a new `cAutoScout_PlanStateMode*` enum, separate from the existing `cAutoScoutState_*` unit-behavior enum. This avoids overloading `Idle`/`Walking`/`Working` with plan-parking semantics and keeps herd/flee behavior orthogonal.

### R3 Registration no longer unconditionally parks

The system MUST remove the unconditional `aiPlanSetState(planID, cAutoScout_PlanStateIdle)` from `autoScout_register`. After registration the slot MUST start in `cAutoScout_PlanStateActive`.

### R4 Re-assign areas on reactivation

On any transition from `parked-divert`, `parked-flee`, or `paused-oracle` to `active`, the system MUST recompute and reassign `cExplorePlanExploreAreaIDs` before calling `aiPlanSetState(planID, cPlanStateExplore)`.

### R5 Transition logging

The system SHALL log every state transition with `aiEcho`, including the slot, unit ID, old mode, and new mode.

### R6 Death handling

The system SHALL handle unit death in any mode by calling `autoScout_dropFromPool(slot)` and destroying the plan, preserving existing `cleanupLingeringExplorePlans` behavior.

## Scenarios

### S1 Fresh registration stays active

- GIVEN `enableAutoScouting(unitID)` creates a new `cPlanExplore`
- WHEN `autoScout_register(planID, unitID)` is called for a human player
- THEN the slot is added with mode `active` and the engine immediately consumes `cExplorePlanExploreAreaIDs`.

### S2 Herd divert parks and reactivates

- GIVEN a scout is active and `autoScout_tryDivert` succeeds
- WHEN the slot mode changes to `parked-divert`
- THEN `aiPlanSetState(planID, cPlanStateIdle)` is called and a manual herd move is issued
- AND when the herd converts, the mode returns to `active` with a fresh area list.

### S3 Oracle LOS dip pauses and resumes

- GIVEN an Oracle is active and its LOS monitor detects `currentLOS < 0.5 * maxLOS`
- WHEN the monitor fires
- THEN the mode becomes `paused-oracle`, the plan is parked, and the unit is stopped
- AND when `currentLOS / maxLOS >= 1.0`, the mode returns to `active` with a fresh area list.

### S4 Scout killed in any mode

- GIVEN a registered scout in any lifecycle mode
- WHEN the unit dies
- THEN `autoScout_dropFromPool` removes the slot and `cleanupLingeringExplorePlans` destroys the now-empty plan.

## XS API constraints

| API | Signature | Source |
| --- | --- | --- |
| `aiPlanSetState` | `void (int planID, int planState)` | `docs/doxygen_retail/aifuncs_8cpp.html:4139` |
| `aiPlanGetState` | `int (int planID)` | `docs/doxygen_retail/aifuncs_8cpp.html:2968` |
| `aiPlanDestroy` | `void (int planID)` | `docs/doxygen_retail/aifuncs_8cpp.html:1862` |
| `cPlanStateIdle` | `const int = 23` | `docs/MythTRConstants.txt:3166`; also `auto_scout.xs:80` |
| `cPlanStateExplore` | `const int = 6` | `docs/MythTRConstants.txt:3149` |

Note: `aiPlanSetState` "sets the plan state" and "has a lot of consequences for the plan (different for each type and state)" (`docs/doxygen_retail/aifuncs_8cpp.html:4157`). For this migration the safe disable/enable pair is `cPlanStateIdle` to park and `cPlanStateExplore` to unpark.

## Out of scope

- A `parked-manual-override` state for player-issued commands.
- Reusing engine `cPlanStateDone` or `cPlanStateFailed` for pause; only `cPlanStateIdle` is used.

## Open research

- **[RESEARCH-DEFERRED: RP1/RP6]** No dedicated `aiPlanSetActive` API exists. The confirmed disable/enable mechanism is `aiPlanSetState(planID, cPlanStateIdle)` / `aiPlanSetState(planID, cPlanStateExplore)`. The value `cPlanStateIdle = 23` is stable in `docs/MythTRConstants.txt:3166`; patch stability is assumed but should be re-checked after each game update.
