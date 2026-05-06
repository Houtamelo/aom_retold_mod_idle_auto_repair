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

// Extra detection radius beyond the scout's LOS, in tiles. Catches herds that
// briefly flicker at the LOS boundary between ticks. Per-result we still
// require the herd's tile to be visible OR fogged (i.e. not pure black /
// never-explored), which keeps the AI from "cheating" toward unseen herds.
const float cAutoScout_HerdLOSBuffer = 12.0;

// Skip areas where less than this percent of tiles are still black (unexplored).
const int cAutoScout_BlackTilesPercentMin = 10;

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

// Action-type code reported by kbUnitGetActionType when an Oracle has reached
// its peak AutoLOS bonus and switches to the meditation/glow animation.
// Identified empirically from the OracleDiag run on 2026-05-06: the unit's
// reported action transitioned 7 -> 37 the moment its current LOS hit the cap
// and stayed at 37 thereafter. No documented cActionType* constant in the
// extracted doxygen / shipped XS scripts / BANG docs maps to this value, so
// we hardcode it. If a future patch surfaces a real constant (something like
// cActionTypeIdleStatBonusFull), replace this magic number with that.
const int cAutoScout_OracleSaturatedActionType = 37;

// Cold-start value for the dynamic gAutoScout_maxOracleLOS cache. Used until
// any oracle is observed in the saturated action state (action ==
// cAutoScout_OracleSaturatedActionType) with a higher current LOS. 20.0 is
// chosen so the 50% movement floor (cAutoScout_OracleMovementLOSFloor) has a
// meaningful threshold from tick 1, even before any oracle reaches its real cap.
const float cAutoScout_OracleColdCacheMaxLOS = 20.0;

// Movement-state LOS floor for Oracles, expressed as a ratio of MaxOracleLOS.
// While Walking, an Oracle whose currentLOS / MaxOracleLOS drops below this
// ratio stops in place and becomes Stationed, avoiding the vanilla failure
// mode of draining all the way to base LOS (where favor income is minimal).
// Suspended during Diverting -- herd claim outranks LOS preservation.
const float cAutoScout_OracleMovementLOSFloor = 0.5;

// Oracle-vs-oracle exclusion factor used by autoScout_areaIsCandidate when
// the source unit is an Oracle. Areas whose centroid lies within
// cAutoScout_OracleExclusionFactor * gAutoScout_maxOracleLOS of any other
// oracle (toggled-on or not) are skipped. 0.8 leaves a small buffer around
// each oracle's claim so oracles park with adjacent (not overlapping) circles.
const float cAutoScout_OracleExclusionFactor = 0.8;

// Area-score weights (sum need not be exactly 1.0 since we only compare
// scores, but normalized weights make tuning intuitive). Each subscore is
// produced in roughly [0, 1].
const float cAutoScout_WeightTC      = 0.4;  // closer to main TC -> higher
const float cAutoScout_WeightScout   = 0.4;  // closer to picking scout -> higher
const float cAutoScout_WeightDensity = 0.2;  // fewer other scouts nearby -> higher

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
   autoScout_releaseClaim(slot);
   gAutoScout_unitID.removeIndex(slot);
   gAutoScout_planID.removeIndex(slot);
   gAutoScout_state.removeIndex(slot);
   gAutoScout_targetAreaID.removeIndex(slot);
   gAutoScout_targetWaypoint.removeIndex(slot);
   gAutoScout_workSteps.removeIndex(slot);
   gAutoScout_stuckTicks.removeIndex(slot);
   gAutoScout_targetHerdID.removeIndex(slot);
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
   if (current > gAutoScout_maxOracleLOS) { gAutoScout_maxOracleLOS = current; }
}

// Returns true if areaPos lies within (radiusFactor * gAutoScout_maxOracleLOS)
// of ANY of cMyID's alive oracles, excluding excludeUnitID. Used for the
// oracle-vs-oracle hard-skip in autoScout_areaIsCandidate (radiusFactor=0.8).
bool autoScout_anyOracleNear(
   vector areaPos = cInvalidVector, int excludeUnitID = -1, float radiusFactor = 0.8)
{
   autoScout_initOracleQuery();
   kbUnitQueryResetResults(gAutoScout_oracleQuery);
   int n = kbUnitQueryExecute(gAutoScout_oracleQuery);
   if (n <= 0) { return(false); }
   float threshold = radiusFactor * gAutoScout_maxOracleLOS;
   for (int i = 0; i < n; i = i + 1)
   {
      int oracleID = kbUnitQueryGetResult(gAutoScout_oracleQuery, i);
      if (oracleID < 0) { continue; }
      if (oracleID == excludeUnitID) { continue; }
      vector pos = kbUnitGetPosition(oracleID);
      if (xsVectorDistanceXZ(pos, areaPos) < threshold) { return(true); }
   }
   return(false);
}

// Returns a [0, 1] penalty representing summed overlap of areaPos with all
// our oracles' claim circles, excluding excludeUnitID. Penalty per oracle is
// linear in distance: 1.0 at zero distance, 0.0 at >= claim radius. Sums then
// clamps to 1.0. Used as a multiplicative discount in autoScout_areaScore for
// non-oracle source units. Moving oracles use their current dynamic LOS as
// the claim radius (small, transient) instead of MaxOracleLOS.
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
      float radius = gAutoScout_maxOracleLOS;
      if (kbUnitGetActionType(oracleID) == cActionTypeMove)
      {
         radius = kbUnitGetStatFloat(oracleID, cUnitStatLOS);
      }
      if (radius < 0.001) { continue; }
      if (d < radius)
      {
         penalty = penalty + (radius - d) / radius;
      }
   }
   if (penalty > 1.0) { penalty = 1.0; }
   return(penalty);
}

//------------------------------------------------------------------------------
// Candidate area criteria + BFS (closest-to-TC selection)
//------------------------------------------------------------------------------

bool autoScout_areaIsCandidate(int areaID = -1, int scoutUnitID = -1)
{
   if (areaID < 0) { return(false); }
   if (areaID >= gAutoScout_areaClaim.size()) { return(false); }
   if (gAutoScout_areaClaim[areaID] != 0) { return(false); }
   if (gAutoScout_areaSelfScouted[areaID] == 1) { return(false); }

   int totalTiles = kbAreaGetNumberTiles(areaID);
   if (totalTiles <= 0) { return(false); }
   int blackTiles = kbAreaGetNumberBlackTiles(areaID);
   if (blackTiles * 100 / totalTiles < cAutoScout_BlackTilesPercentMin) { return(false); }

   vector areaPos = kbAreaGetCenter(areaID);
   if (autoScout_isOnMap(areaPos) == false) { return(false); }

   vector unitPos = kbUnitGetPosition(scoutUnitID);
   int unitProto = kbUnitGetProtoUnitID(scoutUnitID);
   if (kbCanPath(unitPos, areaPos, unitProto, 1.0, -1) == false) { return(false); }

   // Oracle source hard-skip: refuse candidates whose centroid lies within
   // cAutoScout_OracleExclusionFactor * MaxOracleLOS of any OTHER oracle.
   // Prevents oracle-on-oracle LOS overlap (which throttles favor income
   // for the overlapped pair) and redundant coverage.
   if (autoScout_isOracle(scoutUnitID) == true &&
       autoScout_anyOracleNear(areaPos, scoutUnitID, cAutoScout_OracleExclusionFactor) == true)
   {
      return(false);
   }

   return(true);
}

// Score function for ranking candidate areas during BFS. Three weighted
// subscores, each in roughly [0, 1]:
//   - tcScore:      closer to player's main TC -> higher
//   - scoutScore:   closer to picking scout    -> higher
//   - densityScore: fewer other scouts within cAutoScout_DensityRadius
//                   of this area's center -> higher (avoids overlap)
// The score function is the SOLE prioritization mechanism for candidate
// areas during BFS -- the BFS itself is a flat full-graph traversal (no
// depth-batched layering). Depth-0 (scout's current area) is still handled
// as a hard top-priority before this function runs.
float autoScout_areaScore(
   int areaID = -1, int scoutUnitID = -1,
   vector tcPos = cInvalidVector, vector scoutPos = cInvalidVector, float mapDiag = 1.0)
{
   vector areaPos = kbAreaGetCenter(areaID);

   float distTC = xsVectorDistanceXZ(tcPos, areaPos);
   float tcScore = 1.0 - distTC / mapDiag;
   if (tcScore < 0.0) { tcScore = 0.0; }

   float distScout = xsVectorDistanceXZ(scoutPos, areaPos);
   float scoutScore = 1.0 - distScout / mapDiag;
   if (scoutScore < 0.0) { scoutScore = 0.0; }

   // Density: per-other-scout penalty using the MIN of the other scout's
   // current position and its target waypoint, both measured to areaPos.
   // Using the min captures the "scout in-flight" case: if another scout's
   // body is currently far but it's heading toward this region, we still
   // want to discount this candidate. This addresses the cross-walk overlap
   // (two scouts walking past each other to similar destinations) that pure
   // current-position distance misses.
   //
   // Penalty ramps linearly from 1.0 at exact overlap to 0.0 at
   // cAutoScout_DensityRadius. Resulting subscore = 1.0 - sum(penalties),
   // clamped to [0, 1].
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

      vector otherWaypoint = gAutoScout_targetWaypoint[i];
      // Skip waypoint contribution if the other scout doesn't have a
      // valid waypoint set (e.g. just transitioned to IDLE this tick).
      // We treat it as valid if the other scout has a claimed area.
      if (gAutoScout_targetAreaID[i] >= 0)
      {
         float dWp = xsVectorDistanceXZ(otherWaypoint, areaPos);
         if (dWp < dEffective) { dEffective = dWp; }
      }

      if (dEffective < cAutoScout_DensityRadius)
      {
         densityPenalty = densityPenalty
            + (cAutoScout_DensityRadius - dEffective) / cAutoScout_DensityRadius;
      }
   }
   float densityScore = 1.0 - densityPenalty;
   if (densityScore < 0.0) { densityScore = 0.0; }

   float baseScore = cAutoScout_WeightTC * tcScore
                   + cAutoScout_WeightScout * scoutScore
                   + cAutoScout_WeightDensity * densityScore;

   // Oracle-overlap discount applies only when the source scout is NOT an
   // oracle. Oracle sources already use the hard-skip in
   // autoScout_areaIsCandidate; double-applying a discount here would over-
   // penalise oracle re-positioning. Multiplicative so areas deeply inside an
   // oracle's claim approach zero score, while areas just barely touching the
   // claim circle keep most of their score.
   if (autoScout_isOracle(scoutUnitID) == false)
   {
      float oracleDiscount = 1.0 - autoScout_oraclePenalty(areaPos, scoutUnitID);
      baseScore = baseScore * oracleDiscount;
   }

   return(baseScore);
}

// Layered BFS over the scout's reachable area subgraph. Candidates are
// gathered in batches by graph depth from the scout's current area:
//   - Top priority (handled before the BFS): scout's current area (depth 0)
//     if it's still a candidate. Scout in a partially-fogged area finishes
//     scouting it before walking elsewhere.
//   - First batch from BFS: depths 1 + 2 combined.
//   - Subsequent batches: single depth each (3, then 4, then ...).
// As soon as a batch contains at least one candidate, we pick the one with
// the highest autoScout_areaScore and return.
int autoScout_findNextArea(int scoutUnitID = -1)
{
   if (scoutUnitID < 0) { return(-1); }
   vector unitPos = kbUnitGetPosition(scoutUnitID);
   if (autoScout_isOnMap(unitPos) == false) { return(-1); }
   int startArea = kbAreaGetIDByPosition(unitPos);
   if (startArea < 0) { return(-1); }

   // Top priority: scout's current area if it still has unexplored tiles.
   if (autoScout_areaIsCandidate(startArea, scoutUnitID) == true)
   {
      return(startArea);
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
   queue.add(startArea);
   queueDepth.add(0);
   visited[startArea] = 1;

   // currentBatchMax: largest depth still in the current batch. First batch
   // covers depths up to 2; later batches are single-depth.
   int currentBatchMax = 2;
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
         if (batchBest >= 0) { return(batchBest); }
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

      int n = kbAreaGetNumberBorderAreas(areaID);
      for (int j = 0; j < n; j++)
      {
         int next = kbAreaGetBorderAreaID(areaID, j);
         if (next < 0 || next >= areaCount) { continue; }
         if (visited[next] == 1) { continue; }
         visited[next] = 1;
         queue.add(next);
         queueDepth.add(depth + 1);
      }
   }

   // End of queue: evaluate the in-flight batch.
   if (batchBest >= 0) { return(batchBest); }
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
   gAutoScout_state[slot] = cAutoScoutState_Idle;
   gAutoScout_targetWaypoint[slot] = cInvalidVector;
   gAutoScout_workSteps[slot] = 0;
   gAutoScout_stuckTicks[slot] = 0;
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

   // Opportunistic cache update -- harmless if not saturated (the helper is
   // gated). Done every tick so we capture the saturation moment regardless
   // of which state branch we hit.
   autoScout_updateMaxOracleLOS(unitID);

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
      aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
      return(true);
   }

   if (state == cAutoScoutState_Walking)
   {
      vector waypoint = gAutoScout_targetWaypoint[slot];
      int areaID = gAutoScout_targetAreaID[slot];

      // 50% LOS floor: if currentLOS bled below half MaxOracleLOS, commit to
      // current position. Vanilla failure mode is letting LOS drain to base,
      // where favor income (proportional to LOS area) is minimal.
      bool floorTrigger = false;
      if (gAutoScout_maxOracleLOS > 0.0001)
      {
         float pct = los / gAutoScout_maxOracleLOS;
         if (pct < cAutoScout_OracleMovementLOSFloor) { floorTrigger = true; }
      }
      if (floorTrigger == true)
      {
         aiTaskStopUnit(unitID);
         if (areaID >= 0 && areaID < gAutoScout_areaSelfScouted.size())
         {
            gAutoScout_areaSelfScouted[areaID] = 1;
         }
         aiEcho("autoScout: oracle " + unitID + " LOS-floor stop (los=" + los
            + " maxLOS=" + gAutoScout_maxOracleLOS + ")");
         gAutoScout_state[slot] = cAutoScoutState_Stationed;
         gAutoScout_stuckTicks[slot] = 0;
         return(true);
      }

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
      aiTaskMoveUnit(unitID, waypoint, false, false);
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
      // Still growing -- stay parked, no action.
      return(false);
   }

   // Working state should not occur for oracles (they don't enter frontier-
   // walk). Defensive: promote to Stationed.
   if (state == cAutoScoutState_Working)
   {
      gAutoScout_state[slot] = cAutoScoutState_Stationed;
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

   // Route oracles to their dedicated state machine. They share the pool and
   // the Diverting handler with regular scouts but have their own
   // Idle/Walking/Stationed flow (no Working / frontier-walk).
   if (autoScout_isOracle(unitID) == true)
   {
      return(autoScout_tickOracleUnit(slot));
   }

   float los = kbUnitGetStatFloat(unitID, cUnitStatLOS);
   if (los < 1.0) { los = 18.0; }

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
      aiTaskMoveUnit(unitID, gAutoScout_targetWaypoint[slot], false, false);
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
      aiTaskMoveUnit(unitID, waypoint, false, false);
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
      aiTaskMoveUnit(unitID, waypoint, false, false);
      return(false);
   }

   if (state == cAutoScoutState_Diverting)
   {
      return(autoScout_tickDivertingState(slot, unitID));
   }

   return(false);
}

//------------------------------------------------------------------------------
// Public registration (called from human_assist.xs::enableAutoScouting)
//------------------------------------------------------------------------------

void autoScout_register(int planID = -1, int unitID = -1)
{
   if (planID < 0 || unitID < 0) { return; }

   // Engine plan stays alive as the housekeeping marker (UI button state +
   // player-override / cancel detection); NumberOfLoops=0 makes its own
   // movement loop a no-op so it doesn't fight our aiTaskMoveUnit calls.
   // Oracles share this scaffolding -- our autoScout_tickUnit dispatches
   // them to autoScout_tickOracleUnit at runtime, while the engine's
   // oracle-specific cExplorePlanStopLOSPercentage logic in human_assist.xs
   // becomes inert because the loop count is zero.
   aiPlanSetVariableInt(planID, cExplorePlanNumberOfLoops, 0, 0);

   gAutoScout_unitID.add(unitID);
   gAutoScout_planID.add(planID);
   gAutoScout_state.add(cAutoScoutState_Idle);
   gAutoScout_targetAreaID.add(-1);
   gAutoScout_targetWaypoint.add(cInvalidVector);
   gAutoScout_workSteps.add(0);
   gAutoScout_stuckTicks.add(0);
   gAutoScout_targetHerdID.add(-1);

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

rule autoScout_tick
minInterval 1
active
{
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
   autoScout_homeMoveScan();
   xsSetContextPlayer(-1);
}

//------------------------------------------------------------------------------
// Oracle diagnostic — read every named/integer-indexed stat we can think of
// from each Oracle owned by cMyID. Once-per-game full dump (action stats brute
// forced over enum 0..49 for AutoLOS / AutoGatherFavor / HandAttack) plus a
// per-tick LOS sample so we can see whether kbUnitGetStatFloat(.., cUnitStatLOS)
// actually moves over time.
//
// Flip cAutoScout_OracleDiag to false to disable.
//------------------------------------------------------------------------------

const bool cAutoScout_OracleDiag = false;

extern int  gAutoScout_oracleDiagQuery     = -1;
extern bool gAutoScout_oracleDiagFullDumped = false;

void autoScout_oracleDiagInit()
{
   if (gAutoScout_oracleDiagQuery >= 0) { return; }
   gAutoScout_oracleDiagQuery = kbUnitQueryCreate("autoScout_oracleDiag");
   kbUnitQuerySetPlayerID(gAutoScout_oracleDiagQuery, cMyID, false);
   kbUnitQuerySetUnitType(gAutoScout_oracleDiagQuery, cUnitTypeAbstractOracle);
   kbUnitQuerySetState(gAutoScout_oracleDiagQuery, cUnitStateAlive);
}

// Brute-force every action-stat enum int 0..49 for both float and int stat
// getters on the named protoaction. Logs only non-zero values to keep chat
// readable. The numeric IDs map to whatever enum the engine uses; we match
// known proto.xml values (modifyamount=1.0, modifyratecap=25/30, modifydecay
// =0.3, modifytype=LOS-as-int, etc.) by inspection after the run.
void autoScout_oracleDiagBruteForce(int proto = -1, string action = "")
{
   for (int s = 0; s < 50; s = s + 1)
   {
      float fv = kbProtoUnitGetActionStatFloat(cMyID, proto, action, s);
      if (fv != 0.0)
      {
         aiEcho("[OracleDiag] " + action + " float[" + s + "]=" + fv);
      }
      int iv = kbProtoUnitGetActionStatInt(cMyID, proto, action, s);
      if (iv != 0)
      {
         aiEcho("[OracleDiag] " + action + " int[" + s + "]=" + iv);
      }
   }
   // kbProtoUnitGetActionMaximumRange takes a damage type, not a stat enum;
   // pass -1 (any) and see what comes back. Useful as a separate signal.
   float r = kbProtoUnitGetActionMaximumRange(cMyID, proto, action, -1);
   aiEcho("[OracleDiag] " + action + " maxRange=" + r);
}

rule autoScout_oracleDiag
minInterval 2
active
{
   if (cAutoScout_OracleDiag == false) { return; }
   xsSetContextPlayer(cMyID);
   autoScout_oracleDiagInit();
   kbUnitQueryResetResults(gAutoScout_oracleDiagQuery);
   int n = kbUnitQueryExecute(gAutoScout_oracleDiagQuery);
   if (n <= 0) { xsSetContextPlayer(-1); return; }

   // Only dump the FIRST oracle in the result set — keeps chat readable when
   // multiple oracles are alive. Query result order is stable enough for a
   // diagnostic; we'll typically be tracking the same unit across ticks.
   int unitID = kbUnitQueryGetResult(gAutoScout_oracleDiagQuery, 0);
   if (unitID < 0) { xsSetContextPlayer(-1); return; }
   int    proto      = kbUnitGetProtoUnitID(unitID);
   float  curLOS     = kbUnitGetStatFloat(unitID, cUnitStatLOS);
   int    actionType = kbUnitGetActionType(unitID);
   vector pos        = kbUnitGetPosition(unitID);

   // Per-tick sample — verifies cUnitStatLOS moves over time.
   aiEcho("[OracleDiag] id=" + unitID + " proto=" + proto
      + " curLOS=" + curLOS + " action=" + actionType + " pos=" + pos);

   if (gAutoScout_oracleDiagFullDumped == false)
   {
      // Proto-level baselines — confirms whether base/player APIs differ.
      float baseProtoLOS   = kbDefaultGetProtoStatFloat(proto, cUnitStatLOS);
      float playerProtoLOS = kbPlayerGetProtoStatFloat(cMyID, proto, cUnitStatLOS);
      int   numActions     = kbUnitGetNumberActions(unitID);
      aiEcho("[OracleDiag] one-shot: baseProtoLOS=" + baseProtoLOS
         + " playerProtoLOS=" + playerProtoLOS + " numActions=" + numActions);

      // Action-stat brute force on the three named actions we care about.
      autoScout_oracleDiagBruteForce(proto, "AutoLOS");
      autoScout_oracleDiagBruteForce(proto, "AutoGatherFavor");
      autoScout_oracleDiagBruteForce(proto, "HandAttack");

      gAutoScout_oracleDiagFullDumped = true;
   }
   xsSetContextPlayer(-1);
}
