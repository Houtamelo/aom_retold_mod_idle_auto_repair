//==============================================================================
/* techs.xs

   This file contains stuffs for managing techs including age upgrades.

*/
//==============================================================================

//==============================================================================
/* ageUpgradeMonitor
   In this rule we decide what age up option we must go for,
   and what priority we must give it.
*/
//==============================================================================
rule ageUpgradeMonitor
inactive
group defaultArchaicRules
minInterval 10
{
   int currentAge = kbPlayerGetAge(cMyID);
   int maxAge = aiGetMaxAge(cMyID);
   if (maxAge == cAge5)
   {
      maxAge = cAge4; // Wonder age must be excluded here.
   }
   if (currentAge >= maxAge)
   {
      debugTechs("Disabling ageUpgradeMonitor because we're at our maximum allowed age.");
      xsDisableRule("ageUpgradeMonitor");
      return;
   }

   if (checkStrategyFlag(cStrategyFlagAutoAgeUp) == false || getUnit(cUnitTypeAbstractSocketedTownCenter) == -1)
   {
      if (aiPlanGetIsIDValid(gAgeUpResearchPlan) == true)
      {
         aiPlanDestroy(gAgeUpResearchPlan);
         gAgeUpResearchPlan = -1;
      }
      return;
   }

   debugTechs("--- Running Rule ageUpgradeMonitor. ---");

   // Don't wait when we're sieger because then our trait goes messed up and it appears broken.
   if (cDifficultyCurrent <= cDifficultyModerate && (currentAge >= cAge3 || cPersonalityCurrent != cPersonalitySieger))
   {
      // If there is a human in the game we can only age up if another player has done so before us.
      // If there is no human left in the game we just keep aging up.
      // And if there is another AI on higher difficulty that has already aged up we follow suit, not waiting on the human.
      if (getHighestPlayerAge() <= currentAge && getHumanPresentInGame() == true)
      {
         debugTechs("Delaying age up because no player in the game has aged up before us.");
         return;
      }
   }

   int ageUpPriority = 48;
   int currentTime = xsGetTime();
   int fastestAgeUpTimeForNextAge = gFastestAgeUpTimes[currentAge + 1];

   if (fastestAgeUpTimeForNextAge != -1)
   {
      debugTechs("We're behind at least 1 age in comparison to another player in the age, we may need to bump our age up priority.");
      int timeDisparity = currentTime - fastestAgeUpTimeForNextAge;
      debugTechs("The fastest age up to " + getAgeName(currentAge + 1) + " was at " +
         turnNumberIntoTimeDisplay(fastestAgeUpTimeForNextAge) + ", we're " + turnNumberIntoTimeDisplay(timeDisparity) + " behind.");
      int maxDisparity = selectByDifficulty(15, 12, 9, 3, 3, 3) * 60;
      if (cPersonalityCurrent == cPersonalityEconomist)
      {
         maxDisparity *= 0.8; // 20% faster bumping the priority.
      }
      debugTechs("We're allowed to be max " + turnNumberIntoTimeDisplay(maxDisparity) + " behind.");
      if (timeDisparity > maxDisparity)
      {
         debugTechs("We're behind in ages for too long, bumping age up priority now.");
         ageUpPriority = 55;
      }
   }
   else
   {
      debugTechs("We're not behind an age, not increasing age up priority because of that.");
   }

   // If we've been in the current age for thresholdTime we start focusing on going to the next age.
   int thresholdTime = selectByDifficulty(15, 12, 9, 6, 5, 4) * 60;
   if (cPersonalityCurrent == cPersonalityEconomist)
   {
      debugTechs("Reducing threshold time for next age by 20 percent since we're the Economist personality.");
      thresholdTime *= 0.8; // 20% faster bumping the priority.
   }
   else if (cPersonalityCurrent == cPersonalitySieger && currentAge == cAge2)
   {
      // Go to Heroic very fast.
      debugTechs("Reducing threshold time for next age to 60 seconds since we're the Sieger personality and are currently in Classical.");
      thresholdTime = 60;
   }
   if (gAgeUpTimes[currentAge] + thresholdTime < currentTime)
   {
      debugTechs("We've been in the current age for " + turnNumberIntoTimeDisplay(currentTime - gAgeUpTimes[currentAge]) +
         ", this is longer than our threshold of " + turnNumberIntoTimeDisplay(thresholdTime) + ", bumping age up priority now.");
      ageUpPriority = 55;
   }
   else
   {
      debugTechs("We've been in the current age for " + turnNumberIntoTimeDisplay(currentTime - gAgeUpTimes[currentAge]) +
         ", our threshold for increasing age up prority is " + turnNumberIntoTimeDisplay(thresholdTime) + ".");
   }

   if (gDefensivelyOverrun == true)
   {
      debugTechs("We're currently being defensively overrun, keep age up priority low until we deal with this threat.");
      ageUpPriority = 48;
   }
   else if (gAttackManager.mKOTHPanic == true)
   {
      debugTechs("We're currently panicking about losing the game to KOTH timer, keep age up priority low for now.");
      ageUpPriority = 48;
   }
   
   debugTechs("Our ageUpPriority is: " + ageUpPriority + ".");
   if (aiPlanGetIsIDValid(gAgeUpResearchPlan) == true)
   {
      aiPlanSetPriority(gAgeUpResearchPlan, ageUpPriority);
   }
   else // We have no plan yet so create one.
   {
      // We take overrides into account here only if they are valid.
      // The reason for that is if you want to test let's say Hera for Zeus but you have other AIs in the game that aren't Zeus.
      // That Hera override shouldn't block all others from going to Mythic.
      int minorGod = -1;
      switch (currentAge)
      {
         case cAge1:
         {
            if (gOverrideClassicalMinorGod >= 0 && kbTechGetStatus(gOverrideClassicalMinorGod, false) == cTechStatusObtainable)
            {
               minorGod = gOverrideClassicalMinorGod;
            }
            break;
         }
         case cAge2:
         {
            if (gOverrideHeroicMinorGod >= 0 && kbTechGetStatus(gOverrideHeroicMinorGod, false) == cTechStatusObtainable)
            {
               minorGod = gOverrideHeroicMinorGod;
            }
            break;
         }
         case cAge3:
         {
            if (gOverrideMythicMinorGod >= 0 && kbTechGetStatus(gOverrideMythicMinorGod, false) == cTechStatusObtainable)
            {
               minorGod = gOverrideMythicMinorGod;
            }
            break;
         }
      }
      if (minorGod >= 0)
      {
         debugTechs("Override age upgrade with: " + kbTechGetName(minorGod) + ".");
      }
      else
      {
         minorGod = aiGetAgeUpListByIndex(currentAge + 1, xsRandBool() == true ? 0 : 1); // Randomly pick an index.
      }

      if (minorGod < 0) // We somehow failed to get a valid age up option so chose the one at index 0.
      {
         minorGod = aiGetAgeUpListByIndex(currentAge + 1, 0);
         if (minorGod < 0)
         {
            aiEchoWarning("We completely failed to pick an age up option to make an age up plan for, how could this happen?");
            return;
         }
         aiEchoWarning("We failed to pick an age up option to create a plan for, " +
            "choosing first option from failsafe: " + kbTechGetName(minorGod) + ".");
      }

      // We have managed to pick a minor god (or got defaulted to index 0).
      // So let's create the research plan. If we somehow still don't have one we just don't make a plan.
      // We use createSimpleResearchPlanSpecificBuildingPUID directly here because we make these plans before we meet the prereqs.
      gAgeUpResearchPlan = createSimpleResearchPlanSpecificBuildingPUID(minorGod, cUnitTypeAbstractSocketedTownCenter, ageUpPriority);
      aiPlanSetEventHandler(gAgeUpResearchPlan, cPlanEventStateChange, "ageUpPlanHandler");
   }
}

//==============================================================================
// getWeightsPerCulture
//==============================================================================
void getWeightsPerCulture(ref int armoryUpgradeChance, ref int lineUpgradeChance, ref int mythUpgradeChance,
   ref int levyUpgradeChance, int currentAge = 0)
{
   switch (cMyCulture)
   {
      case cCultureGreek:
      {
         armoryUpgradeChance = 35;
         lineUpgradeChance = 40;
         mythUpgradeChance = 10 + (currentAge * 5);
         if (currentAge == cAge3)
         {
            levyUpgradeChance = 10;
         }
         else
         {
            levyUpgradeChance = 25;
         }
         break;
      }
      case cCultureEgyptian:
      {
         armoryUpgradeChance = 35;
         lineUpgradeChance = 50; // There are more line upgrades for Egyptians.
         mythUpgradeChance = 10 + (currentAge * 5);
         if (currentAge == cAge3)
         {
            levyUpgradeChance = 10;
         }
         else
         {
            levyUpgradeChance = 25;
         }
         break;
      }
      case cCultureNorse:
      {
         armoryUpgradeChance = 35;
         if (cMyCiv == cCivThor)
         {
            armoryUpgradeChance = 45;
         }
         lineUpgradeChance = 40;
         mythUpgradeChance = 10 + (currentAge * 5);
         if (currentAge == cAge3)
         {
            levyUpgradeChance = 10;
         }
         else
         {
            levyUpgradeChance = 25;
         }
         break;
      }
      case cCultureAtlantean:
      {
         armoryUpgradeChance = 35;
         lineUpgradeChance = 40;
         mythUpgradeChance = 10 + (currentAge * 5);
         if (currentAge == cAge3)
         {
            levyUpgradeChance = 10;
         }
         else
         {
            levyUpgradeChance = 25;
         }
         break;
      }
      case cCultureChinese:
      {
         armoryUpgradeChance = 35;
         lineUpgradeChance = 40;
         mythUpgradeChance = 10 + (currentAge * 5);
         if (currentAge == cAge3)
         {
            levyUpgradeChance = 10;
         }
         else
         {
            levyUpgradeChance = 25;
         }
         break;
      }  
      case cCultureJapanese:
      {
         armoryUpgradeChance = 35;
         lineUpgradeChance = 40;
         mythUpgradeChance = 10 + (currentAge * 5);
         if (currentAge == cAge3)
         {
            levyUpgradeChance = 10;
         }
         else
         {
            levyUpgradeChance = 25;
         }
         break;
      }
      case cCultureAztec:
      {
         armoryUpgradeChance = 35;
         lineUpgradeChance = 40;
         mythUpgradeChance = 10 + (currentAge * 5);
         if (currentAge == cAge3)
         {
            levyUpgradeChance = 10;
         }
         else
         {
            levyUpgradeChance = 25;
         }
         break;
      }
   }
}

//==============================================================================
// haveForcedMilitaryTechnologyToResearch
//==============================================================================
bool haveForcedMilitaryTechnologyToResearch(int currentAge = -1)
{
   switch (cMyCulture)
   {
      case cCultureGreek:
      {
         if (cMyCiv == cCivZeus)
         {
            if (currentAge >= cAge3 && kbTechGetStatus(cTechOlympianParentage) == cTechStatusObtainable)
            {
               gMilitaryResearchPlanID = researchSimpleTech(cTechOlympianParentage);
               return true;
            }
         }
         if (currentAge >= cAge4 && kbTechGetStatus(cTechForgeOfOlympus) == cTechStatusObtainable)
         {
            gMilitaryResearchPlanID = researchSimpleTech(cTechForgeOfOlympus);
            return true;
         }
         break;
      }
      case cCultureEgyptian:
      {
         if (kbTechGetStatus(cTechHandsOfThePharaoh) == cTechStatusObtainable)
         {
            gMilitaryResearchPlanID = researchSimpleTech(cTechHandsOfThePharaoh);
            return true;
         }
         if (cMyCiv == cCivRa)
         {
            if (kbTechGetStatus(cTechSkinOfTheRhino) == cTechStatusObtainable)
            {
               gMilitaryResearchPlanID = researchSimpleTech(cTechSkinOfTheRhino);
               return true;
            }
         }
         break;
      }
      case cCultureChinese:
      {
         if (kbTechGetStatus(cTechDivineLight) == cTechStatusObtainable)
         {
            gMilitaryResearchPlanID = researchSimpleTech(cTechDivineLight);
            return true;
         }
         if (cMyCiv == cCivFuxi)
         {
            if (kbTechGetStatus(cTechPeachOfImmortality) == cTechStatusObtainable)
            {
               gMilitaryResearchPlanID = researchSimpleTech(cTechPeachOfImmortality);
               return true;
            }
         }
         break;
      }
   }

   if (kbTechGetStatus(cTechBallistics) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechBallistics);
      return true;
   }

   return false;
}

///////////////////////////////
const int cDebugNotDesired = -1;
const int cArmoryUpgrades = 0;
const int cDockUpgrades = 1;
const int cDockMythUpgrades = 2;
const int cLevyUpgrades = 3;
const int cLineUpgrades = 4;
const int cMythUpgrades = 5;
//==============================================================================
// fillOutMilitaryUpgradeArrays
//==============================================================================
void fillOutMilitaryUpgradeArrays(ref int[] array, int mode = -2)
{
   array.clear();
   // ATTENTION: we get all obtainable technologies which means we need to do a lot of filtering + category sorting.
   // The order in which this is done MATTERS, some techs fulfill the condition of multiple types but should be added to the correct one.
   int[] obtainableTechs = kbTechTreeGetAllObtainableTechnologies(false);
   int numObtainableTechs = obtainableTechs.size();
   for (int i = 0; i < numObtainableTechs; i++)
   {
      int currentTechID = obtainableTechs[i];
      if (kbTechGetFlag(currentTechID, cTechFlagCountsTowardMilitaryScore) == false)
      {
         if (mode == cDebugNotDesired)
         {
            debugTechs(kbTechGetName(currentTechID) + " is not a desired technology for this logic, " +
               "because it doesn't have the military flag.");
         }
         continue;
      }
      
      // Specific exclusions that we handle via custom logic or just don't want.
      if (currentTechID == cTechMasons || currentTechID == cTechArchitects || currentTechID == cTechFortifiedTownCenter ||
          currentTechID == cTechOmniscience || currentTechID == cTechDraftHorses || currentTechID == cTechEngineers ||
          currentTechID == cTechTemporalChaos || currentTechID == cTechOracle || currentTechID == cTechYdalir ||
          currentTechID == cTechFeastsOfRenown || currentTechID == cTechClairvoyance || currentTechID == cTechMountainousMight ||
          currentTechID == cTechEastWind || currentTechID == cTechSkyFire || currentTechID == cTechDroughtShips || 
          currentTechID == cTechAdvancedFortifications || currentTechID == cTechSacredCustodians || currentTechID == cTechKagura)
      {
         if (mode == cDebugNotDesired)
         {
            debugTechs(kbTechGetName(currentTechID) + " is not a desired technology for this logic, because it's specifically excluded.");
         }
         continue;
      }
      if (currentTechID == cTechFreyrsGift)
      {
         float[] cost = kbTechGetCost(cTechFreyrsGift);
         if (cost[cResourceFavor] > 30.0)
         {
            if (mode == cDebugNotDesired)
            {
               debugTechs(kbTechGetName(currentTechID) + " is not a desired technology for this logic because it's still " + 
                  "costing more than 30.0 favor.");
            }
            continue;
         }
      }
      if (currentTechID == cTechRingOath && kbPlayerGetAge(cMyID) < cAge4)
      {
         if (mode == cDebugNotDesired)
         {
            debugTechs(kbTechGetName(currentTechID) + " is not a desired technology for this logic because we're not in Mythic yet.");
         }
         continue;
      }

      // towerOffensiveUpgradeMonitor handles these upgrades.
      if (kbProtoUnitCanResearch(cUnitTypeSentryTower, currentTechID) == true)
      {
         if (mode == cDebugNotDesired)
         {
            debugTechs(kbTechGetName(currentTechID) + " is not a desired technology for this logic, because it's a Tower tech.");
         }
         continue;
      }

      // wallUpgradeMonitor handles these upgrades.
      if (kbProtoUnitCanResearch(cUnitTypeWallLong, currentTechID) == true)
      {
         if (mode == cDebugNotDesired)
         {
            debugTechs(kbTechGetName(currentTechID) + " is not a desired technology for this logic, because it's a Wall tech.");
         }
         continue;
      }

      // The order here matters!
      if (kbProtoUnitCanResearch(cUnitTypeDock, currentTechID) == true)
      {
         if (kbTechGetFlag(currentTechID, cTechFlagMythTech) == true)
         {
            if (mode == cDockMythUpgrades)
            {
               debugTechs("Added " + kbTechGetName(currentTechID) + " to Dock myth tech array.");
               array.add(currentTechID);
            }
         }
         else if (mode == cDockUpgrades)
         {
            debugTechs("Added " + kbTechGetName(currentTechID) + " to Dock tech array.");
            array.add(currentTechID);
         }
         continue;
      }
      // Thor's 3 unique upgrades we categorize as Armory upgrades instead of Myth upgrades.
      else if (kbTechGetFlag(currentTechID, cTechFlagMythTech) == true && currentTechID != cTechDwarvenWeapons &&
               currentTechID != cTechMeteoricIronArmor && currentTechID != cTechDragonscaleShields)
      {
         if (mode == cMythUpgrades)
         {
            // Herbal Medicine is available from Archaic but it buffs Sages which are Heroic.
            if (cMyCiv == cCivShennong && currentTechID == cTechHerbalMedicine && kbPlayerGetAge(cMyID) < cAge3)
            {
               continue;
            }
            debugTechs("Added " + kbTechGetName(currentTechID) + " to Myth tech array.");
            array.add(currentTechID);
         }
         continue;
      }
      else if (kbProtoUnitCanResearch(gArmoryUnit, currentTechID) == true)
      {
         if (mode == cArmoryUpgrades)
         {
            debugTechs("Added " + kbTechGetName(currentTechID) + " to Armory tech array.");
            array.add(currentTechID);
         }
         continue;
      }
      else
      {
         // A line/levy upgrade should always only have data effects, but guard against it anyway.
         if (kbTechGetEffectType(currentTechID, 0) != cEffectTypeData)
         {
            aiEchoWarning("Found a technology that we classify as either line or levy which has a non data effect as the first effect: " +
               kbTechGetName(currentTechID) + ".");
            continue;
         }
         // No need to call kbTechGetDataEffectType if it's not the right mode.
         if (mode != cLevyUpgrades && mode != cLineUpgrades)
         {
            continue;
         }
         int effectType = kbTechGetDataEffectType(currentTechID, 0);
         if (effectType == cDataEffectTrainPoints)
         {
            if (mode == cLevyUpgrades)
            {
               debugTechs("Added " + kbTechGetName(currentTechID) + " to levy tech array.");
               array.add(currentTechID);
            }
            continue;
         }
         else
         {
            if (mode == cLineUpgrades)
            {
               debugTechs("Added " + kbTechGetName(currentTechID) + " to line tech array.");
               array.add(currentTechID);
            }
            continue;
         }
      }
   }
}

//==============================================================================
// militaryUpgradeManager
//==============================================================================
rule militaryUpgradeManager
inactive
group defaultClassicalRules
minInterval 30
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("militaryUpgradeManager");
      return;
   }
   static int progress = 0;
   static int lastAge = -1;
   static int[] techIDs = default;
   static int[] upgradeUnitTypes = default;
   static float[] unitTypesAliveValue = default;
   static float[] unitTypesPlannedValue = default;
   static int chosenSegment = -1;
   static int researchStartTime = -1;
   const float cLowPrioArmyPercentage = 0.2;
   // Set the research plan to higher priority quicker for overwhelmers, so they can arrive with a big upgraded army.
   float cHighPrioArmyPercentage = cPersonalityCurrent == cPersonalityOverwhelmer ? 0.4 : 0.6;

   if (checkStrategyFlag(cStrategyFlagAutoResearchMilitaryUpgrades) == false)
   {
      progress = 0;
      chosenSegment = -1;
      researchStartTime = -1;
      if (aiPlanGetIsIDValid(gMilitaryResearchPlanID) == true && aiPlanGetState(gMilitaryResearchPlanID) != cPlanStateResearch)
      {
         aiPlanDestroy(gMilitaryResearchPlanID);
         gMilitaryResearchPlanID = -1;
      }
      return;
   }
   debugTechs("--- Running Rule militaryUpgradeManager. ---");
    
   int currentTime = xsGetTime();
   int currentMilitaryPop = aiGetCurrentMilitaryPop();
   int allowedMilitaryPop = aiGetMilitaryPop();

   // If we already have a plan we need to check up on it.
   if (aiPlanGetIsIDValid(gMilitaryResearchPlanID) == true)
   {
      if (aiPlanGetState(gMilitaryResearchPlanID) == cPlanStateResearch)
      {
         debugTechs("We're currently already researching a military technology, plan name: " +
            aiPlanGetName(gMilitaryResearchPlanID) + ".");
         return;
      }
      if (currentTime > researchStartTime + 120)
      {
         debugTechs("We had a research plan that didn't go into state research for 2 minutes, destroying it. " +
            "Plan name: " + aiPlanGetName(gMilitaryResearchPlanID) + ".");
         // Don't use a setState here because then our handler will fire next frame and mess our new plan up.
         aiPlanDestroy(gMilitaryResearchPlanID);
         gMilitaryResearchPlanID = -1;
         researchStartTime = -1;
         // Instantly start the chain again.
      }
      else // Plan is valid but not yet researching.
      {
         // Low prio on upgrades when we're low on military, high prio when we have a lot of military.
         int prio = 50;
         if (currentMilitaryPop >= 1 && allowedMilitaryPop >= 1)
         {
            float percentageMilitaryPopAliveQueued = xsIntToFloat(currentMilitaryPop) / xsIntToFloat(allowedMilitaryPop);
            if (percentageMilitaryPopAliveQueued < cLowPrioArmyPercentage)
            {
               prio = 49;
            }
            else if (percentageMilitaryPopAliveQueued > cHighPrioArmyPercentage)
            {
               prio = 51;
            }
         }
         int currentPrio = aiPlanGetPriority(gMilitaryResearchPlanID);
         debugTechs("We currently already have a plan to research a military technology, plan name: " +
            aiPlanGetName(gMilitaryResearchPlanID) + ", priority: " + currentPrio + ".");
         if (currentPrio != prio)
         {
            aiPlanSetPriority(gMilitaryResearchPlanID, prio);
            debugTechs("   Adjusting its priority to " + prio + ".");
         }
         return;
      }
   }
   
   if (progress == 0)
   {
      int currentAge = kbPlayerGetAge(cMyID);
      // Don't take forced technologies into account for humanoid since they could cost favor.
      if (cPersonalityCurrent != cPersonalityHumanoid && haveForcedMilitaryTechnologyToResearch(currentAge) == true)
      {
         aiPlanSetEventHandler(gMilitaryResearchPlanID, cPlanEventStateChange, "resetMilitaryResearchPlan");
         debugTechs("Not running through the default logic because we have a forced technology to research.");
         return;
      }

      if (lastAge == -1) // First run.
      {
         lastAge = currentAge;
      }
      static bool haveAllArmoryUpgrades = false;
      static bool haveAllLevyUpgrades = false;
      static bool haveAllLineUpgrades = false;
      static bool haveAllMythUpgrades = false;
      if (currentAge != cAge5 && lastAge < currentAge)
      {
         // We've aged up and unlocked more upgrades, reset the bools.
         debugTechs("Resetting haveAll bools because we aged up.");
         haveAllArmoryUpgrades = false;
         haveAllLevyUpgrades = false;
         haveAllLineUpgrades = false;
         haveAllMythUpgrades = cPersonalityCurrent == cPersonalityHumanoid ? true : false;
      }
      lastAge = currentAge;

      if (haveAllArmoryUpgrades == true && haveAllLineUpgrades == true && haveAllMythUpgrades == true && haveAllLevyUpgrades == true)
      {
         if (currentAge >= cAge4)
         {
            debugTechs("Disabling rule militaryUpgradeManager because we have all technologies from it and are in Mythic or Wonder age.");
            xsDisableRule("militaryUpgradeManager");
         }
         else
         {
            debugTechs("We have all upgrades from militaryUpgradeManager for now, waiting until we age up again.");
         }
         return;
      }

      // Base values.
      int armoryUpgradeChance = 0;
      int lineUpgradeChance = 0;
      int mythUpgradeChance = 0;
      int levyUpgradeChance = 0;
      getWeightsPerCulture(armoryUpgradeChance, lineUpgradeChance, mythUpgradeChance, levyUpgradeChance, currentAge);
      if (cPersonalityCurrent == cPersonalityHumanoid)
      {
         mythUpgradeChance = 0;
      }
      // Building an Armory could take as long as the interval of this rule, so guard against rolling that when not available atm.
      bool haveArmory = true;
      if (kbUnitCount(gArmoryUnit, cMyID, cUnitStateAlive) <= 0)
      {
         haveArmory = false;
         armoryUpgradeChance = 0;
      }

      // These will hold the "ranges" that the randInt can roll in.
      int armorySegmentCutoff = -1;
      int lineSegmentCutoff = -1;
      int mythSegmentCutoff = -1;
      int levySegmentCutoff = -1;

      int totalRoll = 0; // All ranges combined.
      if (haveAllArmoryUpgrades == false)
      {
         armorySegmentCutoff = totalRoll + armoryUpgradeChance;
         totalRoll += armoryUpgradeChance;
      }
      if (haveAllLineUpgrades == false)
      {
         lineSegmentCutoff = totalRoll + lineUpgradeChance;
         totalRoll += lineUpgradeChance;
      }
      if (haveAllMythUpgrades == false)
      {
         mythSegmentCutoff = totalRoll + mythUpgradeChance;
         totalRoll += mythUpgradeChance;
      }
      if (haveAllLevyUpgrades == false)
      {
         levySegmentCutoff = totalRoll + levyUpgradeChance;
         totalRoll += levyUpgradeChance;
      }

      debugTechs("totalRoll: " + totalRoll + ".");
      if (totalRoll == 0)
      {
         debugTechs("We don't have everything yet but our totalRoll == 0 because of other limitations (like no Armory), quiting.");
         return;
      }
      debugTechs("armorySegmentCutoff: " + armorySegmentCutoff + ".");
      debugTechs("lineSegmentCutoff: " + lineSegmentCutoff + ".");
      debugTechs("mythSegmentCutoff: " + mythSegmentCutoff + ".");
      debugTechs("levySegmentCutoff: " + levySegmentCutoff + ".");

      // We roll between 1 and totalRoll.
      int rand = xsRandInt(1, totalRoll);
      debugTechs("rand: " + rand + ".");

      if (haveArmory == true && armorySegmentCutoff != -1 && rand <= armorySegmentCutoff)
      {
         debugTechs("Chosen to research an armory upgrade!");
         chosenSegment = cArmoryUpgrades;
      }
      else if (lineSegmentCutoff != -1 && rand <= lineSegmentCutoff)
      {
         debugTechs("Chosen to research a line upgrade!");
         chosenSegment = cLineUpgrades;
      }
      else if (mythSegmentCutoff != -1 && rand <= mythSegmentCutoff)
      {
         debugTechs("Chosen to research a myth upgrade!");
         chosenSegment = cMythUpgrades;
      }
      else if (levySegmentCutoff != -1 && rand <= levySegmentCutoff)
      {
         debugTechs("Chosen to research a levy upgrade!");
         chosenSegment = cLevyUpgrades;
      }
      else
      {
         aiEchoWarning("We should never not find a segment in militaryUpgradeManager!");
         return;
      }

      /*
      debugTechs("cDebugNotDesired");
      fillOutMilitaryUpgradeArrays(techIDs, cDebugNotDesired);
      debugTechs("cArmoryUpgrades");
      fillOutMilitaryUpgradeArrays(techIDs, cArmoryUpgrades);
      debugTechs("cDockUpgrades");
      fillOutMilitaryUpgradeArrays(techIDs, cDockUpgrades);
      debugTechs("cLevyUpgrades");
      fillOutMilitaryUpgradeArrays(techIDs, cLevyUpgrades);
      debugTechs("cLineUpgrades");
      fillOutMilitaryUpgradeArrays(techIDs, cLineUpgrades);
      debugTechs("cMythUpgrades");
      fillOutMilitaryUpgradeArrays(techIDs, cMythUpgrades);
      */

      fillOutMilitaryUpgradeArrays(techIDs, chosenSegment);
      if (techIDs.size() == 0)
      {
         debugTechs("We rolled segment " + chosenSegment + " but we can't reserach any technologies in that segment anymore. " +
            "Marking it as completed for now.");
         switch (chosenSegment)
         {
            case cArmoryUpgrades:
            {
               haveAllArmoryUpgrades = true;
               break;
            }
            case cLineUpgrades:
            {
               haveAllLineUpgrades = true;
               break;
            }
            case cMythUpgrades:
            {
               haveAllMythUpgrades = true;
               break;
            }
            case cLevyUpgrades:
            {
               haveAllLevyUpgrades = true;
               break;
            }
         }
         // Go again instantly.
         xsRuleIgnoreIntervalOnce("militaryUpgradeManager");
         progress = 0;
         return;
      }

      if (chosenSegment == cLineUpgrades)
      {
         progress = 1;
      }
      else
      {
         progress = 2; // Skip progress 1 as it's not needed.
      }
      // Go again instantly.
      xsRuleIgnoreIntervalOnce("militaryUpgradeManager");
      return;
   }

   if (progress == 1)
   {
      // Only line needs this analysis.
      if (chosenSegment != cLineUpgrades)
      {
         aiEchoWarning("We should never hit progress == 1 when we're not researching a line upgrade in militaryUpgradeManager.");
         progress = 2;
         xsRuleIgnoreIntervalOnce("militaryUpgradeManager");
         return;
      }

      int numTechs = techIDs.size();
      upgradeUnitTypes = new int(numTechs, -1);
      unitTypesAliveValue = new float(numTechs, 0.0);
      unitTypesPlannedValue = new float(numTechs, 0.0);
      for (int i = 0; i < numTechs; i++)
      {
         // We assume here that the first effect of a line upgrade always holds the correct unit type.
         upgradeUnitTypes[i] = kbTechGetDataEffectTargetID(techIDs[i], 0);
         // We must query all alive units here since if we're affecting abstract types we can have multiple unit types that don't
         // have the same AI cost. And kbAICostGetUnitCost takes current health % into account for which we need the unitID.
         int queryID = useSimpleUnitQuery(upgradeUnitTypes[i], cMyID, cUnitStateAlive);
         int numResults = kbUnitQueryExecute(queryID);
         for (int iResult = 0; iResult < numResults; iResult++)
         {
            int unitID = kbUnitQueryGetResult(queryID, iResult);
            // Units that cost more give more value.
            float unitAICost = kbAICostGetUnitCost(unitID);
            // Keep the numbers relatively low so that the weights make sense.
            unitTypesAliveValue[i] = unitTypesAliveValue[i] + (unitAICost / 100.0);
         }
         debugTechs("We have " + unitTypesAliveValue[i] + " alive unit value for " + kbTechGetName(techIDs[i]) + ".");

         // Loop through all our human unit maintain plans, finding plans that train a puid that matches what we upgrade.
         for (int iPlan = gMaintainPlanHumanArcherStartIndex; iPlan < gMaintainPlanHeroStartIndex; iPlan++)
         {
            int planID = gArmyUnitMaintainPlans[iPlan];
            if (aiPlanGetIsIDValid(planID) == false)
            {
               continue;
            }
            int planUnitType = aiPlanGetVariableInt(planID, cTrainPlanUnitType, 0);
            // Some line upgrades effect single unit types, find that out here.
            if (upgradeUnitTypes[i] < cNumberProtoUnits)
            {
               if (upgradeUnitTypes[i] == planUnitType)
               {
                  int numExistingUnits = kbUnitCount(planUnitType, cMyID, cUnitStateAlive);
                  int numLeftToTrain = max(0, aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) - numExistingUnits);
                  // Keep the numbers relatively low so that the weights make sense.
                  float aiCost = numLeftToTrain * (kbAICostGetProtoUnitCost(planUnitType) / 100.0);
                  unitTypesPlannedValue[i] = unitTypesPlannedValue[i] + aiCost;
                  debugTechs(aiPlanGetName(planID) + " added " + aiCost + " planned unit value to " + kbTechGetName(techIDs[i]) + ".");
               }
            }
            else if (kbProtoUnitIsType(planUnitType, upgradeUnitTypes[i]) == true)
            {
               int numExistingUnits = kbUnitCount(planUnitType, cMyID, cUnitStateAlive);
               int numLeftToTrain = max(0, aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) - numExistingUnits);
               // Keep the numbers relatively low so that the weights make sense.
               float aiCost = numLeftToTrain * (kbAICostGetProtoUnitCost(planUnitType) / 100.0);
               unitTypesPlannedValue[i] = unitTypesPlannedValue[i] + aiCost;
               debugTechs(aiPlanGetName(planID) + " added " + aiCost + " planned unit value to " + kbTechGetName(techIDs[i]) + ".");
            }
         }

         // Also loop through all the strategy maintain plans, mainly for SPC.
         int[] planIDs = new int(0, 0);
         int numPlans = getStrategyMaintainPlans(planIDs);
         for (int iPlan = 0; iPlan < numPlans; iPlan++)
         {
            int planID = planIDs[iPlan];
            if (aiPlanGetIsIDValid(planID) == false)
            {
               continue;
            }
            int planUnitType = aiPlanGetVariableInt(planID, cTrainPlanUnitType, 0);
            // Some line upgrades effect single unit types, find that out here.
            if (upgradeUnitTypes[i] < cNumberProtoUnits)
            {
               if (upgradeUnitTypes[i] == planUnitType)
               {
                  int numExistingUnits = kbUnitCount(planUnitType, cMyID, cUnitStateAlive);
                  int numLeftToTrain = max(0, aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) - numExistingUnits);
                  // Keep the numbers relatively low so that the weights make sense.
                  float aiCost = numLeftToTrain * (kbAICostGetProtoUnitCost(planUnitType) / 100.0);
                  unitTypesPlannedValue[i] = unitTypesPlannedValue[i] + aiCost;
                  debugTechs(aiPlanGetName(planID) + " added " + aiCost + " planned unit value to " + kbTechGetName(techIDs[i]) + ".");
               }
            }
            else if (kbProtoUnitIsType(planUnitType, upgradeUnitTypes[i]) == true)
            {
               int numExistingUnits = kbUnitCount(planUnitType, cMyID, cUnitStateAlive);
               int numLeftToTrain = max(0, aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) - numExistingUnits);
               // Keep the numbers relatively low so that the weights make sense.
               float aiCost = numLeftToTrain * (kbAICostGetProtoUnitCost(planUnitType) / 100.0);
               unitTypesPlannedValue[i] = unitTypesPlannedValue[i] + aiCost;
               debugTechs(aiPlanGetName(planID) + " added " + aiCost + " planned unit value to " + kbTechGetName(techIDs[i]) + ".");
            }
         }
      }

      progress = 2;
      // Go again instantly.
      xsRuleIgnoreIntervalOnce("militaryUpgradeManager");
      return;
   }

   // Actually pick a technology to research from the array we created previous run.
   if (progress == 2)
   {
      int choosenTechID = -1;
      switch (chosenSegment)
      {
         // For these we take the lowest cost one.
         case cArmoryUpgrades:
         case cLevyUpgrades:
         {
            float lowestCost = cMaxFloat;
            int numTechs = techIDs.size();
            for (int iTech = 0; iTech < numTechs; iTech++)
            {
               if (kbTechGetStatus(techIDs[iTech]) == cTechStatusActive)
               {
                  // These arrays are made/handled over several frames, mechanics could've put this tech to active, like New Moon.
                  continue;
               }
               float cost = kbAICostGetTechCost(techIDs[iTech]);
               if (cost < lowestCost)
               {
                  lowestCost = cost;
                  choosenTechID = techIDs[iTech];
               }
            }
            break;
         }

         case cMythUpgrades:
         {
            int numMythTechs = techIDs.size();
            if (cPersonalityCurrent == cPersonalityDefender)
            {
               for (int iTech = 0; iTech < numMythTechs; iTech++)
               {
                  if (kbTechGetStatus(techIDs[iTech]) == cTechStatusActive)
                  {
                     // These arrays are made/handled over several frames, mechanics could've put this tech to active, like New Moon.
                     continue;
                  }
                  if (kbTechGetType(techIDs[iTech], cTechTypeMythTechDefensive) == true)
                  {
                     debugTechs("Found a Myth defensive upgrade to get since we're a defender: " + kbTechGetName(techIDs[iTech]) + ".");
                     choosenTechID = techIDs[iTech];
                     break;
                  }
                  // If we can't find any we randInt below.
               }
            }

            if (cPersonalityCurrent == cPersonalityMythical)
            {
               for (int iTech = 0; iTech < numMythTechs; iTech++)
               {
                  if (kbTechGetStatus(techIDs[iTech]) == cTechStatusActive)
                  {
                     // These arrays are made/handled over several frames, mechanics could've put this tech to active, like New Moon.
                     continue;
                  }
                  if (kbTechGetType(techIDs[iTech], cTechTypeMythUnitUpgrade) == true)
                  {
                     debugTechs("Found a Myth unit upgrade to get since we're mythical: " + kbTechGetName(techIDs[iTech]) + ".");
                     choosenTechID = techIDs[iTech];
                     break;
                  }
                  // If we can't find any we randInt below.
               }
            }
            
            if (choosenTechID == -1)
            {
               // Just rand for myth techs.
               int rand = xsRandInt(0, numMythTechs - 1);
               choosenTechID = techIDs[rand];
            }
            break;
         }

         // Analyze cost / how many units we have / how many units we've planned / hp and dmg boost for line upgrades.
         case cLineUpgrades:
         {
            const int costWeight = 1; // Every 1 resource the upgrade costs will reduce its score by 1.
            const int aliveUnitWeight = 45;
            const int plannedUnitWeight = 60;
            const int militaryUpgradeDamageWeight = 5; // For every 1% the upgrade adds this number gets added to the score.
            const int militaryUpgradeHitpointsWeight = 10; // For every 1% the upgrade adds this number gets added to the score.

            int minimumScoreNeeded = 100;
            // Overwhelmers want upgrades faster so they can build a big deathball.
            if (cPersonalityCurrent == cPersonalityOverwhelmer)
            {
               minimumScoreNeeded = 50;
            }
            debugTechs("Minimum score is: " + minimumScoreNeeded + ".");
            if (haveExcessResourceAmount(200) == true)
            {
               debugTechs("We have 200 excess across the board, removing minimum score.");
               minimumScoreNeeded = cMinInt;
            }

            // If we're close to popcap we will barely have any planned units so the minimum score may block upgrades while
            // we should be upgrading our big army.
            if (cDifficultyCurrent >= cDifficultyTitan &&
                buildingGetNumberAliveAndPlanned(gHouseUnit) == kbPlayerGetProtoStatInt(cMyID, gHouseUnit, cProtoStatBuildLimit) &&
                kbPlayerGetPop(cMyID) > (kbPlayerGetPopCap(cMyID) * 0.3))
            {
               debugTechs("We're close to max pop, removing minimum score.");
               minimumScoreNeeded = cMinInt;
            }

            int bestScore = cMinInt;
            int numTechs = techIDs.size();
            for (int i = 0; i < numTechs; i++)
            {
               if (kbTechGetStatus(techIDs[i]) == cTechStatusActive)
               {
                  // These arrays are made/handled over several frames, mechanics could've put this tech to active, like New Moon.
                  continue;
               }
               int score = 0;
               debugTechs("Analyzing " + kbTechGetName(techIDs[i]) + ".");
               int cost = kbAICostGetTechCost(techIDs[i]);
               score -= cost * costWeight;
               debugTechs("   Cost subtracted: " + (cost * costWeight) + ".");
               score += unitTypesAliveValue[i] * aliveUnitWeight;
               debugTechs("   Alive units added: " + xsFloatToInt((unitTypesAliveValue[i] * aliveUnitWeight)) + ".");
               score += unitTypesPlannedValue[i] * plannedUnitWeight;
               debugTechs("   Planned units added: " + xsFloatToInt((unitTypesPlannedValue[i] * plannedUnitWeight)) + ".");

               int numEffects = kbTechGetNumberEffects(techIDs[i]);
               for (int j = 0; j < numEffects; j++)
               {
                  // We can only analyze data affects.
                  if (kbTechGetEffectType(techIDs[i], j) != cEffectTypeData)
                  {
                     continue;
                  }
                  int effectType = kbTechGetDataEffectType(techIDs[i], j);
                  float amount = kbTechGetDataEffectAmount(techIDs[i], j);
                  switch (effectType)
                  {
                     case cDataEffectDamage:
                     {
                        // Get to the actual % it adds.
                        amount -= 1.0;
                        amount *= 100;
                        score += militaryUpgradeDamageWeight * amount;
                        debugTechs("   Damage weight added: " + xsFloatToInt((militaryUpgradeDamageWeight * amount)) + ".");
                        break;
                     }
                     case cDataEffectHitpoints:
                     {
                        // Get to the actual % it adds.
                        amount -= 1.0;
                        amount *= 100;
                        score += militaryUpgradeHitpointsWeight * amount;
                        debugTechs("   Hitpoints weight added: " + xsFloatToInt((militaryUpgradeHitpointsWeight * amount)) + ".");
                        break;
                     }
                  }
               }

               debugTechs(   kbTechGetName(techIDs[i]) + " gets score: " + score + ".");
               if (score > bestScore)
               {
                  bestScore = score;
                  choosenTechID = techIDs[i];
               }
            }
            if (bestScore < minimumScoreNeeded)
            {
               debugTechs("We couldn't find a line upgrade with a high enough score, not researching any now.");
               progress = 0;
               return;
            }
            break;
         }
      }

      // Low prio on upgrades when we're low on military, high prio when we have a lot of military.
      int prio = 50;
      if (currentMilitaryPop >= 1 && allowedMilitaryPop >= 1)
      {
         float percentageMilitaryPopAliveQueued = xsIntToFloat(currentMilitaryPop) / xsIntToFloat(allowedMilitaryPop);
         if (percentageMilitaryPopAliveQueued < cLowPrioArmyPercentage)
         {
            prio = 49;
         }
         else if (percentageMilitaryPopAliveQueued > cHighPrioArmyPercentage)
         {
            prio = 51;
         }
      }
      
      gMilitaryResearchPlanID = researchSimpleTech(choosenTechID, -1, -1, prio);
      aiPlanSetEventHandler(gMilitaryResearchPlanID, cPlanEventStateChange, "resetMilitaryResearchPlan");

      progress = 0;
      chosenSegment = -1;
      researchStartTime = currentTime;
   }
}

//==============================================================================
// dockUpgradeManager
// This doesn't get Fishing upgrades, economyUpgradeManager does that instead.
// Enclosed Deck not covered yet.
//==============================================================================
rule dockUpgradeManager
inactive
group defaultHeroicRules
minInterval 45
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("dockUpgradeManager");
      return;
   }
   // Easy doesn't get upgrades.
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("dockUpgradeManager");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoResearchMilitaryUpgrades) == false)
   {
      return;
   }
   debugTechs("--- Running Rule dockUpgradeManager ---");
   
   if (kbTechGetStatus(cTechConscriptSailors) == cTechStatusActive)
   {
      debugTechs("Researched all Dock upgrades, disabling dockUpgradeManager.");
      xsDisableRule("dockUpgradeManager");
      return;
   }

   static int researchStartTime = 0;
   int currentTime = xsGetTime();
   int dockCount = kbUnitCount(cUnitTypeDock, cMyID, cUnitStateAlive);
   if (aiPlanGetIsIDValid(gMilitaryDockResearchPlanID) == true)
   {
      if ((researchStartTime + 120) < currentTime && aiPlanGetState(gMilitaryDockResearchPlanID) != cPlanStateResearch)
      {
         debugTechs("We had a research plan that didn't go into state research for 2 minutes, destroying it. " +
            "Plan name: " + aiPlanGetName(gMilitaryDockResearchPlanID) + ".");
         // Don't use a setState here because then our handler will fire next frame and mess our new plan up.
         aiPlanDestroy(gMilitaryDockResearchPlanID);
         gMilitaryDockResearchPlanID = -1;
         researchStartTime = -1;
         // Instantly pick a new upgrade to get.
      }
      else
      {
         return;
      }
   }

   if (dockCount <= 0)
   {
      if (aiPlanGetIsIDValid(gMilitaryDockResearchPlanID) == true)
      {
         debugTechs("We have no Docks left but had a Dock research plan, destroying it. Plan name: " +
            aiPlanGetName(gMilitaryDockResearchPlanID) + ".");
         // Don't use a setState here because then our handler will fire next frame and mess our new plan up.
         aiPlanDestroy(gMilitaryDockResearchPlanID);
         gMilitaryDockResearchPlanID = -1;
         researchStartTime = -1;
      }
      debugTechs("We have no Docks currently, not attempting to research any naval upgrades.");
      return;
   }

   if (areAtMaxConcurrentResearchPlans("dockUpgradeManager") == true)
   {
      return;
   }

   // First get the line upgrades.
   // Use the != cTechStatusActive here instead of obtainable so that we always enter this even in Heroic when we don't have
   // Champion Warships unlocked. This is so we early out on the age check and only get myth/levy upgrades in Mythic.
   if (kbTechGetStatus(cTechHeavyWarships) != cTechStatusActive ||
       kbTechGetStatus(cTechChampionWarships) != cTechStatusActive)
   {
      int numWarships = kbUnitCount(cUnitTypeAbstractWarship, cMyID, cUnitStateAlive);
      if (kbTechGetStatus(cTechHeavyWarships) == cTechStatusObtainable)
      {
         if (numWarships < 8)
         {
            debugTechs("We have too few war ships, not researching Heavy Warships: " + numWarships + "/8.");
            return;
         }
         gMilitaryDockResearchPlanID = researchSimpleTech(cTechHeavyWarships);
         aiPlanSetEventHandler(gMilitaryDockResearchPlanID, cPlanEventStateChange, "resetMilitaryDockResearchPlan");
         return;
      }
      if (kbPlayerGetAge(cMyID) <= cAge3)
      {
         debugTechs("Still need to research Champion Warships but we're in Heroic, quiting.");
         return;
      }
      if (numWarships < 13)
      {
         debugTechs("We have too few war ships, not researching Champion Warships: " + numWarships + "/13.");
         return;
      }
      gMilitaryDockResearchPlanID = researchSimpleTech(cTechChampionWarships);
      aiPlanSetEventHandler(gMilitaryDockResearchPlanID, cPlanEventStateChange, "resetMilitaryDockResearchPlan");
      return;
   }

   if (cPersonalityCurrent != cPersonalityHumanoid)
   {
      // Then get the myth upgrades.
      int[] mythUpgrades = new int(0, -1);
      fillOutMilitaryUpgradeArrays(mythUpgrades, cDockMythUpgrades);
      if (mythUpgrades.size() > 0)
      {
         float lowestCost = cMaxFloat;
         int choosenTechID = -1;
         for (int i = 0; i < mythUpgrades.size(); i++)
         {
            float cost = kbAICostGetTechCost(mythUpgrades[i]);
            if (cost < lowestCost)
            {
               lowestCost = cost;
               choosenTechID = mythUpgrades[i];
            }
         }

         gMilitaryDockResearchPlanID = researchSimpleTech(choosenTechID, -1, -1, 49);
         aiPlanSetEventHandler(gMilitaryDockResearchPlanID, cPlanEventStateChange, "resetMilitaryDockResearchPlan");
         return;
      }
   }

   // Finally the levy upgrade. It's really unimportant...
   if (kbTechGetStatus(cTechConscriptSailors) == cTechStatusObtainable)
   {
      if (haveExcessResourceAmount(500, cResourceWood) == true)
      {
         gMilitaryDockResearchPlanID = researchSimpleTech(cTechConscriptSailors, -1, -1, 49);
         aiPlanSetEventHandler(gMilitaryDockResearchPlanID, cPlanEventStateChange, "resetMilitaryDockResearchPlan");
      }
      else
      {
         debugTechs("Not researching Conscript Sailors because we don't have 500 excess wood.");
      }
      return;
   }
}

extern bool gAreResearchingForcedEconomicUpgrade = false;
//==============================================================================
// haveForcedEconomicTechnologyToResearch
//==============================================================================
bool haveForcedEconomicTechnologyToResearch(ref int techID)
{
   int currentAge = kbPlayerGetAge(cMyID);
   // Get this also for the Farm preloading, and to be ready before we start the Farm transition.
   if ((cMyCiv == cCivRa || cMyCiv == cCivSet) &&
       kbTechGetStatus(cTechShaduf) == cTechStatusObtainable)
   {
      techID = cTechShaduf;
      return true;
   }

   // Always get Plow if we're fully committed to farming. If we're preloading we could also get it after a threshold.
   if (currentAge >= cAge2 &&
       (gTimeToFarm == true || kbUnitCount(cUnitTypeAbstractFarm, cMyID, cUnitStateABQ) > (cMyCulture == cCultureAtlantean ? 4 : 8))
       && kbTechGetStatus(cTechPlow) == cTechStatusObtainable)
   {
      techID = cTechPlow;
      return true;
   }

   if (currentAge >= cAge3 &&
       kbTechGetStatus(cTechHuntingEquipment) == cTechStatusObtainable)
   {
      techID = cTechHuntingEquipment;
      return true;
   }

   if ((cMyCiv == cCivHades || cMyCiv == cCivPoseidon) && currentAge >= cAge3)
   {
      if (kbTechGetStatus(cTechDivineBlood) == cTechStatusObtainable)
      {
         techID = cTechDivineBlood;
         return true;
      }
      if (kbTechGetStatus(cTechGoldenApples) == cTechStatusObtainable)
      {
         techID = cTechGoldenApples;
         return true;
      }
   }

   if (cPersonalityCurrent != cPersonalityHumanoid && (cMyCiv == cCivSet || cMyCiv == cCivIsis) &&
       kbTechGetStatus(cTechNecropolis) == cTechStatusObtainable)
   {
      techID = cTechNecropolis;
      return true;
   }

   if (cPersonalityCurrent != cPersonalityHumanoid && (cMyCiv == cCivKronos || cMyCiv == cCivOranos) &&
       kbTechGetStatus(cTechPerception) == cTechStatusObtainable)
   {
      techID = cTechPerception;
      return true;
   }

   if (cPersonalityCurrent != cPersonalityHumanoid && (cMyCiv == cCivGaia || cMyCiv == cCivOranos) &&
       kbTechGetStatus(cTechPropheticSight) == cTechStatusObtainable)
   {
      techID = cTechPropheticSight;
      return true;
   }

   if (cPersonalityCurrent != cPersonalityHumanoid && (cMyCiv == cCivKronos || cMyCiv == cCivOranos) &&
       kbTechGetStatus(cTechSonsOfTheSun) == cTechStatusObtainable)
   {
      techID = cTechSonsOfTheSun;
      return true;
   }

   if (cPersonalityCurrent != cPersonalityHumanoid && (cMyCiv == cCivNuwa || cMyCiv == cCivShennong) &&
       kbTechGetStatus(cTechAbundance) == cTechStatusObtainable)
   {
      techID = cTechAbundance;
      return true;
   }

   if (cMyCiv == cCivNuwa && kbTechGetStatus(cTechChasingTheSun) == cTechStatusObtainable)
   {
      techID = cTechChasingTheSun;
      return true;
   }
   if (kbTechGetStatus(cTechWatchTower) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechWatchTower);
      return true;
   }
   if (kbTechGetStatus(cTechTzompantliWatchTower) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechTzompantliWatchTower);
      return true;
   }
   if (kbTechGetStatus(cTechCopperWeapons) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechCopperWeapons);
      return true;
   }
   if (kbTechGetStatus(cTechCopperArmor) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechCopperArmor);
      return true;
   }
    if (kbTechGetStatus(cTechCopperShields) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechCopperShields);
      return true;
   }
   if (kbTechGetStatus(cTechBronzeWeapons) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechBronzeWeapons);
      return true;
   }
   if (kbTechGetStatus(cTechBronzeArmor) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechBronzeArmor);
      return true;
   }
   if (kbTechGetStatus(cTechBronzeShields) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechBronzeShields);
      return true;
   }
   if (kbTechGetStatus(cTechIronWeapons) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechIronWeapons);
      return true;
   }
   if (kbTechGetStatus(cTechIronArmor) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechIronArmor);
      return true;
   }
   if (kbTechGetStatus(cTechIronShields) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechIronShields);
      return true;
   }
   if (kbTechGetStatus(cTechBallistics) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechBallistics);
      return true;
   }
   if (kbTechGetStatus(cTechBoilingOil) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechBoilingOil);
      return true;
   }
   if (kbTechGetStatus(cTechTemiminaloyanTrials) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechTemiminaloyanTrials);
      return true;
   }
   if (kbTechGetStatus(cTechCrenellations) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechCrenellations);
      return true;
   }
   if (kbTechGetStatus(cTechGuardTower) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechGuardTower);
      return true;
   }
    if (kbTechGetStatus(cTechBallistaTower) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechBallistaTower);
      return true;
   }
    if (kbTechGetStatus(cTechCrossbowTower) == cTechStatusObtainable)
   {
      gMilitaryResearchPlanID = researchSimpleTech(cTechCrossbowTower);
      return true;
   }
   
   return false;
}

//==============================================================================
// economyUpgradeManager
//==============================================================================
rule economyUpgradeManager
inactive
group defaultClassicalRules
minInterval 30
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("economyUpgradeManager");
      return;
   }
   if (cStartingResourcesCurrent == cStartingResourcesInfinite)
   {
      xsDisableRule("economyUpgradeManager");
      return;
   }
   static bool initialized = false;
   static int[] gatherTargets = default;
   static int[] gatherTargetTypes = default;
   static string[] gatherNames = default;
   const int numGatheringCategories = 7;
   static int researchStartTime = -1;

   if (checkStrategyFlag(cStrategyFlagAutoResearchEconomyUpgrades) == false)
   {
      if (aiPlanGetIsIDValid(gEconomyResearchPlanID) == true && aiPlanGetState(gEconomyResearchPlanID) != cPlanStateResearch)
      {
         aiPlanDestroy(gEconomyResearchPlanID);
         gEconomyResearchPlanID = -1;
         gAreResearchingForcedEconomicUpgrade = false;
      }
      return;
   }

   debugTechs("--- Running Rule economyUpgradeManager. ---");

   if (aiPlanGetIsIDValid(gEconomyResearchPlanID) == true && aiPlanGetState(gEconomyResearchPlanID) == cPlanStateResearch)
   {
      // Still in research, wait for it to complete, we don't stack by default.
      debugTechs("We still have a research plan that is actually researching, quiting early: " +
         aiPlanGetName(gEconomyResearchPlanID) + ".");
      return;
   }

   if (initialized == false) // First run.
   {
      gatherTargets = new int(numGatheringCategories, -1);
      gatherTargetTypes = new int(numGatheringCategories, -1);
      gatherNames = new string(numGatheringCategories, "");

      gatherTargets[0] = cUnitTypeBerryBush;
      gatherTargetTypes[0] = cResourceFood;
      gatherNames[0] = "Berries";

      gatherTargets[1] = cUnitTypeHuntable;
      gatherTargetTypes[1] = cResourceFood;
      gatherNames[1] = "Hunters";

      gatherTargets[2] = cUnitTypeHerdable;
      gatherTargetTypes[2] = cResourceFood;
      gatherNames[2] = "Herders";

      gatherTargets[3] = cUnitTypeWoodResource;
      gatherTargetTypes[3] = cResourceWood;
      gatherNames[3] = "Lumberjacks";

      gatherTargets[4] = cUnitTypeGoldResource;
      gatherTargetTypes[4] = cResourceGold;
      gatherNames[4] = "Gold Miners";

      gatherTargets[5] = cUnitTypeFishResource;
      gatherTargetTypes[5] = cResourceFood;
      gatherNames[5] = "Fishing Ships";

      gatherTargets[6] = cUnitTypeAbstractFarm;
      gatherTargetTypes[6] = cResourceFood;
      gatherNames[6] = "Farmers";

      initialized = true;
   }

   if (gAreResearchingForcedEconomicUpgrade == true)
   {
      // Don't needlessly run haveForcedEconomicTechnologyToResearch and arrive at the same tech most likely.
      debugTechs("Not running through the default logic because we're already researching a forced technology.");
      return;
   }

   int techID = -1;
   // Don't take forced technologies into account for humanoid since they could cost favor.
   if (cPersonalityCurrent != cPersonalityHumanoid && haveForcedEconomicTechnologyToResearch(techID) == true)
   {
      if (aiPlanGetIsIDValid(gEconomyResearchPlanID) == true)
      {
         debugTechs("Cancelling old research plan since we have a forced tech now, cancelling: " +
            aiPlanGetName(gEconomyResearchPlanID) + ".");
         // Don't use a setState here because then our handler will fire next frame and mess our new plan up.
         aiPlanDestroy(gEconomyResearchPlanID);
      }
      gEconomyResearchPlanID = researchSimpleTech(techID);
      aiPlanSetEventHandler(gEconomyResearchPlanID, cPlanEventStateChange, "resetEconomyResearchPlan");
      gAreResearchingForcedEconomicUpgrade = true;
      debugTechs("Not running through the default logic because we have a forced technology to research.");
      return;
   }

   int numVillagers = kbUnitCount(cUnitTypeAbstractVillager, cMyID, cUnitStateAlive);
   int[] gatherersByTarget = new int(numGatheringCategories, 0);

   for (int i = 0; i < numGatheringCategories; i++)
   {
      if (i == 5)
      {
         gatherersByTarget[i] = kbUnitCount(gFishingUnit, cMyID, cUnitStateAlive);
      }
      else
      {
         gatherersByTarget[i] = aiGetNumberGatherers(cUnitTypeAbstractVillager, gatherTargetTypes[i], -1, gatherTargets[i]);
      }
      debugTechs(gatherNames[i] + " = " + gatherersByTarget[i] + ".");
   }
   
   int bestScore = cMinInt;
   int bestTechID = -1;
   int minimumScoreNeeded = 50;
   // We like economic upgrades as an economist.
   if (cPersonalityCurrent == cPersonalityEconomist)
   {
      minimumScoreNeeded = 25;
   }
   debugTechs("Minimum score is: " + minimumScoreNeeded + ".");
   if (haveExcessResourceAmount(200) == true)
   {
      debugTechs("We have 200 excess across the board, removing minimum score.");
      minimumScoreNeeded = cMinInt;
   }

   const float costWeight = 0.5; // Every 1 resource the upgrade costs will reduce its score by 0.5.
   const int workRateWeight = 5; // For every 1% the upgrade adds this number gets added to the score.
   // Double worker weight for Atlanteans because of their Citizens.
   // For every 1 gatherer affected this gets added to the score.
   int affectedGatherersWeight = cMyCulture == cCultureAtlantean ? 50 : 25;
   // For every 1 capacity the upgrade adds this number gets added to the score, usually walk further for wood, so higher.
   int carryCapacityWoodWeight = cMyCulture == cCultureAtlantean ? 0 : 10;
   int carryCapacityWeight = cMyCulture == cCultureAtlantean ? 0 : 5;
   const int trickleRateWeight = 500; // Trickles just really good because we can't mess it up :P
   int numGatherersForHighPrio = cPersonalityCurrent == cPersonalityEconomist ? 7 : 10;
   int prio = 50;

   bool foundAnyTechnologyToEvaluate = false;

   int[] obtainableTechs = kbTechTreeGetAllObtainableTechnologies(false);
   for (int i = 0; i < obtainableTechs.size(); i++)
   {
      int currentTechID = obtainableTechs[i];
      if (cPersonalityCurrent == cPersonalityHumanoid)
      {
         if (kbTechGetCostPerResource(currentTechID, cResourceFavor) > 0.0)
         {
            debugTechs("We're a humanoid and " + kbTechGetName(currentTechID) + " costs favor, so we don't get it.");
            continue;
         }
      }
      if (kbTechGetFlag(currentTechID, cTechFlagCountsTowardEconomicScore) == false)
      {
         continue;
      }

      int numEffects = kbTechGetNumberEffects(currentTechID);
      // If this tech doesn't improve our Villagers is must be a resource trickle or we don't know how to analyze it here.
      // This means that any economic upgrades that don't affect Villagers and aren't trickles must be in the forced section.
      if (kbTechAffectsUnitType(currentTechID, cUnitTypeAbstractVillager) == false &&
          kbTechAffectsUnitType(currentTechID, cUnitTypeAbstractFishingShip) == false)
      {
         bool resourceTrickle = false;
         for (int j = 0; j < numEffects; j++)
         {
            // We can only analyze data affects.
            if (kbTechGetEffectType(currentTechID, j) != cEffectTypeData)
            {
               continue;
            }
            if (kbTechGetDataEffectType(currentTechID, j) == cDataEffectResourceTrickleRate)
            {
               resourceTrickle = true;
               break;
            }
         }
         if (resourceTrickle == false)
         {
            debugTechs("Invalid for this logic: " + kbTechGetName(obtainableTechs[i]) + ".");
            continue;
         }
      }

      foundAnyTechnologyToEvaluate = true;
      debugTechs("Analyzing " + kbTechGetName(obtainableTechs[i]) + ".");

      int score = 0;
      int numGatherersAffected = 0;
      float cost = kbAICostGetTechCost(currentTechID);
      score -= cost * costWeight;
      debugTechs("   Cost subtracted: " + (cost * costWeight) + ".");

      for (int j = 0; j < numEffects; j++)
      {
         // We can only analyze data affects.
         int effectType = kbTechGetEffectType(currentTechID, j);
         if (effectType != cEffectTypeData)
         {
            continue;
         }

         int dataEffectType = kbTechGetDataEffectType(currentTechID, j);
         float amount = kbTechGetDataEffectAmount(currentTechID, j);

         switch (dataEffectType)
         {
            case cDataEffectWorkRate:
            {
               int unitType = kbTechGetDataEffectData3(currentTechID, j);
               for (int k = 0; k < numGatheringCategories; k++)
               {
                  if (unitType == gatherTargets[k])
                  {
                     // Get to the actual % it adds.
                     amount -= 1.0;
                     amount *= 100;
                     score += workRateWeight * amount;
                     debugTechs("   Workrate weight added: " + (workRateWeight * amount) + ".");

                     score += gatherersByTarget[k] * affectedGatherersWeight;
                     debugTechs("   Affected gatherers weight added: " + (gatherersByTarget[k] * affectedGatherersWeight) + ".");

                     numGatherersAffected += gatherersByTarget[k];
                  }
               }
               break;
            }
            case cDataEffectCarryCapacity:
            {
               // Get to the actual % it adds.
               int carryCapacityResourceID = kbTechGetDataEffectData2(currentTechID, j);
               if (carryCapacityResourceID == cResourceWood)
               {
                  score += amount * carryCapacityWoodWeight;
                  debugTechs("   Carry capacity wood weight added: " + (carryCapacityWoodWeight * amount) + ".");
               }
               else
               {
                  score += amount * carryCapacityWeight;
                  debugTechs("   Carry capacity weight added: " + (carryCapacityWeight * amount) + ".");
               }
               break;
            }
            case cDataEffectResourceTrickleRate:
            {
               //int trickleResourceID = kbTechGetDataEffectData2(currentTechID, j);
               score += amount * trickleRateWeight;
               debugTechs("   Trickle rate weight added: " + trickleRateWeight + ".");
               break;
            }
         }
      }

      debugTechs(kbTechGetName(currentTechID) + " has a total score of: " + score + ".");

      if (score < minimumScoreNeeded)
      {
         continue;
      }

      if (bestScore < score)
      {
         bestTechID = currentTechID;
         bestScore = score;

         if (numGatherersAffected > numGatherersForHighPrio)
         {
            prio = 51;
         }
         else
         {
            prio = 50;
         }
      }
   }

   if (kbPlayerGetAge(cMyID) >= cAge4 && foundAnyTechnologyToEvaluate == false)
   {
      if (aiPlanGetIsIDValid(gEconomyResearchPlanID) == true)
      {
         aiEchoWarning("How can we find no technologies to evaluate but we still have gEconomyResearchPlanID valid?");
      }
      debugTechs("No more technologies to research for the economyUpgradeManager, disabling it.");
      xsDisableRule("economyUpgradeManager");
      return;
   }

   // Reset the plan if we've chosen a new one tech to research.
   if (aiPlanGetIsIDValid(gEconomyResearchPlanID) == true)
   {
      if (aiPlanGetVariableInt(gEconomyResearchPlanID, cResearchPlanTechID, 0) != bestTechID)
      {
         debugTechs("Cancelling old research plan since our needs have changed, cancelling: " +
            aiPlanGetName(gEconomyResearchPlanID) + ".");
         // Don't use a setState here because then our handler will fire next frame and mess our new plan up.
         aiPlanDestroy(gEconomyResearchPlanID);
         gEconomyResearchPlanID = -1;
      }
      else
      {
         int currentPriority = aiPlanGetPriority(gEconomyResearchPlanID);
         debugTechs("We're already researching the tech with the highest score in plan: " + aiPlanGetName(gEconomyResearchPlanID) +
            ", with priority: " + currentPriority + ".");
         if (currentPriority != prio)
         {
            aiPlanSetPriority(gEconomyResearchPlanID, prio);
            debugTechs("   Adjusting its priority to " + prio + ".");
         }
         return;
      }
   }

   if (bestTechID >= 0)
   {
      gEconomyResearchPlanID = researchSimpleTech(bestTechID, -1, -1, prio);
      aiPlanSetEventHandler(gEconomyResearchPlanID, cPlanEventStateChange, "resetEconomyResearchPlan");
      debugTechs("Research tech: " + kbTechGetName(bestTechID) + ", it had the highest score of = " + bestScore + ".");
   }   
}

//==============================================================================
// towerOffensiveUpgradeMonitor
// Gets all the offensive Tower upgrades.
//==============================================================================
rule towerOffensiveUpgradeMonitor
inactive
group defaultClassicalRules
minInterval 60
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("towerOffensiveUpgradeMonitor");
      return;
   }
   // Easy doesn't get upgrades.
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("towerOffensiveUpgradeMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoResearchMilitaryUpgrades) == false)
   {
      return;
   }
   debugTechs("--- Running Rule towerOffensiveUpgradeMonitor ---");

   if (aiPlanGetIsIDValid(gTowerOffensiveResearchPlanID) == true &&
       aiPlanGetState(gTowerOffensiveResearchPlanID) == cPlanStateResearch)
   {
      // Still in research, wait for it to complete, we don't stack and don't cancel the plan midway.
      debugTechs("We still have a research plan that is actually researching, quiting early: " +
         aiPlanGetName(gTowerOffensiveResearchPlanID) + ".");
      return;
   }

   static int minRequiredTowersForCurrentUpgrade = 1;
   int towerCount = kbUnitCount(cUnitTypeSentryTower, cMyID, cUnitStateAlive);
   // If we've dropped below our wanted Tower count for our current upgrade we cancel it.
   if (aiPlanGetIsIDValid(gTowerOffensiveResearchPlanID) == true)
   {
      if (towerCount < minRequiredTowersForCurrentUpgrade)
      {
         debugTechs("Destroying plan: " + aiPlanGetName(gTowerOffensiveResearchPlanID) + " because we have not enough Towers left: " +
            towerCount + "/" + minRequiredTowersForCurrentUpgrade + ".");
         aiPlanDestroy(gTowerOffensiveResearchPlanID);
         gTowerOffensiveResearchPlanID = -1;
         // We can try to get Crenellations maybe this run, but mostly our minRequiredTowersForCurrentUpgrade will remain not met.
      }
      else
      {
         debugTechs("We have a functional plan in: " + aiPlanGetName(gTowerOffensiveResearchPlanID) + ", quiting.");
         return;
      }
   }

   if (areAtMaxConcurrentResearchPlans("towerOffensiveUpgradeMonitor") == true)
   {
      return;
   }

   if (towerCount <= 1)
   {
      debugTechs("Not researching any Tower upgrades because we have 1 or fewer Towers left.");
      return;
   }

   int age = kbPlayerGetAge(cMyID);
   if (cPersonalityCurrent == cPersonalityAttacker && age == cAge2)
   {
      debugTechs("Not researching any Tower upgrades because we're attacker personality and still in the Classical Age.");
      return;
   }

   int prio = 51; // Actually get the upgrades with some haste.
   if (cPersonalityCurrent == cPersonalityDefender)
   {
      prio = 55;
      debugTechs("Increase priority we give to offensive Tower upgrades to 55 from 51 since we're a Defender.");
   }

   bool researchUpgrade = false;

   if (kbTechGetStatus(cTechWatchTower) == cTechStatusObtainable)
   {
      // Research Watch Tower after 4 minutes being in Classical.
      int time = gAgeUpTimes[cAge2] + 240;
      if (cPersonalityCurrent == cPersonalityDefender)
      {
         time -= 60; // Minute faster.
      }
      if (time < xsGetTime())
      {
         debugTechs("Researching Watch Tower because 4 (3 defender) minutes have passed since we reached the Classical Age.");
         researchUpgrade = true;
      }

      // Get the upgrade if our main base is in peril.
      if (researchUpgrade == false && gDefensivelyOverrun == true)
      {
         debugTechs("Researching Watch Tower because gDefensivelyOverrun == true.");
         researchUpgrade = true;
      }

      // Just always get it if we've aged up further already.
      if (researchUpgrade == false && age >= cAge3)
      {
         debugTechs("Researching Watch Tower because we're already in the Heroic/Mythic/Wonder Age and don't have it yet.");
         researchUpgrade = true;
      }

      if (researchUpgrade == true)
      {
         gTowerOffensiveResearchPlanID = researchSimpleTech(cTechWatchTower, cUnitTypeSentryTower, -1, prio);
         aiPlanSetEventHandler(gTowerOffensiveResearchPlanID, cPlanEventStateChange, "resetOffensiveTowerResearchPlan");
      }
      else
      {
         debugTechs("We're not ready to research Watch Tower yet.");
      }
      return;
   }
   
   if (kbTechGetStatus(cTechTzompantliWatchTower) == cTechStatusObtainable)
   {
      // Research Watch Tower after 4 minutes being in Classical.
      int time = gAgeUpTimes[cAge2] + 240;
      if (cPersonalityCurrent == cPersonalityDefender)
      {
         time -= 60; // Minute faster.
      }
      if (time < xsGetTime())
      {
         debugTechs("Researching Watch Tower because 4 (3 defender) minutes have passed since we reached the Classical Age.");
         researchUpgrade = true;
      }

      // Get the upgrade if our main base is in peril.
      if (researchUpgrade == false && gDefensivelyOverrun == true)
      {
         debugTechs("Researching Watch Tower because gDefensivelyOverrun == true.");
         researchUpgrade = true;
      }

      // Just always get it if we've aged up further already.
      if (researchUpgrade == false && age >= cAge3)
      {
         debugTechs("Researching Watch Tower because we're already in the Heroic/Mythic/Wonder Age and don't have it yet.");
         researchUpgrade = true;
      }

      if (researchUpgrade == true)
      {
         gTowerOffensiveResearchPlanID = researchSimpleTech(cTechTzompantliWatchTower, cUnitTypeSentryTower, -1, prio);
         aiPlanSetEventHandler(gTowerOffensiveResearchPlanID, cPlanEventStateChange, "resetOffensiveTowerResearchPlan");
      }
      else
      {
         debugTechs("We're not ready to research Watch Tower yet.");
      }
      return;
   }
   
    if (kbTechGetStatus(cTechTemiminaloyanTrials) == cTechStatusObtainable)
   {
      int queryID = useSimpleUnitQuery(cUnitTypeAbstractInfantry, cPlayerRelationEnemyNotGaia);
      int numInfantry = kbUnitQueryExecute(queryID);
      debugTechs("Found " + numInfantry + "/10 for researching Boiling Oil");
      if (numInfantry >= 10)
      {
         gTowerOffensiveResearchPlanID = researchSimpleTech(cTechTemiminaloyanTrials, cUnitTypeSentryTower, -1, prio);
         aiPlanSetEventHandler(gTowerOffensiveResearchPlanID, cPlanEventStateChange, "resetOffensiveTowerResearchPlan");
         return;
      }
      // We can potentially skip Boiling Oil so don't always return here.
   }
   
    if (kbTechGetStatus(cTechBoilingOil) == cTechStatusObtainable)
   {
      int queryID = useSimpleUnitQuery(cUnitTypeAbstractInfantry, cPlayerRelationEnemyNotGaia);
      int numInfantry = kbUnitQueryExecute(queryID);
      debugTechs("Found " + numInfantry + "/10 for researching Boiling Oil");
      if (numInfantry >= 10)
      {
         gTowerOffensiveResearchPlanID = researchSimpleTech(cTechBoilingOil, cUnitTypeSentryTower, -1, prio);
         aiPlanSetEventHandler(gTowerOffensiveResearchPlanID, cPlanEventStateChange, "resetOffensiveTowerResearchPlan");
         return;
      }
      // We can potentially skip Boiling Oil so don't always return here.
   }

   if (kbTechGetStatus(cTechCrenellations) == cTechStatusObtainable)
   {
      int queryID = useSimpleUnitQuery(cUnitTypeAbstractCavalry, cPlayerRelationEnemyNotGaia);
      int numCavalry = kbUnitQueryExecute(queryID);
      debugTechs("Found " + numCavalry + "/10 for researching Crenellations");
      if (numCavalry >= 10)
      {
         gTowerOffensiveResearchPlanID = researchSimpleTech(cTechCrenellations, cUnitTypeSentryTower, -1, prio);
         aiPlanSetEventHandler(gTowerOffensiveResearchPlanID, cPlanEventStateChange, "resetOffensiveTowerResearchPlan");
         return;
      }
      // We can potentially skip crenellations so don't always return here.
   }

   // Norse can only get Watch Tower / Crenellations.
   // Don't allow moderate to get strong Tower upgrades.
   if (cMyCulture == cCultureNorse || cDifficultyCurrent == cDifficultyModerate)
   {
      if (kbTechGetStatus(cTechCrenellations) == cTechStatusActive)
      {
         debugTechs("Disabling rule towerOffensiveUpgradeMonitor because we got all upgrades we can get/are allowed to get.");
         xsDisableRule("towerOffensiveUpgradeMonitor");
      }
      return;
   }

   if (age <= cAge2)
   {
      return; // Wait for Heroic.
   }

   if (towerCount <= 4)
   {
      debugTechs("Not researching Guard Tower upgrade because we have 4 or fewer Towers left.");
      return;
   }
   minRequiredTowersForCurrentUpgrade = 4;

   if (kbTechGetStatus(cTechGuardTower) == cTechStatusObtainable)
   {
      // Research Guard Tower after 5 minutes being in Heroic.
      int time = 300;
      if (cPersonalityCurrent == cPersonalityDefender)
      {
         time -= 90; // Minute and a half faster.
      }
      if (time < xsGetTime())
      {
         debugTechs("Researching Guard Tower because 5 (3.5 defender) minutes have passed since we reached the Heroic Age.");
         researchUpgrade = true;
      }

      // Get the upgrade if our main base is in peril.
      if (researchUpgrade == false && gDefensivelyOverrun == true)
      {
         debugTechs("Researching Guard Tower because gDefensivelyOverrun == true.");
         researchUpgrade = true;
      }

      // Just always get it if we've aged up further already.
      if (researchUpgrade == false && age >= cAge4)
      {
         debugTechs("Researching Guard Tower because we're already in the Mythic Age and don't have it yet.");
         researchUpgrade = true;
      }

      if (researchUpgrade == true)
      {
         gTowerOffensiveResearchPlanID = researchSimpleTech(cTechGuardTower, cUnitTypeSentryTower, -1, prio);
         aiPlanSetEventHandler(gTowerOffensiveResearchPlanID, cPlanEventStateChange, "resetOffensiveTowerResearchPlan");
      }
      else
      {
         debugTechs("We're not ready to research Guard Tower yet.");
      }
      return;
   }

   // Only Egyptians/Chinese can get Mythic age Tower upgrades.
   // Don't allow Hard to get the strongest Tower upgrades.
   if ((cMyCulture != cCultureEgyptian && cMyCulture != cCultureChinese) || cDifficultyCurrent == cDifficultyHard)
   {
      if (kbTechGetStatus(cTechCrenellations) == cTechStatusActive)
      {
         debugTechs("Disabling rule towerOffensiveUpgradeMonitor because we got all upgrades we can get/are allowed to get.");
         xsDisableRule("towerOffensiveUpgradeMonitor");
      }
      return;
   }

   if (age <= cAge3)
   {
      return; // Wait for Mythic.
   }

   if (towerCount <= 6)
   {
      if (cMyCulture == cCultureEgyptian)
      {
         debugTechs("Not researching Ballista Tower upgrade because we have 6 or fewer Towers left.");
      }
      else
      {
         debugTechs("Not researching Crossbow Tower upgrade because we have 6 or fewer Towers left.");
      }
      return;
   }
   minRequiredTowersForCurrentUpgrade = 6;

   if (cMyCulture == cCultureEgyptian)
   {
      if (kbTechGetStatus(cTechBallistaTower) == cTechStatusObtainable)
      {
         // Research Ballista Tower after 6 minutes being in Mythic.
         int time = gAgeUpTimes[cAge4] + 360;
         if (cPersonalityCurrent == cPersonalityDefender)
         {
            time -= 120; // 2 Minutes faster.
         }
         if (time < xsGetTime())
         {
            debugTechs("Researching Ballista Tower because 6 (4 defender) minutes have passed since we reached the Mythic Age.");
            researchUpgrade = true;
         }

         // Get the upgrade if our main base is in peril.
         if (researchUpgrade == false && gDefensivelyOverrun == true)
         {
            debugTechs("Researching Ballista Tower because gDefensivelyOverrun == true.");
            researchUpgrade = true;
         }

         if (researchUpgrade == true)
         {
            gTowerOffensiveResearchPlanID = researchSimpleTech(cTechBallistaTower, cUnitTypeSentryTower, -1, prio);
            aiPlanSetEventHandler(gTowerOffensiveResearchPlanID, cPlanEventStateChange, "resetOffensiveTowerResearchPlan");
         }
         else
         {
            debugTechs("We're not ready to research Ballista Tower yet.");
         }
         return;
      }
   }

   if (cMyCulture == cCultureChinese)
   {
      if (kbTechGetStatus(cTechCrossbowTower) == cTechStatusObtainable)
      {
         // Research Crossbow Tower after 6 minutes being in Mythic.
         int time = gAgeUpTimes[cAge4] + 360;
         if (cPersonalityCurrent == cPersonalityDefender)
         {
            time -= 120; // 2 Minutes faster.
         }
         if (time < xsGetTime())
         {
            debugTechs("Researching Crossbow Tower because 6 (4 defender) minutes have passed since we reached the Mythic Age.");
            researchUpgrade = true;
         }

         // Get the upgrade if our main base is in peril.
         if (researchUpgrade == false && gDefensivelyOverrun == true)
         {
            debugTechs("Researching Crossbow Tower because gDefensivelyOverrun == true.");
            researchUpgrade = true;
         }

         if (researchUpgrade == true)
         {
            gTowerOffensiveResearchPlanID = researchSimpleTech(cTechCrossbowTower, cUnitTypeSentryTower, -1, prio);
            aiPlanSetEventHandler(gTowerOffensiveResearchPlanID, cPlanEventStateChange, "resetOffensiveTowerResearchPlan");
         }
         else
         {
            debugTechs("We're not ready to research Crossbow Tower yet.");
         }
         return;
      }
   }

   if (kbTechGetStatus(cTechCrenellations) == cTechStatusActive)
   {
      debugTechs("Disabling rule towerOffensiveUpgradeMonitor because we got all upgrades we can get.");
      xsDisableRule("towerOffensiveUpgradeMonitor");
   }
}

//==============================================================================
// towerLOSUpgradeMonitor
// Gets all the LOS Tower upgrades.
//==============================================================================
rule towerLOSUpgradeMonitor
inactive
group defaultClassicalRules
minInterval 60
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("towerLOSUpgradeMonitor");
      return;
   }
   // Easy doesn't get upgrades.
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("towerLOSUpgradeMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoResearchEconomyUpgrades) == false)
   {
      return;
   }
   debugTechs("--- Running Rule towerLOSUpgradeMonitor ---");

   if (kbTechGetStatus(cTechSignalFires) == cTechStatusActive &&
       kbTechGetStatus(cTechCarrierPigeons) == cTechStatusActive)
   {
      xsDisableRule("towerLOSUpgradeMonitor");
      return;
   }

   if (areAtMaxConcurrentResearchPlans("towerLOSUpgradeMonitor") == true)
   {
      return;
   }

   int towerCount = kbUnitCount(cUnitTypeSentryTower, cMyID, cUnitStateAlive);
   if (towerCount <= 0)
   {
      debugTechs("Not researching any LOS upgrades atm because we have no Towers left to research them in.");
      return;
   }

   int age = kbPlayerGetAge(cMyID);

   float currentWoodStockpile = kbResourceGet(cResourceWood);
   if (kbTechGetStatus(cTechSignalFires) == cTechStatusObtainable)
   {
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechSignalFires) != -1)
      {
         return; // Already getting the upgrade.
      }
      // Not really a useful upgrade, only get if we have a big stockpile of wood.
      if (currentWoodStockpile > 600)
      {
         researchSimpleTech(cTechSignalFires, cUnitTypeSentryTower, -1, 50);
      }
      else
      {
         debugTechs("Not enough stockpiled wood yet to research Signal Fires. " + currentWoodStockpile + "/600.");
      }
      return;
   }

   // Don't get Carrier Pigeons on lower difficulties.
   if (cDifficultyCurrent <= cDifficultyHard)
   {
      xsDisableRule("towerLOSUpgradeMonitor");
      return;
   }

   // Carrier Pigeons is unlocked in Heroic.
   if (age <= cAge2)
   {
      return;
   }

   if (aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechCarrierPigeons) != -1)
   {
      return; // Already getting the upgrade.
   }
   // Carrier Pigeons is not really useful and very expensive. Just make it random when we get it and already have a lot of wood.
   if (haveExcessResourceAmount(500, cResourceWood) == true)
   {
      if (xsRandInt(0, 9) == 0)
      {
         researchSimpleTech(cTechCarrierPigeons, cUnitTypeSentryTower, -1, 50);
      }
      else
      {
         debugTechs("Didn't roll right to start researching Carrier Pigeons.");
      }
   }
   else
   {
      debugTechs("Not enough excess wood yet to attempt researching Carrier Pidgeons.");
   }
}

//==============================================================================
// townCenterUpgradeMonitor
// Gets all the Town Center upgrades.
//==============================================================================
rule townCenterUpgradeMonitor
inactive
group defaultHeroicRules
minInterval 60
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("townCenterUpgradeMonitor");
      return;
   }
   // Easy doesn't get upgrades.
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("townCenterUpgradeMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoResearchEconomyUpgrades) == false)
   {
      return;
   }
   debugTechs("--- Running Rule townCenterUpgradeMonitor ---");

   if (kbTechGetStatus(cTechFortifiedTownCenter) == cTechStatusActive &&
       kbTechGetStatus(cTechMasons) == cTechStatusActive &&
       kbTechGetStatus(cTechArchitects) == cTechStatusActive)
   {
      xsDisableRule("townCenterUpgradeMonitor");
      return;
   }

   int tcCount = kbUnitCount(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateAlive);
   if (tcCount <= 0)
   {
      debugTechs("Not researching any Town Center upgrades atm because we have no TCs left to research them in.");
      return;
   }

   if (kbTechGetStatus(cTechFortifiedTownCenter) == cTechStatusObtainable)
   {
      int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechFortifiedTownCenter);
      if (planID >= 0)
      {
         // Potentially bump priority if we already have a plan.
         if (cDifficultyCurrent >= cDifficultyTitan && aiPlanGetPriority(planID) <= 50)
         {
            if (buildingGetNumberAliveAndPlanned(gHouseUnit) == kbPlayerGetProtoStatInt(cMyID, gHouseUnit, cProtoStatBuildLimit) &&
                kbPlayerGetPop(cMyID) > (kbPlayerGetPopCap(cMyID) * 0.9))
            {
               aiPlanSetPriority(planID, 51);
               debugTechs("Found a Fortified Town Center plan that will be bumped in priority to 51 because we're nearly maxed out.");
            }
         }
         debugTechs("Already researching Fortified Town Center, not doing anything else.");
         return;
      }
      else
      {
         bool shouldResearch = false;
         int prio = 50;
         if (cDifficultyCurrent >= cDifficultyTitan &&
             buildingGetNumberAliveAndPlanned(gHouseUnit) == kbPlayerGetProtoStatInt(cMyID, gHouseUnit, cProtoStatBuildLimit) &&
             kbPlayerGetPop(cMyID) > (kbPlayerGetPopCap(cMyID) * 0.9))
         {
            debugTechs("We're at our House build limit and are close to maxing out, getting Fortified Town Center now with high prio.");
            shouldResearch = true;
            prio = 51;
         }
         int time = gAgeUpTimes[cAge4] + 600; // 10 minutes. 
         if (time < xsGetTime())
         {
            shouldResearch = true;
            debugTechs("Researching Fortified Town Center because 10 minutes have passed since we reached the Heroic Age.");
         }
         if (shouldResearch == true)
         {
            researchSimpleTech(cTechFortifiedTownCenter, cUnitTypeAbstractSocketedTownCenter, -1, prio);
         }
      }
      return;
   }

   // Masons + Architects in Mythic for now.
   if (kbPlayerGetAge(cMyID) <= cAge3)
   {
      return;
   }

   // Put Masons + Architects behind the global limit, not fortified.
   if (areAtMaxConcurrentResearchPlans("townCenterUpgradeMonitor") == true)
   {
      return;
   }

   if (kbTechGetStatus(cTechMasons) == cTechStatusObtainable)
   {
      researchSimpleTech(cTechMasons, cUnitTypeAbstractSocketedTownCenter, -1, 50);
   }
   if (kbTechGetStatus(cTechArchitects) == cTechStatusObtainable)
   {
      // If we're already researching we don't want to analyze it again.
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechArchitects) == -1)
      {
         // Need at least a part of the cost as excess before we get this, cuz it's very expensive.
         if (haveExcessResourceAmount(300, cResourceFood) == true &&
             haveExcessResourceAmount(400, cResourceWood) == true)
         {
            researchSimpleTech(cTechArchitects, cUnitTypeAbstractSocketedTownCenter, -1, 50);
         }
         else
         {
            debugTechs("We don't have enough excess resources to start an Architects research plan.");
         }
      }
   }
}

//==============================================================================
// wallUpgradeMonitor
// Gets all the Wall upgrades.
// Don't bother with tracking if we should cancel a Wall upgrade. It's cool to get them and it's defender only anyway.
//==============================================================================
rule wallUpgradeMonitor
inactive
group defaultClassicalRules
minInterval 60
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("wallUpgradeMonitor");
      return;
   }
   // Easy doesn't get upgrades.
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("wallUpgradeMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoResearchWallUpgrades) == false)
   {
      return;
   }
   debugTechs("--- Running Rule wallUpgradeMonitor ---");

   // Don't put it on the global research limit. Walls are a defining factor of the defender personality,
   // we need to showcase the upgrades too.

   int wallCount = kbUnitCount(cUnitTypeAbstractWall, cMyID, cUnitStateAlive);
   if (wallCount < 5)
   {
      debugTechs("We have " + wallCount + "/5 Walls, it's too few to justify getting any upgrades at all.");
      return;
   }

   int prio = 50;
   int minRequiredWallCount = 20;
   int currentAge = kbPlayerGetAge(cMyID);
   int currentTime = xsGetTime();
   if (wallCount < minRequiredWallCount && currentAge == cAge2) // In higher ages we just skip this and get Stone Wall instantly.
   {
      if (gAgeUpTimes[cAge2] + 300 > currentTime)
      {
         debugTechs("We have " + wallCount + "/" + minRequiredWallCount + " Walls, and aren't in the Classical age for long " +
            "enough, can't research any technology. We can skip the wall count check by either aging up or waiting till " +
            turnNumberIntoTimeDisplay(gAgeUpTimes[cAge2] + 300) + ".");
         return;
      }
   }
   if (wallCount >= minRequiredWallCount)
   {
      prio = 51;
   }

   if (kbTechGetStatus(cTechStoneWall) == cTechStatusObtainable)
   {
      int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechStoneWall);
      if (planID == -1)
      {
         researchSimpleTech(cTechStoneWall, cUnitTypeAbstractWall, -1, prio);
      }
      else
      {
         aiPlanSetPriority(planID, prio);
         debugTechs("We already have a research plan for Stone Wall going.");
      }
      return;
   }

   if (cMyCulture == cCultureNorse)
   {
      xsDisableRule("wallUpgradeMonitor");
      return;
   }
   if (currentAge < cAge3)
   {
      debugTechs("Waiting until the next Age to get more upgrades.");
      return;
   }

   minRequiredWallCount = 40;
   if (wallCount < minRequiredWallCount && currentAge == cAge3) // In higher ages we just skip this and get Fortified/Bronze Wall instantly.
   {
      if (gAgeUpTimes[cAge3] + 360 > currentTime)
      {
         debugTechs("We have " + wallCount + "/" + minRequiredWallCount + " Walls, and aren't in the Heroic age for long enough, " +
            "can't research any technology. We can skip the wall count check by either aging up or waiting till " +
            turnNumberIntoTimeDisplay(gAgeUpTimes[cAge3] + 360) + ".");
         return;
      }
   }
   if (wallCount >= minRequiredWallCount)
   {
      prio = 51;
   }
   else
   {
      prio = 50;
   }

   int wallTech = cTechFortifiedWall;
   if (cMyCulture == cCultureAtlantean)
   {
      // Yes this upgrade is actually available in the Classical Age already, but that makes this script so complex to handle that too.
      wallTech = cTechBronzeWall;
   }
   if (kbTechGetStatus(wallTech) == cTechStatusObtainable)
   {
      int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, wallTech);
      if (planID == -1)
      {
         researchSimpleTech(wallTech, cUnitTypeAbstractWall, -1, prio);
      }
      else
      {
         aiPlanSetPriority(planID, prio);
         debugTechs("We already have a research plan for " + kbTechGetName(wallTech) + " going.");
      }
      return;
   }

   if (cMyCulture == cCultureGreek || cMyCulture == cCultureJapanese)
   {
      xsDisableRule("wallUpgradeMonitor");
      return;
   }
   if (currentAge < cAge4)
   {
      debugTechs("Waiting until the next Age to get more upgrades.");
      return;
   }

   minRequiredWallCount = 50;
   if (wallCount < minRequiredWallCount && currentAge == cAge4) // In Wonder Age we just skip this and get the last Wall upgrades instantly.
   {
      if (gAgeUpTimes[cAge4] + 420 > currentTime)
      {
         debugTechs("We have " + wallCount + "/" + minRequiredWallCount + " Walls, and aren't in the Mythic age for long enough, " +
            "can't research any technology. We can skip the wall count check by either aging up or waiting till " +
            turnNumberIntoTimeDisplay(gAgeUpTimes[cAge4] + 420) + ".");
         return;
      }
   }
   if (wallCount >= minRequiredWallCount)
   {
      prio = 51;
   }
   else
   {
      prio = 50;
   }

   if (cMyCulture == cCultureEgyptian)
   {
      if (kbTechGetStatus(cTechCitadelWall) == cTechStatusObtainable)
      {
         int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechCitadelWall);
         if (planID == -1)
         {
            if (xsRandInt(0, 5) == 0)
            {
               researchSimpleTech(cTechCitadelWall, cUnitTypeAbstractWall, -1, prio);
            }
            else
            {
               debugTechs("Didn't roll correctly to start the Citadel Wall research plan.");
            }
         }
         else
         {
            aiPlanSetPriority(planID, prio);
            debugTechs("We already have a research plan for Citadel Wall going.");
         }
      }
      else
      {
         xsDisableRule("wallUpgradeMonitor");
         return;
      }
   }

   if (cMyCulture == cCultureAtlantean)
   {
      if (kbTechGetStatus(cTechIronWall) == cTechStatusObtainable)
      {
         // Yes this upgrade is actually available in the Heroic Age already, but that makes this script so complex to handle that too.
         int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechIronWall);
         if (planID == -1)
         {
            if (xsRandInt(0, 5) == 0)
            {
               researchSimpleTech(cTechIronWall, cUnitTypeAbstractWall, -1, prio);
            }
            else
            {
               debugTechs("Didn't roll correctly to start the Iron Wall research plan.");
            }
         }
         else
         {
            aiPlanSetPriority(planID, prio);
            debugTechs("We already have a research plan for Iron Wall going.");
         }
         return;
      }
      
      if (kbTechGetStatus(cTechOrichalcumWall) == cTechStatusObtainable)
      {
         int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechOrichalcumWall);
         if (planID == -1)
         {
            if (xsRandInt(0, 5) == 0)
            {
               researchSimpleTech(cTechOrichalcumWall, cUnitTypeAbstractWall, -1, prio);
            }
            else
            {
               debugTechs("Didn't roll correctly to start the Orichalcum Wall research plan.");
            }
         }
         else
         {
            aiPlanSetPriority(planID, prio);
            debugTechs("We already have a research plan for Orichalcum Wall going.");
         }
      }
      else
      {
         xsDisableRule("wallUpgradeMonitor");
      }
   }

   if (cMyCulture == cCultureChinese)
   {
      if (kbTechGetStatus(cTechGreatWall) == cTechStatusObtainable)
      {
         int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechGreatWall);
         if (planID == -1)
         {
            if (xsRandInt(0, 5) == 0)
            {
               researchSimpleTech(cTechGreatWall, cUnitTypeAbstractWall, -1, prio);
            }
            else
            {
               debugTechs("Didn't roll correctly to start the Great Wall research plan.");
            }
         }
         else
         {
            aiPlanSetPriority(planID, prio);
            debugTechs("We already have a research plan for Great Wall going.");
         }
      }
      else
      {
         xsDisableRule("wallUpgradeMonitor");
         return;
      }
   }
}

//==============================================================================
// siegeUpgradeMonitor
// Gets all the Siege upgrades.
//==============================================================================
rule siegeUpgradeMonitor
inactive
group defaultMythicRules
minInterval 60
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("siegeUpgradeMonitor");
      return;
   }
   // Easy doesn't get upgrades.
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("siegeUpgradeMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoResearchMilitaryUpgrades) == false)
   {
      return;
   }
   debugTechs("--- Running Rule siegeUpgradeMonitor ---");

   // We really want Engineers...
   if (kbTechGetStatus(cTechEngineers) == cTechStatusObtainable)
   {
      researchSimpleTech(cTechEngineers);
      return;
   }

   // That's enough for moderate.
   if (cDifficultyCurrent == cDifficultyModerate)
   {
      xsDisableRule("siegeUpgradeMonitor");
      return;
   }

   // Only put Draft Horses behind the global limit.
   if (areAtMaxConcurrentResearchPlans("siegeUpgradeMonitor") == true)
   {
      return;
   }

   if (kbTechGetStatus(cTechDraftHorses) == cTechStatusActive)
   {
      xsDisableRule("siegeUpgradeMonitor");
      return;
   }
   // If we're already researching we don't want to analyze it again.
   if (aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechDraftHorses) == -1)
   {
      // Draft Horses we care less about, only get it if we have some excess.
      if (haveExcessResourceAmount(500, cResourceFood) == true &&
          haveExcessResourceAmount(500, cResourceGold) == true)
      {
         researchSimpleTech(cTechDraftHorses);
      }
   }
}

//==============================================================================
// advancedFortificationsMonitor
//==============================================================================
rule advancedFortificationsMonitor
inactive
group defaultMythicRules
minInterval 60
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("omniscienceMonitor");
      return;
   }
   // Easy doesn't get upgrades.
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("omniscienceMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoResearchMilitaryUpgrades) == false)
   {
      return;
   }
   debugTechs("--- Running Rule advancedFortificationsMonitor ---");

   if (kbTechGetStatus(cTechAdvancedFortifications) == cTechStatusActive)
   {
      xsDisableRule("advancedFortificationsMonitor");
      return;
   }

   if (cPersonalityCurrent != cPersonalityDefender && areAtMaxConcurrentResearchPlans("advancedFortificationsMonitor") == true)
   {
      return;
   }

   bool haveEnoughFortresses = true;
   int fortressThreshold = selectByDifficulty(1, 1, 2, 2, 3, 3);
   int fortressCount = buildingGetNumberAliveAndPlanned(gFortressUnit);
   if (fortressCount < fortressThreshold)
   {
      haveEnoughFortresses = false;
   }

   int prio = 50;
   if (cPersonalityCurrent == cPersonalityDefender)
   {
      prio = 51;
   }

   // Already researching?
   int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechAdvancedFortifications);
   if (planID >= 0)
   {
      if (aiPlanGetState(planID) != cPlanStateResearch && haveEnoughFortresses == false)
      {
         aiPlanDestroy(planID);
         debugTechs("Cancelling Advanced Fortifications research plan because we no longer have the required amount of Fortresses.");
      }
      else
      {
         debugTechs("We already have a research plan for Advanced Fortifications.");
      }
      return;
   }

   researchSimpleTech(cTechAdvancedFortifications, gFortressUnit, -1, prio);
}

//==============================================================================
// omniscienceMonitor
//==============================================================================
rule omniscienceMonitor
inactive
group defaultMythicRules
minInterval 60
{
   if (cGameModeCurrent == cGameModeDeathmatch || cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("omniscienceMonitor");
      return;
   }
   // Easy doesn't get upgrades.
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("omniscienceMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoResearchMilitaryUpgrades) == false)
   {
      return;
   }
   debugTechs("--- Running Rule omniscienceMonitor ---");

   if (kbTechGetStatus(cTechOmniscience) == cTechStatusActive)
   {
      xsDisableRule("omniscienceMonitor");
      return;
   }

   float omniscienceCost = kbAICostGetTechCost(cTechOmniscience);
   // Already researching?
   int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechOmniscience);
   if (planID >= 0)
   {
      if (aiPlanGetState(planID) != cPlanStateResearch && haveExcessResourceAmount(omniscienceCost, cResourceGold) == false)
      {
         aiPlanDestroy(planID);
         debugTechs("Cancelling Omniscience research plan because we no longer have the required gold excess needed.");
      }
      else
      {
         debugTechs("We already have a research plan for Omniscience.");
      }
      return;
   }

   if (haveExcessResourceAmount(omniscienceCost, cResourceGold) == true)
   {
      researchSimpleTech(cTechOmniscience);
   }
   else
   {
      debugTechs("Don't have enough excess gold to get Omniscience: " + gResourceNeeds[cResourceGold] + "/" + omniscienceCost + ".");
   }
}

//==============================================================================
// secretsOfTheTitansMonitor
// Via the Wonder we can obtain more chances to research Secrets of the Titans, don't disable this rule.
//==============================================================================
rule secretsOfTheTitansMonitor
inactive
group defaultMythicRules
minInterval 60
{
   if (cGameAllowTitans == false || cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("secretsOfTheTitansMonitor");
      return;
   }
   // We allow Easy to get a titan.
   if (checkStrategyFlag(cStrategyFlagBuildTitan) == false)
   {
      return;
   }
   debugTechs("--- Running Rule secretsOfTheTitansMonitor ---");

   // If we already have a charge of this god power, don't obtain another one.
   if (kbGodPowerGetNumCharges(cProtoPowerTitanGate, cMyID) >= 1)
   {
      return;
   }
   if (kbTechGetStatus(cTechSecretsOfTheTitans) != cTechStatusObtainable)
   {
      return;
   }

   // Are we already researching?
   if (aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechSecretsOfTheTitans) != -1)
   {
      return;
   }

   if (cPersonalityCurrent == cPersonalityMythical)
   {
      float[] cost = kbTechGetCost(cTechSecretsOfTheTitans);
      for (int i = 0; i < cNumberResources; i++)
      {
         // For mythical personality we skip all resource needs, and just get the tech if we have enough resources banked.
         if (kbResourceGet(i) < cost[i])
         {
            debugTechs("We don't have a big enough bank of " + kbGetResourceName(i) + " to afford Secrets of the Titans instantly.");
            return;
         }
      }
   }
   else
   {
      const float requiredExcess = 1500.0;
      if (haveExcessResourceAmount(requiredExcess) == false)
      {
         debugTechs("We don't have " + requiredExcess + " excess resources across the board, can't research Secrets of the Titans.");
         return;
      }
      float favorCost = kbTechGetCostPerResource(cTechSecretsOfTheTitans, cResourceFavor);
      if (kbResourceGet(cResourceFavor) < favorCost)
      {
         debugTechs("We don't have " + favorCost + " favor banked, can't research Secrets of the Titans.");
         return;
      }
   }
   
   // Get this fast now that we have the resources.
   researchSimpleTech(cTechSecretsOfTheTitans, -1, -1, 99);
}

//==============================================================================
// marketUpgradeMonitor
//==============================================================================
rule marketUpgradeMonitor
inactive
group defaultMythicRules
minInterval 60
{
   if (cGameModeCurrent == cGameModeDeathmatch)
   {
      xsDisableRule("marketUpgradeMonitor");
      return;
   }
   if (cStartingResourcesCurrent == cStartingResourcesInfinite)
   {
      xsDisableRule("economyUpgradeManager");
      return;
   }
   // Easy doesn't get upgrades.
   if (cDifficultyCurrent == cDifficultyEasy)
   {
      xsDisableRule("marketUpgradeMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoResearchEconomyUpgrades) == false)
   {
      return;
   }
   debugTechs("--- Running Rule marketUpgradeMonitor ---");

   // AutumnOfAbundance is ignored because because it buffs Pixiu which we don't train.

   if (kbTechGetStatus(cTechTaxCollectors) == cTechStatusActive &&
       kbTechGetStatus(cTechAmbassadors) == cTechStatusActive &&
       kbTechGetStatus(cTechCoinage) == cTechStatusActive &&
       (cMyCulture != cCultureChinese || kbTechGetStatus(cTechSilkRoad) == cTechStatusActive))
   {
      xsDisableRule("marketUpgradeMonitor");
      return;
   }

   if (kbUnitCount(gMarketUnit, cMyID, cUnitStateAlive) <= 0)
   {
      debugTechs("We have no Markets currently, not attempting to research any Market upgrades.");
      return;
   }

   // Coinage and Silk Road have the same prereqs.
   if (kbTechGetStatus(cTechCoinage) == cTechStatusObtainable ||
       (cMyCulture == cCultureChinese && kbTechGetStatus(cTechSilkRoad) == cTechStatusObtainable))
   {
      int techID = cTechCoinage;
      // If we already have Coinage we switch to Silk Road.
      if (cMyCulture == cCultureChinese && kbTechGetStatus(cTechCoinage) == cTechStatusActive)
      {
         techID = cTechSilkRoad;
      }
      int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, techID);
      int numCaravans = kbUnitCount(gCaravanUnit, cMyID, cUnitStateAlive);
      int threshold = cPersonalityCurrent == cPersonalityEconomist ? 6 : 8;
      if (planID >= 0)
      {
         if (aiPlanGetState(planID) == cPlanStateResearch)
         {
            debugTechs("We're already researching " + kbTechGetName(techID) + ".");
         }
         else // We could cancel it.
         {
            if (numCaravans < threshold)
            {
               aiPlanDestroy(planID);
               debugTechs("Destroying " + kbTechGetName(techID) + " plan because we have too few " +
                  kbProtoUnitGetName(gCaravanUnit) + " left.");
            }
            else if (tradeInformation.mState != cTradeStateTrading)
            {
               aiPlanDestroy(planID);
               debugTechs("Destroying " + kbTechGetName(techID) + " plan because we have no functional trade route left.");
            }
            else
            {
               debugTechs("Plan for researching " + kbTechGetName(techID) + " needs no changes.");
               return;
            }
         }
      }
      else // Don't have a plan.
      {
         if (tradeInformation.mState != cTradeStateTrading)
         {
            debugTechs("Can't start researching " + kbTechGetName(techID) + " because we have no functional trade route.");
         }
         else if (numCaravans >= threshold)
         {
            researchSimpleTech(techID);
            return;
         }
         else
         {
            debugTechs("We have too few " + kbProtoUnitGetName(gCaravanUnit) + ", not researching " + kbTechGetName(techID) + ": "
               + numCaravans + "/" + threshold + ".");
         }
      }
   }

   // Only put Tax Collectors + Ambassadors behind the global limit.
   if (areAtMaxConcurrentResearchPlans("marketUpgradeMonitor") == true)
   {
      return;
   }

   if (kbTechGetStatus(cTechTaxCollectors) == cTechStatusObtainable)
   {
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechTaxCollectors) != -1)
      {
         return;
      }
      if (gNumMarketUsage >= 10)
      {
         researchSimpleTech(cTechTaxCollectors);
      }
      else
      {
         debugTechs("We haven't used the Market trading enough, not researching Tax Collectors: " + gNumMarketUsage + "/10.");
      }
      return;
   }

   if (kbTechGetStatus(cTechAmbassadors) == cTechStatusObtainable)
   {
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechAmbassadors) != -1)
      {
         return;
      }
      if (gNumMarketUsage >= 20)
      {
         researchSimpleTech(cTechAmbassadors);
      }
      else
      {
         debugTechs("We haven't used the Market trading enough, not researching Ambassadors: " + gNumMarketUsage + "/20.");
      }
   }
}