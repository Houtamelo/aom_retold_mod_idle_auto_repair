# oracle-los-monitor

## Overview

Run a per-tick monitor for `cUnitTypeAbstractOracle` scouts. When the Oracle's current LOS is well below its maximum the monitor parks the engine explore plan and stops the unit; when LOS is saturated it re-enables the plan so the engine moves the Oracle to a fresh area.

## Requirements

### R1 Oracle-only scope

The system MUST apply the LOS monitor only to units where `kbUnitIsType(unitID, cUnitTypeAbstractOracle) == true`.

### R2 Creation-time engine knob

During `enableAutoScouting`, the system MUST set `aiPlanSetVariableFloat(planID, cExplorePlanStopLOSPercentage, 0, 0.8)` for every Oracle plan.

### R3 LOS polling and transition logic

The system MUST run a rule with `minInterval 1` that, for each Oracle slot:

1. Reads `currentLOS = kbUnitGetStatFloat(unitID, cUnitStatLOS)`.
2. Reads `maxLOS = kbPlayerGetProtoStatInt(cMyID, kbUnitGetProtoUnitID(unitID), cProtoStatLOS)` cast to `float`. If the proto-stat returns `<= 0`, fall back to the cached `gAutoScout_maxOracleLOS` value.
3. If `currentLOS < 0.5 * maxLOS` AND the plan is currently active (`aiPlanGetState(planID) != cPlanStateIdle`), the system MUST call `aiPlanSetState(planID, cPlanStateIdle)` and `aiTaskStopUnit(unitID)`.
4. If `currentLOS / maxLOS >= 1.0` AND the plan is currently parked (`aiPlanGetState(planID) == cPlanStateIdle`), the system MUST call `aiPlanSetState(planID, cPlanStateExplore)`.

### R4 Logging

The system MUST log every transition between paused and active states via `aiEcho`, including the unit ID and the LOS ratio.

### R5 Herd precedence

The system SHALL NOT issue a LOS-monitor pause while the slot is in `cAutoScoutState_Diverting`; herd divert takes precedence over the LOS monitor.

## Scenarios

### S1 Oracle exploring a fresh area

- GIVEN an Oracle with `currentLOS / maxLOS == 0.1` and the plan is active
- WHEN the LOS monitor tick fires
- THEN no pause is issued and the engine continues moving the Oracle.

### S2 Oracle reaches the StopLOSPercentage threshold

- GIVEN the engine pauses the Oracle at `StopLOSPercentage = 0.8`
- WHEN the monitor reads `currentLOS < 0.5 * maxLOS` while the plan is active
- THEN it calls `aiPlanSetState(planID, cPlanStateIdle)` followed by `aiTaskStopUnit(unitID)` and logs the pause.

### S3 Oracle LOS saturates

- GIVEN the Oracle is paused and `currentLOS / maxLOS` reaches `1.0`
- WHEN the monitor tick fires
- THEN it calls `aiPlanSetState(planID, cPlanStateExplore)` and logs the resume.

### S4 Herd divert overrides the monitor

- GIVEN an Oracle is diverting to a herd
- WHEN the monitor would otherwise pause the plan
- THEN it skips the pause so the herd conversion move is not interrupted.

## XS API constraints

| API | Signature | Source |
| --- | --- | --- |
| `kbUnitIsType` | `bool (int unitID, int unitTypeID)` | `docs/doxygen_retail/kbfuncs_8cpp.html:207` |
| `kbUnitGetStatFloat` | `float (int unitID, int statEnumID)` | `docs/doxygen_retail/kbfuncs_8cpp.html:10366` |
| `kbPlayerGetProtoStatInt` | `int (int playerID, int protoUnitID, int statEnumID)` | `docs/doxygen_retail/kbfuncs_8cpp.html:7238` |
| `kbUnitGetProtoUnitID` | `int (int unitID)` | `docs/doxygen_retail/kbfuncs_8cpp.html:221` |
| `cUnitStatLOS` | `const int = 7` | `docs/MythTRConstants.txt:3771` |
| `cProtoStatLOS` | `const int = 6` | `docs/MythTRConstants.txt:3810` |
| `cUnitTypeAbstractOracle` | `const int = 987` | `docs/MythTRConstants.txt:1136` |
| `aiPlanSetVariableFloat` | `void (int planID, int variableIndex, int valueIndex, float value)` | `docs/doxygen_retail/aifuncs_8cpp.html:4421` |
| `aiPlanSetState` | `void (int planID, int planState)` | `docs/doxygen_retail/aifuncs_8cpp.html:4139` |
| `aiPlanGetState` | `int (int planID)` | `docs/doxygen_retail/aifuncs_8cpp.html:2968` |
| `aiTaskStopUnit` | `bool (int unitID)` | `docs/doxygen_retail/aifuncs_8cpp.html:5901` |

## Out of scope

- Non-Oracle scouts (regular `AbstractScout` units use engine explore without LOS pausing).
- Military-unit scouting.

## Open research

- **[RESEARCH-DEFERRED: RP3]** The exact engine semantics of `cExplorePlanStopLOSPercentage` are not documented beyond the constant. The spec treats it as an Oracle-specific halt threshold and relies on the explicit monitor for the `< 50 % LOS` pause and `100 % LOS` resume.
