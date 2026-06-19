//==============================================================================
/* buildings.xs

   This file is intended for managing what buildings the AI should create and when.

*/
//==============================================================================

// Greeks/Japanese have 3 different military buildings, * 2 = 6 total.
#if (cMyCulture == cCultureGreek || cMyCulture == cCultureJapanese)
const int cMaxOfRegularBuildingInOneBase = 2;
#else
// All others have 2 military buildings * 3 = 6 total.
const int cMaxOfRegularBuildingInOneBase = 3;
#endif
// We allow 2 Fortress per base, this is a rare occassion since we also expand other bases with one automatically.
const int cMaxOfFortressBuildingInOneBase = 2;
// Limit amount of concurrent build plans that still require resources, otherwise our wood need sky rockets out of control.
const int cMaxBuildPlansActive = 2;
// Keep track of the start time where the militaryBuildingManager didn't want any more buildings.
// This is then used to, after a delay, start expanding regardless.
int startTimeNoExtraBuildingsNeeded = cMaxInt;
// === AoModAi: layered walls state begin ===
extern bool mRusher = false;
extern int  gSecondRingWallPlanID = -1;
extern int  gSecondRingWallStartTime = -1;
extern int  gSecondRingWallLastDestroyedTime = -1;
extern int  gSecondRingAttackStartTime = -1;
extern bool gDebugSecondRing = true;
// === AoModAi: layered walls state end ===
//==============================================================================
// === AoModAi: layered walls begin ===
//==============================================================================
// secondRingWallInit
//==============================================================================
rule secondRingWallInit
inactive
group defaultArchaicRules
minInterval 1
{
   mRusher = (cPersonalityCurrent == cPersonalityAttacker);
   xsDisableRule("secondRingWallInit");
}

//==============================================================================
// militaryBuildingManager 
//==============================================================================
rule militaryBuildingManager
inactive
group defaultClassicalRules
minInterval 10
{
   if (checkStrategyFlag(cStrategyFlagAutoBuildMilitaryBuildings) == false)
   {
      return;
   }
   debugMilitaryBuildings("--- Running Rule militaryBuildingManager. ---");

   int totalNumberBuildPlansMade = 0;
   int totalBuildPlansActive = 0;

   // We scan here for all build plans in the gMilitaryBuildings array. This includes Fortress + Temple too while we also build those
   // outside of this rule. This means that those buildings get prio over regular military buildings since they don't cap themselves.
   int numberMilitaryBuildings = gMilitaryBuildings.size();
   for (int i = 0; i < numberMilitaryBuildings; i++)
   {
      int buildingPUID = gMilitaryBuildings[i];
      if (kbProtoUnitAvailable(buildingPUID) == false)
      {
         continue;
      }
      int[] existingBuildPlans = aiPlanGetIDsByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, buildingPUID);
      for (int iPlan = 0; iPlan < existingBuildPlans.size(); iPlan++)
      {
         // Build plans that are in the none state still require resources to be assigned to them, and we want to limit resource needs.
         if (aiPlanGetState(existingBuildPlans[iPlan]) == cPlanStateNone)
         {
            totalBuildPlansActive++;
         }
         // If we're in state place we've already claimed the resources but the placement can fail, and then we would stack plans again.
         if (aiPlanGetState(existingBuildPlans[iPlan]) == cPlanStatePlace)
         {
            totalBuildPlansActive++;
         }
      }
      if (totalBuildPlansActive >= 2)
      {
         debugMilitaryBuildings("We already have 2 or more military build plans active, can't create any more.");
         return;
      }
   }

   // Go through all the buildings in our gMilitaryBuildings array.
   for (int i = 0; i < numberMilitaryBuildings; i++)
   {
      int buildingPUID = gMilitaryBuildings[i];
      if (kbProtoUnitAvailable(buildingPUID) == false)
      {
         continue;
      }

      // Limit how many of these we can make per base based on what kind of building it is.
      int maxOfBuildingInOneBase = cMaxOfRegularBuildingInOneBase;
      if (buildingPUID == gFortressUnit)
      {
         maxOfBuildingInOneBase = cMaxOfFortressBuildingInOneBase;
      }
      // Custom limit on Siege Works, you just don't need to spam these... Unless we're a Sieger!
      else if (buildingPUID == cUnitTypeSiegeWorks && cPersonalityCurrent != cPersonalitySieger)
      {
         maxOfBuildingInOneBase = 1;
      }

      int buildLimit = kbPlayerGetProtoStatInt(cMyID, buildingPUID, cProtoStatBuildLimit);
      int numberExistingBuildings = buildingGetNumberAliveAndPlanned(buildingPUID, false, gLandAreaGroupID, cPassabilityLand);
      if (numberExistingBuildings >= buildLimit && buildLimit != -1)
      {
         debugMilitaryBuildings("We are already at the build limit for: " + kbProtoUnitGetName(buildingPUID) + ".");
         continue;
      }
      
      float numberBuildingsReq = 0.0;
      bool inUse = false;

      // Go through all maintain plans that we have and see if they require the current building from the buildings array.
      for (int j = 0; j < gNumTotalArmyUnitTypes; j++)
      {
         // Check the array in which we store what building is needed to train the unit, if it's not the building we're analyzing, skip.
         if (buildingPUID != gArmyUnitBuildings[j])
         {
            continue;
         }
         int maintainPlanID = gArmyUnitMaintainPlans[j];
         if (aiPlanGetIsIDValid(maintainPlanID) == false)
         {
            continue;
         }
         int numberToMaintain = aiPlanGetVariableInt(maintainPlanID, cTrainPlanNumberToMaintain, 0);
         if (numberToMaintain < 1)
         {
            continue;
         }

         // How many buildings do we want?
         // We aim to have enough buildings to train all our wanted units in < 2 minutes.
         int puid = aiPlanGetVariableInt(maintainPlanID, cTrainPlanUnitType, 0);
         float numPerMinute = 60 / kbPlayerGetProtoStatFloat(cMyID, puid, cProtoStatTrainPoints);
         float minutesToGoal = numberToMaintain / numPerMinute;
         debugMilitaryBuildings("Maintain plan for " + kbProtoUnitGetName(puid) + " adds " + (minutesToGoal / 2.0) +
            " to our number buildings required.");
         numberBuildingsReq += minutesToGoal / 2.0;
         inUse = true;
      }

      if (inUse == false)
      {
         debugMilitaryBuildings("We have completely no need for " + kbProtoUnitGetName(buildingPUID) + ".");
         continue;
      }

      // Always round upwards so we're sure we have enough buildings.
      int numberTotalBuildingsWanted = ceil(numberBuildingsReq); 
      // If we have a low eco pop we don't want to build that many buildings, clamp to 1.
      int numberEcoPop = aiGetCurrentEconomyPop() + aiGetCurrentNavalEconomyPop();
      if (numberEcoPop < 20)
      {
         debugMilitaryBuildings("We have too few eco pop, capping how many buildings we can make of: " +
            kbProtoUnitGetName(buildingPUID) + " to 1.");
         numberTotalBuildingsWanted = 1;
      }
      debugMilitaryBuildings("We want: " + numberTotalBuildingsWanted + " total " + kbProtoUnitGetName(buildingPUID) +
         ", we already have " + numberExistingBuildings + " of them.");
      numberTotalBuildingsWanted -= numberExistingBuildings;

      if (buildLimit != -1 && (numberExistingBuildings + numberTotalBuildingsWanted) > buildLimit)
      {
         numberTotalBuildingsWanted = buildLimit - numberExistingBuildings;
         debugMilitaryBuildings("With the amount of buildings we wanted to make we would go over the build limit, capping at " +
            numberTotalBuildingsWanted + " buildings instead.");
      }
      if (totalBuildPlansActive + numberTotalBuildingsWanted > cMaxBuildPlansActive)
      {
         numberTotalBuildingsWanted = cMaxBuildPlansActive - totalBuildPlansActive;
         debugMilitaryBuildings("We want more new buildings than we allow max concurrent build plans, limiting number wanted to "
            + numberTotalBuildingsWanted + ".");
      }
      if (numberTotalBuildingsWanted <= 0)
      {
         continue;
      }

      // Find TC bases that we can use.
      int numBases = kbBaseGetNumber(cMyID);
      for (int iBase = 0; iBase < numBases; iBase++)
      {
         int baseID = kbBaseGetIDByIndex(cMyID, iBase);
         if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
         {
            continue;
         }
         if (gLandAreaGroupID != -1 &&
             kbPathAreAreaGroupsConnected(gLandAreaGroupID,
             kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityLand) == false)
         {
            continue;
         }
         int numBuildingsInBase = getCountOfOwnAliveBuildingInBase(buildingPUID, baseID); 
         numBuildingsInBase += getAmountBuildPlansInBase(buildingPUID, baseID);
         if (numBuildingsInBase >= maxOfBuildingInOneBase)
         {
            continue;
         }

         debugMilitaryBuildings(kbBaseGetNameByID(cMyID, baseID) + " already has : " + numBuildingsInBase + "/" +
            maxOfBuildingInOneBase + ".");
         int numBuildingsToMake = numberTotalBuildingsWanted;
         if (numBuildingsInBase + numBuildingsToMake > maxOfBuildingInOneBase)
         {
            numBuildingsToMake = maxOfBuildingInOneBase - numBuildingsInBase;
            debugMilitaryBuildings("Capping number buildings to make to " + numBuildingsToMake + " in this base because otherwise " +
               "we would go over our max in one base.");
         }

         debugMilitaryBuildings("Creating build plans for " + numBuildingsToMake + " " + kbProtoUnitGetName(buildingPUID) + ".");
         int prio = 70;
         // Don't have too high prio for a Fortress unit, it can block a lot of other expenses.
         if (buildingPUID == gFortressUnit)
         {
            prio = 50;
         }
         createSimpleBuildPlan(buildingPUID, numBuildingsToMake, prio, gMilitaryBuildingsCategoryID, baseID,
            cCalculateNumBuildersAutomatically);
         totalNumberBuildPlansMade += numBuildingsToMake;
         totalBuildPlansActive += numBuildingsToMake;
         numberTotalBuildingsWanted -= numBuildingsToMake;
         if (numberTotalBuildingsWanted == 0)
         {
            break;
         }
      }
      if (totalBuildPlansActive >= 2)
      {
         debugMilitaryBuildings("We already have 2 or more military build plans active, can't create any more.");
         break;
      }
   }
   if (totalNumberBuildPlansMade > 0)
   {
      startTimeNoExtraBuildingsNeeded = cMaxInt;
   }
   else
   {
      startTimeNoExtraBuildingsNeeded = xsGetTime();
   }
}

//==============================================================================
// fortressBuildingManager
// Even though our automatic military construction + base expansion also create Fortresses we still need this rule.
// Example: Egyptian military logic throws out all Migdol units if we have no Migdol -> auto military building construction
// sees no need for a Migdol -> base expansion is off for easy/moderate -> no Migdol is ever made -> stuck in Heroic. 
//==============================================================================
rule fortressBuildingManager
inactive
group defaultHeroicRules
minInterval 30
{
   if (checkStrategyFlag(cStrategyFlagAutoBuildMilitaryBuildings) == false)
   {
      return;
   }
   debugMilitaryBuildings("--- Running Rule fortressBuildingManager. ---");
   
   // We never want to have more than 1 Fortress build plan active at the same time, just so expensive.
   int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, gFortressUnit);
   int fortressCount = kbUnitCount(cUnitTypeAbstractFortress, cMyID, cUnitStateAlive);
   if (aiPlanGetIsIDValid(planID) == true)
   {
      // Make sure if we no Fortress alive that the priority is set correctly.
      if (fortressCount == 0)
      {
         aiPlanSetPriority(planID, 55);
      }
      debugMilitaryBuildings("Already have a " + kbProtoUnitGetName(gFortressUnit) + " build plan, quiting.");
      return;
   }

   // If we're here we know we aren't already building a Fortress.
   if (fortressCount == 0)
   {
      // If we currently own no Fortresses, place one in our most fortified TC base.
      int baseID = getMostDefendedTCBase();
      if (baseID == -1)
      {
         debugMilitaryBuildings("We have no TC bases, not building a " + kbProtoUnitGetName(gFortressUnit) + " now.");
      }
      else
      {
         // Get one back with some prio.
         createSimpleBuildPlan(gFortressUnit, 1, 55, gMilitaryBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      }
      return;
   }

   // We just want to build Fortresses as a defender, skip cost checks.
   if (cPersonalityCurrent != cPersonalityDefender)
   {
      // If we're here we already have 1 Fortress at least.
      // Even though these buildings are strong they are also a big drain on the eco.
      // So if we really need them the militaryBuildingManager will create them, otherwise we do it here when we can afford it.
      if (haveExcessResourceAmount(450.0, cResourceGold) == false)
      {
         debugMilitaryBuildings("We don't have enough excess gold to make more " + kbProtoUnitGetName(gFortressUnit) + ".");
         return;
      }
      if (cMyCulture != cCultureEgyptian && haveExcessResourceAmount(450.0, cResourceWood) == false)
      {
         debugMilitaryBuildings("We don't have enough excess wood to make more " + kbProtoUnitGetName(gFortressUnit) + ".");
         return;
      }
   }
   else
   {
      debugMilitaryBuildings("Skipping cost checks for building more Fortresses, we always want these as a Defender!");
   }

   int maxFortressPerBase = cPersonalityCurrent == cPersonalityDefender || cPersonalityCurrent == cPersonalityBuilder ? 2 : 1;
   debugMilitaryBuildings("We can make a maximum of " + maxFortressPerBase + " Fortresses per base via this rule.");
   bool madePlan = false;
   // Find TC bases that we can use.
   int numBases = kbBaseGetNumber(cMyID);
   for (int iBase = 0; iBase < numBases; iBase++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, iBase);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityLand) == false)
      {
         continue;
      }
      int numExistingFortressesInBase = getCountOfOwnAliveBuildingInBase(cUnitTypeAbstractFortress, baseID);
      if (numExistingFortressesInBase >= maxFortressPerBase)
      {
         continue;
      }
      // Not high prio because it's so expensive.
      createSimpleBuildPlan(gFortressUnit, 1, 50, gMilitaryBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      madePlan = true;
      break;
   }

   if (madePlan == false)
   {
      debugMilitaryBuildings("Didn't find a suitable TC base to make a " + kbProtoUnitGetName(gFortressUnit) + " in.");
   }
}

//==============================================================================
// baseBuildingExpansionMonitor 
//==============================================================================
rule baseBuildingExpansionMonitor
inactive
group defaultClassicalRules
minInterval 60
{
   // Builders have their own rule.
   if (cPersonalityCurrent == cPersonalityBuilder)
   {
      xsDisableRule("baseBuildingExpansionMonitor");
      return;
   }
   // Lower difficulties don't expand their base if they're not "builder" type personalities.
   if (cDifficultyCurrent <= cDifficultyModerate &&
       cPersonalityCurrent != cPersonalityConqueror && cPersonalityCurrent != cPersonalityDefender)
   {
      xsDisableRule("baseBuildingExpansionMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoBuildMilitaryBuildings) == false)
   {
      return;
   }
   debugMilitaryBuildings("--- Running Rule baseBuildingExpansionMonitor. ---");

   if (gDefensivelyOverrun == true)
   {
      debugMilitaryBuildings("We're defensively overrun, don't needlessly expand our base right now.");
      return;
   }

   int waitTime = 120;
   // Expand our bases more when we're a "builder" type personality.
   if (cPersonalityCurrent == cPersonalityConqueror || cPersonalityCurrent == cPersonalityDefender)
   {
      waitTime = 60;
   }
   if (startTimeNoExtraBuildingsNeeded + waitTime > xsGetTime())
   {
      debugMilitaryBuildings("We will start expanding our Town Center base at: " +
         turnNumberIntoTimeDisplay(startTimeNoExtraBuildingsNeeded + waitTime) +
         ". We must wait " + waitTime + " seconds after our regular logic determined we don't need any extra buildings.");
      return;
   }

   static int queryID = -1;
   if (queryID == -1)
   {
      queryID = kbUnitQueryCreate("baseBuildingExpansionMonitor");
      kbUnitQuerySetPlayerID(queryID, cMyID, false);
      kbUnitQuerySetState(queryID, cUnitStateAlive);
   }

   int numBases = kbBaseGetNumber(cMyID);
   int numPlansCreated = 0;
   int maxBuildingsToCreate = 1;
   if (cDifficultyCurrent >= cDifficultyExtreme)
   {
      maxBuildingsToCreate = 2;
   }
   for (int i = 0; i < numBases; i++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, i);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         continue; // We just expand TC bases.
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityAmphibious) == false)
      {
         continue;
      }
      debugMilitaryBuildings("Analyzing base: " + kbBaseGetNameByID(cMyID, baseID) + ".");
      // Loop through our Military buildings and add some to our bases.
      for (int j = 0; j < gMilitaryBuildings.size(); j++)
      {
         int buildingPUID = gMilitaryBuildings[j];
         // Don't build Fortresses with this, those are very expensive and their construction needs to be really needed or controlled.
         if (kbProtoUnitAvailable(buildingPUID) == false || buildingPUID == gFortressUnit)
         {
            continue;
         }

         kbUnitQuerySetUnitType(queryID, buildingPUID);
         kbUnitQuerySetBaseID(queryID, baseID);
         kbUnitQueryResetResults(queryID);
         int numBuildingsInBase = kbUnitQueryExecute(queryID);
         numBuildingsInBase += getAmountBuildPlansInBase(buildingPUID, baseID);
         
         // Limit how many of these we can make per base based on what kind of building it is.
         int maxOfBuildingInOneBase = cMaxOfRegularBuildingInOneBase;
         if (buildingPUID == cUnitTypeSiegeWorks && cPersonalityCurrent != cPersonalitySieger)
         {
            maxOfBuildingInOneBase = 1;
         }
         debugMilitaryBuildings("We have " + numBuildingsInBase + "/" + maxOfBuildingInOneBase + " " +
            kbProtoUnitGetName(buildingPUID) + " in this base.");
         if (numBuildingsInBase >= maxOfBuildingInOneBase)
         {
            continue;
         }

         createSimpleBuildPlan(buildingPUID, 1, 51, gMilitaryBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
         numPlansCreated++;
         if (numPlansCreated >= maxBuildingsToCreate)
         {
            debugMilitaryBuildings("We've created all the build plans we're allowed for this rule, quiting.");
            return;
         }
      }
   }
}

//==============================================================================
// builderBuildsBasesMonitor 
//==============================================================================
rule builderBuildsBasesMonitor
inactive
group defaultClassicalRules
minInterval 30
{
   if (cPersonalityCurrent != cPersonalityBuilder)
   {
      xsDisableRule("builderBuildsBasesMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoBuildMilitaryBuildings) == false)
   {
      return;
   }
   // Only start doing this after we've been in Classical for a while.
   // We try to make sure we actually built some stuff we really needed already before we start sprawling like mad.
   if (gAgeUpTimes[cAge2] + 120 > xsGetTime())
   {
      return;
   }
   
   debugMilitaryBuildings("--- Running Rule builderBuildsBasesMonitor. ---");

   static int queryID = -1;
   static int[] ongoingBuildPlans = default;
   static int[] shuffledValidBuildings = default;
   if (queryID == -1)
   {
      queryID = kbUnitQueryCreate("builderBuildsBasesMonitor");
      kbUnitQuerySetPlayerID(queryID, cMyID, false);
      kbUnitQuerySetState(queryID, cUnitStateAlive);

      ongoingBuildPlans = new int(0, 0);

      // The purpose of this array is to compose a list of PUIDs that we're allowed to build within this rule.
      // We don't want this order to always be the same and don't want to reshuffle it each run.
      // Thus each game you get a new shuffle.
      shuffledValidBuildings = new int(0, 0);

      int[] validBuildings = new int(gMilitaryBuildings.size(), -1);
      for (int i = 0; i < gMilitaryBuildings.size(); i++)
      {
         validBuildings[i] = gMilitaryBuildings[i];
      }
      validBuildings.add(cUnitTypeTemple);
      validBuildings.add(gMarketUnit);
      validBuildings.add(gArmoryUnit);
      if (cMyCulture == cCultureEgyptian)
      {
         validBuildings.add(cUnitTypeLighthouse); // LET'S GO!
      }
      if (cMyCulture == cCultureAtlantean)
      {
         validBuildings.add(cUnitTypeEconomicGuild);
      }

      while (validBuildings.size() > 0)
      {
         int randomIndex = xsRandInt(0, validBuildings.size() - 1);
         shuffledValidBuildings.add(validBuildings[randomIndex]);
         validBuildings.removeIndex(randomIndex);
      }

      debugMilitaryBuildings("Randomly shuffled buildings array for this game:");
      for (int i = 0; i < shuffledValidBuildings.size(); i++)
      {
         debugMilitaryBuildings("   " + i + ": " + kbProtoUnitGetName(shuffledValidBuildings[i]) + ".");
      }
   }

   // Maintain the static array.
   for (int i = ongoingBuildPlans.size() - 1; i >= 0; i--)
   {
      if (aiPlanGetIsIDValid(ongoingBuildPlans[i]) == false)
      {
         ongoingBuildPlans.removeIndex(i);
      }
   }

   // Stop these plans if we're overrun.
   if (gDefensivelyOverrun == true)
   {
      debugMilitaryBuildings("We're defensively overrun, don't needlessly expand our base right now.");
      for (int i = ongoingBuildPlans.size() - 1; i >= 0; i--)
      {
         if (aiPlanGetState(ongoingBuildPlans[i]) == cPlanStateNone || aiPlanGetState(ongoingBuildPlans[i]) == cPlanStatePlace)
         {
            debugMilitaryBuildings("Destroying build plan: " + aiPlanGetName(ongoingBuildPlans[i]) + ".");
            aiPlanDestroy(ongoingBuildPlans[i]);
            ongoingBuildPlans.removeIndex(i);
         }
      }
      return;
   }

   // Unlike baseBuildingExpansionMonitor we're going to do far more random expansion + allow more buildings and building types.
   int[] tcBases = new int(0, 0);
   int numBases = kbBaseGetNumber(cMyID);
   for (int i = 0; i < numBases; i++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, i);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         continue; // We just expand TC bases.
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityAmphibious) == false)
      {
         continue;
      }
      debugMilitaryBuildings("Base: " + kbBaseGetNameByID(cMyID, baseID) + " is valid to analyze later.");
      tcBases.add(baseID);
   }

   int numValidTCBases = tcBases.size();
   if (numValidTCBases == 0)
   {
      debugMilitaryBuildings("Found no valid TC bases to expand.");
      return;
   }

   int maxBuildPlansToCreatePerExecution = selectByDifficulty(1, 1, 1, 2, 2, 2);
   int numPlansCreated = ongoingBuildPlans.size();
   int maxBuildingsToCreate = selectByDifficulty(1, 2, 3, 4, 5, 6);
   debugMilitaryBuildings("We're allowed to have a maximum of " + maxBuildingsToCreate + " expansion build plans ongoing. " +
      "We already have " + numPlansCreated + " from previous runs. We're allowed to create a maximum of " + 
      maxBuildPlansToCreatePerExecution + " each time we run this rule.");
   if (numPlansCreated >= maxBuildingsToCreate)
   {
      return;
   }

   int currentIndex = xsRandInt(0, numValidTCBases - 1);
   int basesAnalyzed = 0;
   while (basesAnalyzed < numValidTCBases)
   {
      int baseID = tcBases[currentIndex];
      debugMilitaryBuildings("Analyzing base: " + kbBaseGetNameByID(cMyID, baseID) + ".");

      // Loop through our allowed buildings and add some to our bases.
      for (int j = 0; j < shuffledValidBuildings.size(); j++)
      {
         int buildingPUID = shuffledValidBuildings[j];
         if (kbProtoUnitAvailable(buildingPUID) == false)
         {
            continue;
         }

         kbUnitQuerySetUnitType(queryID, buildingPUID);
         kbUnitQuerySetBaseID(queryID, baseID);
         kbUnitQueryResetResults(queryID);
         int numBuildingsInBase = kbUnitQueryExecute(queryID);
         numBuildingsInBase += getAmountBuildPlansInBase(buildingPUID, baseID);
         
         // Limit how many of these we can make per base based on what kind of building it is.
         int maxOfBuildingInOneBase = selectByDifficulty(4, 4, 5, 5, 6, 6); // These numbers are mainly for military production.

         if (buildingPUID == gArmoryUnit || buildingPUID == gMarketUnit || buildingPUID == cUnitTypeEconomicGuild ||
             buildingPUID == cUnitTypeLighthouse)
         {
            // Just show these buildings more prevelant instead of 1 in total, 1 in each base.
            maxOfBuildingInOneBase = 1;
         }
         else if (buildingPUID == cUnitTypeTemple || buildingPUID == gFortressUnit || buildingPUID == cUnitTypeSiegeWorks)
         {
            maxOfBuildingInOneBase = 2;
         }
         debugMilitaryBuildings("We have " + numBuildingsInBase + "/" + maxOfBuildingInOneBase + " " +
            kbProtoUnitGetName(buildingPUID) + " in this base.");
         if (numBuildingsInBase >= maxOfBuildingInOneBase)
         {
            continue;
         }

         int prio = xsRandBool() == true ? 51 : 50; //  Some plans higher prio, some regular.
         int planID = createSimpleBuildPlan(buildingPUID, 1, prio, gMilitaryBuildingsCategoryID, baseID,
            cCalculateNumBuildersAutomatically);
         ongoingBuildPlans.add(planID);
         if (ongoingBuildPlans.size() >= maxBuildingsToCreate)
         {
            debugMilitaryBuildings("We're at our max of ongoing build plans for this rule, quiting.");
            return;
         }
         numPlansCreated++;
         if (numPlansCreated >= maxBuildingsToCreate)
         {
            debugMilitaryBuildings("We've created the maximum allowed number of plans per rule execution, quiting.");
            return;
         }
      }

      currentIndex++;
      currentIndex %= numValidTCBases;
      basesAnalyzed++;
   }
}

//==============================================================================
// calculateTowerAmountPerTCBase
//==============================================================================
int calculateTowerAmountPerTCBase()
{
   int personalityModifier = 1; // Standard.
   if (cPersonalityCurrent == cPersonalityAttacker)
   {
      personalityModifier = 0;
   }
   else if (cPersonalityCurrent == cPersonalityDefender)
   {
      personalityModifier = 2;
   }
   int age = kbPlayerGetAge(cMyID);
   int ageModifier = 0;
   if (age == cAge3) // Heroic.
   {
      ageModifier = 1;
   }
   if (age == cAge4) // Mythic.
   {
      ageModifier = 2;
      if (cPersonalityCurrent == cPersonalityDefender)
      {
         personalityModifier = 3;
      }
   }
   int towerTotal = personalityModifier + ageModifier;

   switch (cDifficultyCurrent)
   {
      case cDifficultyModerate:
      {
         towerTotal = max(towerTotal, 2);
         break;
      }
      case cDifficultyHard:
      {
         towerTotal = max(towerTotal, 3);
         break;
      }
   }

   debugMilitaryBuildings("calculateTowerAmountPerTCBase returned: " + towerTotal + ".");
   return towerTotal;
}

//==============================================================================
// towerManager
// Tries to maintain as many towers as the strategy says.
// Or as how many we calculated automatically.
//==============================================================================
rule towerManager
inactive
group defaultClassicalRules
minInterval 1 // We set the proper interval on the first run.
{
   if (checkStrategyFlag(cStrategyFlagBuildTowers) == false)
   {
      return;
   }
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("towerManager");
      return;
   }
   debugMilitaryBuildings("--- Running Rule towerManager. ---");
   static bool firstRun = true;
   if (firstRun == true)
   {
      int intervalTime = 120;
      if (cDifficultyCurrent <= cDifficultyHard)
      {
         intervalTime += 60;
      }
      if (cPersonalityCurrent == cPersonalityAttacker)
      {
         intervalTime += 30;
      }
      else if (cPersonalityCurrent == cPersonalityDefender)
      {
         intervalTime -= 30;
      }
      debugMilitaryBuildings("Setting minInterval of towerManager to " + intervalTime + ".");
      xsSetRuleMinInterval("towerManager", intervalTime);
      firstRun = false;
   }
   
   // First fetch the strategy numbers. If the strategy hasn't set this number it's cCalculateNumberTowersAutomatically.
   int towersWanted = getStrategyTowerAmount();
   if (towersWanted == cCalculateNumberTowersAutomatically)
   {
      towersWanted = calculateTowerAmountPerTCBase();
   }
   if (towersWanted == 0)
   {
      debugMilitaryBuildings("Quiting towerManager because we want to build 0 Towers right now.");
      return;
   }
   debugMilitaryBuildings("We want " + towersWanted + " Towers per Town Center base.");

   int numPossibleToBuild = 0;
   int towerPUID = cUnitTypeSentryTower;
   if ((cMyCiv == cCivOranos || cMyCiv == cCivKronos) && kbTechGetStatus(cTechMythicAgeHelios) == cTechStatusActive)
   {
      numPossibleToBuild = calculateNumPossibleToBuildBuildings(cUnitTypeMirrorTower);
      if (numPossibleToBuild == 0)
      {
         debugMilitaryBuildings("We've aged up with Helios, we are however already at our Mirror Tower build limit.");
      }
      else
      {
         debugMilitaryBuildings("We've aged up with Helios, we could randomly decide to build a Mirror Tower now.");
         if (xsRandBool() == true)
         {
            towerPUID = cUnitTypeMirrorTower;
            debugMilitaryBuildings("We've decided to build a mirror tower!");
         }
         else
         {
            debugMilitaryBuildings("Not going to build a Mirror Tower.");
         }
      }
   }
   // If we have already choosen to build a Mirror Tower we don't need to check Sentry Tower BL anymore.
   if (towerPUID == cUnitTypeSentryTower)
   {
      numPossibleToBuild = calculateNumPossibleTowersToBuild();
      if (numPossibleToBuild == 0)
      {
         debugMilitaryBuildings("Quiting towerManager because we're already at our build limit.");
         return;
      }
   }

   int numTotalTowersBuilt = 0;
   int numMaxAllowedTowersPerIteration = 1;
   if (cDifficultyCurrent >= cDifficultyTitan && cPersonalityCurrent == cPersonalityDefender && numPossibleToBuild >= 2)
   {
      numMaxAllowedTowersPerIteration = 2;
   }

   int numBases = kbBaseGetNumber(cMyID);
   for (int i = 0; i < numBases; i++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, i);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityLand) == false)
      {
         continue;
      }
      if (getCountOfOwnAliveBuildingInBase(cUnitTypeAbstractTower, baseID) +
          getAmountBuildPlansInBase(cUnitTypeMirrorTower, baseID) +
          getAmountBuildPlansInBase(cUnitTypeSentryTower, baseID) >= towersWanted)
      {
         debugMilitaryBuildings("Skipping base: " + kbBaseGetNameByID(cMyID, baseID) + ", because it already has our max amount of " +
            "Towers per base.");
         continue;
      }
      // Valid base to place a Tower!
      debugMilitaryBuildings("Analyzing base: " + kbBaseGetNameByID(cMyID, baseID) + ", to place a Tower.");

      vector baseVec =  kbBaseGetLocation(cMyID, baseID);
      const int numTestVecs = 12;
      float towerAngle = cTwoPi / numTestVecs;
      const float cBaseTowerRadius = 24.0;
      // Half distance between each test vec.
      float exclusionRadius = cBaseTowerRadius * sin(towerAngle / 2.0);
      vector startingVec = rotateByAngle(vector(cBaseTowerRadius, 0.0, 0.0), xsRandFloat(0.0, cTwoPi));

      int towerQueryID = useSimpleUnitQuery(cUnitTypeAbstractTower, cMyID, cUnitStateABQ);
      kbUnitQuerySetBaseID(towerQueryID, baseID);
      int numberTowers = kbUnitQueryExecute(towerQueryID);
      int[] towers = kbUnitQueryGetResults(towerQueryID);
      int gatesQueryID = useSimpleUnitQuery(cUnitTypeWallGate, cMyID, cUnitStateABQ);
      kbUnitQuerySetBaseID(gatesQueryID, baseID);
      int numberGates = kbUnitQueryExecute(gatesQueryID);
      int[] gates = kbUnitQueryGetResults(gatesQueryID);
      int testVecIndex = 0;

      bool createdBuildPlan = false;
      while (createdBuildPlan == false && testVecIndex < numTestVecs)
      {
         bool validPlacement = false;
         vector location = cInvalidVector;
         for (int j = 0; j < numberGates; j++)
         {
            validPlacement = true;
            location = kbUnitGetPosition(gates[j]);
            vector stepBack = baseVec - location;
            location = location + xsVectorNormalize(stepBack) * 3.0;
            for (int k = 0; k < numberTowers; k++)
            {
               if (xsVectorLength(kbUnitGetPosition(towers[k]) - location) < exclusionRadius)
               {
                  validPlacement = false;
                  break;
               }
            }
            if (kbGetIsLocationOnMap(location) == false)
            {
               validPlacement = false;
            }
            if (validPlacement == true)
            {
               debugMilitaryBuildings("Gate method: valid Tower location: " + location + ".");
               break;
            }
         }

         // Fallback to old logic.
         if (validPlacement == false)
         {
            location = baseVec + rotateByAngle(startingVec, towerAngle * testVecIndex);
            validPlacement = true;
            // Check if location is used already.
            for (int j = 0; j < numberTowers; j++)
            {
               if (xsVectorLength(kbUnitGetPosition(towers[j]) - location) < exclusionRadius)
               {
                  validPlacement = false;
                  break;
               }
            }
            if (kbGetIsLocationOnMap(location) == false)
            {
               validPlacement = false;
            }
            if (validPlacement == true)
            {
               debugMilitaryBuildings("Circle around base method: valid Tower location: " + location + ".");
            }
         }

         if (validPlacement == true)
         {
            int prio = 50;
            if (cPersonalityCurrent == cPersonalityDefender)
            {
               debugMilitaryBuildings("Increase priority for Tower build plan to 51 because we're a Defender.");
               prio = 51;
            }
            int planID = createLocationBuildPlan(towerPUID, 1, prio, gMilitaryBuildingsCategoryID, location, 7.5, 2.0);
            aiPlanSetBaseID(planID, baseID); // Force it to the current base so it properly gets put in that base too.
            createdBuildPlan = true;
            numTotalTowersBuilt++;
         }

         testVecIndex++;
      }

      if (createdBuildPlan == true)
      {
         debugMilitaryBuildings("Created a Tower build plan for this base!.");
      }
      else
      {
         debugMilitaryBuildings("Couldn't find a spot to build a Tower on for this base.");
      }

      if (numTotalTowersBuilt >= numMaxAllowedTowersPerIteration)
      {
         debugMilitaryBuildings("Created " + numTotalTowersBuilt + " Tower build plans, which is also our max per iteration, " +
            "not analyzing more bases.");
         return;
      }
   }
}

//==============================================================================
// wallManager
//==============================================================================
rule wallManager
inactive
group defaultClassicalRules
minInterval 10
{
   static int[] planIDs = default;
   // Remove invalid plans, and plans we no longer want.
   for (int i = planIDs.size() - 1; i >= 0; i--)
   {
      if (aiPlanGetIsIDValid(planIDs[i]) == false)
      {
         planIDs.removeIndex(i);
         continue;
      }
      int baseID = aiPlanGetBaseID(planIDs[i]);
      if (kbBaseGetIsIDValid(cMyID, baseID) == false)
      {
         aiPlanSetState(planIDs[i], cPlanStateDone);
         planIDs.removeIndex(i);
         continue;
      }
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         aiPlanSetState(planIDs[i], cPlanStateDone);
         planIDs.removeIndex(i);
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID, kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)),
          cPassabilityLand) == false)
      {
         aiPlanSetState(planIDs[i], cPlanStateDone);
         planIDs.removeIndex(i);
         continue;
      }
   }

   if (checkStrategyFlag(cStrategyFlagBuildWalls) == false)
   {
      // Our existing plans need to disappear.
      for (int i = 0; i < planIDs.size(); i++)
      {
         aiPlanSetState(planIDs[i], cPlanStateDone);
      }
      planIDs.clear();
      return;
   }
   debugMilitaryBuildings("--- Running Rule wallManager. ---");

   int numWallCircles = getStrategyWallCircleAmount();
   if (numWallCircles == 0)
   {
      // Our existing plans need to disappear.
      for (int i = 0; i < planIDs.size(); i++)
      {
         aiPlanSetState(planIDs[i], cPlanStateDone);
      }
      planIDs.clear();
      debugMilitaryBuildings("We don't want to build any wall circles right now, quiting.");
      return;
   }

   // We want circles around all our Town Center bases.
   // Find TC bases that we can use.
   int numBases = kbBaseGetNumber(cMyID);
   for (int iBase = 0; iBase < numBases; iBase++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, iBase);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID, kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)),
          cPassabilityLand) == false)
      {
         continue;
      }
      // This base needs walls!

      int existingPlanID = -1;
      for (int i = 0; i < planIDs.size(); i++)
      {
         if (aiPlanGetBaseID(planIDs[i]) == baseID)
         {
            existingPlanID = planIDs[i];
            break;
         }
      }
      // Yes this is flawed. If we suddenly want more Wall circles we should check if this base actually has enough plans already.
      // But then we need to figure out what plan is missing based on the radius, so just don't do that...
      // Wait until the existing plan is done.
      if (existingPlanID != -1)
      {
         continue;
      }

      // This base has no plans, create the appropriate amount.
      for (int iCircle = 0 ; iCircle < numWallCircles; iCircle++)
      {
         int wallPlanID = aiPlanCreate(kbBaseGetNameByID(cMyID, baseID) + " Wall " + iCircle, cPlanBuildWall, -1,
            gMilitaryBuildingsCategoryID);
         aiPlanSetVariableInt(wallPlanID, cBuildWallPlanWallType, 0, cBuildWallPlanWallTypeRing);
         // TODO: we should allow multiple workers but currently the plan doesn't support it.
         if (cMyCulture != cCultureNorse)
         {
            aiPlanAddUnitType(wallPlanID, cUnitTypeAbstractVillager, 1, 1, 1);
         }
         else
         {
            aiPlanAddUnitType(wallPlanID, cUnitTypeLogicalTypeNorseSoldierThatBuilds, 1, 1, 1);
         }
         aiPlanSetVariableVector(wallPlanID, cBuildWallPlanWallRingCenterPoint, 0, kbBaseGetLocation(cMyID, baseID));
         float radius = 30.0 + iCircle * 20.0;
         aiPlanSetVariableFloat(wallPlanID, cBuildWallPlanWallRingRadius, 0, radius);
         // Gate obstruction size is 8, we want 1 gate every 3 walls.
         aiPlanSetVariableInt(wallPlanID, cBuildWallPlanNumberOfGates, 0, radius * cTwoPi / 36.0);
         aiPlanSetBaseID(wallPlanID, baseID);
         // Prioritize inner circles more than outer.
         aiPlanSetPriority(wallPlanID, 52 - iCircle);
         planIDs.add(wallPlanID);
      }
   }
}

//==============================================================================
// secondRingWallPlanMonitor helpers
//==============================================================================
void debugSecondRing(string message = "")
{
   if (gDebugSecondRing == true)
   {
      aiEcho(message);
   }
}

int createSecondRingWallPlan(int baseID)
{
   int wallPlanID = aiPlanCreate(kbBaseGetNameByID(cMyID, baseID) + " 2nd Ring Wall", cPlanBuildWall, -1,
      gMilitaryBuildingsCategoryID);
   if (wallPlanID == -1)
   {
      debugSecondRing("Failed to create 2nd ring wall plan.");
      return -1;
   }
   aiPlanSetVariableInt(wallPlanID, cBuildWallPlanWallType, 0, cBuildWallPlanWallTypeRing);
   if (cMyCulture != cCultureNorse)
   {
      aiPlanAddUnitType(wallPlanID, cUnitTypeAbstractVillager, 1, 1, 1);
   }
   else
   {
      aiPlanAddUnitType(wallPlanID, cUnitTypeLogicalTypeNorseSoldierThatBuilds, 1, 1, 1);
   }
   aiPlanSetVariableVector(wallPlanID, cBuildWallPlanWallRingCenterPoint, 0, kbBaseGetLocation(cMyID, baseID));
   aiPlanSetVariableFloat(wallPlanID, cBuildWallPlanWallRingRadius, 0, 50.0);
   aiPlanSetVariableInt(wallPlanID, cBuildWallPlanNumberOfGates, 0, 50.0 * cTwoPi / 36.0);
   aiPlanSetBaseID(wallPlanID, baseID);
   aiPlanSetPriority(wallPlanID, 51);
   gSecondRingWallStartTime = xsGetTime();
   debugSecondRing("Created 2nd ring wall plan " + wallPlanID + " for base " + baseID + ".");
   return wallPlanID;
}

void destroySecondRingWallPlan(string reason)
{
   if (gSecondRingWallPlanID != -1 && aiPlanGetIsIDValid(gSecondRingWallPlanID) == true)
   {
      aiPlanDestroy(gSecondRingWallPlanID);
   }
   debugSecondRing("Destroyed 2nd ring wall plan: " + reason);
   gSecondRingWallPlanID = -1;
   gSecondRingWallStartTime = -1;
   gSecondRingAttackStartTime = -1;
   gSecondRingWallLastDestroyedTime = xsGetTime();
}

bool isBaseUnderSustainedAttack(int baseID)
{
   int baseIndex = gDefendTCBases.find(baseID);
   if (baseIndex == -1)
   {
      gSecondRingAttackStartTime = -1;
      return false;
   }
   if (gEnemyPowerInBases[baseIndex] == 0)
   {
      gSecondRingAttackStartTime = -1;
      return false;
   }
   if (gSecondRingAttackStartTime == -1)
   {
      gSecondRingAttackStartTime = xsGetTime();
   }
   // AoModAi's early-game exemption: don't cancel walls for minor pushes before 19 min.
   if (xsGetTime() <= 19 * 60 * 1000)
   {
      return false;
   }
   return (xsGetTime() - gSecondRingAttackStartTime) > 25 * 1000;
}

//==============================================================================
// secondRingWallPlanMonitor
//==============================================================================
rule secondRingWallPlanMonitor
inactive
group defaultClassicalRules
minInterval 10
{
   if (checkStrategyFlag(cStrategyFlagBuildWalls) == false)
   {
      if (gSecondRingWallPlanID != -1)
      {
         destroySecondRingWallPlan("strategy wall flag disabled");
      }
      return;
   }

   int mainBaseID = kbBaseGetMainID(cMyID);
   if (kbBaseGetIsIDValid(cMyID, mainBaseID) == false)
   {
      if (gSecondRingWallPlanID != -1)
      {
         destroySecondRingWallPlan("main base invalid");
      }
      return;
   }

   // Same area-group check used by wallManager.
   if (gLandAreaGroupID != -1 &&
       kbPathAreAreaGroupsConnected(gLandAreaGroupID, kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, mainBaseID)),
       cPassabilityLand) == false)
   {
      if (gSecondRingWallPlanID != -1)
      {
         destroySecondRingWallPlan("main base unreachable");
      }
      return;
   }

   // Rusher delay gates the second ring.
   if (mRusher == true &&
       (kbGetAge() < cAge3 || xsGetTime() < 15 * 60 * 1000))
   {
      if (gSecondRingWallPlanID != -1)
      {
         destroySecondRingWallPlan("rusher delay");
      }
      debugSecondRing("Rusher delay active: waiting until Age 3 + 15 min.");
      return;
   }

   // Maintain an existing second-ring plan.
   if (gSecondRingWallPlanID != -1)
   {
      if (aiPlanGetIsIDValid(gSecondRingWallPlanID) == false)
      {
         debugSecondRing("2nd ring plan became invalid, resetting.");
         gSecondRingWallPlanID = -1;
         gSecondRingWallStartTime = -1;
         return;
      }

      // 12-minute lifetime cap.
      if (gSecondRingWallStartTime != -1 &&
          xsGetTime() - gSecondRingWallStartTime > 12 * 60 * 1000)
      {
         destroySecondRingWallPlan("12 minute lifetime expired");
         return;
      }

      // Destroy the plan if the main base has been under attack for more than 25 s.
      if (isBaseUnderSustainedAttack(mainBaseID) == true)
      {
         destroySecondRingWallPlan("sustained attack");
         return;
      }

      // Gating drop: destroy if resources or villagers fall below threshold.
      if (kbResourceGet(cResourceGold) < 150.0 ||
          kbUnitCount(cUnitTypeAbstractVillager, cMyID, cUnitStateAlive) < 10)
      {
         destroySecondRingWallPlan("gating no longer met");
         return;
      }

      return;
   }

   // WAITING: require a first-ring wall plan before creating the second ring.
   if (aiPlanGetNumber(cPlanBuildWall, -1, true) == 0)
   {
      return;
   }

   // Creation gating (AoModAi defaults).
   if (kbGetAge() < cAge2) return;
   if (xsGetTime() < 8 * 60 * 1000) return;
   if (kbUnitCount(cUnitTypeAbstractVillager, cMyID, cUnitStateAlive) < 10) return;
   if (kbResourceGet(cResourceGold) < 150.0) return;

   // 60-second re-creation cooldown after a destruction.
   if (gSecondRingWallLastDestroyedTime != -1 &&
       xsGetTime() < gSecondRingWallLastDestroyedTime + 60 * 1000)
   {
      return;
   }

   gSecondRingWallPlanID = createSecondRingWallPlan(mainBaseID);
}

// === AoModAi: layered walls end ===

//==============================================================================
// templeMonitor
// We always maintain 1 Temple in our most defend base, and slowly build more if resources allow it and we have enough TC bases.
//==============================================================================
rule templeMonitor
inactive
group defaultArchaicRules
minInterval 30
{
   if (checkStrategyFlag(cStrategyFlagBuildTemple) == false)
   {
      return;
   }
   debugMilitaryBuildings("--- Running Rule templeMonitor. ---");
   // We never want to have more than 1 Temple build plan active at the same time.
   // This also solves an issue in Nomad where your mainBase changes and then suddenly getAmountBuildPlansInBase can't 
   // find your existing build plan for the old mainBase anymore and you end up with 2.
   if (aiPlanGetIsIDValid(aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeTemple)) == true)
   {
      debugMilitaryBuildings("Already have a Temple build plan, quiting.");
      return;
   }

   // If we're here we know we aren't already building a temple.
   int templeCount = kbUnitCount(cUnitTypeTemple, cMyID, cUnitStateAlive);
   if (templeCount == 0)
   {
      // If we currently own no Temples, place one in our most fortified TC base.
      int baseID = getMostDefendedTCBase();
      if (baseID == -1)
      {
         debugMilitaryBuildings("We have no TC bases, not building a Temple now.");
      }
      else
      {
         createSimpleBuildPlan(cUnitTypeTemple, 1, 55, gMilitaryBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      }
      return;
   }

   // If we're here we already have 1 Temple at least. We don't really need many more but will slowly build more in other TC bases.
   if (haveExcessResourceAmount(400.0, cResourceGold) == false)
   {
      debugMilitaryBuildings("We don't have enough excess gold to make more Temples.");
      return;
   }
   if (cMyCulture != cCultureEgyptian && haveExcessResourceAmount(400.0, cResourceWood) == false)
   {
      debugMilitaryBuildings("We don't have enough excess wood to make more Temples.");
      return;
   }

   bool madePlan = false;
   // Find TC bases that we can use.
   int numBases = kbBaseGetNumber(cMyID);
   for (int iBase = 0; iBase < numBases; iBase++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, iBase);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityLand) == false)
      {
         continue;
      }
      // Just one per base.
      if (getCountOfOwnAliveBuildingInBase(cUnitTypeTemple, baseID) >= 1)
      {
         continue;
      }
      createSimpleBuildPlan(cUnitTypeTemple, 1, 50, gMilitaryBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      madePlan = true;
      break;
   }

   if (madePlan == false)
   {
      debugMilitaryBuildings("Didn't find a suitable TC base to make a Temple in.");
   }
}

//==============================================================================
// chineseSocketManager
//==============================================================================
rule chineseSocketManager
group defaultClassicalRules
inactive
minInterval 60
{
   if (cMyCulture != cCultureChinese)
   {
      xsDisableRule("chineseSocketManager");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticChineseSocketBuilding) == false)
   {
      return;
   }
   debugMilitaryBuildings("--- Running Rule chineseSocketManager. ---");

   bool haveMilitaryCamp = kbUnitCount(cUnitTypeMilitaryCamp, cMyID, cUnitStateAlive) > 0;
   bool haveMachineWorkshop = kbUnitCount(cUnitTypeMachineWorkshop, cMyID, cUnitStateAlive) > 0;
   debugMilitaryBuildings("Have Military Camp: " + xsBoolToString(haveMilitaryCamp) + ", have Machine Workshop: " +
      xsBoolToString(haveMilitaryCamp) + ".");
   if (haveMilitaryCamp == false && haveMachineWorkshop == false)
   {
      return;
   }

   bool allowedToMakeTowers = calculateNumPossibleTowersToBuild() >= 1;
   if (allowedToMakeTowers == false && gDefensivelyOverrun == true)
   {
      debugMilitaryBuildings("Can't make any more Towers and we're defensively overrun, no valid sockets to build.");
      return;
   }

   int[] possibleUpgrades = new int(0, 0);
   if (haveMilitaryCamp == true)
   {
      if (aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechMilitaryCampToTower) > 0 ||
          aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechMilitaryCampToTrainingYard) > 0)
      {
         debugMilitaryBuildings("We already have a research plan for a socket upgrade, not stacking them.");
         return;
      }

      if (allowedToMakeTowers == true)
      {
         possibleUpgrades.add(cTechMilitaryCampToTower);
      }
      if (gDefensivelyOverrun == false)
      {
         possibleUpgrades.add(cTechMilitaryCampToTrainingYard);
      }
   }
   if (haveMachineWorkshop == true)
   {
      if (aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechMachineWorkshopToTower) > 0 ||
          aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechMachineWorkshopToTrainingYard) > 0)
      {
         debugMilitaryBuildings("We already have a research plan for a socket upgrade, not stacking them.");
         return;
      }

      if (allowedToMakeTowers == true)
      {
         possibleUpgrades.add(cTechMachineWorkshopToTower);
      }
      if (gDefensivelyOverrun == false)
      {
         possibleUpgrades.add(cTechMachineWorkshopToTrainingYard);
      }
   }
   
   int rand = xsRandInt(0, possibleUpgrades.size() - 1);
   int techID = possibleUpgrades[rand];
   if (techID == cTechMilitaryCampToTower || techID == cTechMilitaryCampToTrainingYard)
   {
      debugMilitaryBuildings("Chosen to transform a Military Camp into: " + kbTechGetName(techID) + ".");
      int unitID = getUnit(cUnitTypeMilitaryCamp);
      int planID = createSimpleResearchPlanSpecificResearcher(techID, unitID, 50, true);
      aiPlanSetVariableInt(planID, cResearchPlanTransformToPUID, 0,
         techID == cTechMilitaryCampToTower ? cUnitTypeMilitaryCampTower : cUnitTypeMilitaryCampTrainingYard);
   }
   else
   {
      debugMilitaryBuildings("Chosen to transform a Machine Workshop into: " + kbTechGetName(techID) + ".");
      int unitID = getUnit(cUnitTypeMachineWorkshop);
      int planID = createSimpleResearchPlanSpecificResearcher(techID, unitID, 50, true);
      aiPlanSetVariableInt(planID, cResearchPlanTransformToPUID, 0,
         techID == cTechMachineWorkshopToTower ? cUnitTypeMachineWorkshopTower : cUnitTypeMachineWorkshopTrainingYard);
   }
}

//==============================================================================
// fortressRepairMonitor
//==============================================================================
rule fortressRepairMonitor
group defaultHeroicRules
inactive
minInterval 30
{
   if (checkStrategyFlag(cStrategyFlagAutomaticFortressRepair) == false)
   {
      return;
   }
   debugMilitaryBuildings("--- Running Rule fortressRepairMonitor. ---");
   
   if (kbUnitCount(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateABQ) == 0)
   {
      debugMilitaryBuildings("Can't look to repair any Fortresses because we have no Town Centers left, focus on that fully.");
      return;
   }

   int queryID = useSimpleUnitQuery(cUnitTypeBuilding, cMyID, cUnitStateAlive);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(queryID, gLandAreaGroupID, cPassabilityLand);
   }
   int numResults = kbUnitQueryExecute(queryID);
   int[] results = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int unitID = results[i];
      if (kbUnitGetStatFloat(unitID, cUnitStatHPRatio) < 1.0)
      {
         // We need to repair!
         if (aiPlanGetIDByTypeAndVariableIntValue(cPlanRepair, cRepairPlanTargetID, unitID) < 0)
         {
            int areaID = kbUnitGetAreaID(unitID);
            // Dont start repairing in a warzone.
            if (kbAreaGetDangerLevel(areaID) >= 100.0)
            {
               debugMilitaryBuildings("Won't repair " + kbProtoUnitGetName(gFortressUnit) + "(" + unitID +
                              ") right now because the area is too dangerous.");
               continue;
            }
            debugMilitaryBuildings("Found a " + kbProtoUnitGetName(gFortressUnit) + "(" + unitID +
                           ") that has been damaged, creating a repair plan for it.");
            int planID = aiPlanCreate("Repair " + kbProtoUnitGetName(gFortressUnit) + " ID: " + unitID, cPlanRepair, -1,
                                      gMilitaryBuildingsCategoryID);
            aiPlanSetVariableInt(planID, cRepairPlanTargetID, 0, unitID);
            // Little bit higher prio since we need these buildings to compete.
            aiPlanSetPriority(planID, 51);
            addBuilderTypesToPlan(planID, gFortressUnit, 2);
            aiPlanSetBaseID(planID, kbUnitGetBaseID(unitID));
         }
      }
   }
}

//==============================================================================
// buildingRepairMonitor
// This rule is very infrequent and tries to repair our lesser important buildings.
// Also this rule has more limitations, if we're already repairing something we just quit.
//==============================================================================
rule buildingRepairMonitor
group defaultClassicalRules
inactive
minInterval 60
{
   if (checkStrategyFlag(cStrategyFlagAutomaticBuildingRepair) == false)
   {
      return;
   }
   debugMilitaryBuildings("--- Running Rule buildingRepairMonitor. ---");
   
   if (kbUnitCount(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateABQ) == 0)
   {
      debugMilitaryBuildings("Can't look to repair any new buildings because we have no Town Centers left, focus on that fully.");
      return;
   }
   if (aiPlanGetNumberByType(cPlanRepair) != 0)
   {
      debugMilitaryBuildings("Can't look to repair any new buildings because we already have an active repair plan.");
      return;
   }

   static bool firstRun = true;
   static int[] excludes = default;
   if (firstRun == true)
   {
      firstRun = false;
      excludes = new int(4, -1);
      // We repair these buildings in other rules.
      excludes[0] = cUnitTypeAbstractSocketedTownCenter;
      excludes[1] = cUnitTypeAbstractFortress;
      excludes[2] = cUnitTypeTitanGate;
      excludes[3] = cUnitTypeWonder;
   }
   
   int queryID = useSimpleUnitQuery(cUnitTypeBuilding, cMyID, cUnitStateAlive);
   if (gLandAreaGroupID != -1)
   {
      kbUnitQuerySetConnectedAreaGroupID(queryID, gLandAreaGroupID, cPassabilityLand);
   }
   kbUnitQuerySetExcludeTypes(queryID, excludes);
   int numResults = kbUnitQueryExecute(queryID);
   int[] results = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int unitID = results[i];
      if (kbPlayerGetProtoStatFlag(cMyID, kbUnitGetProtoUnitID(unitID), cProtoUnitFlagRepairable) == false)
      {
         continue;
      }
      if (kbUnitGetStatFloat(unitID, cUnitStatHPRatio) < 1.0)
      {
         // We need to repair!
         int protoUnitID = kbUnitGetProtoUnitID(unitID);
         int areaID = kbUnitGetAreaID(unitID);
         // Dont start repairing in a warzone.
         if (kbAreaGetDangerLevel(areaID) >= 100.0)
         {
            debugMilitaryBuildings("Won't repair " + kbProtoUnitGetName(protoUnitID) + "(" + unitID +
                           ") right now because the area is too dangerous.");
            continue;
         }
         debugMilitaryBuildings("Found a " + kbProtoUnitGetName(protoUnitID) + "(" + unitID + ") that has been damaged, " +
            "creating a repair plan for it.");
         int outputCategoryID = gEconomicBuildingsCategoryID;
         if (isMilitaryBuilding(protoUnitID) == true)
         {
            outputCategoryID = gMilitaryBuildingsCategoryID;
         }
         int planID = aiPlanCreate("Repair " + kbProtoUnitGetName(protoUnitID) + " ID: " + unitID, cPlanRepair, -1, outputCategoryID);
         aiPlanSetVariableInt(planID, cRepairPlanTargetID, 0, unitID);
         aiPlanSetPriority(planID, 50);
         addBuilderTypesToPlan(planID, protoUnitID, 1);
         aiPlanSetBaseID(planID, kbUnitGetBaseID(unitID));
         // One plan at a time.
         return;
      }
   }
}

//==============================================================================
// arenaGates
// Transform some of our Wall Longs into Gates.
//==============================================================================
rule arenaGates
inactive
minInterval 10
{
   debugMilitaryBuildings("--- Running Rule arenaGates. ---");

   static int[] toTransform = default;
   static bool firstRun = true;
   if (firstRun == true)
   {
      toTransform = new int (2, -1);
      int queryID = useSimpleUnitQuery(cUnitTypeWallLong);
      int numResults = kbUnitQueryExecute(queryID);
      // Few Wall Longs? Just transform 1 and be done otherwise the index math below doesn't work out anymore.
      if (numResults < 7)
      {
         aiTransformWallIntoGate(kbUnitQueryGetResult(queryID, numResults / 2));
         debugMilitaryBuildings("Disabling rule arenaGates because we started with few Wall Longs and just transformed 1.");
         xsDisableRule("arenaGates");
         return;
      }
      // On the Arena maps this makes sure we get Gates in all corners of our Wall.
      toTransform[0] = kbUnitQueryGetResult(queryID, 2);
      toTransform[1] = kbUnitQueryGetResult(queryID, numResults - 3);
      // We start with 15 extra gold on Arena to make 1 Gate, do so now.
      aiTransformWallIntoGate(kbUnitQueryGetResult(queryID, numResults / 2));
      firstRun = false;
   }

   // Don't need extra Gates in Archaic and don't want to mess with BO resource management.
   if (kbPlayerGetAge(cMyID) == cAge1 || isBuildOrderDone() == false)
   {
      return;
   }

   for (int i = 0; i < toTransform.size(); i++)
   {
      // Remove Wall Longs that died or have already been transformed into Gates.
      if (kbUnitGetIsIDValid(toTransform[i]) == false ||
          kbUnitIsType(toTransform[i], cUnitTypeWallLong) == false ||
          kbUnitGetPlayerID(toTransform[i]) != cMyID)
      {
         toTransform.removeIndex(i);
         i--;
         continue;
      }
      aiTransformWallIntoGate(toTransform[i]);
   }

   if (toTransform.size() == 0)
   {
      debugMilitaryBuildings("Disabling rule arenaGates because we either got our 3 gates or the Wall Longs went invalid.");
      xsDisableRule("arenaGates");
   }
}

//==============================================================================
// maintainTraps
// Build traps in our TC bases.
// Also maintain a minimal amount of builders for this.
//==============================================================================
rule maintainTraps
group defaultClassicalRules
inactive
minInterval 30
{
   if (checkStrategyFlag(cStrategyFlagBuildTraps) == false)
   {
      return;
   }
   debugMilitaryBuildings("--- Running Rule maintainTraps. ---");

   if (gDefensivelyOverrun == true)
   {
      debugMilitaryBuildings("We're currently defensively overrun, use all army to fight and not build.");
      return;
   }

   if (aiPlanGetNumberByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeSpikeTrap) != 0 ||
       aiPlanGetNumberByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, cUnitTypeSmokeTrap) != 0)
   {
      debugMilitaryBuildings("We already have an ongoing trap build plan, don't stack them.");
      return;
   }

   int maxSmokeTrapsPerBase = 1;
   int maxSpikeTrapsPerBase = selectByDifficulty(1, 1, 1, 2, 2 ,2);
   if (cPersonalityCurrent == cPersonalityDefender)
   {
      maxSmokeTrapsPerBase++;
      maxSpikeTrapsPerBase++;
   }

   // Find TC bases that we can use.
   int numBases = kbBaseGetNumber(cMyID);
   for (int iBase = 0; iBase < numBases; iBase++)
   {
      int baseID = kbBaseGetIDByIndex(cMyID, iBase);
      if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
      {
         continue;
      }
      if (gLandAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(gLandAreaGroupID,
          kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityLand) == false)
      {
         continue;
      }

      int numSpikeTraps = getCountOfOwnAliveBuildingInBase(cUnitTypeSpikeTrap, baseID);
      int numSmokeTraps = getCountOfOwnAliveBuildingInBase(cUnitTypeSmokeTrap, baseID);
      bool haveEnoughSpikeTraps = numSpikeTraps >= maxSpikeTrapsPerBase;
      bool haveEnoughSmokeTraps = numSmokeTraps >= maxSmokeTrapsPerBase;
      debugMilitaryBuildings(kbBaseGetNameByID(cMyID, baseID) + " we have enough spike traps: " +
         xsBoolToString(haveEnoughSpikeTraps) + ", and smoke traps: " + xsBoolToString(haveEnoughSmokeTraps) + ".");
      if (haveEnoughSmokeTraps == true && haveEnoughSpikeTraps == true)
      {
         continue;
      }
      // Force 1 trap per base for thematic purposes.
      int prio = 50;
      if (numSpikeTraps == 0 && numSmokeTraps == 0)
      {
         prio = 51;
      }

      if (haveEnoughSpikeTraps == false)
      {
         createSimpleBuildPlan(cUnitTypeSpikeTrap, 1, prio, gMilitaryBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      }
      else
      {
         createSimpleBuildPlan(cUnitTypeSmokeTrap, 1, prio, gMilitaryBuildingsCategoryID, baseID, cCalculateNumBuildersAutomatically);
      }
      return;
   }
}

/////////////////////////////////////////////////////////////////////////
// DOCKS.
/////////////////////////////////////////////////////////////////////////

const int cCheckForBetterBaseID = 0;
const int cRequiredToFindNewBase = 1;
const int cNoTownCentersLeft = 2;

class BaseDockInfo
{
   // Standard info.
   int mBaseID = -1;
   vector mBaseLocation = cInvalidVector;

   // Actual calculated info.
   bool mValidForDock = false;
   vector mClosestFishLocation = cInvalidVector;
   vector mWaterDefendPoint = cInvalidVector;
   int mShorelineAreaID = -1;
   int mWaterAreaID = -1;
   int mDockAreaGroupID = -1;
   float mDistanceToShoreline = cMaxFloat;

   //==============================================================================
   // displayInfo
   //==============================================================================
   void displayInfo()
   {
      debugMilitaryBuildings("   *** displayInfo. ***");
      debugMilitaryBuildings("      mBaseID: " + mBaseID);
      debugMilitaryBuildings("      mBaseLocation: " + mBaseLocation);
      debugMilitaryBuildings("      mValidForDock: " + xsBoolToString(mValidForDock));
      debugMilitaryBuildings("      mClosestFishLocation: " + mClosestFishLocation);
      debugMilitaryBuildings("      mWaterDefendPoint: " + mWaterDefendPoint);
      debugMilitaryBuildings("      mShorelineAreaID: " + mShorelineAreaID);
      debugMilitaryBuildings("      mWaterAreaID: " + mWaterAreaID);
      debugMilitaryBuildings("      mDockAreaGroupID: " + mDockAreaGroupID);
      debugMilitaryBuildings("      mDistanceToShoreline: " + mDistanceToShoreline);
   }

   //==============================================================================
   // doAnalysis
   //==============================================================================
   void doAnalysis(int baseID = -1)
   {
      debugMilitaryBuildings("*** Performing Dock analysis for BaseID: " + baseID + " ***.");

      mBaseID = baseID;
      mBaseLocation = kbBaseGetLocation(cMyID, baseID);
      int startAreaID = -1; // This is the water area.
      if (gMapInfo.mHasFish == true)
      {
         debugMilitaryBuildings("   The map has Fish, see if we're close enough to fish from this base.");
         if (gOverrideClosestFishLocation == cInvalidVector)
         {
            debugMilitaryBuildings("   No override Fish location exists, querying for Fish instead.");
            vector searchStartPosition = mBaseLocation;
            xsSetContextPlayer(0);
            int natureQueryID = useSimpleNatureUnitQuery(cUnitTypeFishResource, cUnitStateAlive, searchStartPosition, gMaxFishDockScanRange);
            kbUnitQuerySetAscendingSort(natureQueryID, true);
            int numResults = kbUnitQueryExecute(natureQueryID);
            for (int i = 0; i < numResults; i++)
            {
               int resultID = kbUnitQueryGetResult(natureQueryID, i);
               vector resultPosition = kbUnitGetPosition(resultID);
               int waterAreaID = kbAreaGetIDByPosition(resultPosition);
               if (gMapInfo.mUsefulWaterAreaGroupIDs.find(kbAreaGetGroupID(waterAreaID)) != -1)
               {
                  xsSetContextPlayer(cMyID);
                  debugMilitaryBuildings("   Fish " + resultID + " is the closest Fish we can find that is " +
                     "in an area group that we think is useful to have a Dock in.");
                  mClosestFishLocation = resultPosition;
                  break;
               }
            }
            if (mClosestFishLocation == cInvalidVector)
            {
               debugMilitaryBuildings("   No Fish close enough could be found, we can't fish from this base.");
            }
            xsSetContextPlayer(cMyID);
         }
         else
         {
            // We assume we always want to fish if we have an override.
            debugMilitaryBuildings("   We have an override Fish location, we now always want to fish from each base.");
            mClosestFishLocation = gOverrideClosestFishLocation;
         }

         if (mClosestFishLocation != cInvalidVector)
         {
            startAreaID = kbAreaGetIDByPosition(mClosestFishLocation);
            debugMilitaryBuildings("   mClosestFishLocation: " + mClosestFishLocation + ", inside areaID: " + startAreaID + ".");
         }
      }

      bool kothOnDifferentIsland = false;
      if ((cVictoryTypesCurrent & cVictoryTypeKingOfTheHill) != 0 && mClosestFishLocation == cInvalidVector)
      {
         int ownAreaGroupID = kbAreaGroupGetIDByPosition(mBaseLocation);
         int kothAreaGroupID = kbAreaGroupGetIDByPosition(gKOTHPosition);
         if (kbPathAreAreaGroupsConnected(ownAreaGroupID, kothAreaGroupID, cPassabilityLand) == false)
         {
            debugMilitaryBuildings("   The KOTH is on another island than this TC base is.");
            kothOnDifferentIsland = true;
         }
      }

      if ((gMapInfo.mIsIslandMap == true || kothOnDifferentIsland == true) && mClosestFishLocation == cInvalidVector)
      {
         // This means we found no fish to gather from but we definitely should have a Dock because we need to transport.
         if (gMapInfo.mIsIslandMap == true)
         {
            debugMilitaryBuildings("   We found no close Fish but it's an island map, we should build a Dock regardless.");
         }
         else if (kothOnDifferentIsland == true)
         {
            debugMilitaryBuildings("   We found no close Fish but the KOTH is not on the same island as us,  " + 
               "we should build a Dock regardless.");
         }

         // Now we do a mental loop to find the closest water area ID that is useful.
         float closestDistance = cMaxFloat;
         vector searchStartPosition = mBaseLocation;
         for (int i = 0; i < gMapInfo.mUsefulWaterAreaGroupIDs.size(); i++)
         {
            int numAreas = kbAreaGroupGetNumberAreas(gMapInfo.mUsefulWaterAreaGroupIDs[i]);
            for (int iArea = 0; iArea < numAreas; iArea++)
            {
               vector center = kbAreaGetCenter(kbAreaGroupGetAreaID(gMapInfo.mUsefulWaterAreaGroupIDs[i], iArea));
               float distance = xsVectorLength(center - searchStartPosition);
               if (distance < closestDistance)
               {
                  closestDistance = distance;
                  startAreaID = kbAreaGroupGetAreaID(gMapInfo.mUsefulWaterAreaGroupIDs[i], iArea);
               }
            }
         }
         debugMilitaryBuildings("   Loop found closest water area ID to be: " + startAreaID + ".");
      }

      if (startAreaID != -1) // This means that we do want a Dock, either because of fishing or because we need to transport.
      {
         mValidForDock = true;
         int pathID = kbPathCreate("Calculate Dock Position");
         if (kbPathCreateAreaPath(pathID, startAreaID, kbAreaGetIDByPosition(mBaseLocation), cPassabilityAmphibious) == false)
         {
            if (cGameTypeCurrent == cGameTypeCampaign || cGameTypeCurrent == cGameTypeScenario)
            {
               debugMilitaryBuildings("doAnalysis - couldn't create a path to find a shoreline with, adjust scenario if needed.");
            }
            else
            {
               aiEchoWarning("doAnalysis - couldn't create a path to find a shoreline with, map is incompatible.");
            }
            mValidForDock = false;
            return;
         }
         int shorelineAreaID = -1;
         int waterAreaID = startAreaID;
         // We start at i = 1 because we already know waypoint 0 will be in the water at the fish/closest water area.
         for (int i = 1; i < kbPathGetNumberWaypoints(pathID); i++)
         {
            vector waypoint = kbPathGetWaypoint(pathID, i);
            int areaID = kbAreaGetIDByPosition(waypoint);
            if (isAreaPassableByLand(areaID) == true)
            {
               shorelineAreaID = areaID;
               debugMilitaryBuildings("   Found a land area on our path with ID: " + shorelineAreaID +
                  ", closest water area ID: " + waterAreaID + ".");
               break;
            }
            else // Must be water/amphibious then.
            {
               // Update waterAreaID if this is a proper water area and not a small amphibious area.
               if (kbAreaGetNumberTiles(areaID) > 10)
               {
                  waterAreaID = areaID;
               }
            }
         }
         kbPathDestroy(pathID);
         if (shorelineAreaID == -1 || waterAreaID == -1)
         {
            aiEchoWarning("doAnalysis - couldn't find a shorelineAreaID/waterAreaID, this shouldn't be possible ever!");
            mValidForDock = false;
            return;
         }

         vector shorelineCenter = kbAreaGetCenter(shorelineAreaID);
         mDistanceToShoreline = xsVectorLength(mBaseLocation - shorelineCenter);
         mShorelineAreaID = shorelineAreaID;
         mWaterAreaID = waterAreaID;
         mDockAreaGroupID = kbAreaGetGroupID(waterAreaID);

         // Calculate a proper defend point.
         vector waterCenter = kbAreaGetCenter(waterAreaID);
         vector directionStep = xsVectorNormalize(waterCenter - shorelineCenter) * 5;
         mWaterDefendPoint = waterCenter;
         int foundWaterCounter = 0;
         for (int i = 1; i < 15; i++)
         {
            vector testLocation = shorelineCenter + (directionStep * i);
            if (kbGetIsLocationOnMap(testLocation) == false)
            {
               break;
            }
            // Get a position firmly inside the water, away from the Docks.
            if (foundWaterCounter == 4)
            {
               // Account for odd shapes in land/water.
               int testAreaID = kbAreaGetIDByPosition(testLocation);
               if (kbAreaGetType(testAreaID) == cAreaTypeWater || kbAreaGetType(testAreaID) == cAreaTypeAmphibious)
               {
                  mWaterDefendPoint = testLocation;
               }
               break;
            }
            int testAreaID = kbAreaGetIDByPosition(testLocation);
            if (kbAreaGetType(testAreaID) == cAreaTypeWater || kbAreaGetType(testAreaID) == cAreaTypeAmphibious)
            {
               // Already save this point in case the next point is off the map.
               mWaterDefendPoint = testLocation;
               foundWaterCounter++;
            }
         }
      }
      else
      {
         debugMilitaryBuildings("   We do not want to build a Dock for this base.");
      }
      
      // Sum it all up.
      displayInfo();
   }
};

class DockManager
{
   // These 2 should be in sync.
   BaseDockInfo[] mBaseDockInfos = default;
   int[] mTCBases = default;

   int mCurrentBaseID = -1;
   int mCurrentBaseIndex = -1;
   bool mCurrentBaseCanFish = false;

   int[] mShorelineIDs = default;
   int[] mValidwaterIDs = default;
   int[] mValidDockIDs = default;
   int[] mInvalidDockIDs = default;

   int[] mDockBuildPlans = default;
   int mScoutPlanID = -1;

   //==============================================================================
   // resetCurrentInfo
   //==============================================================================
   void resetCurrentInfo()
   {
      mCurrentBaseID = -1;
      mCurrentBaseIndex = -1;
      mCurrentBaseCanFish = false;
      // Also some globals, they're linked to this.
      gDockAreaGroupID = -1;
      gWaterDefendPoint = cInvalidVector;
      gShouldBuildDock = false;
      gShouldFish = false;
      mShorelineIDs.clear();
      mValidwaterIDs.clear();
      mValidDockIDs.clear();
      mInvalidDockIDs.clear();
      // Just blindly destroy all plans we had, not going to find out if they're maybe still valid for the new TC.
      for (int i = 0; i < mDockBuildPlans.size(); i++)
      {
         debugMilitaryBuildings("Destroying plan: " + aiPlanGetName(mDockBuildPlans[i]) + ".");
         aiPlanDestroy(mDockBuildPlans[i]);
      }
      mDockBuildPlans.clear();
      if (aiPlanGetIsIDValid(mScoutPlanID) == true)
      {
         debugMilitaryBuildings("Destroying plan: " + aiPlanGetName(mScoutPlanID) + ".");
         aiPlanDestroy(mScoutPlanID);
      }
      mScoutPlanID = -1;
   }

   //==============================================================================
   // updateBaseArrays
   //==============================================================================
   int updateBaseArrays()
   {
      debugMilitaryBuildings("*** Updating base arrays. ***");
      for (int i = mTCBases.size() - 1; i >= 0; i--)
      {
         int baseID = mTCBases[i];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            mTCBases.removeIndex(i);
            mBaseDockInfos.removeIndex(i);
            if (mCurrentBaseIndex != -1 && i < mCurrentBaseIndex)
            {
               mCurrentBaseIndex--;
            }
            continue;
         }
         if (gLandAreaGroupID != -1 &&
             kbPathAreAreaGroupsConnected(gLandAreaGroupID,
             kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityLand) == false)
         {
            debugMilitaryBuildings("   TC base: " + kbBaseGetNameByID(cMyID, baseID) + ", is not connected to gLandAreaGroupID, " +
               "remove from arrays.");
            mTCBases.removeIndex(i);
            mBaseDockInfos.removeIndex(i);
            if (mCurrentBaseIndex != -1 && i < mCurrentBaseIndex)
            {
               mCurrentBaseIndex--;
            }
            continue;
         }
         if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
         {
            mTCBases.removeIndex(i);
            mBaseDockInfos.removeIndex(i);
            if (mCurrentBaseIndex != -1 && i < mCurrentBaseIndex)
            {
               mCurrentBaseIndex--;
            }
            debugMilitaryBuildings("   TC base: " + kbBaseGetNameByID(cMyID, baseID) + ", has lost its TC, removing from arrays.");
         }
      }

      bool needToFindNewBestBase = false;
      if (mCurrentBaseID == -1 || mTCBases.find(mCurrentBaseID) == -1)
      {
         needToFindNewBestBase = true;
      }

      int numBases = kbBaseGetNumber(cMyID);
      for (int i = 0; i < numBases; i++)
      {
         int baseID = kbBaseGetIDByIndex(cMyID, i);
         if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
         {
            continue;
         }
         if (mTCBases.find(baseID) != -1)
         {
            continue;
         }
         if (gLandAreaGroupID != -1 &&
             kbPathAreAreaGroupsConnected(gLandAreaGroupID,
             kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityLand) == false)
         {
            continue;
         }
         mTCBases.add(baseID);
         debugMilitaryBuildings("   Found a new TC base to add to gDockManager: " + kbBaseGetNameByID(cMyID, baseID) + ".");

         // Calculate everything.
         BaseDockInfo newInfo;
         newInfo.doAnalysis(baseID);
         mBaseDockInfos.add(newInfo);
      }

      if (mTCBases.size() == 0)
      {
         debugMilitaryBuildings("Array update result: we have no Town Center bases left, can't build a Dock anymore.");
         resetCurrentInfo();
         return cNoTownCentersLeft;
      }
      if (needToFindNewBestBase == true)
      {
         if (mCurrentBaseIndex == -1)
         {
            debugMilitaryBuildings("Array update result: We didn't have a TC we were orienting our Dock around, " +
               "need to calculate a new one.");
         }
         else
         {
            debugMilitaryBuildings("Array update result: we lost the TC base we were orienting our Dock around, " +
               "need to calculate a new one.");
            resetCurrentInfo();
         }
         return cRequiredToFindNewBase;
      }
      debugMilitaryBuildings("Array update result: current base ID is still valid.");
      return cCheckForBetterBaseID;
   }

   //==============================================================================
   // analyzeShorelineAreaID
   //==============================================================================
   void analyzeShorelineAreaID(int shorelineAreaID = -1, int dockAreaGroupID = -1)
   {
      for (int j = 0; j < kbAreaGetNumberBorderAreas(shorelineAreaID); j++)
      {
         int borderAreaID = kbAreaGetBorderAreaID(shorelineAreaID, j);
         if (kbAreaGetGroupID(borderAreaID) == dockAreaGroupID ||
             (kbAreaGetType(borderAreaID) == cAreaTypeAmphibious &&
              kbPathAreAreaGroupsConnected(dockAreaGroupID, kbAreaGetGroupID(borderAreaID), cPassabilityWater) == true))
         {
            mShorelineIDs.uniqueAdd(shorelineAreaID); // This area has bordering water areas.
            if (mValidwaterIDs.find(borderAreaID) != -1)
            {
               continue; // Already added.
            }
            debugMilitaryBuildings("      " + borderAreaID);
            mValidwaterIDs.add(borderAreaID);
         }
      }
   }

   //==============================================================================
   // calculateValidWaterIDs
   // Docks always get forced onto water/amphibious areas, not the land areas.
   // Thus we need to have an array of all these kinds of areas that border our shoreline land areas.
   //==============================================================================
   void calculateValidWaterIDs(int shorelineAreaID = -1, int dockAreaGroupID = -1)
   {
      if (mValidwaterIDs.size() != 0)
      {
         aiEchoWarning("Our arrays weren't properly reset before trying to find a new best TC to orient our Docks around.");
      }
      debugMilitaryBuildings("   Valid water area IDs:");
      analyzeShorelineAreaID(shorelineAreaID, dockAreaGroupID);
      int landAreaGroupID = kbAreaGetGroupID(shorelineAreaID);
      for (int i = 0; i < kbAreaGetNumberBorderAreas(shorelineAreaID); i++)
      {
         int borderAreaID = kbAreaGetBorderAreaID(shorelineAreaID, i);
         if (kbAreaGetGroupID(borderAreaID) != landAreaGroupID)
         {
            continue;
         }
         analyzeShorelineAreaID(borderAreaID, dockAreaGroupID);
      }
   }

   //==============================================================================
   // findBestBaseToDock
   //==============================================================================
   void findBestBaseToDock()
   {
      debugMilitaryBuildings("*** Analyzing all our TC bases to see which one is best for fishing. ***");
      int bestIndex = -1;
      float bestDistance = cMaxFloat;
      bool foundBaseThatCanFish = false;

      if (mCurrentBaseIndex != -1)
      {
         BaseDockInfo bestInfo = mBaseDockInfos[mCurrentBaseIndex];
         bestIndex = mCurrentBaseIndex;
         foundBaseThatCanFish = bestInfo.mClosestFishLocation != cInvalidVector;
         bestDistance = bestInfo.mDistanceToShoreline;
         debugMilitaryBuildings("We already have " + kbBaseGetNameByID(cMyID, bestInfo.mBaseID) +
            " selected to orient our Docks around, this base can fish: " + xsBoolToString(foundBaseThatCanFish) +
            ", with distance: " + bestDistance + ".");
      }

      for (int i = 0; i < mTCBases.size(); i++)
      {
         if (i == mCurrentBaseIndex)
         {
            continue;
         }
         BaseDockInfo tempInfo = mBaseDockInfos[i];
         debugMilitaryBuildings("Analyzing base: " + kbBaseGetNameByID(cMyID, tempInfo.mBaseID) + ".");
         if (tempInfo.mValidForDock == false)
         {
            debugMilitaryBuildings("   This base isn't valid to build a Dock for, skipping.");
            continue;
         }
         bool baseCanFish = tempInfo.mClosestFishLocation != cInvalidVector;
         if (foundBaseThatCanFish == true && baseCanFish == false)
         {
            debugMilitaryBuildings("   We already found a base that can fish and this one can't, skipping.");
            continue;
         }
         if (tempInfo.mDistanceToShoreline < bestDistance)
         {
            bestIndex = i;
            bestDistance = tempInfo.mDistanceToShoreline;
            foundBaseThatCanFish = baseCanFish;
            debugMilitaryBuildings("  This base is for now the best base, can fish: " + xsBoolToString(foundBaseThatCanFish) +
               ", with distance: " + bestDistance + ".");
         }
      }

      if (bestIndex == -1)
      {
         debugMilitaryBuildings("Found no valid base for Dock building.");
         return;
      }
      if (bestIndex != mCurrentBaseIndex)
      {
         if (mCurrentBaseIndex != -1)
         {
            resetCurrentInfo(); // Reset our previously saved stuff.
         }
         BaseDockInfo bestInfo = mBaseDockInfos[bestIndex];
         debugMilitaryBuildings("!!! Switching best Dock base to " + kbBaseGetNameByID(cMyID, bestInfo.mBaseID) + " !!!");
         mCurrentBaseIndex = bestIndex;
         mCurrentBaseID = bestInfo.mBaseID;
         mCurrentBaseCanFish = foundBaseThatCanFish;
         gDockAreaGroupID = bestInfo.mDockAreaGroupID;
         gWaterDefendPoint = bestInfo.mWaterDefendPoint;
         gShouldBuildDock = true; // Implied when we're here.
         gShouldFish = foundBaseThatCanFish;
         calculateValidWaterIDs(bestInfo.mShorelineAreaID, bestInfo.mDockAreaGroupID);
         // React to the new defend spot + if we didn't want to create a Dock before we need our defend plan created asap.
         xsRuleIgnoreIntervalOnce("navalDefendManager");
      }
      else
      {
         BaseDockInfo bestInfo = mBaseDockInfos[bestIndex];
         debugMilitaryBuildings(kbBaseGetNameByID(cMyID, bestInfo.mBaseID) + " remains our best Dock base.");
      }
   }

   //==============================================================================
   // updateDockArrays
   //==============================================================================
   void updateDockArrays()
   {
      debugMilitaryBuildings("*** updateDockArrays. ***");
      if (mCurrentBaseIndex == -1)
      {
         aiEchoWarning("Can't call updateDockArrays while we have no current base to fish from.");
         return;
      }
      for (int i = mValidDockIDs.size() - 1; i >= 0; i--)
      {
         if (kbUnitGetIsIDValid(mValidDockIDs[i]) == false || kbUnitGetPlayerID(mValidDockIDs[i]) != cMyID)
         {
            mValidDockIDs.removeIndex(i);
         }
      }
      for (int i = mInvalidDockIDs.size() - 1; i >= 0; i--)
      {
         if (kbUnitGetIsIDValid(mInvalidDockIDs[i]) == false || kbUnitGetPlayerID(mInvalidDockIDs[i]) != cMyID)
         {
            mInvalidDockIDs.removeIndex(i);
         }
      }

      int queryID = useSimpleUnitQuery(cUnitTypeAbstractDock, cMyID, cUnitStateAlive);
      int numResults = kbUnitQueryExecute(queryID);
      for (int i = 0; i < numResults; i++)
      {
         int dockID = kbUnitQueryGetResult(queryID, i);
         if (mValidDockIDs.find(dockID) != -1 || mInvalidDockIDs.find(dockID) != -1)
         {
            continue; // Already analyzezd.
         }
         int areaID = kbUnitGetAreaID(dockID);
         if (mValidwaterIDs.find(areaID) != -1)
         {
            debugMilitaryBuildings("   Adding " + dockID + " to valid Dock IDs.");
            mValidDockIDs.add(dockID);
         }
         else
         {
            debugMilitaryBuildings("   Adding " + dockID + " to invalid Dock IDs.");
            mInvalidDockIDs.add(dockID);
         }
      }
   }
   
   //==============================================================================
   // scoutDockShoreline
   //==============================================================================
   void scoutDockShoreline()
   {
      int[] exploreAreas = new int(0, 0);
      for (int i = 0; i < mShorelineIDs.size(); i++)
      {
         if (kbAreaGetPercentExplored(mShorelineIDs[i]) < 1)
         {
            exploreAreas.add(mShorelineIDs[i]);
         }
      }
      
      if (exploreAreas.size() == 0)
      {
         debugMilitaryBuildings("Don't need to scout for our Dock building since all shorelines are 100 percent explored.");
         if (aiPlanGetIsIDValid(mScoutPlanID) == true)
         {
            debugMilitaryBuildings("Destroying existing shoreline scout plan: " + aiPlanGetName(mScoutPlanID) + ".");
            aiPlanDestroy(mScoutPlanID);
            mScoutPlanID = -1;
         }
         return;
      }
      if (aiPlanGetIsIDValid(mScoutPlanID) == true && aiPlanGetNumberUnits(mScoutPlanID) > 0)
      {
         debugMilitaryBuildings("Already scouting with: " + aiPlanGetName(mScoutPlanID) + ", and it has units.");
         return;
      }

      if (gDefensivelyOverrun == true)
      {
         debugMilitaryBuildings("We're defensively overrun, not taking new units to scout for shorelines.");
         return;
      }

      int[] defendPlanUnits = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypeHumanSoldier);
      if (defendPlanUnits.size() == 0)
      {
         debugMilitaryBuildings("Defend plan has no human soldiers we can steal to scout with.");
         return;
      }

      if (aiPlanGetIsIDValid(mScoutPlanID) == true)
      {
         aiPlanAddUnit(mScoutPlanID, defendPlanUnits[0]);
      }
      else
      {
         mScoutPlanID = aiPlanCreate("Explore shorelines", cPlanExplore, -1, gMilitaryBuildingsCategoryID);
         aiPlanSetPriority(mScoutPlanID, 50);
         aiPlanSetFlag(mScoutPlanID, cPlanFlagCantBeStolenFrom, true);
         aiPlanAddUnitType(mScoutPlanID, cUnitTypeHumanSoldier, 1, 1, 1);
         aiPlanAddUnit(mScoutPlanID, defendPlanUnits[0]);
         debugMilitaryBuildings("Creating explore plan to explore shorelines, plan name: " + aiPlanGetName(mScoutPlanID) + ".");

         aiPlanSetNumberVariableValues(mScoutPlanID, cExplorePlanExploreAreaIDs, exploreAreas.size());
         for (int i = 0; i < exploreAreas.size(); i++)
         {
            debugMilitaryBuildings("We want to scout areaID: " + exploreAreas[i] + ".");
            aiPlanSetVariableInt(mScoutPlanID, cExplorePlanExploreAreaIDs, i, exploreAreas[i]);
         }
      }
   }
};
extern DockManager gDockManager;

//==============================================================================
// dockManagerMonitor
//==============================================================================
rule dockManagerMonitor
group defaultArchaicRules
inactive
minInterval 15
{
   if (gMapInfo.mHasWater == false)
   {
      debugMilitaryBuildings("Map has no water, disabling dockManagerMonitor.");
      xsDisableRule("dockManagerMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticDockBuilding) == false)
   {
      return;
   }
   
   debugMilitaryBuildings("--- Running Rule dockManagerMonitor. ---");

   int currentAge = kbPlayerGetAge(cMyID);
   // Prevent our Berserk going to scout shorelines and delaying our Temple by too much.
   if (cMyCulture == cCultureNorse && currentAge == cAge1 && kbUnitCount(cUnitTypeTemple, cMyID, cUnitStateAlive) == 0)
   {
      debugMilitaryBuildings("Not building a Dock right now because we are Norse and have no Temple while in the Archaic age.");
      return;
   }

   int currentState = gDockManager.updateBaseArrays();

   if (currentState == cNoTownCentersLeft)
   {
      return;
   }
   bool checkForBetterBaseID = currentState == cRequiredToFindNewBase;
   if (currentState == cCheckForBetterBaseID)
   {
      if (gDockManager.mCurrentBaseCanFish == true)
      {
         debugMilitaryBuildings("Our current base can already fish, no need to potentially switch bases.");
      }
      else if (gDockManager.mTCBases.size() == 1)
      {
         debugMilitaryBuildings("Our current base can't fish but we only have 1 TC base, no point in checking for better options.");
      }
      else
      {
         debugMilitaryBuildings("Our current base can't fish, let's see if we can switch to a base that can.");
         checkForBetterBaseID = true;
      }
   }

   if (checkForBetterBaseID == true)
   {
      // Search for a better base to orient everything around.
      gDockManager.findBestBaseToDock();
      if (gDockManager.mCurrentBaseIndex == -1)
      {
         return; // This means we gShouldBuildDock == false.
      }
   }
   // Finally! We have something we can use to orient our Docks around.
   BaseDockInfo currentInfo = gDockManager.mBaseDockInfos[gDockManager.mCurrentBaseIndex];

   // Do the scouting if required.
   gDockManager.scoutDockShoreline();
   // Get all our Docks picked up.
   gDockManager.updateDockArrays();

   int numCurrentValidDocks = gDockManager.mValidDockIDs.size();
   int numCurrentBuildPlans = gDockManager.mDockBuildPlans.size();
   int currentDockCount = numCurrentValidDocks + numCurrentBuildPlans;
   debugMilitaryBuildings("We already have " + currentDockCount + " alive/planned Docks.");
   // Don't create Docks while we're under serious attack.
   if (gDefensivelyOverrun == true)
   {
      debugMilitaryBuildings("Not building a Dock right now because we're in serious danger on the land.");
      return;
   }

   if (currentDockCount >= 1 && currentAge == cAge1)
   {
      debugMilitaryBuildings("Not building a Dock right now because we already have one and are in the Archaic age.");
      return;
   }
   // Not completely reliable since we don't do a path, but oh well.
   float dangerRating = kbAreaGetDangerLevel(currentInfo.mShorelineAreaID, false);
   if (dangerRating >= aiGetExploreDangerThreshold())
   {
      debugMilitaryBuildings("Not building a Dock right now because our wanted shoreline area is too dangerous.");
      return;
   }

   int numDocksToBuild = 1;
   int maxNavalPop = aiGetNavalMilitaryPop();
   switch (cDifficultyCurrent)
   {
      case cDifficultyEasy:
      case cDifficultyModerate:
      {
         // Max 1 Dock for these.
         debugMilitaryBuildings("We want a minimum of " + numDocksToBuild + " Docks as a baseline.");
         numDocksToBuild -= currentDockCount;
         break;
      }
      case cDifficultyHard:
      case cDifficultyTitan:
      {
         if (currentAge >= cAge3)
         {
            numDocksToBuild = 2; // Baseline of 2 from Heroic onwards.
         }
         int numDocksNeeded = ceil(xsIntToFloat(maxNavalPop) / 15.0); 
         debugMilitaryBuildings("We want a minimum of " + numDocksToBuild + " Docks as a baseline.");
         debugMilitaryBuildings("We have a need of " + numDocksNeeded + " Docks for naval unit production.");
         // Every 15 naval military pop we want another Dock.
         numDocksToBuild = max(numDocksToBuild, numDocksNeeded);
         numDocksToBuild -= currentDockCount;
         break;
      }
      case cDifficultyExtreme:
      case cDifficultyLegendary:
      {
         if (currentAge >= cAge3)
         {
            numDocksToBuild = 2; // Baseline of 2 from Heroic onwards.
         }
         if (currentAge >= cAge4)
         {
            numDocksToBuild = 3; // Baseline of 3 from Mythic onwards.
         }
         // Every 10 naval military pop we want another Dock.
         int numDocksNeeded = ceil(xsIntToFloat(maxNavalPop) / 10.0);
         debugMilitaryBuildings("We want a minimum of " + numDocksToBuild + " Docks as a baseline.");
         debugMilitaryBuildings("We have a need of " + numDocksNeeded + " Docks for naval unit production.");
         numDocksToBuild = max(numDocksToBuild, numDocksNeeded);
         numDocksToBuild -= currentDockCount;
         break;
      }
   }

   debugMilitaryBuildings("We want to build " + numDocksToBuild + " new Docks.");

   for (int i = 0; i < numDocksToBuild; i++)
   {
      int planID = createDockBuildPlan(kbAreaGetCenter(currentInfo.mShorelineAreaID), currentInfo.mWaterDefendPoint);
      gDockManager.mDockBuildPlans.add(planID);
      aiPlanSetEventHandler(planID, cPlanEventStateChange, "dockHandler");
   }
}