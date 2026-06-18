//==============================================================================
/* buildings_economic.xs

   This file is intended for managing what economic buildings the AI should create and when.

*/
//==============================================================================

//==============================================================================
// House monitor
// Build extra houses if we need them.
//==============================================================================
rule houseMonitor
inactive
group defaultArchaicRules
minInterval 3
{
   if (checkStrategyFlag(cStrategyFlagBuildHouses) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule houseMonitor. ---");

   int numHousesNeeded = calculateNumberHousesNeeded();
   if (numHousesNeeded == 0)
   {
      return;
   }
   // Spread out our houses over Town Center bases.
   int baseID = getRandomTownCenterBaseID();
   createSimpleBuildPlan(gHouseUnit, numHousesNeeded, 95, gEconomicBuildingsCategoryID, baseID, 1);
}

//==============================================================================
// mainBaseMonitor
// Switch main bases if needed.
//==============================================================================
rule mainBaseMonitor
group defaultArchaicRules
inactive
minInterval 5
{
   if (checkStrategyFlag(cStrategyFlagAutomaticMainBaseSwitching) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule mainBaseMonitor. ---");

   int mainBaseID = kbBaseGetMainID(cMyID);
   if (mainBaseID == -1)
   {
      debugEconomicBuildings("No bases alive at all, can't do anything here.");
      return;
   }
   bool needSwapMainBase = false;
   if (kbBaseIsFlagSet(cMyID, mainBaseID, cBaseFlagTownCenter) == false)
   {
      debugEconomicBuildings("We should swap mainbases because our old one doesn't have a Town Center anymore.");
      needSwapMainBase = true;
   }
   if (needSwapMainBase == false && gLandAreaGroupID != -1 &&
       kbPathAreAreaGroupsConnected(gLandAreaGroupID,
       kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, mainBaseID)), cPassabilityAmphibious) == false)
   {
      debugEconomicBuildings("We should swap mainbases because our old one isn't connected to gLandAreaGroupID.");
      needSwapMainBase = true;
   }
   if (needSwapMainBase == true)
   {
      // No other TC bases, don't swap main.
      int mostDefendedTCBaseID = getMostDefendedTCBase();
      if (mostDefendedTCBaseID == -1)
      {
         debugEconomicBuildings("Couldn't swap mainbase because we couldn't find a new suitable Town Center base.");
         return;
      }
      debugEconomicBuildings("Swapping main to " + kbBaseGetNameByID(cMyID, mostDefendedTCBaseID) + ".");
      aiSwitchMainBase(mostDefendedTCBaseID);
   }
}

//==============================================================================
// tcRepairMonitor
// Always repair our Town / Citadel Centers.
//==============================================================================
rule tcRepairMonitor
group defaultArchaicRules
inactive
minInterval 15
{
   if (checkStrategyFlag(cStrategyFlagAutomaticTCRepair) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule tcRepairMonitor. ---");
   
   int queryID = useSimpleUnitQuery(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateAlive);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(queryID, gLandAreaGroupID, cPassabilityLand);
   }
   int numResults = kbUnitQueryExecute(queryID);
   int[] results = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < results.size(); i++)
   {
      int unitID = results[i];
      if (kbUnitGetStatFloat(unitID, cUnitStatHPRatio) < 1.0)
      {
         // If we only have one TC remaining we just keep potentially suicide repairing it and don't check for enemies.
         bool shouldRepair = true;
         if (numResults > 1)
         {
            int baseID = kbUnitGetBaseID(unitID);
            vector baseLocation = kbBaseGetLocation(cMyID, baseID);
            float baseRange = kbBaseGetDistance(cMyID, baseID);
            int numEnemies = getUnitCountByLocation(cUnitTypeLogicalTypeLandMilitary, cPlayerRelationEnemyNotGaia, cUnitStateAlive,
               baseLocation, baseRange + 10.0);
            int numAllies = getUnitCountByLocation(cUnitTypeLogicalTypeLandMilitary, cPlayerRelationAlly, cUnitStateAlive,
               baseLocation, baseRange + 10.0);
            if (numEnemies * 1.3 > numAllies)
            {
               debugEconomicBuildings("We found TC " + unitID +
                  " that is damaged but is also still under heavy attack, not repairing now.");
               shouldRepair = false;
            }
         }
         else
         {
            debugEconomicBuildings("We found TC " + unitID + " that is damaged, it's our last TC, force repair it.");
         }
         int existingPlanID = aiPlanGetIDByTypeAndVariableIntValue(cPlanRepair, cRepairPlanTargetID, unitID);
         if (existingPlanID == -1 && shouldRepair == true)
         {
            // We need to repair!
            debugEconomicBuildings("Found a TC/Citadel(" + unitID + ") that has been damaged, creating a repair plan for it.");
            int protoUnitID = kbUnitGetProtoUnitID(unitID);
            int planID = aiPlanCreate("Repair " + kbProtoUnitGetName(protoUnitID) + " ID: " + unitID, cPlanRepair, -1,
                                      gEconomicBuildingsCategoryID);
            aiPlanSetVariableInt(planID, cRepairPlanTargetID, 0, unitID);
            // Little bit higher prio since we need these buildings to remain alive.
            aiPlanSetPriority(planID, 60);
            // Repair a Citadel Center with a few more units since they're so valuable.
            addBuilderTypesToPlan(planID, protoUnitID, protoUnitID == cUnitTypeCitadelCenter ? 4 : 2, true);
            aiPlanSetBaseID(planID, kbUnitGetBaseID(unitID));
         }
         else if (existingPlanID >= 0)
         {
            if (shouldRepair == false)
            {
               // We need to cancel this repair!
               debugEconomicBuildings("Destroying plan: " + aiPlanGetName(existingPlanID) +
                  " because the TC it was repairing is dangerous.");
               aiPlanDestroy(existingPlanID);
            }
            else
            {
               debugEconomicBuildings("TC " + unitID + " is damaged and we're already repairing it via: " +
                  aiPlanGetName(existingPlanID) + ".");
            }
         }
      }
   }
}

//==============================================================================
// buildVillageCenter
//==============================================================================
void buildVillageCenter(int baseID = -1, int prio = 50)
{
   int planID = aiPlanCreate("Village Center Build Plan", cPlanBuild, -1, gEconomicBuildingsCategoryID);
   int bpID = kbBuildingPlacementCreate(aiPlanGetName(planID));
   kbBuildingPlacementSetBuildingPUID(bpID, cUnitTypeVillageCenter);
   addSafeBackAreasToBuildingPlacement(bpID, baseID, gEconomicBuildingsCategoryID);
   aiPlanSetVariableInt(planID, cBuildPlanBuildingPlacementID, 0, bpID);
   aiPlanSetVariableInt(planID, cBuildPlanBuildingTypeID, 0, cUnitTypeVillageCenter);
   // Don't send a ton of builders to this since we don't really need the building and it would drain us for no reason.
   addBuilderTypesToPlan(planID, cUnitTypeVillageCenter, selectByDifficulty(1, 1, 2, 2, 3, 3), true);
   aiPlanSetPriority(planID, prio);

   // Custom avoidBlockingImportantSpots because we can be built next to resources.
   float halfObstruction = kbPlayerGetProtoStatFloat(cMyID, cUnitTypeVillageCenter, cProtoStatObstruction) / 2.0;
   kbBuildingPlacementAddUnitInfluence(bpID, cUnitTypeAbstractTownCenter, -10000, 16.0 + halfObstruction, cFalloffNone);
   kbBuildingPlacementAddUnitInfluence(bpID, cUnitTypeSettlement, -10000, 16.0 + halfObstruction, cFalloffNone, -1,
      cPlayerMotherNatureID);

   debugEconomicBuildings("Created plan: " + aiPlanGetName(planID) + ", for a Village Center in base " +
      kbBaseGetNameByID(cMyID, baseID) + ".");
}

//==============================================================================
// villageCenterConstructionMonitor
// Create more Village Centers during the game.
//==============================================================================
rule villageCenterConstructionMonitor
group defaultClassicalRules
inactive
minInterval 30
{
   if (cPersonalityCurrent != cPersonalityEconomist && cPersonalityCurrent != cPersonalityBuilder)
   {
      xsDisableRule("villageCenterConstructionMonitor");
      return;
   }
   // Same flag as for TCs is intended.
   if (checkStrategyFlag(cStrategyFlagAutomaticTCExpansion) == false)
   {
      return;
   }
   if (gDefensivelyOverrun == true)
   {
      debugEconomicBuildings("Not going to make a Village Center build plan since we're defensively overrun.");
      return;
   }

   int queryID = useSimpleUnitQuery(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateAlive);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(queryID, gLandAreaGroupID, cPassabilityAmphibious);
   }
   int numAliveTC = kbUnitQueryExecute(queryID);
   if (numAliveTC == 0)
   {
      debugEconomicBuildings("We have no Town Centers alive to orient Village Centers around.");
      return;
   }
   if (cPersonalityCurrent == cPersonalityBuilder && numAliveTC < 2)
   {
      debugEconomicBuildings("Not going to make a Village Center since we have fewer than 2 TCs alive and are a Builder.");
      return;
   }
   queryID = useSimpleUnitQuery(cUnitTypeVillageCenter, cMyID, cUnitStateAlive);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(queryID, gLandAreaGroupID, cPassabilityAmphibious);
   }
   int numAliveVCs = kbUnitQueryExecute(queryID);

   int prio = 50;
   // We want at least as many VCs as we have TCs, this also forces us to actually build them instead of it being in queue forever.
   if (numAliveVCs < numAliveTC)
   {
      prio = 51;
   }

   int existingPlanID = aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeVillageCenter, 0);
   if (existingPlanID != -1)
   {
      // Update prio of existing plan.
      aiPlanSetPriority(existingPlanID, prio);
      debugEconomicBuildings("Not going to make a Village Center build plan since we already have one going on.");
      return;
   }
   
   // Village Centers can really drain our eco, try to avoid that when we want to really age up.
   if (aiPlanGetIsIDValid(gAgeUpResearchPlan) == true && aiPlanGetPriority(gAgeUpResearchPlan) > 50)
   {
      debugEconomicBuildings("Not going to make a Village Center build plan since we have a high age up priority.");
      return;
   }
   // Also need at least 20% eco pop.
   float percentageEcoPopNeeded = 0.20;
   int wantedEcoPop = aiGetEconomyPop() + aiGetCurrentNavalEconomyPop();
   int currentEcoPop = aiGetCurrentEconomyPop() + aiGetNavalEconomyPop();
   float currentEconomicPopPercentage = xsIntToFloat(currentEcoPop) / xsIntToFloat(wantedEcoPop);
   if (currentEconomicPopPercentage < percentageEcoPopNeeded)
   {
      debugEconomicBuildings("Won't build new Village Centers because we have to few alive eco pop compared to what we want, " +
         "percentage: " + currentEconomicPopPercentage + "/" + percentageEcoPopNeeded + ".");
      return;
   }

   int maxNumVCs = numAliveTC;
   if (cPersonalityCurrent == cPersonalityEconomist)
   {
      if (cDifficultyCurrent == cDifficultyHard || cDifficultyCurrent == cDifficultyTitan)
      {
         maxNumVCs = numAliveTC * 1.5;
      }
      else if (cDifficultyCurrent >= cDifficultyExtreme)
      {
         maxNumVCs = numAliveTC * 2;
      }
   }
   else // Builder, we want far fewer.
   {
      if (xsRandBool(0.33) == false)
      {
         debugEconomicBuildings("We're a builder and didn't roll correctly to build a Village Center, 33 percent true chance.");
         return;
      }
      maxNumVCs = ceil(xsIntToFloat(numAliveTC) / 2.0);
   }
   debugEconomicBuildings("We have " + numAliveTC + " Town Centers alive, , we're allowed a maximum of " + maxNumVCs +
      " Village Centers based on that number. We already have " + numAliveVCs + " Village Centers alive.");
   if (numAliveVCs >= maxNumVCs)
   {
      debugEconomicBuildings("We're already at or above our maximum allowed number of Village Centers.");
      return;
   }
   // Due to how we just grab a random ID we could build many Village Centers very close to each other.
   // But since they're created in back areas we can't determine how many already belong to an existing TC base,
   // they could be outside of it.
   int randomTCBaseID = getRandomTownCenterBaseID(); // This should always be valid since we already query above.
   buildVillageCenter(randomTCBaseID, prio);
}

//==============================================================================
// updateTCArrays
//==============================================================================
void updateTCArrays()
{
   for (int i = gTCBases.size() - 1; i >= 0; i--)
   {
      int baseID = gTCBases[i];
      if (kbBaseGetIsIDValid(cMyID, baseID) == false)
      {
         gTCBases.removeIndex(i);
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityAmphibious) == false)
      {
         debugEconomicBuildings("TC base: " + kbBaseGetNameByID(cMyID, baseID) + ", is not connected to gLandAreaGroupID, " +
            "remove from gTCBases.");
         gTCBases.removeIndex(i);
         continue;
      }
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         gTCBases.removeIndex(i);
         gLostTCBases.add(baseID);
         debugEconomicBuildings("TC base: " + kbBaseGetNameByID(cMyID, baseID) + ", has lost its TC, adding it to gLostTCBases.");
      }
   }

   for (int i = gLostTCBases.size() - 1; i >= 0; i--)
   {
      int baseID = gLostTCBases[i];
      if (kbBaseGetIsIDValid(cMyID, baseID) == false)
      {
         gLostTCBases.removeIndex(i);
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityAmphibious) == false)
      {
         debugEconomicBuildings("TC base: " + kbBaseGetNameByID(cMyID, baseID) + ", is not connected to gLandAreaGroupID, " +
            "remove from gLostTCBases.");
         gLostTCBases.removeIndex(i);
         continue;
      }
      int settlementID = getClosestUnitByLocation(cUnitTypeSettlement, 0, cUnitStateAlive, kbBaseGetLocation(cMyID, baseID), 5.0);
      if (settlementID == -1)
      {
         // Remove from this array since something happened with this socket.
         gLostTCBases.removeIndex(i);
         debugEconomicBuildings("We had a TC in base: " + kbBaseGetNameByID(cMyID, baseID) + ", but somebody took it from us!");

         int tcID = getClosestUnitByLocation(cUnitTypeAbstractSocketedTownCenter, cPlayerRelationAny, cUnitStateABQ,
            kbBaseGetLocation(cMyID, baseID), 5.0);
         if (tcID == -1)
         {
            tcID = getClosestUnitByLocation(cUnitTypeSettlement, cPlayerRelationAny, cUnitStateABQ,
               kbBaseGetLocation(cMyID, baseID), 5.0);
            if (tcID == -1)
            {
               aiEchoWarning(kbBaseGetNameByID(cMyID, baseID) + ", how can the Socket/TC be completely gone from this base?");
               continue;
            }
         }

         int ownerPlayerID = kbUnitGetPlayerID(tcID);
         if (ownerPlayerID == cMyID)
         {
            debugEconomicBuildings("We have reclaimed the Town Center in our base: " + kbBaseGetNameByID(cMyID, baseID) + ".");
            continue;
         }
         if (kbPlayerIsAlly(ownerPlayerID) == true || kbPlayerIsNeutral(ownerPlayerID) == true)
         {
            debugEconomicBuildings("Our ally/neutral " + ownerPlayerID +
               " took the Town Center in our base, we can't recapture/rebuild it now.");
            continue;
         }

         if (cPersonalityCurrent == cPersonalityPassive)
         {
            debugEconomicBuildings("Enemy " + ownerPlayerID +
               " took the Town Center in our base, we're passive though so won't fight over it.");
         }
         else
         {
            debugEconomicBuildings("Enemy " + ownerPlayerID + " took the Town Center in our base, we should recapture it!");
            gTCBasesToRecapture.add(baseID);
         }
         continue;
      }
      debugEconomicBuildings("Base " + kbBaseGetNameByID(cMyID, baseID) + " still has an available Settlement for us to take.");
   }

   for (int i = gTCBasesToRecapture.size() - 1; i >= 0; i--)
   {
      int baseID = gTCBasesToRecapture[i];
      if (kbBaseGetIsIDValid(cMyID, baseID) == false)
      {
         gTCBasesToRecapture.removeIndex(i);
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityAmphibious) == false)
      {
         debugEconomicBuildings("TC base: " + kbBaseGetNameByID(cMyID, baseID) + ", is not connected to gLandAreaGroupID, " +
            "remove from gTCBasesToRecapture.");
         gTCBasesToRecapture.removeIndex(i);
         continue;
      }
      int tcID = getClosestUnitByLocation(cUnitTypeAbstractSocketedTownCenter, cPlayerRelationAny, cUnitStateABQ,
         kbBaseGetLocation(cMyID, baseID), 5.0);
      if (tcID == -1)
      {
         // There is no Town Center in this base, but it could be one is being constructed.
         tcID = getClosestUnitByLocation(cUnitTypeSettlement, cPlayerRelationAny, cUnitStateABQ,
            kbBaseGetLocation(cMyID, baseID), 5.0);
         // If this Settlement now belongs to player 0 again it's truly unclaimed and not a WiP TC instead.
         if (kbUnitGetPlayerID(tcID) == 0)
         {
            tcID = -1;
         }
         if (tcID == -1)
         {
            gTCBasesToRecapture.removeIndex(i);
            gLostTCBases.add(baseID);
            debugEconomicBuildings("The TC somebody stole in " + kbBaseGetNameByID(cMyID, baseID) + " is gone, we could retake it.");
            continue;
         }
      }
   }

   int numBases = kbBaseGetNumber(cMyID);
   for (int i = 0; i < numBases; i++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, i);
      if (gTCBases.find(baseID) != -1)
      {
         continue;
      }
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityAmphibious) == false)
      {
         continue;
      }
      gTCBases.add(baseID);
      debugEconomicBuildings("Found a new TC base to add to gTCBases: " + kbBaseGetNameByID(cMyID, baseID) + ".");
   }
}

//==============================================================================
// tcExpansionMonitor
// Create more Town Centers during the game.
//==============================================================================
rule tcExpansionMonitor
group defaultClassicalRules
inactive
minInterval 30
{
   if (checkStrategyFlag(cStrategyFlagAutomaticTCExpansion) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule tcExpansionMonitor. ---");

   // Update the list of TCs that we could rebuild as we prioritize those.
   updateTCArrays();

   if (aiPlanGetIsIDValid(gTCBuildPlanID) == true)
   {
      debugEconomicBuildings("We already have a TC build plan ongoing, don't need to create another one now.");
      return;
   }

   int currentAge = kbPlayerGetAge(cMyID);
   int numAliveTC = gTCBases.size();
   if (cDifficultyCurrent <= cDifficultyHard)
   {
      int maxTownCenters = selectByDifficulty(1, 2, 3);
      // Conquerors can build 1 extra Town Center on lower difficulties.
      if (cPersonalityCurrent == cPersonalityConqueror)
      {
         maxTownCenters++;
      }
      if (numAliveTC >= maxTownCenters)
      {
         debugEconomicBuildings("We're already at our max Town Centers: " + maxTownCenters + ".");
         return;
      }
   }
   if (cPersonalityCurrent == cPersonalitySieger && numAliveTC >= 1 && currentAge < cAge3)
   {
      debugEconomicBuildings("We're a Sieger and aren't in the Heroic Age yet and still own a Town Center, don't build more.");
      return;
   }

   bool massiveExcess = haveExcessResourceAmount(1000.0);

   if (massiveExcess == true || numAliveTC == 0)
   {
      if (massiveExcess == true)
      {
         debugEconomicBuildings("Skipping all checks to potentially not build a TC since we have massive excess resources.");
      }
      if (numAliveTC == 0)
      {
         debugEconomicBuildings("Skipping all checks to potentially not build a TC since we have no TC and need to get one ASAP.");
      }
   }
   else
   {
      // Do we even want to build a new TC now?
      if (gDefensivelyOverrun == true)
      {
         debugEconomicBuildings("Won't analyze new Settlements to claim because we're defensively overrun.");
         return;
      }

      if (aiPlanGetIsIDValid(gAgeUpResearchPlan) == true && aiPlanGetPriority(gAgeUpResearchPlan) > 50)
      {
         debugEconomicBuildings("Won't analyze new Settlements to claim because we're trying to age up.");
         return;
      }

      // We need increasingly more alive eco pop to be able to build new TCs.
      float percentageEcoPopNeeded = 0.30;
      if (numAliveTC == 2)
      {
         percentageEcoPopNeeded = 0.70;
      }
      else if (numAliveTC == 3)
      {
         percentageEcoPopNeeded = 0.80;
      }
      else if (numAliveTC >= 4)
      {
         percentageEcoPopNeeded = 0.90;
      }
      if (cPersonalityCurrent == cPersonalityConqueror)
      {
         percentageEcoPopNeeded -= 0.20;
      }

      // On a land map the naval syscalls will give back -1 and skew the calc a little, it's fine...
      int wantedEcoPop = aiGetEconomyPop() + aiGetNavalEconomyPop();
      if (wantedEcoPop == 0)
      {
         debugEconomicBuildings("We currently want 0 economic pop, waiting until other logic sets these values for us.");
         return;
      }
      int currentEcoPop = aiGetCurrentEconomyPop() + aiGetCurrentNavalEconomyPop();
      float currentEconomicPopPercentage = xsIntToFloat(currentEcoPop) / xsIntToFloat(wantedEcoPop);
      if (currentEconomicPopPercentage < percentageEcoPopNeeded)
      {
         debugEconomicBuildings("Won't analyze new Settlements to claim because we have too few alive eco pop compared to what we want, " +
            "percentage: " + currentEconomicPopPercentage + "/" + percentageEcoPopNeeded + ".");
         return;
      }

      // On a land map the naval syscalls will give back -1 and skew the calc a little, it's fine...
      float percentageMilitaryPopNeeded = 0.30;
      if (cPersonalityCurrent == cPersonalityConqueror || cPersonalityCurrent == cPersonalityEconomist)
      {
         percentageMilitaryPopNeeded -= 0.10;
      }
      int wantedMilitaryPop = aiGetMilitaryPop() + aiGetNavalMilitaryPop();
      if (wantedMilitaryPop == 0)
      {
         debugEconomicBuildings("We currently want 0 military pop, waiting until other logic sets these values for us.");
         return;
      }
      int currentMilitaryPop = aiGetCurrentMilitaryPop() + aiGetCurrentNavalMilitaryPop();
      float currentMilitaryPopPercentage = xsIntToFloat(currentMilitaryPop) / xsIntToFloat(wantedMilitaryPop);
      if (currentMilitaryPopPercentage < percentageMilitaryPopNeeded)
      {
         debugEconomicBuildings("Won't analyze new Settlements to claim because we have to few alive military pop compared to what we " +
            "want, percentage: " + currentMilitaryPopPercentage + "/" + percentageMilitaryPopNeeded + ".");
         return;
      }
   }

   // First we analyze all our lost bases. Those we want to rebuild before expanding to new locations.
   bool haveSafeBaseToRebuild = false;
   int numBuildingsInBestBase = 0;
   int bestBaseToRebuildID = -1;
   int fewestEnemySoldiers = cMaxInt;
   int lostBasesSize = gLostTCBases.size();
   for (int i = 0; i < lostBasesSize; i++)
   {
      int baseID = gLostTCBases[i];
      vector baseLocation = kbBaseGetLocation(cMyID, baseID);
      float baseRange = kbBaseGetDistance(cMyID, baseID);
      int numEnemies = getUnitCountByLocation(cUnitTypeLogicalTypeLandMilitary, cPlayerRelationEnemyNotGaia, cUnitStateAlive,
         baseLocation, baseRange + 10.0);
      if (numEnemies > 0)
      {
         // No point in analyzing an unsafe base if we have a safe base to rebuild.
         if (haveSafeBaseToRebuild == true)
         {
            debugEconomicBuildings("Not going to rebuild " + kbBaseGetNameByID(cMyID, baseID) + " because we already see a safe base " +
               "and this one has enemies in it.");
            continue;
         }
         // If we already have a Town Center we don't want to rebuild in a dangerous area.
         if (numAliveTC >= 1)
         {
            debugEconomicBuildings("Not going to rebuild " + kbBaseGetNameByID(cMyID, baseID) + " because we already have an alive TC " +
               "and this base has enemies in it, so no need to rush a TC here.");
            continue;
         }
         if (numEnemies < fewestEnemySoldiers)
         {
            fewestEnemySoldiers = numEnemies;
            bestBaseToRebuildID = baseID;
         }
      }
      else
      {
         haveSafeBaseToRebuild = true;
         // This tries to make sure we rebuild our most built up base instead of a random Settlement with 1 House next to it.
         int numBuildings = kbBaseGetNumberUnitsOfType(cMyID, baseID, cUnitTypeBuilding);
         if (numBuildings > numBuildingsInBestBase)
         {
            bestBaseToRebuildID = baseID;
            numBuildingsInBestBase = numBuildings;
         }
      }
   }

   // If we have something to rebuild, yaay do that.
   if (bestBaseToRebuildID != -1)
   {
      // If we already have a TC we have regular prio, otherwise very high prio but below dropsites.
      int prio = numAliveTC >= 1 ? 55 : 90;
      gBaseToReclaimID = bestBaseToRebuildID;
      int settlementID = getClosestUnitByLocation(cUnitTypeSettlement, 0, cUnitStateAlive,
         kbBaseGetLocation(cMyID, bestBaseToRebuildID), 5.0); // This Settlement ID must be valid since we checked above.
      gTCBuildPlanID = createSocketBuildPlan(cUnitTypeTownCenter, settlementID, prio, cCalculateNumBuildersAutomatically, true);
      aiPlanSetEventHandler(gTCBuildPlanID, cPlanEventStateChange, "tcExpansionBPHandler");
      debugEconomicBuildings("We will try and reclaim the Settlement in base " + kbBaseGetNameByID(cMyID, bestBaseToRebuildID) +
         ", plan: " + aiPlanGetName(gTCBuildPlanID) + ".");
      return;
   }

   // If we have bases to reclaim then fully focus on that first.
   if (lostBasesSize > 0)
   {
      debugEconomicBuildings("We still have TC bases to reclaim, they're just too dangerous right now, quiting.");
      return;
   }

   if (numAliveTC == 0)
   {
      // If we're here we have no active TC build plan + no base to rebuild + no alive Town Centers, kinda panic.
      int closestSettlementID = -1;
      int numBases = kbBaseGetNumber(cMyID);
      float bestDefenseRating = cMinFloat;
      int bestBaseID = -1;
      for (int i = 0; i < numBases; i++)
      {
         int baseID = kbBaseGetIDByIndex(cMyID, i);
         float defenseRating = kbBaseGetDefenseRating(cMyID, baseID);
         if (defenseRating > bestDefenseRating)
         {
            bestDefenseRating = defenseRating;
            bestBaseID = baseID;
         }
      }
      if (bestBaseID == -1)
      {
         debugEconomicBuildings("We have no bases left to orient our new TC around, take a random unit's position now.");
         int unitID = (getUnit(cUnitTypeUnit));
         if (unitID == -1)
         {
            debugEconomicBuildings("Also couldn't find a unit, nothing to do anymore.");
            return;
         }
         closestSettlementID = getClosestUnitByLocation(cUnitTypeSettlement, 0, cUnitStateAlive, kbUnitGetPosition(unitID), 9999.0);
      }
      else
      {
         debugEconomicBuildings("Going to search for a Settlement to take near base: " + kbBaseGetNameByID(cMyID, bestBaseID) + ".");
         closestSettlementID = getClosestUnitByLocation(cUnitTypeSettlement, 0, cUnitStateAlive,
            kbBaseGetLocation(cMyID, bestBaseID), 9999.0);
      }
      if (closestSettlementID == -1)
      {
         debugEconomicBuildings("Couldn't find any Settlement to take, we must wait.");
         return;
      }
      gTCBuildPlanID = createSocketBuildPlan(cUnitTypeTownCenter, closestSettlementID, 90, cCalculateNumBuildersAutomatically, true);
      aiPlanSetEventHandler(gTCBuildPlanID, cPlanEventStateChange, "tcExpansionBPHandler");
      debugEconomicBuildings("We will try and claim the Settlement with ID " + closestSettlementID + ", plan: " +
         aiPlanGetName(gTCBuildPlanID) + ".");
      return;
   }

   // If we're here we need to search for a completely new Settlement to take.
   // We prefer Town Centers close to our main base.
   // Building many TCs far away from each other can really mess up our gathering and defending.
   // Also now we should only consider Settlements in the same area group, else we put ourself in the split base problem.
   
   float searchRange = 150.0;
   if (cMapSizeCurrent == cMapSizeLarge)
   {
      searchRange = 200.0;
   }
   // Passive can't expand too far away.
   if (cPersonalityCurrent != cPersonalityPassive && cDifficultyCurrent >= cDifficultyTitan && currentAge >= cAge4 &&
       haveExcessResourceAmount(1000.0) == true)
   {
      // So yes this can rekt our defending/gathering but will also give as extra pop that we may need.
      debugEconomicBuildings("Removing maximum search range for an unclaimed Settlement since we're in very lategame.");
      searchRange = cMaxFloat;
   }

   // We must have a valid mainbase here.
   int mainBaseID = kbBaseGetMainID(cMyID);
   debugEconomicBuildings("We're now going to scan for the closest Settlement to our mainbase that's on a connected area group.");
   int closestSettlementID = getClosestUnitByLocationConnectedAreaGroup(cUnitTypeSettlement, 0, cUnitStateAlive,
      kbBaseGetLocation(cMyID, mainBaseID), searchRange, cPassabilityAmphibious);
   if (closestSettlementID != -1)
   {
      vector settlementPosition = kbUnitGetPosition(closestSettlementID);
      // Don't expand towards enemies when we're meant to be passive.
      // Also don't do this when we're a retaliator since we can unintentionally get into combat.
      if (cPersonalityCurrent == cPersonalityPassive || cPersonalityCurrent == cPersonalityRetaliator)
      {   
         int enemyBaseID = kbFindClosestBase(-1, cPlayerRelationEnemyNotGaia, settlementPosition, cPassabilityAmphibious, false);
         if (enemyBaseID != -1)
         {
            int baseOwner = kbBaseGetOwner(enemyBaseID);
            float distance = xsVectorLength(settlementPosition - kbBaseGetLocation(baseOwner, enemyBaseID));
            // If we're within a range that we would engage the enemy because of the defend logic we won't claim it.
            if (distance < kbGetAutoMyBaseCreationDistanceTC() + 10.0)
            {
               debugEconomicBuildings("Found a settlement with ID: " + closestSettlementID + ", but it's too close to enemy base: " +
                  kbBaseGetNameByID(baseOwner, enemyBaseID) + ", not claiming it since we're " + getCurrentPersonalityName() + ".");
               return;
            }
         }
      }

      debugEconomicBuildings("Found a Settlement with ID: " + closestSettlementID + ", seeing if we can safely path to it now.");
      int mainBaseAreaID = kbAreaGetIDByPosition(kbBaseGetLocation(cMyID, mainBaseID));
      int areaID = kbAreaGetIDByPosition(settlementPosition);
      int pathID = kbPathCreate("tcExpansionMonitor");
      // Create a full path that doesn't check for danger. We do this because our builders will also take a straight path to the
      // foundation. If we avoid danger we could end up pathing around the enemies with this area path but our builders would
      // definitely walk through the enemy.
      if (kbPathCreateAreaPath(pathID, mainBaseAreaID, areaID, cPassabilityAmphibious) == false)
      {
         debugEconomicBuildings("Couldn't create an area path towards this settlement, but it's meant to be on " +
            "a connected area group. This must be blocked by a forest.");
         kbPathDestroy(pathID);
         return;
      }
      int numWaypoints = kbPathGetNumberWaypoints(pathID);
      float threshold = aiGetExploreDangerThreshold();
      for (int iWaypoint = 0; iWaypoint < numWaypoints; iWaypoint++)
      {
         int waypointAreaID = kbAreaGetIDByPosition(kbPathGetWaypoint(pathID, iWaypoint));
         debugEconomicBuildings("Analyzing waypoint: " + iWaypoint + ", areaID: " + waypointAreaID + ", center: " +
            kbAreaGetCenter(waypointAreaID) + ".");
         if (kbAreaGetDangerLevel(waypointAreaID) >= threshold)
         {
            debugEconomicBuildings("This area is too dangerous, aborting Town Center construction.");
            kbPathDestroy(pathID);
            return;
         }
         else
         {
            debugEconomicBuildings("This area is safe enough for us to continue");
         }
      }
      kbPathDestroy(pathID);

      gTCBuildPlanID = createSocketBuildPlan(cUnitTypeTownCenter, closestSettlementID, 55, cCalculateNumBuildersAutomatically, true);
      aiPlanSetEventHandler(gTCBuildPlanID, cPlanEventStateChange, "tcExpansionBPHandler");
      debugEconomicBuildings("Created a plan to create a new Town Center: " + aiPlanGetName(gTCBuildPlanID) + ".");
   }
   else
   {
      debugEconomicBuildings("Couldn't find a Settlement to claim within " + searchRange + " range from " +
         kbBaseGetNameByID(cMyID, mainBaseID) + ".");
   }
}

//==============================================================================
// cleanupGranaries
// We only delete Granaries that sit on our mgp, we don't like that.
//==============================================================================
void cleanupGranaries()
{
   int queryID = useSimpleUnitQuery(cUnitTypeGranary);
   int numResults = kbUnitQueryExecute(queryID);
   int[] dropsites = kbUnitQueryGetResults(queryID); // Have to save the results because we use useSimpleUnitQuery again.

   for (int i = 0; i < numResults; i++)
   {
      int dropsiteID = dropsites[i];
      debugEconomicBuildings("Analyzing Granary(" + dropsiteID + ").");

      int baseID = kbUnitGetBaseID(dropsiteID);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         debugEconomicBuildings("   Not deleting this dropsite since it's not in a TC base to begin with.");
         continue;
      }
      vector mgp = kbBaseGetMilitaryGatherPoint(cMyID, baseID);
      if (mgp == cInvalidVector)
      {
         continue;
      }
      vector unitPosition = kbUnitGetPosition(dropsiteID);

      bool hasClosebyResource = false;
      int[] resources = kbGetValidResourcesByPosition(unitPosition, cResourceFood, -1, 15.0);
      for (int iResource = 0; iResource < resources.size(); iResource++)
      {
         if (kbResourceGetSubType(resources[iResource]) == cAIResourceSubTypeFarm ||
             kbResourceGetSubType(resources[iResource]) == cUnitTypeFishResource)
         {
            continue;
         }
         // Resources have varying distances set, make sure the center is actually close to us.
         if (xsVectorDistance(kbResourceGetPosition(resources[iResource]), unitPosition) <= 15.0)
         {
            debugEconomicBuildings("   Granary is close to resource " + resources[iResource] + ", not deleting.");
            hasClosebyResource = true;
            break;
         }
      }
      if (hasClosebyResource == true)
      {
         continue;
      }

      // Farm resources are massive, we just check Farms here manually.
      int foodQueryID = useSimpleUnitQuery(cUnitTypeFarm, cMyID, cUnitStateABQ, unitPosition, 15.0);
      int numSurroundingFarms = kbUnitQueryExecute(foodQueryID);
      if (numSurroundingFarms == 0)
      {
         // Now we will delete if we're close to our MGP.
         if (xsVectorDistance(mgp, kbUnitGetPosition(dropsiteID)) <= 10.0)
         {
            debugEconomicBuildings("   Granary has no close food resources and is close to our MGP, deleting.");
            aiTaskDeleteUnit(dropsiteID);
         }
         else
         {
            debugEconomicBuildings("   Granary has no close food resources but isn't close to our MGP, not deleting.");
         }
      }
      else
      {
         debugEconomicBuildings("   Found " + numSurroundingFarms + " nearby Farms, no need to delete.");
      }
   }
}

//==============================================================================
// cleanupWoodGoldDropsiteType
//==============================================================================
void cleanupWoodGoldDropsiteType(int puid = -1, bool woodDropsite = false, bool goldDropsite = false)
{
   int queryID = useSimpleUnitQuery(puid);
   int numResults = kbUnitQueryExecute(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int dropsiteID = kbUnitQueryGetResult(queryID, i);
      debugEconomicBuildings("Analyzing " + kbProtoUnitGetName(kbUnitGetProtoUnitID(dropsiteID)) + "(" + dropsiteID + ").");

      int baseID = kbUnitGetBaseID(dropsiteID);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         // We only need to create room in the bases we actually build in, out on the map for remote gather bases we don't care.
         debugEconomicBuildings("   Not deleting this dropsite since it's not in a TC base.");
         continue;
      }

      int numSurroundingTrees = 0;
      if (woodDropsite == true)
      {
         numSurroundingTrees = getUnitCountByLocation(cUnitTypeTree, 0, cUnitStateAlive | cUnitStateDead,
            kbUnitGetPosition(dropsiteID), 15.0);
         debugEconomicBuildings("   Number Surrounding Trees = " + numSurroundingTrees + ".");
         if (numSurroundingTrees >= 1)
         {
            continue;
         }
      }

      int numSurroundingMines = 0;
      if (goldDropsite == true)
      {
         numSurroundingMines = getUnitCountByLocation(cUnitTypeGoldResource, 0, cUnitStateAlive,
             kbUnitGetPosition(dropsiteID), 15.0);
         debugEconomicBuildings("   Number Surrounding mines = " + numSurroundingMines + ".");
         if (numSurroundingMines == 0)
         {
            numSurroundingMines = getUnitCountByLocation(cUnitTypeGoldResource, 0, cUnitStateAlive,
                kbUnitGetPosition(dropsiteID), 24.0);
            debugEconomicBuildings("   Number Surrounding mines in extended amphibious range = " + numSurroundingMines + ".");
         }
         if (numSurroundingMines >= 1)
         {
            continue;
         }
      }

      debugEconomicBuildings("   Deleting dropsite because there are no surrounding trees/mines left.");
      aiTaskDeleteUnit(dropsiteID);
   }
}

//==============================================================================
// cleanupCustomFoodDropsites
// We could potentially save some of these in TC bases for Farms but it's pretty difficult to determine that unless
// we do the loops differently. It's not a big deal if we end up needing to replace some anyway.
// And since we don't avoid thes dropsites in avoidBlockingImportantSpots we can't guarantee they're unobstructed anyway.
// All of this is basically because these dropsites support more resources than just food, complicates things a lot...
//==============================================================================
void cleanupCustomFoodDropsites(int unitTypeID = -1, bool alsoWoodDropsite = false, bool alsoGoldDropsite = false)
{
   int queryID = useSimpleUnitQuery(unitTypeID);
   int numResults = kbUnitQueryExecute(queryID);
   int[] dropsites = kbUnitQueryGetResults(queryID); // Have to save the results because we use useSimpleUnitQuery again.
   int[] excludeTypes = new int(1, cUnitTypeFishResource);
   for (int i = 0; i < numResults; i++)
   {
      int dropsiteID = dropsites[i];
      debugEconomicBuildings("Analyzing " + kbProtoUnitGetName(unitTypeID) + "(" + dropsiteID + ").");

      int baseID = kbUnitGetBaseID(dropsiteID);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         // We only need to create room in the bases we actually build in, out on the map for remote gather bases we don't care.
         debugEconomicBuildings("   Not deleting this dropsite since it's not in a TC base.");
         continue;
      }
      vector unitPosition = kbUnitGetPosition(dropsiteID);

      if (alsoWoodDropsite == true)
      {
         int numSurroundingTrees = getUnitCountByLocation(cUnitTypeTree, 0, cUnitStateAlive | cUnitStateDead,
            unitPosition, 15.0);
         debugEconomicBuildings("   Number Surrounding Trees = " + numSurroundingTrees + ".");
         if (numSurroundingTrees >= 1)
         {
            continue;
         }
      }

      if (alsoGoldDropsite == true)
      {
         int numSurroundingMines = getUnitCountByLocation(cUnitTypeGoldResource, 0, cUnitStateAlive,
             unitPosition, 15.0);
         debugEconomicBuildings("   Number Surrounding mines = " + numSurroundingMines + ".");
         if (numSurroundingMines == 0)
         {
            numSurroundingMines = getUnitCountByLocation(cUnitTypeGoldResource, 0, cUnitStateAlive,
                unitPosition, 24.0);
            debugEconomicBuildings("   Number Surrounding mines in extended amphibious range = " + numSurroundingMines + ".");
         }
         if (numSurroundingMines >= 1)
         {
            continue;
         }
      }

      bool hasClosebyResource = false;
      int[] resources = kbGetValidResourcesByPosition(unitPosition, cResourceFood, -1, 15.0);
      for (int iResource = 0; iResource < resources.size(); iResource++)
      {
         if (kbResourceGetSubType(resources[iResource]) == cAIResourceSubTypeFarm ||
             kbResourceGetSubType(resources[iResource]) == cUnitTypeFishResource)
         {
            continue;
         }
         debugEconomicBuildings("   Found resource " + resources[iResource] + " which is potentially close enough.");
         // Resources have varying distances set, make sure the center is actually close to us.
         if (xsVectorDistance(kbResourceGetPosition(resources[iResource]), unitPosition) <= 15.0)
         {
            debugEconomicBuildings("   Silo is close to resource " + resources[iResource] + ", not deleting.");
            hasClosebyResource = true;
            break;
         }
      }
      if (hasClosebyResource == true)
      {
         continue;
      }

      // Farm resources are massive, we just check Farms here manually.
      int foodQueryID = useSimpleUnitQuery(cUnitTypeFarm, cMyID, cUnitStateABQ, unitPosition, 15.0);
      int numSurroundingFarms = kbUnitQueryExecute(foodQueryID);
      if (numSurroundingFarms == 0)
      {
         debugEconomicBuildings("   Silo has no close food/wood/gold resources, deleting.");
         aiTaskDeleteUnit(dropsiteID);
      }
      else
      {
         debugEconomicBuildings("   Found " + numSurroundingFarms + " nearby Farms, no need to delete.");
      }
   }
}

//==============================================================================
// dropsiteCleanupMonitor
// Cleans up dropsites.
//==============================================================================
rule dropsiteCleanupMonitor
group defaultClassicalRules
inactive
minInterval 180
{
   if (cMyCulture == cCultureAtlantean)
   {
      xsDisableRule("dropsiteCleanupMonitor");
      return;
   }

   if (checkStrategyFlag(cStrategyFlagAutomaticDropsiteCleanup) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule dropsiteCleanupMonitor. ---");

   switch (cMyCulture)
   {
      case cCultureGreek:
      {
         cleanupGranaries();
         cleanupWoodGoldDropsiteType(cUnitTypeStorehouse, true, true);
         break;
      }
      case cCultureEgyptian:
      {
         cleanupGranaries();
         cleanupWoodGoldDropsiteType(cUnitTypeLumberCamp, true, false);
         cleanupWoodGoldDropsiteType(cUnitTypeMiningCamp, false, true);
         break;
      }
      case cCultureChinese:
      {
         cleanupCustomFoodDropsites(cUnitTypeSilo, true, true);
         break;
      }
      case cCultureJapanese:
      {
         cleanupCustomFoodDropsites(cUnitTypeWatermill, true, false);
         cleanupWoodGoldDropsiteType(cUnitTypeMiningCampJapanese, false, true);
         break;
      }
   }
}

//==============================================================================
// wonderBuildStateChangeHandler
//==============================================================================
void wonderBuildStateChangeHandler(int planID = -1)
{
   static int numTries = 0;
   int state = aiPlanGetState(planID);
   switch (state)
   {
      case cPlanStateDone:
      {
         debugGodPowers("Wonder construction succeeded!");
         xsEnableRuleGroup("defaultWonderRules");
         xsRuleGroupIgnoreIntervalOnce("defaultWonderRules");
         numTries = 0;
         break;
      }
      case cPlanStateFailed:
      {
         debugGodPowers("Wonder construction failed, need to see if we must change our build plan parameters.");
         // TODO new logic with adding more safe back areas later.
         numTries++;
         break;
      }
   }
}

//==============================================================================
// haveEnoughExcessForWonder
//==============================================================================
bool haveEnoughExcessForWonder()
{
   // Wonder doesn't cost the same for all civs so dynamically check it all.
   float[] costs = kbProtoUnitGetCost(cUnitTypeWonder);
   for (int i = 0; i < cNumberResources; i++)
   {
      if (costs[i] == 0.0)
      {
         continue;
      }
      // If we're in early Deathmatch we need to have an insane excess to build it.
      // This prevents us instantly placing it down after the BO is done, this can now only happen on higher starting resources.
      if (cGameModeCurrent == cGameModeDeathmatch && xsGetTime() < 900)
      {
         if (haveExcessResourceAmount(20000, i) == false)
         {
            debugEconomicBuildings("Don't have enough excess resources of " + kbGetResourceName(i) + " to build a Wonder in early DM.");
            return false;
         }
         continue;
      }
      if (cPersonalityCurrent == cPersonalityMythical)
      {
         // For mythical personality we skip all resource needs, and just build it if we have enough resources banked.
         if (kbResourceGet(i) < costs[i])
         {
            debugEconomicBuildings("We don't have a big enough bank of " + kbGetResourceName(i) + " to build a Wonder instantly.");
            return false;
         }
      }
      else
      {
         if (haveExcessResourceAmount(costs[i], i) == false)
         {
            debugEconomicBuildings("Don't have enough excess resources of " + kbGetResourceName(i) + " to build a Wonder.");
            return false;
         }
      }
   }
   return true;
}

//==============================================================================
// wonderConstructionMonitor
//==============================================================================
rule wonderConstructionMonitor
group defaultMythicRules
inactive
minInterval 60
{
   if (cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("wonderConstructionMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagBuildWonder) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule wonderConstructionMonitor. ---");

   if (buildingGetNumberAliveAndPlanned(cUnitTypeWonder) >= 1)
   {
      debugEconomicBuildings("We already have a Wonder or are planning to build one, quiting.");
      return;
   }
   if (gDefensivelyOverrun == true)
   {
      debugEconomicBuildings("We're defensively overrun, quiting.");
      return;
   }

   if (haveEnoughExcessForWonder() == false)
   {
      return;
   }

   int safestBaseID = getMostDefendedTCBase();
   if (safestBaseID == -1)
   {
      debugEconomicBuildings("We currently have no TC base, can't build a Wonder.");
      return;
   }

   int planID = aiPlanCreate("Wonder Build Plan", cPlanBuild, -1, gEconomicBuildingsCategoryID);
   int bpID = kbBuildingPlacementCreate(aiPlanGetName(planID));
   kbBuildingPlacementSetBuildingPUID(bpID, cUnitTypeWonder);
   addSafeBackAreasToBuildingPlacement(bpID, safestBaseID, gEconomicBuildingsCategoryID);
   aiPlanSetVariableInt(planID, cBuildPlanBuildingPlacementID, 0, bpID);
   aiPlanSetVariableInt(planID, cBuildPlanBuildingTypeID, 0, cUnitTypeWonder);
   
   if (cMyCulture == cCultureChinese)
   {
      // Make sure this plan doesn't take all the Kuafu.
      int amountKuafu = kbUnitCount(cUnitTypeKuafu, cMyID, cUnitStateAlive) / 3; // 30% Kuafu.
      aiPlanAddUnitType(planID, cUnitTypeKuafu, amountKuafu, amountKuafu, amountKuafu);

      int amountVillager = max(5, kbUnitCount(cUnitTypeVillagerChinese, cMyID, cUnitStateAlive) / 5); // 20%.
      aiPlanAddUnitType(planID, cUnitTypeVillagerChinese, amountVillager, amountVillager, amountVillager);
   }
   else
   {
      int unitType = cUnitTypeAbstractVillager;
      int amount = 0;
      if (cMyCulture == cCultureNorse)
      {
         unitType = cUnitTypeLogicalTypeNorseSoldierThatBuilds;
         amount = max(5, kbUnitCount(unitType, cMyID, cUnitStateAlive) / 2); // 50%.
      }
      else
      {
         amount = max(5, kbUnitCount(unitType, cMyID, cUnitStateAlive) / 5); // 20%.
      }
      aiPlanAddUnitType(planID, unitType, amount, amount, amount);
   }

   aiPlanSetPriority(planID, 99);
   aiPlanSetEventHandler(planID, cPlanEventStateChange, "wonderBuildStateChangeHandler");
   debugEconomicBuildings("Created Wonder build plan!!!");
}

//==============================================================================
// wonderRepairMonitor
// Makes sure we keep our Wonder at full HP.
//==============================================================================
rule wonderRepairMonitor
group defaultWonderRules
inactive
minInterval 10
{
   debugEconomicBuildings("--- Running Rule wonderRepairMonitor. ---");

   int wonderID = getUnit(cUnitTypeWonder);
   if (wonderID == -1)
   {
      debugEconomicBuildings("We lost our Wonder, disabling all Wonder age rules now.");
      xsDisableRuleGroup("defaultWonderRules");
      return;
   }

   if (aiPlanGetIDByTypeAndVariableIntValue(cPlanRepair, cRepairPlanTargetID, wonderID) >= 0)
   {
      debugEconomicBuildings("We're already repairing our Wonder!");
      return;
   }

   // Start repairing if we took 10%+ damage.
   if (kbUnitGetStatFloat(wonderID, cUnitStatHPRatio) <= 0.9)
   {
      // We need to repair!
      debugEconomicBuildings("Our Wonder has been significantly damaged, creating a repair plan for it.");
      int planID = aiPlanCreate("Repair Wonder", cPlanRepair, -1, gEconomicBuildingsCategoryID);
      aiPlanSetVariableInt(planID, cRepairPlanTargetID, 0, wonderID);
      // Higher prio cuz we don't want to lose our Wonder obviously.
      aiPlanSetPriority(planID, 99);

      int unitType = cUnitTypeAbstractVillager;
      int amount = 0;
      if (cMyCulture == cCultureNorse)
      {
         unitType = cUnitTypeLogicalTypeNorseSoldierThatBuilds;
         amount = max(5, kbUnitCount(unitType, cMyID, cUnitStateAlive) / 4); // 25%.
      }
      else
      {
         amount = max(5, kbUnitCount(unitType, cMyID, cUnitStateAlive) / 10); // 10%.
      }
      aiPlanAddUnitType(planID, unitType, amount, amount, amount);
   }
}

//==============================================================================
// armoryMonitor
//==============================================================================
rule armoryMonitor
inactive
group defaultClassicalRules
minInterval 30
{
   if (checkStrategyFlag(cStrategyFlagBuildArmory) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule armoryMonitor. ---");
   int currentAge = kbPlayerGetAge(cMyID);

   // Baseline upgrades active.
   // Only start the disabling progress if we're already in Heroic, we can already have all upgrades before if we're in DM Thor.
   if (currentAge >= cAge3 &&
       kbTechGetStatus(cTechIronWeapons) == cTechStatusActive &&
       kbTechGetStatus(cTechIronArmor) == cTechStatusActive &&
       kbTechGetStatus(cTechIronShields) == cTechStatusActive &&
       kbTechGetStatus(cTechBallistics) == cTechStatusActive &&
       kbTechGetStatus(cTechBurningPitch) == cTechStatusActive)
   {
      if (cMyCulture == cCultureGreek)
      {
         if (currentAge >= cAge4)
         {
            if (kbTechGetStatus(cTechMythicAgeHephaestus) == cTechStatusActive)
            {
               if (kbTechGetStatus(cTechOlympianWeapons) == cTechStatusActive &&
                   kbTechGetStatus(cTechForgeOfOlympus) == cTechStatusActive)
               {
                  xsDisableRule("armoryMonitor");
                  return;
               }
            }
            else
            {
               // Aged up with Hera, can just disable now.
               xsDisableRule("armoryMonitor");
               return;
            }
         }
      }
      else if (cMyCiv == cCivLoki)
      {
         if (kbTechGetStatus(cTechClassicalAgeForseti) == cTechStatusActive)
         {
            if (kbTechGetStatus(cTechDwarvenBreastplate) == cTechStatusActive)
            {
               xsDisableRule("armoryMonitor");
               return;
            }
         }
         else
         {
            xsDisableRule("armoryMonitor");
            return;
         }
      }
      else if (cMyCiv == cCivThor)
      {
         bool canDisable = true;
         if (kbTechGetStatus(cTechClassicalAgeForseti) == cTechStatusActive)
         {
            if (kbTechGetStatus(cTechDwarvenBreastplate) == cTechStatusObtainable)
            {
               canDisable = false;
            }
         }
         if (canDisable == true && kbTechGetStatus(cTechDwarvenWeapons) == cTechStatusObtainable)
         {
            canDisable = false;
         }
         if (canDisable == true && kbTechGetStatus(cTechMeteoricIronArmor) == cTechStatusObtainable)
         {
            canDisable = false;
         }
         if (canDisable == true && kbTechGetStatus(cTechDragonscaleShields) == cTechStatusObtainable)
         {
            canDisable = false;
         }
         if (canDisable == true)
         {
            xsDisableRule("armoryMonitor");
            return;
         }
      }
      else
      {
         // No unique upgrades to get, can just disable.
         xsDisableRule("armoryMonitor");
         return;
      }
   }

   if (kbUnitCount(gArmoryUnit, cMyID, cUnitStateAlive) >= 1)
   {
      debugEconomicBuildings("We already have an " + kbProtoUnitGetName(gArmoryUnit) + "  alive, quiting.");
      return;
   }
   int randomTCBaseID = getRandomTownCenterBaseID();
   if (randomTCBaseID == -1)
   {
      debugEconomicBuildings("Can't build a new " + kbProtoUnitGetName(gArmoryUnit) + " since we have no TC bases left.");
      return;
   }
   
   int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, gArmoryUnit);
   if (currentAge == cAge2 && kbUnitCount(gMarketUnit, cMyID, cUnitStateAlive) == 0 &&
       ((kbResourceGet(cResourceFood) > 800 && kbResourceGet(cResourceGold) > 500) ||
       (aiPlanGetIsIDValid(gAgeUpResearchPlan) == true && aiPlanGetPriority(gAgeUpResearchPlan) > 50)))
   {
      debugEconomicBuildings("We have enough resources to age up to Heroic/have high age up prio but have no Armory/Market, " +
         "creating an " + kbProtoUnitGetName(gArmoryUnit) + " with all haste!");
      if (aiPlanGetIsIDValid(planID) == true)
      {
         if (aiPlanGetPriority(planID) != 100)
         {
            debugEconomicBuildings("Existing " + aiPlanGetName(planID) + " found that we're now bumping the " + 
               "priority on to 100.");
            aiPlanSetPriority(planID, 100);
         }
         else
         {
            debugEconomicBuildings("Our existing " + aiPlanGetName(planID) + " requires no changes.");
         }
      }
      else
      {
         createSimpleBuildPlan(gArmoryUnit, 1, 100, gEconomicBuildingsCategoryID, randomTCBaseID);
      }
      return;
   }
   else
   {
      if (aiPlanGetIsIDValid(planID) == true && aiPlanGetPriority(planID) != 50)
      {
         debugEconomicBuildings("Existing " + kbProtoUnitGetName(gArmoryUnit) + " build plan found that doesn't require the highest " +
            "priority anymore, setting it to 50.");
         aiPlanSetPriority(planID, 50);
      }
   }

   if (aiPlanGetIsIDValid(planID) == true)
   {
      debugEconomicBuildings("We already have a build plan for " + kbProtoUnitGetName(gArmoryUnit) + ".");
      return;
   }
   
   // Egyptian Armory is free so just build it.
   if (cMyCulture == cCultureEgyptian)
   {
      debugEconomicBuildings("We are an Egyptian, instantly building an Armory.");
      // As Egyptian we don't want to instantly put 5 builders on this when we hit Classical, just slowly build it.
      createSimpleBuildPlan(gArmoryUnit, 1, 50, gEconomicBuildingsCategoryID, randomTCBaseID, 1);
      return;
   }

   if (haveExcessResourceAmount(300.0, cResourceWood) == true)
   {
      debugEconomicBuildings("We have a lot of excess wood, building an " + kbProtoUnitGetName(gArmoryUnit) + " now!");
      createSimpleBuildPlan(gArmoryUnit, 1, 50, gEconomicBuildingsCategoryID, randomTCBaseID);
      return;
   }

   if (gAgeUpTimes[cAge2] + 240 < xsGetTime())
   {
      debugEconomicBuildings("We're already in Classical for 4 minutes, start building an " + kbProtoUnitGetName(gArmoryUnit) + ".");
      createSimpleBuildPlan(gArmoryUnit, 1, 50, gEconomicBuildingsCategoryID, randomTCBaseID);
   }
   else
   {
      debugEconomicBuildings("We can build an " + kbProtoUnitGetName(gArmoryUnit) + " at: " +
         turnNumberIntoTimeDisplay(gAgeUpTimes[cAge2] + 300) + ".");
   }
}

//==============================================================================
// economicGuildMonitor
// Build only 1 Economic Guild at a time max.
//==============================================================================
rule economicGuildMonitor
inactive
group defaultClassicalRules
minInterval 60
{
   if (cMyCulture != cCultureAtlantean)
   {
      xsDisableRule("economicGuildMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagBuildEconomicGuild) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule economicGuildMonitor. ---");

   int planID = -1;
   if (kbTechGetStatus(cTechHusbandry) == cTechStatusActive &&
       kbTechGetStatus(cTechPlow) == cTechStatusActive &&
       kbTechGetStatus(cTechIrrigation) == cTechStatusActive &&
       kbTechGetStatus(cTechFloodControl) == cTechStatusActive &&
       kbTechGetStatus(cTechHuntingEquipment) == cTechStatusActive &&
       kbTechGetStatus(cTechHandAxe) == cTechStatusActive &&
       kbTechGetStatus(cTechBowSaw) == cTechStatusActive &&
       kbTechGetStatus(cTechCarpenters) == cTechStatusActive &&
       kbTechGetStatus(cTechPickaxe) == cTechStatusActive &&
       kbTechGetStatus(cTechShaftMine) == cTechStatusActive &&
       kbTechGetStatus(cTechQuarry) == cTechStatusActive)
   {
      // If we can get Prometheus we must make sure we also get Theft of Fire.
      // If we're Kronos or Oranos we can't hit this part until we're at least in Classical because
      // Hunting Equipment is locked behind Heroic. So we don't need to do an age check.
      if (kbTechGetStatus(cTechClassicalAgePrometheus) == cTechStatusActive)
      {
         if (kbTechGetStatus(cTechTheftOfFire) == cTechStatusActive)
         {
            planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeEconomicGuild);
            if (aiPlanGetIsIDValid(planID) == true)
            {
               aiPlanDestroy(planID);
            }
            xsDisableRule("economicGuildMonitor");
            return;
         }
      }
      else
      {
         planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeEconomicGuild);
         if (aiPlanGetIsIDValid(planID) == true)
         {
            aiPlanDestroy(planID);
         }
         xsDisableRule("economicGuildMonitor");
         return;
      }
   }
   
   if (kbUnitCount(cUnitTypeEconomicGuild, cMyID, cUnitStateAlive) >= 1 ||
       aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeEconomicGuild) != -1)
   {
      debugEconomicBuildings("We already have an Economic Guild or already have a plan to build one, quiting.");
      return;
   }

   int baseID = getRandomTownCenterBaseID();
   if (baseID == -1)
   {
      debugEconomicBuildings("Can't build a new Economic Guild since we have no TC bases left.");
      return;
   }

   createSimpleBuildPlan(cUnitTypeEconomicGuild, 1, 50, gEconomicBuildingsCategoryID, baseID, 1);
}

//==============================================================================
// monumentMonitor
// Build only 1 Monument at a time max.
// Also artificially lock Monuments behind ages.
//==============================================================================
rule monumentMonitor
inactive
group defaultArchaicRules
minInterval 60
{
   if (cMyCulture != cCultureEgyptian)
   {
      xsDisableRule("monumentMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagBuildMonuments) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule monumentMonitor. ---");

   int baseID = getRandomTownCenterBaseID();
   if (baseID == -1)
   {
      debugEconomicBuildings("Can't build a new Monument since we have no TC bases left.");
      return;
   }
   int age = kbPlayerGetAge(cMyID);

   bool monumentVillagersAlive = kbUnitCount(cUnitTypeMonumentToVillagers, cMyID, cUnitStateAlive) >= 1;
   if (monumentVillagersAlive == false)
   {
      debugEconomicBuildings("We currently don't have a Monument To Villagers alive.");
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeMonumentToVillagers) == -1)
      {
         createSimpleBuildPlan(cUnitTypeMonumentToVillagers, 1, 50, gEconomicBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      }
      else
      {
         debugEconomicBuildings("We however do already have a build plan for it.");
      }
      return;
   }

   // We need the minimum amount of favor income for Humanoid so that we can build Migdol Strongholds.
   if (cPersonalityCurrent == cPersonalityHumanoid)
   {
      return;
   }
   if (age < cAge2)
   {
      debugEconomicBuildings("We're in the Archaic Age, can't build subsequent Monuments yet.");
      return;
   }

   bool monumentSoldiersAlive = kbUnitCount(cUnitTypeMonumentToSoldiers, cMyID, cUnitStateAlive) >= 1;
   if (monumentSoldiersAlive == false)
   {
      debugEconomicBuildings("We currently don't have a Monument To Soldiers alive.");
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeMonumentToSoldiers) == -1)
      {
         createSimpleBuildPlan(cUnitTypeMonumentToSoldiers, 1, 50, gEconomicBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      }
      else
      {
         debugEconomicBuildings("We however do already have a build plan for it.");
      }
      return;
   }

   if (age < cAge3)
   {
      debugEconomicBuildings("We're in the Classical Age, can't build subsequent Monuments yet.");
      return;
   }

   bool monumentPriestsAlive = kbUnitCount(cUnitTypeMonumentToPriests, cMyID, cUnitStateAlive) >= 1;
   if (monumentPriestsAlive == false)
   {
      debugEconomicBuildings("We currently don't have a Monument To Priests alive.");
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeMonumentToPriests) == -1)
      {
         createSimpleBuildPlan(cUnitTypeMonumentToPriests, 1, 50, gEconomicBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      }
      else
      {
         debugEconomicBuildings("We however do already have a build plan for it.");
      }
      return;
   }

   if (age < cAge4)
   {
      debugEconomicBuildings("We're in the Heroic Age, can't build subsequent Monuments yet.");
      return;
   }

   bool monumentPharaohsAlive = kbUnitCount(cUnitTypeMonumentToPharaohs, cMyID, cUnitStateAlive) >= 1;
   if (monumentPharaohsAlive == false)
   {
      debugEconomicBuildings("We currently don't have a Monument To Pharaohs alive.");
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeMonumentToPharaohs) == -1)
      {
         createSimpleBuildPlan(cUnitTypeMonumentToPharaohs, 1, 50, gEconomicBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      }
      else
      {
         debugEconomicBuildings("We however do already have a build plan for it.");
      }
      return;
   }
   
   bool monumentGodsAlive = kbUnitCount(cUnitTypeMonumentToGods, cMyID, cUnitStateAlive) >= 1;
   if (monumentGodsAlive == false)
   {
      debugEconomicBuildings("We currently don't have a Monument To Gods alive.");
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeMonumentToGods) == -1)
      {
         createSimpleBuildPlan(cUnitTypeMonumentToGods, 1, 50, gEconomicBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      }
      else
      {
         debugEconomicBuildings("We however do already have a build plan for it.");
      }
      return;
   }
}

//==============================================================================
// marketMonitor
// Builds a Market in one of our TC bases if we don't already have one.
// The main use of this is to already be able to get a Market in Classical to trade with.
//==============================================================================
rule marketMonitor
inactive
group defaultClassicalRules
minInterval 60
{
   if (checkStrategyFlag(cStrategyFlagBuildMarket) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule marketMonitor. ---");

   int randomTCBaseID = getRandomTownCenterBaseID();
   if (randomTCBaseID == -1)
   {
      debugEconomicBuildings("Can't build a new Market since we have no TC bases left.");
      return;
   }
   if (buildingGetNumberAliveAndPlanned(gMarketUnit) >= 1)
   {
      debugEconomicBuildings("We already have a Market or a build plan for it, quiting.");
      return;
   }

   // We need a market as Supporter to tribute to our allies. Egyptian Market is free so just build it.
   if (cPersonalityCurrent == cPersonalitySupporter || cMyCulture == cCultureEgyptian)
   {
      debugEconomicBuildings("We are a Supporter or Egyptian, instantly building a Market.");
      // As Egyptian we don't want to instantly put 5 builders on this when we hit Classical, just slowly build it.
      createSimpleBuildPlan(gMarketUnit, 1, 50, gEconomicBuildingsCategoryID,
         randomTCBaseID, cMyCulture == cCultureEgyptian ? 1 : cCalculateNumBuildersAutomatically);
      return;
   }

   if (haveExcessResourceAmount(300.0, cResourceWood) == true)
   {
      debugEconomicBuildings("We have a lot of excess wood, building a Market now!");
      createSimpleBuildPlan(gMarketUnit, 1, 50, gEconomicBuildingsCategoryID, randomTCBaseID);
      return;
   }

   if (gAgeUpTimes[cAge2] + 300 < xsGetTime())
   {
      debugEconomicBuildings("We're already in Classical for 5 minutes, start building a Market.");
      createSimpleBuildPlan(gMarketUnit, 1, 50, gEconomicBuildingsCategoryID, randomTCBaseID);
   }
   else
   {
      debugEconomicBuildings("We can build a Market at: " + turnNumberIntoTimeDisplay(gAgeUpTimes[cAge2] + 300) + ".");
   }
}

//==============================================================================
// shrineMonitor
// Builds a shrine for each Miko we own.
//==============================================================================
rule shrineMonitor
inactive
group defaultArchaicRules
minInterval 30
{
   if (cMyCulture != cCultureJapanese)
   {
      xsDisableRule("shrineMonitor");
      return;
   }
   static int mikoReservePlanID = -1;
   if (checkStrategyFlag(cStrategyFlagAutomaticMikoManagement) == false)
   {
      if (aiPlanGetIsIDValid(mikoReservePlanID) == true)
      {
         aiPlanDestroy(mikoReservePlanID);
      }
      mikoReservePlanID = -1;
      return;
   }
   debugEconomicBuildings("--- Running Rule shrineMonitor. ---");

   if (aiPlanGetIsIDValid(mikoReservePlanID) == false)
   {
      mikoReservePlanID = aiPlanCreate("Miko reserve plan", cPlanReserve, -1, gEconomicBuildingsCategoryID);
      aiPlanSetPriority(mikoReservePlanID, 100);
      aiPlanAddUnitType(mikoReservePlanID, cUnitTypeMiko, 100, 100, 100);
      aiPlanSetFlag(mikoReservePlanID, cPlanFlagNoMoreUnits, true); // Prevent auto assignment.
   }

   int randomTCBaseID = getRandomTownCenterBaseID();
   if (randomTCBaseID == -1)
   {
      debugEconomicBuildings("Can't build a new Shrine since we have no TC bases left.");
      return;
   }

   // We need to know how many empty Shrines we have that we can assign Mikos to.
   // A Shrine is empty when there are no Mikos currently working on it.
   int shrineQueryID = useSimpleUnitQuery(cUnitTypeShrineJapanese);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(shrineQueryID, gLandAreaGroupID, cPassabilityAmphibious);
   }
   int numShrineResults = kbUnitQueryExecute(shrineQueryID);
   int[] availableShrines = new int(0, 0);
   for (int i = 0; i < numShrineResults; i++)
   {
      int shrineID = kbUnitQueryGetResult(shrineQueryID, i);
      int baseID = kbUnitGetBaseID(shrineID);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         bool needToSkip = true;
         // In campaigns we may have Shrines outside of our TC base since the designer can place them wherever.
         if (cGameTypeCurrent == cGameTypeCampaign || cGameTypeCurrent == cGameTypeScenario)
         {
            int areaID = kbUnitGetAreaID(shrineID);
            if (kbAreaGetDangerLevel(areaID, false) <= 100.0)
            {
               needToSkip = false;
            }
         }
         if (needToSkip == true)
         {
            // Keep our Miko close to our TCs for reliable gathering.
            debugEconomicBuildings("Skipping Shrine(" + shrineID + ") because it's no longer in a TC base, expect it's too dangerous.");
            continue;
         }
      }

      int numWorkers = kbUnitGetNumberWorkers(shrineID);
      if (numWorkers == 0)
      {
         debugEconomicBuildings("Shrine(" + shrineID + ") is available to work on.");
         availableShrines.add(shrineID);
         continue;
      }
      bool foundWorkingMiko = false;
      for (int iWorker = 0; iWorker < numWorkers; iWorker++)
      {
         int workerID = kbUnitGetWorkerID(shrineID, iWorker);
         if (kbUnitGetIsIDValid(workerID) == false)
         {
            continue;
         }
         if (kbUnitGetPlayerID(workerID) != cMyID)
         {
            continue;
         }
         if (kbUnitGetProtoUnitID(workerID) == cUnitTypeMiko)
         {
            debugEconomicBuildings("Shrine(" + shrineID + ") already has a Miko(" + workerID + ") working on it.");
            foundWorkingMiko = true;
            break;
         }
      }
      if (foundWorkingMiko == false)
      {
         debugEconomicBuildings("Shrine(" + shrineID + ") is available to work on.");
         availableShrines.add(shrineID);
      }
   }

   bool canAssignToExtingBuildPlan = false;
   int existingShrineBuildPlanID = aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeShrineJapanese);
   if (existingShrineBuildPlanID != -1 && aiPlanGetNumberUnits(existingShrineBuildPlanID, -1, true) == 0)
   {
      debugEconomicBuildings("Found plan: " + aiPlanGetName(existingShrineBuildPlanID) + " that can be assigned a Miko.");
      canAssignToExtingBuildPlan = true;
   }

   int mikoQueryID = useSimpleUnitQuery(cUnitTypeMiko);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(mikoQueryID, gLandAreaGroupID, cPassabilityAmphibious);
   }
   int numMikoResults = kbUnitQueryExecute(mikoQueryID);
   for (int i = 0; i < numMikoResults; i++)
   {
      int mikoID = kbUnitQueryGetResult(mikoQueryID, i);
      if (isUnitAlreadyInPlanOrChildOf(mikoID, mikoReservePlanID) == false)
      {
         debugEconomicBuildings("Found Miko(" + mikoID + ") to add to our reserve plan.");
         aiPlanAddUnit(mikoReservePlanID, mikoID);
      }
      int planID = kbUnitGetPlanID(mikoID);
      if (aiPlanGetType(planID) == cPlanBuild)
      {
         debugEconomicBuildings("Miko(" + mikoID + ") is already building a Shrine in " + aiPlanGetName(planID) + ".");
         continue;
      }
      int targetID = kbUnitGetTargetUnitID(mikoID);
      if (targetID != -1 && kbUnitGetProtoUnitID(targetID) == cUnitTypeShrineJapanese)
      {
         debugEconomicBuildings("Miko(" + mikoID + ") is already working on Shrine(" + targetID + ").");
         continue;
      }

      // This Miko needs a Shrine, either take an existing one that's free or build a new one.
      if (availableShrines.size() > 0)
      {
         debugEconomicBuildings("Tasking Miko(" + mikoID + ") to Shrine(" + availableShrines[0] + ").");
         aiTaskWorkUnit(mikoID, availableShrines[0]);
         availableShrines.removeIndex(0);
         continue;
      }

      if (canAssignToExtingBuildPlan == true)
      {
         debugEconomicBuildings("Adding Miko(" + mikoID + ") to " + aiPlanGetName(existingShrineBuildPlanID) + ".");
         aiPlanAddUnit(existingShrineBuildPlanID, mikoID);
         canAssignToExtingBuildPlan = false;
         continue;
      }

      // Don't stack multiple build plans because they need to influence each other, 1 Shrine can't be next to another.
      if (existingShrineBuildPlanID != -1)
      {
         debugEconomicBuildings("Miko(" + mikoID + ") - We can't make more Shrine build plans currently.");
         continue;
      }

      // Create a new Shrine build plan and assign the Miko to it, which will be a loan.
      existingShrineBuildPlanID = createSimpleBuildPlan(cUnitTypeShrineJapanese, 1, 55, gEconomicBuildingsCategoryID,
         randomTCBaseID, 0, mikoReservePlanID);
      aiPlanSetEventHandler(existingShrineBuildPlanID, cPlanEventStateChange, "shrineBuildPlanHandler");
      // Prevents the Miko being kicked out before we have the foundation placed.
      aiPlanSetFlag(existingShrineBuildPlanID, cPlanFlagReadyForUnits, true);
      aiPlanAddUnit(existingShrineBuildPlanID, mikoID);
   }
}

//==============================================================================
// calpulliVariationMonitor
//==============================================================================
rule calpulliVariationMonitor
inactive
group defaultClassicalRules
minInterval 30
{
   if (checkStrategyFlag(cStrategyFlagAutomaticCalpulliVariations) == false)
   {
      return;
   }
   debugEconomicBuildings("--- Running Rule calpulliVariationMonitor. ---");

   // Track which Calpullis have what addition. Since there's only ever 1 of each we can do it this way.
   static int livestockPenID = -1;
   static int warRoomID = -1;
   static int craftWorkshopID = -1;

   const int cMinimumRequiredFarmCountLivestockPen = 5;

   int livestockPenResearchPlanID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID,
      cTechCalpulliToLivestockPen);
   int warRoomResearchPlanID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID,
      cTechCalpulliToLumberOutpost);
   int craftWorkshopResearchPlanID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID,
      cTechCalpulliToCraftWorkshop);

   // Check if our Calpullis are still alive at all.
   if (livestockPenID != -1 && kbUnitGetIsIDValid(livestockPenID) == false)
   {
      if (livestockPenResearchPlanID >= 0)
      {
         aiPlanDestroy(livestockPenResearchPlanID);
         livestockPenResearchPlanID = -1;
      }
      livestockPenID = -1;
      debugEconomicBuildings("Our livestock pen Calpulli died, we can try and make a new one now.");
   }
   if (warRoomID != -1 && kbUnitGetIsIDValid(warRoomID) == false)
   {
      if (warRoomResearchPlanID >= 0)
      {
         aiPlanDestroy(warRoomResearchPlanID);
         warRoomResearchPlanID = -1;
      }
      warRoomID = -1;
      debugEconomicBuildings("Our war room Calpulli died, we can try and make a new one now.");
   }
   if (craftWorkshopID != -1 && kbUnitGetIsIDValid(craftWorkshopID) == false)
   {
      if (craftWorkshopResearchPlanID >= 0)
      {
         aiPlanDestroy(craftWorkshopResearchPlanID);
         craftWorkshopResearchPlanID = -1;
      }
      craftWorkshopID = -1;
      debugEconomicBuildings("Our craft workshop Calpulli died, we can try and make a new one now.");
   }

   // Check if our modified Calpullis should remain modified.
   // If they shouldn't remain modified it doesn't mean we will turn them back to normal instantly.
   // We just allow others to take their place.
   if (livestockPenID != -1)
   {
      int farmCount = getUnitCountByLocation(cUnitTypeAbstractFarm, cMyID, cUnitStateABQ, kbUnitGetPosition(livestockPenID), 15.0);
      if (farmCount < cMinimumRequiredFarmCountLivestockPen)
      {
         debugEconomicBuildings("Our livestock pen Calpulli: " + livestockPenID + " no longer has sufficient Farms close: "
            + farmCount + ", we can now select another Calpulli to modify.");
         livestockPenID = -1;
         if (livestockPenResearchPlanID >= 0)
         {
            aiPlanDestroy(livestockPenResearchPlanID);
            livestockPenResearchPlanID = -1;
         }
      }
      else
      {
         debugEconomicBuildings("Our livestock pen Calpulli: " + livestockPenID + " still has sufficient Farms close: " + farmCount +
            ", do nothing.");
      }
   }

   if (warRoomID != -1)
   {
      int numPlans = kbUnitGetNumberUsedByPlans(warRoomID);
      bool foundWoodGatherPlan = false;
      for (int i = 0; i < numPlans; i++)
      {
         int planID = kbUnitGetUsedByPlanID(warRoomID, i);
         if (aiPlanGetIsIDValid(planID) == false)
         {
            debugEconomicBuildings("---  FAIL");
            continue;
         }
         if (aiPlanGetType(planID) == cPlanGather && aiPlanGetVariableInt(planID, cGatherPlanResourceType, 0) == cResourceWood)
         {
            foundWoodGatherPlan = true;
            break;
         }
      }
      if (foundWoodGatherPlan == false)
      {
         debugEconomicBuildings("Our war room Calpulli: " + warRoomID + " no longer has a wood gather plan associated with it" +
            ", we can now select another Calpulli to modify.");
         warRoomID = -1;
         if (warRoomResearchPlanID >= 0)
         {
            aiPlanDestroy(warRoomResearchPlanID);
            warRoomResearchPlanID = -1;
         }
      }
      else
      {
         debugEconomicBuildings("Our war room Calpulli: " + warRoomID + " still has a wood gather plan associated with it" +
            ", do nothing.");
      }
   }

   if (craftWorkshopID != -1)
   {
      int numPlans = kbUnitGetNumberUsedByPlans(craftWorkshopID);
      bool foundGoldGatherPlan = false;
      for (int i = 0; i < numPlans; i++)
      {
         int planID = kbUnitGetUsedByPlanID(craftWorkshopID, i);
         if (aiPlanGetIsIDValid(planID) == false)
         {
            debugEconomicBuildings("---  FAIL");
            continue;
         }
         if (aiPlanGetType(planID) == cPlanGather && aiPlanGetVariableInt(planID, cGatherPlanResourceType, 0) == cResourceGold)
         {
            foundGoldGatherPlan = true;
            break;
         }
      }
      if (foundGoldGatherPlan == false)
      {
         debugEconomicBuildings("Our craft workshop Calpulli: " + craftWorkshopID + " no longer has a gold gather plan associated with it" +
            ", we can now select another Calpulli to modify.");
         craftWorkshopID = -1;
         if (craftWorkshopResearchPlanID >= 0)
         {
            aiPlanDestroy(craftWorkshopResearchPlanID);
            craftWorkshopResearchPlanID = -1;
         }
      }
      else
      {
         debugEconomicBuildings("Our craft workshop Calpulli: " + craftWorkshopID + " still has a gold gather plan associated with it" +
            ", do nothing.");
      }
   }

   ///////////////////////////////////////
   // Part 2: analyze new Calpullis to transform.
   ///////////////////////////////////////

   if (livestockPenID >= 0 && warRoomID >= 0 && craftWorkshopID >= 0)
   {
      debugEconomicBuildings("We already have all three Calpulli variations either active, can't get new ones.");
      return;
   }
   
   if (livestockPenResearchPlanID >= 0 || warRoomResearchPlanID >= 0 || craftWorkshopResearchPlanID >= 0)
   {
      debugEconomicBuildings("We already have an active research plan going, don't stack them.");
      return;
   }
   
   // Get the info we will need for all 3 variations.
   int queryID = useSimpleUnitQuery(cUnitTypeCalpulli);
   kbUnitQuerySetAreaGroupID(queryID, gLandAreaGroupID);
   int calpulliCount = kbUnitQueryExecute(queryID);

   if (livestockPenID == -1 && livestockPenResearchPlanID == -1)
   {
      bool madePlan = false;
      for (int i = 0; i < calpulliCount; i++)
      {
         int calpulliID = kbUnitQueryGetResult(queryID, i);
         int farmCount = getUnitCountByLocation(cUnitTypeAbstractFarm, cMyID, cUnitStateABQ, kbUnitGetPosition(calpulliID), 15.0);
         if (farmCount >= cMinimumRequiredFarmCountLivestockPen)
         {
            debugEconomicBuildings("Found Calpulli " + calpulliID + " with " + farmCount +
               " Farms around it, transform it into a livestock pen.");
            int planID = createSimpleResearchPlanSpecificResearcher(cTechCalpulliToLivestockPen, calpulliID);
            aiPlanSetVariableInt(planID, cResearchPlanTransformToPUID, 0, cUnitTypeCalpulliLivestockPen);
            livestockPenID = calpulliID;
            madePlan = true;
            break;
         }
      }
      if (madePlan == false)
      {
         debugEconomicBuildings("Didn't find a suitable Calpulli to transform into a livestock pen.");
      }
      else
      {
         return; // Create one plan at a time.
      }
   }
   else
   {
      debugEconomicBuildings("We already have a proper livestock pen Calpulli, or a research plan to get one.");
   }

   if (warRoomID == -1 && warRoomResearchPlanID == -1)
   {
      bool madePlan = false;
      for (int i = 0; i < calpulliCount; i++)
      {
         int calpulliID = kbUnitQueryGetResult(queryID, i);
         int numPlans = kbUnitGetNumberUsedByPlans(calpulliID);
         bool foundWoodGatherPlan = false;
         for (int j = 0; j < numPlans; j++)
         {
            int planID = kbUnitGetUsedByPlanID(calpulliID, j);
            if (aiPlanGetIsIDValid(planID) == false)
            {
               debugEconomicBuildings("---  FAIL");
               continue;
            }
            if (aiPlanGetType(planID) == cPlanGather && aiPlanGetVariableInt(planID, cGatherPlanResourceType, 0) == cResourceWood)
            {
               foundWoodGatherPlan = true;
               break;
            }
         }
         if (foundWoodGatherPlan == true)
         {
            debugEconomicBuildings("Found Calpulli " + calpulliID + " with an active wood gather plan, transform it into a war room.");
            int planID = createSimpleResearchPlanSpecificResearcher(cTechCalpulliToLumberOutpost, calpulliID);
            aiPlanSetVariableInt(planID, cResearchPlanTransformToPUID, 0, cUnitTypeCalpulliLumberOutpost);
            warRoomID = calpulliID;
            madePlan = true;
            break;
         }
      }
      if (madePlan == false)
      {
         debugEconomicBuildings("Didn't find a suitable Calpulli to transform into a war room.");
      }
      else
      {
         return; // Create one plan at a time.
      }
   }
   else
   {
      debugEconomicBuildings("We already have a proper war room Calpulli, or a research plan to get one.");
   }

   if (craftWorkshopID == -1 && craftWorkshopResearchPlanID == -1)
   {
      bool madePlan = false;
      for (int i = 0; i < calpulliCount; i++)
      {
         int calpulliID = kbUnitQueryGetResult(queryID, i);
         int numPlans = kbUnitGetNumberUsedByPlans(calpulliID);
         bool foundGoldGatherPlan = false;
         for (int j = 0; j < numPlans; j++)
         {
            int planID = kbUnitGetUsedByPlanID(calpulliID, j);
            if (aiPlanGetIsIDValid(planID) == false)
            {
               debugEconomicBuildings("---  FAIL");
               continue;
            }
            if (aiPlanGetType(planID) == cPlanGather && aiPlanGetVariableInt(planID, cGatherPlanResourceType, 0) == cResourceGold)
            {
               foundGoldGatherPlan = true;
               break;
            }
         }
         if (foundGoldGatherPlan == true)
         {
            debugEconomicBuildings("Found Calpulli " + calpulliID + " with an active gold gather plan, transform it into a craft workshop.");
            int planID = createSimpleResearchPlanSpecificResearcher(cTechCalpulliToCraftWorkshop, calpulliID);
            aiPlanSetVariableInt(planID, cResearchPlanTransformToPUID, 0, cUnitTypeCalpulliCraftWorkshop);
            craftWorkshopID = calpulliID;
            madePlan = true;
            break;
         }
      }
      if (madePlan == false)
      {
         debugEconomicBuildings("Didn't find a suitable Calpulli to transform into a craft workshop.");
      }
      else
      {
         return; // Create one plan at a time.
      }
   }
   else
   {
      debugEconomicBuildings("We already have a proper craft workshop Calpulli, or a research plan to get one.");
   }
}
