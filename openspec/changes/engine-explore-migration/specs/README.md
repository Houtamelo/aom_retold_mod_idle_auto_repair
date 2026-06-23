# engine-explore-migration specs

This directory holds the delta specifications for the `engine-explore-migration` change. The change moves `intelligent_auto_scout` from hand-issued `aiTaskMoveUnit` movement to engine-driven `cPlanExplore` area-ID assignment while keeping BFS scoring, heat-map filtering, herd divert, Oracle LOS pausing, and per-area claims.

- [`engine-area-explore.md`](engine-area-explore.md) — How the mod builds and writes area-ID lists into `cExplorePlanExploreAreaIDs`, including the POC helper port and freshness rule after un-park.
- [`oracle-los-monitor.md`](oracle-los-monitor.md) — Oracle-specific pause/resume logic driven by `kbUnitGetStatFloat(..., cUnitStatLOS)` and `cPlanStateIdle`/`cPlanStateExplore` toggles.
- [`intelligent-auto-scout-movement.md`](intelligent-auto-scout-movement.md) — What manual-movement code is removed and what stays, plus the BFS-then-filter ordering.
- [`auto-scout-plan-lifecycle.md`](auto-scout-plan-lifecycle.md) — Lifecycle states (`active`, `parked-divert`, `parked-flee`, `paused-oracle`) and the rule that reassigns area IDs on every return to active.

Inputs:

- Proposal: [`openspec/changes/engine-explore-migration/proposal.md`](../proposal.md)
- Exploration: [`openspec/changes/engine-explore-migration/exploration.md`](../exploration.md)
