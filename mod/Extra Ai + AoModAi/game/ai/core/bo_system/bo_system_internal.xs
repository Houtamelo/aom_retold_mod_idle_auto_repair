//==============================================================================
/* bo_system_internal.xs

   This file is intended for all the BO system handling, the back end.

*/
//==============================================================================

//==============================================================================
// Class BOSystem
//==============================================================================
class BOSystem
{
   BOStep[] buildOrderSteps = default;
   int currentStep = 0;
   int currentVillagerStep = 0;
   int[] mVillagerMaintainPlans = default;
   int nNumVillagerMaintainPlans = 0;
   int[] villagers = default;
   int numVillagers = 0;
   int nextUpdate = -1;
   int nextForecast = -1;
   int[] currentGatherPlans = default;
   int timeout = 0;
   int timeoutBegin = 0;
   bool done = false;
   int mainBaseID = -1;
   vector mainBasePosition = cInvalidVector;
   int secondBaseID = -1;
   int secondBaseTCID = -1;
   vector secondBasePosition = cInvalidVector;
};
extern BOSystem boSystem;

//==============================================================================
// internalBOAddOrder
//==============================================================================
void internalBOAddOrder(ref BOStep step)
{
   boSystem.buildOrderSteps.add(step);
}

//==============================================================================
// On create defaults.
//==============================================================================
void internalBOUnitDefaultCreate(int planID = -1)
{
}

void internalBOBuildDefaultCreate(int planID = -1)
{
   aiEchoWarning("Never use internalBOBuildDefaultCreate because then we assign no units to the build plan.");
}

void internalBOEmpowerDefaultCreate(int planID = -1)
{
   debugBOStep("Default empowering planID: " + planID + " with a Pharaoh.");
   aiPlanAddUnitType(planID, cUnitTypePharaoh, 1, 1, 1);
   int pharaohID = getUnit(cUnitTypePharaoh);
   if (pharaohID == -1)
   {
      aiEchoWarning("We have no Pharaoh to empower with.");
      internalBODoEndStep();
      return;
   }
   aiPlanAddUnit(planID, pharaohID);
}

void internalBOExploreDefaultCreate(int planID = -1)
{
}

//==============================================================================
// internalBOQueueUpVillager
//==============================================================================
int internalBOQueueUpVillager(int resourceType = -1, int villagerPUID = -1)
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      debugBOStep("Queuing " + kbProtoUnitGetName(villagerPUID) + ".");
   }
   else
   {
      debugBOStep("Queuing " + kbProtoUnitGetName(villagerPUID) + " to " + kbGetResourceName(resourceType) + ".");
      // The economic rally point here only works as long as we only have 1 plan per resource type.
      switch (resourceType)
      {
         case cResourceFood:
         {
            int kbResourceID = aiPlanGetVariableInt(boSystem.currentGatherPlans[cResourceFood], cGatherPlanKBResourceID, 0);
            if (kbResourceGetIsIDValid(kbResourceID) == true)
            {
               aiUnitSetRallyPointToPosition(getUnit(cUnitTypeAbstractTownCenter), kbResourceGetPosition(kbResourceID));
            }
            break;
         }
         case cResourceWood:
         {
            int kbResourceID = aiPlanGetVariableInt(boSystem.currentGatherPlans[cResourceWood], cGatherPlanKBResourceID, 0);
            if (kbResourceGetIsIDValid(kbResourceID) == true)
            {
               aiUnitSetRallyPointToPosition(getUnit(cUnitTypeAbstractTownCenter), kbResourceGetPosition(kbResourceID));
            }
            break;
         }
         case cResourceGold:
         {
            int kbResourceID = aiPlanGetVariableInt(boSystem.currentGatherPlans[cResourceGold], cGatherPlanKBResourceID, 0);
            if (kbResourceGetIsIDValid(kbResourceID) == true)
            {
               aiUnitSetRallyPointToPosition(getUnit(cUnitTypeAbstractTownCenter), kbResourceGetPosition(kbResourceID));
            }
            break;
         }
         case cResourceFavor:
         {
            int templeID = getUnit(cUnitTypeTemple);
            if (templeID >= 0)
            {
               aiUnitSetRallyPointToPosition(getUnit(cUnitTypeAbstractTownCenter), kbUnitGetPosition(templeID));
            }
            break;
         }
      }
   }
   
   // Find the maintain plan matching the unittype.
   int num = boSystem.nNumVillagerMaintainPlans;
   int[] maintainPlans = boSystem.mVillagerMaintainPlans;
   for (int i = 0; i < num; i++)
   {
      int planID = maintainPlans[i];
      int unittype = aiPlanGetVariableInt(planID, cTrainPlanUnitType, 0);
      if (unittype != villagerPUID)
      {
         continue;
      }
      // We found it now just add 1 to train if we didn't already
      int oldNum = aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0);
      if (oldNum <= boSystem.numVillagers)
      {
         debugBOStep("Adding unforecasted Villager to our maintain plan.");
         aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, kbUnitCount(villagerPUID, cMyID) + 1);
      }
      else
      {
         debugBOStep("We already forecasted this Villager.");
      }
      return planID;
   }

   // We didn't find the puid yet so we need to create a maintain plan
   int newPlan = createSimpleMaintainPlan(villagerPUID, kbUnitCount(villagerPUID, cMyID) + 1, gLandAreaGroupID, gEconomyCategoryID,
      70);
   aiPlanSetName(newPlan, newPlan + ": BOMaintain: " + kbProtoUnitGetName(villagerPUID));
   aiPlanSetVariableInt(newPlan, cTrainPlanMaxQueueSize, 0, 2);
   aiPlanSetEventHandler(newPlan, cTrainPlanEventUnitTrained, "internalBOVillagerTrained");

   int newNum = num + 1;
   int[] newArray = new int(newNum, -1);
   for (int iCopy = 0; iCopy < num; iCopy++)
   {
      newArray[iCopy] = maintainPlans[iCopy];
   }
   newArray[num] = newPlan;
   boSystem.mVillagerMaintainPlans = newArray;
   boSystem.nNumVillagerMaintainPlans = boSystem.nNumVillagerMaintainPlans + 1;
   return newPlan;
}

//==============================================================================
// internalBOEndStrategy
//==============================================================================
void internalBOEndStrategy()
{
   boSystem.done = true;
}

//==============================================================================
// isBuildOrderDone
//==============================================================================
bool isBuildOrderDone()
{
   return boSystem.done;
}

//==============================================================================
// internalGetSecondBaseID
//==============================================================================
int internalGetSecondBaseID()
{
   return boSystem.secondBaseID;
}

//==============================================================================
// updateBOSystem
//==============================================================================
void updateBOSystem()
{
   debugBO("updateBOSystem step: " + boSystem.currentStep);
   BOStep currentOrder = boSystem.buildOrderSteps[boSystem.currentStep];
   switch (currentOrder.type)
   {
      case cBOStepTypeVillager:
      {
         internalBODoVillagerStep(currentOrder);
         // Save what step this Villager belongs to so that the internalBOVillagerTrained handler knows what resource to assign to.
         boSystem.currentVillagerStep = boSystem.currentStep;
         // We must wait on the Villager to be trained to progress the BO, otherwise all the steps get messed up.
         // The callback that happens on completion of the Villager will set the nextUpdate correcty for us,
         // we must just prevent it here from updating too early!
         boSystem.nextUpdate = cMaxInt;
         // Just do this 1 second later.
         boSystem.nextForecast = xsGetTime() + 1;
         break;
      }

      case cBOStepTypeBuild:
      {
         internalBODoBuildStep(currentOrder);
         break;
      }

      case cBOStepTypeTech:
      {
         internalBODoTechStep(currentOrder);
         break;
      }

      case cBOStepTypeUnit:
      {
         internalBODoUnitStep(currentOrder);
         if (currentOrder.params[cBOStepUnitBlocking] == cBOStepBlocking)
         {
            // We must wait befor the unit is trained like explained for the Villager above too.
            boSystem.nextUpdate = cMaxInt;
            boSystem.nextForecast = cMaxInt;
         }
         else
         {
            // Just do this 1 second later.
            boSystem.nextForecast = xsGetTime() + 1;
         }
         break;
      }

      case cBOStepTypeTransaction:
      {
         int newResourceType = currentOrder.params[cBOStepTransactionNewResource];
         internalBODoTransactionStep(
            boSystem.villagers[currentOrder.params[cBOStepTransactionVillagerIndex]],
            newResourceType,
            boSystem.currentGatherPlans[newResourceType],
            currentOrder.onPlanCreate
         );
         break;
      }

      case cBOStepTypeAdvance:
      {
         internalBODoAdvanceStep(currentOrder);
         if (currentOrder.params[cBOStepAdvanceBlocking] == cBOStepBlocking)
         {
            boSystem.nextUpdate = cMaxInt;
         }
         boSystem.nextForecast = cMaxInt;
         break;
      }

      case cBOStepTypeEmpower:
      {
         internalBODoEmpowerStep(currentOrder);
         break;
      }

      case cBOStepTypeExplore:
      {
         internalBODOExploreStep(currentOrder);
         break;
      }

      case cBOStepTypeExecute:
      {
         currentOrder.onPlanCreate(-1);
         break;
      }

      case cBOStepTypeConditionalWait:
      {
         static int timeout = 0;
         if (internalBODoConditionalWait(currentOrder) == false)
         {
            // Return so we keep repeating the order until the condition is met
            boSystem.nextUpdate = xsGetTime() + 1;
            boSystem.nextForecast = cMaxInt;
            if (timeout > currentOrder.params[cBOStepConditionalWaitTimeout])
            {
               if (currentOrder.params[cBOStepConditionalWarning] == cWarning)
               {
                  debugBOStep("We didn't meet our condition within the timeout, failing BO!");
               }
               internalBODoEndStep();
               return;
            }
            timeout++;
            return;
         }
         timeout = 0;
         break;
      }

      case cBOStepTypeWait:
      {
         static bool doneWait = false;
         if (doneWait == false)
         {
            debugBOStep("Waiting for " + currentOrder.params[cBOStepWaitTime] + " seconds.");
            boSystem.nextUpdate = xsGetTime() + currentOrder.params[cBOStepWaitTime];
            doneWait = true;
            return;
         }
         doneWait = false;
         break;
      }

      case cBOStepTypeActivateRule:
      {
         xsEnableRule(currentOrder.onCompleteHandler);
         break;
      }

      case cBOStepTypeTimeoutIncrease:
      {
         int time = xsGetTime();
         debugBOStep("New timeout total time: " + currentOrder.params[cBOStepTimeoutTime] + " seconds, we need to complete " +
            "the next part of the BO before " + turnNumberIntoTimeDisplay(time + currentOrder.params[cBOStepTimeoutTime]) + ".");
         boSystem.timeout = currentOrder.params[cBOStepTimeoutTime];
         boSystem.timeoutBegin = time;
         break;
      }

      case cBOStepTypeEnd:
      {
         internalBODoEndStep();
         break;
      }
   }
   boSystem.currentStep++;
}

//==============================================================================
// forecastBOSystem
//==============================================================================
void forecastBOSystem()
{
   boSystem.nextForecast = cMaxInt;
   debugBO("forecastBOSystem step: " + boSystem.currentStep);
   debugBO("forecastBOSystem Villager step: " + boSystem.currentVillagerStep);
   int forecastStep = boSystem.currentStep;
   int size = boSystem.buildOrderSteps.size();
   while (forecastStep < size)
   {
      BOStep forecastOrder = boSystem.buildOrderSteps[forecastStep];
      switch (forecastOrder.type)
      {
         case cBOStepTypeVillager:
         {
            // Queue up the Villager.
            int[] params = forecastOrder.params;
            int villagerUnitType = params[cBOStepVillagerUnitType];
            int[] maintainPlans = boSystem.mVillagerMaintainPlans;
            bool found = false;
            for (int i = 0; i < boSystem.nNumVillagerMaintainPlans; i++)
            {
               int planID = maintainPlans[i];
               int unittype = aiPlanGetVariableInt(planID, cTrainPlanUnitType, 0);
               if (unittype != villagerUnitType)
               {
                  continue;
               }
               // We found it now just add 1 to train.
               found = true;
               int oldNum = aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0);
               aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, oldNum + 1);
               break;
            }
            if (found == false)
            {
               debugBO("forecastBOSystem we have a Villager forecast but don't have a maintain plan for that unitType, not forecasting.");
            }
            else
            {
               debugBO("ForecastBOSystem forecasted villager type: " + kbProtoUnitGetName(villagerUnitType));
            }
            return;
         }

         // If there is a blocking unit or any type of wait or an advance next in the queue we don't forecast.
         // Every advance here, regardless on if it's blocking or not,  causes us to skip the forecast because otherwise
         // we run the risk of queueing up Villagers forever and not getting enough food to age up.
         case cBOStepTypeUnit:
         {
            if (forecastOrder.params[cBOStepUnitBlocking] == 1)
            {
               return;
            }
            break;
         }
         case cBOStepTypeWait:
         case cBOStepTypeConditionalWait:
         case cBOStepTypeAdvance:
         {
            return;
         }
      }
      forecastStep++;
   }
}

//==============================================================================
// findBOFoodKBResource
//==============================================================================
int findBOFoodKBResource(ref int returnSubtype, int planID = -1)
{
   debugBO("Searching for the best available food resource subtype.");
   int[] validResources = new int(0, 0);
   for (int i = 0; i <= cAIResourceSubTypeHuntAggressive; i++)
   {
      float maxDistance = kbGetAutoMyBaseCreationDistanceTC();
      if (i == cAIResourceSubTypeHunt || i == cAIResourceSubTypeHuntAggressive)
      {
         maxDistance += 25.0; // We must also gather a second hunt if it's relatively close.
      }
      int[] resources = kbGetValidResourcesByPosition(boSystem.mainBasePosition, cResourceFood, i, maxDistance, 9999.0);
      for (int iResource = 0; iResource < resources.size(); iResource++)
      {
         validResources.add(resources[iResource]);
      }
   }
   float[] startingFoodDistances = new float(cAIResourceSubTypeHuntAggressive + 1, cMaxFloat);
   int[] startingFoodKBs = new int(cAIResourceSubTypeHuntAggressive + 1, -1);
   bool foundChickenBerry = false;
   bool foundHerdable = false;
   bool foundPassiveHunt = false;
   bool foundReactiveHunt = false;
   int numResources = validResources.size();
   for (int i = 0; i < numResources; i++)
   {
      int id = validResources[i];
      float resourceAmount = kbResourceGetTotalResources(id);
      if (resourceAmount < 100.0)
      {
         debugBO("Skipping food spot below 100 food total: " + id + ".");
         continue;
      }
      vector loc = kbResourceGetPosition(id);
      float distance = xsVectorDistanceXZ(boSystem.mainBasePosition, loc);
      // Dont take food that is too close to the TC, that is for later (basically don't take food attracted by Lure);
      if (distance < 15.0)
      {
         debugBO("Food resource is too close to TC: " + id + ".");
         continue;
      }

      int subtype = kbResourceGetSubType(id);
      switch (subtype)
      {
         case cAIResourceSubTypeEasy:
         {
            foundChickenBerry = true;
            break;
         }
         case cAIResourceSubTypeHerdable:
         {
            foundHerdable = true;
            break;
         }
         case cAIResourceSubTypeHunt:
         {
            foundPassiveHunt = true;
            break;
         }
         case cAIResourceSubTypeHuntAggressive:
         {
            foundReactiveHunt = true;
            break;
         }
      }
      
      if (startingFoodDistances[subtype] > distance)
      {
         debugBO("Resouce ID " + id + " for subtype: " + subtype + " is now our best option for that subtype.");
         startingFoodKBs[subtype] = id;
         startingFoodDistances[subtype] = distance;
      }
   }
   // If we can't micro we can only go to reactive hunt if we have sufficient Villagers.
   bool areAllowedReactiveHunt = (gMicroFlags & cMicroHuntMicro) != 0;
   if (areAllowedReactiveHunt == false && aiPlanGetIsIDValid(planID) == true)
   {
      int planUnits = aiPlanGetNumberUnits(planID, -1, false);
      int threshold = cMyCulture == cCultureAtlantean ? 4 : 8;
      if (planUnits >= threshold)
      {
         areAllowedReactiveHunt = true;
      }
   }
   if (areAllowedReactiveHunt == true && foundReactiveHunt == true)
   {
      debugBO("Best subtype found: reactive hunt.");
      returnSubtype = cAIResourceSubTypeHuntAggressive;
      return startingFoodKBs[cAIResourceSubTypeHuntAggressive];
   }
   if (foundPassiveHunt == true)
   {
      debugBO("Best subtype found: passive hunt.");
      returnSubtype = cAIResourceSubTypeHunt;
      return startingFoodKBs[cAIResourceSubTypeHunt];
   }
   if (foundChickenBerry == true)
   {
      debugBO("Best subtype found: chickens/berries.");
      returnSubtype = cAIResourceSubTypeEasy;
      return startingFoodKBs[cAIResourceSubTypeEasy];
   }
   if (foundHerdable == true)
   {
      debugBO("Best subtype found: herdables.");
      returnSubtype = cAIResourceSubTypeHerdable;
      return startingFoodKBs[cAIResourceSubTypeHerdable];
   }

   debugBO("findBOFoodKBResourceSubtype - couldn't find any food resources anymore, failing BO!");
   internalBODoEndStep();
   return -1;
}

//==============================================================================
// findBOWoodKBResourceHelper
//==============================================================================
int findBOWoodKBResourceHelper(vector searchPosition = cInvalidVector)
{
   int[] resources = kbGetValidResourcesByPosition(searchPosition, cResourceWood, cAIResourceSubTypeEasy,
      kbGetAutoMyBaseCreationDistanceTC(), 9999.0);
   int bestResourceID = -1;
   float bestDistance = cMaxFloat;
   int numResources = resources.size();
   for (int i = 0; i < numResources; i++)
   {
      int id = resources[i];
      float resourceAmount = kbResourceGetTotalResources(id);
      if (resourceAmount < 600.0)
      {
         debugBO("Skipping forest below 600 wood total: " + id + ".");
         continue;
      }
      vector loc = kbResourceGetPosition(id);
      int areaID = kbAreaGetIDByPosition(loc);
      if (kbAreaGetType(areaID) != cAreaTypeForest)
      {
         debugBO("Skipping stragglers kbResource: " + id + ".");
         continue;
      }

      float resDistance = xsVectorLength(searchPosition - loc);
      if (resDistance < bestDistance)
      {
         bestDistance = resDistance;
         bestResourceID = id;
      }
   }
   return bestResourceID;
}

//==============================================================================
// findBOWoodKBResource
// Using custom function for this so we can prioritize woodlines at the back.
//==============================================================================
int findBOWoodKBResource()
{
   debugBO("Searching for valid wood KB resources.");
   static bool canUseBackVector = true;
   // We want to prio forests that are at the back of our base at the start, for safety.
   vector searchPosition = cInvalidVector;
   if (canUseBackVector == true)
   {
      vector backVector = kbBaseGetBackVector(cMyID, boSystem.mainBaseID);
      vector backVectorNormalized = xsVectorNormalize(backVector);
      searchPosition = boSystem.mainBasePosition + (backVectorNormalized * 20);
      if (kbGetIsLocationOnMap(searchPosition) == false)
      {
         debugBO("findBOWoodKBResource - our calculated back vector search point wasn't on the map anymore.");
         searchPosition = boSystem.mainBasePosition;
         canUseBackVector = false;
      }
   }
   else
   {
      searchPosition = boSystem.mainBasePosition;
   }

   int bestResourceID = findBOWoodKBResourceHelper(searchPosition);
   
   if (bestResourceID == -1 && canUseBackVector == true)
   {
      debugBO("findBOWoodKBResource - couldn't find a resource using the back vector, using main base location now.");
      canUseBackVector = false;
      bestResourceID = findBOWoodKBResourceHelper(boSystem.mainBasePosition);
   }
   if (bestResourceID == -1)
   {
      debugBO("findBOWoodKBResource - couldn't find a valid resource, this makes the BO fail!");
      internalBODoEndStep();
      return -1;
   }
   return bestResourceID;
}

//==============================================================================
// findBOGoldKBResource
//==============================================================================
int findBOGoldKBResource()
{
   debugBO("Searching for valid gold KB resources.");
   int bestResourceID = -1;
   float bestDistance = cMaxFloat;
   float maxDistance = kbGetAutoMyBaseCreationDistanceTC();
   int mainBaseAreaGroupID = kbAreaGroupGetIDByPosition(boSystem.mainBasePosition);

   int[] resources = kbGetValidResourcesByPosition(boSystem.mainBasePosition, cResourceGold, cAIResourceSubTypeEasy,
      maxDistance, 9999.0);
   for (int i = 0; i < resources.size(); i++)
   {
      int id = resources[i];
      if (kbResourceGetTotalResources(id) < 100.0)
      {
         debugBO("Skipping gold resource below 100 gold total: " + id + ".");
         continue;
      }

      vector loc = kbResourceGetPosition(id);
      int resourceAreaGroupID = kbAreaGroupGetIDByPosition(loc);
      if (mainBaseAreaGroupID != -1 && resourceAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(mainBaseAreaGroupID, resourceAreaGroupID, cPassabilityAmphibious) == false)
      {
         debugBO("Skipping gold resource not connected by amphibious passability: " + id + ".");
         continue;
      }

      float distance = xsVectorDistanceXZ(boSystem.mainBasePosition, loc);
      if (distance < bestDistance)
      {
         bestDistance = distance;
         bestResourceID = id;
      }
   }

   static int goldQueryID = -1;
   if (goldQueryID == -1)
   {
      goldQueryID = kbUnitQueryCreate("BO amphibious gold resources");
      kbUnitQuerySetPlayerID(goldQueryID, cPlayerMotherNatureID);
      kbUnitQuerySetUnitType(goldQueryID, cUnitTypeGoldResource);
      kbUnitQuerySetState(goldQueryID, cUnitStateAlive);
      kbUnitQuerySetVisibleState(goldQueryID, cUnitQueryVisibleStateRecentPositionKnown);
   }
   if (goldQueryID != -1)
   {
      kbUnitQuerySetPosition(goldQueryID, boSystem.mainBasePosition);
      kbUnitQuerySetMaximumDistance(goldQueryID, maxDistance);
      kbUnitQueryResetResults(goldQueryID);
      int numResults = kbUnitQueryExecute(goldQueryID);
      for (int i = 0; i < numResults; i++)
      {
         int unitID = kbUnitQueryGetResult(goldQueryID, i);
         int kbResourceID = kbUnitGetKBResourceID(unitID);
         if (kbResourceGetIsIDValid(kbResourceID) == false ||
             kbResourceGetType(kbResourceID) != cResourceGold ||
             kbResourceGetSubType(kbResourceID) != cAIResourceSubTypeEasy ||
             kbResourceGetTotalResources(kbResourceID) < 100.0)
         {
            continue;
         }

         vector loc = kbUnitGetPosition(unitID);
         int resourceAreaGroupID = kbAreaGroupGetIDByPosition(loc);
         if (mainBaseAreaGroupID != -1 && resourceAreaGroupID != -1 &&
             kbPathAreAreaGroupsConnected(mainBaseAreaGroupID, resourceAreaGroupID, cPassabilityAmphibious) == false)
         {
            continue;
         }

         float distance = xsVectorDistanceXZ(boSystem.mainBasePosition, loc);
         if (distance < bestDistance)
         {
            debugBO("Gold resource " + kbResourceID + " was found by amphibious unit fallback.");
            bestDistance = distance;
            bestResourceID = kbResourceID;
         }
      }
   }

   if (bestResourceID == -1)
   {
      debugBO("findBOGoldKBResource - couldn't find a valid resource, this makes the BO fail!");
      internalBODoEndStep();
      return -1;
   }
   return bestResourceID;
}

//==============================================================================
// initBOSystem
//==============================================================================
bool initBOSystem()
{
   debugBO("BOSystem init.");
   boSystem.mainBaseID = kbBaseGetMainID(cMyID);
   boSystem.mainBasePosition = kbBaseGetLocation(cMyID, boSystem.mainBaseID);
   debugBO("Main Base ID: " + boSystem.mainBaseID + ".");

   // Disable all auto assignment / auto gather plan creation.
   aiSetNextGathererDistributionTime(-1);
   aiSetUnassignedUnitAssignmentTime(-1);
   aiSetFullUnitAssignmentTime(-1);
   // To pick up idle military during BO we have a custom rule.
   xsEnableRule("addMilitaryToDefendPlanDuringBO");

   if (cGameModeCurrent != cGameModeDeathmatch)
   {
      // Reduce our base's size so build plans go much faster.
      kbBaseSetDistance(cMyID, boSystem.mainBaseID, 37.50);

      // We should just make this big enough for all.
      boSystem.currentGatherPlans = new int(4, -1);
      int numResources = 3;
      if (cMyCulture == cCultureGreek)
      {
         numResources++; // Favor plan.
      }
      for (int resourceType = 0; resourceType < numResources; resourceType++)
      {
         int planID = aiPlanCreate("GatherPlan: " + kbGetResourceName(resourceType), cPlanGather, -1, gEconomyCategoryID);
         aiPlanSetVariableBool(planID, cGatherPlanAutoBuildDropsite, 0, false);
         aiPlanSetVariableInt(planID, cGatherPlanResourceType, 0, resourceType);

         // For food we need to find the optimal subtype + kbResourceID.
         int subtype = cAIResourceSubTypeEasy;
         if (resourceType == cResourceFood)
         {
            int kbResourceID = findBOFoodKBResource(subtype);
            if (kbResourceID == -1)
            {
               // We ended the BO already.
               return false;
            }
            aiPlanSetVariableInt(planID, cGatherPlanKBResourceID, 0, kbResourceID);
         }
         aiPlanSetVariableInt(planID, cGatherPlanResourceSubType, 0, subtype);

         // For wood we manually find the woodline, because it should be at the back.
         if (resourceType == cResourceWood)
         {
            int kbResourceID = findBOWoodKBResource();
            if (kbResourceID == -1)
            {
               // We ended the BO already.
               return false;
            }
            aiPlanSetVariableInt(planID, cGatherPlanKBResourceID, 0, kbResourceID);
         }

         // For gold we also assign the KB resource manually so amphibious-edge mines are not skipped by the source selector.
         if (resourceType == cResourceGold)
         {
            int kbResourceID = findBOGoldKBResource();
            if (kbResourceID == -1)
            {
               // We ended the BO already.
               return false;
            }
            aiPlanSetVariableInt(planID, cGatherPlanKBResourceID, 0, kbResourceID);
         }

         // Already add the Ox Cart unit type here, this makes sure that if the BO fails it's still properly added and not blank.
         if (cMyCulture == cCultureNorse &&
             subtype != cAIResourceSubTypeFarm &&
             subtype != cAIResourceSubTypeHerdable)
         {
            aiPlanAddUnitType(planID, cUnitTypeOxCart, 1, 1, 1);
         }

         if (resourceType != cResourceFavor)
         {
            // This handler will search for new resources for us via the same BO find system if we run out + enable dropsites if needed.
            aiPlanSetEventHandler(planID, cGatherPlanEventResourceUpdate, "internalBO" + kbGetResourceName(resourceType) +
               "ResourceUpdate");
         }
         aiPlanSetBaseID(planID, boSystem.mainBaseID);
         boSystem.currentGatherPlans[resourceType] = planID;
      }
   }
   else
   {
      // Reduce our base's size a lot so that the Temple placement goes really fast, increase in Classical again.
      kbBaseSetDistance(cMyID, boSystem.mainBaseID, 25.00);

      // For Deathmatch we need to instantly start building at a second Settlement, create a base for that.
      // Deathmatch has a revealed map so this will work.
      int settlementID = -1;
      if (cMyCulture == cCultureAtlantean && cNumberPlayers == 2)
      {
         // If we're in a 1v1 as Atlantean, build on a forward TC.
         int queryID = useSimpleUnitQuery(cUnitTypeSettlement, cPlayerMotherNatureID, cUnitStateAlive,
            boSystem.mainBasePosition, 100.0);
         kbUnitQuerySetConnectedAreaGroupID(queryID, kbAreaGroupGetIDByPosition(boSystem.mainBasePosition), cPassabilityLand);
         kbUnitQuerySetAscendingSort(queryID, true);
         int numResults = kbUnitQueryExecute(queryID);
         int[] results = kbUnitQueryGetResults(queryID);
         if (numResults == 0)
         {
            debugBO("initBOSystem - couldn't find a valid settlement for a second base, this makes the BO fail!");
            internalBODoEndStep();
            return false;
         }
         else if (numResults == 1)
         {
            settlementID = results[0];
         }
         else
         {
            settlementID = results[1];
         }
      }
      else
      {
         settlementID = getClosestUnitByLocationConnectedAreaGroup(cUnitTypeSettlement, cPlayerMotherNatureID, cUnitStateAlive,
            boSystem.mainBasePosition, 100.0, cPassabilityLand);
         if (settlementID == -1)
         {
            debugBO("initBOSystem - couldn't find a valid settlement for a second base, this makes the BO fail!");
            internalBODoEndStep();
            return false;
         }
      }
      boSystem.secondBaseTCID = settlementID;
      vector basePosition = kbUnitGetPosition(settlementID);
      // Create another base so that we can put build/defend plans in there.
      // 42.50 range so buildings have space to go into but it's not too big that it slows placement too much.
      int baseID = kbBaseCreate(cMyID, kbBaseGetNextID() + " DM Second Base", basePosition, 42.50);
      if (kbBaseGetIsIDValid(cMyID, baseID) == false)
      {
         debugBO("initBOSystem - couldn't create a base for the DM settlement base, this makes the BO fail!");
         internalBODoEndStep();
         return false;
      }
      kbBaseSetFrontVector(cMyID, baseID, kbGetMapCenter() - basePosition);
      kbBaseSetMilitaryGatherPoint(cMyID, baseID, (kbBaseGetFrontVector(cMyID, baseID) * 30) + basePosition);
   
      // Make sure the base isn't instantly deleted too, kinda abuse the gatherbase flag.
      kbBaseSetFlag(cMyID, baseID, cBaseFlagRemoteGatherBase, true);
      boSystem.secondBaseID = baseID;
      boSystem.secondBasePosition = basePosition;
      debugBO("Second Base ID: " + boSystem.secondBaseID + ".");

      // Oracles must gather favor instantly.
      xsDisableRule("startupOracleScoutingMonitor");
      xsEnableRule("oracleMonitor");
      xsEnableRule("oracleMaintainMonitor");
   }
   return true;
}

//==============================================================================
// startBOSystem
// This gets called after our BO steps are already filled.
//==============================================================================
bool startBOSystem() 
{
   if (boSystem.buildOrderSteps.size() == 0)
   {
      aiEchoWarning("We want to do a BO but found 0 steps.");
      return false;
   }
   if (cGameModeCurrent != cGameModeDeathmatch)
   {
      // Set our start of game rally point on the resource the first Villager wants to gather.
      int i = 0;
      int size = boSystem.buildOrderSteps.size();
      bool found = false;
      while (i < size && found == false)
      {
         BOStep nextOrder = boSystem.buildOrderSteps[i];
         switch (nextOrder.type)
         {
            case cBOStepTypeVillager:
            {
               int resourceType = nextOrder.params[cBOStepVillagerResourceType];
               // We know this resourceID is valid because otherwise we wouldn't reach here.
               aiUnitSetRallyPointToPosition(getUnit(cUnitTypeAbstractTownCenter),
                  kbResourceGetPosition(aiPlanGetVariableInt(boSystem.currentGatherPlans[resourceType], cGatherPlanKBResourceID, 0)));
               found = true;
               break;
            }
         }
         i++;
      }
   }
   xsEnableRule("monitorBO");
   return true;
}

mutable void internalBOVillagerTrained(int unitID = -1) {}
//==============================================================================
// handleStartingVillagers
// Your first orders inside the BO MUST be handling your starting Villagers.
// If you have 4 starting Villagers your first 4 steps MUST BE 4 Villager steps, or it all crumbles.
//==============================================================================
bool handleStartingVillagers()
{
   int villagerUnitQuery = useSimpleUnitQuery(cUnitTypeAbstractVillager);
   int numberVillagers = -1;
   if (villagerUnitQuery != -1)
   {
      numberVillagers = kbUnitQueryExecute(villagerUnitQuery);
      // In Deathmatch we spawn with the Hero Kuafu, handle it so we don't think we need a step for it.
      if (cGameModeCurrent == cGameModeDeathmatch && cMyCiv == cCivNuwa)
      {
         numberVillagers--;
      }
   }
   else
   {
      // We can't function if the query failed.
      aiEchoWarning("handleStartingVillagers - We couldn't create a query.");
      internalBODoEndStep();
      return false;
   }

   for (int i = 0; i < numberVillagers; i++)
   {
      BOStep step = boSystem.buildOrderSteps[i];
      if (step.type != cBOStepTypeVillager)
      {
         // You must handle all starting Villagers first.
         aiEchoWarning("handleStartingVillagers - Found a non Villager step.");
         internalBODoEndStep();
         return false;
      }
   }

   debugBO("Handling our starting Villagers.");
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      boSystem.currentVillagerStep = numberVillagers;
      boSystem.currentStep = numberVillagers;
      // Add back the Hero Kuafu so we don't think we lost a Villager.
      if (cGameModeCurrent == cGameModeDeathmatch && cMyCiv == cCivNuwa)
      {
         numberVillagers++;
      }
      boSystem.numVillagers = numberVillagers;
      return true;
   }

   for (int i = 0; i < numberVillagers; i++)
   {
      int id = kbUnitQueryGetResult(villagerUnitQuery, i);
      internalBOVillagerTrained(id); // Here we do the default assignment.
      boSystem.currentVillagerStep++;
      boSystem.currentStep++;
   }
   
   return true;
}

//==============================================================================
// monitorBO
//==============================================================================
rule monitorBO
inactive
highFrequency
group groupBOSystem
{
   static bool firstUnitsSpawned = false;
   if (firstUnitsSpawned == false)
   {
      // If we're Chinese we must wait until the first Kuafu is also spawned, or our starting Villager handling doesn't work.
      // The KuafuHero check is here for DM BOs.
      #if (cMyCulture == cCultureChinese)
      if (getUnit(cUnitTypeVillagerChinese) >= 0 && (getUnit(cUnitTypeKuafu) >= 0 || getUnit(cUnitTypeKuafuHero) >= 0))
      {
         if (handleStartingVillagers() == false)
         {
            return;
         }
         firstUnitsSpawned = true;
      }
      #else
      if (getUnit(cUnitTypeAbstractVillager) >= 0)
      {
         if (handleStartingVillagers() == false)
         {
            return;
         }
         firstUnitsSpawned = true;
      }
      #endif
      else if (cGameModeCurrent == cGameModeDeathmatch && cMyCulture == cCultureNorse && getUnit(cUnitTypeBerserk) >= 0)
      {
         firstUnitsSpawned = true;
      }
      else
      {
         // If it takes longer than normal to have units spawn we must be in some custom situation, BOs aren't great for that.
         if (xsGetTime() > 10)
         {
            aiEchoWarning("Our Villagers haven't spawned after 10 seconds, we shouldn't have picked the BO strategy.");
            internalBODoEndStep();
         }
         return;
      }
   }

   // Do our steps.
   // boSystem.nextUpdate sometimes gets set to cMaxInt to wait for a step to complete before continuing.
   // And sometimes it's not set to cMaxInt and then steps follow each other up quite rapidly.
   int time = xsGetTime();
   int size = boSystem.buildOrderSteps.size();
   bool ageUpDelayed = false;
   while (time >= boSystem.nextUpdate && boSystem.currentStep < size)
   {
      // If one of our steps fails but we were going to execute multiple steps in the same frame, we need to stop those next steps.
      if (boSystem.done == true)
      {
         return;
      }
      // We can only age up on lower difficulties if a human has done so before us.
      // If there is no human left in the game we just age up.
      // And if there is another AI on higher difficulty that has already aged up we follow suit, not waiting on the human.
      if (cDifficultyCurrent <= cDifficultyModerate)
      {
         static int timeBlockedStart = 0;
         BOStep currentOrder = boSystem.buildOrderSteps[boSystem.currentStep];
         if (currentOrder.type == cBOStepTypeAdvance)
         {
            if (getHighestPlayerAge() == cAge1 && getHumanPresentInGame() == true) // Heroic BOs are not waiting on player age ups.
            {
               if (timeBlockedStart == 0)
               {
                  timeBlockedStart = xsGetTime();
                  debugBO("Waiting on human/higher difficulty AI to age up, timeBlockedStart = " + timeBlockedStart + ".");
               }
               ageUpDelayed = true;
               break;
            }
            else
            {
               if (timeBlockedStart != 0)
               {
                  int totalTimeBlocked = xsGetTime() - timeBlockedStart;
                  debugBO("Increasing our timeout by: " + totalTimeBlocked + ", because of waiting on AgeUp.");
                  boSystem.timeout += totalTimeBlocked;
                  timeBlockedStart = 0;
               }
            }
         }
      }
      updateBOSystem();
   }

   if (time >= boSystem.nextForecast)
   {
      forecastBOSystem();
   }
   
   if (getUnit(cUnitTypeAbstractTownCenter) == -1)
   {
      debugBO("LOST OUR TOWN CENTER / CITADEL CENTER, ENDING BO!");
      internalBODoEndStep();
      return;
   }

   if (boSystem.numVillagers != kbUnitCount(cUnitTypeAbstractVillager, cMyID, cUnitStateAlive))
   {
      bool mustEndBO = true;
      if (cMyCiv == cCivDemeter)
      {
         // We handle the Pan ageup here.
         static bool handledLykaonVillager = false;
         if (handledLykaonVillager == false && kbUnitCount(cUnitTypeLykaonVillager, cMyID, cUnitStateAlive) > 0)
         {
            boSystem.numVillagers++;
            handledLykaonVillager = true;
            mustEndBO = false;
         }
      }
      if (mustEndBO == true)
      {
         debugBO("LOST A VILLAGER, ENDING BO!");
         internalBODoEndStep();
         return;
      }
   }

   if (cMyCulture == cCultureNorse)
   {
      static int lastOxCartCount = 0;
      int currentOxCartCount = kbUnitCount(cUnitTypeOxCart, cMyID, cUnitStateAlive);
      if (currentOxCartCount >= lastOxCartCount)
      {
         lastOxCartCount = currentOxCartCount;
      }
      else
      {
         debugBO("LOST AN OX CART, ENDING BO!");
         internalBODoEndStep();
         return;
      }
   }

   if (cMyCulture == cCultureNorse && xsGetTime() >= 10 &&
       kbUnitCount(cUnitTypeLogicalTypeNorseSoldierThatBuilds, cMyID, cUnitStateABQ) <= 0)
   {
      debugBO("LOST ALL OUR BUILDERS, ENDING BO!");
      internalBODoEndStep();
      return;
   }
   // TODO insert ending logic for when we're under serious threat.
   //if (???)
   //{
   //   debugBO("WE'RE UNDER ATTACK, ENDING BO!");
   //   internalBODoEndStep();
   //   return;
   //}

   if (ageUpDelayed == false && boSystem.timeout != 0)
   {
      int timeSpentInCurrentSection = time - boSystem.timeoutBegin;
      if (timeSpentInCurrentSection >= boSystem.timeout)
      {
         debugBO("BO TIMED OUT, ENDING!");
         internalBODoEndStep();
         return;
      }
   }
}

//==============================================================================
// addMilitaryToDefendPlanDuringBO
// We want all our military in the defend plan but can't assign them onCreate
// because that messes with parent-child logic and sometimes overwrites existing plans.
// So just assign free military to the plan here, during the BO you can take units from this plan.
//==============================================================================
rule addMilitaryToDefendPlanDuringBO
inactive
minInterval 5
{
   if (aiPlanGetIsIDValid(gPrimaryLandDefendPlan) == false)
   {
      return;
   }
   int queryID = useSimpleUnitQuery(cUnitTypeLogicalTypeLandMilitary);
   int numResults = kbUnitQueryExecute(queryID);
   int[] results = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int unitID = results[i];
      int unitPlanID = kbUnitGetPlanID(unitID);
      if (unitPlanID == -1)
      {
         aiPlanAddUnit(gPrimaryLandDefendPlan, unitID);
      }
   }
}
