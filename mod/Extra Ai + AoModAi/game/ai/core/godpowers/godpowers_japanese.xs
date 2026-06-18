//==============================================================================
/* godpowers_japanese.xs

   This file contains all logic for the Japanese god powers.
*/
//==============================================================================

extern int gSolarShieldPlanID = -1;
extern int gKusanagiPlanID = -1;
extern int gNewMoonPlanID = -1;
extern const float gNewMoonMinimumValue = 400.0;
extern int gGoshinbokuPlanID = -1;
extern int gShrineOfTheHuntPlanID = -1;
extern int gShogunatePlanID = -1;
extern int gSmitingGustPlanID = -1;
extern int gDivineSlashPlanID = -1;
extern int gDragonTyphoonPlanID = -1;
extern int gSacredGatePlanID = -1;

//==============================================================================
// setupJapaneseGodPowerPlan
//==============================================================================
void setupJapaneseGodPowerPlan(int planID = -1, int protoPowerID = -1)
{
   aiPlanSetVariableInt(planID, cPlanGodPower, 0, protoPowerID);

   switch (protoPowerID)
   {
      case cProtoPowerSolarShield:
      {
         gSolarShieldPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelUnit);
         xsEnableRule("solarShieldMonitor");
         xsRuleIgnoreIntervalOnce("solarShieldMonitor");
         break;
      }

      case cProtoPowerKusanagi:
      {
         gKusanagiPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationManually);
         xsEnableRule("kusanagiMonitor");
         xsRuleIgnoreIntervalOnce("kusanagiMonitor");
         break;
      }

      case cProtoPowerNewMoon:
      {
         gNewMoonPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelUnit);
         // We don't use this in Deathmatch since we don't use the training boost.
         if (cGameModeCurrent != cGameModeDeathmatch)
         {
            xsEnableRule("newMoonMonitor");
            xsRuleIgnoreIntervalOnce("newMoonMonitor");
         }
         break;
      }

      case cProtoPowerGoshinboku:
      {
         gGoshinbokuPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelBuildingPlacement);
         xsEnableRule("goshinbokuMonitor");
         xsRuleIgnoreIntervalOnce("goshinbokuMonitor");
         break;
      }

      case cProtoPowerSwampland:
      {
         int queryID = kbUnitQueryCreate(aiPlanGetName(planID) + " Swampland Query");
         kbUnitQuerySetPlayerRelation(queryID, cPlayerRelationEnemyNotGaia, false);
         kbUnitQuerySetUnitType(queryID, cUnitTypeHumanSoldier);
         kbUnitQuerySetMaximumDistance(queryID, 20.0);
         kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateVisible);
         kbUnitQuerySetState(queryID, cUnitStateAlive);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryMinimumCount, 0, 10);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryID, 0, queryID);
         
         aiPlanSetVariableBool(planID, cGodPowerPlanRequiresCombatPlan, 0, true);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelCombatPlanDistance);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocation);
         break;
      }

      case cProtoPowerShrineOfTheHunt:
      {
         gShrineOfTheHuntPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelUnit);
         xsEnableRule("shrineOfTheHuntMonitor");
         xsRuleIgnoreIntervalOnce("shrineOfTheHuntMonitor");
         break;
      }

      case cProtoPowerShogun:
      {
         gShogunatePlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelUnit);
         xsEnableRule("shogunateMonitor");
         xsRuleIgnoreIntervalOnce("shogunateMonitor");
         break;
      }

      case cProtoPowerSmitingGust:
      {
         gDivineSlashPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationDual);
         xsEnableRule("divineSlashMonitor");
         xsRuleIgnoreIntervalOnce("divineSlashMonitor");
         break;
      }

      case cProtoPowerThunderBurst:
      {
         int queryID = kbUnitQueryCreate(aiPlanGetName(planID) + " Thunder Burst Query");
         kbUnitQuerySetPlayerRelation(queryID, cPlayerRelationEnemyNotGaia, false);
         kbUnitQuerySetUnitType(queryID, cUnitTypeMilitaryUnit);
         kbUnitQuerySetMaximumDistance(queryID, 20.0);
         kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateVisible);
         kbUnitQuerySetState(queryID, cUnitStateAlive);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryMinimumCount, 0, 10);
         aiPlanSetVariableInt(planID, cGodPowerPlanQueryID, 0, queryID);
         
         aiPlanSetVariableBool(planID, cGodPowerPlanRequiresCombatPlan, 0, true);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelCombatPlanDistance);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocation);
         break;
      }

      case cProtoPowerDivineSlash:
      {
         gDivineSlashPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationDual);
         xsEnableRule("divineSlashMonitor");
         xsRuleIgnoreIntervalOnce("divineSlashMonitor");
         break;
      }

      case cProtoPowerDragonTyphoon:
      {
         gDragonTyphoonPlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelLocationDual);
         xsEnableRule("dragonTyphoonMonitor");
         xsRuleIgnoreIntervalOnce("dragonTyphoonMonitor");
         break;
      }

      case cProtoPowerSacredGate:
      {
         gSacredGatePlanID = planID;
         aiPlanSetVariableBool(planID, cGodPowerPlanAutoCast, 0, false);
         aiPlanSetVariableInt(planID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(planID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelBuildingPlacement);
         xsEnableRule("sacredGateMonitor");
         xsRuleIgnoreIntervalOnce("sacredGateMonitor");
         break;
      }
   }
}

//==============================================================================
// solarShieldMonitor
// Cast Solar Shield if we're in combat and have a high value unit to protect that's being attacked.
//==============================================================================
rule solarShieldMonitor
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule solarShieldMonitor. ---");

   int[] plans = getValidPlansForGodpowerCasting(cIncludeAttackPlans, cIncludeDefendPlans, cIncludeExplorePlans, cExcludeNavalPlans);
   int numPlans = plans.size();
   if (numPlans <= 0)
   {
      debugGodPowers("Found 0 attack/defend/explore plans in attack state to analyze.");
      return;
   }

   int[] excludeTypes = new int(1, cUnitTypeHumanSoldier);
   int targetUnitID = -1;
   int highestAmountWorkers = 0;
   for (int i = 0; i < plans.size(); i++)
   {
      if (aiPlanGetUserVariableIndex(plans[i], "Titan Plan") != -1)
      {
         continue; // Don't analyze Titan attack plans, we can't cast on Titans.
      }

      // Only fetch high value units.
      int[] units = aiPlanGetUnits(plans[i], -1, false, excludeTypes);
      for (int iUnit = 0; iUnit < units.size(); iUnit++)
      {
         int numWorkersOnUnit = kbUnitGetNumberWorkers(units[iUnit]);
         int numWorkers = 0;
         for (int iWorker = 0; iWorker < numWorkersOnUnit; iWorker++)
         {
            int workerID = kbUnitGetWorkerID(units[iUnit], iWorker);
            if (kbUnitGetIsIDValid(workerID) == false)
            {
               continue;
            }
            int playerID = kbUnitGetPlayerID(workerID);
            if (playerID != cMyID && kbPlayerIsEnemy(kbUnitGetPlayerID(workerID)) == true)
            {
               numWorkers++;
            }
         }
         if (numWorkers > 2 && numWorkers > highestAmountWorkers)
         {
            highestAmountWorkers = numWorkers;
            targetUnitID = units[iUnit];
            debugGodPowers("   " + kbProtoUnitGetName(kbUnitGetProtoUnitID(targetUnitID)) + "(" + targetUnitID +
               ") is now our best target with " + highestAmountWorkers + " workers.");
         }
      }
   }
   if (targetUnitID != -1)
   {
      debugGodPowers("Casting Solar Shield on: " + kbProtoUnitGetName(kbUnitGetProtoUnitID(targetUnitID)) + "(" + targetUnitID + ".");
      aiPlanSetVariableInt(gSolarShieldPlanID, cGodPowerPlanTargetUnit, 0, targetUnitID);
      aiPlanSetVariableBool(gSolarShieldPlanID, cGodPowerPlanAutoCast, 0, true);
      xsDisableRule("solarShieldMonitor");
   }
   else
   {
      debugGodPowers("Didn't find a suitable target to cast Solar Shield on.");
   }
}

//==============================================================================
// kusanagiMonitor
// Try to find some enemy military and randomly Kusanagi one of them, pray it hits more.
//==============================================================================
rule kusanagiMonitor
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule kusanagiMonitor. ---");

   int[] plans = getValidPlansForGodpowerCasting(cIncludeAttackPlans, cIncludeDefendPlans, cIncludeExplorePlans, cIncludeNavalPlans);
   int numPlans = plans.size();
   if (numPlans <= 0)
   {
      debugGodPowers("Found 0 attack/defend/explore plans in attack state to analyze.");
      return;
   }

   for (int i = 0; i < plans.size(); i++)
   {
      int queryID = useSimpleUnitQuery(cUnitTypeMilitaryUnit, cPlayerRelationEnemyNotGaia, cUnitStateAlive,
         aiPlanGetLocation(plans[i]), 15.0);
      kbUnitQuerySetAscendingSort(queryID, true);
      kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateVisible);
      kbUnitQueryExecute(queryID);
      int[] enemies = kbUnitQueryGetResults(queryID);
      if (enemies.size() < 6)
      {
         debugGodPowers("We didn't find enough enemies to cast Kusanagi for this plan: " + aiPlanGetName(plans[i]) + ".");
         continue;
      }
      debugGodPowers("We found enough enemies near plan: " + aiPlanGetName(plans[i]) + " to cast Kusanagi!");
      // Take closest unit to cast on.
      aiPlanSetVariableVector(gKusanagiPlanID, cGodPowerPlanTargetLocation, 0, kbUnitGetTruePosition(enemies[0]));
      aiPlanSetVariableBool(gKusanagiPlanID, cGodPowerPlanAutoCast, 0, true);
      xsDisableRule("kusanagiMonitor");
      return;
   }
}

//==============================================================================
// newMoonMonitor
//==============================================================================
int getHighestValueNewMoonPUID(ref float totalValue)
{
   debugGodPowers("Running getHighestValueNewMoonPUID function.");
   int[] obtainableTechnologies = kbTechTreeGetAllObtainableTechnologies(true); // Only with researchers since we need to cast it.
   float[] costTechnologiesInBuilding = new float(0, 0);
   int[] researchBuildings = new int(0, 0);
   for (int i = 0; i < obtainableTechnologies.size(); i++)
   {
      int techID = obtainableTechnologies[i];
      float techCost = kbAICostGetTechCost(techID);
      debugGodPowers("Analyzing " + kbTechGetName(techID) + " with AI cost: " + techCost + ".");
      int[] researchers = kbTechTreeGetResearchers(techID);
      for (int iResearcher = 0; iResearcher < researchers.size(); iResearcher++)
      {
         // Some techs have multiple researchers, we must have 1 of them alive but not per se all.
         if (kbUnitCount(researchers[iResearcher], cMyID, cUnitStateAlive) == 0)
         {
            continue;
         }
         // We don't need to instantly get max upgraded Towers once we hit Heroic, unless we're a defender.
         if (cPersonalityCurrent != cPersonalityDefender && researchers[iResearcher] == cUnitTypeSentryTower)
         {
            continue;
         }
         int index = researchBuildings.find(researchers[iResearcher]);
         if (index == -1)
         {
            researchBuildings.add(researchers[iResearcher]);
            costTechnologiesInBuilding.add(techCost);
         }
         else
         {
            costTechnologiesInBuilding[index] = costTechnologiesInBuilding[index] + techCost;
         }
      }
   }
   if (costTechnologiesInBuilding.size() != researchBuildings.size())
   {
      aiEchoWarning("newMoonMonitor - logic is wrong for the arrays.");
   }
   if (researchBuildings.size() == 0)
   {
      debugGodPowers("Couldn't find a valid PUID for New Moon.");
      return gNewMoonMinimumValue - 10000;
   }
   int bestIndex = -1;
   float highestCost = cMinFloat;
   for (int i = 0; i < researchBuildings.size(); i++)
   {
      if (researchBuildings[i] == cUnitTypeTownCenter && (xsRandBool() == true || cPersonalityCurrent == cPersonalityAttacker))
      {
         costTechnologiesInBuilding[i] = 0.0;
         debugGodPowers("Randomly setting (or we are an attacker) our Town Center worth to 0.0 so that we don't every game " +
            "cast it on the TC.");
      }
      debugGodPowers("   " + kbProtoUnitGetName(researchBuildings[i]) + " has " + costTechnologiesInBuilding[i] +
         " cost worth of technologies.");
      if (costTechnologiesInBuilding[i] > highestCost)
      {
         highestCost = costTechnologiesInBuilding[i];
         bestIndex = i;
      }
   }
   totalValue = highestCost;
   return researchBuildings[bestIndex];
}

//==============================================================================
// newMoonMonitor
// Try to find a building that has a lot of techs in it.
// We don't use the training boost for now, TODO.
//==============================================================================
rule newMoonMonitor
inactive
minInterval 30
{
   // We get so much value from Heroic onwards it's better to wait.
   // If we're an attacker we might just want to get all Armory upgrades instantly etc.
   if (cPersonalityCurrent != cPersonalityAttacker && kbPlayerGetAge(cMyID) < cAge3)
   {
      return;
   }
   debugGodPowers("--- Running Rule newMoonMonitor. ---");

   float totalValue = 0.0;
   int buildingPUID = getHighestValueNewMoonPUID(totalValue);
   if (totalValue < gNewMoonMinimumValue)
   {
      debugGodPowers("Best value is " +  kbProtoUnitGetName(buildingPUID) + " with " + totalValue +
         ", however this is below the " + gNewMoonMinimumValue + " minimum, won't cast New Moon now.");
   }
   else
   {
      debugGodPowers("Casting New Moon on a " + kbProtoUnitGetName(buildingPUID) + ".");
      int unitID = getUnit(buildingPUID); // Should work since we requested alive researchers.
      aiPlanSetVariableInt(gNewMoonPlanID, cGodPowerPlanTargetUnit, 0, unitID);
      aiPlanSetVariableBool(gNewMoonPlanID, cGodPowerPlanAutoCast, 0, true);
      xsDisableRule("newMoonMonitor");
   }
}

//==============================================================================
// goshinbokuMonitor
// Place a Goshinboku in the most defended TC base.
//==============================================================================
rule goshinbokuMonitor
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule goshinbokuMonitor. ---");

   int baseID = getMostDefendedTCBase();
   if (baseID == -1)
   {
      debugGodPowers("Can't cast Goshinboku atm because we have no Town Center base.");
      return;
   }
   vector basePosition = kbBaseGetLocation(cMyID, baseID);
   if (getUnitCountByLocation(cUnitTypeMilitaryUnit, cPlayerRelationEnemyNotGaia, cUnitStateAlive, basePosition,
         kbBaseGetDistance(cMyID, baseID)) > 0)
   {
      debugGodPowers("Can't cast Goshinboku atm because we see enemies in base " + kbBaseGetNameByID(cMyID, baseID) + ".");
      return;
   }

   int bpID = kbBuildingPlacementCreate(aiPlanGetName(gGoshinbokuPlanID) + " Placement");
   kbBuildingPlacementSetBuildingPUID(bpID, cUnitTypeGoshinboku);
   addSafeBackAreasToBuildingPlacement(bpID, baseID, gGodpowersCategoryID);
   kbBuildingPlacementSetRequiresCompletelyUnobstructed(bpID, true);
   // Risky cuz it can block builders but this thing is hard enough to place as it is.
   kbBuildingPlacementSetBufferSpace(bpID, 1.0);
   aiPlanSetVariableInt(gGoshinbokuPlanID, cGodPowerPlanBPID, 0, bpID);
   aiPlanSetVariableBool(gGoshinbokuPlanID, cGodPowerPlanAutoCast, 0, true);
   xsDisableRule("goshinbokuMonitor");
}

//==============================================================================
// shrineOfTheHuntMonitor
//==============================================================================
int findShrineForShrineOfTheHunt()
{
   debugGodPowers("Running findShrineForShrineOfTheHunt function.");
   int shrineQueryID = useSimpleUnitQuery(cUnitTypeShrineJapanese);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(shrineQueryID, gLandAreaGroupID, cPassabilityLand);
   }
   int numShrineResults = kbUnitQueryExecute(shrineQueryID);
   for (int i = 0; i < numShrineResults; i++)
   {
      int shrineID = kbUnitQueryGetResult(shrineQueryID, i);
      int baseID = kbUnitGetBaseID(shrineID);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         debugGodPowers("   Skipping Shrine(" + shrineID + ") because it's no longer in a TC base, expect it's too dangerous.");
         continue;
      }

      int numWorkers = kbUnitGetNumberWorkers(shrineID);
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
            foundWorkingMiko = true;
            break;
         }
      }
      if (foundWorkingMiko == false)
      {
         debugGodPowers("   Shrine(" + shrineID + ") has no Miko working on it, won't consider it.");
         continue;
      }

      debugGodPowers("   Found Shrine(" + shrineID + ") to potentially cast Shrine of the Hunt on.");
      return shrineID;
   }
   return -1;
}

//==============================================================================
// shrineOfTheHuntMonitor
//==============================================================================
rule shrineOfTheHuntMonitor
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule shrineOfTheHuntMonitor. ---");

   int shrineID = findShrineForShrineOfTheHunt();
   if (shrineID == -1)
   {
      debugGodPowers("Didn't find a suitable Shrine at this time.");
      return;
   }

   debugGodPowers("Casting Shrine of the Hunt on Shrine(" + shrineID + ").");
   aiPlanSetVariableInt(gShrineOfTheHuntPlanID, cGodPowerPlanTargetUnit, 0, shrineID);
   aiPlanSetVariableBool(gShrineOfTheHuntPlanID, cGodPowerPlanAutoCast, 0, true);
   xsDisableRule("shrineOfTheHuntMonitor");
}

//==============================================================================
// shogunateMonitor
// Try to find a unit in the defend plan that has a high cost to heroize.
//==============================================================================
rule shogunateMonitor
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule shogunateMonitor. ---");

   int[] units = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypeDaimyo);
   for (int i = 0; i < units.size(); i++)
   {
      int unitID = units[i];
      if (kbUnitGetStatFloat(unitID, cUnitStatHPRatio) < 0.95)
      {
         continue;
      }
      debugGodPowers("Casting Shogunate on Daimyo(" + unitID + ").");
      aiPlanSetVariableInt(gShogunatePlanID, cGodPowerPlanTargetUnit, 0, unitID);
      aiPlanSetVariableBool(gShogunatePlanID, cGodPowerPlanAutoCast, 0, true);
      xsDisableRule("shogunateMonitor");
      return;
   }
}

//==============================================================================
// divineSlashMonitor
// Find 2 enemy positions and slash away.
//==============================================================================
rule divineSlashMonitor
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule divineSlashMonitor. ---");

   int[] plans = getValidPlansForGodpowerCasting(cIncludeAttackPlans, cIncludeDefendPlans, cIncludeExplorePlans, cIncludeNavalPlans);
   int numPlans = plans.size();
   if (numPlans <= 0)
   {
      debugGodPowers("Found 0 attack/defend/explore plans in attack state to analyze.");
      return;
   }

   for (int i = 0; i < plans.size(); i++)
   {
      int queryID = useSimpleUnitQuery(cUnitTypeMilitaryUnit, cPlayerRelationEnemyNotGaia, cUnitStateAlive,
         aiPlanGetLocation(plans[i]), 30.0);
      kbUnitQuerySetAscendingSort(queryID, true);
      kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateVisible);
      int numEnemies = kbUnitQueryExecute(queryID);
      int[] enemies = kbUnitQueryGetResults(queryID);
      if (numEnemies < 12)
      {
         debugGodPowers("We didn't find enough enemies to cast Divine Slash for this plan: " + aiPlanGetName(plans[i]) + ".");
         continue;
      }
      debugGodPowers("We found enough enemies near plan: " + aiPlanGetName(plans[i]) + " to cast Divine Slash!");
      // Take closest and furthest away units to cast on.
      aiPlanSetVariableVector(gDivineSlashPlanID, cGodPowerPlanTargetLocation, 0, kbUnitGetTruePosition(enemies[0]));
      aiPlanSetVariableVector(gDivineSlashPlanID, cGodPowerPlanTargetLocation, 1, kbUnitGetTruePosition(enemies[numEnemies - 1]));
      aiPlanSetVariableBool(gDivineSlashPlanID, cGodPowerPlanAutoCast, 0, true);
      xsDisableRule("divineSlashMonitor");
      return;
   }
}

//==============================================================================
// dragonTyphoonMonitor
//==============================================================================
rule dragonTyphoonMonitor
inactive
minInterval 5
{
   debugGodPowers("--- Running Rule dragonTyphoonMonitor. ---");

   static int reservePlanID = -1;
   if (aiPlanGetIsIDValid(reservePlanID) == false)
   {
      reservePlanID = aiPlanCreate("Dragon Typhoon reserve plan", cPlanReserve, -1, gGodpowersCategoryID);
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
         bool foundTC = godPowerFindTCInRangeAndScout(gDragonTyphoonPlanID, scoutID, targetID, castGodPower);
         // TC can already be in range, then we're already done!
         if (castGodPower == true)
         {
            // Dragon Typhoon requires 2 locations, do that now.
            // We find the closest unit to the TC, step a little bit from that position to the TC.
            // We don't fully avoid our own army with it, but at least try to.
            vector targetLocation = kbUnitGetPosition(targetID);
            aiPlanSetVariableVector(gDragonTyphoonPlanID, cGodPowerPlanTargetLocation, 1, targetLocation);
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
            aiPlanSetVariableVector(gDragonTyphoonPlanID, cGodPowerPlanTargetLocation, 0, startLocation);

            xsSetRuleMinInterval("dragonTyphoonMonitor", 5);
            currentState = cGPStateCleanup;
            return;
         }
         if (foundTC == true)
         {
            currentState = cGPStatePathingToLocation;
            // Need to keep checking godPowerExploreTargetPosition often.
            xsSetRuleMinInterval("dragonTyphoonMonitor", 1);
            // Force another run asap now that we have a favourable setup.
            xsRuleIgnoreIntervalOnce("dragonTyphoonMonitor");
         }
         break;
      }

      case cGPStatePathingToLocation:
      {
         bool pathingToLocation = godPowerExploreTargetPosition(gDragonTyphoonPlanID, scoutID, targetID, iterator, reservePlanID, castGodPower);
         if (castGodPower == true)
         {
            // Great Flood requires 2 locations, do that now.
            // We find the closest unit to the TC, step a little bit from that position to the TC.
            // We don't fully avoid our own army with it, but at least try to.
            vector targetLocation = kbUnitGetPosition(targetID);
            aiPlanSetVariableVector(gDragonTyphoonPlanID, cGodPowerPlanTargetLocation, 1, targetLocation);
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
            aiPlanSetVariableVector(gDragonTyphoonPlanID, cGodPowerPlanTargetLocation, 0, startLocation);

            // Give some time before we do the cleanup, so the scout keeps close to the target for vision.
            xsSetRuleMinInterval("dragonTyphoonMonitor", 5);
            currentState = cGPStateCleanup;
            return;
         }
         if (pathingToLocation == false)
         {
            iterator = 0;
            currentState = cGPStateBegin;
            xsSetRuleMinInterval("dragonTyphoonMonitor", 5);
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
         xsDisableRule("dragonTyphoonMonitor");
         break;
      }
   }
}

//==============================================================================
// sacredGateMonitor
// Place a Goshinboku in the most defended TC base.
//==============================================================================
rule sacredGateMonitor
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule sacredGateMonitor. ---");

   int baseID = getMostDefendedTCBase();
   if (baseID == -1)
   {
      debugGodPowers("Can't cast Sacred Gate atm because we have no Town Center base.");
      return;
   }
   vector basePosition = kbBaseGetLocation(cMyID, baseID);
   if (getUnitCountByLocation(cUnitTypeMilitaryUnit, cPlayerRelationEnemyNotGaia, cUnitStateAlive, basePosition,
         kbBaseGetDistance(cMyID, baseID)) > 0)
   {
      debugGodPowers("Can't cast Sacred Gate atm because we see enemies in base " + kbBaseGetNameByID(cMyID, baseID) + ".");
      return;
   }

   int bpID = kbBuildingPlacementCreate(aiPlanGetName(gSacredGatePlanID) + " Placement");
   kbBuildingPlacementSetBuildingPUID(bpID, cUnitTypeSacredGate);
   addSafeBackAreasToBuildingPlacement(bpID, baseID, gGodpowersCategoryID);
   kbBuildingPlacementSetRequiresCompletelyUnobstructed(bpID, true);
   // Risky cuz it can block builders but this thing is hard enough to place as it is.
   kbBuildingPlacementSetBufferSpace(bpID, 1.0);
   aiPlanSetVariableInt(gSacredGatePlanID, cGodPowerPlanBPID, 0, bpID);
   aiPlanSetVariableBool(gSacredGatePlanID, cGodPowerPlanAutoCast, 0, true);
   xsDisableRule("sacredGateMonitor");
}