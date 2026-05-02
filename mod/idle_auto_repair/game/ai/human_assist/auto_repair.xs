//==============================================================================
// auto_repair.xs (Idle Auto-Repair mod)
//
// Idle units capable of repairing (villagers for most civs, infantry for Norse)
// auto-task themselves to repair nearby damaged friendly buildings via the
// engine's regular cPlanRepair plans. Repair goes through the normal Repair
// action with normal resource cost -- not a free Heal hack.
//
// Player commands always win: a `highFrequency` watchdog rule walks our plans
// every game update and removes any unit whose target no longer matches the
// plan's repair target, so a player redirect (move, attack, garrison, etc.)
// is honored within one frame.
//
// Loaded from human_assist.xs via:
//     include "human_assist/auto_repair.xs";
// Relies on the global `gReservePlan` declared in human_assist.xs.
//==============================================================================

// Max simultaneous builders per damaged building.
const int cAutoRepair_MaxBuilders = 10;

// Buffer added to LOS in the coarse building-query radius. The query filters
// by center-to-center distance, but we want LOS to cover the building's edge.
// A buffer larger than the biggest building's obstruction radius (e.g. Wonder
// at ~6 tiles) ensures we don't miss visible big buildings in the pre-filter.
// The exact LOS gate is done per-candidate via kbUnitGetDistanceToUnit, which
// returns edge-to-edge distance.
const float cAutoRepair_LOSQueryBuffer = 10.0;

int gAutoRepair_villagerQuery  = -1;  // idle villagers (most civs)
int gAutoRepair_norseQuery     = -1;  // idle Norse soldier-builders
int gAutoRepair_buildingQuery  = -1;  // friendly buildings near a unit (position + radius set per call)

void autoRepair_setupQueries()
{
   if (gAutoRepair_villagerQuery == -1)
   {
      gAutoRepair_villagerQuery = kbUnitQueryCreate("autoRepair_villagers");
      kbUnitQuerySetPlayerID(gAutoRepair_villagerQuery, cMyID, false);
      kbUnitQuerySetUnitType(gAutoRepair_villagerQuery, cUnitTypeAbstractVillager);
      kbUnitQuerySetState(gAutoRepair_villagerQuery, cUnitStateAlive);
      kbUnitQuerySetActionType(gAutoRepair_villagerQuery, cActionTypeIdle);
   }
   if (gAutoRepair_norseQuery == -1)
   {
      // Same logical type vanilla AI uses for Norse soldier-builders in
      // core/buildings/buildings.xs Titan-Gate repair logic. Returns 0 for
      // non-Norse civs (no unit has the type).
      gAutoRepair_norseQuery = kbUnitQueryCreate("autoRepair_norseInfantry");
      kbUnitQuerySetPlayerID(gAutoRepair_norseQuery, cMyID, false);
      kbUnitQuerySetUnitType(gAutoRepair_norseQuery, cUnitTypeLogicalTypeNorseSoldierThatBuilds);
      kbUnitQuerySetState(gAutoRepair_norseQuery, cUnitStateAlive);
      kbUnitQuerySetActionType(gAutoRepair_norseQuery, cActionTypeIdle);
   }
   if (gAutoRepair_buildingQuery == -1)
   {
      // Position and maximum distance get set per call in autoRepair_tryAssign
      // (distance = the calling unit's LOS, so the unit only repairs what it
      // can actually see).
      gAutoRepair_buildingQuery = kbUnitQueryCreate("autoRepair_buildings");
      kbUnitQuerySetPlayerID(gAutoRepair_buildingQuery, cMyID, false);
      kbUnitQuerySetUnitType(gAutoRepair_buildingQuery, cUnitTypeBuilding);
      kbUnitQuerySetState(gAutoRepair_buildingQuery, cUnitStateAlive);
      kbUnitQuerySetAscendingSort(gAutoRepair_buildingQuery, true);  // closest first
   }
}

// Returns true if the unit can perform useful work on the given building's
// damaged state. Uses the engine's Build-rate table because that's what the
// engine actually consults for under-max-HP buildings -- vanilla chairon's
// `addBuilderTypesToPlan` (core/buildings/utilities_buildings.xs:635) confirms
// this by routing repair plans for House/Farm/OxCartBuilding to Norse
// villagers and everything else to NorseSoldierThatBuilds, even though the
// villagers' Repair protoaction's <rate> list only contains House. The Build
// protoaction's <rate> list is the load-bearing one for damaged-building work.
//
//   - Greek/Egyptian/Atlantean/Chinese/Aztec/Japanese villagers: Build rate
//     against <Building> (catch-all) -> > 0 for any building.
//   - Norse villagers: Build rate against Farm, House, OxCartBuilding -> > 0
//     for those, 0 for TC/Storehouse/Temple/etc.
//   - Norse soldier-builders: Build rate against <Building> -> > 0 for any.
//   - Freyr Norse villagers (if tech extends rates at runtime): the API
//     reflects current player state, so Freyr-specific additions are auto-
//     included.
bool autoRepair_unitCanRepairTarget(int unitID = -1, int buildingProtoID = -1)
{
   if (unitID < 0 || buildingProtoID < 0) { return(false); }
   int unitProtoID = kbUnitGetProtoUnitID(unitID);
   return(kbProtoUnitGetBuildRate(cMyID, unitProtoID, buildingProtoID) > 0.0);
}

// Tries to assign one idle unit to a nearby damaged building, either by joining
// an existing repair plan or creating a new one. Returns true if assigned.
//
// builderType is the unit-type SLOT added to the plan -- not a specific unit
// ID. This matches the vanilla pattern used by `addBuilderTypesToPlan` in
// core/buildings/utilities_buildings.xs, where the plan declares a slot and
// the engine fills it from the matching idle pool. Specific-unit assignment
// via aiPlanAddUnit alone (without a type slot) does not work.
bool autoRepair_tryAssign(int unitID = -1, int builderType = -1, string kind = "Unknown")
{
   if (unitID < 0) { return(false); }

   // Restrict candidates to what THIS unit can see. LOS in AoMR is a circular
   // radius around the unit. The query's maxDistance filter is center-to-
   // center, so we use LOS + buffer here to make sure big buildings (TC,
   // Wonder, Fortress) whose centers are several tiles inside the footprint
   // aren't excluded just because their CENTER is past LOS while their EDGE
   // is within LOS. The precise LOS gate is the per-building edge-distance
   // check below via kbUnitGetDistanceToUnit (which returns edge-to-edge).
   vector pos = kbUnitGetPosition(unitID);
   float los = kbUnitGetStatFloat(unitID, cUnitStatLOS);
   kbUnitQuerySetPosition(gAutoRepair_buildingQuery, pos);
   kbUnitQuerySetMaximumDistance(gAutoRepair_buildingQuery, los + cAutoRepair_LOSQueryBuffer);
   kbUnitQueryResetResults(gAutoRepair_buildingQuery);
   int buildingCount = kbUnitQueryExecute(gAutoRepair_buildingQuery);
   if (buildingCount <= 0) { return(false); }

   for (int j = 0; j < buildingCount; j++)
   {
      int buildingID = kbUnitQueryGetResult(gAutoRepair_buildingQuery, j);
      if (buildingID < 0) { continue; }

      // Precise LOS gate: kbUnitGetDistanceToUnit returns EDGE-to-edge
      // distance (includes both units' obstruction radii), so a unit standing
      // adjacent to a Town Center sees distance ~= 0 even though the TC's
      // center is several tiles into the footprint.
      float edgeDistance = kbUnitGetDistanceToUnit(unitID, buildingID);
      if (edgeDistance > los) { continue; }

      // Damage detection: same pattern vanilla AI uses for fortress repair in
      // core/buildings/buildings.xs (kbUnitGetStatFloat(unitID, cUnitStatHPRatio) < 1.0).
      // HPRatio is current/max HP, range 0.0..1.0; below 1.0 means damaged.
      float hpRatio = kbUnitGetStatFloat(buildingID, cUnitStatHPRatio);
      if (hpRatio >= 0.999) { continue; }

      // Skip buildings flagged non-repairable (e.g. some special-case unit
      // types). Vanilla `core/buildings/buildings.xs` line 1345 uses this same
      // check before creating a repair plan.
      int buildingProtoID = kbUnitGetProtoUnitID(buildingID);
      if (kbPlayerGetProtoStatFlag(cMyID, buildingProtoID, cProtoUnitFlagRepairable) == false) { continue; }

      // Skip buildings the unit can't actually perform Repair on (e.g. Norse
      // villagers can only repair Houses, not Town Centers/Storehouses/etc.
      // by default).
      if (autoRepair_unitCanRepairTarget(unitID, buildingProtoID) == false) { continue; }

      // Skip buildings the unit can't path to. Without this, units endlessly
      // attempt to reach unreachable targets -- common for towers surrounded
      // by houses, or buildings on isolated terrain. kbCanPath only checks
      // structural pathing (ignores moveable obstructions like units), so
      // transient blockers don't cause false negatives.
      vector buildingPos = kbUnitGetPosition(buildingID);
      int unitProto = kbUnitGetProtoUnitID(unitID);
      if (kbCanPath(pos, buildingPos, unitProto, 1.0, buildingID) == false) { continue; }

      // Skip warzones: don't suicide-march workers into hot areas. Vanilla
      // `core/buildings/buildings.xs` line 1274 uses the same threshold.
      int areaID = kbUnitGetAreaID(buildingID);
      if (kbAreaGetDangerLevel(areaID, false) >= 100.0) { continue; }

      int existingPlan = aiPlanGetIDByTypeAndVariableIntValue(cPlanRepair, cRepairPlanTargetID, buildingID);
      if (existingPlan >= 0)
      {
         int existingUnits = aiPlanGetNumberUnits(existingPlan);
         if (existingUnits == 0)
         {
            // Orphaned plan (e.g. cancelled mid-repair). Destroy and recreate.
            aiPlanDestroy(existingPlan);
            existingPlan = -1;
         }
         else if (existingUnits >= cAutoRepair_MaxBuilders)
         {
            continue;  // plan already at max builders; try the next building
         }
         else
         {
            // Add this unit to the existing plan. If the unit's type isn't in
            // any of the plan's slots (e.g. Berserker joining a villager-
            // -created plan), aiPlanAddUnit returns false -- add the missing
            // type slot and retry.
            bool addedToExisting = aiPlanAddUnit(existingPlan, unitID);
            if (addedToExisting == false)
            {
               aiPlanAddUnitType(existingPlan, builderType, 1, cAutoRepair_MaxBuilders, cAutoRepair_MaxBuilders);
               addedToExisting = aiPlanAddUnit(existingPlan, unitID);
            }
            if (addedToExisting == true) { return(true); }
            continue;
         }
      }

      // Create a fresh repair plan for this building.
      // Parent = gReservePlan: villagers parked in vanilla VPS reserve get
      // auto-loaned via aiPlanAddUnit. Non-reserved units (Norse infantry)
      // are direct-assigned with no loaning -- benign.
      int planID = aiPlanCreate("autoRepair " + buildingID, cPlanRepair, gReservePlan, -1);
      if (planID < 0) { continue; }
      aiPlanSetVariableInt(planID, cRepairPlanTargetID, 0, buildingID);
      aiPlanSetPriority(planID, 70);
      // Engine auto-destroys the plan when its last unit leaves, so the
      // watchdog yanking the last unit (e.g. on player override) cleanly
      // tears the plan down without our intervention.
      aiPlanSetFlag(planID, cPlanFlagDestroyWhenNoUnitsLeft, true);
      int mainBaseID = kbBaseGetMainID(cMyID);
      if (mainBaseID >= 0) { aiPlanSetBaseID(planID, mainBaseID); }
      // aiPlanAddUnitType MUST come before aiPlanAddUnit per docs -- the plan
      // rejects a unit that doesn't fit any declared type slot.
      aiPlanAddUnitType(planID, builderType, 1, cAutoRepair_MaxBuilders, cAutoRepair_MaxBuilders);
      aiPlanAddUnit(planID, unitID);
      return(true);
   }
   return(false);
}

// Watchdog: walk every cPlanRepair plan; drop units whose current action
// target no longer matches the plan's target. This is what makes player
// overrides "win" -- when a player redirects an auto-repairing unit to do
// anything else, this watchdog detects the target divergence and removes
// the unit from our plan (which then auto-destroys via cPlanFlagDestroyWhenNoUnitsLeft
// once its last unit leaves).
//
// Brief Idle is allowed -- the unit may be transitioning between Move and
// Repair frames within the plan's normal flow.
void autoRepair_watchdog()
{
   int planCount = aiPlanGetNumberByType(cPlanRepair);
   for (int p = 0; p < planCount; p++)
   {
      int planID = aiPlanGetIDByTypeIndex(cPlanRepair, p);
      if (planID < 0) { continue; }

      int planTarget = aiPlanGetVariableInt(planID, cRepairPlanTargetID, 0);

      int[] units = aiPlanGetUnits(planID);
      for (int u = 0; u < units.size(); u++)
      {
         int unitID = units[u];
         int action = kbUnitGetActionType(unitID);

         // Brief Idle: leave alone, transitional.
         if (action == cActionTypeIdle) { continue; }

         // Repair-like actions only count as "in plan" if the unit's current
         // target IS the plan's target. A Move toward a different point or
         // unit means the player redirected, so we drop the unit.
         bool repairLikeAction = (action == cActionTypeRepair) ||
                                 (action == cActionTypeMove)   ||
                                 (action == cActionTypeBuild)  ||
                                 (action == cActionTypeWork);
         int unitTarget = kbUnitGetTargetUnitID(unitID);
         if (repairLikeAction == true && unitTarget == planTarget) { continue; }

         aiPlanRemoveUnit(planID, unitID);
      }
   }
}

// Watchdog runs every game update so we beat the engine's plan-system
// re-task cycle. Per XS docs, each game update the script's rules run first,
// THEN the engine processes plans (including unit re-task). Running the
// watchdog on every frame at high priority means we remove player-redirected
// units BEFORE the plan's Repair task gets re-issued.
//
// Body is cheap (a handful of KB queries per active plan); fits well within
// the per-AI ~5 ms per-frame script budget.
rule autoRepair_watchdogRule
highFrequency
priority 80
active
{
   xsSetContextPlayer(cMyID);
   autoRepair_watchdog();
   xsSetContextPlayer(-1);
}

// Main rule: every 3 seconds, scan idle repair-capable units and assign them
// to nearby damaged buildings. Iterates ALL idle units per tick (no early
// return) so multiple builders converge on the same plan quickly when the
// player has many idle units.
rule autoRepair
minInterval 3
active
{
   xsSetContextPlayer(cMyID);
   autoRepair_setupQueries();

   // Pool 1: villagers. Culture-appropriate builder type for the plan slot.
   int villagerBuilderType = cUnitTypeAbstractVillager;
   if (cMyCulture == cCultureChinese) { villagerBuilderType = cUnitTypeVillagerChinese; }

   kbUnitQueryResetResults(gAutoRepair_villagerQuery);
   int villagerCount = kbUnitQueryExecute(gAutoRepair_villagerQuery);
   for (int i = 0; i < villagerCount; i++)
   {
      int villagerID = kbUnitQueryGetResult(gAutoRepair_villagerQuery, i);
      autoRepair_tryAssign(villagerID, villagerBuilderType, "Villager");
   }

   // Pool 2: Norse soldier-builders (Berserk, Hersir, Throwing Axeman, Jarl,
   // Huskarl, Raiding Cavalry, etc.). Returns 0 for non-Norse civs.
   kbUnitQueryResetResults(gAutoRepair_norseQuery);
   int norseCount = kbUnitQueryExecute(gAutoRepair_norseQuery);
   for (int n = 0; n < norseCount; n++)
   {
      int norseID = kbUnitQueryGetResult(gAutoRepair_norseQuery, n);
      autoRepair_tryAssign(norseID, cUnitTypeLogicalTypeNorseSoldierThatBuilds, "NorseInfantry");
   }

   xsSetContextPlayer(-1);
}
