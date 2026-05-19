//==============================================================================
// auto_scout.xs (Intelligent Auto-Scout mod)
//
// Frontier-based replacement for the engine's player-triggered cPlanExplore
// for non-Oracle AbstractScout units. Walks each scout to nearby unexplored
// areas, prioritizing areas closest to the player's main town center via a
// BFS over the area-adjacency graph (BFS bounds reachability from the scout
// position; the chosen candidate among reachable ones is the one with
// smallest Euclidean distance to the main TC).
//
// See docs/superpowers/specs/2026-05-04-intelligent-auto-scout-design.md
// for the full design.
//==============================================================================

const int cAutoScoutState_Idle      = 0;
const int cAutoScoutState_Walking   = 1;
const int cAutoScoutState_Working   = 2;
const int cAutoScoutState_Diverting = 3;
const int cAutoScoutState_Stationed = 4;
const int cAutoScoutState_Fleeing   = 5;

// Extra detection radius beyond the scout's LOS, in tiles. Catches herds that
// briefly flicker at the LOS boundary between ticks. Per-result we still
// require the herd's tile to be visible OR fogged (i.e. not pure black /
// never-explored), which keeps the AI from "cheating" toward unseen herds.
const float cAutoScout_HerdLOSBuffer = 12.0;

// Skip areas where less than this percent of tiles are still black (unexplored).
// Regular scouts use 10 (avoid mostly-revealed areas where they'd waste a hop).
// Oracles use a lower bar because their MaxLOS=34 saturation reveals huge
// rings, so by the time the oracle has 7+ siblings parked the only candidates
// left have very small unrevealed fractions; setting the bar too high blocks
// every nearby option and forces the BFS to bubble out to map-edge slivers.
const int cAutoScout_BlackTilesPercentMin       = 10;
const int cAutoScout_BlackTilesPercentMinOracle = 3;

// Arrival = within this distance of target waypoint (kbUnitGetDistanceToPoint
// is edge-to-point, so 1.5 tiles ~= unit center is ~2 tiles from target).
// Tight threshold so the scout walks all the way to the waypoint before we
// re-evaluate; otherwise small areas (sqrt(tileCount) <= LOS) trigger
// "arrived + small area = move on" while the scout is still LOS tiles short
// of the centroid. Engine-Idle is accepted as a fallback for blocked
// centroids only after the scout has had time to actually start moving.
const float cAutoScout_ArrivalDistance       = 1.5;
const int   cAutoScout_MinTicksBeforeIdleAccept = 2;

// Margin to keep clamped positions inside the map (kbAreaGetIDByPosition
// at the exact map edge can still warn).
const float cAutoScout_MapEdgeMargin = 1.0;

// Drop scout's claim if it sits in WALKING/WORKING with no progress for this
// many consecutive rule ticks.
const int cAutoScout_StuckTickLimit = 30;

// Cap on chained state-machine iterations per scout per rule firing (defensive
// against unexpected state cycles).
const int cAutoScout_MaxChainPerTick = 4;

// Max hops in a per-scout corridor (BFS predecessor chain from start area to
// chosen target area, inclusive of both endpoints). BFS batchMax is 2 for
// regular scouts and 3 for oracles, so a real corridor is at most 4 entries;
// 8 leaves comfortable headroom and keeps the flat-array math trivial.
const int cAutoScout_MaxCorridorHops = 8;

// Action-type code reported by kbUnitGetActionType when an Oracle has reached
// its peak AutoLOS bonus and switches to the meditation/glow animation.
// Identified empirically (2026-05-06): the unit's reported action transitioned
// 7 -> 37 the moment its current LOS hit the cap and stayed at 37 thereafter.
// No documented cActionType* constant in the
// extracted doxygen / shipped XS scripts / BANG docs maps to this value, so
// we hardcode it. If a future patch surfaces a real constant (something like
// cActionTypeIdleStatBonusFull), replace this magic number with that.
const int cAutoScout_OracleSaturatedActionType = 37;

// Plan-state integer for cPlanStateIdle (from docs/MythTRConstants.txt).
// Setting this on a cPlanExplore at registration tells the engine "this
// plan is parked, don't iterate" while keeping the plan alive as a UI
// marker. The visible state for an active cPlanExplore is cPlanStateExplore
// (value 6); cPlanStateIdle (value 23) is what we set after registration.
const int cAutoScout_PlanStateIdle = 23;

// Cold-start value for the dynamic gAutoScout_maxOracleLOS cache. Used until
// any oracle is observed in the saturated action state (action ==
// cAutoScout_OracleSaturatedActionType) with a higher current LOS. 20.0 is
// a plausible default below the regular Oracle cap of 25 (so the cache will
// climb on first observed saturation) and above any oracle's base LOS.
const float cAutoScout_OracleColdCacheMaxLOS = 20.0;

// Oracle-on-oracle soft penalty (2026-05-14). Replaces the previous
// hard-skip based on cAutoScout_OracleExclusionFactor. Two oracles whose
// claim circles touch can still overlap their LOS rings; the linear
// falloff out to 2 * gAutoScout_maxOracleLOS captures that.
//   range  = cAutoScout_OracleOnOracleRangeFactor * gAutoScout_maxOracleLOS
//   weight = cAutoScout_OracleOnOracleWeight        (additive, not multiplicative;
//            should dominate the sum of the other weights -- WeightTC + WeightScout
//            + WeightDensity + DangerWeight = ~1.175 -- so a fully-overlapping other
//            oracle alone is enough to disqualify the area in practice).
const float cAutoScout_OracleOnOracleRangeFactor = 2.0;
const float cAutoScout_OracleOnOracleWeight      = 3.0;

// Padding added to an oracle's claim radius when a NON-oracle source is
// evaluating area overlap with the oracle (mechanism #2 in the doc:
// autoScout_oraclePenalty). Keeps regular scouts further from oracle
// claim edges than a strict radius would.
const float cAutoScout_OraclePenaltyRadiusPad = 15.0;

// Density-radius override: when source is an oracle and the other scout
// is NOT an oracle, expand the soft density radius from
// cAutoScout_DensityRadius (30) to this value so oracles avoid steering
// to areas already covered by regular scouts even at slightly larger range.
const float cAutoScout_DensityRadiusOracleToOther = 45.0;

// Area-score weights (sum need not be exactly 1.0 since we only compare
// scores, but normalized weights make tuning intuitive). Each subscore is
// produced in roughly [0, 1].
const float cAutoScout_WeightTC      = 0.35;   // closer to main TC -> higher
const float cAutoScout_WeightScout   = 0.525;  // closer to picking scout -> higher (0.35 * 1.5)
const float cAutoScout_WeightDensity = 0.15;   // fewer other scouts nearby -> higher

// Danger avoidance. Threshold is calibrated against OUR heat-map
// (autoScout_effectiveDanger), not engine kbAreaGetDangerLevel which was
// the old reference. A single TC with DPS ~= 20 should hard-skip every
// area within its flood-fill reach. The flood adds
// magnitude*(1 - dist/(maxDist+pad)) per pass.
const float cAutoScout_DangerHardSkip      = 20.0;
const float cAutoScout_DangerWeight        = 0.15;
const int   cAutoScout_FleeMinDurationMs   = 5000;
const int   cAutoScout_BlacklistDurationMs = 90000;

// Baseline effective danger applied to fully-unexplored areas (where we have
// no ground-truth). 0.0 = presume safe: with no observation, scouts treat
// unknown terrain as low-danger and will explore freely. The previous value
// (20) was calibrated for the engine's kbAreaGetDangerLevel which returned
// 35-95; our own heat-map is 0-based so the baseline must be 0-based too.
// For fully-explored areas we trust gAutoScout_heat[] directly; we blend
// linearly between the two by the fraction of explored tiles. See
// autoScout_effectiveDanger.
const float cAutoScout_DangerBaseline      = 0.0;

// Heat-map parameters. Per-area float array recomputed each tick from
// visible enemy threats (full reset + ADD across threats, so multiple units
// in the same area pile up). Each threat does a flood-fill outward from its
// position; an area receives heat only while
//   distance(entityPos, kbAreaGetCenter(area)) <= max(entityRange, MinFloodReach).
// Damage/death event bumps follow the same flood-fill rules with a
// synthetic event range.
//
//   cAutoScout_HeatMinDPS:           per-threat DPS gate (filters out scouts).
//   cAutoScout_HeatRangeDivisor:     contribution = DPS * (1 + range / divisor).
//   cAutoScout_HeatMinFloodReach:    floor on the flood-fill cutoff distance
//                                    so melee threats spread at least this
//                                    far. Bigger threats use their own range.
//   cAutoScout_HeatMobileFogTimeoutMs: how long a non-building threat remains
//                                      a heat producer after we lose LOS.
//   cAutoScout_HeatDamageEvent*:  bump + flood radius for a scout hit event.
//   cAutoScout_HeatDeathEvent*:   stronger bump + radius for a scout death.
const float cAutoScout_HeatMinDPS                  = 4.0;
const float cAutoScout_HeatRangeDivisor            = 20.0;
const float cAutoScout_HeatMinFloodReach           = 15.0;
const float cAutoScout_HeatAreaPad                 = 14.0;
const int   cAutoScout_HeatMobileFogTimeoutMs      = 10000;
const float cAutoScout_HeatDamageEventMagnitude    = 150.0;
const float cAutoScout_HeatDamageEventRange        = 20.0;
const int   cAutoScout_HeatDamageEventDurationMs   = 10000;
const float cAutoScout_HeatDeathEventMagnitude     = 500.0;
const float cAutoScout_HeatDeathEventRange         = 30.0;
const int   cAutoScout_HeatDeathEventDurationMs    = 10000;

// Flee-area picker (autoScout_findFleeArea). BFS expands outward from the
// scout's current area up to cAutoScout_FleeBfsDepth hops, refusing to
// propagate through dangerous areas (so the chosen flee target is reachable
// via a safe corridor). Score has three components:
//   safety       (0.7)  : low danger = high score, dominant.
//   awayDanger   (0.2)  : far from the area we fled from = high score.
//   nearScout    (0.075): close to scout's current position = high score.
//                          0.05 baseline * 1.5 bump for near-scout preference.
const int   cAutoScout_FleeBfsDepth         = 4;
const float cAutoScout_FleeWeightSafety     = 0.7;
const float cAutoScout_FleeWeightAwayDanger = 0.2;
const float cAutoScout_FleeWeightNearScout  = 0.075;

// Other scouts beyond this distance from a candidate area do not influence
// that area's density subscore.
const float cAutoScout_DensityRadius = 30.0;

// Frontier-walk parameters (in-area exploration during WORKING state).
// Cap on number of frontier waypoints walked before giving up on the area;
// safety net against oscillation in pathological shapes.
const int cAutoScout_MaxFrontierSteps = 15;
// Sample radii (multiples of LOS), inside-out. The first radius is just
// outside scout's current LOS so the candidate is provably unscouted.
const int cAutoScout_FrontierRadii   = 5;
const int cAutoScout_FrontierAngles  = 8;

// Frontier candidates within this many tiles of ANY previously-visited
// waypoint in the current WORKING session are rejected. Defeats the
// deterministic A->B->A oscillation that the angle-sweep sampling otherwise
// produces in elongated / awkwardly-shaped areas, where each of two positions
// happens to be the other's first-qualifying frontier candidate.
// 4.0 is comfortably larger than the arrival threshold (1.5) so "essentially
// the same spot" is filtered, and comfortably smaller than the inter-angle
// chord length at the smallest sampling radius (~0.76 * (LOS+2) >= 15 tiles
// for typical scout LOS), so legitimate adjacent angular samples still pass.
const float cAutoScout_VisitedRevisitDistance = 4.0;

//------------------------------------------------------------------------------
// Globals
//------------------------------------------------------------------------------

extern int[]    gAutoScout_unitID         = default;
extern int[]    gAutoScout_planID         = default;
extern int[]    gAutoScout_state          = default;
extern int[]    gAutoScout_targetAreaID   = default;
extern vector[] gAutoScout_targetWaypoint = default;
// gAutoScout_workSteps: in WORKING state, counts frontier steps taken so far
// (capped by cAutoScout_MaxFrontierSteps). Reset to 0 on entering WORKING.
extern int[]    gAutoScout_workSteps      = default;
extern int[]    gAutoScout_stuckTicks     = default;

// Per-scout safe-corridor state. The Walking-state next-area BFS reconstructs
// its predecessor chain into this flat array so the engine pathfinder is
// forced through a sequence of BFS-validated safe areas (rather than letting
// it cut corners through a heat zone on the way to a far target). Layout:
// gAutoScout_corridorAreas[slot * MaxCorridorHops + i] is the i-th area in
// the corridor for that slot, where i = 0 is the scout's start area and
// i = len-1 is the chosen target. gAutoScout_corridorLen[slot] is the total
// number of valid entries (1..MaxCorridorHops). Reset to len = 1 (just the
// start area, no extra hops) by setStateIdle and friends.
extern int[] gAutoScout_corridorAreas = default;
extern int[] gAutoScout_corridorLen   = default;

// Transient buffer filled by autoScout_findNextArea after a successful pick:
// ordered area IDs from the scout's start area (index 0) to the chosen target
// (index Len-1), reconstructed from the BFS predecessor chain. Read once by
// the caller (Idle-state handler) and copied into per-slot corridor state.
extern int[] gAutoScout_bfsResultPath = default;
extern int   gAutoScout_bfsResultLen  = 0;
// Per-call BFS predecessor table. Index = area ID, value = predecessor area
// ID (or -1 if not yet visited, or the start area's slot which stays -1).
// Reallocated to size = kbAreaGetNumber() at every findNextArea entry, then
// walked back from the chosen target by autoScout_buildBfsPath.
extern int[] gAutoScout_bfsPredecessor = default;

// Per-area claim. Value = claiming scout's unit ID, 0 = unclaimed.
extern int[] gAutoScout_areaClaim         = default;

// Per-area "we visited this area's centroid" flag. Set permanently to 1 once
// any of our scouts reaches the area's centroid (the WALKING-state target).
// Areas with this flag are excluded from candidacy thereafter, regardless of
// how many black tiles the engine still reports for the area. This breaks
// the loop where a scout reaches the centroid but the area's black-tile
// count is still > 10% (the coverage of our centroid + mini-ring algorithm
// is approximate), causing repeated re-assignment of the same area to the
// same scout.
extern int[] gAutoScout_areaSelfScouted   = default;

extern bool  gAutoScout_areaArraysInited  = false;

// Per-scout: herd currently being diverted to in DIVERTING state. -1 when not diverting.
extern int[] gAutoScout_targetHerdID = default;

// Per-scout danger-avoidance state.
//   fleeUntilMs[slot]:  xsGetTimeMS() value at which the FLEEING hold expires.
//   fleeFromArea[slot]: area we fled from (-1 when not fleeing). Diagnostics only.
extern int[] gAutoScout_fleeUntilMs   = default;
extern int[] gAutoScout_fleeFromArea  = default;

// Per-scout HP and last-known position tracking. Used to detect damage
// (lastHP decreases tick-over-tick) and death (unit becomes invalid) events
// so we can bump the heat-map at the scout's location.
extern float[]  gAutoScout_lastHP  = default;
extern vector[] gAutoScout_lastPos = default;

// Danger blacklist. Two parallel append-only arrays keyed by areaID. Areas
// enter when a scout aborts because of them and remain excluded from BFS
// picking until expiryMs < xsGetTimeMS(). Linear scan on lookup; bounded by
// the number of distinct dangerous areas seen, which is small in practice.
extern int[] gAutoScout_blacklistedAreaIDs  = default;
extern int[] gAutoScout_blacklistedExpiryMs = default;

// Per-area heat-map. Indexed by areaID, sized to kbAreaGetNumber() at first
// rule firing (autoScout_initHeatArray). Fully reset and rebuilt each tick
// in autoScout_updateHeatMap from currently-known threats and active events.
extern float[] gAutoScout_heat       = default;
extern bool    gAutoScout_heatInited = false;

// Cached threat-query handles for the heat-map update. Split into two:
//   - threatUnitQuery:     cUnitTypeMilitaryUnit (soldiers, archers, cavalry,
//                          myth units -- combat actors only; excludes villagers,
//                          heroes, animals which the DPS gate would drop anyway).
//   - threatBuildingQuery: cUnitTypeBuilding (TCs, towers, walls, gates, all
//                          static structures including resource-drop sites).
// Two queries instead of one cUnitTypeAll because cUnitTypeAll also returns
// gold mines, trees, columns, relics, berries -- inert flora/scenery that we
// process and then discard. The military-only narrowing on the unit side
// further drops villagers + heroes (DPS too low) and animals (no attack) from
// per-tick iteration.
extern int gAutoScout_threatUnitQuery     = -1;
extern int gAutoScout_threatBuildingQuery = -1;

// Per-proto threat profile cache. Parallel arrays:
//   ID:           proto unit type ID (key)
//   isThreat:     1 if peak DPS > cAutoScout_HeatMinDPS, else 0
//   contribution: max DPS * (1 + range/divisor) across attack actions
//   range:        attack range of the action that produced the max contrib
//                 (used by the heat-map flood-fill to bound spread distance)
//   isVillager:   1 if proto is AbstractVillager (skip entirely)
//   isBuilding:   1 if proto is Building (fogged-OK, no fog timeout)
// Linear scan; the proto set we encounter in a game is small (~20-50 distinct).
extern int[]   gAutoScout_protoThreatID     = default;
extern int[]   gAutoScout_protoThreatIsT    = default;
extern float[] gAutoScout_protoThreatContrib = default;
extern float[] gAutoScout_protoThreatRange   = default;
extern int[]   gAutoScout_protoThreatIsVill = default;
extern int[]   gAutoScout_protoThreatIsBld  = default;

// Per-(mobile-)unit fog timer. We record the last time each enemy unit was
// CURRENTLY VISIBLE; mobile units (non-building) stop contributing heat once
// they've been fogged for more than cAutoScout_HeatMobileFogTimeoutMs.
// Buildings are exempt (they don't move, so fogged-position is still accurate).
extern int[] gAutoScout_unitLastSeenID = default;
extern int[] gAutoScout_unitLastSeenMs = default;

// Damage/death event bumps. Five parallel arrays — origin position, flood
// radius, magnitude, expiry timestamp, and a diag-only area. Active events
// are flood-filled into the heat map each tick by autoScout_updateHeatMap
// until expiry.
extern vector[] gAutoScout_eventPos       = default;
extern float[]  gAutoScout_eventRange     = default;
extern float[]  gAutoScout_eventMagnitude = default;
extern int[]    gAutoScout_eventExpiryMs  = default;
extern int[]    gAutoScout_eventArea      = default;  // diag/log only

// Diagnostic counters for one BFS pass (reset at top of findNextArea, echoed
// at every return path). Used to triage "scout immediately untoggles" issues
// where the candidate pool is being filtered out by an unexpected reason.
// Heat-map diagnostic throttle counter. Incremented every tick; logs fire
// every 5 ticks (~5s with minInterval=1) plus any tick with non-empty
// threat query or active damage/death events.
extern int   gAutoScout_heatDiagTickCounter = 0;

extern int   gAutoScout_diag_considered    = 0;
extern int   gAutoScout_diag_rejClaim      = 0;
extern int   gAutoScout_diag_rejSelf       = 0;
extern int   gAutoScout_diag_rejBlacklist  = 0;
extern int   gAutoScout_diag_rejDanger     = 0;
extern int   gAutoScout_diag_rejTiles      = 0;
extern int   gAutoScout_diag_rejPath       = 0;
extern int   gAutoScout_diag_rejOracle     = 0;
extern int   gAutoScout_diag_passed        = 0;
// Counts of BFS propagation-blocks: how many times we refused to expand
// neighbors from a dangerous or blacklisted area.
extern int   gAutoScout_diag_blockDanger    = 0;
extern int   gAutoScout_diag_blockBlacklist = 0;
extern float gAutoScout_diag_dangerMin     = 0.0;
extern float gAutoScout_diag_dangerMax     = 0.0;
extern float gAutoScout_diag_dangerSum     = 0.0;
extern int   gAutoScout_diag_dangerCount   = 0;
// First three danger-rejected areas (areaID + danger value) for value sampling.
extern int   gAutoScout_diag_rejID0        = -1;
extern float gAutoScout_diag_rejDng0       = 0.0;
extern int   gAutoScout_diag_rejID1        = -1;
extern float gAutoScout_diag_rejDng1       = 0.0;
extern int   gAutoScout_diag_rejID2        = -1;
extern float gAutoScout_diag_rejDng2       = 0.0;

// Visited-waypoint memory for the frontier-walk algorithm. Two parallel flat
// arrays keyed by unitID (not by slot, so removeIndex-driven slot shifts don't
// invalidate the bookkeeping). Each entry records a waypoint the scout has
// already been issued to walk to during the current WORKING session. Cleared
// on every IDLE transition (including pool eviction). Read by
// autoScout_findFrontierWaypoint to reject candidates near any recorded
// waypoint, breaking the no-history oscillation loop.
extern vector[] gAutoScout_workVisitedPos  = default;
extern int[]    gAutoScout_workVisitedUnit = default;

// Per-herd, append-only, never unmarked. Any herd ever selected by any scout for divert.
// Filters subsequent divert candidates so each herd is attempted at most once globally.
extern int[] gAutoScout_attemptedHerdIDs = default;

// Per-herd, append-only, never unmarked. Any herd we've already issued a home-move for.
// Single-issue guarantee — prevents fighting subsequent player command overrides.
extern int[] gAutoScout_redirectedHerdIDs = default;

// Cached unit-query handles, lazily initialized.
extern int gAutoScout_herdQuery       = -1;
extern int gAutoScout_ownedHerdQuery  = -1;
extern int gAutoScout_nearestTCQuery  = -1;

// Cached unit-type ID for "LogicalTypeConvertsHerds". Resolved lazily via
// kbGetUnitTypeID — the named constant cUnitTypeLogicalTypeConvertsHerds is
// not exposed in the stock AI scripts, so we look it up by string instead.
extern int gAutoScout_typeConvertsHerds = -1;

// Cached unit-type ID for "AbstractTownCenter". Resolved lazily — covers
// TownCenter, CitadelCenter (created by the Egyptian Citadel god-power on a
// TC), VillageCenter, and TownCenterAbandoned. cUnitTypeTownCenter only
// matches the "TownCenter" proto, missing the post-godpower CitadelCenter
// proto and other variants.
extern int gAutoScout_typeAbstractTC = -1;

// Dynamic cache: highest currentLOS ever observed on any of our oracles while
// in the saturated action state (action == cAutoScout_OracleSaturatedActionType).
// Cold-started to cAutoScout_OracleColdCacheMaxLOS; climbs only on confirmed
// saturation events (never decreases). Used as the denominator for the 50%
// movement floor and as the claim radius for the oracle-overlap heuristic
// checks. Keyed at the player level (techs that bump the cap apply equally).
extern float gAutoScout_maxOracleLOS = cAutoScout_OracleColdCacheMaxLOS;

// Cached query handle for "all of cMyID's alive AbstractOracle units". Used
// by the heuristic to enumerate every oracle (toggled-on AND not), so player-
// controlled oracles still influence target-area selection.
extern int gAutoScout_oracleQuery = -1;

float floatClamp01(float value = 0.0) {
    if (value < 0.0) { value = 0.0; }
    if (value > 1.0) { value = 1.0; }
    return(value);
}

float floatClamp(float value = 0.0, float lo = 0.0, float hi = 1.0) {
    if (value < lo) { value = lo; }
    if (value > hi) { value = hi; }
    return(value);
}

//------------------------------------------------------------------------------
// Visited-waypoint memory (used by frontier-walk to break A<->B oscillation).
// Defined here -- ahead of pool management -- because autoScout_dropFromPool
// and autoScout_setStateIdle both call autoScout_clearVisited, and XS resolves
// function references at parse time (no forward refs).
//
// Storage is flat parallel arrays keyed by unitID rather than by slot, so
// removeIndex-driven slot shifts in autoScout_dropFromPool don't invalidate
// the bookkeeping.
//------------------------------------------------------------------------------

bool autoScout_hasVisitedNear(int unitID = -1, vector pos = cInvalidVector)
{
   if (unitID < 0) { return(false); }
   int n = gAutoScout_workVisitedUnit.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_workVisitedUnit[i] != unitID) { continue; }
      if (xsVectorDistanceXZ(gAutoScout_workVisitedPos[i], pos) < cAutoScout_VisitedRevisitDistance)
      {
         return(true);
      }
   }
   return(false);
}

void autoScout_recordVisited(int unitID = -1, vector pos = cInvalidVector)
{
   if (unitID < 0) { return; }
   gAutoScout_workVisitedPos.add(pos);
   gAutoScout_workVisitedUnit.add(unitID);
}

// Drops every entry whose unit matches unitID. Backward iteration so
// in-place removeIndex doesn't skip elements.
void autoScout_clearVisited(int unitID = -1)
{
   if (unitID < 0) { return; }
   for (int i = gAutoScout_workVisitedUnit.size() - 1; i >= 0; i = i - 1)
   {
      if (gAutoScout_workVisitedUnit[i] == unitID)
      {
         gAutoScout_workVisitedUnit.removeIndex(i);
         gAutoScout_workVisitedPos.removeIndex(i);
      }
   }
}

//------------------------------------------------------------------------------
// Pool management
//------------------------------------------------------------------------------

void autoScout_initAreaArrays()
{
   if (gAutoScout_areaArraysInited == true) { return; }
   int areaCount = kbAreaGetNumber();
   for (int i = 0; i < areaCount; i++)
   {
      gAutoScout_areaClaim.add(0);
      gAutoScout_areaSelfScouted.add(0);
   }
   gAutoScout_areaArraysInited = true;
}

void autoScout_releaseClaim(int slot = -1)
{
   if (slot < 0) { return; }
   int areaID = gAutoScout_targetAreaID[slot];
   if (areaID >= 0 && areaID < gAutoScout_areaClaim.size())
   {
      gAutoScout_areaClaim[areaID] = 0;
   }
   gAutoScout_targetAreaID[slot] = -1;
}

void autoScout_dropFromPool(int slot = -1)
{
   if (slot < 0) { return; }
   if (slot < gAutoScout_unitID.size())
   {
      int droppedUnit = gAutoScout_unitID[slot];
      int droppedState = gAutoScout_state[slot];
      aiEcho("autoScout: dropFromPool slot=" + slot + " unit=" + droppedUnit
         + " state=" + droppedState);
      autoScout_clearVisited(droppedUnit);

      // Death detection: if the unit ID is no longer valid AT THE MOMENT WE
      // DROP, the scout was killed (vs. player-cancelled, in which case the
      // unit is still alive and the plan was destroyed). Emit a strong heat
      // event at the cached last-known position. Inlined here because
      // autoScout_addHeatEvent / autoScout_isOnMap live below this point in
      // source order.
      if (kbUnitGetIsIDValid(droppedUnit) == false)
      {
         vector lastPos = gAutoScout_lastPos[slot];
         if (kbGetIsLocationOnMap(lastPos) == true)
         {
            int deathArea = kbAreaGetIDByPosition(lastPos);
            if (deathArea >= 0)
            {
               gAutoScout_eventPos.add(lastPos);
               gAutoScout_eventRange.add(cAutoScout_HeatDeathEventRange);
               gAutoScout_eventMagnitude.add(cAutoScout_HeatDeathEventMagnitude);
               gAutoScout_eventExpiryMs.add(xsGetTimeMS() + cAutoScout_HeatDeathEventDurationMs);
               gAutoScout_eventArea.add(deathArea);
               aiEcho("autoScout: DEATH event slot=" + slot + " unit=" + droppedUnit
                  + " area=" + deathArea + " pos=" + lastPos);
            }
         }
      }
   }
   autoScout_releaseClaim(slot);
   gAutoScout_unitID.removeIndex(slot);
   gAutoScout_planID.removeIndex(slot);
   gAutoScout_state.removeIndex(slot);
   gAutoScout_targetAreaID.removeIndex(slot);
   gAutoScout_targetWaypoint.removeIndex(slot);
   gAutoScout_workSteps.removeIndex(slot);
   gAutoScout_stuckTicks.removeIndex(slot);
   gAutoScout_targetHerdID.removeIndex(slot);
   gAutoScout_fleeUntilMs.removeIndex(slot);
   gAutoScout_fleeFromArea.removeIndex(slot);
   gAutoScout_lastHP.removeIndex(slot);
   gAutoScout_lastPos.removeIndex(slot);
   gAutoScout_corridorLen.removeIndex(slot);
   // Remove this slot's MaxCorridorHops entries from the flat areas array.
   // Each removeIndex call shifts subsequent entries left by 1, so calling it
   // MAX times at the same starting offset pops exactly the slot's block.
   int corridorOffset = slot * cAutoScout_MaxCorridorHops;
   for (int j = 0; j < cAutoScout_MaxCorridorHops; j = j + 1)
   {
      if (corridorOffset < gAutoScout_corridorAreas.size())
      {
         gAutoScout_corridorAreas.removeIndex(corridorOffset);
      }
   }
}

//------------------------------------------------------------------------------
// Position helpers
//------------------------------------------------------------------------------

bool autoScout_isOnMap(vector pos = cInvalidVector)
{
   return(kbGetIsLocationOnMap(pos));
}

// Clamp X/Z to [margin, mapSize - margin]. Used on computed ring waypoints
// so far-out rings produce in-bounds destinations along the map edge instead
// of being skipped, which would leave map-edge bands unexplored.
vector autoScout_clampToMap(vector pos = cInvalidVector)
{
   float minX = cAutoScout_MapEdgeMargin;
   float maxX = kbGetMapXSize() - cAutoScout_MapEdgeMargin;
   float minZ = cAutoScout_MapEdgeMargin;
   float maxZ = kbGetMapZSize() - cAutoScout_MapEdgeMargin;
   float x = pos.x;
   float z = pos.z;
   if (x < minX) { x = minX; }
   if (x > maxX) { x = maxX; }
   if (z < minZ) { z = minZ; }
   if (z > maxZ) { z = maxZ; }
   return(xsVectorCreate(x, pos.y, z));
}

//------------------------------------------------------------------------------
// Heat-map (2026-05-13). Per-area float, fully reset and rebuilt each tick.
// The base layer sums (ADD) contributions of known-position enemy threats
// (buildings: always; mobile units: only while currently-visible OR within
// cAutoScout_HeatMobileFogTimeoutMs of last LOS; villagers: never).
// Persistent damage/death event bumps are added on top of the base layer
// for their duration. Replaces kbAreaGetDangerLevel.
//------------------------------------------------------------------------------

// Lazy-init heat array sized to the current map's area count.
void autoScout_initHeatArray()
{
   if (gAutoScout_heatInited == true) { return; }
   int areaCount = kbAreaGetNumber();
   for (int i = 0; i < areaCount; i = i + 1)
   {
      gAutoScout_heat.add(0.0);
   }
   gAutoScout_heatInited = true;
}

// True if the given cActionType integer is an attack-style action.
//
// Broad whitelist (revisited 2026-05-14 after the narrow {4, 10} version
// missed every military threat in a playtest). Empirically, kbProtoUnit-
// GetActionIDs returns the generic "Attack" type (15) for many protos --
// villagers, soldiers, towers, town centres -- even when the proto's XML
// uses a specific action name like HandAttack or RangedAttack. The engine
// categorises broadly internally, so we must accept type 15 to catch those
// threats. Side effect: kbProtoUnitGetActionStatFloat will emit a "provided
// action: Attack, doesn't exist for protoUnitID N" engine warning when
// the proto's XML doesn't literally name an action "Attack" -- harmless
// (stat returns 0, our `if (rof <= 0) continue` skips), just noisy in the
// log. Specialty types (BombardAttack=39, Bombard=56, etc.) are kept for
// siege coverage even though they're less common.
bool autoScout_isAttackActionType(int actionType = -1)
{
   if (actionType == 4)  { return(true); }   // HandAttack
   if (actionType == 10) { return(true); }   // RangedAttack
   if (actionType == 15) { return(true); }   // Attack (generic; needed for most threats)
   if (actionType == 39) { return(true); }   // BombardAttack
   if (actionType == 40) { return(true); }   // BroadsideAttack
   if (actionType == 56) { return(true); }   // Bombard
   if (actionType == 64) { return(true); }   // TruckAttack
   if (actionType == 68) { return(true); }   // StunAttack
   if (actionType == 79) { return(true); }   // RoundelAttack
   if (actionType == 80) { return(true); }   // RoundelMultiAttack
   return(false);
}

// Lazy lookup for the AbstractVillager and Building unit-type IDs. Cached
// after first call; -1 if the lookup failed.
extern int gAutoScout_typeVillager = -2;
extern int gAutoScout_typeBuilding = -2;
int autoScout_getVillagerType()
{
   if (gAutoScout_typeVillager == -2)
   {
      gAutoScout_typeVillager = kbGetUnitTypeID("AbstractVillager");
   }
   return(gAutoScout_typeVillager);
}
int autoScout_getBuildingType()
{
   if (gAutoScout_typeBuilding == -2)
   {
      gAutoScout_typeBuilding = kbGetUnitTypeID("Building");
   }
   return(gAutoScout_typeBuilding);
}

// Hardcoded projectile-count lookup for known multi-projectile protos. The
// underlying <displayednumberprojectiles> XML field is not exposed by any
// of the kbProtoUnitGetActionStat* APIs (Float and Int variants both reject
// stat IDs > 3; the engine's "Valid range: 0 - 17" message was misleading,
// real valid range is 0..3 and none of those slots contain the projectile
// count). Without this multiplier, TC contribution underestimates by 2-3x
// and the heat-flood radius around enemy bases is too small to repel
// scouts -- the critical mod goal.
//
// Limited to actionName == "RangedAttack" because per-shot projectile counts
// in proto.xml are only attached to ranged-attack action entries (SentryTower
// also has a HandAttack at range 4 with no projectiles; we must not multiply
// that). Specialty attacks (BombardAttack on Cheiroballista, etc.) accept
// the single-shot underestimate -- siege is rarely a scout threat.
//
// List of 37 multi-projectile protos extracted from
// extracted/gameplay/proto.xml as of 2026-05-14.
int autoScout_getProtoProjectiles(int proto = -1, string actionName = "")
{
   if (actionName != "RangedAttack") { return(1); }
   string name = kbProtoUnitGetName(proto);

   // Buildings (Town Centres, fortresses, towers). The critical early-game
   // protos for the "scouts avoid enemy TC" goal are in this group.
   if (name == "TownCenter")          { return(2); }
   if (name == "TownCenterAbandoned") { return(2); }
   if (name == "VillageCenter")       { return(2); }
   if (name == "CitadelCenter")       { return(3); }
   if (name == "Castle")              { return(3); }
   if (name == "Palace")              { return(3); }
   if (name == "Fortress")            { return(3); }
   if (name == "MigdolStronghold")    { return(3); }
   if (name == "GreatTemple")         { return(3); }
   if (name == "HillFort")            { return(3); }
   if (name == "AsgardianHillFort")   { return(3); }
   if (name == "Baolei")              { return(3); }
   if (name == "SentryTower")             { return(2); }
   if (name == "MilitaryCampTower")       { return(2); }
   if (name == "MachineWorkshopTower")    { return(2); }
   if (name == "CalpulliLumberOutpost")   { return(2); }

   // Myth units / heroes (later-game; included for completeness).
   if (name == "BaiHu")               { return(5); }
   if (name == "ZhuQue")              { return(5); }
   if (name == "Fafnir")              { return(6); }
   if (name == "FafnirBoss")          { return(6); }
   if (name == "UFO")                 { return(6); }
   if (name == "FireGiant")           { return(2); }
   if (name == "FireKing")            { return(2); }
   if (name == "FireKingSPC")         { return(2); }
   if (name == "YingLong")            { return(2); }
   if (name == "Kamaitachi")          { return(3); }
   if (name == "ScorpionMan")         { return(3); }
   if (name == "Tzitzimitl")          { return(3); }
   if (name == "TeixiptlaTezca")          { return(4); }
   if (name == "SuperTeixiptlaTezca")     { return(4); }
   if (name == "CheatSuperTeixiptlaTezca"){ return(4); }
   if (name == "Tezcatlipoca")            { return(4); }
   if (name == "Cheiroballista")          { return(4); }
   if (name == "Junkozosen")              { return(4); }
   if (name == "Shinobi")                 { return(3); }
   if (name == "ChuKoNu")                 { return(4); }
   return(1);
}

// Find or append a cache row for the given proto. Computes isThreat,
// contribution, isVillager, isBuilding on first encounter.
int autoScout_protoCacheRow(int proto = -1)
{
   if (proto < 0) { return(-1); }
   int n = gAutoScout_protoThreatID.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_protoThreatID[i] == proto) { return(i); }
   }

   // First encounter: append, then compute.
   int slot = n;
   gAutoScout_protoThreatID.add(proto);
   gAutoScout_protoThreatIsT.add(0);
   gAutoScout_protoThreatContrib.add(0.0);
   gAutoScout_protoThreatRange.add(0.0);
   gAutoScout_protoThreatIsVill.add(0);
   gAutoScout_protoThreatIsBld.add(0);

   // Villager / Building flags.
   int vType = autoScout_getVillagerType();
   if (vType >= 0 && kbProtoUnitIsType(proto, vType) == true)
   {
      gAutoScout_protoThreatIsVill[slot] = 1;
   }
   int bType = autoScout_getBuildingType();
   if (bType >= 0 && kbProtoUnitIsType(proto, bType) == true)
   {
      gAutoScout_protoThreatIsBld[slot] = 1;
   }

   // Threat profile: iterate attack actions by NAME and pick max contribution
   // (formula: contrib = DPS * (1 + range / cAutoScout_HeatRangeDivisor)).
   //
   // Why we don't go through kbProtoUnitGetActionIDs: empirically, that call
   // returns the generic "Attack" action type (15) for protos whose XML names
   // its attacks "RangedAttack" or "HandAttack" -- SentryTower, CitadelCenter,
   // Berserk all reported only type 15 in playtest 2026-05-14, even though
   // their XML uses the specific names. kbActionGetName(15) returns "Attack",
   // and kbProtoUnitGetActionStatFloat(proto, "Attack", ...) then fails with
   // "action doesn't exist for protoUnitID" because the proto literally has
   // no action named "Attack". Result: every threat proto profiled as
   // isThreat=0 and the heat-map saw zero kb-tracked threats.
   //
   // Fix: iterate the known XML action-name list directly. Each name we try
   // either returns valid stats (rof > 0, used) or returns 0 (skipped). The
   // engine warns when an action doesn't exist for the proto, but the warning
   // is cosmetic -- correct data still flows through. Names cover the common
   // schema entries from extracted/gameplay/proto.xml.
   bool isThreat = false;
   float bestContrib = 0.0;
   float bestRange = 0.0;
   string[] attackNames = new string(10, "");
   attackNames[0] = "HandAttack";
   attackNames[1] = "RangedAttack";
   attackNames[2] = "Attack";
   attackNames[3] = "BombardAttack";
   attackNames[4] = "BroadsideAttack";
   attackNames[5] = "Bombard";
   attackNames[6] = "TruckAttack";
   attackNames[7] = "StunAttack";
   attackNames[8] = "RoundelAttack";
   attackNames[9] = "RoundelMultiAttack";
   int aN = attackNames.size();
   for (int a = 0; a < aN; a = a + 1)
   {
      string actionName = attackNames[a];
      if (actionName == "") { continue; }

      float rof = kbProtoUnitGetActionStatFloat(cMyID, proto, actionName, 0);
      if (rof <= 0.0) { continue; }
      float range = kbProtoUnitGetActionMaximumRange(cMyID, proto, actionName, -1);
      if (range < 0.0) { range = 0.0; }

      float dmgHack   = kbProtoUnitGetActionDamageForType(cMyID, proto, actionName, 0);
      float dmgPierce = kbProtoUnitGetActionDamageForType(cMyID, proto, actionName, 1);
      float dmgCrush  = kbProtoUnitGetActionDamageForType(cMyID, proto, actionName, 2);
      float dmgDivine = kbProtoUnitGetActionDamageForType(cMyID, proto, actionName, 3);
      float dmgRaw  = dmgHack + dmgPierce + dmgCrush + dmgDivine;
      if (dmgRaw <= 0.0) { continue; }

      // Multi-projectile damage multiplier. See autoScout_getProtoProjectiles
      // for why this is hardcoded (the engine doesn't expose
      // displayednumberprojectiles through kbProtoUnitGetActionStatFloat;
      // probe of stats 3..17 was all -1 or unrelated values).
      int numProj = autoScout_getProtoProjectiles(proto, actionName);
      if (numProj < 1) { numProj = 1; }
      float dmgTotal = dmgRaw * numProj;

      float dps = dmgTotal / rof;
      // Per-action verification echo so we can confirm each KB reading.
      // Logged once per unique proto x action since protoCacheRow is lazy.
      aiEcho("autoScout: profile proto=" + proto + " (" + kbProtoUnitGetName(proto)
         + ") action=" + actionName
         + " rof=" + rof
         + " range=" + range
         + " dmgRaw=" + dmgRaw
         + " (H=" + dmgHack + " P=" + dmgPierce + " C=" + dmgCrush + " D=" + dmgDivine + ")"
         + " numProj=" + numProj
         + " dmgTotal=" + dmgTotal
         + " dps=" + dps);
      if (dps <= cAutoScout_HeatMinDPS) { continue; }

      isThreat = true;
      float contrib = dps * (1.0 + range / cAutoScout_HeatRangeDivisor);
      if (contrib > bestContrib)
      {
         bestContrib = contrib;
         bestRange   = range;
      }
   }
   if (isThreat == true)
   {
      gAutoScout_protoThreatIsT[slot]      = 1;
      gAutoScout_protoThreatContrib[slot] = bestContrib;
      gAutoScout_protoThreatRange[slot]   = bestRange;
      aiEcho("autoScout: PROFILE result proto=" + proto + " (" + kbProtoUnitGetName(proto)
         + ") isThreat=1 bestContrib=" + bestContrib
         + " bestRange=" + bestRange);
   }
   else
   {
      aiEcho("autoScout: PROFILE result proto=" + proto + " (" + kbProtoUnitGetName(proto)
         + ") isThreat=0 (no qualifying attack actions)");
   }
   return(slot);
}

// Per-unit last-seen-visible bookkeeping. Returns the row index (lazily
// appending if absent). lastSeenMs is updated by autoScout_updateHeatMap.
int autoScout_unitLastSeenRow(int unitID = -1)
{
   if (unitID < 0) { return(-1); }
   int n = gAutoScout_unitLastSeenID.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_unitLastSeenID[i] == unitID) { return(i); }
   }
   int slot = n;
   gAutoScout_unitLastSeenID.add(unitID);
   gAutoScout_unitLastSeenMs.add(-1);
   return(slot);
}

// Initialise the cached enemy-threat queries (live mobile units + buildings).
// Two separate queries instead of one cUnitTypeAll. cUnitTypeAll returns
// trees / gold mines / relics / columns / berries as well -- ~1000 units per
// tick that we then have to process and discard. cUnitTypeUnit (889) alone
// EXCLUDES buildings so the heat-map would miss enemy TCs and towers. The
// split: one query for live mobile units, one for buildings. Both share the
// same downstream processing in autoScout_updateHeatMap.
//
// Visibility filter is left at the engine default (cUnitQuerySeeableStateAllValid
// = -1, no filtering) for both queries. Adding cUnitQueryVisibleStateRecent-
// PositionKnown (=4) was found to filter out everything in a previous playtest --
// the downstream fog timer + isOnMap check already gate stale positions.
void autoScout_initThreatQuery()
{
   if (gAutoScout_threatUnitQuery < 0)
   {
      gAutoScout_threatUnitQuery = kbUnitQueryCreate("autoScout_threats_units");
      kbUnitQuerySetPlayerRelation(gAutoScout_threatUnitQuery, cPlayerRelationEnemy, false);
      kbUnitQuerySetState(gAutoScout_threatUnitQuery, cUnitStateAlive);
      kbUnitQuerySetUnitType(gAutoScout_threatUnitQuery, cUnitTypeMilitaryUnit);
   }
   if (gAutoScout_threatBuildingQuery < 0)
   {
      gAutoScout_threatBuildingQuery = kbUnitQueryCreate("autoScout_threats_buildings");
      kbUnitQuerySetPlayerRelation(gAutoScout_threatBuildingQuery, cPlayerRelationEnemy, false);
      kbUnitQuerySetState(gAutoScout_threatBuildingQuery, cUnitStateAlive);
      kbUnitQuerySetUnitType(gAutoScout_threatBuildingQuery, cUnitTypeBuilding);
   }
}

// Flood-fill heat from an entity's position outward through the area graph.
// Areas receive (and propagate from) only while
//   dist(entityPos, kbAreaGetCenter(area)) <= max(entityRange + AreaPad, MinFloodReach).
// Heat applied per area: magnitude * max(0, 1 - dist / (maxDist + AreaPad)).
// The +AreaPad bumps (cAutoScout_HeatAreaPad, ~average area radius) reflect
// the fact that the centroid is a single point representing a region; an
// area whose centroid is just outside the threat's bare range may still
// contain tiles within range, and conversely a steep "1 - dist/range"
// falloff at the boundary is too sharp for the centroid-as-stand-in model.
void autoScout_addHeatFlood(vector entityPos = cInvalidVector, float entityRange = 0.0, float magnitude = 0.0)
{
   if (magnitude <= 0.0) { return; }
   if (autoScout_isOnMap(entityPos) == false) { return; }
   int startArea = kbAreaGetIDByPosition(entityPos);
   if (startArea < 0 || startArea >= gAutoScout_heat.size()) { return; }

   float maxDist = entityRange + cAutoScout_HeatAreaPad;
   if (maxDist < cAutoScout_HeatMinFloodReach) { maxDist = cAutoScout_HeatMinFloodReach; }
   float falloffDenom = maxDist + cAutoScout_HeatAreaPad;

   int areaCount = gAutoScout_heat.size();
   int[] visited = new int(areaCount, 0);
   int[] queue   = new int(0, 0);
   queue.add(startArea);
   visited[startArea] = 1;

   int head = 0;
   while (head < queue.size())
   {
      int area = queue[head];
      head = head + 1;

      vector areaPos = kbAreaGetCenter(area);
      float dist = xsVectorDistanceXZ(entityPos, areaPos);
      if (dist > maxDist) { continue; }  // skip + don't propagate

      float weight = 1.0 - dist / falloffDenom;
      if (weight > 0.0)
      {
         gAutoScout_heat[area] = gAutoScout_heat[area] + magnitude * weight;
      }

      int borderCount = kbAreaGetNumberBorderAreas(area);
      for (int b = 0; b < borderCount; b = b + 1)
      {
         int nbr = kbAreaGetBorderAreaID(area, b);
         if (nbr < 0 || nbr >= areaCount) { continue; }
         if (visited[nbr] == 1) { continue; }
         visited[nbr] = 1;
         queue.add(nbr);
      }
   }
}

// Append a damage/death heat event. Active for durationMs from now; applied
// each tick on top of the per-tick base heat via the flood-fill helper.
void autoScout_addHeatEvent(vector pos = cInvalidVector, float range = 0.0,
                            float magnitude = 0.0, int durationMs = 0)
{
   if (magnitude <= 0.0) { return; }
   if (durationMs <= 0) { return; }
   if (range <= 0.0) { return; }
   if (autoScout_isOnMap(pos) == false) { return; }
   int area = kbAreaGetIDByPosition(pos);
   if (area < 0) { return; }
   gAutoScout_eventPos.add(pos);
   gAutoScout_eventRange.add(range);
   gAutoScout_eventMagnitude.add(magnitude);
   gAutoScout_eventExpiryMs.add(xsGetTimeMS() + durationMs);
   gAutoScout_eventArea.add(area);
}

// Per-scout damage detection: compare current HP to last-known. If lower,
// emit a damage heat event at the scout's current area. Also keep the cached
// lastPos fresh so dropFromPool's death-event helper has an on-map position
// to attribute the kill to.
void autoScout_trackHPAndPos(int slot = -1, int unitID = -1)
{
   if (slot < 0 || unitID < 0) { return; }
   if (slot >= gAutoScout_lastHP.size()) { return; }

   float currentHP = kbUnitGetStatFloat(unitID, cUnitStatCurrHP);
   vector currentPos = kbUnitGetPosition(unitID);

   float lastHP = gAutoScout_lastHP[slot];
   if (currentHP < lastHP && autoScout_isOnMap(currentPos) == true)
   {
      autoScout_addHeatEvent(currentPos,
         cAutoScout_HeatDamageEventRange,
         cAutoScout_HeatDamageEventMagnitude,
         cAutoScout_HeatDamageEventDurationMs);
      int dmgArea = kbAreaGetIDByPosition(currentPos);
      aiEcho("autoScout: DAMAGE event slot=" + slot + " unit=" + unitID
         + " area=" + dmgArea + " hp=" + currentHP + "/" + lastHP);
   }

   gAutoScout_lastHP[slot] = currentHP;
   if (autoScout_isOnMap(currentPos) == true)
   {
      gAutoScout_lastPos[slot] = currentPos;
   }
}

// Rebuild heat from scratch this tick. Reset + ADD across observations =
// volatile: heat reflects currently-relevant threats every tick. Events
// (damage/death) add their magnitude on top until they expire.
void autoScout_updateHeatMap()
{
   autoScout_initHeatArray();
   autoScout_initThreatQuery();

   // Full reset.
   int areaCount = gAutoScout_heat.size();
   for (int i = 0; i < areaCount; i = i + 1)
   {
      gAutoScout_heat[i] = 0.0;
   }

   int now = xsGetTimeMS();

   // Per-tick filter accounting for the diagnostic at the bottom of this
   // function. Helps narrow down where heat-map data is being lost. Counters
   // are aggregated across BOTH the unit query and the building query.
   int diagQueryN     = 0;
   int diagRejInvalid = 0;
   int diagRejVill    = 0;
   int diagRejNonT    = 0;
   int diagRejFogMob  = 0;
   int diagRejOffMap  = 0;
   int diagContrib    = 0;

   // Visible-threat layer: process both queries with the same per-unit logic.
   for (int q = 0; q < 2; q = q + 1)
   {
      int queryID = gAutoScout_threatUnitQuery;
      if (q == 1) { queryID = gAutoScout_threatBuildingQuery; }
      if (queryID < 0) { continue; }
      kbUnitQueryResetResults(queryID);
      int n = kbUnitQueryExecute(queryID);
      diagQueryN = diagQueryN + n;
      for (int i = 0; i < n; i = i + 1)
      {
         int unitID = kbUnitQueryGetResult(queryID, i);
         if (unitID < 0) { diagRejInvalid = diagRejInvalid + 1; continue; }
         if (kbUnitGetIsIDValid(unitID) == false) { diagRejInvalid = diagRejInvalid + 1; continue; }
         int proto = kbUnitGetProtoUnitID(unitID);
         int cacheRow = autoScout_protoCacheRow(proto);
         if (cacheRow < 0) { diagRejInvalid = diagRejInvalid + 1; continue; }

         // Skip villagers entirely.
         if (gAutoScout_protoThreatIsVill[cacheRow] == 1) { diagRejVill = diagRejVill + 1; continue; }
         if (gAutoScout_protoThreatIsT[cacheRow] != 1)    { diagRejNonT = diagRejNonT + 1; continue; }

         bool visible = kbUnitVisible(unitID);
         bool isBuilding = (gAutoScout_protoThreatIsBld[cacheRow] == 1);

         // Track per-unit last-seen-visible. Mobile units get a fog timeout;
         // buildings get an indefinite kb-tracked position (kb only forgets
         // them if we never saw them, which is fine).
         int seenRow = autoScout_unitLastSeenRow(unitID);
         if (visible == true && seenRow >= 0)
         {
            gAutoScout_unitLastSeenMs[seenRow] = now;
         }
         if (visible == false && isBuilding == false)
         {
            int lastSeen = -1;
            if (seenRow >= 0) { lastSeen = gAutoScout_unitLastSeenMs[seenRow]; }
            if (lastSeen < 0) { diagRejFogMob = diagRejFogMob + 1; continue; }  // never seen visible
            if (now - lastSeen > cAutoScout_HeatMobileFogTimeoutMs)
            {
               diagRejFogMob = diagRejFogMob + 1;
               continue;  // fog timer expired
            }
         }

         vector unitPos = kbUnitGetPosition(unitID);
         if (autoScout_isOnMap(unitPos) == false) { diagRejOffMap = diagRejOffMap + 1; continue; }
         autoScout_addHeatFlood(unitPos,
            gAutoScout_protoThreatRange[cacheRow],
            gAutoScout_protoThreatContrib[cacheRow]);
         diagContrib = diagContrib + 1;
      }
   }

   // Damage/death event layer. Applied on top until expired, using the same
   // flood-fill with the event's stored position + range.
   int activeEvents = 0;
   int en = gAutoScout_eventExpiryMs.size();
   for (int e = 0; e < en; e = e + 1)
   {
      if (gAutoScout_eventExpiryMs[e] <= now) { continue; }  // expired
      autoScout_addHeatFlood(gAutoScout_eventPos[e],
         gAutoScout_eventRange[e],
         gAutoScout_eventMagnitude[e]);
      activeEvents = activeEvents + 1;
   }

   // Throttled per-tick diagnostic. Log every 5s (counter % 5) OR any tick
   // where the threat query returned anything OR any tick with an active
   // damage/death event. Silent ticks when no enemies are in kb avoid log
   // spam but the modulus floor confirms the heat-map is still ticking.
   gAutoScout_heatDiagTickCounter = gAutoScout_heatDiagTickCounter + 1;
   bool logThisTick = false;
   if (diagQueryN > 0)              { logThisTick = true; }
   if (activeEvents > 0)            { logThisTick = true; }
   if (gAutoScout_heatDiagTickCounter % 5 == 0) { logThisTick = true; }
   if (logThisTick == true)
   {
      aiEcho("autoScout: heat tick query=" + diagQueryN
         + " contrib=" + diagContrib
         + " rej[invalid=" + diagRejInvalid
         + " vill=" + diagRejVill
         + " nonThreat=" + diagRejNonT
         + " fogMob=" + diagRejFogMob
         + " offMap=" + diagRejOffMap + "]"
         + " events=" + activeEvents);
   }
}

//------------------------------------------------------------------------------
// Danger / blacklist helpers (2026-05-13)
//------------------------------------------------------------------------------

// Effective danger of an area, blending baseline (for unexplored portions
// where we have no ground-truth) with our heat-map (for explored portions
// where it reflects threats we've observed):
//
//   explore_percent = 1.0 - blackTiles / totalTiles
//   danger = baseline * (1 - explore_percent) + heat[area] * explore_percent
//
// Fully unexplored area -> baseline (presumed safe; scouts will go there).
// Fully explored area -> heat[area] = sum of nearby observed threats (towers,
// military with DPS > cAutoScout_HeatMinDPS). Mixed areas blend linearly.
float autoScout_effectiveDanger(int areaID = -1)
{
   if (areaID < 0) { return(cAutoScout_DangerBaseline); }
   if (kbAreaGetIsIDValid(areaID) == false) { return(cAutoScout_DangerBaseline); }
   int total = kbAreaGetNumberTiles(areaID);
   if (total <= 0) { return(cAutoScout_DangerBaseline); }
   int black = kbAreaGetNumberBlackTiles(areaID);
   float blackFrac = 1.0 * black / total;
   if (blackFrac < 0.0) { blackFrac = 0.0; }
   if (blackFrac > 1.0) { blackFrac = 1.0; }
   float explorePercent = 1.0 - blackFrac;
   float heatDanger = 0.0;
   if (gAutoScout_heatInited == true && areaID < gAutoScout_heat.size())
   {
      heatDanger = gAutoScout_heat[areaID];
   }
   return(cAutoScout_DangerBaseline * (1.0 - explorePercent)
        + heatDanger * explorePercent);
}

// Wrapper used by hard-skip / path-aware-block / per-tick flee trigger.
bool autoScout_areaIsDangerous(int areaID = -1)
{
   return(autoScout_effectiveDanger(areaID) > cAutoScout_DangerHardSkip);
}

// Add areaID to the blacklist (or bump its expiry if already present).
void autoScout_blacklistArea(int areaID = -1)
{
   if (areaID < 0) { return; }
   int newExpiry = xsGetTimeMS() + cAutoScout_BlacklistDurationMs;
   int n = gAutoScout_blacklistedAreaIDs.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_blacklistedAreaIDs[i] == areaID)
      {
         gAutoScout_blacklistedExpiryMs[i] = newExpiry;
         return;
      }
   }
   gAutoScout_blacklistedAreaIDs.add(areaID);
   gAutoScout_blacklistedExpiryMs.add(newExpiry);
}

// True if areaID is blacklisted AND its expiry has not yet passed. Expired
// entries are left in place; they get bumped naturally on re-blacklist.
bool autoScout_isAreaBlacklisted(int areaID = -1)
{
   if (areaID < 0) { return(false); }
   int now = xsGetTimeMS();
   int n = gAutoScout_blacklistedAreaIDs.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_blacklistedAreaIDs[i] == areaID)
      {
         return(gAutoScout_blacklistedExpiryMs[i] > now);
      }
   }
   return(false);
}

// Picks a safe flee area for unitID fleeing dangerAreaID. Algorithm:
//   1. Flood-fill BFS from the SCOUT's current area up to
//      cAutoScout_FleeBfsDepth hops. (Earlier versions seeded BFS from a
//      polar-projected "away from danger" point; that occasionally placed
//      the seed inside another danger pocket and the BFS then picked an
//      area whose path crossed it, killing the scout. Starting from the
//      scout's own area eliminates that failure mode.)
//   2. Path-aware: don't propagate BFS through dangerous areas, so the
//      chosen target is reachable via a safe corridor. Blacklisted areas
//      are NOT blocked (blacklisted != dangerous; blocking through them
//      could trap the scout).
//   3. Score every reachable area by:
//        safety       (cAutoScout_FleeWeightSafety)     -- low danger
//        away-danger  (cAutoScout_FleeWeightAwayDanger) -- far from dangerCenter
//        near-scout   (cAutoScout_FleeWeightNearScout)  -- close to scoutPos
//      Safety dominates; away-danger biases direction; near-scout discourages
//      cross-map flee targets.
//   4. Return the best-scoring area's ID, or -1 if no reachable area was
//      found (caller issues no move, scout holds in FLEEING for the timer).
//
// Danger is read via autoScout_effectiveDanger, which blends baseline (for
// unexplored portions) with our per-tick heat-map (for explored portions).
// The heat-map's 1-hop spread covers adjacent-danger leakage so we don't
// need averageInBorderAreas-style smoothing inside this BFS.
int autoScout_findFleeArea(int scoutUnitID = -1, int dangerAreaID = -1)
{
   if (scoutUnitID < 0) { return(-1); }
   if (dangerAreaID < 0) { return(-1); }

   vector scoutPos = kbUnitGetPosition(scoutUnitID);
   if (autoScout_isOnMap(scoutPos) == false) { return(-1); }
   int startArea = kbAreaGetIDByPosition(scoutPos);
   if (startArea < 0) { return(-1); }

   vector dangerCenter = kbAreaGetCenter(dangerAreaID);
   int unitProto       = kbUnitGetProtoUnitID(scoutUnitID);
   float mapX          = kbGetMapXSize();
   float mapZ          = kbGetMapZSize();
   float mapDiag       = sqrt(mapX * mapX + mapZ * mapZ);
   if (mapDiag < 1.0) { mapDiag = 1.0; }

   int areaCount = kbAreaGetNumber();
   int[] visited    = new int(areaCount, 0);
   int[] queue      = new int(0, 0);
   int[] queueDepth = new int(0, 0);
   queue.add(startArea);
   queueDepth.add(0);
   visited[startArea] = 1;

   int bestArea = -1;
   float bestScore = -1.0e18;

   int head = 0;
   while (head < queue.size())
   {
      int areaID = queue[head];
      int depth  = queueDepth[head];
      head = head + 1;

      // Score every reachable area beyond the start area. Skip depth==0
      // because "stand still" is not a useful flee outcome -- if we have to
      // pick our own area we should at least try to move within it.
      if (depth >= 1 && kbAreaGetIsIDValid(areaID) == true)
      {
         vector areaPos = kbAreaGetCenter(areaID);
         if (autoScout_isOnMap(areaPos) == true)
         {
            if (kbCanPath(scoutPos, areaPos, unitProto, 1.0, -1) == true)
            {
               // Safety: low danger = high score, dominant component.
               float danger = autoScout_effectiveDanger(areaID);
               float dRatio = danger / cAutoScout_DangerHardSkip;
               if (dRatio < 0.0) { dRatio = 0.0; }
               if (dRatio > 1.0) { dRatio = 1.0; }
               float safetyScore = 1.0 - dRatio;

               // Away from the area we fled from: far = high score.
               float distFromDanger = xsVectorDistanceXZ(dangerCenter, areaPos);
               float awayScore = distFromDanger / mapDiag;
               if (awayScore < 0.0) { awayScore = 0.0; }
               if (awayScore > 1.0) { awayScore = 1.0; }

               // Near scout's current position: close = high score.
               float distFromScout = xsVectorDistanceXZ(scoutPos, areaPos);
               float nearScoutScore = 1.0 - distFromScout / mapDiag;
               if (nearScoutScore < 0.0) { nearScoutScore = 0.0; }
               if (nearScoutScore > 1.0) { nearScoutScore = 1.0; }

               float score = cAutoScout_FleeWeightSafety     * safetyScore
                           + cAutoScout_FleeWeightAwayDanger * awayScore
                           + cAutoScout_FleeWeightNearScout  * nearScoutScore;
               if (score > bestScore)
               {
                  bestScore = score;
                  bestArea  = areaID;
               }
            }
         }
      }

      // Path-aware expansion (P1): don't propagate BFS through dangerous
      // areas, so the chosen flee target is reachable via a safe corridor.
      // Start area (depth 0) is the scout's current position -- always
      // expand from there even if it's the area we're fleeing FROM, so
      // we can find safe neighbors. Blacklisted areas are NOT blocked
      // here: blacklisted != dangerous (a blacklisted area may be safe
      // now), and blocking expansion through them could trap the scout.
      bool blockExpand = false;
      if (depth >= 1 && autoScout_areaIsDangerous(areaID) == true)
      {
         blockExpand = true;
      }
      if (depth >= cAutoScout_FleeBfsDepth) { blockExpand = true; }

      if (blockExpand == false)
      {
         int n = kbAreaGetNumberBorderAreas(areaID);
         for (int j = 0; j < n; j = j + 1)
         {
            int next = kbAreaGetBorderAreaID(areaID, j);
            if (next < 0 || next >= areaCount) { continue; }
            if (visited[next] == 1) { continue; }
            visited[next] = 1;
            queue.add(next);
            queueDepth.add(depth + 1);
         }
      }
   }

   return(bestArea);
}

// Transition slot/unit to FLEEING. Picks a flee target via autoScout_findFleeArea
// (safety-weighted BFS from a polar-projected seed), issues an aiTaskMoveUnit
// to the chosen area's centroid, releases area claim, sets a 5-second hold
// timer. The handler keeps the scout in FLEEING until the timer expires
// regardless of arrival; if no flee area was found, no move is issued and the
// scout simply holds in place until the timer expires.
void autoScout_enterFleeing(int slot = -1, int unitID = -1, int dangerAreaID = -1)
{
   if (slot < 0) { return; }
   if (unitID < 0) { return; }

   // Clean up Diverting bookkeeping if we flee mid-divert. attemptedHerdIDs
   // already records the herd; we don't re-attempt it, matching the
   // "attempt once globally" rule.
   if (gAutoScout_state[slot] == cAutoScoutState_Diverting)
   {
      gAutoScout_targetHerdID[slot] = -1;
   }

   int fleeArea = autoScout_findFleeArea(unitID, dangerAreaID);

   vector dest = kbUnitGetPosition(unitID);
   float destDanger = -1.0;
   bool issuedMove = false;
   if (fleeArea >= 0)
   {
      dest = kbAreaGetCenter(fleeArea);
      destDanger = autoScout_effectiveDanger(fleeArea);
      aiTaskMoveUnit(unitID, dest, false, false);
      issuedMove = true;
   }

   autoScout_releaseClaim(slot);
   gAutoScout_state[slot]        = cAutoScoutState_Fleeing;
   gAutoScout_fleeFromArea[slot] = dangerAreaID;
   gAutoScout_fleeUntilMs[slot]  = xsGetTimeMS() + cAutoScout_FleeMinDurationMs;
   gAutoScout_stuckTicks[slot]   = 0;

   aiEcho("autoScout: FLEE slot=" + slot + " unit=" + unitID
      + " fromArea=" + dangerAreaID + " toArea=" + fleeArea
      + " dest=" + dest + " destDanger=" + destDanger
      + " issuedMove=" + issuedMove);
}

//------------------------------------------------------------------------------
// Herd diversion helpers
//------------------------------------------------------------------------------

bool autoScout_isHerdAttempted(int herdID = -1)
{
   if (herdID < 0) { return(false); }
   int n = gAutoScout_attemptedHerdIDs.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_attemptedHerdIDs[i] == herdID) { return(true); }
   }
   return(false);
}

bool autoScout_isHerdRedirected(int herdID = -1)
{
   if (herdID < 0) { return(false); }
   int n = gAutoScout_redirectedHerdIDs.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_redirectedHerdIDs[i] == herdID) { return(true); }
   }
   return(false);
}

// Lazily resolves "LogicalTypeConvertsHerds" -> unit-type ID. AoMR's stock AI
// scripts don't reference a cUnitTypeLogicalTypeConvertsHerds named constant,
// so we look up the type ID by its proto.xml string name.
int autoScout_getConvertsHerdsType()
{
   if (gAutoScout_typeConvertsHerds < 0)
   {
      gAutoScout_typeConvertsHerds = kbGetUnitTypeID("LogicalTypeConvertsHerds");
   }
   return(gAutoScout_typeConvertsHerds);
}

int autoScout_getAbstractTCType()
{
   if (gAutoScout_typeAbstractTC < 0)
   {
      gAutoScout_typeAbstractTC = kbGetUnitTypeID("AbstractTownCenter");
   }
   return(gAutoScout_typeAbstractTC);
}

void autoScout_initHerdQuery()
{
   if (gAutoScout_herdQuery >= 0) { return; }
   gAutoScout_herdQuery = kbUnitQueryCreate("autoScout_herds");
   // cPlayerRelationEnemy includes Gaia (cPlayerRelationEnemyNotGaia exists as
   // the explicit-exclude variant), so this covers the "Gaia + enemies" target
   // set we want for divert candidates.
   kbUnitQuerySetPlayerRelation(gAutoScout_herdQuery, cPlayerRelationEnemy, false);
   kbUnitQuerySetUnitType(gAutoScout_herdQuery, cUnitTypeHerdable);
   kbUnitQuerySetState(gAutoScout_herdQuery, cUnitStateAlive);
   kbUnitQuerySetAscendingSort(gAutoScout_herdQuery, true);
}

void autoScout_initOwnedHerdQuery()
{
   if (gAutoScout_ownedHerdQuery >= 0) { return; }
   gAutoScout_ownedHerdQuery = kbUnitQueryCreate("autoScout_ownedHerds");
   kbUnitQuerySetPlayerID(gAutoScout_ownedHerdQuery, cMyID, false);
   kbUnitQuerySetUnitType(gAutoScout_ownedHerdQuery, cUnitTypeHerdable);
   kbUnitQuerySetState(gAutoScout_ownedHerdQuery, cUnitStateAlive);
}

void autoScout_initNearestTCQuery()
{
   if (gAutoScout_nearestTCQuery >= 0) { return; }
   int tcType = autoScout_getAbstractTCType();
   if (tcType < 0) { return; }
   gAutoScout_nearestTCQuery = kbUnitQueryCreate("autoScout_nearestTC");
   kbUnitQuerySetPlayerID(gAutoScout_nearestTCQuery, cMyID, false);
   kbUnitQuerySetUnitType(gAutoScout_nearestTCQuery, tcType);
   kbUnitQuerySetState(gAutoScout_nearestTCQuery, cUnitStateAlive);
}

// Cheap existence check using the cached TC query. Used by homeMoveScan to
// short-circuit when we own no TC, avoiding the per-tick owned-herd query.
bool autoScout_anyTCAlive()
{
   autoScout_initNearestTCQuery();
   if (gAutoScout_nearestTCQuery < 0) { return(false); }
   kbUnitQueryResetResults(gAutoScout_nearestTCQuery);
   return(kbUnitQueryExecute(gAutoScout_nearestTCQuery) > 0);
}

// Returns the unit ID of our nearest alive TC to refPos, or -1 if we own no TC.
int autoScout_findNearestTCID(vector refPos = cInvalidVector)
{
   autoScout_initNearestTCQuery();
   if (gAutoScout_nearestTCQuery < 0) { return(-1); }
   kbUnitQueryResetResults(gAutoScout_nearestTCQuery);
   int n = kbUnitQueryExecute(gAutoScout_nearestTCQuery);
   if (n <= 0) { return(-1); }

   int bestID = -1;
   float bestDist = 1.0e18;
   for (int i = 0; i < n; i = i + 1)
   {
      int tcID = kbUnitQueryGetResult(gAutoScout_nearestTCQuery, i);
      if (tcID < 0) { continue; }
      vector tcPos = kbUnitGetPosition(tcID);
      float d = xsVectorDistanceXZ(tcPos, refPos);
      if (d < bestDist) { bestDist = d; bestID = tcID; }
   }
   return(bestID);
}

// Returns ID of the closest reachable, not-yet-attempted, eligible herd
// within (los + cAutoScout_HerdLOSBuffer) of the scout, or -1 if none.
// The buffer catches herds that flicker at the LOS edge between ticks; we
// then per-result drop herds whose tile is pure black (never explored) so
// we don't divert toward unseen positions. Fogged tiles (explored, currently
// dark) are accepted — those are herds we previously saw and remember.
//
// Caller is responsible for checking the scout's LogicalTypeConvertsHerds
// eligibility before calling.
int autoScout_findVisibleHerd(int scoutUnitID = -1, float los = 18.0)
{
   if (scoutUnitID < 0 || los < 1.0) { return(-1); }
   autoScout_initHerdQuery();
   vector pos = kbUnitGetPosition(scoutUnitID);
   kbUnitQuerySetPosition(gAutoScout_herdQuery, pos);
   kbUnitQuerySetMaximumDistance(gAutoScout_herdQuery, los + cAutoScout_HerdLOSBuffer);
   kbUnitQueryResetResults(gAutoScout_herdQuery);
   int n = kbUnitQueryExecute(gAutoScout_herdQuery);
   int unitProto = kbUnitGetProtoUnitID(scoutUnitID);
   for (int i = 0; i < n; i = i + 1)
   {
      int herdID = kbUnitQueryGetResult(gAutoScout_herdQuery, i);
      if (herdID < 0) { continue; }
      if (autoScout_isHerdAttempted(herdID) == true) { continue; }
      vector herdPos = kbUnitGetPosition(herdID);
      bool seen = (kbLocationVisible(herdPos) == true || kbLocationFogged(herdPos) == true);
      if (seen == false) { continue; }
      if (kbCanPath(pos, herdPos, unitProto, 1.0, herdID) == false) { continue; }
      return(herdID);
   }
   return(-1);
}

// Tries to enter Diverting state for slot. Returns true if the scout transitioned to
// Diverting (caller should `return(true)` to short-circuit normal handler logic
// for this tick).
bool autoScout_tryDivert(int slot = -1, int unitID = -1, float los = 18.0)
{
   int unitProto = kbUnitGetProtoUnitID(unitID);
   int convertType = autoScout_getConvertsHerdsType();
   if (convertType < 0) { return(false); }
   if (kbProtoUnitIsType(unitProto, convertType) == false) { return(false); }

   int herdID = autoScout_findVisibleHerd(unitID, los);
   if (herdID < 0) { return(false); }
   if (kbUnitGetIsIDValid(herdID) == false) { return(false); }

   gAutoScout_attemptedHerdIDs.add(herdID);
   gAutoScout_targetHerdID[slot]   = herdID;
   gAutoScout_state[slot]          = cAutoScoutState_Diverting;
   gAutoScout_targetWaypoint[slot] = kbUnitGetPosition(herdID);
   gAutoScout_stuckTicks[slot]     = 0;

   aiEcho("autoScout: DIVERTING unit " + unitID + " to herd " + herdID);
   aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
   return(true);
}

//------------------------------------------------------------------------------
// Oracle helpers — identification, query setup, MaxOracleLOS cache update,
// overlap predicates used by the BFS heuristic.
//------------------------------------------------------------------------------

bool autoScout_isOracle(int unitID = -1)
{
   if (unitID < 0) { return(false); }
   return(kbUnitIsType(unitID, cUnitTypeAbstractOracle));
}

void autoScout_initOracleQuery()
{
   if (gAutoScout_oracleQuery >= 0) { return; }
   gAutoScout_oracleQuery = kbUnitQueryCreate("autoScout_oracles");
   kbUnitQuerySetPlayerID(gAutoScout_oracleQuery, cMyID, false);
   kbUnitQuerySetUnitType(gAutoScout_oracleQuery, cUnitTypeAbstractOracle);
   kbUnitQuerySetState(gAutoScout_oracleQuery, cUnitStateAlive);
}

// Updates gAutoScout_maxOracleLOS only if the given oracle is currently
// saturated AND its current LOS exceeds the cached value. Saturation gating
// prevents mid-growth readings from polluting the cache.
void autoScout_updateMaxOracleLOS(int unitID = -1)
{
   if (unitID < 0) { return; }
   if (kbUnitGetActionType(unitID) != cAutoScout_OracleSaturatedActionType) { return; }
   float current = kbUnitGetStatFloat(unitID, cUnitStatLOS);
   if (current > gAutoScout_maxOracleLOS)
   {
      float prev = gAutoScout_maxOracleLOS;
      gAutoScout_maxOracleLOS = current;
      aiEcho("autoScout: maxOracleLOS cache " + prev + " -> " + current
         + " (from oracle " + unitID + ")");
   }
}

// Returns the pool slot for unitID, or -1 if the unit isn't in the pool.
// Used by the oracle overlap predicates so they can also consider each
// pool-tracked oracle's currently-set target waypoint, not just its
// current physical position.
int autoScout_findSlotForUnit(int unitID = -1)
{
   if (unitID < 0) { return(-1); }
   int n = gAutoScout_unitID.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoScout_unitID[i] == unitID) { return(i); }
   }
   return(-1);
}

// Returns a [0, 1] penalty representing summed overlap of areaPos with all
// our oracles' claim circles, excluding excludeUnitID. Per-oracle distance is
// min(dCurrentPos, dTargetWaypoint) for pool-tracked oracles with a valid
// target; just dCurrentPos otherwise. Per-oracle radius is MaxOracleLOS for
// stationed/pool-tracked oracles and currentLOS for non-toggled oracles seen
// in cActionTypeMove (small, transient claim during player micro). Sums then
// clamps to 1.0. Used as a multiplicative discount in autoScout_areaScore for
// non-oracle source units.
float autoScout_oraclePenalty(vector areaPos = cInvalidVector, int excludeUnitID = -1)
{
   autoScout_initOracleQuery();
   kbUnitQueryResetResults(gAutoScout_oracleQuery);
   int n = kbUnitQueryExecute(gAutoScout_oracleQuery);
   if (n <= 0) { return(0.0); }
   float penalty = 0.0;
   for (int i = 0; i < n; i = i + 1)
   {
      int oracleID = kbUnitQueryGetResult(gAutoScout_oracleQuery, i);
      if (oracleID < 0) { continue; }
      if (oracleID == excludeUnitID) { continue; }

      vector pos = kbUnitGetPosition(oracleID);
      float d = xsVectorDistanceXZ(pos, areaPos);

      int slot = autoScout_findSlotForUnit(oracleID);
      if (slot >= 0 && gAutoScout_targetAreaID[slot] >= 0)
      {
         float dWp = xsVectorDistanceXZ(gAutoScout_targetWaypoint[slot], areaPos);
         if (dWp < d) { d = dWp; }
      }

      float radius = gAutoScout_maxOracleLOS;
      // Small transient claim ONLY for non-pool-tracked oracles caught
      // mid-move. Pool-tracked moving oracles already have their future
      // stationed location accounted for via the target-waypoint min above,
      // so they deserve the full MaxOracleLOS claim there.
      if (slot < 0 && kbUnitGetActionType(oracleID) == cActionTypeMove)
      {
         radius = kbUnitGetStatFloat(oracleID, cUnitStatLOS);
      }
      if (radius < 0.001) { continue; }
      // Pad: regular scouts should keep extra distance from oracle claim edges.
      radius = radius + cAutoScout_OraclePenaltyRadiusPad;
      if (d < radius)
      {
         // Doubled (2026-05-13) -- regular scouts were still picking areas
         // partially inside an oracle's circle even with the original 1x
         // formula. With 2x, even a slight overlap discounts the area more
         // sharply; a single oracle fully overlapping gives penalty=2.0
         // (clamped to 1.0 below), effectively zeroing out the area's score.
         penalty = penalty + 2.0 * (radius - d) / radius;
      }
   }
   if (penalty > 1.0) { penalty = 1.0; }
   return(penalty);
}

// Oracle-source -> other-oracle additive penalty. Replaces the previous
// hard-skip (cAutoScout_OracleExclusionFactor) with a soft term: oracles
// strongly avoid overlapping each other but can still pick an overlapping
// area if no better option exists. Range scales with current maxOracleLOS:
// two oracles each at MaxLOS can still cover non-overlapping ground if the
// distance between them is >= 2 * MaxLOS. Inside that, the penalty ramps
// linearly to 1.0 at exact overlap, and is summed (NOT clamped) so multiple
// nearby oracles compound. Multiplied by cAutoScout_OracleOnOracleWeight at
// the call site to dominate the rest of the score.
float autoScout_oracleOnOraclePenalty(vector areaPos = cInvalidVector, int excludeUnitID = -1)
{
   autoScout_initOracleQuery();
   kbUnitQueryResetResults(gAutoScout_oracleQuery);
   int n = kbUnitQueryExecute(gAutoScout_oracleQuery);
   if (n <= 0) { return(0.0); }
   float radius = cAutoScout_OracleOnOracleRangeFactor * gAutoScout_maxOracleLOS;
   if (radius < 0.001) { return(0.0); }
   float penalty = 0.0;
   for (int i = 0; i < n; i = i + 1)
   {
      int oracleID = kbUnitQueryGetResult(gAutoScout_oracleQuery, i);
      if (oracleID < 0) { continue; }
      if (oracleID == excludeUnitID) { continue; }

      vector pos = kbUnitGetPosition(oracleID);
      float d = xsVectorDistanceXZ(pos, areaPos);

      int slot = autoScout_findSlotForUnit(oracleID);
      if (slot >= 0 && gAutoScout_targetAreaID[slot] >= 0)
      {
         float dWp = xsVectorDistanceXZ(gAutoScout_targetWaypoint[slot], areaPos);
         if (dWp < d) { d = dWp; }
      }

      if (d < radius)
      {
         penalty = penalty + (radius - d) / radius;
      }
   }
   return(penalty);
}

//------------------------------------------------------------------------------
// Candidate area criteria + BFS (closest-to-TC selection)
//------------------------------------------------------------------------------

bool autoScout_areaIsCandidate(int areaID = -1, int scoutUnitID = -1)
{
   if (areaID < 0) { return(false); }
   if (areaID >= gAutoScout_areaClaim.size()) { return(false); }
   gAutoScout_diag_considered = gAutoScout_diag_considered + 1;

   if (gAutoScout_areaClaim[areaID] != 0)
   {
      gAutoScout_diag_rejClaim = gAutoScout_diag_rejClaim + 1;
      return(false);
   }
   if (gAutoScout_areaSelfScouted[areaID] == 1)
   {
      gAutoScout_diag_rejSelf = gAutoScout_diag_rejSelf + 1;
      return(false);
   }
   if (autoScout_isAreaBlacklisted(areaID) == true)
   {
      gAutoScout_diag_rejBlacklist = gAutoScout_diag_rejBlacklist + 1;
      return(false);
   }

   // Sample danger for stats regardless of pass/fail. Uses the effective
   // danger (baseline blended with engine reading) so the min/max/avg diag
   // values match what hard-skip / scoring actually use for decisions.
   bool dangerValid = kbAreaGetIsIDValid(areaID);
   float danger = 0.0;
   if (dangerValid == true)
   {
      danger = autoScout_effectiveDanger(areaID);
      gAutoScout_diag_dangerSum   = gAutoScout_diag_dangerSum + danger;
      gAutoScout_diag_dangerCount = gAutoScout_diag_dangerCount + 1;
      if (gAutoScout_diag_dangerCount == 1)
      {
         gAutoScout_diag_dangerMin = danger;
         gAutoScout_diag_dangerMax = danger;
      }
      else
      {
         if (danger < gAutoScout_diag_dangerMin) { gAutoScout_diag_dangerMin = danger; }
         if (danger > gAutoScout_diag_dangerMax) { gAutoScout_diag_dangerMax = danger; }
      }
   }

   if (danger > cAutoScout_DangerHardSkip)
   {
      // Record up to 3 areaID + danger value samples for the BFS-summary log.
      if (gAutoScout_diag_rejID0 < 0)
      {
         gAutoScout_diag_rejID0  = areaID;
         gAutoScout_diag_rejDng0 = danger;
      }
      else if (gAutoScout_diag_rejID1 < 0)
      {
         gAutoScout_diag_rejID1  = areaID;
         gAutoScout_diag_rejDng1 = danger;
      }
      else if (gAutoScout_diag_rejID2 < 0)
      {
         gAutoScout_diag_rejID2  = areaID;
         gAutoScout_diag_rejDng2 = danger;
      }
      gAutoScout_diag_rejDanger = gAutoScout_diag_rejDanger + 1;
      return(false);
   }

   int totalTiles = kbAreaGetNumberTiles(areaID);
   if (totalTiles <= 0)
   {
      gAutoScout_diag_rejTiles = gAutoScout_diag_rejTiles + 1;
      return(false);
   }
   int blackTiles = kbAreaGetNumberBlackTiles(areaID);
   int minBlackPercent = cAutoScout_BlackTilesPercentMin;
   if (autoScout_isOracle(scoutUnitID) == true)
   {
      minBlackPercent = cAutoScout_BlackTilesPercentMinOracle;
   }
   if (blackTiles * 100 / totalTiles < minBlackPercent)
   {
      gAutoScout_diag_rejTiles = gAutoScout_diag_rejTiles + 1;
      return(false);
   }

   vector areaPos = kbAreaGetCenter(areaID);
   if (autoScout_isOnMap(areaPos) == false)
   {
      gAutoScout_diag_rejPath = gAutoScout_diag_rejPath + 1;
      return(false);
   }

   vector unitPos = kbUnitGetPosition(scoutUnitID);
   int unitProto = kbUnitGetProtoUnitID(scoutUnitID);
   if (kbCanPath(unitPos, areaPos, unitProto, 1.0, -1) == false)
   {
      gAutoScout_diag_rejPath = gAutoScout_diag_rejPath + 1;
      return(false);
   }

   // Oracle-source -> other-oracle was previously a hard skip here. It is
   // now a soft additive penalty applied inside autoScout_areaScore (via
   // autoScout_oracleOnOraclePenalty), so oracles can still pick an
   // overlapping area when no better option exists.

   gAutoScout_diag_passed = gAutoScout_diag_passed + 1;
   return(true);
}

// Score function for ranking candidate areas during BFS. Four subscores
// combined into a single baseScore (regular + oracle share the same formula):
//   - tcScore:      closer to player's main TC -> higher; clamped to [0, 1].
//   - scoutScore:   distance to picking scout, signed-squared (see below).
//   - densityScore: fewer other scouts within cAutoScout_DensityRadius
//                   of this area's center -> higher; clamped to [0, 1].
//   - dangerScore:  multiplicative (NOT additive) on the sum of the above.
//
// scoutScore for regular scouts: peak at distance 0, linear decay over
// mapDiag. For oracles: symmetric triangle peaking at MaxOracleLOS / 4 with
// MaxOracleLOS-sharp falloff, allowed to go negative past the cliff so the
// final additive term in baseScore punishes far oracle picks directly.
// The signed-square transform (-x*x for x<0, x*x for x>=0) sharpens the
// curve at both ends: nearby picks are pulled up superlinearly and far/past-
// cliff picks are pushed down superlinearly. Replaces the previous oracle-
// only multiplicative far-penalty -- the squared decay now does that job
// for both source types in one shape.
//
// dangerScore is applied multiplicatively as a final factor: baseScore *
// (1 - dangerRatio) for positive baseScore (the usual case -- the close-to-
// hard-skip area gets attenuated toward zero); baseScore * (1 + dangerRatio)
// clamped to [0, 2] for negative baseScore (oracle far picks get pushed
// further negative when they're also in heat). Areas above hardSkip are
// already excluded by autoScout_areaIsCandidate so dangerRatio stays in
// [0, 1] for the positive branch.
float autoScout_areaScore(
   int areaID = -1, int scoutUnitID = -1,
   vector tcPos = cInvalidVector, vector scoutPos = cInvalidVector, float mapDiag = 1.0)
{
   vector areaPos = kbAreaGetCenter(areaID);
   bool sourceIsOracle = autoScout_isOracle(scoutUnitID);

   float distTC = xsVectorDistanceXZ(tcPos, areaPos);
   float tcScore = floatClamp01(1.0 - distTC / mapDiag);

   // scoutScore: distance from the picking scout to the candidate area.
   //   - Regular scouts: peak at distance 0, linear decay over mapDiag.
   //   - Oracles: symmetric triangle peaking at MaxOracleLOS / 4 (the optimal
   //     hop length that maximises new LOS without wasted overlap or
   //     overshoot, biased even closer in than half-LOS so the oracle leaves
   //     its current claim ring barely overlapping the next). Denominator is
   //     MaxOracleLOS (NOT mapDiag) so the slope is sharp: scoutScore reaches
   //     0 at ~1.25 * MaxOracleLOS from the oracle and stays there for
   //     everything farther. Soft rather than hard-skip -- if no near area
   //     qualifies, far picks can still win via tcScore + density, but the
   //     bias toward nearby is strong.
   float distScout = xsVectorDistanceXZ(scoutPos, areaPos);
   // XS requires every variable to be initialized with a literal or const at
   // declaration -- can't do `float x; if (...) x = ...;`. So we start with
   // a dummy 0.0 value.
   float scoutScore = 0.0;
   if (sourceIsOracle == true)
   {
      float oraclePeakDist = gAutoScout_maxOracleLOS * 0.25;
      float scoutDelta = distScout - oraclePeakDist;
      if (scoutDelta < 0.0) { scoutDelta = -scoutDelta; }

      scoutScore = 1.0 - scoutDelta / gAutoScout_maxOracleLOS;
   }
   else
   {
      scoutScore = 1.0 - distScout / mapDiag;
   }

   // Signed-square: amplifies BOTH ends of the curve. Near scoutScore=1 the
   // square preserves the peak; far values (scoutScore approaching 0 or
   // negative for past-cliff oracle picks) get pulled down faster than
   // linear, so the bias toward nearby grows with the gap. The sign-preserving
   // branch keeps negative values negative so they remain additive penalties
   // in the final baseScore sum.
   if (scoutScore < 0.0)
   {
       scoutScore = - (scoutScore * scoutScore);
   }
   else
   {
       scoutScore =  scoutScore * scoutScore;
   }

   // Density: per-other-scout penalty using the MIN of the other scout's
   // current position and its target waypoint, both measured to areaPos.
   // Using the min captures the "scout in-flight" case: if another scout's
   // body is currently far but it's heading toward this region, we still
   // want to discount this candidate. This addresses the cross-walk overlap
   // (two scouts walking past each other to similar destinations) that pure
   // current-position distance misses.
   //
   // Per-pair radius:
   //   - oracle source vs non-oracle other: cAutoScout_DensityRadiusOracleToOther (45)
   //   - all other combinations: cAutoScout_DensityRadius (30)
   // Penalty ramps linearly from 1.0 at exact overlap to 0.0 at radius.
   // Resulting subscore = 1.0 - sum(penalties), clamped to [0, 1].
   float densityPenalty = 0.0;
   int poolSize = gAutoScout_unitID.size();
   for (int i = 0; i < poolSize; i++)
   {
      int otherID = gAutoScout_unitID[i];

      if (otherID == scoutUnitID) { continue; }
      if (kbUnitGetIsIDValid(otherID) == false) { continue; }

      vector otherPos = kbUnitGetPosition(otherID);
      float dPos = xsVectorDistanceXZ(otherPos, areaPos);
      float dEffective = dPos;

      // Skip waypoint contribution if the other scout doesn't have a
      // valid waypoint set (e.g. just transitioned to IDLE this tick).
      // We treat it as valid if the other scout has a claimed area.
      if (gAutoScout_targetAreaID[i] >= 0)
      {
         vector otherWaypoint = gAutoScout_targetWaypoint[i];
         float dWp = xsVectorDistanceXZ(otherWaypoint, areaPos);
         if (dWp < dEffective) { dEffective = dWp; }
      }

      float densityRadius = cAutoScout_DensityRadius;
      if (sourceIsOracle == true && autoScout_isOracle(otherID) == false)
      {
         densityRadius = cAutoScout_DensityRadiusOracleToOther;
      }

      if (dEffective < densityRadius)
      {
         densityPenalty = densityPenalty + (densityRadius - dEffective) / densityRadius;
      }
   }

   float densityScore = floatClamp01(1.0 - densityPenalty);

   // Weighted sum of the three "directional" subscores. scoutScore can be
   // negative (oracle past its cliff), which directly drags baseScore down --
   // that's the same role the old oracle-only multiplicative far-penalty
   // played, now baked into the signed-square scoutScore shape.
   float baseScore = cAutoScout_WeightTC      * tcScore
                   + cAutoScout_WeightDensity * densityScore
                   + cAutoScout_WeightScout   * scoutScore;

   // Multiplicative danger. The candidate gate excludes areas with
   // danger > hardSkip, so dangerRatio is in [0, 1] for the positive branch.
   // Positive baseScore + danger -> attenuates toward zero as danger climbs.
   // Negative baseScore + danger -> stretches further negative (1 + ratio),
   // clamped to [0, 2] so the worst case is a 2x amplification of negativity.
   float danger = autoScout_effectiveDanger(areaID);
   float dangerRatio = danger / cAutoScout_DangerHardSkip;
   float dangerScore = 0.0;
   if (baseScore < 0.0) { dangerScore = 1.0 + dangerRatio; }
   else                 { dangerScore = 1.0 - dangerRatio; }
   dangerScore = floatClamp(dangerScore, 0.0, 2.0);
   baseScore = baseScore * dangerScore;

   // Oracle-overlap shaping:
   //   - source is NOT an oracle: multiplicative discount against oracle
   //     claim circles (autoScout_oraclePenalty, range = oracle radius + pad).
   //     Multiplicative so areas deeply inside an oracle's claim approach
   //     zero score, while areas just barely touching keep most of their score.
   //   - source IS an oracle: strong additive penalty against other oracles
   //     (autoScout_oracleOnOraclePenalty, range = 2 * MaxOracleLOS, weight
   //     ~3.0). Designed to dominate the rest of the score so a fully-
   //     overlapping other oracle alone disqualifies the area in practice.
   //     Soft (not hard-skip) so an oracle with no better option can still
   //     pick a partially-overlapped area.
   if (sourceIsOracle == false)
   {
      float oracleDiscount = floatClamp01(1.0 - autoScout_oraclePenalty(areaPos, scoutUnitID));
      baseScore = baseScore * oracleDiscount;
   }
   else
   {
      float oraclePen = autoScout_oracleOnOraclePenalty(areaPos, scoutUnitID);
      baseScore = baseScore - cAutoScout_OracleOnOracleWeight * oraclePen;
   }

   return(baseScore);
}

// Layered BFS over the scout's reachable area subgraph. Candidates are
// gathered in batches by graph depth from the scout's current area:
//   - Top priority (handled before the BFS): scout's current area (depth 0)
//     if it's still a candidate. Scout in a partially-fogged area finishes
//     scouting it before walking elsewhere.
//   - First batch from BFS: depths 1..N combined where N=2 for regular scouts
//     and N=4 for oracles. Oracles have a far larger effective coverage radius
//     (full MaxOracleLOS when parked), so a wider first-batch gives the
//     oracle-on-oracle additive penalty more candidates to differentiate
//     between (originally a hard skip; now soft via autoScout_oracleOnOraclePenalty).
//   - Subsequent batches: single depth each (N+1, then N+2, ...).
// As soon as a batch contains at least one candidate, we pick the one with
// the highest autoScout_areaScore and return.
// Reset diagnostic counters before a BFS pass.
void autoScout_diag_reset()
{
   gAutoScout_diag_considered   = 0;
   gAutoScout_diag_rejClaim     = 0;
   gAutoScout_diag_rejSelf      = 0;
   gAutoScout_diag_rejBlacklist = 0;
   gAutoScout_diag_rejDanger    = 0;
   gAutoScout_diag_rejTiles     = 0;
   gAutoScout_diag_rejPath      = 0;
   gAutoScout_diag_rejOracle    = 0;
   gAutoScout_diag_passed       = 0;
   gAutoScout_diag_blockDanger    = 0;
   gAutoScout_diag_blockBlacklist = 0;
   gAutoScout_diag_dangerMin    = 0.0;
   gAutoScout_diag_dangerMax    = 0.0;
   gAutoScout_diag_dangerSum    = 0.0;
   gAutoScout_diag_dangerCount  = 0;
   gAutoScout_diag_rejID0       = -1;
   gAutoScout_diag_rejDng0      = 0.0;
   gAutoScout_diag_rejID1       = -1;
   gAutoScout_diag_rejDng1      = 0.0;
   gAutoScout_diag_rejID2       = -1;
   gAutoScout_diag_rejDng2      = 0.0;
}

// Echo the diagnostic stats for the last BFS pass.
void autoScout_diag_log(int scoutUnitID = -1, int result = -1)
{
   float avg = 0.0;
   if (gAutoScout_diag_dangerCount > 0)
   {
      avg = gAutoScout_diag_dangerSum / gAutoScout_diag_dangerCount;
   }
   aiEcho("autoScout: BFS unit=" + scoutUnitID + " result=" + result
      + " considered=" + gAutoScout_diag_considered
      + " passed=" + gAutoScout_diag_passed
      + " rej[claim=" + gAutoScout_diag_rejClaim
      + " self=" + gAutoScout_diag_rejSelf
      + " blacklist=" + gAutoScout_diag_rejBlacklist
      + " danger=" + gAutoScout_diag_rejDanger
      + " tiles=" + gAutoScout_diag_rejTiles
      + " path=" + gAutoScout_diag_rejPath
      + " oracle=" + gAutoScout_diag_rejOracle + "]"
      + " block[danger=" + gAutoScout_diag_blockDanger
      + " blacklist=" + gAutoScout_diag_blockBlacklist + "]");
   aiEcho("autoScout: BFS danger min=" + gAutoScout_diag_dangerMin
      + " max=" + gAutoScout_diag_dangerMax
      + " avg=" + avg
      + " sampled=" + gAutoScout_diag_dangerCount
      + " (hardSkip=" + cAutoScout_DangerHardSkip + ")");
   if (gAutoScout_diag_rejID0 >= 0)
   {
      aiEcho("autoScout: BFS dangerRejSamples area0=" + gAutoScout_diag_rejID0 + "/" + gAutoScout_diag_rejDng0
         + " area1=" + gAutoScout_diag_rejID1 + "/" + gAutoScout_diag_rejDng1
         + " area2=" + gAutoScout_diag_rejID2 + "/" + gAutoScout_diag_rejDng2);
   }
}

// Clear gAutoScout_bfsResultPath / Len. Called on every BFS failure path and
// at the start of every successful path-build to wipe the previous result.
void autoScout_clearBfsResult()
{
   while (gAutoScout_bfsResultPath.size() > 0)
   {
      gAutoScout_bfsResultPath.removeIndex(0);
   }
   gAutoScout_bfsResultLen = 0;
}

// Fill gAutoScout_bfsResultPath with the ordered chain start -> ... -> target
// by walking gAutoScout_bfsPredecessor backwards from target. Capped at
// cAutoScout_MaxCorridorHops as a defensive guard against unexpected loops.
// XS doesn't allow int[] function parameters, hence the global predecessor
// table instead of passing it in.
void autoScout_buildBfsPath(int startArea = -1, int targetArea = -1)
{
   autoScout_clearBfsResult();
   if (targetArea < 0 || startArea < 0) { return; }

   // Walk back from target -> start. tmp holds the chain in target-first order;
   // we reverse it into gAutoScout_bfsResultPath afterwards.
   int[] tmp = new int(0, 0);
   int cur = targetArea;
   while (cur >= 0 && cur != startArea && tmp.size() < cAutoScout_MaxCorridorHops)
   {
      tmp.add(cur);
      if (cur >= gAutoScout_bfsPredecessor.size()) { cur = -1; }
      else                                          { cur = gAutoScout_bfsPredecessor[cur]; }
   }
   if (cur != startArea)
   {
      // Walkback failed (loop guard hit, or predecessor table corrupted).
      // Bail out -- caller will fall back to a single-hop direct move.
      autoScout_clearBfsResult();
      return;
   }
   tmp.add(startArea);

   for (int q = tmp.size() - 1; q >= 0; q = q - 1)
   {
      gAutoScout_bfsResultPath.add(tmp[q]);
   }
   gAutoScout_bfsResultLen = gAutoScout_bfsResultPath.size();
}

int autoScout_findNextArea(int scoutUnitID = -1)
{
   autoScout_diag_reset();
   autoScout_clearBfsResult();
   if (scoutUnitID < 0) { autoScout_diag_log(scoutUnitID, -1); return(-1); }
   vector unitPos = kbUnitGetPosition(scoutUnitID);
   if (autoScout_isOnMap(unitPos) == false) { autoScout_diag_log(scoutUnitID, -1); return(-1); }
   int startArea = kbAreaGetIDByPosition(unitPos);
   if (startArea < 0) { autoScout_diag_log(scoutUnitID, -1); return(-1); }

   // Top priority: scout's current area if it still has unexplored tiles.
   // Skipped for oracles -- their huge LOS makes "finish the current area"
   // an anti-pattern: once an oracle has parked here, areaSelfScouted is
   // set and depth-0 fails anyway, but for oracles relocated by an outside
   // force (engine wandering, player command) the BFS picks should be
   // strictly score-based, not "stick to wherever you happen to be".
   if (autoScout_isOracle(scoutUnitID) == false)
   {
      if (autoScout_areaIsCandidate(startArea, scoutUnitID) == true)
      {
         // Trivial corridor: scout stays in current area, no intermediate hops.
         gAutoScout_bfsResultPath.add(startArea);
         gAutoScout_bfsResultLen = 1;
         autoScout_diag_log(scoutUnitID, startArea);
         return(startArea);
      }
   }

   // Reference TC position for the score function. If no main TC exists,
   // fall back to scout position (TC subscore then degenerates to match
   // the scout subscore -- still useful behavior).
   vector tcPos = unitPos;
   int mainBaseID = kbBaseGetMainID(cMyID);
   if (mainBaseID >= 0)
   {
      vector tc = kbBaseGetLocation(cMyID, mainBaseID);
      if (autoScout_isOnMap(tc) == true) { tcPos = tc; }
   }
   float mapX = kbGetMapXSize();
   float mapZ = kbGetMapZSize();
   float mapDiag = sqrt(mapX * mapX + mapZ * mapZ);
   if (mapDiag < 1.0) { mapDiag = 1.0; }

   int areaCount = kbAreaGetNumber();
   int[] visited     = new int(areaCount, 0);
   int[] queue       = new int(0, 0);
   int[] queueDepth  = new int(0, 0);
   // Grow predecessor table to areaCount if it's smaller (extern globals
   // can't be wholesale-reassigned in XS like locals can), then reset every
   // entry to -1. Walk-back from chosen target terminates on area-ID equality
   // with startArea, so -1 is just "not yet visited" -- including the start
   // itself, which has no predecessor.
   while (gAutoScout_bfsPredecessor.size() < areaCount)
   {
      gAutoScout_bfsPredecessor.add(-1);
   }
   for (int k = 0; k < areaCount; k = k + 1)
   {
      gAutoScout_bfsPredecessor[k] = -1;
   }
   queue.add(startArea);
   queueDepth.add(0);
   visited[startArea] = 1;

   // currentBatchMax: largest depth still in the current batch. First batch
   // is wider for oracles so the hard-skip exclusion has more candidates to
   // choose from -- avoids cases where the only depth-1/2 areas overlap an
   // existing oracle and the algorithm has to pick the lesser-evil overlap.
   int currentBatchMax = 2;
   if (autoScout_isOracle(scoutUnitID) == true) { currentBatchMax = 3; }
   int batchBest = -1;
   float batchBestScore = -1.0e18;

   int head = 0;
   while (head < queue.size())
   {
      int areaID = queue[head];
      int depth  = queueDepth[head];
      head = head + 1;

      // Batch boundary: if we've moved past the current batch, finalize it.
      // Loop because intermediate empty depths are skipped over.
      while (depth > currentBatchMax)
      {
         if (batchBest >= 0)
         {
            autoScout_buildBfsPath(startArea, batchBest);
            autoScout_diag_log(scoutUnitID, batchBest);
            return(batchBest);
         }
         currentBatchMax = currentBatchMax + 1;
         batchBest = -1;
         batchBestScore = -1.0e18;
      }

      if (depth >= 1 && autoScout_areaIsCandidate(areaID, scoutUnitID) == true)
      {
         float s = autoScout_areaScore(areaID, scoutUnitID, tcPos, unitPos, mapDiag);
         if (s > batchBestScore)
         {
            batchBestScore = s;
            batchBest = areaID;
         }
      }

      // Path-aware propagation: don't expand BFS through a dangerous or
      // blacklisted area. The engine's pathfinder doesn't see our blacklist
      // and may route the scout through a danger pocket on the way to a
      // candidate on the far side; treating those areas as walls in our
      // area-graph traversal means candidates reachable only via such a
      // corridor are never visited and so can never be picked. Start area
      // (depth 0) is the scout's current position -- always allow expansion
      // from there so the scout can leave a temporarily-dangerous starting
      // location.
      bool blockExpand = false;
      if (depth >= 1)
      {
         if (autoScout_areaIsDangerous(areaID) == true)
         {
            blockExpand = true;
            gAutoScout_diag_blockDanger = gAutoScout_diag_blockDanger + 1;
         }
         else if (autoScout_isAreaBlacklisted(areaID) == true)
         {
            blockExpand = true;
            gAutoScout_diag_blockBlacklist = gAutoScout_diag_blockBlacklist + 1;
         }
      }
      if (blockExpand == false)
      {
         int n = kbAreaGetNumberBorderAreas(areaID);
         for (int j = 0; j < n; j++)
         {
            int next = kbAreaGetBorderAreaID(areaID, j);
            if (next < 0 || next >= areaCount) { continue; }
            if (visited[next] == 1) { continue; }
            visited[next] = 1;
            gAutoScout_bfsPredecessor[next] = areaID;
            queue.add(next);
            queueDepth.add(depth + 1);
         }
      }
   }

   // End of queue: evaluate the in-flight batch.
   if (batchBest >= 0)
   {
      autoScout_buildBfsPath(startArea, batchBest);
      autoScout_diag_log(scoutUnitID, batchBest);
      return(batchBest);
   }
   autoScout_diag_log(scoutUnitID, -1);
   return(-1);
}

//------------------------------------------------------------------------------
// Frontier-walk: in-area exploration during WORKING state
//------------------------------------------------------------------------------

// Sample candidate waypoints around the scout's current position at
// progressively-larger radii (LOS+2, 1.5*LOS, 2*LOS, 2.5*LOS, 3*LOS), 8
// angular positions per radius. For each candidate, accept the first that:
//   - is in the target area (kbAreaGetIDByPosition match)
//   - is reachable from scout via kbCanPath
// Returned candidate is by construction outside scout's current LOS (each
// radius >= LOS + 2), so walking there reveals new tiles. Returns
// cInvalidVector if no candidate qualifies (area's reachable extent is
// fully within scout's current LOS -> coverage done).
//
// Sampling is centered on the SCOUT's position, not the area centroid,
// because the scout's actual position may differ from the centroid for
// non-convex areas (the engine pathfinder may have stopped short). This
// makes the sampling responsive to where the scout actually is, naturally
// expanding outward as the scout walks.
vector autoScout_findFrontierWaypoint(int scoutUnitID = -1, int areaID = -1, float los = 18.0)
{
   if (scoutUnitID < 0 || areaID < 0 || los < 1.0) { return(cInvalidVector); }
   vector scoutPos = kbUnitGetPosition(scoutUnitID);
   int unitProto = kbUnitGetProtoUnitID(scoutUnitID);

   for (int r = 0; r < cAutoScout_FrontierRadii; r = r + 1)
   {
      float radius = los + 2.0;
      if (r == 1) { radius = 1.5 * los; }
      if (r == 2) { radius = 2.0 * los; }
      if (r == 3) { radius = 2.5 * los; }
      if (r == 4) { radius = 3.0 * los; }

      for (int a = 0; a < cAutoScout_FrontierAngles; a = a + 1)
      {
         float angle = 2.0 * 3.14159265 * xsIntToFloat(a) / xsIntToFloat(cAutoScout_FrontierAngles);
         float dx = cos(angle) * radius;
         float dz = sin(angle) * radius;
         vector raw = xsVectorCreate(scoutPos.x + dx, scoutPos.y, scoutPos.z + dz);
         vector cand = autoScout_clampToMap(raw);

         int candArea = kbAreaGetIDByPosition(cand);
         if (candArea != areaID) { continue; }

         if (kbCanPath(scoutPos, cand, unitProto, 1.0, -1) == false) { continue; }

         // Skip candidates close to any waypoint we've already walked to in
         // this WORKING session -- breaks the deterministic A<->B oscillation.
         if (autoScout_hasVisitedNear(scoutUnitID, cand) == true) { continue; }

         return(cand);
      }
   }
   return(cInvalidVector);
}

//------------------------------------------------------------------------------
// State machine per scout
//------------------------------------------------------------------------------

void autoScout_setStateIdle(int slot = -1)
{
   if (slot >= 0 && slot < gAutoScout_unitID.size())
   {
      autoScout_clearVisited(gAutoScout_unitID[slot]);
   }
   gAutoScout_state[slot] = cAutoScoutState_Idle;
   gAutoScout_targetWaypoint[slot] = cInvalidVector;
   gAutoScout_workSteps[slot] = 0;
   gAutoScout_stuckTicks[slot] = 0;
   gAutoScout_corridorLen[slot] = 0;
}

// Copies the just-computed BFS corridor (gAutoScout_bfsResultPath) into this
// slot's flat-array slice, then issues the aiTaskMoveUnit chain. Index 0 of
// the chain is the scout's current area (we're already there, no move
// needed); the first move issued is the first hop AWAY from the start area
// and uses queue=false to clear any pending engine queue and start moving
// immediately. Subsequent hops append to the engine queue with queue=true so
// the unit transitions through them seamlessly -- this is what forces the
// engine pathfinder to stay inside the BFS-validated safe corridor instead
// of cutting corners through a heat zone toward the final centroid.
//
// Degenerate corridors (len == 1, i.e. the scout's start area was itself the
// chosen target) issue a single move to the start area's center so the unit
// is tasked; the engine no-ops if already there.
void autoScout_issueCorridor(int slot = -1, int unitID = -1)
{
   if (slot < 0 || unitID < 0) { return; }
   int len = gAutoScout_bfsResultLen;
   if (len <= 0) { gAutoScout_corridorLen[slot] = 0; return; }
   if (len > cAutoScout_MaxCorridorHops) { len = cAutoScout_MaxCorridorHops; }

   int baseIdx = slot * cAutoScout_MaxCorridorHops;
   for (int i = 0; i < len; i = i + 1)
   {
      gAutoScout_corridorAreas[baseIdx + i] = gAutoScout_bfsResultPath[i];
   }
   gAutoScout_corridorLen[slot] = len;

   if (len == 1)
   {
      vector startCenter = kbAreaGetCenter(gAutoScout_corridorAreas[baseIdx]);
      aiTaskMoveUnit(unitID, startCenter, false, false);
      return;
   }

   bool first = true;
   for (int j = 1; j < len; j = j + 1)
   {
      int hopArea = gAutoScout_corridorAreas[baseIdx + j];
      vector hopCenter = kbAreaGetCenter(hopArea);
      bool queueFlag = false;
      if (first == false) { queueFlag = true; }
      aiTaskMoveUnit(unitID, hopCenter, false, queueFlag);
      first = false;
   }
}

// Walk the slot's corridor hops (excluding the start area at index 0 and the
// final target area at index len-1, which are checked separately by the
// existing currentArea / targetArea per-tick gates). Return the first hop
// area that is now dangerous, or -1 if none. Used so a freshly-discovered
// heat zone on a mid-corridor hop triggers a flee BEFORE the scout walks
// into it, instead of waiting for currentArea to update on arrival.
int autoScout_corridorFirstDangerousHop(int slot = -1)
{
   if (slot < 0) { return(-1); }
   int len = gAutoScout_corridorLen[slot];
   if (len < 3) { return(-1); }  // no intermediates (len<=2: just start + maybe target)
   int baseIdx = slot * cAutoScout_MaxCorridorHops;
   for (int i = 1; i < len - 1; i = i + 1)
   {
      int hopArea = gAutoScout_corridorAreas[baseIdx + i];
      if (hopArea >= 0 && autoScout_areaIsDangerous(hopArea) == true)
      {
         return(hopArea);
      }
   }
   return(-1);
}

// Arrived = scout is right at the waypoint (within cAutoScout_ArrivalDistance,
// edge-to-point), OR scout has been in-state long enough and engine reports
// it Idle (means it pathed as close as it could and stopped, e.g. blocked
// centroid). The min-ticks gate avoids interpreting the brief Idle frame
// between issuing the task and the engine picking it up as "arrived".
//
// Tight arrival distance is important: a loose threshold (like one LOS) makes
// the scout stop short of small areas and short-circuit through the
// "small area = move on" branch before actually reaching the centroid.
bool autoScout_arrived(int unitID = -1, vector target = cInvalidVector, int ticksInState = 0)
{
   float dist = kbUnitGetDistanceToPoint(unitID, target);
   if (dist <= cAutoScout_ArrivalDistance) { return(true); }
   if (ticksInState >= cAutoScout_MinTicksBeforeIdleAccept &&
       kbUnitGetActionType(unitID) == cActionTypeIdle)
   {
      return(true);
   }
   return(false);
}

// Shared Diverting-state handler. Called from both autoScout_tickUnit (regular
// scouts) and autoScout_tickOracleUnit (oracles). Identical semantics: re-target
// the herd every tick (herds wander), complete on herd-invalid OR herd-flipped-
// to-us OR arrived OR stuck-timeout. On completion, releases area claim, sets
// Idle, clears targetHerdID. Returns true if the state transitioned.
bool autoScout_tickDivertingState(int slot = -1, int unitID = -1)
{
   if (slot < 0 || unitID < 0) { return(false); }

   int herdID = gAutoScout_targetHerdID[slot];
   bool herdInvalid = (kbUnitGetIsIDValid(herdID) == false);
   bool herdOurs = false;
   if (herdInvalid == false) { herdOurs = (kbUnitGetPlayerID(herdID) == cMyID); }

   bool arrived = false;
   if (herdInvalid == false && herdOurs == false)
   {
      arrived = autoScout_arrived(unitID, gAutoScout_targetWaypoint[slot], gAutoScout_stuckTicks[slot]);
   }

   bool stuck = (gAutoScout_stuckTicks[slot] >= cAutoScout_StuckTickLimit);

   if (herdInvalid == true || herdOurs == true || arrived == true || stuck == true)
   {
      aiEcho("autoScout: DIVERT done unit " + unitID + " herd " + herdID
         + " (invalid=" + herdInvalid + " ours=" + herdOurs
         + " arrived=" + arrived + " stuck=" + stuck + ")");
      autoScout_releaseClaim(slot);
      autoScout_setStateIdle(slot);
      gAutoScout_targetHerdID[slot] = -1;
      return(true);
   }

   gAutoScout_stuckTicks[slot] = gAutoScout_stuckTicks[slot] + 1;
   gAutoScout_targetWaypoint[slot] = kbUnitGetPosition(herdID);
   aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
   return(false);
}

// Oracle-specific tick handler. State machine:
//   Idle -> Walking (issue move to chosen area centroid)
//   Walking -> Stationed (on arrival OR currentLOS/MaxOracleLOS < 0.5)
//   Stationed -> Idle (on saturation: action == cAutoScout_OracleSaturatedActionType)
//   Diverting -> handled by shared autoScout_tickDivertingState
// tryDivert is called before any other state transition so herd claims always
// preempt -- including suspending the 50% LOS floor.
//
// Returns true on a state transition this tick (caller may re-tick).
bool autoScout_tickOracleUnit(int slot = -1)
{
   int unitID = gAutoScout_unitID[slot];
   int planID = gAutoScout_planID[slot];

   if (kbUnitGetIsIDValid(unitID) == false ||
       kbUnitGetPlayerID(unitID) != cMyID ||
       aiPlanGetIsIDValid(planID) == false)
   {
      autoScout_dropFromPool(slot);
      return(true);
   }

   autoScout_trackHPAndPos(slot, unitID);

   // Opportunistic cache update -- harmless if not saturated (the helper is
   // gated). Done every tick so we capture the saturation moment regardless
   // of which state branch we hit.
   autoScout_updateMaxOracleLOS(unitID);

   // Danger check: same logic as regular tickUnit. Applies to all non-Idle
   // non-Fleeing oracle states (Walking / Stationed / Diverting).
   int preState = gAutoScout_state[slot];
   if (preState != cAutoScoutState_Idle && preState != cAutoScoutState_Fleeing)
   {
      vector unitPos = kbUnitGetPosition(unitID);
      int currentArea = -1;
      if (autoScout_isOnMap(unitPos) == true)
      {
         currentArea = kbAreaGetIDByPosition(unitPos);
      }
      int targetArea = gAutoScout_targetAreaID[slot];

      if (currentArea >= 0 && autoScout_areaIsDangerous(currentArea) == true)
      {
         autoScout_blacklistArea(currentArea);
         autoScout_enterFleeing(slot, unitID, currentArea);
         return(true);
      }
      if (targetArea >= 0 && targetArea != currentArea
          && autoScout_areaIsDangerous(targetArea) == true)
      {
         autoScout_blacklistArea(targetArea);
         autoScout_enterFleeing(slot, unitID, targetArea);
         return(true);
      }
      int dangerHop = autoScout_corridorFirstDangerousHop(slot);
      if (dangerHop >= 0)
      {
         autoScout_blacklistArea(dangerHop);
         autoScout_enterFleeing(slot, unitID, dangerHop);
         return(true);
      }
   }

   float los = kbUnitGetStatFloat(unitID, cUnitStatLOS);
   int   state = gAutoScout_state[slot];

   // Diverting: shared logic. No tryDivert call here since we are already
   // committed to a herd target.
   if (state == cAutoScoutState_Diverting)
   {
      return(autoScout_tickDivertingState(slot, unitID));
   }

   // tryDivert outranks all other state transitions (incl. the 50% LOS floor).
   if (autoScout_tryDivert(slot, unitID, los) == true) { return(true); }

   if (state == cAutoScoutState_Idle)
   {
      int nextArea = autoScout_findNextArea(unitID);
      if (nextArea < 0)
      {
         // No candidate area. Drop from pool and let engine plan housekeeping
         // revert the UI button (same as regular scouts when BFS is exhausted).
         aiTaskStopUnit(unitID);
         aiPlanDestroy(planID);
         autoScout_dropFromPool(slot);
         return(true);
      }
      gAutoScout_areaClaim[nextArea] = unitID;
      gAutoScout_targetAreaID[slot] = nextArea;
      gAutoScout_targetWaypoint[slot] = kbAreaGetCenter(nextArea);
      gAutoScout_state[slot] = cAutoScoutState_Walking;
      gAutoScout_stuckTicks[slot] = 0;
      aiEcho("autoScout: oracle " + unitID + " picked area " + nextArea
         + " centroid=" + gAutoScout_targetWaypoint[slot]
         + " corridorLen=" + gAutoScout_bfsResultLen
         + " (maxLOS=" + gAutoScout_maxOracleLOS + ")");
      autoScout_issueCorridor(slot, unitID);
      return(true);
   }

   if (state == cAutoScoutState_Walking)
   {
      vector waypoint = gAutoScout_targetWaypoint[slot];
      int areaID = gAutoScout_targetAreaID[slot];

      if (autoScout_arrived(unitID, waypoint, gAutoScout_stuckTicks[slot]) == true)
      {
         if (areaID >= 0 && areaID < gAutoScout_areaSelfScouted.size())
         {
            gAutoScout_areaSelfScouted[areaID] = 1;
         }
         aiEcho("autoScout: oracle " + unitID + " arrived; parking (los=" + los + ")");
         gAutoScout_state[slot] = cAutoScoutState_Stationed;
         gAutoScout_stuckTicks[slot] = 0;
         return(true);
      }

      gAutoScout_stuckTicks[slot] = gAutoScout_stuckTicks[slot] + 1;
      if (gAutoScout_stuckTicks[slot] >= cAutoScout_StuckTickLimit)
      {
         autoScout_releaseClaim(slot);
         autoScout_setStateIdle(slot);
         return(true);
      }
      // No per-tick aiTaskMoveUnit re-issue -- the engine cPlanExplore is
      // parked in cPlanStateIdle so it can't override our initial move; the
      // unit follows the command issued at state-entry until arrival.
      return(false);
   }

   if (state == cAutoScoutState_Stationed)
   {
      // Saturation == LOS hit cap. Action 37 is the engine signal (the
      // meditation animation plays at this point). Re-pick a new area.
      if (kbUnitGetActionType(unitID) == cAutoScout_OracleSaturatedActionType)
      {
         aiEcho("autoScout: oracle " + unitID + " saturated at los=" + los
            + " (maxLOS cache=" + gAutoScout_maxOracleLOS + "), repicking");
         autoScout_releaseClaim(slot);
         autoScout_setStateIdle(slot);
         return(true);
      }
      // Still growing -- stay parked, do nothing. The engine cPlanExplore
      // plan is kept inert via the loop-radius trick set in
      // autoScout_register; nothing to override here.
      return(false);
   }

   // Working state should not occur for oracles (they don't enter frontier-
   // walk). Defensive: promote to Stationed.
   if (state == cAutoScoutState_Working)
   {
      gAutoScout_state[slot] = cAutoScoutState_Stationed;
      return(true);
   }

   if (state == cAutoScoutState_Fleeing)
   {
      if (xsGetTimeMS() < gAutoScout_fleeUntilMs[slot]) { return(false); }
      aiEcho("autoScout: oracle flee-hold expired slot=" + slot + " unit=" + unitID
         + ", returning to Idle");
      autoScout_setStateIdle(slot);
      gAutoScout_fleeFromArea[slot] = -1;
      return(true);
   }

   return(false);
}

// Returns true if this tick caused a state transition (caller may want to
// re-tick the scout in the same rule firing to avoid wasting a frame in
// the new state).
bool autoScout_tickUnit(int slot = -1)
{
   int unitID = gAutoScout_unitID[slot];
   int planID = gAutoScout_planID[slot];

   if (kbUnitGetIsIDValid(unitID) == false ||
       kbUnitGetPlayerID(unitID) != cMyID ||
       aiPlanGetIsIDValid(planID) == false)
   {
      autoScout_dropFromPool(slot);
      return(true);
   }

   autoScout_trackHPAndPos(slot, unitID);

   // Route oracles to their dedicated state machine. They share the pool and
   // the Diverting handler with regular scouts but have their own
   // Idle/Walking/Stationed flow (no Working / frontier-walk).
   if (autoScout_isOracle(unitID) == true)
   {
      return(autoScout_tickOracleUnit(slot));
   }

   float los = kbUnitGetStatFloat(unitID, cUnitStatLOS);
   if (los < 1.0) { los = 18.0; }

   // Danger check: applies to all non-Idle non-Fleeing states. Idle is exempt
   // because the next BFS pick respects the hard-skip and blacklist directly;
   // Fleeing is exempt to avoid recursive entry while the timer is held.
   int preState = gAutoScout_state[slot];
   if (preState != cAutoScoutState_Idle && preState != cAutoScoutState_Fleeing)
   {
      vector unitPos = kbUnitGetPosition(unitID);
      int currentArea = -1;
      if (autoScout_isOnMap(unitPos) == true)
      {
         currentArea = kbAreaGetIDByPosition(unitPos);
      }
      int targetArea = gAutoScout_targetAreaID[slot];

      if (currentArea >= 0 && autoScout_areaIsDangerous(currentArea) == true)
      {
         autoScout_blacklistArea(currentArea);
         autoScout_enterFleeing(slot, unitID, currentArea);
         return(true);
      }
      if (targetArea >= 0 && targetArea != currentArea
          && autoScout_areaIsDangerous(targetArea) == true)
      {
         autoScout_blacklistArea(targetArea);
         autoScout_enterFleeing(slot, unitID, targetArea);
         return(true);
      }
      int dangerHop = autoScout_corridorFirstDangerousHop(slot);
      if (dangerHop >= 0)
      {
         autoScout_blacklistArea(dangerHop);
         autoScout_enterFleeing(slot, unitID, dangerHop);
         return(true);
      }
   }

   int state = gAutoScout_state[slot];

   if (state == cAutoScoutState_Idle)
   {
      int nextArea = autoScout_findNextArea(unitID);
      if (nextArea < 0)
      {
         // Inline of vanilla disableAutoScouting (defined later in
         // human_assist.xs, can't forward-reference): stop unit + destroy
         // engine plan (which reverts the UI button).
         aiTaskStopUnit(unitID);
         aiPlanDestroy(planID);
         autoScout_dropFromPool(slot);
         return(true);
      }
      gAutoScout_areaClaim[nextArea] = unitID;
      gAutoScout_targetAreaID[slot] = nextArea;
      gAutoScout_targetWaypoint[slot] = kbAreaGetCenter(nextArea);
      gAutoScout_state[slot] = cAutoScoutState_Walking;
      gAutoScout_stuckTicks[slot] = 0;
      aiEcho("autoScout: scout " + unitID + " picked area " + nextArea
         + " centroid=" + gAutoScout_targetWaypoint[slot]
         + " corridorLen=" + gAutoScout_bfsResultLen);
      autoScout_issueCorridor(slot, unitID);
      return(true);
   }

   if (state == cAutoScoutState_Walking)
   {
      if (autoScout_tryDivert(slot, unitID, los) == true) { return(true); }

      vector waypoint = gAutoScout_targetWaypoint[slot];
      int areaID = gAutoScout_targetAreaID[slot];

      if (autoScout_arrived(unitID, waypoint, gAutoScout_stuckTicks[slot]) == true)
      {
         // Mark this area as "we visited the centroid" regardless of how
         // many black tiles remain. Prevents the loop where the engine's
         // fog count keeps the area as a candidate even after our scout
         // has reached its center, causing the scout to be reassigned to
         // the same area indefinitely.
         if (areaID >= 0 && areaID < gAutoScout_areaSelfScouted.size())
         {
            gAutoScout_areaSelfScouted[areaID] = 1;
         }

         vector unitPos = kbUnitGetPosition(unitID);
         int currentArea = -1;
         if (autoScout_isOnMap(unitPos) == true)
         {
            currentArea = kbAreaGetIDByPosition(unitPos);
         }

         // Scout couldn't actually enter the target area (non-convex,
         // centroid blocked or outside the polygon): no point doing
         // frontier-walk -- the centroid was unreachable so we'd just
         // re-walk to it. Pass done.
         if (currentArea != areaID)
         {
            aiEcho("autoScout: SHORT-CIRCUIT (centroid not in area) unit " + unitID
               + " area " + areaID + " currentArea=" + currentArea);
            autoScout_releaseClaim(slot);
            autoScout_setStateIdle(slot);
            return(true);
         }

         // Already-covered fast path: if the centroid visit + LOS sweep
         // already brought blackTiles below threshold, skip WORKING.
         int totalTiles = kbAreaGetNumberTiles(areaID);
         int blackTiles = kbAreaGetNumberBlackTiles(areaID);
         int blackPercent = 0;
         if (totalTiles > 0) { blackPercent = blackTiles * 100 / totalTiles; }
         if (blackPercent < cAutoScout_BlackTilesPercentMin)
         {
            autoScout_releaseClaim(slot);
            autoScout_setStateIdle(slot);
            return(true);
         }

         // Frontier-walk: sample for an in-area, reachable, currently-
         // out-of-LOS waypoint. If none exists, the area's reachable
         // extent is already within scout's LOS -- done.
         vector frontierWp = autoScout_findFrontierWaypoint(unitID, areaID, los);
         if (frontierWp == cInvalidVector)
         {
            autoScout_releaseClaim(slot);
            autoScout_setStateIdle(slot);
            return(true);
         }

         aiEcho("autoScout: entering WORKING for unit " + unitID + " area " + areaID
            + " (tileCount=" + totalTiles + ", blackTiles=" + blackTiles + ", LOS=" + los + ")");
         gAutoScout_state[slot] = cAutoScoutState_Working;
         gAutoScout_workSteps[slot] = 0;
         gAutoScout_stuckTicks[slot] = 0;
         gAutoScout_targetWaypoint[slot] = frontierWp;
         // Clear any stale visited memory (defensive -- setStateIdle should
         // have done it already) and seed the new session with this waypoint.
         autoScout_clearVisited(unitID);
         autoScout_recordVisited(unitID, frontierWp);
         aiTaskMoveUnit(unitID, frontierWp, false, false);
         return(true);
      }

      gAutoScout_stuckTicks[slot] = gAutoScout_stuckTicks[slot] + 1;
      if (gAutoScout_stuckTicks[slot] >= cAutoScout_StuckTickLimit)
      {
         autoScout_releaseClaim(slot);
         autoScout_setStateIdle(slot);
         return(true);
      }
      // No per-tick aiTaskMoveUnit re-issue -- the engine cPlanExplore is
      // parked in cPlanStateIdle so it can't override our initial move; the
      // unit follows the command issued at state-entry until arrival.
      return(false);
   }

   if (state == cAutoScoutState_Working)
   {
      if (autoScout_tryDivert(slot, unitID, los) == true) { return(true); }

      vector waypoint = gAutoScout_targetWaypoint[slot];
      int areaID = gAutoScout_targetAreaID[slot];

      if (autoScout_arrived(unitID, waypoint, gAutoScout_stuckTicks[slot]) == true)
      {
         // Early exit: black-tile percentage dropped below threshold ->
         // area effectively covered.
         int totalTiles = kbAreaGetNumberTiles(areaID);
         int blackTiles = kbAreaGetNumberBlackTiles(areaID);
         int blackPercent = 0;
         if (totalTiles > 0) { blackPercent = blackTiles * 100 / totalTiles; }
         if (blackPercent < cAutoScout_BlackTilesPercentMin)
         {
            autoScout_releaseClaim(slot);
            autoScout_setStateIdle(slot);
            return(true);
         }

         // Step cap: defensive against pathological shapes / oscillation.
         int step = gAutoScout_workSteps[slot] + 1;
         if (step >= cAutoScout_MaxFrontierSteps)
         {
            aiEcho("autoScout: WORKING step cap hit unit " + unitID + " area " + areaID
               + " (blackTiles=" + blackTiles + " of " + totalTiles + ")");
            autoScout_releaseClaim(slot);
            autoScout_setStateIdle(slot);
            return(true);
         }
         gAutoScout_workSteps[slot] = step;
         gAutoScout_stuckTicks[slot] = 0;

         // Find next frontier waypoint (in-area, reachable, outside scout's
         // current LOS). If none, the reachable extent is now within LOS
         // -> coverage done from where we are.
         vector nextWp = autoScout_findFrontierWaypoint(unitID, areaID, los);
         if (nextWp == cInvalidVector)
         {
            autoScout_releaseClaim(slot);
            autoScout_setStateIdle(slot);
            return(true);
         }

         gAutoScout_targetWaypoint[slot] = nextWp;
         autoScout_recordVisited(unitID, nextWp);
         aiTaskMoveUnit(unitID, nextWp, false, false);
         return(true);
      }

      gAutoScout_stuckTicks[slot] = gAutoScout_stuckTicks[slot] + 1;
      if (gAutoScout_stuckTicks[slot] >= cAutoScout_StuckTickLimit)
      {
         autoScout_releaseClaim(slot);
         autoScout_setStateIdle(slot);
         return(true);
      }
      // No per-tick aiTaskMoveUnit re-issue -- the engine cPlanExplore is
      // parked in cPlanStateIdle so it can't override our initial move; the
      // unit follows the command issued at state-entry until arrival.
      return(false);
   }

   if (state == cAutoScoutState_Diverting)
   {
      return(autoScout_tickDivertingState(slot, unitID));
   }

   if (state == cAutoScoutState_Fleeing)
   {
      if (xsGetTimeMS() < gAutoScout_fleeUntilMs[slot]) { return(false); }
      aiEcho("autoScout: flee-hold expired slot=" + slot + " unit=" + unitID
         + ", returning to Idle");
      autoScout_setStateIdle(slot);
      gAutoScout_fleeFromArea[slot] = -1;
      return(true);
   }

   return(false);
}

//------------------------------------------------------------------------------
// Public registration (called from human_assist.xs::enableAutoScouting)
//------------------------------------------------------------------------------

void autoScout_register(int planID = -1, int unitID = -1)
{
   if (planID < 0 || unitID < 0) { return; }

   // Park the cPlanExplore in cPlanStateIdle (23, from docs/MythTRConstants.txt).
   // Verified empirically (2026-05-13) that the engine respects this state and
   // does NOT drive the unit while leaving the plan alive as the UI marker.
   // Applies to BOTH oracles and regular scouts so the state machine has full
   // control without per-tick override. The cPlanStateIdle is not Done/Failed,
   // so the plan is not auto-destroyed.
   aiPlanSetState(planID, cAutoScout_PlanStateIdle);

   gAutoScout_unitID.add(unitID);
   gAutoScout_planID.add(planID);
   gAutoScout_state.add(cAutoScoutState_Idle);
   gAutoScout_targetAreaID.add(-1);
   gAutoScout_targetWaypoint.add(cInvalidVector);
   gAutoScout_workSteps.add(0);
   gAutoScout_stuckTicks.add(0);
   gAutoScout_targetHerdID.add(-1);
   gAutoScout_fleeUntilMs.add(0);
   gAutoScout_fleeFromArea.add(-1);
   gAutoScout_lastHP.add(kbUnitGetStatFloat(unitID, cUnitStatCurrHP));
   gAutoScout_lastPos.add(kbUnitGetPosition(unitID));
   gAutoScout_corridorLen.add(0);
   for (int i = 0; i < cAutoScout_MaxCorridorHops; i = i + 1)
   {
      gAutoScout_corridorAreas.add(-1);
   }

   // Immediate first-tick: BFS + initial move now, instead of waiting up to
   // a full rule interval. Without this, the scout starts moving in whatever
   // direction the engine plan briefly chose before NumberOfLoops=0 kicked in.
   autoScout_initAreaArrays();
   int slot = gAutoScout_unitID.size() - 1;
   autoScout_tickUnit(slot);
}

//------------------------------------------------------------------------------
// Home-move scan: route newly-converted herdables to the nearest TC, once.
//------------------------------------------------------------------------------

// Scans all herdables we own (regardless of how the conversion happened —
// intentional divert OR organic walk-past during scouting) and issues a
// single move-to-nearest-TC for each one we haven't yet redirected. The
// per-herd redirected mark is permanent for the game so subsequent player
// command overrides aren't fought.
// Drops redirectedHerdIDs entries for herds that are dead or no longer ours.
// When a herd flips back to us (e.g. an enemy stole it, then our scout
// walks past and proximity-converts it again), this lets homeMoveScan
// re-issue the move-to-TC on the re-acquired herd instead of leaving it
// stranded with a stale redirected mark.
void autoScout_pruneRedirectedHerds()
{
   for (int i = gAutoScout_redirectedHerdIDs.size() - 1; i >= 0; i = i - 1)
   {
      int herdID = gAutoScout_redirectedHerdIDs[i];
      bool valid = kbUnitGetIsIDValid(herdID);
      bool ours = false;
      if (valid == true) { ours = (kbUnitGetPlayerID(herdID) == cMyID); }
      if (valid == false || ours == false)
      {
         gAutoScout_redirectedHerdIDs.removeIndex(i);
      }
   }
}

void autoScout_homeMoveScan()
{
   // Short-circuit: with no TC there's nowhere to deliver herds. Skips the
   // owned-herd query entirely until we have a TC, at which point pending
   // herdables get redirected on the next tick.
   if (autoScout_anyTCAlive() == false) { return; }

   autoScout_pruneRedirectedHerds();

   autoScout_initOwnedHerdQuery();
   kbUnitQueryResetResults(gAutoScout_ownedHerdQuery);
   int n = kbUnitQueryExecute(gAutoScout_ownedHerdQuery);
   for (int i = 0; i < n; i = i + 1)
   {
      int herdID = kbUnitQueryGetResult(gAutoScout_ownedHerdQuery, i);
      if (herdID < 0) { continue; }
      if (autoScout_isHerdRedirected(herdID) == true) { continue; }

      vector herdPos = kbUnitGetPosition(herdID);
      int tcID = autoScout_findNearestTCID(herdPos);
      if (tcID < 0) { continue; }

      // aiTaskWorkUnit on a TC delivers the herd via the engine's herd-on-TC
      // command — herdables surround the TC instead of stacking on a point,
      // matching the right-click-herd-on-TC player UX.
      aiEcho("autoScout: home-move herd " + herdID + " -> TC " + tcID
         + " at " + kbUnitGetPosition(tcID));
      aiTaskWorkUnit(herdID, tcID, false);
      gAutoScout_redirectedHerdIDs.add(herdID);
   }
}

//------------------------------------------------------------------------------
// Tick rule
//------------------------------------------------------------------------------

// Split tick rules. Both fire at minInterval 1 (the original cadence). The
// split is purely organisational -- heat-map / home-move scan live in one
// rule, the per-slot state machine in another -- so future tuning can move
// either rule's cadence independently without touching the other's body.
//
// An earlier iteration ran autoScout_tickFast at highFrequency + priority 80
// to close the ~500ms-1s "idle military unit" UI flicker between a scout
// finishing WORKING and BFS picking its next area. Backed out 2026-05-14:
// every-frame execution pushed enough script load that the engine started
// throttling scripts (it prefers slowing the script budget over slowing the
// update loop), which manifested as ~10s delays in homeMoveScan picking up
// newly-converted herds (kb-update latency on the owned-herd query). For
// now the UI flicker is accepted; if revisited, prefer reducing per-frame
// work over re-raising priority.

rule autoScout_tickHeavy
minInterval 1
active
{
   if (kbPlayerIsHuman(cMyID) == false) { return; }
   xsSetContextPlayer(cMyID);
   autoScout_initAreaArrays();
   autoScout_updateHeatMap();
   autoScout_homeMoveScan();
   xsSetContextPlayer(-1);
}

rule autoScout_tickFast
minInterval 1
active
{
   if (kbPlayerIsHuman(cMyID) == false) { return; }
   xsSetContextPlayer(cMyID);
   autoScout_initAreaArrays();

   for (int slot = gAutoScout_unitID.size() - 1; slot >= 0; slot = slot - 1)
   {
      // Chain state transitions in the same firing: e.g. WALKING -> arrived
      // small area -> IDLE -> BFS -> WALKING new area, all in one tick. Cap
      // iterations to defend against unexpected cycles.
      bool transitioned = true;
      int iter = 0;
      while (transitioned == true && iter < cAutoScout_MaxChainPerTick)
      {
         if (slot >= gAutoScout_unitID.size()) { break; }  // dropped from pool
         transitioned = autoScout_tickUnit(slot);
         iter = iter + 1;
      }
   }
   xsSetContextPlayer(-1);
}

