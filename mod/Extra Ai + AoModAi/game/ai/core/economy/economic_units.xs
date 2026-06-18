//==============================================================================
/* economy.xs

   This file is intended for economic units related stuffs.

*/
//==============================================================================

////////////////////////////////////////////////////////////////////////////////
const int cStartDeletingGatherersThreshold = 8; // Around 4 minutes of idles before we start deleting them.
int gUnassignedGatherers = 0;
int gNumberTimesWeHaveUnassignedGatherers = 0;
int gMaxNumberVillagersDueToIdle = -1;
//==============================================================================
// getBaseVillagerCountToMaintainOtherAges
//==============================================================================
int getBaseVillagerCountToMaintainOtherAges()
{
   return selectByDifficulty(16, 30, 50, 75, 100, 100);
}

//==============================================================================
// getVillagerCountToMaintainOtherAges
//==============================================================================
int getVillagerCountToMaintainOtherAges()
{
   int wantedVilsOtherAges = getBaseVillagerCountToMaintainOtherAges();
   // Reduce this number by how many Caravans / Fishing Ships we have.
   int numCaravans = kbUnitCount(gCaravanUnit, cMyID, cUnitStateABQ);
   int numFishingShips = kbUnitCount(gFishingUnit, cMyID, cUnitStateABQ);
   debugEconomicUnits("Reducing wantedVilsOtherAges(" + wantedVilsOtherAges + ") by " + (numCaravans + numFishingShips) +
      " because of existing Caravans + Fishing Ships.");
   wantedVilsOtherAges -= (numCaravans + numFishingShips);

   if (gMaxNumberVillagersDueToIdle != -1 && wantedVilsOtherAges > gMaxNumberVillagersDueToIdle)
   {
      wantedVilsOtherAges = gMaxNumberVillagersDueToIdle;
      debugEconomicUnits("Capping wantedVilsOtherAges because of gMaxNumberVillagersDueToIdle, new value: " +
         gMaxNumberVillagersDueToIdle + ".");
   }

   return wantedVilsOtherAges;
}

//==============================================================================
// notifyPotentialIdleGatherers
//==============================================================================
void notifyPotentialIdleGatherers(int numIdles = 0)
{
   gUnassignedGatherers = numIdles;
   if (numIdles == 0)
   {
      if (gNumberTimesWeHaveUnassignedGatherers != 0)
      {
         debugEconomicUnits("We had unassigned gatherers but no longer, reseting the counter to start deleting excess gatherers.");
      }
      gNumberTimesWeHaveUnassignedGatherers = 0;
      gMaxNumberVillagersDueToIdle = -1;
   }
   else
   {
      gNumberTimesWeHaveUnassignedGatherers++;
      if (gNumberTimesWeHaveUnassignedGatherers == cStartDeletingGatherersThreshold)
      {
         int numGatherersAlive = kbUnitCount(cUnitTypeAbstractVillager, cMyID, cUnitStateAlive);
         gMaxNumberVillagersDueToIdle = numGatherersAlive - numIdles;
         debugEconomicUnits("!!! We've had idle gatherers for too long, we will start deleting excess gatherers now. !!!");
         debugEconomicUnits("We have " + gUnassignedGatherers + " idles and " + numGatherersAlive + " gatherers alive, " +
            "setting gMaxNumberVillagersDueToIdle to: " + gMaxNumberVillagersDueToIdle + ".");
         int minimumNeeded = cMyCulture == cCultureAtlantean ? 7 : 15;
         if (gMaxNumberVillagersDueToIdle < minimumNeeded)
         {
            debugEconomicUnits("Keeping a minimum of 15 gatherers alive for whatever may happen.");
            gMaxNumberVillagersDueToIdle = minimumNeeded;
         }
         // deleteExcessGatherers will modify gNumberTimesWeHaveUnassignedGatherers so we can hit this again later.
         xsRuleIgnoreIntervalOnce("deleteExcessGatherers");
         xsRuleIgnoreIntervalOnce("villagerMaintainMonitor"); // Potentially set new maintain numbers fast.
      }
      else
      {
         debugEconomicUnits("We have idle gatherers but not for long enough to start the deleting process: " +
            gNumberTimesWeHaveUnassignedGatherers + "/" + cStartDeletingGatherersThreshold + ".");
      }
   }
}

//==============================================================================
// deleteExcessGatherers
// If we've ran out of resource gathering spots for quite a bit of time we start deleting idles.
//==============================================================================
rule deleteExcessGatherers
active
minInterval 240
{
   int queryID = useSimpleUnitQuery(cUnitTypeAbstractVillager);
   int numResults = kbUnitQueryExecute(queryID);

   for (int i = 0; i < numResults; i++)
   {
      int villagerID = kbUnitQueryGetResult(queryID, i);
	  
      if (kbUnitGetStatBool(villagerID, cUnitStatIdleState) == true)
      {
         if (kbUnitGetPlanID(villagerID) != -1)
         {
            aiPlanRemoveUnitFromAllPlans(villagerID);
         }
         aiTaskDeleteUnit(villagerID);
      }
   }
}

//==============================================================================
// saveOxCartsMonitor
//==============================================================================
rule saveOxCartsMonitor
inactive
group defaultArchaicRules
minInterval 15
{
   if (cMyCulture != cCultureNorse)
   {
      xsDisableRule("saveOxCartsMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticOxCartTraining) == false)
   {
      return;
   }
   debugEconomicUnits("--- Running Rule saveOxCartsMonitor. ---");

   int queryID = useSimpleUnitQuery(cUnitTypeOxCart);
   int numOxCarts = kbUnitQueryExecute(queryID);
   int[] units = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < numOxCarts; i++)
   {
      if (kbUnitGetPlanID(units[i]) != -1)
      {
         continue; // Is already in a plan.
      }
      vector unitPosition = kbUnitGetPosition(units[i]);
      int tcID = getClosestUnitByLocationConnectedAreaGroup(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateAlive,
         unitPosition, 9999.0, cPassabilityLand);
      if (tcID == -1)
      {
         continue;
      }
      // Only send a move command if the Ox Cart is far away from a TC.
      if (xsVectorLength(unitPosition - kbUnitGetPosition(tcID)) >= 15.0)
      {
         aiTaskMoveUnit(units[i], kbUnitGetPosition(tcID));
      }
   }
}

//==============================================================================
// moveHerdablesToBase
// Moves all the herdables we own and aren't next to a Town Center to it.
// God powers can create herdables in whatever game we're in, so keep this running.
//==============================================================================
rule moveHerdablesToBase
inactive
group defaultArchaicRules
minInterval 5
{
   if (cStartingResourcesCurrent == cStartingResourcesInfinite)
   {
      xsDisableRule("moveHerdablesToBase");
      return;
   }
   if (cMyCiv == cCivDemeter) // Has custom logic in moveHerdablesToBaseDemeter.
   {
      xsDisableRule("moveHerdablesToBase");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticHerding) == false)
   {
      return;
   }
   debugEconomicUnits("--- Running Rule moveHerdablesToBase. ---");

   int queryID = useSimpleUnitQuery(cUnitTypeHerdable);
   int numResults = kbUnitQueryExecute(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int herdableID = kbUnitQueryGetResult(queryID, i);
      vector herdablePosition = kbUnitGetPosition(herdableID);
      int townCenterID = getClosestUnitByLocationConnectedAreaGroup(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateABQ,
         herdablePosition, cMaxFloat, cPassabilityLand);
      if (townCenterID == -1)
      {
         debugEconomicUnits("Couldn't find a Town Center to move herdable(" + herdableID + ") to.");
         continue;
      }
      float distanceFromTC = xsVectorLength(kbUnitGetPosition(townCenterID) - herdablePosition);
      // Anything further away than 13 distance should be sent to the Town Center.
      // This distance means that herdables that are properly next to the Town Center won't be picked up.
      if (distanceFromTC > 13.0)
      {
         debugEconomicUnits("Sent herdable: " + herdableID + " to Town Center: " + townCenterID + ".");
         aiTaskWorkUnit(herdableID, townCenterID);
      }
   }

   queryID = useSimpleUnitQuery(cUnitTypeAbstractSocketedTownCenter);
   numResults = kbUnitQueryExecute(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int tcID = kbUnitQueryGetResult(queryID, i);
      kbResourceCombineHerdableResourcesAroundUnit(tcID, 13.0);
   }
}

//==============================================================================
// moveHerdablesToBaseDemeter
// Moves all the herdables we own and aren't next to a Temple to it.
// God powers can create herdables in whatever game we're in, so keep this running.
//==============================================================================
rule moveHerdablesToBaseDemeter
inactive
group defaultArchaicRules
minInterval 5
{
   if (cStartingResourcesCurrent == cStartingResourcesInfinite)
   {
      xsDisableRule("moveHerdablesToBaseDemeter");
      return;
   }
   if (cMyCiv != cCivDemeter)
   {
      xsDisableRule("moveHerdablesToBaseDemeter");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticHerding) == false)
   {
      return;
   }
   debugEconomicUnits("--- Running Rule moveHerdablesToBaseDemeter. ---");

   const float range = 13.0;
   const int maxHerdablesPerTemple = 10;

   int templeQueryID = useSimpleUnitQuery(cUnitTypeAbstractTemple);
   int numTemples = kbUnitQueryExecute(templeQueryID);
   int[] temples = kbUnitQueryGetResults(templeQueryID);
   int[] templeAreaGroupIDs = new int(numTemples, -1);
   int[] herdablesNextToTemples = new int(numTemples, 0);
   for (int i = 0; i < numTemples; i++)
   {
      templeAreaGroupIDs[i] = kbUnitGetAreaGroupID(temples[i]);
   }

   int queryID = useSimpleUnitQuery(cUnitTypeHerdable);
   int numResults = kbUnitQueryExecute(queryID);
   bool[] herdableAlreadyWorkingOnTemple = new bool(numResults, false);
   for (int i = 0; i < numResults; i++)
   {
      int herdableID = kbUnitQueryGetResult(queryID, i);
      int targetID = kbUnitGetTargetUnitID(herdableID);
      int index = temples.find(targetID);
      if (index >= 0)
      {
         herdablesNextToTemples[index] = herdablesNextToTemples[index] + 1;
         herdableAlreadyWorkingOnTemple[i] = true; // These aren't tasked again.
      }
   }
   for (int i = 0; i < numTemples; i++)
   {
      debugEconomicUnits("Temple: " + temples[i] + " is on areaGroupID: " + templeAreaGroupIDs[i] + " and already has " +
         herdablesNextToTemples[i] + " Herdables next to/pathing to it.");
   }

   for (int i = 0; i < numResults; i++)
   {
      int herdableID = kbUnitQueryGetResult(queryID, i);
      if (herdableAlreadyWorkingOnTemple[i] == true)
      {
         continue;
      }

      bool foundTemple = false;
      int herdableAreaGroupID = kbUnitGetAreaGroupID(herdableID);
      for (int j = 0; j < numTemples; j++)
      {
         if (kbPathAreAreaGroupsConnected(herdableAreaGroupID, templeAreaGroupIDs[j], cPassabilityLand) == false)
         {
            continue;
         }
         if (herdablesNextToTemples[j] >= maxHerdablesPerTemple)
         {
            continue;
         }
         herdablesNextToTemples[j] = herdablesNextToTemples[j] + 1;
         debugEconomicUnits("Sent herdable: " + herdableID + " to Temple: " + temples[j] + ".");
         aiTaskWorkUnit(herdableID, temples[j]);
         foundTemple = true;
         break;
      }
      if (foundTemple == true)
      {
         continue;
      }

      debugEconomicUnits("Couldn't find a valid Temple for Herdable " + herdableID + ", trying backup plan now.");

      vector herdablePosition = kbUnitGetPosition(herdableID);
      int townCenterID = getClosestUnitByLocationConnectedAreaGroup(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateABQ,
         herdablePosition, cMaxFloat, cPassabilityLand);
      if (townCenterID == -1)
      {
         debugEconomicUnits("Couldn't find a Town Center to move herdable(" + herdableID + ") to either, it will idle now!");
         continue;
      }
      float distanceFromTC = xsVectorLength(kbUnitGetPosition(townCenterID) - herdablePosition);
      // Anything further away than 13 distance should be sent to the Town Center.
      // This distance means that herdables that are properly next to the Town Center won't be picked up.
      if (distanceFromTC > range)
      {
         debugEconomicUnits("Sent herdable: " + herdableID + " to Town Center: " + townCenterID + ".");
         aiTaskWorkUnit(herdableID, townCenterID);
      }
      else
      {
         debugEconomicUnits("Herdable " + herdableID + " is already close to a Town Center.");
      }
   }

   for (int i = 0; i < numTemples; i++)
   {
      kbResourceCombineHerdableResourcesAroundUnit(temples[i], range);
   }
}

//==============================================================================
// getFishingShipCountToMaintainOtherAges
//==============================================================================
int getFishingShipCountToMaintainOtherAges()
{
   return selectByDifficulty(3, 5, 7, 10, 12, 15);
}

bool gHaveValidFishingDock = false;
//==============================================================================
// fishingShipMaintainMonitor
// Updates the Fishing Ship maintain plan.
//==============================================================================
rule fishingShipMaintainMonitor
inactive
group defaultArchaicRules
priority 80
minInterval 30
{
   if (gMapInfo.mHasFish == false)
   {
      debugEconomicUnits("Map is not suited for fishing for us, disabling fishingShipMaintainMonitor.");
      xsDisableRule("fishingShipMaintainMonitor");
      return;
   }
   // If we're in a campaign mission we can have the situation where we want to fish but not build new Docks. In that situation
   // gShouldFish is always false, but we still need to run this logic.
   if (checkStrategyFlag(cStrategyFlagAutomaticFishing) == false ||
       (gShouldFish == false && gOverrideClosestFishLocation == cInvalidVector))
   {
      if (aiPlanGetIsIDValid(gFishingShipMaintainPlan) == true)
      {
         aiPlanDestroy(gFishingShipMaintainPlan);
      }
      gFishingShipMaintainPlan = -1;
      return;
   }
   debugEconomicUnits("--- Running Rule fishingShipMaintainMonitor. ---");

   int dockID = -1;
   if (gDockManager.mValidDockIDs.size() > 0)
   {
      dockID = gDockManager.mValidDockIDs[0];
   }
   else if (gOverrideClosestFishLocation != cInvalidVector) // Handle campaigns.
   {
      dockID = getClosestUnitByLocationConnectedAreaGroup(cUnitTypeAbstractDock, cMyID, cUnitStateAlive,
         gOverrideClosestFishLocation, cMaxFloat, cPassabilityWater);
   }

   if (dockID == -1)
   {
      debugEconomicUnits("Found no valid Dock to anchor our fishing around, not training any Fishing Ships now.");
      if (aiPlanGetIsIDValid(gFishingShipMaintainPlan) == true)
      {
         aiPlanSetVariableInt(gFishingShipMaintainPlan, cTrainPlanNumberToMaintain, 0, 0);
      }
      gHaveValidFishingDock = false;
      return;
   }

   gHaveValidFishingDock = true;
   // We need to make sure our closest Dock is attached to our fish plan for optimal functionality.
   if (aiPlanGetIsIDValid(gFishingPlan) == true)
   {
      aiPlanSetVariableInt(gFishingPlan, cFishPlanDockID, 0, dockID);
   }

   int numWantedShipsThisAge = 0;

   if (gOverrideMaxFishingShipPop >= 0)
   {
      debugEconomicUnits("Found an override for Fishing Ship maintain, setting it to: " + gOverrideMaxFishingShipPop + ".");
      numWantedShipsThisAge = gOverrideMaxFishingShipPop;
   }
   else
   {
      int age = kbPlayerGetAge(cMyID);
      static int numWantedShipsArchaicAge = -1;
      static int numWantedShipsOtherAges = -1;
      if (numWantedShipsArchaicAge == -1)
      {
         numWantedShipsArchaicAge = selectByDifficulty(2, 3, 5, 7, 9, 9);
         numWantedShipsOtherAges = getFishingShipCountToMaintainOtherAges();
         if (cGameModeCurrent == cGameModeLightning)
         {
            numWantedShipsArchaicAge /= 2;
            numWantedShipsOtherAges /= 2;
         }
      }
      int buildLimit = kbPlayerGetProtoStatInt(cMyID, gFishingUnit, cProtoStatBuildLimit);
      if (buildLimit >= 0)
      {
         if (numWantedShipsArchaicAge > buildLimit)
         {
            aiEchoWarning("Trying to maintain more Fishing Ships in Archaic than our build limit allows: " + numWantedShipsArchaicAge
               + "/" + buildLimit + ".");
            numWantedShipsArchaicAge = buildLimit;
         }
         if (numWantedShipsOtherAges > buildLimit)
         {
            aiEchoWarning("Trying to maintain more Fishing Ships in >= Classical than our build limit allows: "
               + numWantedShipsOtherAges + "/" + buildLimit + ".");
            numWantedShipsOtherAges = buildLimit;
         }
      }
      numWantedShipsThisAge = age == cAge1 ? numWantedShipsArchaicAge : numWantedShipsOtherAges;

      int numberSuitableFishingSpots = kbGetNumberValidResourcesByPosition(gWaterDefendPoint, cResourceFood,
         cAIResourceSubTypeFish, 50.0) * 3; // Every Fish KB resource has 3 spots normally. TODO this can't handle single spots atm.
      debugEconomicUnits("Found " + numberSuitableFishingSpots + " available fishing spots in range of our Dock.");

      if (numWantedShipsThisAge > numberSuitableFishingSpots)
      {
         numWantedShipsThisAge = numberSuitableFishingSpots;
      }

      int fishingBoatQuery = useSimpleUnitQuery(gFishingUnit, cMyID, cUnitStateAlive);
      kbUnitQuerySetActionType(fishingBoatQuery, cActionTypeIdle);

      // We have idle ships indicating we don't know what to use them for so don't train more.
      if (kbUnitQueryExecute(fishingBoatQuery) > 1)
      {
         debugEconomicUnits("We found idle Fishing Ships, not training any more now.");
         numWantedShipsThisAge = 0;
      }

      if (gMapInfo.mIsIslandMap == true && numWantedShipsThisAge < 1)
      {
         debugEconomicUnits("We were planning on training 0 Fishing Ships but this is an island map, always training 1 for scouting.");
         numWantedShipsThisAge = 1; // Train at least 1 Fishing Ship on these maps to explore.
      }
   }

   if (aiPlanGetIsIDValid(gFishingShipMaintainPlan) == false)
   {
      gFishingShipMaintainPlan = createSimpleMaintainPlan(gFishingUnit, numWantedShipsThisAge, gDockAreaGroupID,
         gEconomicUnitsCategoryID, 50, -1, cUnitTypeAbstractDock, true);
      aiPlanSetName(gFishingShipMaintainPlan, gFishingShipMaintainPlan + " Maintain " + numWantedShipsThisAge + " " +
         kbProtoUnitGetName(gFishingUnit));
   }
   else
   {
      // Keep updating this area group since it might change.
      aiPlanSetVariableInt(gFishingShipMaintainPlan, cTrainPlanAreaGroupID, 0, gDockAreaGroupID);
      if (aiPlanGetVariableInt(gFishingShipMaintainPlan, cTrainPlanNumberToMaintain, 0) != numWantedShipsThisAge)
      {
         debugEconomicUnits("Adjusting " + kbProtoUnitGetName(gFishingUnit) + " Maintain plan to maintain: " + numWantedShipsThisAge + ".");
         aiPlanSetVariableInt(gFishingShipMaintainPlan, cTrainPlanNumberToMaintain, 0, numWantedShipsThisAge);
         aiPlanSetName(gFishingShipMaintainPlan, gFishingShipMaintainPlan + " Maintain " + numWantedShipsThisAge + " " +
            kbProtoUnitGetName(gFishingUnit));
      }
   }

   // Priority based on how many Fishing Ships we already have alive, below 80% alive we have high prio.
   if (kbUnitCount(gFishingUnit, cMyID, cUnitStateAlive) >= (numWantedShipsThisAge * 0.8))
   {
      aiPlanSetPriority(gFishingShipMaintainPlan, 50);
   }
   else
   {
      aiPlanSetPriority(gFishingShipMaintainPlan, 70);
   }
}

//==============================================================================
// LykaonMaintainMonitor
// Train a few Lykaons for the sake of it.
//==============================================================================
rule LykaonMaintainMonitor
inactive
group defaultHeroicRules
minInterval 30
{
   if (cStartingResourcesCurrent == cStartingResourcesInfinite)
   {
      xsDisableRule("LykaonMaintainMonitor");
      return;
   }
   if (cMyCiv != cCivDemeter)
   {
      xsDisableRule("LykaonMaintainMonitor");
      return;
   }
   if (kbProtoUnitAvailable(cUnitTypeLykaonVillager) == false)
   {
      xsDisableRule("LykaonMaintainMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticVillagerTraining) == false)
   {
      return;
   }

   if (aiPlanGetNumberByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, cUnitTypeLykaonVillager) > 0)
   {
      debugEconomicUnits("We already have a train plan for more Lykaon Villagers, can't stack multiple.");
      return;
   }

   int maxLykaons = selectByDifficulty(1, 2, 3, 4, 5, 6);
   int currentLykaonCount = kbUnitCount(cUnitTypeLykaonVillager, cMyID, cUnitStateAlive);
   if (currentLykaonCount >= maxLykaons)
   {
      debugEconomicUnits("We already have " + currentLykaonCount + "/" + maxLykaons + " Lykaons, not training more.");
      return;
   }
   // Train 1 at a time.
   createSimpleTrainPlan(cUnitTypeLykaonVillager, 1, gLandAreaGroupID, gEconomicUnitsCategoryID);
}

//==============================================================================
// oxCartMaintainMonitor
// Ox Carts don't take population, so no limits to take into account.defaul
//==============================================================================
rule oxCartMaintainMonitor
inactive
group defaultArchaicRules
minInterval 5
{
   if (cStartingResourcesCurrent == cStartingResourcesInfinite)
   {
      xsDisableRule("oxCartMaintainMonitor");
      return;
   }
   if (cMyCulture != cCultureNorse)
   {
      xsDisableRule("oxCartMaintainMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticOxCartTraining) == false)
   {
      return;
   }
   debugEconomicUnits("--- Running Rule oxCartMaintainMonitor. ---");

   int[] gatherPlans = aiPlanGetIDsByType(cPlanGather);
   int[] plansThatRequireOxCart = new int(0, -1);
   for (int i = 0; i < gatherPlans.size(); i++)
   {
      // These types require no Ox Carts.
      if (aiPlanGetVariableInt(gatherPlans[i], cGatherPlanResourceSubType, 0) == cAIResourceSubTypeFarm ||
          aiPlanGetVariableInt(gatherPlans[i], cGatherPlanResourceSubType, 0) == cAIResourceSubTypeFish||
          aiPlanGetVariableInt(gatherPlans[i], cGatherPlanResourceSubType, 0) == cAIResourceSubTypeHerdable)
      {
         debugEconomicUnits(aiPlanGetName(gatherPlans[i]) + " doesn't require an Ox Cart due to resource subtype.");
         continue;
      }
      if (aiPlanGetVariableInt(gatherPlans[i], cGatherPlanKBResourceID, 0) == -1)
      {
         debugEconomicUnits(aiPlanGetName(gatherPlans[i]) + " can't get an Ox Cart yet due to invalid KB Resource ID.");
         continue;
      }
      // Do we already have an Ox Cart? Count loans because we could be in a garrison child plan situation.
      int[] planUnits = aiPlanGetUnits(gatherPlans[i], cUnitTypeOxCart, true);
      if (planUnits.size() >= 1)
      {
         debugEconomicUnits(aiPlanGetName(gatherPlans[i]) + " already has an Ox Cart.");
         continue;
      }
      int numChildren = aiPlanGetNumberChildren(gatherPlans[i]);
      bool foundOxCartBuildPlan = false;
      for (int iChild = 0; iChild < numChildren; iChild++)
      {
         int childPlanID = aiPlanGetChildIDByIndex(gatherPlans[i], iChild);
         if (aiPlanGetType(childPlanID) != cPlanBuild)
         {
            continue;
         }
         if (aiPlanGetVariableInt(childPlanID, cBuildPlanBuildingTypeID, 0) == cUnitTypeOxCartBuilding)
         {
            foundOxCartBuildPlan = true;
            break;
         }
      }
      if (foundOxCartBuildPlan == true)
      {
         debugEconomicUnits(aiPlanGetName(gatherPlans[i]) + " already has an Ox Cart build plan going.");
         continue;
      }
      // Finally, this gather plan needs an Ox Cart!
      plansThatRequireOxCart.add(gatherPlans[i]);
   }

   if (plansThatRequireOxCart.size() == 0)
   {
      debugEconomicUnits("Found no gather plans that require an Ox Cart build plan.");
      return;
   }

   // Try to find idles and assign them to the closest gather plan.
   int queryID = useSimpleUnitQuery(cUnitTypeOxCart);
   int numOxCarts = kbUnitQueryExecute(queryID);
   int[] units = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < numOxCarts; i++)
   {
      if (kbUnitGetPlanID(units[i]) != -1)
      {
         continue; // Is already in a plan.
      }

      float closestDistance = cMaxFloat;
      int closestPlanID = -1;
      vector oxCartPosition = kbUnitGetPosition(units[i]);
      int[] forbiddenList = aiUnitGetForbiddenPlanIDs(units[i]);
      for (int iPlan = 0; iPlan < plansThatRequireOxCart.size(); iPlan++)
      {
         if (forbiddenList.find(plansThatRequireOxCart[iPlan]) != -1)
         {
            debugEconomicUnits("Ox Cart(" + units[i] + ") is in the forbidden list of " + aiPlanGetName(plansThatRequireOxCart[iPlan]) + ".");
            continue;
         }
         int resourceID = aiPlanGetVariableInt(plansThatRequireOxCart[iPlan], cGatherPlanKBResourceID, 0);
         // Since we need to do pathing we need to fetch the first unit, the center position could be in another area group etc...
         int firstUnitID = kbResourceGetUnit(resourceID, 0);
         if (kbUnitGetIsIDValid(firstUnitID, true) == false)
         {
            continue;
         }
         vector resourcePosition = kbUnitGetPosition(firstUnitID);
         float distance = xsVectorDistanceSqr(oxCartPosition, resourcePosition);
         if (distance < closestDistance)
         {
            if (kbCanPath(oxCartPosition, resourcePosition, cUnitTypeOxCart, 1.0) == false)
            {
               debugEconomicUnits("Ox Cart(" + units[i] + ") can't path to " + aiPlanGetName(plansThatRequireOxCart[iPlan]) + ".");
               continue;
            }
            distance = closestDistance;
            closestPlanID = plansThatRequireOxCart[iPlan];
         }
      }
      if (closestPlanID == -1)
      {
         debugEconomicUnits("Couldn't find a suitable plan to add Ox Cart(" + units[i] + ") to.");
         continue;
      }

      aiPlanAddUnit(closestPlanID, units[i]);
      debugEconomicUnits("Assigning idle Ox Cart " + units[i] + " to " + aiPlanGetName(closestPlanID) + ".");
      plansThatRequireOxCart.removeValue(closestPlanID);
      if (plansThatRequireOxCart.size() == 0)
      {
         return;
      }
   }

   for (int i = 0; i < plansThatRequireOxCart.size(); i++)
   {
      if (aiPlanGetNumberUnits(plansThatRequireOxCart[i], -1, false) == 0)
      {
         debugEconomicUnits(aiPlanGetName(plansThatRequireOxCart[i]) + " can't create a build plan because the plan has no units in it.");
         continue;
      }
      debugEconomicUnits("Creating an Ox Cart build plan for " + aiPlanGetName(plansThatRequireOxCart[i]) + ".");
      int planID = aiPlanCreate("Build Dropsite", cPlanBuild, plansThatRequireOxCart[i], gEconomicBuildingsCategoryID);
      aiPlanSetVariableInt(planID, cBuildPlanBuildingTypeID, 0, cUnitTypeOxCartBuilding);
      // Prevents the builers we assign in selectDropsitePlacement being kicked out by the auto assignment before we have a foundation.
      aiPlanSetFlag(planID, cPlanFlagReadyForUnits, true);
      selectDropsitePlacement(planID);
   }
}

//==============================================================================
// getCaravanInformationToMaintain
// In this function we analyze in what kind of situation we are in relation to trading/gold income.
// Based on that we set caravans numbers.
// We must be really careful with our caravan maintain numbers because they're very expensive and can drain our eco quickly.
//==============================================================================
void getCaravanInformationToMaintain(ref int maintainAmount, ref int trainDelay)
{
   // If we don't have a valid trade route we don't train anything.
   if (tradeInformation.mState != cTradeStateTrading)
   {
      maintainAmount = 0;
      trainDelay = 0;
      return;
   }
   // If we haven't ran out of gold resources yet, or not for long enough, we maintain a low amount with a train delay.
   if (gCantFindGoldResourcesCounter <= 2) // Need to have ran out for at least 1 minute.
   {
      maintainAmount = selectByDifficulty(2, 3, 5, 10, 12, 15);
      trainDelay = selectByDifficulty(40, 35, 30, 25, 20, 20);
      return;
   }
   maintainAmount = selectByDifficulty(3, 5, 10, 20, 25, 30);
   trainDelay = 0; // We need these Caravans now!
}

//==============================================================================
// caravanMaintainMonitor
//==============================================================================
rule caravanMaintainMonitor
inactive
group defaultHeroicRules
priority 79
minInterval 30
{
   if (cStartingResourcesCurrent == cStartingResourcesInfinite)
   {
      xsDisableRule("caravanMaintainMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagCanTrade) == false)
   {
      if (aiPlanGetIsIDValid(gCaravanMaintainPlan) == true)
      {
         aiPlanDestroy(gCaravanMaintainPlan);
      }
      gCaravanMaintainPlan = -1;
      return;
   }
   debugEconomicUnits("--- Running Rule caravanMaintainMonitor. ---");

   int buildLimit = kbPlayerGetProtoStatInt(cMyID, gCaravanUnit, cProtoStatBuildLimit);
   int numCaravansToTrain = 0;
   int trainDelay = 0;
   if (gOverrideMaxCaravanPop >= 0)
   {
      debugEconomicUnits("Override - Setting our wanted Caravans to: " + gOverrideMaxCaravanPop + ".");
      numCaravansToTrain = gOverrideMaxCaravanPop;
   }
   else
   {
      getCaravanInformationToMaintain(numCaravansToTrain, trainDelay);
      if (cGameModeCurrent == cGameModeLightning)
      {
         numCaravansToTrain /= 2;
         if (buildLimit >= 0)
         {
            numCaravansToTrain = min(numCaravansToTrain, buildLimit); // BL of 10 in Lightning.
         }
      }
      debugEconomicUnits("We want to maintain " + numCaravansToTrain + " Caravans with a train delay of: " + trainDelay + ".");
   }
   
   if (buildLimit >= 0 && numCaravansToTrain > buildLimit)
   {
      aiEchoWarning("Trying to maintain more Caravans than our build limit allows: " + numCaravansToTrain + "/" + buildLimit + ".");
      numCaravansToTrain = buildLimit;
   }

   // Create the plan if it isn't valid (anymore).
   if (aiPlanGetIsIDValid(gCaravanMaintainPlan) == false)
   {
      gCaravanMaintainPlan = createSimpleMaintainPlan(gCaravanUnit, numCaravansToTrain, -1, gTradeCategoryID, 50, -1, -1, true);
      // The Market ID (buildingID) gets updated dynamically by the trading logic, for now take the current (could be -1).
      // We also need to setNumber here explicitly because then we force the train plan to use our specific buildings.
      aiPlanSetNumberVariableValues(gCaravanMaintainPlan, cTrainPlanBuildingID, 1);
      aiPlanSetVariableInt(gCaravanMaintainPlan, cTrainPlanBuildingID, 0, tradeInformation.mCurrentMarketID);
      // Set the train delay.
      aiPlanSetVariableInt(gCaravanMaintainPlan, cTrainPlanFrequency, 0, trainDelay);
      // Make sure the train delay is actually respected.
      aiPlanSetVariableBool(gCaravanMaintainPlan, cTrainPlanUseMultipleBuildings, 0, false);
   }
   else
   {
      if (aiPlanGetVariableInt(gCaravanMaintainPlan, cTrainPlanNumberToMaintain, 0) != numCaravansToTrain)
      {
         debugEconomicUnits("Adjusting " + kbProtoUnitGetName(gCaravanUnit) + " Maintain plan to maintain: " + numCaravansToTrain + ".");
         aiPlanSetVariableInt(gCaravanMaintainPlan, cTrainPlanNumberToMaintain, 0, numCaravansToTrain);
         aiPlanSetName(gCaravanMaintainPlan, gCaravanMaintainPlan + " Maintain " + numCaravansToTrain + " " +
            kbProtoUnitGetName(gCaravanUnit));
      }
      if (aiPlanGetVariableInt(gCaravanMaintainPlan, cTrainPlanFrequency, 0) != trainDelay)
      {
         debugEconomicUnits("Adjusting " + kbProtoUnitGetName(gCaravanUnit) + " Maintain plan train delay to: " + trainDelay + ".");
         aiPlanSetVariableInt(gCaravanMaintainPlan, cTrainPlanFrequency, 0, trainDelay);
      }
   }

   // Priority based on how badly we need Caravans.
   int prio = gCantFindGoldResourcesCounter < 5 ? 50 : 70;
   if (aiPlanGetPriority(gCaravanMaintainPlan) != prio)
   {
      debugEconomicUnits("Adjusting Caravan maintain plan priority to: " + prio + ".");
      aiPlanSetPriority(gCaravanMaintainPlan, prio);
   }
}

//==============================================================================
// villagerMaintainMonitor
// Sets the Villager maintain plans.
//==============================================================================
rule villagerMaintainMonitor
inactive
group defaultArchaicRules
priority 78
minInterval 30
{
   if (checkStrategyFlag(cStrategyFlagAutomaticVillagerTraining) == false)
   {
      if (aiPlanGetIsIDValid(gVillagerMaintainPlan) == true)
      {
         aiPlanDestroy(gVillagerMaintainPlan);
      }
      gVillagerMaintainPlan = -1;
      if (aiPlanGetIsIDValid(gSecondVillagerMaintainPlan) == true)
      {
         aiPlanDestroy(gSecondVillagerMaintainPlan);
      }
      gSecondVillagerMaintainPlan = -1;
      return;
   }
   debugEconomicUnits("--- Running Rule villagerMaintainMonitor. ---");

   int wantedVilsThisAge = 0;
   int buildLimit = kbPlayerGetProtoStatInt(cMyID, gEconUnit, cProtoStatBuildLimit);

   // Take our override into account if requested. We take our secondary override into account in the culture specific sections.
   if (gOverrideMaxVillagerPop >= 0)
   {
      debugEconomicUnits("Override - Setting our wanted Vills to: " + gOverrideMaxVillagerPop + ".");
      wantedVilsThisAge = gOverrideMaxVillagerPop;
      if (gOverrideMaxVillagerPop > buildLimit)
      {
         aiEchoWarning("Override - Trying to maintain more Villagers than our build limit allows: " + wantedVilsThisAge + "/" +
            buildLimit + ".");
         wantedVilsThisAge = buildLimit;
      }
   }
   else
   {
      // Figure out how many Villagers we want to maintain based on current age + build limits.
      int wantedVilsArchaicAge = selectByDifficulty(16, 16, 20, 20, 20, 20);
      int wantedVilsOtherAges = getVillagerCountToMaintainOtherAges();

      // Don't train too many Villagers when we don't have to gather.
      if (cStartingResourcesCurrent == cStartingResourcesInfinite)
      {
         if (cMyCulture == cCultureNorse)
         {
            // We don't need vills to build as Norse, maybe some Houses.
            if (wantedVilsArchaicAge > 6 || wantedVilsOtherAges > 6)
            {
               debugEconomicUnits("Starting resources are set to infinite, capping Villager amount to 6.");
               wantedVilsArchaicAge = 6;
               wantedVilsOtherAges = 6;
            }
         }
         else if (wantedVilsArchaicAge > 16 || wantedVilsOtherAges > 16)
         {
            debugEconomicUnits("Starting resources are set to infinite, capping Villager amount to 16.");
            wantedVilsArchaicAge = 16;
            wantedVilsOtherAges = 16;
         }
      }

      if (cMyCulture == cCultureAtlantean)
      {
         wantedVilsArchaicAge /= 2;
         wantedVilsOtherAges /= 2;
         debugEconomicUnits("Dividing Villager numbers by 2 because we're Atlantean.");
      }
      if (cGameModeCurrent == cGameModeLightning)
      {
         wantedVilsArchaicAge /= 2;
         wantedVilsOtherAges /= 2;
         debugEconomicUnits("Dividing Villager numbers by 2 because we're playing lightning.");
      }

      // This only works since Gatherers have a 100 limit and we never go above this in total gatherers.
      // If done properly we would do this later once we determine if we're going to build > 1 types of Villagers.
      if (buildLimit >= 0)
      {
         if (wantedVilsArchaicAge > buildLimit)
         {
            aiEchoWarning("Trying to maintain more Villagers in Archaic than our build limit allows: " + wantedVilsArchaicAge +
               "/" + buildLimit + ".");
            wantedVilsArchaicAge = buildLimit;
         }
         if (wantedVilsOtherAges > buildLimit)
         {
            aiEchoWarning("Trying to maintain more Villagers in >= Classical than our build limit allows: " + wantedVilsOtherAges +
               "/" + buildLimit + ".");
            wantedVilsOtherAges = buildLimit;
         }
      }
      wantedVilsThisAge = kbPlayerGetAge(cMyID) == cAge1 ? wantedVilsArchaicAge : wantedVilsOtherAges;
      debugEconomicUnits("Setting our wanted Vills this age to: " + wantedVilsThisAge + ".");
   }

   // Culture specific parts.

   if (cMyCulture == cCultureGreek || cMyCulture == cCultureEgyptian || cMyCulture == cCultureAtlantean ||
       cMyCulture == cCultureJapanese || cMyCulture == cCultureAztec)
   {
      // Create the plan if it isn't valid (anymore).
      if (aiPlanGetIsIDValid(gVillagerMaintainPlan) == false)
      {
         gVillagerMaintainPlan = createSimpleMaintainPlan(gEconUnit, wantedVilsThisAge, gLandAreaGroupID, gEconomicUnitsCategoryID, 50,
            -1, -1, true);
      }
      // Adjust the name/maintain numbers if it differs from before.
      else if (aiPlanGetVariableInt(gVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0) != wantedVilsThisAge)
      {
         debugEconomicUnits("Adjusting " + kbProtoUnitGetName(gEconUnit) + " Maintain plan to maintain: " + wantedVilsThisAge + ".");
         aiPlanSetVariableInt(gVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0, wantedVilsThisAge);
         aiPlanSetName(gVillagerMaintainPlan, gVillagerMaintainPlan + ": Maintain " + wantedVilsThisAge + " " +
                       kbProtoUnitGetName(gEconUnit) + ".");
      }

      // Priority based on how many Villagers we already have alive, below 80% alive we have high prio.
      // Search for AbstractVillager to account for Hero Citizens.
      if (kbUnitCount(cUnitTypeAbstractVillager, cMyID, cUnitStateAlive) >= (wantedVilsThisAge * 0.8))
      {
         aiPlanSetPriority(gVillagerMaintainPlan, 50);
      }
      else
      {
         aiPlanSetPriority(gVillagerMaintainPlan, 70);
      }
   }

   // Norse
   else if (cMyCulture == cCultureNorse)
   {
      
      int plan1Number = 0;
      int plan2Number = 0;

      if (gOverrideMaxSecondaryVillagerCount >= 0)
      {
         // For the Gatherers assume another override was also used for those, so just assign wantedVilsThisAge.
         plan1Number = wantedVilsThisAge;
         plan2Number = gOverrideMaxSecondaryVillagerCount;
         debugEconomicUnits("Override - Setting our wanted Dwarfs to: " + plan2Number + ".");
         int dwarfLimit = kbPlayerGetProtoStatInt(cMyID, gEconUnit, cProtoStatBuildLimit);
         if (plan2Number > dwarfLimit)
         {
            aiEchoWarning("Override - Trying to maintain more Dwarfs than our build limit allows: " + plan2Number + "/" +
               dwarfLimit + ".");
            plan2Number = dwarfLimit;
         }
      }
      else
      {
         // Ceil one here to not lose a Villager.
         plan1Number = wantedVilsThisAge * 0.7;
         plan2Number = ceil(wantedVilsThisAge * 0.3);
         if (cMyCiv == cCivThor)
         {
            plan1Number = wantedVilsThisAge * 0.6;
            plan2Number = ceil(wantedVilsThisAge * 0.4);
         }
      }

      // Create the first plan if it isn't valid (anymore).
      if (aiPlanGetIsIDValid(gVillagerMaintainPlan) == false)
      {
         gVillagerMaintainPlan = createSimpleMaintainPlan(gEconUnit, plan1Number, gLandAreaGroupID, gEconomicUnitsCategoryID, 50,
            -1, -1, true);
      }
      // Adjust the name/maintain numbers if it differs from before.
      else if (aiPlanGetVariableInt(gVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0) != plan1Number)
      {
         debugEconomicUnits("Adjusting " + kbProtoUnitGetName(gEconUnit) + " Maintain plan to maintain: " + plan1Number + ".");
         aiPlanSetVariableInt(gVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0, plan1Number);
         aiPlanSetName(gVillagerMaintainPlan, gVillagerMaintainPlan + " Maintain " + plan1Number + " " +
                       kbProtoUnitGetName(gEconUnit));
      }

      // Create the second plan if it isn't valid (anymore).
      if (aiPlanGetIsIDValid(gSecondVillagerMaintainPlan) == false)
      {
         gSecondVillagerMaintainPlan = createSimpleMaintainPlan(cUnitTypeVillagerDwarf, plan2Number, gLandAreaGroupID,
            gEconomicUnitsCategoryID, 50, -1, -1, true);
      }
      // Adjust the name/maintain numbers if it differs from before.
      else if (aiPlanGetVariableInt(gSecondVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0) != plan2Number)
      {
         debugEconomicUnits("Adjusting " + kbProtoUnitGetName(cUnitTypeVillagerDwarf) + " Maintain plan to maintain: " + plan2Number + ".");
         aiPlanSetVariableInt(gSecondVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0, plan2Number);
         aiPlanSetName(gSecondVillagerMaintainPlan, gSecondVillagerMaintainPlan + ": Maintain " + plan2Number + " " +
                       kbProtoUnitGetName(cUnitTypeVillagerDwarf));
      }

      // Priority based on how many Villagers/Dwarfs we already have alive, below 80% alive we have high prio.
      if (kbUnitCount(gEconUnit, cMyID, cUnitStateAlive) >= (plan1Number * 0.8))
      {
         aiPlanSetPriority(gVillagerMaintainPlan, 50);
      }
      else
      {
         aiPlanSetPriority(gVillagerMaintainPlan, 70);
      }
      if (kbUnitCount(cUnitTypeVillagerDwarf, cMyID, cUnitStateAlive) >= (plan2Number * 0.8))
      {
         aiPlanSetPriority(gSecondVillagerMaintainPlan, 50);
      }
      else
      {
         aiPlanSetPriority(gSecondVillagerMaintainPlan, 70);
      }
   }

   // Chinese
   else
   {
      int plan1Number = 0;
      int plan2Number = 0;
      int kuafuBL = kbPlayerGetProtoStatInt(cMyID, cUnitTypeKuafu, cProtoStatBuildLimit);
      if (gOverrideMaxSecondaryVillagerCount >= 0)
      {
         // For the Peasants assume another override was also used for those, so just assign wantedVilsThisAge.
         plan1Number = wantedVilsThisAge;
         plan2Number = gOverrideMaxSecondaryVillagerCount;
         debugEconomicUnits("Override - Setting our wanted Kuafu to: " + plan2Number + ".");
         if (plan2Number > kuafuBL)
         {
            aiEchoWarning("Override - Trying to maintain more Kuafu than our build limit currently allows: " + plan2Number + "/" +
               kuafuBL + ", clamping it to " + kuafuBL + ".");
            plan2Number = kuafuBL;
         }
      }
      else
      {
         // We aim for 70% Peasants, 30% Kuafu, but just allow max Peasants since we will be sure we can train those reliably.
         plan1Number = wantedVilsThisAge;
         plan2Number = ceil(wantedVilsThisAge * 0.3 / kbPlayerGetProtoStatInt(cMyID, cUnitTypeKuafu, cProtoStatPopCost));
         if (plan2Number > kuafuBL)
         {
            debugEconomicUnits("Trying to maintain more Kuafu than our build limit currently allows: " + plan2Number + "/" +
               kuafuBL + ", clamping it to " + kuafuBL + ".");
            plan2Number = kuafuBL;
         }
      }

      // Create the first plan if it isn't valid (anymore).
      if (aiPlanGetIsIDValid(gVillagerMaintainPlan) == false)
      {
         gVillagerMaintainPlan = createSimpleMaintainPlan(gEconUnit, plan1Number, gLandAreaGroupID, gEconomicUnitsCategoryID, 50, -1,
            -1, true);
      }
      // Adjust the name/maintain numbers if it differs from before.
      else if (aiPlanGetVariableInt(gVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0) != plan1Number)
      {
         debugEconomicUnits("Adjusting " + kbProtoUnitGetName(gEconUnit) + " Maintain plan to maintain: " + plan1Number + ".");
         aiPlanSetVariableInt(gVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0, plan1Number);
         aiPlanSetName(gVillagerMaintainPlan, gVillagerMaintainPlan + " Maintain " + plan1Number + " " +
                       kbProtoUnitGetName(gEconUnit));
      }

      // Create the second plan if it isn't valid (anymore).
      if (aiPlanGetIsIDValid(gSecondVillagerMaintainPlan) == false)
      {
         gSecondVillagerMaintainPlan = createSimpleMaintainPlan(cUnitTypeKuafu, plan2Number, gLandAreaGroupID, gEconomicUnitsCategoryID,
            50, -1, -1, true);
         // These units are very expensive and we potentially don't optimally use them. Limit their impact via train delay.
         aiPlanSetVariableInt(gSecondVillagerMaintainPlan, cTrainPlanFrequency, 0, 10);
         aiPlanSetVariableBool(gSecondVillagerMaintainPlan, cTrainPlanUseMultipleBuildings, 0, false);
      }
      // Adjust the name/maintain numbers if it differs from before.
      else if (aiPlanGetVariableInt(gSecondVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0) != plan2Number)
      {
         debugEconomicUnits("Adjusting " + kbProtoUnitGetName(cUnitTypeKuafu) + " Maintain plan to maintain: " + plan2Number + ".");
         aiPlanSetVariableInt(gSecondVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0, plan2Number);
         aiPlanSetName(gSecondVillagerMaintainPlan, gSecondVillagerMaintainPlan + ": Maintain " + plan2Number + " " +
                       kbProtoUnitGetName(cUnitTypeKuafu));
      }

      // Priority based on how many Villagers we already have alive, below 80% alive we have high prio.
      if (kbUnitCount(gEconUnit, cMyID, cUnitStateAlive) >= (plan1Number * 0.8))
      {
         aiPlanSetPriority(gVillagerMaintainPlan, 50);
      }
      else
      {
         aiPlanSetPriority(gVillagerMaintainPlan, 70);
      }
      if (kbUnitCount(cUnitTypeKuafu, cMyID, cUnitStateAlive) >= (plan2Number * 0.8))
      {
         aiPlanSetPriority(gSecondVillagerMaintainPlan, 50);
      }
      else
      {
         aiPlanSetPriority(gSecondVillagerMaintainPlan, 70);
      }
   }
}

//==============================================================================
// getMaxHeroCitizenCount
//==============================================================================
int getMaxHeroCitizenCount()
{
   int numHeroes = 0;
   switch (kbPlayerGetAge(cMyID))
   {
      case cAge1:
      {
         return 0; // Don't delay Classical for this.
      }
      case cAge2:
      {
         numHeroes = selectByDifficulty(1, 1, 2, 2, 3, 3);
         break;
      }
      case cAge3:
      {
         numHeroes = selectByDifficulty(2, 2, 4, 4, 6, 6);
         break;
      }
      case cAge4:
      case cAge5:
      {
         numHeroes = selectByDifficulty(2, 2, 5, 6, 7, 8);
         break;
      }
   }
   if (cPersonalityCurrent == cPersonalityEconomist)
   {
      debugEconomicUnits("We are " + getCurrentPersonalityName() + ", we want 1 more Hero Citizen.");
      numHeroes++;
   }
   if (cMyCiv == cCivGaia)
   {
      debugEconomicUnits("We are Gaia, we want 1 more Hero Citizen.");
      numHeroes++;
   }
   debugEconomicUnits("We want to have a maximum of " + numHeroes + " Hero Citizens right now.");
   return numHeroes;
}

//==============================================================================
// heroizeCitizens
// Attempt to transform Citizens into heroes.
//==============================================================================
rule heroizeCitizens
inactive
group defaultClassicalRules
minInterval 30
{
   if (cMyCulture != cCultureAtlantean || cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("heroizeCitizens");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticCitizenHeroize) == false)
   {
      return;
   }
   debugEconomicUnits("--- Running Rule heroizeCitizens. ---");

   int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechVillagerAtlanteanToHero);
   if (gDefensivelyOverrun == true)
   {
      debugEconomicUnits("We're defensively overrun, not starting a new Hero Citizen plan.");
      if (planID >= 0)
      {
         debugEconomicUnits("Destroying existing Hero Citizen research plan: " + aiPlanGetName(planID) + ".");
         aiPlanDestroy(planID);
      }
      return;
   }
   
   int numExistingHeroCitizens = kbUnitCount(cUnitTypeVillagerAtlanteanHero, cMyID, cUnitStateAlive);
   int numMaxHeroCitizenCount = getMaxHeroCitizenCount();
   debugEconomicUnits("We already have " + numExistingHeroCitizens + "/" + numMaxHeroCitizenCount + " Hero Citizens.");
   if (numExistingHeroCitizens >= numMaxHeroCitizenCount)
   {
      debugEconomicUnits("We are not allowed to make more.");
      return;
   }

   int prio = 50;
   if (numExistingHeroCitizens < (numMaxHeroCitizenCount / 2))
   {
      prio = 51;
      debugEconomicUnits("We have fewer than 50 percent of our wanted Hero Citizen count, prio for the research plan goes to " +
         prio + ".");
   }

   if (planID >= 0)
   {
      aiPlanSetPriority(planID, prio);
      debugEconomicUnits("We already have a research plan going to get a Hero Citizens, don't stack them (only adjust prio).");
      return;
   }

   int queryID = useSimpleUnitQuery(cUnitTypeVillagerAtlantean);
   int numUnits = kbUnitQueryExecute(queryID);
   if (numUnits == 0)
   {
      debugEconomicUnits("We have no regular Citizens alive, can't heroize one of them either.");
      return;
   }
   int toHeroizeID = -1;
   float highestHPPercentage = 0.0;
   for (int i = 0; i < numUnits; i++)
   {
      int unitID = kbUnitQueryGetResult(queryID, i);
      float hpPercentage = kbUnitGetStatFloat(unitID, cUnitStatHPRatio);
      if (hpPercentage > highestHPPercentage && kbAreaGetDangerLevel(kbUnitGetAreaID(unitID)) < aiGetExploreDangerThreshold())
      {
         highestHPPercentage = hpPercentage;
         toHeroizeID = unitID;
         if (hpPercentage == 1.0)
         {
            break;
         }
      }
   }
   if (numUnits > 0 && highestHPPercentage < 0.7)
   {
      debugEconomicUnits("We have no healthy Citizens left, we don't want to heroize very damaged ones...");
      return;
   }
   if (toHeroizeID == -1)
   {
      debugEconomicUnits("We have Citizens but none were valid to heroize, their areas are too dangerous.");
      return;
   }

   // No further economic checks here. We want these economic units badly regardless, just like the other Villager maintains.
   debugEconomicUnits("We will try and transform " + toHeroizeID + " to a Hero!");
   createSimpleResearchPlanSpecificResearcher(cTechVillagerAtlanteanToHero, toHeroizeID, prio, false);
}

//==============================================================================
// getMikoCountToMaintain
//==============================================================================
int getMikoCountToMaintain()
{
   int numMikos = 0;
   switch (kbPlayerGetAge(cMyID))
   {
      case cAge1:
      {
         numMikos = 0; // Starting Miko is enough, don't train more.
         break;
      }
      case cAge2:
      {
         numMikos = selectByDifficulty(1, 1, 1, 2, 2, 2);
         break;
      }
      case cAge3:
      {
         numMikos = selectByDifficulty(2, 2, 2, 3, 4, 4);
         break;
      }
      case cAge4:
      case cAge5:
      {
         numMikos = selectByDifficulty(2, 3, 3, 4, 5, 6);
         break;
      }
   }
   if (cPersonalityCurrent == cPersonalityMythical || cPersonalityCurrent == cPersonalityEconomist)
   {
      debugEconomicUnits("We are " + getCurrentPersonalityName() + ", adding 1 more Miko to maintain.");
      numMikos++;
   }
   debugEconomicUnits("We want to maintain " + numMikos + " Mikos right now.");
   return numMikos;
}

//==============================================================================
// mikoMaintainMonitor
// Sets the Miko maintain plan.
//==============================================================================
rule mikoMaintainMonitor
inactive
group defaultArchaicRules
priority 77
minInterval 10
{
   if (cMyCulture != cCultureJapanese)
   {
      xsDisableRule("mikoMaintainMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticMikoTraining) == false)
   {
      if (aiPlanGetIsIDValid(gMikoMaintainPlan) == true)
      {
         aiPlanDestroy(gMikoMaintainPlan);
      }
      gMikoMaintainPlan = -1;
      return;
   }
   debugEconomicUnits("--- Running Rule mikoMaintainMonitor. ---");

   int numMikoToMaintain = 0;
   if (gOverrideMaxMikoCount >= 0)
   {
      debugEconomicUnits("Override - Setting our wanted Mikos to: " + gOverrideMaxMikoCount + ".");
      numMikoToMaintain = gOverrideMaxMikoCount;
   }
   else
   {
      numMikoToMaintain = getMikoCountToMaintain();
   }
   
   // Create the plan if it isn't valid (anymore).
   if (aiPlanGetIsIDValid(gMikoMaintainPlan) == false)
   {
      gMikoMaintainPlan = createSimpleMaintainPlan(cUnitTypeMiko, numMikoToMaintain, gLandAreaGroupID, gEconomicUnitsCategoryID,
         50, -1, -1, true);
   }
   else
   {
      if (aiPlanGetVariableInt(gMikoMaintainPlan, cTrainPlanNumberToMaintain, 0) != numMikoToMaintain)
      {
         debugEconomicUnits("Adjusting Miko Maintain plan to maintain: " + numMikoToMaintain + ".");
         aiPlanSetVariableInt(gMikoMaintainPlan, cTrainPlanNumberToMaintain, 0, numMikoToMaintain);
         aiPlanSetName(gMikoMaintainPlan, gMikoMaintainPlan + " Maintain " + numMikoToMaintain + " Miko");
      }
   }

   // Priority based on how many Miko we already have alive, below 70% alive we have high prio.
   if (kbUnitCount(cUnitTypeMiko, cMyID, cUnitStateAlive) >= (numMikoToMaintain * 0.7))
   {
      aiPlanSetPriority(gMikoMaintainPlan, 50);
   }
   else
   {
      aiPlanSetPriority(gMikoMaintainPlan, 70);
   }
}

//==============================================================================
// economyPopCountsMonitor
// Overrides are not specifcally taken into account here, you're basically not meant to use cStrategyFlagAutomaticPopLimits 
// when you have overrides since it defeats the purpose.
// This is because the override numbers would be saved directly into the maintain plans anyway,
// And chances are that when you use overrides you have cStrategyFlagAutomaticPopLimits disabled.
//==============================================================================
rule economyPopCountsMonitor
inactive
group defaultArchaicRules
priority 76
minInterval 10
{
   if (checkStrategyFlag(cStrategyFlagAutomaticPopLimits) == false)
   {
      // Dont reset the economy pop limits in this if block.
      // The strategy is now fully responsible for setting these, we don't want to override that.
      return;
   }
   debugEconomicUnits("--- Running Rule economyPopCountsMonitor. ---");

   // Total land eco pop.
   int wantedLandEcoPop = 0;
   int num = 0;

   // Incorporate our Villager counts.
   if (aiPlanGetIsIDValid(gVillagerMaintainPlan) == true)
   {
      num = aiPlanGetVariableInt(gVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0);
      wantedLandEcoPop += num;
      debugEconomicUnits("First Villager maintain plan is valid and added " + num + " to our total wantedLandEcoPop, which now is: " +
         wantedLandEcoPop + ".");
   }
   else
   {
      debugEconomicUnits("No valid first Villager maintain plan, not taking first Villagers into account now.");
   }

   // Only Norse have a gSecondVillagerMaintainPlan that we need to take into account.
   // The other cultures that have multiple Villager types don't have a use for the numbers in gSecondVillagerMaintainPlan.
   if (cMyCulture == cCultureNorse)
   {
      if (aiPlanGetIsIDValid(gSecondVillagerMaintainPlan) == true)
      {
         num = aiPlanGetVariableInt(gSecondVillagerMaintainPlan, cTrainPlanNumberToMaintain, 0);
         wantedLandEcoPop += num;
         debugEconomicUnits("Second Villager maintain plan is valid and added " + num + " to our total wantedLandEcoPop, which now is: " +
            wantedLandEcoPop + ".");
      }
      else
      {
         debugEconomicUnits("No valid second Villager maintain plan, not taking second Villagers into account now.");
      }
   }
   
   // Offest the Villager part for Atlanteans.
   if (cMyCulture == cCultureAtlantean)
   {
      // wantedLandEcoPop currently holds how many Citizens we want to maintain.
      wantedLandEcoPop *= kbPlayerGetProtoStatInt(cMyID, gEconUnit, cProtoStatPopCost); // Offset for 2 pop cost.
      debugEconomicUnits("We're Atlantean, adjusting our wantedLandEcoPop for 2 pop Citizens, new wantedLandEcoPop: " +
         wantedLandEcoPop + ".");
   }

   if (cMyCulture == cCultureJapanese)
   {
      if (aiPlanGetIsIDValid(gMikoMaintainPlan) == true)
      {
         num = aiPlanGetVariableInt(gMikoMaintainPlan, cTrainPlanNumberToMaintain, 0);
         wantedLandEcoPop += num;
         debugEconomicUnits("Miko maintain plan is valid and added " + num + " to our total wantedLandEcoPop, which now is: " +
            wantedLandEcoPop + ".");
      }
      else
      {
         debugEconomicUnits("No valid Miko maintain plan, not taking Mikos into account now.");
      }
   }

   // Incorporate our Caravan counts.
   if (aiPlanGetIsIDValid(gCaravanMaintainPlan) == true)
   {
      // If we don't have a valid Market assigned to train from we take our current Caravan count instead.
      if (kbUnitGetIsIDValid(aiPlanGetVariableInt(gCaravanMaintainPlan, cTrainPlanBuildingID, 0)) == false)
      {
         num = kbUnitCount(gCaravanUnit, cMyID, cUnitStateAlive);
         wantedLandEcoPop += num;
         debugEconomicUnits("Caravan maintain plan is valid BUT we have no valid Market. Adding our current Caravan count of: "
            + num + " to our total wantedLandEcoPop, which now is: " + wantedLandEcoPop + ".");
      }
      else
      {
         num = aiPlanGetVariableInt(gCaravanMaintainPlan, cTrainPlanNumberToMaintain, 0);
         wantedLandEcoPop += num;
         debugEconomicUnits("Caravan maintain plan is valid and added " + num + " to our total wantedLandEcoPop, which now is: " +
            wantedLandEcoPop + ".");
      }
   }
   else
   {
      // We must add the alive Caravans in this situation anyway because they are currently taking up alive land economy pop.
      // If we don't add them our Villager training could get blocked on the limits because we have some idle Caravans around.
      num = kbUnitCount(gCaravanUnit, cMyID, cUnitStateAlive);
      wantedLandEcoPop += num;
      debugEconomicUnits("No valid Caravan maintain plan, add our currently alive Caravans only. Adding our current Caravan count of: "
         + num + " to our total wantedLandEcoPop, which now is: " + wantedLandEcoPop + ".");
   }

   // Actually set how many LAND eco pop units we're allowed to train.
   debugEconomicUnits("Wanted land economy pop: " + wantedLandEcoPop + ".");
   aiSetEconomyPop(wantedLandEcoPop);

   /////////////////////////////////////////////////////////////////////////////
   // Total naval eco pop.
   int wantedNavalEcoPop = 0;

   if (aiPlanGetIsIDValid(gFishingShipMaintainPlan) == true)
   {
      if (gHaveValidFishingDock == false)
      {
         num = kbUnitCount(gFishingUnit, cMyID, cUnitStateAlive);
         wantedNavalEcoPop += num;
         debugEconomicUnits("Fishing Ship maintain plan is valid BUT we have no valid Dock. Adding our current Fishing Ship count of: "
            + num + " to our total wantedNavalEcoPop, which now is: " + wantedNavalEcoPop + ".");
      }
      else
      {
         num = aiPlanGetVariableInt(gFishingShipMaintainPlan, cTrainPlanNumberToMaintain, 0);
         wantedNavalEcoPop += num;
         debugEconomicUnits("Fishing Ship maintain plan is valid and added " + num + " to our total wantedNavalEcoPop, which now is: " +
               wantedNavalEcoPop + ".");
      }
   }
   else
   {
      // No need to add alive Fishing Ships here because there is no other training for naval eco pop that these
      // idle ships could interfere with.
      debugEconomicUnits("No valid Fishing Ship maintain plan, not taking Fishing Ships into account now.");
   }

   // Actually set how many NAVAL eco pop units we're allowed to train.
   debugEconomicUnits("Wanted naval economy pop: " + wantedNavalEcoPop + ".");
   aiSetNavalEconomyPop(wantedNavalEcoPop);

   /////////////////////////////////////////////////////////////////////////////

   // gMaxMilitaryPop is used to control how much military lower difficulties are allowed to make, period.
   // Even if the AI has tons of excess resources or max eco pop we never want to unlock military pop for these lower difficulties.
   // We will instead use this number, it is calculated by multiplying our total eco (land + naval) * gMilitaryToEcoRatio.
   if (cDifficultyCurrent <= cDifficultyEasy)
   {
      // If we have infinite starting resources all our numbers above will be low since we don't need to gather.
      // But our military pop should then not also become very low.
      // Take the maintain numbers as they would usually be and calculate using them.
      // We can also have a ton of resources but low current eco, also take a high amount for gMaxMilitaryPop then.
      if (cStartingResourcesCurrent == cStartingResourcesInfinite || haveExcessResourceAmount(2000.0, cAllResources) == true)
      {
         // ATTENTION: here overrides are specifically ignored because we don't take the numbers from the plans.
         // So if you are running overrides + resource infinite + cStrategyFlagAutomaticPopLimits this logic is bad, mega edge case...

         // We only take the Villager count because if we train Caravans/Fishing Ships we lower the amount of Villagers we want
         // in the actual plan accordingly. So just the flat Villager count is a proper representation of our total eco wants.
         int numVillagers = getBaseVillagerCountToMaintainOtherAges();
         gMaxMilitaryPop = numVillagers * gMilitaryToEcoRatio;
         debugEconomicUnits("We have a lot of resources, unlocking gMaxMilitaryPop as if we had our max alllowed economic pop: " +
            numVillagers + " * " + gMilitaryToEcoRatio + ": " + gMaxMilitaryPop + ".");
      }
      // Base the max military pop on our current economy.
      else
      {
         gMaxMilitaryPop = (wantedLandEcoPop + wantedNavalEcoPop) * gMilitaryToEcoRatio;
         debugEconomicUnits("Max military pop (lower difficulties) ((" + wantedLandEcoPop + " + " + wantedNavalEcoPop + ") * " +
         gMilitaryToEcoRatio + "): " + gMaxMilitaryPop + ".");
      }
   }
}