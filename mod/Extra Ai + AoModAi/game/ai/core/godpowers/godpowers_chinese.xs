//==============================================================================
/* godpowers_chinese.xs

   This file contains all logic for the Chinese god powers.
*/
//==============================================================================

extern int gCreationPlanID = -1;
extern int gThePeachBlossomSpringPlanID = -1;
extern int gProsperousSeedsPlanID = -1;
extern int gFeiBeastPlanID = -1;
extern int gVanishPlanID = -1;
extern int gFeiBeastsAttackPlanID = -1;
extern int gFeiBeastsDefendPlanID = -1;
extern int gForestProtectionPlanID = -1;
extern int gDroughtLandPlanID = -1;
extern int gGreatFloodPlanID = -1;

//==============================================================================
// setupChineseGodPowerPlan
//==============================================================================
void setupChineseGodPowerPlan(int planID = -1, int protoPowerID = -1)
{
   aiPlanSetVariableInt(planID, cPlanGodPower, 0, protoPowerID);

   switch (protoPowerID)
   {
      case cProtoPowerCreation:
      {
         gCreationPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationManually);
         xsEnableRule("creationMonitor");
         xsRuleIgnoreIntervalOnce("creationMonitor");
         break;
      }

      case cProtoPowerThePeachBlossomSpring:
      {
         gThePeachBlossomSpringPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelBuildingPlacement);
         xsEnableRule("thePeachBlossomSpringMonitor");
         xsRuleIgnoreIntervalOnce("thePeachBlossomSpringMonitor");
         break;
      }

      case cProtoPowerProsperousSeeds:
      {
         gProsperousSeedsPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationManually);
         xsEnableRule("prosperousSeedsMonitor");
         xsRuleIgnoreIntervalOnce("prosperousSeedsMonitor");
         break;
      }

      case cProtoPowerEarthWall:
      {
         gForestProtectionPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelUnit);
         xsEnableRule("forestProtectionMonitor");
         xsRuleIgnoreIntervalOnce("forestProtectionMonitor");
         break;
      }

      case cProtoPowerVanish:
      {
         gVanishPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationManually);
         xsEnableRule("vanishMonitor");
         xsRuleIgnoreIntervalOnce("vanishMonitor");
         break;
      }

      case cProtoPowerLightningWeapons:
      {
         int queryID = kbUnitQueryCreate(aiPlanGetName(planID) + " Lightning Weapons Query");
         kbUnitQuerySetPlayerRelation(queryID, cPlayerRelationEnemyNotGaia);
         kbUnitQuerySetUnitType(queryID, cUnitTypeMilitaryUnit);
         kbUnitQuerySetMaximumDistance(queryID, 20.0);
         kbUnitQuerySetState(queryID, cUnitStateAlive);
         kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateVisible);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryMinimumCount, 0, 20);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryID, 0, queryID);

         aiPlanAddUserVariableBool(planID, 1, "Not Titan", 1);
         aiPlanSetUserVariableBool(planID, 1, 0, true);

         // Be healthy before we cast.
         aiPlanSetVariableFloat(planID, cGodPowerPlanOwnArmyMinimumPercentHealth, 0, 70);
         aiPlanSetVariableInt(planID, cGodPowerPlanOwnArmyMinimumCount, 0, selectByDifficulty(5, 8, 11, 15, 18, 20));

         aiPlanSetVariableBool(planID, cGodPowerPlanRequiresCombatPlan, 0, true);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelCombatPlanDistance);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelWorld);
         break;
      }

      case cProtoPowerVenomBeast:
      {
         if (cPersonalityCurrent == cPersonalityDefender ||
             (cPersonalityCurrent != cPersonalityAttacker&& xsRandBool() == true))
         {
            gFeiBeastPlanID = planID;
            aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
            aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
            aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationManually);
            xsEnableRule("feiBeastsDefensivelyMonitor");
            xsRuleIgnoreIntervalOnce("feiBeastsDefensivelyMonitor");
            break;
         }

         int queryID = kbUnitQueryCreate(aiPlanGetName(planID) + " Fei Beasts Query");
         kbUnitQuerySetPlayerRelation(queryID, cPlayerRelationEnemyNotGaia, false);
         kbUnitQuerySetUnitType(queryID, cUnitTypeLogicalTypeLandMilitary);
         kbUnitQuerySetMaximumDistance(queryID, 20.0);
         kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateVisible);
         kbUnitQuerySetState(queryID, cUnitStateAlive);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryMinimumCount, 0, 10);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryID, 0, queryID);
         
         aiPlanAddUserVariableBool(planID, 0, "Not Naval", 1);
         aiPlanSetUserVariableBool(planID, 0, 0, true);

         // Can't add units to Titan plans, so don't allow them.
         aiPlanAddUserVariableBool(planID, 1, "Not Titan", 1);
         aiPlanSetUserVariableBool(planID, 1, 0, true);

         aiPlanSetVariableBool(planID, cGodPowerPlanRequiresCombatPlan, 0, true);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelCombatPlanDistance);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocation);
         break;
      }

      case cProtoPowerForestProtection:
      {
         gForestProtectionPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelUnit);
         xsEnableRule("forestProtectionMonitor");
         xsRuleIgnoreIntervalOnce("forestProtectionMonitor");
         break;
      }

      case cProtoPowerDroughtLand:
      {
         gDroughtLandPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationManually);
         xsEnableRule("droughtLandMonitor");
         xsRuleIgnoreIntervalOnce("droughtLandMonitor");
         break;
      }

      case cProtoPowerGreatFlood:
      {
         gGreatFloodPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationDual);
         xsEnableRule("greatFloodMonitor");
         xsRuleIgnoreIntervalOnce("greatFloodMonitor");
         break;
      }
      
      case cProtoPowerYinglongsWrath:
      {
         aiPlanSetVariableVector(planID, cGodPowerPlanTargetLocation, 0, aiPlanGetVariableVector(gPrimaryLandDefendPlan,
            cDefendPlanGatherPoint, 0));
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationManually);
         break;
      }

      case cProtoPowerBlazingPrairie:
      {
         int queryID = kbUnitQueryCreate(aiPlanGetName(planID) + " Blazing Prairie Query");
         kbUnitQuerySetPlayerRelation(queryID, cPlayerRelationEnemyNotGaia, false);
         kbUnitQuerySetUnitType(queryID, cUnitTypeMilitaryUnit);
         kbUnitQuerySetMaximumDistance(queryID, 20.0);
         kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateVisible);
         kbUnitQuerySetState(queryID, cUnitStateAlive);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryMinimumCount, 0, 15);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryID, 0, queryID);
         
         aiPlanAddUserVariableBool(planID, 0, "Not Naval", 1);
         aiPlanSetUserVariableBool(planID, 0, 0, true);

         aiPlanSetVariableBool(planID, cGodPowerPlanRequiresCombatPlan, 0, true);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelCombatPlanDistance);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocation);
         break;
      }
   }
}

//==============================================================================
// haveFewerEcoPopThanCreationThreshold
//==============================================================================
bool haveFewerEcoPopThanCreationThreshold(bool rebuy = false)
{
   const float creationThreshold = 0.3;
   int wantedLandEcoPop = aiGetEconomyPop();
   int currentLandEcoPop = aiGetCurrentEconomyPop();
   float havePercentEcoPop = min(1.0, xsIntToFloat(currentLandEcoPop) / xsIntToFloat(wantedLandEcoPop));
   if (havePercentEcoPop > creationThreshold)
   {
      if (rebuy == true)
      {
         debugGodPowers("   Not rebuying Creation because we have " + havePercentEcoPop + " percent of our land economy pop " +
          "which is higher than the " + creationThreshold + " percent threshold.");
      }
      else
      {
         debugGodPowers("We have " + havePercentEcoPop + " percent of our land economy pop which is higher than the "
            + creationThreshold + " percent threshold, no need for Creation now.");
      }
      return false;
   }
   return true;
}

//==============================================================================
// vanishMonitor
// Try to find some ally military and randomly Vanish one of them, pray it hits more.
//==============================================================================
rule vanishMonitor
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule vanishMonitor. ---");

   int[] plans = getValidPlansForGodpowerCasting(cIncludeAttackPlans, cIncludeDefendPlans, cIncludeExplorePlans, cIncludeNavalPlans);
   int numPlans = plans.size();
   if (numPlans <= 0)
   {
      debugGodPowers("Found 0 attack/defend/explore plans in attack state to analyze.");
      return;
   }

   for (int i = 0; i < plans.size(); i++)
   {
      int queryID = useSimpleUnitQuery(cUnitTypeAbstractVillager, cPlayerRelationAlly, cUnitStateAlive,
         aiPlanGetLocation(plans[i]), 20.0);
      kbUnitQuerySetAscendingSort(queryID, true);
      kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateVisible);
      kbUnitQueryExecute(queryID);
      int[] allies = kbUnitQueryGetResults(queryID);
      if (allies.size() < 10)
      {
         debugGodPowers("We didn't find enough allies to cast Vanish for this plan: " + aiPlanGetName(plans[i]) + ".");
         continue;
      }
      debugGodPowers("We found enough allies near plan: " + aiPlanGetName(plans[i]) + " to cast Vanish!");
      // Take closest unit to cast on.
      aiPlanSetVariableVector(gVanishPlanID, cGodPowerPlanTargetLocation, 0, kbUnitGetTruePosition(allies[0]));
      aiPlanSetVariableBool(gVanishPlanID, cGodPowerPlanAutoCast, 0, true);
      xsDisableRule("vanishMonitor");
      return;
   }
}

//==============================================================================
// creationMonitor
// Cast Creation if we have a low land economy pop %.
//==============================================================================
rule creationMonitor
inactive
minInterval 30
{
   debugGodPowers("--- Running Rule creationMonitor. ---");

   if (haveFewerEcoPopThanCreationThreshold(false) == false)
   {
      return;
   }
   // Find a random Villager in a safe area and cast Creation on it.
   int queryID = useSimpleUnitQuery(cUnitTypeAbstractVillager);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(queryID, gLandAreaGroupID, cPassabilityLand);
   }
   int numResults = kbUnitQueryExecute(queryID);
   int[] results = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int villagerID = results[i];
      if (kbAreaGetDangerLevel(kbUnitGetAreaID(villagerID)) < 100.0)
      {
         debugGodPowers("Casting Creation on " + villagerID + ".");
         aiPlanSetVariableBool(gCreationPlanID, cGodPowerPlanAutoCast, 0, true);
         aiPlanSetVariableVector(gCreationPlanID, cGodPowerPlanTargetLocation, 0, kbUnitGetPosition(villagerID));
         xsDisableRule("creationMonitor");
         return;
      }
   }
   // If we're here we found either no Villagers or none that were valid. Since this could save us from getting stuck in a bad spot
   // we can't give up now. Check for TC bases as a last resort.
   int numBases = kbBaseGetNumber(cMyID);
   for (int i = 0; i < numBases; i++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, i);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         debugGodPowers("Skipping base: " + kbBaseGetNameByID(cMyID, baseID) + ", because it's not a Town Center base.");
         continue;
      }
      vector baseLocation = kbBaseGetLocation(cMyID, baseID);
      if (gLandAreaGroupID != -1 && 
          kbPathAreAreaGroupsConnected(gLandAreaGroupID, kbAreaGroupGetIDByPosition(baseLocation), cPassabilityLand) == false)
      {
         debugGodPowers("Skipping base: " + kbBaseGetNameByID(cMyID, baseID) + ", because it's not connected to gLandAreaGroupID.");
         continue;
      }
      vector castLocation = kbBaseGetMilitaryGatherPoint(cMyID, baseID);
      if (kbLocationVisible(castLocation) == false)
      {
         debugGodPowers("Skipping base: " + kbBaseGetNameByID(cMyID, baseID) + ", because its MGP isn't visible.");
         continue;
      }
      debugGodPowers("Casting Creation on the MGP of " + kbBaseGetNameByID(cMyID, baseID) + ".");
      aiPlanSetVariableBool(gCreationPlanID, cGodPowerPlanAutoCast, 0, true);
      aiPlanSetVariableVector(gCreationPlanID, cGodPowerPlanTargetLocation, 0, castLocation);
      xsDisableRule("creationMonitor");
      return;
   }
}

//==============================================================================
// thePeachBlossomSpringMonitor
// TODO blacklist this resource when it hasn't stacked up enough resources.
//==============================================================================
rule thePeachBlossomSpringMonitor
inactive
minInterval 15
{
   debugGodPowers("--- Running Rule thePeachBlossomSpringMonitor. ---");

   int baseID = getMostDefendedTCBase();
   if (baseID == -1)
   {
      debugGodPowers("Can't cast The Peach Blossom Spring atm because we have no Town Center base.");
      return;
   }

   vector basePosition = kbBaseGetLocation(cMyID, baseID);
   if (getUnitCountByLocation(cUnitTypeMilitaryUnit, cPlayerRelationEnemyNotGaia, cUnitStateAlive, basePosition,
       kbBaseGetDistance(cMyID, baseID)) > 0)
   {
      debugGodPowers("Can't cast The Peach Blossom Spring atm because we see enemies in base " + kbBaseGetNameByID(cMyID, baseID)
         + ".");
      return;
   }

   int bpID = kbBuildingPlacementCreate("ThePeachBlossomSpring Placement"); // Slightly different naming or it gets too long.
   kbBuildingPlacementSetBuildingPUID(bpID, cUnitTypeThePeachBlossomSpring);
   addSafeBackAreasToBuildingPlacement(bpID, baseID, gGodpowersCategoryID);
   kbBuildingPlacementSetRequiresCompletelyUnobstructed(bpID, true);
   // Risky cuz it can block, but this thing is hard enough to place as it is.
   kbBuildingPlacementSetBufferSpace(bpID, 1.0);
   aiPlanSetVariableInt(gThePeachBlossomSpringPlanID, cGodPowerPlanBPID, 0, bpID);
   aiPlanSetVariableBool(gThePeachBlossomSpringPlanID, cGodPowerPlanAutoCast, 0, true);
   xsDisableRule("thePeachBlossomSpringMonitor");
}

//==============================================================================
//	findFarmForProsperousSeeds
//==============================================================================
int findFarmForProsperousSeeds(bool output = false)
{
   int queryID = useSimpleUnitQuery(gFarmUnit);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(queryID, gLandAreaGroupID, cPassabilityLand);
   }
   int numResults = kbUnitQueryExecute(queryID);
   if (numResults == 0)
   {
      if (output == true)
      {
         debugGodPowers("Currently have no Farms alive to cast Prosperous Seeds on.");
      }
      return -1;
   }
   int[] results = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int farmID = results[i];
      int baseID = kbUnitGetBaseID(farmID);
      if (baseID == -1 || kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         if (output == true)
         {
            debugGodPowers("Farm " + farmID + " is not in a TC base, and is thus invalid for Prosperous Seeds.");
         }
         continue;
      }
      return farmID;
   }
   return -1;
}

//==============================================================================
//	prosperousSeedsMonitor
// We don't try to find 2 adjacent Farms to make optimal use.
// We're just hoping that with the big cast radius we actually upgrade 2 instead of 1.
//==============================================================================
rule prosperousSeedsMonitor
inactive
minInterval 15
{
   debugGodPowers("--- Running Rule prosperousSeedsMonitor. ---");
   
   int farmID = findFarmForProsperousSeeds(true);
   if (farmID == -1)
   {
      return;
   }
   debugGodPowers("Casting Prosperous Seeds on " + farmID + ".");
   vector castLocation = kbUnitGetPosition(farmID);
   aiPlanSetVariableBool(gProsperousSeedsPlanID, cGodPowerPlanAutoCast, 0, true);
   aiPlanSetVariableVector(gProsperousSeedsPlanID, cGodPowerPlanTargetLocation, 0, castLocation);
   xsDisableRule("prosperousSeedsMonitor");
}

//==============================================================================
// feiBeastsDefensivelyMonitor
//==============================================================================
rule feiBeastsDefensivelyMonitor
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule feiBeastsDefensivelyMonitor. ---");
   
   int planID = godPowerFindDefendPlanToCastFor();
   if (planID == -1)
   {
      return;
   }

   int baseID = aiPlanGetBaseID(planID);
   int closestUnit = getClosestUnitByLocation(cUnitTypeMilitaryUnit, cPlayerRelationEnemyNotGaia, cUnitStateAlive,
                        kbBaseGetLocation(cMyID, baseID), kbBaseGetDistance(cMyID, baseID) + 10.0);
   if (closestUnit != -1)
   {
      aiPlanSetVariableVector(gFeiBeastPlanID, cGodPowerPlanTargetLocation, 0, kbUnitGetPosition(closestUnit));
      aiPlanSetVariableBool(gFeiBeastPlanID, cGodPowerPlanAutoCast, 0, true);
      debugGodPowers("Casting Fei Beasts defensively for plan " + aiPlanGetName(planID) + ".");
      xsDisableRule("feiBeastsDefensivelyMonitor");
      gFeiBeastsDefendPlanID = planID;
   }
}

//==============================================================================
// addFeiBeastsToPlanMonitor
//==============================================================================
rule addFeiBeastsToPlanMonitor
inactive
minInterval 2
{
   int planID = -1;
   if (aiPlanGetIsIDValid(gFeiBeastsAttackPlanID) == true)
   {
      planID = gFeiBeastsAttackPlanID;
   }
   if (aiPlanGetIsIDValid(gFeiBeastsDefendPlanID) == true)
   {
      planID = gFeiBeastsDefendPlanID;
   }
   if (planID == -1)
   {
      xsDisableRule("addFeiBeastsToPlanMonitor");
      return;
   }
   debugGodPowers("--- Running Rule addFeiBeastsToPlanMonitor. ---");
   int queryID = useSimpleUnitQuery(cUnitTypeFei);
   int numMinions = kbUnitQueryExecute(queryID);
   int[] units = kbUnitQueryGetResults(queryID);
   bool oldValue = aiPlanIsFlagSet(planID, cPlanFlagNoUnitNotification);
   aiPlanSetFlag(planID, cPlanFlagNoUnitNotification, true);
   for (int i = 0; i < numMinions; i++)
   {
      if (isUnitAlreadyInPlanOrChildOf(units[i], planID) == false)
      {
         aiPlanAddUnit(planID, units[i]);
         debugGodPowers("Added unitID: " + units[i] + " to: " +  aiPlanGetName(planID) + ".");
      }
   }
   aiPlanSetFlag(planID, cPlanFlagNoUnitNotification, oldValue);
   xsDisableRule("addFeiBeastsToPlanMonitor");
}

//==============================================================================
// getValidForestProtectionTarget
//==============================================================================
int getValidForestProtectionTarget(bool output = false)
{
   int townCenterQueryID = useSimpleUnitQuery(cUnitTypeAbstractSocketedTownCenter);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(townCenterQueryID, gLandAreaGroupID, cPassabilityLand);
   }
   int numResults = kbUnitQueryExecute(townCenterQueryID);
   int[] results = kbUnitQueryGetResults(townCenterQueryID);
   for (int i = 0; i < numResults; i++)
   {
      int tcID = results[i];
      if (kbUnitGetStatFloat(tcID, cUnitStatHPRatio) < 1.0)
      {
         if (output == true)
         {
            debugGodPowers("Skipping tcID: " + tcID + " because it's damaged.");
         }
         continue;
      }
      if (kbUnitGetStatFlag(tcID, cUnitFlagSpreadsForestProtection) == true)
      {
         if (output == true)
         {
            debugGodPowers("Skipping tcID: " + tcID + " because it's already affected by Forest Protection.");
         }
         continue;
      }
      return tcID;
   }

   int fortressQueryID = useSimpleUnitQuery(cUnitTypeAbstractFortress);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(fortressQueryID, gLandAreaGroupID, cPassabilityLand);
   }
   numResults = kbUnitQueryExecute(fortressQueryID);
   results = kbUnitQueryGetResults(fortressQueryID);
   for (int i = 0; i < numResults; i++)
   {
      int fortressID = results[i];
      if (kbUnitGetStatFloat(fortressID, cUnitStatHPRatio) < 1.0)
      {
         if (output == true)
         {
            debugGodPowers("Skipping fortressID: " + fortressID + " because it's damaged.");
         }
         continue;
      }
      if (kbUnitGetStatFlag(fortressID, cUnitFlagSpreadsForestProtection) == true)
      {
         if (output == true)
         {
            debugGodPowers("Skipping fortressID: " + fortressID + " because it's already affected by Forest Protection.");
         }
         continue;
      }
      return fortressID;
   }
   return -1;
}

//==============================================================================
// forestProtectionMonitor
//==============================================================================
rule forestProtectionMonitor
inactive
minInterval 15
{
   debugGodPowers("--- Running Rule forestProtectionMonitor. ---");
   
   int targetID = getValidForestProtectionTarget(true);
   if (targetID == -1)
   {
      debugGodPowers("Found no valid target for Forest Protection.");
      return;
   }
   debugGodPowers("Casting Forest Protection on " + kbProtoUnitGetName(kbUnitGetProtoUnitID(targetID)) + "(" + targetID + ").");
   aiPlanSetVariableBool(gForestProtectionPlanID, cGodPowerPlanAutoCast, 0, true);
   aiPlanSetVariableInt(gForestProtectionPlanID, cGodPowerPlanTargetUnit, 0, targetID);
   xsDisableRule("forestProtectionMonitor");
}

//==============================================================================
// droughtLandMonitor
//==============================================================================
rule droughtLandMonitor
inactive
minInterval 5
{
   debugGodPowers("--- Running Rule droughtLandMonitor. ---");
   
   int[] plans = getValidPlansForGodpowerCasting(cIncludeAttackPlans, cExcludeDefendPlans, cIncludeExplorePlans, cExcludeNavalPlans);
   int numPlans = plans.size();
   if (numPlans <= 0)
   {
      debugGodPowers("Found 0 attack/explore plans in attack state to analyze.");
      return;
   }
   
   for (int i = 0; i < numPlans; i++)
   {
      int planID = plans[i];
      vector planLocation = aiPlanGetLocation(plans[i]);
      int tcID = getUnitByLocation(cUnitTypeAbstractSocketedTownCenter, cPlayerRelationEnemyNotGaia, cUnitStateABQ, planLocation,
         20.0, cUnitQueryVisibleStateVisible);
      if (tcID == -1)
      {
         debugGodPowers(aiPlanGetName(planID) + " has no visible close enemy TC, skipping.");
         continue;
      }

      debugGodPowers("Casting Drought Land on " + tcID + ", for plan " + aiPlanGetName(planID) + ".");
      aiPlanSetVariableBool(gDroughtLandPlanID, cGodPowerPlanAutoCast, 0, true);
      aiPlanSetVariableVector(gDroughtLandPlanID, cGodPowerPlanTargetLocation, 0, kbUnitGetPosition(tcID));
      xsDisableRule("droughtLandMonitor");
      return;
   }
}

//==============================================================================
// greatFloodMonitor
//==============================================================================
rule greatFloodMonitor
inactive
minInterval 5
{
   debugGodPowers("--- Running Rule greatFloodMonitor. ---");

   static int reservePlanID = -1;
   if (aiPlanGetIsIDValid(reservePlanID) == false)
   {
      reservePlanID = aiPlanCreate("Great Flood reserve plan", cPlanReserve, -1, gGodpowersCategoryID);
      aiPlanSetPriority(reservePlanID, 100);
      aiPlanSetFlag(reservePlanID, cPlanFlagNoMoreUnits, true); // Prevent auto assignment.
   }

   static int currentState = cGPStateBegin;
   static int scoutID = -1;
   static int targetID = -1;
   static int iterator = 0; // Used to only send a move command/debug output every 5 iterations.
   bool castGodPower = false;

   switch (currentState)
   {
      case cGPStateBegin:
      {
         bool foundTC = godPowerFindTCInRangeAndScout(gGreatFloodPlanID, scoutID, targetID, castGodPower);
         // TC can already be in range, then we're already done!
         if (castGodPower == true)
         {
            // Great Flood requires 2 locations, do that now.
            // We find the closest unit to the TC, step a little bit from that position to the TC.
            // We don't fully avoid our own army with it, but at least try to.
            vector targetLocation = kbUnitGetPosition(targetID);
            aiPlanSetVariableVector(gGreatFloodPlanID, cGodPowerPlanTargetLocation, 1, targetLocation);
            int closestUnitID = getClosestUnitByLocation(cUnitTypeLogicalTypeLandMilitary, cMyID, cUnitStateAlive,
               targetLocation, 30.0);
            vector startLocation = cInvalidVector;
            if (closestUnitID != -1)
            {
               vector unitPosition = kbUnitGetPosition(closestUnitID);
               // Due to the TC obstruction it's unlikely we now get the following: unit close -> cast behind tc -> hit own army.
               startLocation = unitPosition + (xsVectorNormalize(targetLocation - unitPosition) * 5);
            }
            else // Failsafe.
            {
               startLocation = targetLocation + (vector(0.1, 0.0, 0.1));
            }
            aiPlanSetVariableVector(gGreatFloodPlanID, cGodPowerPlanTargetLocation, 0, startLocation);

            xsSetRuleMinInterval("greatFloodMonitor", 5);
            currentState = cGPStateCleanup;
            return;
         }
         if (foundTC == true)
         {
            currentState = cGPStatePathingToLocation;
            // Need to keep checking godPowerExploreTargetPosition often.
            xsSetRuleMinInterval("greatFloodMonitor", 1);
            // Force another run asap now that we have a favourable setup.
            xsRuleIgnoreIntervalOnce("greatFloodMonitor");
         }
         break;
      }

      case cGPStatePathingToLocation:
      {
         bool pathingToLocation = godPowerExploreTargetPosition(gGreatFloodPlanID, scoutID, targetID, iterator, reservePlanID, castGodPower);
         if (castGodPower == true)
         {
            // Great Flood requires 2 locations, do that now.
            // We find the closest unit to the TC, step a little bit from that position to the TC.
            // We don't fully avoid our own army with it, but at least try to.
            vector targetLocation = kbUnitGetPosition(targetID);
            aiPlanSetVariableVector(gGreatFloodPlanID, cGodPowerPlanTargetLocation, 1, targetLocation);
            int closestUnitID = getClosestUnitByLocation(cUnitTypeLogicalTypeLandMilitary, cMyID, cUnitStateAlive, targetLocation, 50.0);
            vector startLocation = cInvalidVector;
            if (closestUnitID != -1)
            {
               vector unitPosition = kbUnitGetPosition(closestUnitID);
               // Due to the TC obstruction it's unlikely we now get the following: unit close -> cast behind tc -> hit own army.
               startLocation = unitPosition + (xsVectorNormalize(targetLocation - unitPosition) * 5);
            }
            else // Failsafe.
            {
               startLocation = targetLocation + (vector(0.1, 0.0, 0.1));
            }
            aiPlanSetVariableVector(gGreatFloodPlanID, cGodPowerPlanTargetLocation, 0, startLocation);

            // Give some time before we do the cleanup, so the scout keeps close to the target for vision.
            xsSetRuleMinInterval("greatFloodMonitor", 5);
            currentState = cGPStateCleanup;
            return;
         }
         if (pathingToLocation == false)
         {
            iterator = 0;
            currentState = cGPStateBegin;
            xsSetRuleMinInterval("greatFloodMonitor", 5);
         }
         break;
      }

      case cGPStateCleanup:
      {
         if (aiPlanGetIsIDValid(reservePlanID) == true)
         {
            aiPlanDestroy(reservePlanID);
         }
         currentState = cGPStateBegin;
         iterator = 0;
         xsDisableRule("greatFloodMonitor");
         break;
      }
   }
}