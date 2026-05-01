//==============================================================================
/* human_assist.xs

   Creates an AI for Retold that just prepares the kb & sets up some basic plans for the Human players to use for
   things like auto-scout and managing resource gatherers.

*/
//==============================================================================

extern int gReservePlan = -1;
include "human_assist/human_assist_debug.xs";
include "human_assist/human_assist_unit_queries.xs";
include "human_assist/human_assist_resource_breakdown_system.xs";

mutable void setDistributionNumbers(int food = 0, int wood = 0, int gold = 0) {}

const bool cAllowManualDistribution = false;
const bool cAllowDropsiteConstruction = false;

bool gSentFoodNotification = false;
bool gSentWoodNotification = false;
bool gSentGoldNotification = false;
bool gHaveFoodBlossom = false;
bool gHaveWoodBlossom = false;
bool gHaveGoldBlossom = false;
bool gNeedToUseFoodBlossom = false;
bool gNeedToUseWoodBlossom = false;
bool gNeedToUseGoldBlossom = false;

float gTSFactorDistance = -100.0; // negative is good
float gTSFactorTotalResources = 10.0; // positive is good
float gTSFactorTimeToDone = 0.0; // positive is good

int gTotalNumber = 20;
int gFoodNumber = 0;
int gWoodNumber = 0;
int gGoldNumber = 0;
// This number is fetched by the engine to know what preset we're on.
int gPresetNumber = 0;

bool gForceUpdateBreakdowns = false;
int gDistributionTime = -1;
int gLastDistributionTime = -1;
bool gEnabled = false;
bool gAllowedToFarm = false;

// === MOD: aom_autorepair_test — POC globals ===
int gAutoRepairPOC_villagerQuery = -1;
int gAutoRepairPOC_buildingQuery = -1;
// === MOD: aom_autorepair_test — END ===

const int cDistributionDelayUI = 1;
const int cDistributionDelayUser = 60;

//==============================================================================
// disableFarmPlacement
//==============================================================================
void disableFarmPlacement()
{
   // This function is called from the UI, the UI doesn't know our context so we must set it explicitly.
   xsSetContextPlayer(cMyID);
   debugVPS("Disabling automatic Farm placement.");
   gAllowedToFarm = false;
   xsSetContextPlayer(-1);
}

//==============================================================================
// enableFarmPlacement
//==============================================================================
void enableFarmPlacement()
{
   // This function is called from the UI, the UI doesn't know our context so we must set it explicitly.
   xsSetContextPlayer(cMyID);
   debugVPS("Enabling automatic Farm placement.");
   gAllowedToFarm = true;
   xsSetContextPlayer(-1);
}

//==============================================================================
// disableAutoScouting
//==============================================================================
void disableAutoScouting(int unitID = -1)
{
   // This function is called from the UI, the UI doesn't know our context so we must set it explicitly.
   xsSetContextPlayer(cMyID);
   debugVPS("Disabling automatic scouting for unit: " + unitID + ".");
   int planID = kbUnitGetPlanID(unitID);
   if (planID != -1)
   {
      aiTaskStopUnit(unitID);
      aiPlanDestroy(planID);
   }
   xsSetContextPlayer(-1);
}

//==============================================================================
// enableAutoScouting
//==============================================================================
void enableAutoScouting(int unitID = -1)
{
   // This function is called from the UI, the UI doesn't know our context so we must set it explicitly.
   xsSetContextPlayer(cMyID);
   debugVPS("Enabling automatic scouting for unit: " + unitID + ".");
   int planID = aiPlanCreate("Autoscout with unit: " + unitID, cPlanExplore);
   aiPlanAddUnitType(planID, cUnitTypeUnit, 1,1,1);
   aiPlanAddUnit(planID, unitID);
   if (kbUnitIsType(unitID, cUnitTypeAbstractOracle) == true)
   {
      aiPlanSetVariableBool(planID, cExplorePlanDoLoops, 0, false);
      // Stand still if less or equal than 20% of our surrounding tiles are explored.
      aiPlanSetVariableFloat(planID, cExplorePlanStopLOSPercentage, 0, 0.2);
   }
   aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true);
   aiPlanSetFlag(planID, cPlanFlagRequiresAllNeedUnits, true);
   xsSetContextPlayer(-1);
}

//==============================================================================
// cleanupLingeringExplorePlans
// If a unit dies the explore plan doesn't automatically die with it.
//==============================================================================
rule cleanupLingeringExplorePlans
minInterval 15
active
{
   int[] scoutPlans = aiPlanGetIDsByType(cPlanExplore);
   for (int i = 0; i < scoutPlans.size(); i++)
   {
      if (aiPlanGetNumberUnits(scoutPlans[i]) == 0)
      {
         debugVPS("Destroying " + aiPlanGetName(scoutPlans[i]) + " because it has no units in it and can't get more either.");
         aiPlanDestroy(scoutPlans[i]);
      }
   }
}

//==============================================================================
// isFarmPlacementEnabled
// This function is called by the UI to know the state of the Farm toggle button.
//==============================================================================
bool isFarmPlacementEnabled() 
{
   return gAllowedToFarm;
}

//==============================================================================
// Distribution getters
//==============================================================================
int getFoodNumber() { return gFoodNumber; }
int getWoodNumber() { return gWoodNumber; }
int getGoldNumber() { return gGoldNumber; }

//==============================================================================
// buildingGetNumberAliveAndPlanned
//==============================================================================
int buildingGetNumberAliveAndPlanned(int puid = -1, bool planDoneWhenFoundationPlaced = false)
{
   int state = cUnitStateAlive;
   if (planDoneWhenFoundationPlaced == true)
   {
      state = cUnitStateABQ;
   }
   int num = kbUnitCount(puid, cMyID, state);
   num += aiPlanGetNumberByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, puid);
   return num;
}

//==============================================================================
// thePeachBlossomSpringMonitor
//==============================================================================
void thePeachBlossomSpringMonitor(bool forceAllForbidden = false)
{
   static int queryID = -1;
   if (queryID == -1)
   {
      queryID = kbUnitQueryCreate("thePeachBlossomSpringMonitor");
      kbUnitQuerySetPlayerID(queryID, cMyID);
      kbUnitQuerySetUnitType(queryID, cUnitTypeThePeachBlossomSpring);
      kbUnitQuerySetState(queryID, cUnitStateAlive);
   }
   kbUnitQueryResetResults(queryID);
   int numResults = kbUnitQueryExecute(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int unitID = kbUnitQueryGetResult(queryID, i);
      int kbResourceID = kbUnitGetKBResourceID(unitID);
      if (kbResourceGetIsIDValid(kbResourceID) == false)
      {
         continue;
      }

      if (kbUnitGetResourceAmount(unitID, cResourceFood) > 0.0)
      {
         gHaveFoodBlossom = true;
         if (gNeedToUseFoodBlossom == true && forceAllForbidden == false)
         {
            kbResourceSetFlag(kbResourceID, cResourceFlagForbidden, false);
         }
         else
         {
            kbResourceSetFlag(kbResourceID, cResourceFlagForbidden, true);
         }
      }
      else if (kbUnitGetResourceAmount(unitID, cResourceWood) > 0.0)
      {
         gHaveWoodBlossom = true;
         if (gNeedToUseWoodBlossom == true && forceAllForbidden == false)
         {
            kbResourceSetFlag(kbResourceID, cResourceFlagForbidden, false);
         }
         else
         {
            kbResourceSetFlag(kbResourceID, cResourceFlagForbidden, true);
         }
      }
      else // Must contain gold then.
      {
         gHaveGoldBlossom = true;
         if (gNeedToUseGoldBlossom == true && forceAllForbidden == false)
         {
            kbResourceSetFlag(kbResourceID, cResourceFlagForbidden, false);
         }
         else
         {
            kbResourceSetFlag(kbResourceID, cResourceFlagForbidden, true);
         }
      }
   }
   if (forceAllForbidden == true)
   {
      debugResourceDistribution("Resetting all our Peach Blossom Spring usage, all of them are forbidden again.");
      gNeedToUseFoodBlossom = false;
      gNeedToUseWoodBlossom = false;
      gNeedToUseGoldBlossom = false;
   }
}

//==============================================================================
// updateBreakdown
//==============================================================================
rule updateBreakdown
minInterval 9
inactive
{
   debugResourceDistribution("--- Running Rule updateBreakdown. ---");
   thePeachBlossomSpringMonitor(false);

   gRBDSystem.resourceBreakdownUpdateGatherPlanPriorities(gAllowedToFarm);
   gRBDSystem.scanForBases();

   int reserved = aiPlanGetNumberUnits(gReservePlan, cUnitTypeAbstractVillager);
   debugResourceDistribution("Reserved num gatherers: " + reserved + ".");
   int gathererCount = kbUnitCount(cUnitTypeAbstractVillager, cMyID, cUnitStateAlive) - reserved;
   // Omega hack to prevent 0 gatherers being assigned when only 1 lives and we have split priorities.
   if (gathererCount == 1)
   {
      gathererCount++;
   }
   debugResourceDistribution("gathererCount: " + gathererCount + ".");
   int plannedFoodGathererCount = xsFloatToInt(round(aiGetResourcePercentage(cResourceFood) * gathererCount));
   debugResourceDistribution("Planned Food gatherers: " + plannedFoodGathererCount + ".");
   debugResourceDistribution("Planned Wood gatherers: " + xsFloatToInt(round(aiGetResourcePercentage(cResourceWood) * gathererCount)) + ".");
   debugResourceDistribution("Planned Gold gatherers: " + xsFloatToInt(round(aiGetResourcePercentage(cResourceGold) * gathererCount)) + ".");
   int previousUnassigned = 0;
   int totalAssigned = 0;
   int unassigned = gRBDSystem.resourceBreakdownUpdateExistingFood(gathererCount, false, totalAssigned);
   unassigned = gRBDSystem.resourceBreakdownUpdateFood(gAllowedToFarm, -1, unassigned, gathererCount, totalAssigned, true);
   if (unassigned > 0)
   {
      bool waitWithNotification = false;
      if (gHaveFoodBlossom == true && gNeedToUseFoodBlossom == false)
      {
         // Don't error yet, first allow the food blossom to be valid.
         waitWithNotification = true;
         debugResourceDistribution("We need to use our Food Blossom Spring now.");
         gNeedToUseFoodBlossom = true;
      }
      if (gSentFoodNotification == false && waitWithNotification == false)
      {
         if (gAllowedToFarm == false)
         {
            gSentFoodNotification = true;
            aiSendNotificationFoundNotEnoughFoodGatheringSpots();
         }
         else
         {
            // Dont immediately error if we are allowed to build Farms. Yes we have unassigned but we place the Farms in batches.
            // Only error if we in total want more Farms than we can support.
            static int farmPUID = -1;
            if (farmPUID == -1)
            {
               farmPUID = kbSharedFunctionUnitGetByIndex(cSharedUnitFunctionFarm, 0);
            }
            int numFarms = buildingGetNumberAliveAndPlanned(farmPUID, true);
            // If we already have our max amount of Farms we have an issue.
            if (numFarms >= gRBDSystem.mNumTCBases * gRBDSystem.mMaxFarmsPerBase)
            {
               gSentFoodNotification = true;
               aiSendNotificationFoundNotEnoughFoodFarmGatheringSpots();
            }
         }
      }
      previousUnassigned = unassigned;
   }

   unassigned = gRBDSystem.resourceBreakdownUpdateGold(gathererCount, unassigned);
   // If we now have more unassigned than before we couldn't assign all wanted gold gatherers.
   if (previousUnassigned < unassigned)
   {
      bool waitWithNotification = false;
      if (gHaveGoldBlossom == true && gNeedToUseGoldBlossom == false)
      {
         // Don't error yet, first allow the Gold blossom to be valid.
         waitWithNotification = true;
         debugResourceDistribution("We need to use our Gold Blossom Spring now.");
         gNeedToUseGoldBlossom = true;
      }
      if (gSentGoldNotification == false && waitWithNotification == false)
      {
         gSentGoldNotification = true;
         aiSendNotificationFoundNotEnoughGoldGatheringSpots();
      }
   }
   previousUnassigned = unassigned;

   unassigned = gRBDSystem.resourceBreakdownUpdateWood(gathererCount, unassigned);
   // If we now have more unassigned than before we couldn't assign all wanted wood gatherers.
   if (previousUnassigned < unassigned)
   {
      bool waitWithNotification = false;
      if (gHaveWoodBlossom == true && gNeedToUseWoodBlossom == false)
      {
         // Don't error yet, first allow the Wood blossom to be valid.
         waitWithNotification = true;
         debugResourceDistribution("We need to use our Gold Blossom Spring now.");
         gNeedToUseWoodBlossom = true;
      }
      if (gSentWoodNotification == false && waitWithNotification == false)
      {
         gSentWoodNotification = true;
         aiSendNotificationFoundNotEnoughWoodGatheringSpots();
      }
   }
   previousUnassigned = unassigned;

   // VPS can't set favor yet but it's here anyway.
   if (cMyCulture == cCultureGreek)
   {
      gRBDSystem.resourceBreakdownUpdateFavor(gathererCount, unassigned);
   }
}

//==============================================================================
// updateVillagerDistributionFromNumbers
//==============================================================================
void updateVillagerDistributionFromNumbers()
{
   float total = gTotalNumber;
   float food = gFoodNumber;
   float wood = gWoodNumber;
   float gold = gGoldNumber;
   aiSetResourcePercentage(cResourceFood, food / total);
   aiSetResourcePercentage(cResourceWood, wood / total);
   aiSetResourcePercentage(cResourceGold, gold / total);
   aiNormalizeResourcePercentages();
}

//==============================================================================
// onAutoPlanCreate handler
//==============================================================================
void onAutoPlanCreate(int planID = -1)
{
   aiPlanSetVariableBool(planID, cGatherPlanAutoBuildDropsite, 0, cAllowDropsiteConstruction);
   
   if (cMyCulture == cCultureNorse &&
       aiPlanGetVariableInt(planID, cGatherPlanResourceSubType, 0) != cAIResourceSubTypeFarm)
   {
      aiPlanAddUnitType(planID, cUnitTypeOxCart, 1, 1, 1);
   }
}

//==============================================================================
// applyDistribution
//==============================================================================
rule applyDistribution
inactive
minInterval 1
maxInterval 2
{
   debugVPS("--- Running Rule applyDistribution. ---");
   if (gDistributionTime == -1)
   {
      if (gForceUpdateBreakdowns == true)
      {
         updateVillagerDistributionFromNumbers();
         updateBreakdown();
         // Assignment timers are all reset when we're in a force.
         gForceUpdateBreakdowns = false;
      }
      return;
   }
   if (xsGetTime() >= gDistributionTime)
   {
      debugVPS("Applying new distribution, Food: " + gFoodNumber + ", Wood: " + gWoodNumber + ", Gold: " + gGoldNumber + ".");
      updateVillagerDistributionFromNumbers();
      updateBreakdown();
      aiSetNextGathererDistributionTime(0);
      aiSetFullUnitAssignmentTime(0);
      gLastDistributionTime = xsGetTime();
      gDistributionTime = -1;
   }
}

//==============================================================================
// checkReservePlan
//==============================================================================
const int cReserveTypeNone = 0;
const int cReserveTypeIdle = cReserveTypeNone + 1;
const int cReserveTypeGathering = cReserveTypeIdle + 1;

int checkReservePlan(bool reclaimGatheringVillagers = false)
{
   debugVPS("Calling checkReservePlan with reclaimGatheringVillagers on: " + xsBoolToString(reclaimGatheringVillagers) + ".");
   int foundType = cReserveTypeNone;
   int[] units = aiPlanGetUnits(gReservePlan);
   if (units.size() == 0)
   {
      return foundType;
   }
   for (int i = 0; i < units.size(); i++)
   {
      int unitID = units[i];
      int protoUnitID = kbUnitGetProtoUnitID(unitID);
      // We handle multiple kinds of units in this reserve plan but the reserve types only apply to Villagers.
      bool isVillager = kbProtoUnitIsType(protoUnitID, cUnitTypeAbstractVillager);
      int actionType = kbUnitGetActionType(unitID);
      switch (actionType)
      {
         case cActionTypeIdle:
         {
            if (kbUnitGetIdleTime(unitID) > 2000)
            {
               debugVPS("Found an idle " + kbProtoUnitGetName(protoUnitID) + "(" + unitID + ") to reclaim.");
               aiPlanRemoveUnit(gReservePlan, unitID);
               if (isVillager == true && foundType < cReserveTypeGathering)
               {
                  foundType = cReserveTypeIdle;
               }
            }
            break;
         }
         case cActionTypeGather:
         case cActionTypeHunting:
         {
            foundType = cReserveTypeGathering;
            //int resourceID = kbResourceGetIDByUnitID(kbUnitGetTargetUnitID(unitID));
            //if (resourceID == -1)
            //{
            //   break;
            //}

            if (reclaimGatheringVillagers == false || isVillager == false)
            {
               break;
            }
            // Don't remove villagers praying at the temple
            int targetID = kbUnitGetTargetUnitID(unitID);
            if (kbUnitIsType(targetID, cUnitTypeAbstractTemple) == true)
            {
               break;
            }

            // Don't remove Fishing Ships from the reserve plan if they're gathering.
            // Because leaving the Fishing Ship in the reserve plan versus the fish plan actually makes no difference at all
            // in terms of it gathering. So we only add Fishing Ships to the fish plan when they're idle.
            aiPlanRemoveUnit(gReservePlan, unitID);

            // We must instantly add to the existing Farm because otherwise these Farmers will walk away if there is another
            // food plan ongoing. Because the resourceBreakdownUpdateExistingFood will claim them before they can be assigned
            // back to their Farms.
            if (kbUnitIsType(targetID, cUnitTypeAbstractFarm) == true)
            {
               // TMNOTE: (pbaulch) Don't try to farm what you can't reach
               vector farmPosition = kbUnitGetPosition(targetID);
               vector farmerPosition = kbUnitGetPosition(unitID);
               if (kbCanPath(farmerPosition, farmPosition, protoUnitID, 1.0, targetID) == true)
               {
                  debugVPS("checkReservePlan : Villager " + unitID + " at [" + farmerPosition + "] can path to farm " + targetID + " at [" + farmPosition + "]");
                  int kbResourceID = kbUnitGetKBResourceID(targetID);
                  int farmGatherPlanID = aiPlanGetIDByTypeAndVariableIntValue(cPlanGather, cGatherPlanKBResourceID, kbResourceID, 0);
                  if (farmGatherPlanID != -1)
                  {
                     // Temp logging for a mysterious warning.
                     debugVPS("Max: " + aiPlanGetNumberMaxUnits(farmGatherPlanID) + ", current: " +
                        aiPlanGetNumberUnits(farmGatherPlanID, cUnitTypeAbstractVillager) + ".");
                     // Increment the numbers since they will be capped to the previous max when we still occupied this Farm.
                     aiPlanAddUnitType(farmGatherPlanID, cUnitTypeAbstractVillager, 1, 1, 1, true);
                     aiPlanAddUnit(farmGatherPlanID, unitID);
                     gRBDSystem.mLastAssignedNumberFarmers++;
                  }
               }
            }
         }
      }
   }
   return foundType;
}

//==============================================================================
// updateReserved
//==============================================================================
const int cReserveStateNormal = 0;
const int cReserveStateUserInput = cReserveStateNormal + 1;
int reserveState = cReserveStateNormal;

rule updateReserved
inactive
minInterval 1
maxInterval 10
{
   xsSetContextPlayer(cMyID);
   debugVPS("--- Running Rule updateReserved. ---");
   int reserveType = checkReservePlan(false);
   // We only want to force a change when our vills are gathering
   if (reserveType == cReserveTypeGathering && reserveState == cReserveStateUserInput)
   {
      // Recalculate distribution numbers from our current distribution
      int[] resources = new int(3, 0);
      int[] gatherPlans = aiPlanGetIDsByType(cPlanGather);
      for (int i = 0; i < gatherPlans.size(); i++)
      {
         int planID = gatherPlans[i];
         int resourceType = aiPlanGetVariableInt(planID, cGatherPlanResourceType, 0);
         if (resourceType < cResourceFavor && resourceType != -1)
         {
            resources[resourceType] = resources[resourceType] + aiPlanGetNumberUnits(planID, cUnitTypeAbstractVillager, true); 
         }
      }
      if (gFoodNumber > 0)
      {
         resources[cResourceFood] = max(resources[cResourceFood], 1);
      }
      if (gWoodNumber > 0)
      {
         resources[cResourceWood] = max(resources[cResourceWood], 1);
      }
      if (gGoldNumber > 0)
      {
         resources[cResourceGold] = max(resources[cResourceGold], 1);
      }
      float total = xsIntToFloat(resources[cResourceFood] + resources[cResourceWood] + resources[cResourceGold]);
      for (int i = 0; i < cResourceFavor; i++)
      {
         resources[i] = xsFloatToInt(round((xsIntToFloat(resources[i]) / total) * 19.99));// So we never somehow overflow
      }
      debugVPS("New Food:" + resources[cResourceFood] + " new wood: " + resources[cResourceWood] + " new gold: " + resources[cResourceGold]);
      setDistributionNumbers(resources[cResourceFood], resources[cResourceWood], resources[cResourceGold]);
      updateVillagerDistributionFromNumbers();
      xsRuleIgnoreIntervalOnce("updateBreakdown"); 
      aiSetFullUnitAssignmentTime(xsGetTimeMS() + cDistributionDelayUser * 1000);
      aiSetNextGathererDistributionTime(xsGetTimeMS() + cDistributionDelayUser * 1000);
      gDistributionTime = xsGetTime() + cDistributionDelayUser;
      debugVPS("Overriding distribution time to have a " + cDistributionDelayUI + " ms delay.");
   }
   reserveState = cReserveStateNormal;
}

//==============================================================================
// checkGatherRadius
// This rule is always on so that in the event of VPS being turned on later we have a proper base range.
//==============================================================================
const float maxRange = cVPSMaxBaseRangeTC;
const float minRange = 30.0;

rule checkGatherRadius
active
minInterval 30
maxInterval 60
{
   debugResourceDistribution("--- Running Rule checkGatherRadius. ---");
   int numberBases = kbBaseGetNumber(cMyID);
   bool anyBaseExpanding = false;

   // Go through all our bases
   for (int i = 0; i < numberBases; i++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, i);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         continue;
      }
      vector basePosition = kbBaseGetLocation(cMyID, baseID);
      float range = kbBaseGetDistance(cMyID, baseID);

      bool wasExpanding = kbBaseIsFlagSet(cMyID, baseID, cBaseFlagExpanding);
      bool isExpanding = false;

      if (wasExpanding == false)
      {
         debugResourceDistribution("Checking for resources in range.");
         // We need to check if we're low on resources
         float foodEasy = kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeEasy, range);
         float foodHunt = kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeHunt, range);
         float foodHerd = kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeHerdable, range);

         float numRes = foodEasy + foodHunt + foodHerd;

         // Hack in case we have berries so we won't move off them
         if (foodEasy >= 10.0)
         {
            numRes = numRes + 500.0;
         }

         debugResourceDistribution("  food: " + numRes + " easy: " + foodEasy + " hunt: " + foodHunt + " herd: " + foodHerd);

         debugResourceDistribution("  Food res left in main base: " + numRes);
         if (numRes < 500 && range < maxRange)
         {
            float newRange = range + 10.0;
            if (newRange > maxRange)
            {
               newRange = maxRange;
            }
            debugResourceDistribution("    We are low on resources, expanding gather range to: " + newRange);
            kbBaseSetDistance(cMyID, baseID, newRange);
            isExpanding = true;
         }
         else
         {
            // Now we need to check if we shrink
            // First we need to get all our gather plans
            float maxRangeSq = 0.0;
            float secondMaxRangeSq = 0.0;
            int numGatherPlans = aiPlanGetNumberByType(cPlanGather);
            for(int iPlan = 0; iPlan < numGatherPlans; iPlan++)
            {
               int planID = aiPlanGetIDByTypeIndex(cPlanGather, iPlan);
               if(aiPlanGetBaseID(planID) != baseID)
               {
                  continue;
               }
               float distanceSq = xsVectorDistanceXZSqr(aiPlanGetLocation(planID), basePosition); 
               if(distanceSq >= maxRangeSq)
               {
                  maxRangeSq = distanceSq;
                  secondMaxRangeSq = maxRangeSq;
               }
            }
   
            // Only shrink at most till our second plan so we blow up just one instead of many in rare cases
            // Also add 10 range buffer zone before we actually shrink
            float shrunkRange = range - 20.0; 
            if(secondMaxRangeSq < shrunkRange * shrunkRange)
            {
               foodEasy = kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeEasy, shrunkRange);
               foodHunt = kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeHunt, shrunkRange);
               foodHerd = kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeHerdable, shrunkRange);
   
               numRes = foodEasy + foodHunt + foodHerd;
   
               debugResourceDistribution("  shrunk range food: " + numRes + " easy: " + foodEasy + " hunt: " + foodHunt + " herd: " + foodHerd);
   
               if (numRes > 500 && range > minRange)
               {
                  float newRange = range - 10.0;
                  if (newRange < minRange)
                  {
                     newRange = minRange;
                  }
                  debugResourceDistribution("    We have found resources closer nearby, reducing gather range to: " + newRange);
                  kbBaseSetDistance(cMyID, baseID, newRange);
               }
            }
         }
      }
      else
      {
         debugResourceDistribution("Checking for resources in expanding range");

         float foodEasy = kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeEasy, range);
         float foodHunt = kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeHunt, range);
         float foodHerd = kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeHerdable, range);

         float numRes = foodEasy + foodHunt + foodHerd;

         debugResourceDistribution("  food: " + numRes + " easy: " + foodEasy + " hunt: " + foodHunt + " herd: " + foodHerd);

         if (numRes > 500)
         {
            debugResourceDistribution("  Found enough resources");
         }
         else if (range >= maxRange)
         {
            debugResourceDistribution("  Maxed out our range");
         }
         else
         {
            float newRange = range + 10.0;
            if (newRange > maxRange)
            {
               newRange = maxRange;
            }
            debugResourceDistribution("  We are still low on resources, expanding gather range to: " + newRange);
            kbBaseSetDistance(cMyID, baseID, newRange);
            isExpanding = true;
         }
      }

      if (isExpanding != wasExpanding)
      {
         kbBaseSetFlag(cMyID, baseID, cBaseFlagExpanding, isExpanding);
      }

      if (isExpanding)
      {
         anyBaseExpanding = true;

         // Don't process any more bases on this update
         // The expanding base may have swallowed some other bases which would invalidate our iteration over the bases array
         break;
      }
   }

   if (anyBaseExpanding)
   {
      xsSetRuleMinInterval("checkGatherRadius", 0);
      xsSetRuleMaxInterval("checkGatherRadius", 0);
   }
   else
   {
      // Give it some time
      xsSetRuleMinInterval("checkGatherRadius", 30);
      xsSetRuleMaxInterval("checkGatherRadius", 60);
   }
}

//==============================================================================
// setUpFishPlans
//==============================================================================
void setUpFishPlans()
{
   debugVPS("Setting up Fish plans.");
   int numAreaGroups = kbAreaGroupGetNumber();
   for (int i = 0; i < numAreaGroups; i++)
   {
      if (kbAreaGroupGetType(i) != cAreaGroupTypeWater)
      {
         continue;
      }
      xsSetContextPlayer(0);
      int queryID = useSimpleNatureUnitQuery(cUnitTypeFishResource);
      kbUnitQuerySetAreaGroupID(queryID, i);
      int numResults = kbUnitQueryExecute(queryID);
      xsSetContextPlayer(cMyID);
      debugVPS("Found " + numResults + " Fish for area group " + i +".");
      if (numResults == 0)
      {
         continue;
      }
      int planID = aiPlanCreate("Gather Fish areaGroup " + i, cPlanFish);
      debugVPS("Created Fishing plan: " + aiPlanGetName(planID) + ".");
      aiPlanAddUnitType(planID, cUnitTypeAbstractFishingShip, 0, 0, 200);
      aiPlanSetVariableInt(planID, cFishPlanResourceType, 0, cResourceFood);
      aiPlanSetVariableInt(planID, cFishPlanResourceSubType, 0, cAIResourceSubTypeFish);
      aiPlanSetVariableInt(planID, cFishPlanWaterGroupID, 0, i);
   }
}

//==============================================================================
// deleteFishPlans
//==============================================================================
void deleteFishPlans()
{
   debugVPS("Deleting Fish plans.");
   int[] plans = aiPlanGetIDsByType(cPlanFish);
   for (int i = 0; i < plans.size(); i++)
   {
      aiPlanDestroy(plans[i]);
   }
}

//==============================================================================
// setPresetNumber
//==============================================================================
void setPresetNumber(int preset = 0) 
{
   if (preset < 0) 
   {
      aiEchoWarning("WARNING: Received an invalid VP preset number.");
      preset = 0;
   }
   else 
   {
      gPresetNumber = preset;
   }
}

//==============================================================================
// getPresetNumber
//==============================================================================
int getPresetNumber() 
{
   if (gEnabled == true) 
   {
      return gPresetNumber;
   }
   return 0; // None,
}

//==============================================================================
// userControlledVillagers
//==============================================================================
void userControlledVillagers(int unused = -1)
{
   debugVPS("User controlled villager(s)");
   if (cAllowManualDistribution)
   {
      xsRuleIgnoreIntervalOnce("updateReserved");
      reserveState = cReserveStateUserInput;
   }
   // We need to offset this by last update time
   debugVPS("Last Update time: " + gLastDistributionTime);
   
   int newTimeMS = min(gLastDistributionTime * 1000 + cDistributionDelayUser * 1000, xsGetTimeMS() + cDistributionDelayUser * 1000); 
   debugVPS("New Update time: " + newTimeMS / 1000);
   aiSetFullUnitAssignmentTime(newTimeMS);
   //aiSetUnassignedUnitAssignmentTime(newTimeMS);
   //aiSetNextGathererDistributionTime(newTimeMS);
   gDistributionTime = newTimeMS / 1000;
}

//==============================================================================
// disableVillagerAssist
// This turns off all automatic behavior and destroys all the plans.
//==============================================================================
void disableVillagerAssist()
{
   // This function is called from the UI, the UI doesn't know our context so we must set it explicitly.
   xsSetContextPlayer(cMyID);
   gEnabled = false;
   debugVPS("Disabling Villager Assist.");
   aiSetNextGathererDistributionTime(-1);
   aiSetFullUnitAssignmentTime(-1);
   aiSetUnassignedUnitAssignmentTime(-1);
   // This removes all the gather plans.
   gRBDSystem.applyResourceFlags(false, false, false, false);
   xsDisableRule("updateBreakdown");
   xsDisableRule("applyDistribution");
   xsDisableRule("updateReserved");
   deleteFishPlans();
   gDistributionTime = -1;
   xsSetContextPlayer(-1);
}

//==============================================================================
// enableVillagerAssist
// This turns on all automatic behavior and creates the reserve plan (gather plans are auto created).
//==============================================================================
void enableVillagerAssist()
{
   // Remove idle Villagers from the reserve plan so that we can instantly task them.
   int[] units = aiPlanGetUnits(gReservePlan);
   for (int i = 0; i < units.size(); i++)
   {
      int unitID = units[i];
      int actionType = kbUnitGetActionType(unitID); 
      if (actionType == cActionTypeIdle)
      {
         aiPlanRemoveUnit(gReservePlan, unitID);
      }
   }
   gEnabled = true;
   debugVPS("Enabling Villager Assist.");
   xsEnableRule("updateBreakdown");
   xsEnableRule("applyDistribution");
   gDistributionTime = -1;
   gForceUpdateBreakdowns = true;
   // Instantly run it so our activation instantly takes effect.
   applyDistribution();
   xsEnableRule("updateReserved");
   setUpFishPlans();
   aiSetNextGathererDistributionTime(0);
   aiSetFullUnitAssignmentTime(0);
}

//==============================================================================
// setDistributionNumbers
// This function is called by the UI to set our new distribution.
//==============================================================================
void setDistributionNumbers(int food = 0, int wood = 0, int gold = 0)
{
   // Check invalid inputs.
   int total = food + wood + gold;
   if (food < 0 || wood < 0 || gold < 0 || total > 20 || total == 0)
   {
      aiEchoWarning("DETECTED AN INVALID INPUT FROM THE UI FOR THE DISTRIBUTION.");
      return;
   }
   // This function is called from the UI, the UI doesn't know our context so we must set it explicitly.
   xsSetContextPlayer(cMyID);
   debugResourceDistribution("*** Running setDistributionNumbers. ***");

   gRBDSystem.applyResourceFlags(true, true, true, true);
   gTotalNumber = total;
   gFoodNumber = food;
   gWoodNumber = wood;
   gGoldNumber = gold;
   debugResourceDistribution("New distribution (point system): Food: " + food + ", Wood: " + wood + ", Gold: " + gold + ".");

   // We can send notifications again for this preset.
   gSentFoodNotification = false;
   gSentWoodNotification = false;
   gSentGoldNotification = false;

   // Remove all Farm plans. If we went down on how many food gatherers we want we don't want to randomly place Farms.
   // If we go up we just recreate the plans with a little delay.
   static int farmPUID = -1;
   if (farmPUID == -1)
   {
      farmPUID = kbSharedFunctionUnitGetByIndex(cSharedUnitFunctionFarm, 0);
   }
   int[] plans = aiPlanGetIDsByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, farmPUID);
   for (int i = 0; i < plans.size(); i++)
   {
      aiPlanDestroy(plans[i]);
   }

   // Reclaim all Villagers that were gathering while in the reserve plan.
   debugResourceDistribution("Reclaiming all reserved Gatherers that are actually gathering.");
   checkReservePlan(true);

   if (gEnabled == true)
   {
      gDistributionTime = xsGetTime() + cDistributionDelayUI;
      debugResourceDistribution("Villager Assist was already on, just changing the distribution after " + cDistributionDelayUI + " second delay.");
   }
   else
   {
      debugResourceDistribution("Villager Assist was off, enabling it now and setting the distribution instantly.");
      enableVillagerAssist();
   }
   // Reset our Peach Blossom Spring usage.
   thePeachBlossomSpringMonitor(true);
   xsSetContextPlayer(-1);
}

//==============================================================================
// === MOD: aom_autorepair_test — POC RULE BEGIN ===
// Tests whether aiTaskWorkUnit on a damaged friendly building results in a
// real Repair work command (animation plays, HP recovers, wood is deducted).
// Active by default for POC; production version will gate on a player toggle.
//==============================================================================
void autoRepairPOC_setupQueries()
{
   if (gAutoRepairPOC_villagerQuery == -1)
   {
      gAutoRepairPOC_villagerQuery = kbUnitQueryCreate("autoRepairPOC_villagers");
      kbUnitQuerySetPlayerID(gAutoRepairPOC_villagerQuery, cMyID, false);
      kbUnitQuerySetUnitType(gAutoRepairPOC_villagerQuery, cUnitTypeAbstractVillager);
      kbUnitQuerySetState(gAutoRepairPOC_villagerQuery, cUnitStateAlive);
      kbUnitQuerySetActionType(gAutoRepairPOC_villagerQuery, cActionTypeIdle);
   }
   if (gAutoRepairPOC_buildingQuery == -1)
   {
      gAutoRepairPOC_buildingQuery = kbUnitQueryCreate("autoRepairPOC_buildings");
      kbUnitQuerySetPlayerID(gAutoRepairPOC_buildingQuery, cMyID, false);
      kbUnitQuerySetUnitType(gAutoRepairPOC_buildingQuery, cUnitTypeBuilding);
      kbUnitQuerySetState(gAutoRepairPOC_buildingQuery, cUnitStateAlive);
      kbUnitQuerySetMaximumDistance(gAutoRepairPOC_buildingQuery, 20.0);
      kbUnitQuerySetAscendingSort(gAutoRepairPOC_buildingQuery, true);
   }
}

rule autoRepairPOC
minInterval 3
active
{
   xsSetContextPlayer(cMyID);
   autoRepairPOC_setupQueries();

   kbUnitQueryResetResults(gAutoRepairPOC_villagerQuery);
   int villagerCount = kbUnitQueryExecute(gAutoRepairPOC_villagerQuery);

   if (villagerCount <= 0)
   {
      xsSetContextPlayer(-1);
      return;
   }

   for (int i = 0; i < villagerCount; i++)
   {
      int villagerID = kbUnitQueryGetResult(gAutoRepairPOC_villagerQuery, i);
      if (villagerID < 0) { continue; }
      // Skip very-briefly idle units to avoid interrupting transitions.
      if (kbUnitGetIdleTime(villagerID) < 2000) { continue; }

      vector pos = kbUnitGetPosition(villagerID);

      kbUnitQuerySetPosition(gAutoRepairPOC_buildingQuery, pos);
      kbUnitQueryResetResults(gAutoRepairPOC_buildingQuery);
      int buildingCount = kbUnitQueryExecute(gAutoRepairPOC_buildingQuery);

      if (buildingCount <= 0) { continue; }

      for (int j = 0; j < buildingCount; j++)
      {
         int buildingID = kbUnitQueryGetResult(gAutoRepairPOC_buildingQuery, j);
         if (buildingID < 0) { continue; }

         // Damage detection via kbUnitGetPower (false=ignore health, true=apply health).
         // Damaged when curPower < maxPower; epsilon avoids false positives from float noise.
         float maxPower = kbUnitGetPower(buildingID, false);
         float curPower = kbUnitGetPower(buildingID, true);

         if (maxPower > curPower + 0.001)
         {
            aiEcho("autoRepairPOC: tasking villager " + villagerID + " -> building " + buildingID + " (power " + curPower + "/" + maxPower + ")");
            bool success = aiTaskWorkUnit(villagerID, buildingID, false);
            aiEcho("autoRepairPOC: aiTaskWorkUnit returned " + success);
            xsSetContextPlayer(-1);
            return;
         }
      }
   }

   xsSetContextPlayer(-1);
}
//==============================================================================
// === MOD: aom_autorepair_test — POC RULE END ===
//==============================================================================

//==============================================================================
// main
//==============================================================================
void main()
{
   aiEcho("Villager Priority startup.");
   aiEcho("Game type is " + cGameTypeCurrent + ", 0 = Scenario, 1 = Save Game, 2 = Random Map, 3 = Campaign, 4 = Recorded Game.");
   aiEcho("Map name is " + cRandomMapName);

   setupDebugCategories();
   aiSetDeleteUnitsForbiddenPlans(false);

   aiSetHandler("onAutoPlanCreate", cXSAutoCreatePlanHandler);

   // Reserve plan to allow player control.
   gReservePlan = aiPlanCreate("Reserve", cPlanReserve);
   aiPlanSetPriority(gReservePlan, 100);
   aiPlanAddUnitType(gReservePlan, cUnitTypeAffectedByTownBell, 1000, 1000, 1000);
   aiPlanAddUnitType(gReservePlan, cUnitTypeAbstractFishingShip, 1000, 1000, 1000);
   aiPlanAddUnitType(gReservePlan, cUnitTypeTransport, 1000, 1000, 1000);
   if (cMyCulture == cCultureChinese)
   {
      aiPlanAddUnitType(gReservePlan, cUnitTypeAbstractKuafu, 1000, 1000, 1000);
   }
   aiPlanSetFlag(gReservePlan, cPlanFlagNoMoreUnits, true);

   // Set the default Resource Selector factor.
   kbSetResourceSelectorFactor(cTSFactorDistance, cResourceFood, gTSFactorDistance);
   kbSetResourceSelectorFactor(cTSFactorTotalResources, cResourceFood, gTSFactorTotalResources);
   kbSetResourceSelectorFactor(cTSFactorTimeToDone, cResourceFood, gTSFactorTimeToDone);
   kbSetResourceSelectorFactor(cTSFactorDanger, cResourceFood, 0.0);

   kbSetResourceSelectorFactor(cTSFactorDistance, cResourceWood, gTSFactorDistance);
   kbSetResourceSelectorFactor(cTSFactorTotalResources, cResourceWood, gTSFactorTotalResources);
   kbSetResourceSelectorFactor(cTSFactorTimeToDone, cResourceWood, gTSFactorTimeToDone);
   kbSetResourceSelectorFactor(cTSFactorDanger, cResourceWood, 0.0);

   kbSetResourceSelectorFactor(cTSFactorDistance, cResourceGold, gTSFactorDistance);
   kbSetResourceSelectorFactor(cTSFactorTotalResources, cResourceGold, 0);
   kbSetResourceSelectorFactor(cTSFactorTimeToDone, cResourceGold, gTSFactorTimeToDone);
   kbSetResourceSelectorFactor(cTSFactorDanger, cResourceGold, 0.0);

   aiNormalizeResourcePercentages();
   if (kbBaseGetMainID(cMyID) != -1)
   {
      kbBaseSetDistance(cMyID, kbBaseGetMainID(cMyID), 50.0);
   }
   // The system starts off by default.
   disableVillagerAssist();
}