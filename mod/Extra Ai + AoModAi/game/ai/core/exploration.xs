//==============================================================================
/* exploration.xs

   This file is intended for any exploration implementation, including both land and
   naval exploration.

*/
//==============================================================================

//==============================================================================
// helperExploreOtherIslands
//==============================================================================
void helperExploreOtherIslands(int planID = -1)
{
   debugExploration("*** helperExploreOtherIslands for " + aiPlanGetName(planID) + " ***");

   aiPlanSetNumberVariableValues(planID, cExplorePlanExploreAreaGroupIDs, gNumberLandAreaGroups, false);
   for (int iGroup = 0; iGroup < gNumberLandAreaGroups; iGroup++)
   {
      aiPlanSetVariableInt(planID, cExplorePlanExploreAreaGroupIDs, iGroup, gLandAreaGroups[iGroup]);
      debugExploration("   Added " + gLandAreaGroups[iGroup] + " to our plan to go to.");
   }
}

//==============================================================================
// helperExploreStartingSurroundings
// In the source this mechanic will skip areas that we consider too dangerous or can't path to at that moment.
// Meaning that it's not guaranteed that we scout all areas we assign to the plan here.
// Which in turn means that we have to keep running this in all scouting rules until we finally scout everything.
//==============================================================================
void helperExploreStartingSurroundings(int planID = -1)
{
   if (gFullyExploredStartingSurroundings == true)
   {
      aiEchoWarning("We shouldn't be calling helperExploreStartingSurroundings when gFullyExploredStartingSurroundings == true.");
      return;
   }
   debugExploration("*** helperExploreStartingSurroundings for plan " + aiPlanGetName(planID) + " ***");

   static int[] areasToScout = default;

   // We analyze this entire thing once, we assume our starting position isn't going to change and that we would then want to call
   // this again.
   static bool firstRun = true;
   if (firstRun == true)
   {
      firstRun = false;

      vector startingPosition = kbPlayerGetStartingPosition(cMyID);
      if (startingPosition == cInvalidVector || gMapInfo.mIsNomadMap == true)
      {
         int mainBaseID = kbBaseGetMainID(cMyID);
         if (mainBaseID != -1)
         {
            startingPosition = kbBaseGetLocation(cMyID, mainBaseID);
         }
      }
      if (kbGetIsLocationOnMap(startingPosition) == false)
      {
         debugExploration("We couldn't find a vector that can serve as a starting position, can't scout surroundings now.");
         gFullyExploredStartingSurroundings = true;
         return;
      }

      int[] validTypes = new int(3, -1);
      validTypes[0] = cAreaTypePassableLand;
      validTypes[1] = cAreaTypeGold;
      validTypes[2] = cAreaTypeSettlement;
      areasToScout = kbAreaGetIDsByPositionAndRange(startingPosition, 85.0, validTypes, true, cPassabilityLand);
   }

   // Only thing that we must verify each team is how much % everything is explored.
   for (int i = areasToScout.size() - 1; i >= 0; i--)
   {
      int areaID = areasToScout[i];
      if (kbAreaGetPercentExplored(areaID) == 1.0)
      {
         debugExploration("   AreaID: " + areaID + " is already fully explored.");
         areasToScout.removeIndex(i);
         continue;
      }
      debugExploration("   Added areaID: " + areaID + " to the plan to explore.");
   }

   if (areasToScout.size() == 0)
   {
      debugExploration("Scouted all of our starting surroundings!");
      gFullyExploredStartingSurroundings = true;
      return;
   }
   aiPlanSetNumberVariableValues(planID, cExplorePlanExploreAreaIDs, areasToScout.size());
   for (int i = 0; i < areasToScout.size(); i++)
   {
      aiPlanSetVariableInt(planID, cExplorePlanExploreAreaIDs, i, areasToScout[i]);
   }
}

//==============================================================================
// scoutingMonitor
//==============================================================================
rule scoutingMonitor
inactive
group defaultArchaicRules
minInterval 10
{
   // Greeks use Pegasus + Kataskopos in another rule, Odin uses Ravens in another rule.
   // Humanoids don't have access to these Myth scouts so they do want to run this.
   // Aztecs use Quimichin Spies instead.
   if (((cMyCulture == cCultureGreek || cMyCiv == cCivOdin) && cPersonalityCurrent != cPersonalityHumanoid) ||
       cMyCulture == cCultureAztec)
   {
      xsDisableRule("scoutingMonitor");
      return;
   }

   static int[] scoutPlans = default;
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      for (int i = 0; i < scoutPlans.size(); i++)
      {
         if (aiPlanGetIsIDValid(scoutPlans[i]) == true)
         {
            aiPlanDestroy(scoutPlans[i]);
         }
      }
      scoutPlans.clear();
      return;
   }
   debugExploration("--- Running Rule scoutingMonitor. ---");

   // Clear out dead plans.
   for (int i = scoutPlans.size() - 1; i >= 0 ; i--)
   {
      if (aiPlanGetIsIDValid(scoutPlans[i]) == false)
      {
         scoutPlans.removeIndex(i);
      }
   }

   // Fixup starting surrounding if we have plan(s).
   if (gFullyExploredStartingSurroundings == false)
   {
      if (scoutPlans.size() >= 1)
      {
         int planID = scoutPlans[0];
         // If we don't have an areaID saved in this plan yet we know we need to set it up still.
         // Only do this for the first plan otherwise we create a useless train.
         if (aiPlanGetVariableInt(planID, cExplorePlanExploreAreaIDs, 0) == -1)
         {
            helperExploreStartingSurroundings(planID);
         }
      }
   }

   int requiredScoutPlans = 1; 
   if (cPersonalityCurrent == cPersonalityRetaliator)
   {
      requiredScoutPlans = 3;
   }
   debugExploration("We want " + requiredScoutPlans + " default explore plans.");

   // If we already have all the plans that we want we need to make sure they have units.
   if (scoutPlans.size() == requiredScoutPlans)
   {
      if (aiPlanGetState(gPrimaryLandDefendPlan) == cPlanStateAttack)
      {
         debugExploration("Our primary land defend plan is engaged, can't pick new scouts now.");
         return;
      }

      bool useKitsune = false;
      bool usePioneer = false;
      int[] units = new int(0, 0);
      if (cMyCulture == cCultureEgyptian)
      {
         units = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypePriest);
      }
      else if (cMyCulture == cCultureChinese)
      {
         // In age1 we have no human soldiers available, only our Pioneer, use it if possible.
         if (kbPlayerGetAge(cMyID) == cAge1)
         {
            units = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypePioneer);
            if (units.size() == 0)
            {
               // Yes we're in Archaic, just leave this here as potential backup.
               units = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypeHumanSoldier);
            }
            else
            {
               usePioneer = true;
            }
         }
      }
      else if (cMyCulture == cCultureJapanese)
      {
         // First try a Kitsune, if not found do the regular check.
         units = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypeKitsune);
         if (units.size() == 0)
         {
            // Don't scout with heroes / myth units, they're too valuable.
            units = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypeHumanSoldier);
         }
         else
         {
            useKitsune = true;
         }
      }
      else
      {
         // Don't scout with heroes / myth units, they're too valuable.
         units = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypeHumanSoldier);
      }
      if (units.size() == 0)
      {
         debugExploration("Our primary land defend plan has no valid scout units in it, can't pick new scouts now. " +
            "Our existing scout plans may already have units in them though, they will go on just fine.");
         return;
      }

      for (int i = 0; i < scoutPlans.size(); i++)
      {
         if (cMyCulture == cCultureEgyptian)
         {
            int planID = scoutPlans[i];
            // Already at max capacity.
            if (aiPlanGetNumberUnits(planID) >= 1)
            {
               continue;
            }
            debugExploration("Added Priest(" + units[0] + ") to " + aiPlanGetName(planID) + ".");
            aiPlanAddUnit(planID, units[0]);
            units.removeIndex(0);
            if (units.size() == 0)
            {
               break;
            }
         }
         else
         {
            int planID = scoutPlans[i];
            // Already at max capacity.
            if (aiPlanGetNumberUnits(planID) >= 1)
            {
               continue;
            }
            if (useKitsune == true)
            {
               debugExploration("Added Kitsune(" + units[0] + ") to " + aiPlanGetName(planID) + ".");
               aiPlanAddUnit(planID, units[0]);
               units.removeIndex(0);
            }
            else if (usePioneer == true)
            {
               debugExploration("Added Pioneer(" + units[0] + ") to " + aiPlanGetName(planID) + ".");
               aiPlanAddUnit(planID, units[0]);
               units.removeIndex(0);
            }
            else
            {
               for (int j = 0; j < units.size(); j++)
               {
                  int protoUnitID = kbUnitGetProtoUnitID(units[j]);
                  // Send out cheap units. Don't fetch current cost here since then low hp strong units could go while we could heal them.
                  if (kbAICostGetProtoUnitCost(protoUnitID) < 120.0)
                  {
                     debugExploration("Added " + kbProtoUnitGetName(protoUnitID) + " to " + aiPlanGetName(planID) + ".");
                     aiPlanAddUnit(planID, units[j]);
                     units.removeIndex(j);
                     break;
                  }
               }
            }
            if (units.size() == 0)
            {
               break;
            }
         }
      }
      return;
   }

   if (scoutPlans.size() > requiredScoutPlans)
   {
      for (int i = scoutPlans.size() - 1; i >= requiredScoutPlans; i--)
      {
         aiPlanDestroy(scoutPlans[i]);
         scoutPlans.removeIndex(i);
      }
   }

   for (int i = scoutPlans.size(); i < requiredScoutPlans; i++)
   {
      // Create the plans needed.
      int planID = -1;
      if (cMyCulture == cCultureEgyptian)
      {
         // We scout with Priests and build Obelisks.
         planID = aiPlanCreate("Default Explore", cPlanExplore, -1, gExplorationCategoryID);
         aiPlanSetPriority(planID, 50);
         aiPlanAddUnitType(planID, cUnitTypePriest, 1, 1, 1);
         aiPlanSetVariableBool(planID, cExplorePlanCanBuildOutpost, 0, true);
         aiPlanSetVariableInt(planID, cExplorePlanOutpostPUID, 0, cUnitTypeObelisk);
         aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true); // Manual assignment.
         scoutPlans.add(planID);
      }
      else
      {
         planID = aiPlanCreate("Default Explore", cPlanExplore, -1, gExplorationCategoryID);
         aiPlanSetPriority(planID, 50);
         aiPlanAddUnitType(planID, cUnitTypeHumanSoldier, 1, 1, 1);
         if (cMyCulture == cCultureChinese)
         {
            // Create room for a potential Pioneer to be assigned.
            aiPlanAddUnitType(planID, cUnitTypePioneer, 1, 1, 1);
         }
         else if (cMyCulture == cCultureJapanese)
         {
            // Create room for a potential Kitsune to be assigned.
            aiPlanAddUnitType(planID, cUnitTypeKitsune, 1, 1, 1);
         }
         aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true); // Manual assignment.
         scoutPlans.add(planID);
      }
      // Explore other islands if we need to.
      if (gMapInfo.mIsIslandMap == true)
      {
         helperExploreOtherIslands(planID);
      }
   }

   // Run again to populate the plans with units.
   xsRuleIgnoreIntervalOnce("scoutingMonitor");
}

//==============================================================================
// armyScoutingMonitor
//==============================================================================
rule armyScoutingMonitor
inactive
group defaultClassicalRules
minInterval 10
{
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      for (int i = 0; i < gArmyScoutPlans.size(); i++)
      {
         if (aiPlanGetIsIDValid(gArmyScoutPlans[i]) == true)
         {
            aiPlanDestroy(gArmyScoutPlans[i]);
         }
      }
      gArmyScoutPlans.clear();
      return;
   }
   if (cPersonalityCurrent == cPersonalityPassive || cPersonalityCurrent == cPersonalityRetaliator)
   {
      xsDisableRule("armyScoutingMonitor");
      return;
   }
   debugExploration("--- Running Rule armyScoutingMonitor. ---");

   bool attackKOTH = false;
   if ((cVictoryTypesCurrent & cVictoryTypeKingOfTheHill) != 0 && gKOTHIsOwnedByAllies == false)
   {
      debugExploration("KOTH isn't owned by us/allies, don't scout but send all units to it.");
      attackKOTH = true;
   }

   // Clear out invalid plans and find the master plan.
   int masterPlanID = -1;
   for (int i = gArmyScoutPlans.size() - 1; i >= 0; i--)
   {
      if (aiPlanGetIsIDValid(gArmyScoutPlans[i]) == false)
      {
         gArmyScoutPlans.removeIndex(i);
         continue;
      }
      // If this plan has no plan assigned to be its master, it must be the master plan itself.
      if (aiPlanGetVariableInt(gArmyScoutPlans[i], cExplorePlanMasterExploreAreasPlan, 0) == -1)
      {
         if (masterPlanID != -1)
         {
            aiEchoWarning("We have two master plans?");
         }
         masterPlanID = gArmyScoutPlans[i];
         debugExploration("Found master plan: " + aiPlanGetName(masterPlanID) + ".");
      }
   }

   if (gAttackManager.mScoutingState == cScoutingForEnemies && attackKOTH == false && gDefensivelyOverrun == false)
   {
      int scoutingGroupSize = selectByDifficulty(1, 2, 3, 5, 6, 7);
      int[] excludeTypes = new int(1, cUnitTypeAbstractSiegeWeapon);
      int[] units = aiPlanGetUnits(gPrimaryLandDefendPlan, -1, false, excludeTypes);
      int numUnits = units.size();

      int maxScoutGroups = selectByDifficulty(1, 2, 4, 8, 10, 12);
      int scoutGroupsToCreate = min(maxScoutGroups - gArmyScoutPlans.size(), numUnits/ scoutingGroupSize);
      int iUnit = 0;

      debugExploration("Scouting with army, creating " + scoutGroupsToCreate + " new army scout groups.");
      for (int i = 0; i < scoutGroupsToCreate; i++)
      {
         // Create explore plans.
         int planID = aiPlanCreate("Army Explore: " + gArmyScoutPlans.size() + 1, cPlanExplore, -1, gExplorationCategoryID);
         aiPlanSetPriority(planID, 50);
         aiPlanSetVariableBool(planID, cExplorePlanAggressiveScouts, 0, true);
         aiPlanSetVariableBool(planID, cExplorePlanAvoidingAttackedAreas, 0, false);
         setDefaultExplorePlanTargetUnitTypes(planID);
         // Don't take the same units in multiple runs.
         for (; iUnit < (i + 1) * scoutingGroupSize && iUnit < numUnits; iUnit++)
         {
            aiPlanAddUnitType(planID, kbUnitGetProtoUnitID(units[iUnit]), 1, 1, 1, true);
            aiPlanAddUnit(planID, units[iUnit]);
         }
         aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true);
         aiPlanSetFlag(planID, cPlanFlagDestroyWhenNoUnitsLeft, true);
         // Explore other islands if we need to.
         if (gMapInfo.mIsIslandMap == true)
         {
            helperExploreOtherIslands(planID);
         }
         if (masterPlanID != -1)
         {
            aiPlanSetVariableInt(planID, cExplorePlanMasterExploreAreasPlan, 0, masterPlanID);
            debugExploration("Assigning master plan: " + aiPlanGetName(masterPlanID) + " to new explore plan: " +
               aiPlanGetName(planID) + ".");
         }
         else
         {
            masterPlanID = planID;
            debugExploration("This is now our master plan: " + aiPlanGetName(masterPlanID) + ".");
         }

         addPlanToAllSuitableGodpowerPlans(planID);

         gArmyScoutPlans.add(planID);
      }
   }
   else
   {
      for (int i = gArmyScoutPlans.size() - 1; i >= 0; i--)
      {
         // If we have a plan in attack/transport state it will be removed eventually when we leave that state.
         int planState = aiPlanGetState(gArmyScoutPlans[i]);
         if (planState != cPlanStateAttack && planState != cPlanStateTransport)
         {
            aiPlanDestroy(gArmyScoutPlans[i]);
            gArmyScoutPlans.removeIndex(i);
         }
      }
   }
}

//==============================================================================
// navalScoutingMonitor
//==============================================================================
rule navalScoutingMonitor
inactive
group defaultClassicalRules
minInterval 10
{
   if (gMapInfo.mHasWater == false)
   {
      xsDisableRule("navalScoutingMonitor");
      return;
   }
   static int[] navalScoutPlans = default;
   if (checkStrategyFlag(cStrategyFlagAutomaticNavalScouting) == false)
   {
      for (int i = 0; i < navalScoutPlans.size(); i++)
      {
         if (aiPlanGetIsIDValid(navalScoutPlans[i]) == true)
         {
            aiPlanDestroy(navalScoutPlans[i]);
         }
      }
      navalScoutPlans.clear();
      return;
   }

   debugExploration("--- Running Rule navalScoutingMonitor. ---");

   if (gShouldBuildDock == false)
   {
      debugExploration("Currently we don't want to build a Dock, can't scout.");
      for (int i = 0; i < navalScoutPlans.size(); i++)
      {
         if (aiPlanGetIsIDValid(navalScoutPlans[i]) == true)
         {
            aiPlanDestroy(navalScoutPlans[i]);
         }
      }
      navalScoutPlans.clear();
      return;
   }
   if (aiPlanGetIsIDValid(gPrimaryNavalDefendPlan) == false)
   {
      debugExploration("gPrimaryNavalDefendPlan isn't valid yet, waiting for the defend logic to run.");
      return;
   }

   // Current max is 1 plans, but have the setup for more. If this is increased the distribution of Ships must be changed.
   const int cMaxNavalScoutPlans = 1;
   
   // Clear out invalid plans.
   for (int i = navalScoutPlans.size() - 1; i >= 0; i--)
   {
      if (aiPlanGetIsIDValid(navalScoutPlans[i]) == false)
      {
         navalScoutPlans.removeIndex(i);
      }
   }

   // Very rare stuck case: we're already doing an aggro scout in a water group ID that we're now no longer docking in.
   // Then we can't make a new plan for our new water group ID.

   if (gNavalAttackManager.mScoutingState == cScoutingForEnemies)
   {
      int currentAmountOfPlans = navalScoutPlans.size();
      if (currentAmountOfPlans < cMaxNavalScoutPlans)
      {
         for (int i = currentAmountOfPlans; i < cMaxNavalScoutPlans; i++)
         {
            if (aiPlanGetNumberUnits(gPrimaryNavalDefendPlan, -1, false) == 0)
            {
               debugExploration("Can't create a new army naval explore plan because our naval defend plan is empty.");
               return;
            }
            // Create water explore plan.
            int planID = aiPlanCreate("Army Naval Explore: ", cPlanExplore, -1, gExplorationCategoryID);
            // Don't fight with these plans for these personalities.
            if (cPersonalityCurrent == cPersonalityRetaliator || cPersonalityCurrent == cPersonalityPassive)
            {
               aiPlanAddUnitType(planID, cUnitTypeLogicalTypeNavalMilitary, 1, 1, 1);
            }
            else
            {
               aiPlanAddUnitType(planID, cUnitTypeLogicalTypeNavalMilitary, 0, 0, 200);
               transferAllUnitsBetweenTwoPlans(gPrimaryNavalDefendPlan, planID);
               aiPlanSetVariableBool(planID, cExplorePlanAggressiveScouts, 0, true);
               aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true);
               aiPlanSetFlag(planID, cPlanFlagDestroyWhenNoUnitsLeft, true);
               setDefaultExplorePlanTargetUnitTypes(planID);
            }
            aiPlanSetVariableBool(planID, cExplorePlanAvoidingAttackedAreas, 0, false);
            aiPlanSetVariableInt(planID, cExplorePlanWaterAreaGroupID, 0, gDockAreaGroupID);
            aiPlanSetVariableBool(planID, cExplorePlanNaval, 0, true);
            aiPlanSetPriority(planID, 50);
            navalScoutPlans.add(planID);
            addPlanToAllSuitableGodpowerPlans(planID);
            debugExploration("Created new army naval explore plan: " + aiPlanGetName(planID) + ".");
         }
      }
   }
   else
   {
      for (int i = navalScoutPlans.size() - 1; i >= 0; i--)
      {
         // If we have a plan in attack state it will be removed eventually when we leave that state.
         int planState = aiPlanGetState(navalScoutPlans[i]);
         if (planState != cPlanStateAttack)
         {
            aiPlanDestroy(navalScoutPlans[i]);
            navalScoutPlans.removeIndex(i);
         }
      }
   }
}

//==============================================================================
// transportScoutingMonitor
// Explore with transport in Archaic, keep it at home later since it's likely we need to transport.
//==============================================================================
rule transportScoutingMonitor
inactive
group defaultArchaicRules
minInterval 10
{
   static int planID = -1;
   if (gMapInfo.mStartsWithTransport == false)
   {
      if (aiPlanGetIsIDValid(planID) == true)
      {
         aiPlanDestroy(planID);
         planID = -1;
      }
      xsDisableRule("transportScoutingMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagScoutWithStartingTransport) == false)
   {
      if (aiPlanGetIsIDValid(planID) == true)
      {
         aiPlanDestroy(planID);
         planID = -1;
      }
      return;
   }

   debugExploration("--- Running Rule transportScoutingMonitor. ---");
   
   if (gShouldBuildDock == false)
   {
      debugExploration("We currently don't want a Dock, can't scout.");
      if (aiPlanGetIsIDValid(planID) == true)
      {
         aiPlanDestroy(planID);
         planID = -1;
      }
      return;
   }

   // We want the transport back to actually transport now.
   if (kbPlayerGetAge(cMyID) >= cAge2)
   {
      if (aiPlanGetIsIDValid(planID) == true)
      {
         if (aiPlanGetNumberUnits(planID, -1, false) > 0)
         {
            int unitID = aiPlanGetUnitIDByIndex(planID, 0);
            // Remove the unit from the plan so that the move command works properly.
            aiPlanRemoveUnit(planID, unitID);
            aiTaskMoveUnit(unitID, gWaterDefendPoint);
         }
         aiPlanDestroy(planID);
      }
      planID = -1;
      xsDisableRule("transportScoutingMonitor");
      return;
   }

   if (aiPlanGetIsIDValid(planID) == false)
   {
      // Create explore plan.
      planID = aiPlanCreate("Transport Explore", cPlanExplore, -1, gExplorationCategoryID);
      aiPlanSetVariableInt(planID, cExplorePlanWaterAreaGroupID, 0, gDockAreaGroupID);
      aiPlanSetVariableBool(planID, cExplorePlanNaval, 0, true);
      aiPlanSetPriority(planID, 1);
      aiPlanAddUnitType(planID, cUnitTypeTransport, 1, 1, 1);
   }

   if (aiPlanGetNumberUnits(planID) == 0)
   {
      int transportID = getUnit(cUnitTypeAbstractTransportShip);
      if (transportID == -1) // If we lost our starting transport it's done.
      {
         aiPlanDestroy(planID);
         planID = -1;
         xsDisableRule("transportScoutingMonitor");
         return;
      }
      // Only assign if we're not already in a plan, this is to prevent stealing from BO plans / real transport plans.
      if (kbUnitGetPlanID(transportID) == -1)
      {
         debugExploration("Added " + kbProtoUnitGetName(kbUnitGetProtoUnitID(transportID)) + " to " + aiPlanGetName(planID) + ".");
         aiPlanAddUnit(planID, transportID);
      }
   }
}

//==============================================================================
// kataskoposManager
//==============================================================================
rule kataskoposManager
inactive
group defaultArchaicRules
minInterval 30
{
   if (cMyCulture != cCultureGreek)
   {
      xsDisableRule("kataskoposManager");
      return;
   }
   debugExploration("--- Running Rule kataskoposManager. ---");

   static int kataskoposScoutPlanID = -1;
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      if (aiPlanGetIsIDValid(kataskoposScoutPlanID) == true)
      {
         aiPlanDestroy(kataskoposScoutPlanID);
         kataskoposScoutPlanID = -1;
      }
      return;
   }

   int kataskoposID = getUnit(cUnitTypeKataskopos);
   if (kataskoposID == -1)
   {
      if (xsGetTime() < 5)
      {
         // Wait for it to spawn.
         debugExploration("We can't find a Kataskopos yet but it's also early in the game, wait for it to spawn.");
         xsSetRuleMinInterval("kataskoposManager", 5); // Run again once it's spawned presumably.
         return;
      }
      if (aiPlanGetIsIDValid(kataskoposScoutPlanID) == true)
      {
         aiPlanDestroy(kataskoposScoutPlanID);
      }
      kataskoposScoutPlanID = -1;
      debugExploration("We lost our Kataskopos, disabling kataskoposManager");
      xsDisableRule("kataskoposManager");
      return;
   }
   xsSetRuleMinInterval("kataskoposManager", 30); // Reset interval.

   // We let the Pegasus that we train do the starting surrounding scouting since it can fly...
   if (aiPlanGetIsIDValid(kataskoposScoutPlanID) == false)
   {
      kataskoposScoutPlanID = aiPlanCreate("Kataskopos Explore", cPlanExplore, -1, gExplorationCategoryID);
      aiPlanSetPriority(kataskoposScoutPlanID, 50);
      aiPlanAddUnitType(kataskoposScoutPlanID, cUnitTypeKataskopos, 1, 1, 1);
      aiPlanSetFlag(kataskoposScoutPlanID, cPlanFlagCantBeStolenFrom, true);
      // Explore other islands if we need to.
      if (gMapInfo.mIsIslandMap == true)
      {
         helperExploreOtherIslands(kataskoposScoutPlanID);
      }
      debugExploration("Created plan: " + aiPlanGetName(kataskoposScoutPlanID) + ".");
   }

   if (isUnitAlreadyInPlanOrChildOf(kataskoposID, kataskoposScoutPlanID) == false)
   {
      // Forcibly add the unit since we always want it to be in this plan.
      aiPlanAddUnit(kataskoposScoutPlanID, kataskoposID);
      debugExploration("Added Kataskopos: " + kataskoposID + " to " + aiPlanGetName(kataskoposScoutPlanID) + ".");
   }
}

//==============================================================================
// hippocampusManager
//==============================================================================
rule hippocampusManager
inactive
group defaultArchaicRules
minInterval 30
{
   if (cMyCiv != cCivPoseidon || cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("hippocampusManager");
      return;
   }
   if (gMapInfo.mHasWater == false)
   {
      debugExploration("Disabling hippocampusManager because there is no water on the map.");
      xsDisableRule("hippocampusManager");
      return;
   }
   
   debugExploration("--- Running Rule hippocampusManager. ---");

   static int hippocampusScoutPlan = -1;
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      if (aiPlanGetIsIDValid(hippocampusScoutPlan) == true)
      {
         aiPlanDestroy(hippocampusScoutPlan);
         hippocampusScoutPlan = -1;
      }
      return;
   }

   if (gShouldBuildDock == false)
   {
      debugExploration("We currently don't want to build a Dock, can't scout.");
      if (aiPlanGetIsIDValid(hippocampusScoutPlan) == true)
      {
         aiPlanDestroy(hippocampusScoutPlan);
         hippocampusScoutPlan = -1;
      }
   }

   // We assume that the Hippocampus will always be added to this plan automatically since it's not naval military.
   if (aiPlanGetIsIDValid(hippocampusScoutPlan) == false)
   {
      hippocampusScoutPlan = aiPlanCreate("Hippocampus Explore", cPlanExplore, -1, gExplorationCategoryID);
      aiPlanSetVariableBool(hippocampusScoutPlan, cExplorePlanNaval, 0, true);
      aiPlanSetVariableInt(hippocampusScoutPlan, cExplorePlanWaterAreaGroupID, 0, gDockAreaGroupID);
      aiPlanSetPriority(hippocampusScoutPlan, 50);
      aiPlanAddUnitType(hippocampusScoutPlan, cUnitTypeHippocampus, 1, 1, 1);
   }
}

//==============================================================================
// pegasusScoutingMonitor
//==============================================================================
rule pegasusScoutingMonitor
group defaultArchaicRules
inactive
minInterval 30
{
   if (cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("pegasusScoutingMonitor");
      return;
   }
   static int[] pegasusScoutPlans = default;
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      for (int i = 0; i < pegasusScoutPlans.size(); i++)
      {
         if (aiPlanGetIsIDValid(pegasusScoutPlans[i]) == true)
         {
            aiPlanDestroy(pegasusScoutPlans[i]);
         }
      }
      pegasusScoutPlans.clear();
      return;
   }
   debugExploration("--- Running Rule pegasusScoutingMonitor ---");
   
   int queryID = useSimpleUnitQuery(cUnitTypePegasus);
   int numberFound = kbUnitQueryExecute(queryID);
   int[] pegasi = kbUnitQueryGetResults(queryID);

   useSimpleUnitQuery(cUnitTypePegasusWingedMessenger);
   numberFound = kbUnitQueryExecute(queryID);
   for (int i = 0; i < numberFound; i++)
   {
      pegasi.add(kbUnitQueryGetResult(queryID, i));
   }

   useSimpleUnitQuery(cUnitTypePegasusBridleOfPegasus);
   numberFound = kbUnitQueryExecute(queryID);
   for (int i = 0; i < numberFound; i++)
   {
      pegasi.add(kbUnitQueryGetResult(queryID, i));
   }
   
   // Clear out invalid plans.
   for (int i = pegasusScoutPlans.size() - 1; i >= 0; i--)
   {
      if (aiPlanGetIsIDValid(pegasusScoutPlans[i]) == false)
      {
         pegasusScoutPlans.removeIndex(i);
      }
   }

   for (int i = 0; i < pegasi.size(); i++)
   {
      if (kbUnitGetPlanID(pegasi[i]) == -1)
      {
         int puid = kbUnitGetProtoUnitID(pegasi[i]);
         int planID = aiPlanCreate(kbProtoUnitGetName(puid) + " Explore", cPlanExplore, -1, gExplorationCategoryID);
         aiPlanSetPriority(planID, 50);
         aiPlanSetFlag(planID, cPlanFlagCantBeStolenFrom, true);
         aiPlanAddUnitType(planID, puid, 1, 1, 1);
         aiPlanAddUnit(planID, pegasi[i]);
         // Explore other islands if we need to.
         if (gMapInfo.mIsIslandMap == true)
         {
            helperExploreOtherIslands(planID);
         }
         pegasusScoutPlans.add(planID);
      }
      // Only first Pegasus does this, otherwise we're forming a useless train.
      if (i == 0)
      {
         // Sanity check.
         int planID = kbUnitGetPlanID(pegasi[i]);
         if (aiPlanGetType(planID) == cPlanExplore && gFullyExploredStartingSurroundings == false)
         {
            // If we don't have an areaID saved in this plan yet we know we need to set it up still.
            if (aiPlanGetVariableInt(planID, cExplorePlanExploreAreaIDs, 0) == -1)
            {
               helperExploreStartingSurroundings(planID);
            }
         }
      }
   }
}

//////////////////////////////
void pegasusTrained(int pegasusID = -1)
{
   // Instantly scout with our new pegasus.
   xsRuleIgnoreIntervalOnce("pegasusScoutingMonitor");
}
//==============================================================================
// pegasusMaintainMonitor
//==============================================================================
rule pegasusMaintainMonitor
group defaultClassicalRules
inactive
minInterval 60 // We're not doing much here, check very infrequently if we got Winged Messenger or a strat flag changed.
{
   if (cMyCulture != cCultureGreek || cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("pegasusMaintainMonitor");
      return;
   }

   static int maintainPlanID = -1;
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      if (aiPlanGetIsIDValid(maintainPlanID) == true)
      {
         aiPlanDestroy(maintainPlanID);
         maintainPlanID = -1;
      }
      return;
   }
   debugExploration("--- Running Rule pegasusMaintainMonitor. ---");

   if (cMyCiv == cCivZeus || cMyCiv == cCivPoseidon)
   {
      if (kbTechGetStatus(cTechWingedMessenger) == cTechStatusActive)
      {
         debugExploration("We researched Winged Messenger, no longer need to train Pegasi ourself.");
         if (aiPlanGetIsIDValid(maintainPlanID) == true)
         {
            aiPlanDestroy(maintainPlanID);
            maintainPlanID = -1;
         }
         xsDisableRule("pegasusMaintainMonitor");
         return;
      }
   }

   // We maintain 1 Pegasus.
   if (aiPlanGetIsIDValid(maintainPlanID) == false)
   {
      // Pegasi can fly, don't need to limit to area groups.
      maintainPlanID = createSimpleMaintainPlan(cUnitTypePegasus, 1, -1, gExplorationCategoryID);
      aiPlanSetEventHandler(maintainPlanID, cTrainPlanEventUnitTrained, "pegasusTrained"); 
   }
   else
   {
      debugExploration("We already have a functioning Pegasus Spy maintain plan.");
   }
}

//==============================================================================
// ravenManager
//==============================================================================
rule ravenManager
inactive
group defaultArchaicRules
minInterval 30
{
   if (cMyCiv != cCivOdin || cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("ravenManager");
      return;
   }
   static int ravenScoutPlan1 = -1;
   static int ravenScoutPlan2 = -1;
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      if (aiPlanGetIsIDValid(ravenScoutPlan1) == true)
      {
         aiPlanDestroy(ravenScoutPlan1);
         ravenScoutPlan1 = -1;
      }
      if (aiPlanGetIsIDValid(ravenScoutPlan2) == true)
      {
         aiPlanDestroy(ravenScoutPlan2);
         ravenScoutPlan2 = -1;
      }
      return;
   }
   debugExploration("--- Running Rule ravenManager. ---");

   if (aiPlanGetIsIDValid(ravenScoutPlan1) == false)
   {
      ravenScoutPlan1 = aiPlanCreate("Raven Explore 1", cPlanExplore, -1, gExplorationCategoryID);
      aiPlanSetPriority(ravenScoutPlan1, 50);
      aiPlanAddUnitType(ravenScoutPlan1, cUnitTypeRaven, 1, 1, 1);
      // Explore other islands if we need to.
      if (gMapInfo.mIsIslandMap == true)
      {
         helperExploreOtherIslands(ravenScoutPlan1);
      }
   }
   if (aiPlanGetIsIDValid(ravenScoutPlan2) == false)
   {
      ravenScoutPlan2 = aiPlanCreate("Raven Explore 2", cPlanExplore, -1, gExplorationCategoryID);
      aiPlanSetPriority(ravenScoutPlan2, 50);
      aiPlanAddUnitType(ravenScoutPlan2, cUnitTypeRaven, 1, 1, 1);
      // Explore other islands if we need to.
      if (gMapInfo.mIsIslandMap == true)
      {
         helperExploreOtherIslands(ravenScoutPlan2);
      }
   }

   // First Raven plan can explore starting surroundings.
   if (gFullyExploredStartingSurroundings == false)
   {
      // If we don't have an areaID saved in this plan yet we know we need to set it up still.
      if (aiPlanGetVariableInt(ravenScoutPlan1, cExplorePlanExploreAreaIDs, 0) == -1)
      {
         helperExploreStartingSurroundings(ravenScoutPlan1);
      }
   }
}

//==============================================================================
// castSkyLantern
//==============================================================================
rule castSkyLantern
group defaultArchaicRules
inactive
minInterval 35
{
   if (cMyCulture != cCultureChinese || cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("castSkyLantern");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticSkyLantern) == false)
   {
      return;
   }
   int queryID = useSimpleUnitQuery(cUnitTypePioneer);
   int numResults = kbUnitQueryExecute(queryID);
   // Every Pioneer has a random chance to cast its Lantern.
   for (int i = 0; i < numResults; i++)
   {
      if (isBuildOrderDone() == true) // Always extra scout during BO.
      {
         if (xsRandBool() == false)
         {
            continue;
         }
      }
      int pioneerID = kbUnitQueryGetResult(queryID, i);
      vector unitPosition = kbUnitGetPosition(pioneerID);
      vector castPosition = cInvalidVector;
      // Don't send them all the same way.
      int rand = xsRandInt(0, 3);
      if (rand == 0)
      {
         castPosition = unitPosition + vector(1.0, 0.0, 1.0);
      }
      else if (rand == 1)
      {
         castPosition = unitPosition + vector(-1.0, 0.0, 1.0);
      }
      else if (rand == 2)
      {
         castPosition = unitPosition + vector(1.0, 0.0, -1.0);
      }
      else
      {
         castPosition = unitPosition + vector(-1.0, 0.0, -1.0);
      }
      if (kbGetIsLocationOnMap(castPosition) == false)
      {
         continue;
      }
      aiTaskSpecialPowerUnit(pioneerID, cProtoPowerAbilityPioneer, -1, castPosition);
   }
}

//==============================================================================
// quimichinSpyScoutingMonitor
//==============================================================================
rule quimichinSpyScoutingMonitor
inactive
group defaultArchaicRules
minInterval 30
{
   if (cMyCulture != cCultureAztec)
   {
      xsDisableRule("quimichinSpyScoutingMonitor");
      return;
   }
   debugExploration("--- Running Rule quimichinSpyScoutingMonitor. ---");

   static int quimichinSpyScoutPlanID = -1;
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      if (aiPlanGetIsIDValid(quimichinSpyScoutPlanID) == true)
      {
         aiPlanDestroy(quimichinSpyScoutPlanID);
         quimichinSpyScoutPlanID = -1;
      }
      return;
   }

   int quimichinSpyID = getUnit(cUnitTypeQuimichinSpy);
   if (quimichinSpyID == -1)
   {
      if (xsGetTime() < 5)
      {
         // Wait for it to spawn.
         debugExploration("We can't find a Quimichin Spy yet but it's also early in the game, wait for it to spawn.");
         xsSetRuleMinInterval("quimichinSpyScoutingMonitor", 5); // Run again once it's spawned presumably.
         return;
      }
      debugExploration("We lost our Quimichin Spy, waiting for a new one to be trained now.");
      return;
   }
   xsSetRuleMinInterval("quimichinSpyScoutingMonitor", 30); // Run again once it's spawned presumably.

   if (aiPlanGetIsIDValid(quimichinSpyScoutPlanID) == false)
   {
      quimichinSpyScoutPlanID = aiPlanCreate("Quimichin Spy Explore", cPlanExplore, -1, gExplorationCategoryID);
      aiPlanSetPriority(quimichinSpyScoutPlanID, 50);
      aiPlanAddUnitType(quimichinSpyScoutPlanID, cUnitTypeQuimichinSpy, 1, 1, 1);
      aiPlanSetFlag(quimichinSpyScoutPlanID, cPlanFlagCantBeStolenFrom, true);
      // Explore other islands if we need to.
      if (gMapInfo.mIsIslandMap == true)
      {
         helperExploreOtherIslands(quimichinSpyScoutPlanID);
      }
      if (gFullyExploredStartingSurroundings == false)
      {
         // If we don't have an areaID saved in this plan yet we know we need to set it up still.
         if (aiPlanGetVariableInt(quimichinSpyScoutPlanID, cExplorePlanExploreAreaIDs, 0) == -1)
         {
            helperExploreStartingSurroundings(quimichinSpyScoutPlanID);
         }
      }
      debugExploration("Created plan: " + aiPlanGetName(quimichinSpyScoutPlanID) + ".");
   }

   if (isUnitAlreadyInPlanOrChildOf(quimichinSpyID, quimichinSpyScoutPlanID) == false)
   {
      // Forcibly add the unit since we always want it to be in this plan.
      aiPlanAddUnit(quimichinSpyScoutPlanID, quimichinSpyID);
      debugExploration("Added Quimichin Spy: " + quimichinSpyID + " to " + aiPlanGetName(quimichinSpyScoutPlanID) + ".");
   }
}

//////////////////////////////
void quimichinSpyTrained(int pegasusID = -1)
{
   // Instantly scout with our new Quimichin Spy.
   xsRuleIgnoreIntervalOnce("quimichinSpyScoutingMonitor");
}
//==============================================================================
// quimichinSpyMaintainMonitor
//==============================================================================
rule quimichinSpyMaintainMonitor
group defaultClassicalRules
inactive
minInterval 60 // We're not doing much here.
{
   if (cMyCulture != cCultureAztec)
   {
      xsDisableRule("quimichinSpyMaintainMonitor");
      return;
   }

   static int maintainPlanID = -1;
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      if (aiPlanGetIsIDValid(maintainPlanID) == true)
      {
         aiPlanDestroy(maintainPlanID);
         maintainPlanID = -1;
      }
      return;
   }
   debugExploration("--- Running Rule quimichinSpyMaintainMonitor. ---");

   // We maintain 1 Quimichin Spy.
   if (aiPlanGetIsIDValid(maintainPlanID) == false)
   {
      maintainPlanID = createSimpleMaintainPlan(cUnitTypeQuimichinSpy, 1, gLandAreaGroupID, gExplorationCategoryID);
      aiPlanSetEventHandler(maintainPlanID, cTrainPlanEventUnitTrained, "quimichinSpyTrained"); 
   }
   else
   {
      debugExploration("We already have a functioning Quimichin Spy maintain plan.");
   }
}

//////////////////////////////////////////////////////
/////////////////////// Oracles //////////////////////
//////////////////////////////////////////////////////

//==============================================================================
// startupOracleScoutingMonitor
//==============================================================================
rule startupOracleScoutingMonitor
group defaultArchaicRules
inactive
minInterval 1
{
   if (cMyCulture != cCultureAtlantean)
   {
      xsDisableRule("startupOracleScoutingMonitor");
      return;
   }
   static int[] planIDs = default;
   if (getHighestPlayerAge() > cAge1)
   {
      debugExploration("Disabling startupOracleScoutingMonitor because somebody reached Classical or higher, we need to be safe now.");
      for (int i = 0; i < planIDs.size(); i++)
      {
         if (aiPlanGetIsIDValid(planIDs[i]) == true)
         {
            aiPlanDestroy(planIDs[i]);
         }
      }
      planIDs.clear();
      xsDisableRule("startupOracleScoutingMonitor");
      xsEnableRule("oracleMonitor");
      xsEnableRule("oracleMaintainMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticScouting) == false)
   {
      return;
   }
   debugExploration("--- Running Rule startupOracleScoutingMonitor. ---");

   int oracleQuery = useSimpleUnitQuery(cUnitTypeOracle, cMyID, cUnitStateAlive);
   int numberFound = kbUnitQueryExecute(oracleQuery);
   int masterPlanID = aiPlanGetIDByTypeAndVariableIntValue(cPlanExplore, cExplorePlanMasterExploreAreasPlan, -1);
   for (int i = 0; i < numberFound; i++)
   {
      int unitID = kbUnitQueryGetResult(oracleQuery, i);
      int planID = kbUnitGetPlanID(unitID);
      if (planID >= 0 && (aiPlanGetType(planID) == cPlanExplore || aiPlanGetPriority(planID) > 50))
      {
         continue;
      }
      planID = aiPlanCreate("Oracle Explore, unitID:" + unitID, cPlanExplore, -1, gExplorationCategoryID);
      aiPlanSetPriority(planID, 50);
      aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true);
      aiPlanAddUnitType(planID, cUnitTypeOracle, 1, 1, 1);
      aiPlanAddUnit(planID, unitID);
      // Stand still if less or equal than 20% of our surrounding tiles are explored.
      aiPlanSetVariableFloat(planID, cExplorePlanStopLOSPercentage, 0, 0.2);
      if (aiPlanGetIsIDValid(masterPlanID) == true)
      {
         aiPlanSetVariableInt(planID, cExplorePlanMasterExploreAreasPlan, 0, masterPlanID);
      }
      planIDs.add(planID);
   }

   static bool increasedInterval = false;
   if (numberFound == 3 && increasedInterval == false)
   {
      debugExploration("We've handled all our starting Oracles, checking for if we need to pull our Oracles back from now on.");
      xsSetRuleMinInterval("startupOracleScoutingMonitor", 5);
      increasedInterval = true;
   }
}

const int cCalculatingAreas = 0;
const int cFinishedCalculatingAreas = 1;
const int cFailedCalculatingAreas = 2;
const int cNoNeedToCalculateAreas = 3;

//==============================================================================
// Class townCenterEntry
//==============================================================================
class townCenterEntry
{
   int mID = -1; // TC ID.
   int[] mAreaIDs = default; // What valid areas we have for Oracles for this TC.
   bool mCalculatedAreas = false;
};

//==============================================================================
// Class OracleInformation
//==============================================================================
class OracleInformation
{
   int[] oracleIDs = default;
   int[] takenAreaIDs = default; // Indexes in sync with oracleIDs.
   int[] validAreaIDs = default;
   int numValidAreas = 0;
   int reservePlanID = -1;
   townCenterEntry[] mTownCenters = default;
   int areaCalculationState = cNoNeedToCalculateAreas;
   int currentRecalculatingTCIndex = 0;
   int[] mValidTypes = default;
   int maxAmountOfOracles = 0;
   int oracleMaintainPlanID = -1;

   vector findClosestTCPosition(vector searchPosition = cInvalidVector)
   {
      vector closestPosition = cInvalidVector;
      float closestDistance = cMaxFloat;
      for (int i = 0; i < mTownCenters.size(); i++)
      {
         townCenterEntry entry = mTownCenters[i];
         if (kbUnitGetIsIDValid(entry.mID) == false || kbUnitGetPlayerID(entry.mID) != cMyID)
         {
            // Let manageTownCenter handle this properly.
            continue;
         }
         vector tcPosition = kbUnitGetPosition(entry.mID);
         float distance = xsVectorLength(searchPosition - tcPosition);
         if (distance < closestDistance)
         {
            closestPosition = tcPosition;
            closestDistance = distance;
         }
      }
      return closestPosition;
   }

   void createReservePlan()
   {
      if (aiPlanGetIsIDValid(reservePlanID) == false)
      {
         reservePlanID = aiPlanCreate("Oracle reserve plan", cPlanReserve, -1, gExplorationCategoryID);
         aiPlanSetPriority(reservePlanID, 100);
         aiPlanAddUnitType(reservePlanID, cUnitTypeOracle, 100, 100, 100);
         aiPlanAddUnitType(reservePlanID, cUnitTypeOracleHero, 100, 100, 100);
         aiPlanSetFlag(reservePlanID, cPlanFlagNoMoreUnits, true); // Prevent auto assignment.
      }
   }

   bool manageTownCenter()
   {
      int[] alreadyTrackedTCIDs = new int(0, 0);
      for (int i = mTownCenters.size() - 1; i >= 0; i--)
      {
         townCenterEntry entry = mTownCenters[i];
         debugExploration("ID present in mTownCenters: " + entry.mID + ".");
         if (kbUnitGetIsIDValid(entry.mID) == false || kbUnitGetPlayerID(entry.mID) != cMyID)
         {
            debugExploration("Town Center with ID " + entry.mID + " is no longer valid, we need to recalculate areas.");
            mTownCenters.removeIndex(i);
            areaCalculationState = cCalculatingAreas;
         }
         else if (gLandAreaGroupID != -1 &&
            kbPathAreAreaGroupsConnected(gLandAreaGroupID, kbUnitGetAreaGroupID(entry.mID), cPassabilityLand) == false)
         {
            debugExploration("Town Center with ID " + entry.mID + " is not connected to gLandAreaGroupID, we need to recalculate areas.");
            mTownCenters.removeIndex(i);
            areaCalculationState = cCalculatingAreas;
         }
         else
         {
            alreadyTrackedTCIDs.add(entry.mID);
         }
      }
      int queryID = useSimpleUnitQuery(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateAlive);
      if (gLandAreaGroupID != -1)
      {
         kbUnitQuerySetConnectedAreaGroupID(queryID, gLandAreaGroupID, cPassabilityLand);
      }
      int numResults = kbUnitQueryExecute(queryID);
      int[] results = kbUnitQueryGetResults(queryID);
      for (int i = 0; i < numResults; i++)
      {
         if (alreadyTrackedTCIDs.find(results[i]) == -1)
         {
            debugExploration("Town Center with ID " + results[i] + " is new to our array, we need to recalculate areas.");
            townCenterEntry newEntry;
            newEntry.mID = results[i];
            mTownCenters.add(newEntry);
            areaCalculationState = cCalculatingAreas;
         }
      }

      if (mTownCenters.size() == 0)
      {
         areaCalculationState = cNoNeedToCalculateAreas;
         debugExploration("We have no Town Centers alive, just leaving the Oracles now.");
         return false;
      }

      if (areaCalculationState == cCalculatingAreas)
      {
         currentRecalculatingTCIndex = 0;
         validAreaIDs.clear();
         takenAreaIDs.clear();
         oracleIDs.clear();
      }
      return true;
   }

   void addValidAreas(ref int[] array)
   {
      for (int i = 0; i < array.size(); i++)
      {
         int areaID = array[i];
         if (validAreaIDs.find(areaID) == -1)
         {
            validAreaIDs.add(areaID);
            debugExploration("   Adding " + areaID + " to valid list of areas.");
         }
      }
   }

   int recalculateAreas()
   {
      const int minAreaSize = 190;
      int numTCs = mTownCenters.size();

      townCenterEntry entry = mTownCenters[currentRecalculatingTCIndex];

      // Use the array over multiple frames, if a TC went invalid we restart everything.
      if (kbUnitGetIsIDValid(entry.mID) == false)
      {
         debugExploration("One of our TCs went invalid during the calculation of areas, restart process next frame.");
         return cFailedCalculatingAreas;
      }
      if (entry.mCalculatedAreas == true)
      {
         debugExploration("We have already analyzed the areas for TCID: " + entry.mID + ".");
         addValidAreas(entry.mAreaIDs);
      }
      else
      {
         // TC is the first valid AreaID and also the start point of our search.
         vector tcPosition = kbUnitGetPosition(entry.mID);
         debugExploration("We need to analyze the areas for TCID: " + entry.mID + " at " + tcPosition + ".");
         int[] areasWithinRange = kbAreaGetIDsByPositionAndRange(tcPosition, 75.0, mValidTypes, true, cPassabilityLand);
         int numAreasWithinRange = areasWithinRange.size();
         for (int iArea = 0; iArea < numAreasWithinRange; iArea++)
         {
            if (kbAreaGetNumberTiles(areasWithinRange[iArea]) <= minAreaSize)
            {
               debugExploration("Skipping areaID " + areasWithinRange[iArea] + " because it's too small.");
               continue;
            }
            entry.mAreaIDs.add(areasWithinRange[iArea]);
            debugExploration("Adding " + areasWithinRange[iArea] + " to TCID " + entry.mID + " valid list of areas.");
         }

         entry.mCalculatedAreas = true;
         mTownCenters[currentRecalculatingTCIndex] = entry;
         addValidAreas(entry.mAreaIDs);
      }

      currentRecalculatingTCIndex++;
      if (currentRecalculatingTCIndex == numTCs)
      {
         debugExploration("We ended up with " + validAreaIDs.size() + " valid areas, this is also our Oracle cap.");
         maxAmountOfOracles = validAreaIDs.size();
         numValidAreas = maxAmountOfOracles;
         return cFinishedCalculatingAreas;
      }

      return cCalculatingAreas;
   }
};
extern OracleInformation oracleInformation;

//==============================================================================
// oracleMonitor
// Manage all our alive Oracles during the game.
// Send them to safe areas to gather favor, retreat them from dangerous areas.
//==============================================================================
rule oracleMonitor
group defaultClassicalRules
inactive
minInterval 10 // Reduced based on current difficulty during first run.
{
   if (cMyCulture != cCultureAtlantean)
   {
      xsDisableRule("oracleMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagManageOracles) == false)
   {
      return;
   }
   debugExploration("--- Running Rule oracleMonitor. ---");

   static bool firstRun = true;
   if (firstRun == true)
   {
      // Reduce more the higher the difficulty.
      xsSetRuleMinInterval("oracleMonitor", 10 - cDifficultyCurrent);
      // Set up the mValidTypes array.
      oracleInformation.mValidTypes.add(cAreaTypePassableLand);
      oracleInformation.mValidTypes.add(cAreaTypeGold);
      oracleInformation.mValidTypes.add(cAreaTypeSettlement);
      firstRun = false;
   }

   oracleInformation.createReservePlan();
   if (oracleInformation.areaCalculationState == cNoNeedToCalculateAreas)
   {
      if (oracleInformation.manageTownCenter() == false)
      {
         // We can't do anything without a valid TC to orient ourself around.
         return;
      }
      // Now we need to calculate, start that next run.
      if (oracleInformation.areaCalculationState != cNoNeedToCalculateAreas)
      {
         debugExploration("We still start the area calculation next frame.");
         xsRuleIgnoreIntervalOnce("oracleMonitor");
         return;
      }
   }

   if (oracleInformation.areaCalculationState == cCalculatingAreas)
   {
      oracleInformation.areaCalculationState = oracleInformation.recalculateAreas();
      // If we've analyzed all TCs we can progress.
      if (oracleInformation.areaCalculationState == cFinishedCalculatingAreas)
      {
         debugExploration("We will analyze the position of our Oracles next run, if no changes in our TC array take place.");
         oracleInformation.areaCalculationState = cNoNeedToCalculateAreas; // Reset.
      }
      xsRuleIgnoreIntervalOnce("oracleMonitor");
      return;
   }

   for (int i = oracleInformation.oracleIDs.size() - 1; i >= 0; i--)
   {
      int oracleID = oracleInformation.oracleIDs[i];
      if (kbUnitGetIsIDValid(oracleID) == false || kbUnitGetPlayerID(oracleID) != cMyID)
      {
         debugExploration("One of our Oracles died/got transformed, removing him from the oracleIDs array.");
         oracleInformation.oracleIDs.removeIndex(i);
         if (oracleInformation.takenAreaIDs[i] != -1)
         {
            oracleInformation.validAreaIDs.add(oracleInformation.takenAreaIDs[i]);
         }
         oracleInformation.takenAreaIDs.removeIndex(i);
      }
   }

   int queryID = useSimpleUnitQuery(cUnitTypeAbstractOracle);
   if (gLandAreaGroupID != -1)
   {
      // We just leave the Oracles that are on the old island idle.
      kbUnitQuerySetConnectedAreaGroupID(queryID, gLandAreaGroupID, cPassabilityLand);
   }
   int numAliveOracles = kbUnitQueryExecute(queryID);
   if (numAliveOracles == 0)
   {
      debugExploration("We have 0 Oracles alive to manage.");
      return;
   }
   int[] aliveOracleIDs = kbUnitQueryGetResults(queryID);
   
   int numOraclesMoved = 0;
   bool shouldTryFindNewArea = true;
   for (int i = 0; i < numAliveOracles; i++)
   {
      if (numOraclesMoved == 3)
      {
         xsRuleIgnoreIntervalOnce("oracleMonitor");
         break;
      }

      int oracleID = aliveOracleIDs[i];
      // TODO some exclusion mechanics via oracleInformation that we can claim Oracles from this system and manually control them
      // and then also give them back to the system. Right now any alive Oracles will always be reclaimed here and if you assign
      // them away all arrays go fubar.
      int index = oracleInformation.oracleIDs.find(oracleID);
      if (index == -1)
      {
         index = oracleInformation.oracleIDs.add(oracleID);
         oracleInformation.takenAreaIDs.add(-1);
         if (kbUnitGetPlanID(oracleID) != oracleInformation.reservePlanID)
         {
            aiPlanAddUnit(oracleInformation.reservePlanID, oracleID);
         }
         debugExploration("Adding Oracle " + oracleID + " to the oracleIDs array at index " + index +  ".");
      }

      int currentAreaID = oracleInformation.takenAreaIDs[index];
      if (currentAreaID != -1 && kbAreaGetDangerLevel(currentAreaID, false) > 100.0)
      {
         debugExploration("Oracle " + oracleID + " is standing in a dangerous areaID " + currentAreaID +
            ", retreating him to TC now.");
         vector position = oracleInformation.findClosestTCPosition(kbUnitGetPosition(oracleID));
         vector passablePosition = kbFindClosestPassablePoint(position, cPassabilityLand, 10.0);
         if (passablePosition != cInvalidVector)
         {
            position = passablePosition;
         }
         aiTaskMoveUnit(oracleID, position);
         oracleInformation.validAreaIDs.add(currentAreaID);
         oracleInformation.takenAreaIDs[index] = -1;
         numOraclesMoved++;
         continue;
      }

      // If this Oracle doesn't have an area assigned, we try to find one for it.
      // This happens when the Oracle is first added to the array and when we had to retreat the Oracle from a danger area.
      if (shouldTryFindNewArea == true && oracleInformation.takenAreaIDs[index] == -1)
      {
         debugExploration("Oracle " + oracleID + " doesn't have an area assigned yet, try to find one.");
         for (int j = 0; j < oracleInformation.validAreaIDs.size(); j++)
         {
            int validAreaID = oracleInformation.validAreaIDs[j];
            if (kbAreaGetDangerLevel(validAreaID, false) > 100.0)
            {
               debugExploration("Skipping " + validAreaID + " because it's too dangerous.");
               continue;
            }
            debugExploration("Found valid areaID " + validAreaID + " for this Oracle, adding it to the takenAreaIDs array at index "
               + index + ".");
            oracleInformation.takenAreaIDs[index] = validAreaID;
            oracleInformation.validAreaIDs.removeIndex(j);
            vector position = kbAreaGetCenter(validAreaID);
            vector passablePosition = kbFindClosestPassablePoint(position, cPassabilityLand, 10.0);
            if (passablePosition != cInvalidVector)
            {
               position = passablePosition;
            }
            aiTaskMoveUnit(oracleID, position);
            break;
         }
         if (oracleInformation.takenAreaIDs[index] == -1)
         {
            debugExploration("Didn't find a valid areaID for Oracle " + oracleID + ", we will try again next time. " +
               "Skipping this check for any other potential Oracles since they won't find any areas either.");
            shouldTryFindNewArea = false;
         }
         numOraclesMoved++;
      }
   }

   // Every 30 seconds we move our Oracles back to their intended position, various things could've forced them to move away.
   static int lastRepositionUpdate = 0;
   if (lastRepositionUpdate + 30 < xsGetTime())
   {
      for (int i = 0; i < oracleInformation.oracleIDs.size(); i++)
      {
         int oracleID = oracleInformation.oracleIDs[i];
         vector position = cInvalidVector;
         vector oraclePosition = kbUnitGetPosition(oracleID);
         if (kbAreaGetIsIDValid(oracleInformation.takenAreaIDs[i]) == false)
         {
            position = oracleInformation.findClosestTCPosition(oraclePosition);
         }
         else
         {
            position = kbAreaGetCenter(oracleInformation.takenAreaIDs[i]);
         }
         vector passablePosition = kbFindClosestPassablePoint(position, cPassabilityLand, 10.0);
         if (passablePosition != cInvalidVector)
         {
            position = passablePosition;
         }
         if (xsVectorLength(oraclePosition - position) > 5.0)
         {
            aiTaskMoveUnit(oracleID, position);
         }
      }
      lastRepositionUpdate = xsGetTime();
   }
}

//==============================================================================
// oracleMaintainMonitor
//==============================================================================
rule oracleMaintainMonitor
group defaultClassicalRules
inactive
minInterval 30
{
   if (cMyCulture != cCultureAtlantean)
   {
      xsDisableRule("oracleMaintainMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagTrainOracles) == false)
   {
      if (aiPlanGetIsIDValid(oracleInformation.oracleMaintainPlanID) == true)
      {
         aiPlanDestroy(oracleInformation.oracleMaintainPlanID);
      }
      oracleInformation.oracleMaintainPlanID = -1;
      return;
   }
   debugExploration("--- Running Rule oracleMaintainMonitor. ---");

   if (aiPlanGetIsIDValid(oracleInformation.oracleMaintainPlanID) == false)
   {
      oracleInformation.oracleMaintainPlanID = createSimpleMaintainPlan(cUnitTypeOracle, 0, gLandAreaGroupID,
         gMilitaryTrainingCategoryID, 50, -1, -1, true);
      aiPlanSetVariableBool(oracleInformation.oracleMaintainPlanID, cTrainPlanUseMultipleBuildings, 0, false);
      // Don't train Oracles too fast after one another. They cost a lot of gold and other army should take prio.
      aiPlanSetVariableInt(oracleInformation.oracleMaintainPlanID, cTrainPlanFrequency, 0, selectByDifficulty(60, 50, 40, 30, 30, 20));
   }

   if (oracleInformation.mTownCenters.size() == 0)
   {
      // We shouldn't train Oracles now.
      aiPlanSetVariableInt(oracleInformation.oracleMaintainPlanID, cTrainPlanNumberToMaintain, 0, 0);
      aiPlanSetName(oracleInformation.oracleMaintainPlanID, oracleInformation.oracleMaintainPlanID + ": Maintain 0 Oracle");
      return;
   }

   // Never train Oracles in Archaic
   int numOraclesWanted = 0;
   int currentAge = kbPlayerGetAge(cMyID);
   if (currentAge >= cAge2)
   {
      switch (cDifficultyCurrent)
      {
         case cDifficultyEasy:
         {
            numOraclesWanted = 1;
            break;
         }
         case cDifficultyModerate:
         {
            numOraclesWanted = 2;
            break;
         }
         case cDifficultyHard:
         {
            numOraclesWanted = 3;
            break;
         }
         case cDifficultyTitan:
         case cDifficultyExtreme:
         case cDifficultyLegendary:
         {
            numOraclesWanted = 4;
            break;
         }
      }
   }
   if (currentAge == cAge3 && cDifficultyCurrent >= cDifficultyTitan)
   {
      numOraclesWanted += 1;
   }
   if (currentAge >= cAge4 && cDifficultyCurrent >= cDifficultyExtreme)
   {
      numOraclesWanted += 2;
   }
   // If we have more TCs available to us we can place more Oracles.
   if (cDifficultyCurrent >= cDifficultyTitan)
   {
      if (oracleInformation.mTownCenters.size() > 1)
      {
         numOraclesWanted += 2;
      }
   }
   // We need the minimum amount of favor income for Humanoid so that we can build Palaces.
   if (cPersonalityCurrent == cPersonalityHumanoid)
   {
      numOraclesWanted = 1;
   }
   if (numOraclesWanted > oracleInformation.maxAmountOfOracles)
   {
      debugExploration("Clamping our wanted number of Oracles from " + numOraclesWanted + " to " +
         oracleInformation.maxAmountOfOracles + " since we don't have enough valid areas.");
      numOraclesWanted = oracleInformation.maxAmountOfOracles;
   }
   // Hero and regular Oracles share a build limit.
   int buildLimit = kbPlayerGetProtoStatInt(cMyID, cUnitTypeOracle, cProtoStatBuildLimit);
   if (numOraclesWanted > buildLimit)
   {
      debugExploration("Clamping our wanted number of Oracles from " + numOraclesWanted + " to " +
         buildLimit + " since that's our build limit.");
      numOraclesWanted = buildLimit;
   }
   debugExploration("We want to maintain " + numOraclesWanted + " Oracles.");
   int aliveHeroOracles = kbUnitCount(cUnitTypeOracleHero, cMyID, cUnitStateAlive);
   if (aliveHeroOracles > 0)
   {
      numOraclesWanted -= aliveHeroOracles;
      debugExploration("We already have " + aliveHeroOracles + " Hero Oracles, reducing our wanted Oracle number by that amount. " +
         "New amount: " + numOraclesWanted + ".");
   }
   aiPlanSetVariableInt(oracleInformation.oracleMaintainPlanID, cTrainPlanNumberToMaintain, 0, numOraclesWanted);
   aiPlanSetName(oracleInformation.oracleMaintainPlanID, oracleInformation.oracleMaintainPlanID + ": Maintain " +
      numOraclesWanted + " Oracle");
}

//////////////////////////////////////////////////////
/////////////////////// Relics ///////////////////////
//////////////////////////////////////////////////////

//==============================================================================
// Relic management
//==============================================================================
int gRelicReservePlanID = -1;
bool gWalkingBackToTemple = false;
int gRelicTempleID = -1;
int gRelicID = -1;
int gRelicHeroID = -1;
int gRelicHeroProtoUnitID = -1;
vector gRelicPosition = cInvalidVector;

void resetRelicData(bool sendOutput = false)
{
   if (sendOutput == true)
   {
      debugExploration("Resetting all Relic collection process.");
   }
   if (aiPlanGetIsIDValid(gRelicReservePlanID) == true)
   {
      aiPlanDestroy(gRelicReservePlanID);
   }
   gRelicReservePlanID = -1;
   gWalkingBackToTemple = false;
   gRelicTempleID = -1;
   gRelicID = -1;
   gRelicHeroID = -1;
   gRelicHeroProtoUnitID = -1;
   gRelicPosition = cInvalidVector;
}

//==============================================================================
// relicTargetNeedsTransport
//==============================================================================
bool relicTargetNeedsTransport(vector startPosition = cInvalidVector, vector targetPosition = cInvalidVector)
{
   if (gMapInfo.mIsIslandMap == false)
   {
      return false;
   }
   if (kbGetIsLocationOnMap(startPosition) == false || kbGetIsLocationOnMap(targetPosition) == false)
   {
      return false;
   }

   int startAreaGroupID = kbAreaGroupGetIDByPosition(startPosition);
   int targetAreaGroupID = kbAreaGroupGetIDByPosition(targetPosition);
   if (startAreaGroupID == -1 || targetAreaGroupID == -1 || startAreaGroupID == targetAreaGroupID)
   {
      return false;
   }
   if (kbPathAreAreaGroupsConnected(startAreaGroupID, targetAreaGroupID, cPassabilityLand) == true)
   {
      return false;
   }
   return kbPathAreAreaGroupsConnected(startAreaGroupID, targetAreaGroupID, cPassabilityAmphibious);
}

//==============================================================================
// relicHeroHasDirectPath
//==============================================================================
bool relicHeroHasDirectPath(int heroID = -1, vector targetPosition = cInvalidVector)
{
   if (kbUnitGetIsIDValid(heroID) == false || kbGetIsLocationOnMap(targetPosition) == false)
   {
      return false;
   }
   return kbCanPath(kbUnitGetPosition(heroID), targetPosition, kbUnitGetProtoUnitID(heroID), 1.0);
}

//==============================================================================
// getRelicHomePosition
//==============================================================================
vector getRelicHomePosition()
{
   int mainBaseID = kbBaseGetMainID(cMyID);
   if (mainBaseID != -1)
   {
      vector mainBasePosition = kbBaseGetLocation(cMyID, mainBaseID);
      if (kbGetIsLocationOnMap(mainBasePosition) == true)
      {
         return mainBasePosition;
      }
   }
   return aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
}

//==============================================================================
// getRelicHomeTempleID
//==============================================================================
int getRelicHomeTempleID()
{
   vector homePosition = getRelicHomePosition();
   int templeID = getSuitableTempleIDForRelic(homePosition);
   if (templeID != -1)
   {
      return templeID;
   }

   static int queryID = -1;
   if (queryID < 0)
   {
      queryID = kbUnitQueryCreate("getRelicHomeTempleID");
   }
   kbUnitQuerySetPlayerID(queryID, cMyID);
   kbUnitQuerySetUnitType(queryID, cUnitTypeTemple);
   kbUnitQuerySetState(queryID, cUnitStateAlive);
   kbUnitQuerySetPosition(queryID, homePosition);
   kbUnitQuerySetMaximumDistance(queryID, cMaxFloat);
   kbUnitQuerySetAscendingSort(queryID, true);
   kbUnitQueryResetResults(queryID);

   int numberFound = kbUnitQueryExecute(queryID);
   int maxContained = kbPlayerGetProtoStatInt(cMyID, cUnitTypeTemple, cProtoStatMaxContained);
   for (int i = 0; i < numberFound; i++)
   {
      templeID = kbUnitQueryGetResult(queryID, i);
      if (kbUnitGetNumberContained(templeID) < maxContained)
      {
         debugExploration("Using fallback home Temple " + templeID + " for Relic deposit.");
         return templeID;
      }
   }
   return -1;
}

//==============================================================================
// createIslandRelicPlan
// On island maps we use an attack point plan only as a transport-capable mover.
// Once the hero has a land path to the target, the monitor switches this to a reserve plan and issues WorkUnit.
//==============================================================================
void createIslandRelicPlan(int heroID = -1, int heroProtoUnitID = -1, vector targetPosition = cInvalidVector,
   int targetUnitID = -1, string planName = "Relic reserve plan")
{
   if (kbUnitGetIsIDValid(heroID) == false || heroProtoUnitID == -1 || kbGetIsLocationOnMap(targetPosition) == false)
   {
      aiEchoWarning("createIslandRelicPlan called with invalid data.");
      return;
   }
   if (aiPlanGetIsIDValid(gRelicReservePlanID) == true)
   {
      aiPlanDestroy(gRelicReservePlanID);
   }

   bool needsTransport = relicTargetNeedsTransport(kbUnitGetPosition(heroID), targetPosition);
   if (needsTransport == true)
   {
      vector gatherPoint = kbUnitGetPosition(heroID);
      gRelicReservePlanID = aiPlanCreate(planName, cPlanAttack, -1, gExplorationCategoryID);
      aiPlanSetVariableInt(gRelicReservePlanID, cAttackPlanTargetMode, 0, cAttackPlanTargetModePoint);
      aiPlanSetVariableVector(gRelicReservePlanID, cAttackPlanTargetPoint, 0, targetPosition);
      aiPlanSetVariableVector(gRelicReservePlanID, cAttackPlanGatherPoint, 0, gatherPoint);
      aiPlanSetVariableFloat(gRelicReservePlanID, cAttackPlanGatherDistance, 0, 1.0);
      aiPlanSetVariableInt(gRelicReservePlanID, cAttackPlanGatherWaitTime, 0, 1 * 1000);
      aiPlanSetVariableFloat(gRelicReservePlanID, cAttackPlanAttackModeEngageRange, 0, 0.0);
      aiPlanSetVariableInt(gRelicReservePlanID, cAttackPlanMovementIntervalTime, 0, 1000);
      aiPlanSetVariableInt(gRelicReservePlanID, cAttackPlanDoneMode, 0, cAttackPlanDoneModeNoUnits);
      setDefaultAttackPlanTargetUnitTypes(gRelicReservePlanID);
      debugExploration("Created attack/transport Relic plan toward area group " +
         kbAreaGroupGetIDByPosition(targetPosition) + ".");
   }
   else
   {
      gRelicReservePlanID = aiPlanCreate(planName, cPlanReserve, -1, gExplorationCategoryID);
   }

   aiPlanSetPriority(gRelicReservePlanID, 100);
   aiPlanAddUnitType(gRelicReservePlanID, heroProtoUnitID, 1, 1, 1);
   aiPlanAddUnit(gRelicReservePlanID, heroID);
   aiPlanSetFlag(gRelicReservePlanID, cPlanFlagNoMoreUnits, true);
   aiPlanSetFlag(gRelicReservePlanID, cPlanFlagCantBeStolenFrom, true);

   gRelicHeroID = heroID;
   gRelicHeroProtoUnitID = heroProtoUnitID;

   if (needsTransport == false)
   {
      if (targetUnitID == -1)
      {
         aiTaskMoveUnit(heroID, targetPosition);
      }
      else
      {
         aiTaskWorkUnit(heroID, targetUnitID);
      }
   }
}

//==============================================================================
// relicIslandCollectionMonitor
//==============================================================================
void relicIslandCollectionMonitor()
{
   if (gRelicHeroID != -1 && aiPlanGetIsIDValid(gRelicReservePlanID) == false)
   {
      if (kbUnitGetIsIDValid(gRelicHeroID) == false || kbUnitGetPlayerID(gRelicHeroID) != cMyID)
      {
         debugExploration("Our Relic hero disappeared and the Relic plan is gone, restarting process.");
         resetRelicData();
         xsSetRuleMinInterval("relicCollectionMonitor", 60);
         return;
      }

      if (gWalkingBackToTemple == true)
      {
         if (kbUnitGetIsIDValid(gRelicTempleID) == false || kbUnitGetPlayerID(gRelicTempleID) != cMyID)
         {
            resetRelicData();
            return;
         }
         debugExploration("Our island Relic return plan disappeared, recreating it.");
         createIslandRelicPlan(gRelicHeroID, kbUnitGetProtoUnitID(gRelicHeroID), kbUnitGetPosition(gRelicTempleID),
            gRelicTempleID, "Relic return plan");
      }
      else
      {
         debugExploration("Our island Relic retrieval plan disappeared, recreating it.");
         createIslandRelicPlan(gRelicHeroID, kbUnitGetProtoUnitID(gRelicHeroID), gRelicPosition, -1,
            "Relic reserve plan");
      }
      xsSetRuleMinInterval("relicCollectionMonitor", 5);
      return;
   }

   if (aiPlanGetIsIDValid(gRelicReservePlanID) == true)
   {
      debugExploration("We already have an island Relic retrieval order going on currently, check in on it.");
      int heroID = gRelicHeroID;
      if (kbUnitGetIsIDValid(heroID) == false || kbUnitGetPlayerID(heroID) != cMyID)
      {
         debugExploration("We've lost the hero that we assigned to our island Relic plan, restarting process.");
         resetRelicData();
         xsSetRuleMinInterval("relicCollectionMonitor", 60);
         return;
      }

      if (gWalkingBackToTemple == false)
      {
         if (xsVectorDistanceXZSqr(kbUnitGetPosition(heroID), gRelicPosition) < 5.0 * 5.0)
         {
            debugExploration("We're within 5 meters of our island Relic, try to pick it up.");
            if (xsVectorDistanceXZSqr(kbUnitGetPosition(gRelicID), gRelicPosition) > 5.0 * 5.0)
            {
               debugExploration("The Relic is no longer at the spot we thought it was.");
               resetRelicData();
            }
            else if (aiPlanGetType(gRelicReservePlanID) != cPlanReserve)
            {
               debugExploration("Hero reached the Relic island, switching from transport to direct pickup.");
               createIslandRelicPlan(heroID, kbUnitGetProtoUnitID(heroID), gRelicPosition, gRelicID,
                  "Relic reserve plan");
            }
            else
            {
               aiTaskWorkUnit(heroID, gRelicID);
            }
         }
         else if (relicHeroHasDirectPath(heroID, gRelicPosition) == true)
         {
            debugExploration("Hero has a direct land path to the Relic, using direct pickup.");
            if (aiPlanGetType(gRelicReservePlanID) != cPlanReserve)
            {
               createIslandRelicPlan(heroID, kbUnitGetProtoUnitID(heroID), gRelicPosition, gRelicID,
                  "Relic reserve plan");
            }
            else
            {
               aiTaskWorkUnit(heroID, gRelicID);
            }
         }
         else
         {
            debugExploration("Relic is not on a direct land path from our hero, waiting for the transport-capable plan.");
         }
      }
      else
      {
         if (kbUnitGetIsIDValid(gRelicTempleID) == false || kbUnitGetPlayerID(gRelicTempleID) != cMyID)
         {
            debugExploration("We've lost the Temple we wanted to deposit at, trying to find a new one now.");
            int templeID = getRelicHomeTempleID();
            if (templeID == -1)
            {
               aiTaskUngarrisonUnit(heroID, kbUnitGetPosition(heroID));
               debugExploration("Couldn't find a new home Temple to deposit the Relic at.");
               resetRelicData();
            }
            else
            {
               debugExploration("Found a new home Temple " + templeID + " to deposit the Relic at.");
               gRelicTempleID = templeID;
               createIslandRelicPlan(heroID, kbUnitGetProtoUnitID(heroID), kbUnitGetPosition(gRelicTempleID),
                  gRelicTempleID, "Relic return plan");
            }
         }
         else if (kbUnitGetNumberContained(gRelicTempleID) >= kbPlayerGetProtoStatInt(cMyID, cUnitTypeTemple, cProtoStatMaxContained))
         {
            aiEchoWarning("We have no more room in our Temple to drop our Relic in, this shouldn't happen since we manage " +
               "only 1 relic at a time.");
            aiTaskUngarrisonUnit(heroID, kbUnitGetPosition(heroID));
            resetRelicData();
         }
         else if (relicHeroHasDirectPath(heroID, kbUnitGetPosition(gRelicTempleID)) == true)
         {
            if (aiPlanGetType(gRelicReservePlanID) != cPlanReserve)
            {
               debugExploration("Hero reached the Temple island, switching from transport to direct deposit.");
               createIslandRelicPlan(heroID, kbUnitGetProtoUnitID(heroID), kbUnitGetPosition(gRelicTempleID),
                  gRelicTempleID, "Relic return plan");
            }
            else
            {
               aiTaskWorkUnit(heroID, gRelicTempleID);
            }
         }
         else
         {
            debugExploration("Temple is not on a direct land path from our hero, waiting for the return transport plan.");
         }
      }
      return;
   }

   xsSetRuleMinInterval("relicCollectionMonitor", 30);
   if (aiPlanGetState(gPrimaryLandDefendPlan) == cPlanStateAttack)
   {
      debugExploration("Defend plan is in attack state, can't take heroes from it to collect island Relics now, quiting.");
      return;
   }
   if (kbUnitCount(cUnitTypeTemple, cMyID, cUnitStateAlive) <= 0)
   {
      debugExploration("We have no alive Temple, can't collect island Relics, quiting.");
      return;
   }

   int[] excludeTypes = new int(0, 0);
   int searchUnitType = cUnitTypeHero;
   if (cMyCulture == cCultureEgyptian && kbTechGetStatus(cTechHandsOfThePharaoh) != cTechStatusActive)
   {
      searchUnitType = cUnitTypePharaoh;
   }
   else if (cMyCulture == cCultureChinese)
   {
      if (kbTechGetStatus(cTechDivineLight) != cTechStatusActive)
      {
         debugExploration("Don't have Divine Light yet, can't collect island Relics yet.");
         return;
      }
      searchUnitType = cUnitTypePioneer;
   }
   else if (cMyCulture == cCultureJapanese)
   {
      excludeTypes.add(cUnitTypeMiko);
   }

   int[] units = aiPlanGetUnits(gPrimaryLandDefendPlan, searchUnitType, false, excludeTypes);
   int heroID = -1;
   int heroProtoUnitID = -1;
   if (units.size() > 0)
   {
      heroID = units[0];
      heroProtoUnitID = kbUnitGetProtoUnitID(heroID);
      debugExploration("Found " + kbProtoUnitGetName(heroProtoUnitID) + " (" + heroID + ") in defend plan to collect an island Relic with.");
   }
   else
   {
      debugExploration("No heroes in defend plan to collect island Relics with, quiting.");
      return;
   }

   vector searchPosition = getRelicHomePosition();
   int startAreaID = kbAreaGetIDByPosition(searchPosition);
   int queryID = useSimpleUnitQuery(cUnitTypeRelic, 0, cUnitStateAlive, searchPosition, cMaxFloat);
   kbUnitQuerySetConnectedAreaGroupID(queryID, kbAreaGetGroupID(startAreaID), cPassabilityAmphibious);
   kbUnitQuerySetAscendingSort(queryID, true);
   kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateRecentPositionKnown);
   int numResults = kbUnitQueryExecute(queryID);
   int[] relics = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < relics.size(); i++)
   {
      debugExploration("Found island Relic, ID: " + relics[i] + ".");
      if (kbCanAreaPath(searchPosition, kbUnitGetPosition(relics[i]), cPassabilityAmphibious, 100.0, false) == false)
      {
         debugExploration("Couldn't find an amphibious area path to this Relic, skipping.");
         continue;
      }
      bool canHeroPathToRelic = kbCanPath(searchPosition, kbUnitGetPosition(relics[i]), heroProtoUnitID, 1.0);
      if (canHeroPathToRelic == false && relicTargetNeedsTransport(searchPosition, kbUnitGetPosition(relics[i])) == false)
      {
         debugExploration("Couldn't find a land path or transport path to this island Relic, skipping.");
         continue;
      }
      if (canHeroPathToRelic == false)
      {
         debugExploration("Relic needs transport, using an attack/transport plan so the hero can travel by ship.");
      }
      gRelicID = relics[i];
      gRelicPosition = kbUnitGetPosition(gRelicID);
      createIslandRelicPlan(heroID, heroProtoUnitID, gRelicPosition, -1, "Relic reserve plan");
      xsSetRuleMinInterval("relicCollectionMonitor", 5);
      break;
   }
}

//==============================================================================
// relicCollectionMonitor
//==============================================================================
rule relicCollectionMonitor
group defaultClassicalRules
inactive
minInterval 30
{
   if (cPersonalityCurrent == cPersonalityHumanoid)
   {
      xsDisableRule("relicCollectionMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagCollectRelics) == false)
   {
      resetRelicData(false); // Don't spam the output each time.
      return;
   }
   debugExploration("--- Running Rule relicCollectionMonitor. ---");

   if (gMapInfo.mIsIslandMap == true)
   {
      relicIslandCollectionMonitor();
      return;
   }

   if (aiPlanGetIsIDValid(gRelicReservePlanID) == true)
   {
      debugExploration("We already have a Relic retrieval order going on currently, check in on it.");
      if (aiPlanGetNumberUnits(gRelicReservePlanID, -1, false) <= 0)
      {
         debugExploration("We've lost all heroes that we assigned to our reserve plan, restarting process.");
         resetRelicData();
         // Wait a bit longer, enemy may be camping the relic etc...
         xsSetRuleMinInterval("relicCollectionMonitor", 60);
         return;
      }
      else
      {
         int heroID = aiPlanGetUnitIDByIndex(gRelicReservePlanID, 0);
         debugExploration("Our reserve plan is still operational.");
         if (gWalkingBackToTemple == false)
         {
            if (xsVectorDistanceXZSqr(kbUnitGetPosition(heroID), gRelicPosition) < 5.0 * 5.0)
            {
               debugExploration("We're within 5 meters of our Relic, try to pick it up.");
               // Ask the position of the Relic, if we can't see the Relic anymore its position will be -10000 so this fails.
               if (xsVectorDistanceXZSqr(kbUnitGetPosition(gRelicID), gRelicPosition) > 5.0 * 5.0)
               {
                  debugExploration("The Relic is no longer at the spot we thought it was.");
                  resetRelicData();
               }
               else
               {
                  debugExploration("The Relic is at the spot we thought it was, try to pick it up now.");
                  aiTaskWorkUnit(heroID, gRelicID);
               }
            }
            else
            {
               debugExploration("We're not close enough to our Relic, sending another move command.");
               aiTaskMoveUnit(heroID, gRelicPosition);
            }
         }
         else
         {
            if (kbUnitGetIsIDValid(gRelicTempleID) == false || kbUnitGetPlayerID(gRelicTempleID) != cMyID)
            {
               debugExploration("We've lost the Temple we wanted to deposit at, trying to find a new one now.");
               int templeID = getSuitableTempleIDForRelic(aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0));
               if (templeID == -1)
               {
                  // We just drop the Relic here, we could also bring it back to our base potentially if we want to complicate this.
                  aiTaskUngarrisonUnit(heroID, kbUnitGetPosition(heroID));
                  debugExploration("Couldn't find a new Temple to deposit the Relic at.");
                  resetRelicData();
               }
               else
               {
                  debugExploration("Found a new Temple " + templeID + " to deposit the Relic at.");
                  gRelicTempleID = templeID;
                  aiTaskWorkUnit(heroID, gRelicTempleID);
               }
            }
            else
            {
               if (kbUnitGetNumberContained(gRelicTempleID) >= kbPlayerGetProtoStatInt(cMyID, cUnitTypeTemple, cProtoStatMaxContained))
               {
                  aiEchoWarning("We have no more room in our Temple to drop our Relic in, this shouldn't happen since we manage " +
                     "only 1 relic at a time.");
                  aiTaskUngarrisonUnit(heroID, kbUnitGetPosition(heroID));
                  resetRelicData();
               }
               else // Still room yaay, this should never fail really.
               {
                  aiTaskWorkUnit(heroID, gRelicTempleID);
               }
            }
         }
         return;
      }
   }

   xsSetRuleMinInterval("relicCollectionMonitor", 30);
   if (aiPlanGetState(gPrimaryLandDefendPlan) == cPlanStateAttack)
   {
      debugExploration("Defend plan is in attack state, can't take heroes from it to collect Relics now, quiting.");
      return;
   }
   if (kbUnitCount(cUnitTypeTemple, cMyID, cUnitStateAlive) <= 0)
   {
      debugExploration("We have no alive Temple, can't collect Relics, quiting.");
      return;
   }

   int[] excludeTypes = new int(0, 0);
   int searchUnitType = cUnitTypeHero;
   if (cMyCulture == cCultureEgyptian && kbTechGetStatus(cTechHandsOfThePharaoh) != cTechStatusActive)
   {
      searchUnitType = cUnitTypePharaoh;
   }
   else if (cMyCulture == cCultureChinese)
   {
      if (kbTechGetStatus(cTechDivineLight) != cTechStatusActive)
      {
         debugExploration("Don't have Divine Light yet, can't collect Relics yet.");
         return;
      }
      searchUnitType = cUnitTypePioneer;
   }
   else if (cMyCulture == cCultureJapanese)
   {
      // Don't collect Relics with Mikos, let them just gather favor.
      excludeTypes.add(cUnitTypeMiko);
   }

   int[] units = aiPlanGetUnits(gPrimaryLandDefendPlan, searchUnitType, false, excludeTypes);
   int heroID = -1;
   int heroProtoUnitID = -1;
   if (units.size() > 0)
   {
      heroID = units[0];
      heroProtoUnitID = kbUnitGetProtoUnitID(heroID);
      debugExploration("Found " + kbProtoUnitGetName(heroProtoUnitID) + " (" + heroID + ") in defend plan to collect a Relic with.");
   }
   else
   {
      debugExploration("No heroes in defend plan to collect Relics with, quiting.");
      return;
   }
   
   // Search entire map, area path danger should prevent us suiciding.
   vector searchPosition = aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
   int startAreaID = kbAreaGetIDByPosition(searchPosition);
   int queryID = useSimpleUnitQuery(cUnitTypeRelic, 0, cUnitStateAlive, searchPosition, cMaxFloat);
   // Area group of our defend plan.
   kbUnitQuerySetConnectedAreaGroupID(queryID, kbAreaGetGroupID(startAreaID), cPassabilityAmphibious);
   // Want to collect closest relics first.
   kbUnitQuerySetAscendingSort(queryID, true);
   // We could've scouted a Relic that is now taken by the enemy, it will now go to unknown position, guard against it.
   kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateRecentPositionKnown);
   int numResults = kbUnitQueryExecute(queryID);
   int[] relics = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < relics.size(); i++)
   {
      debugExploration("Found Relic, ID: " + relics[i] + ".");
      // Don't allow partial paths and avoid danger.
      if (kbCanAreaPath(searchPosition, kbUnitGetPosition(relics[i]), cPassabilityAmphibious, 100.0, false) == false)
      {
         debugExploration("Couldn't find an area path to this Relic, skipping.");
         continue;
      }
      if (kbCanPath(searchPosition, kbUnitGetPosition(relics[i]), heroProtoUnitID, 1.0) == false)
      {
         debugExploration("Couldn't find a real path to this Relic, skipping.");
         continue;
      }
      gRelicID = relics[i];
      gRelicPosition = kbUnitGetPosition(gRelicID);
      if (gRelicReservePlanID != -1)
      {
         aiPlanDestroy(gRelicReservePlanID);
         aiEchoWarning("Our gRelicReservePlanID wasn't -1 when we were going to create a new plan, we must never overlap.");
      }
      gRelicReservePlanID = aiPlanCreate("Relic reserve plan", cPlanReserve, -1, gExplorationCategoryID);
      aiPlanSetPriority(gRelicReservePlanID, 100);
      aiPlanAddUnitType(gRelicReservePlanID, heroProtoUnitID, 1, 1, 1);
      aiPlanAddUnit(gRelicReservePlanID, heroID);
      aiPlanSetFlag(gRelicReservePlanID, cPlanFlagNoMoreUnits, true); // Prevent auto assignment.
      aiPlanSetFlag(gRelicReservePlanID, cPlanFlagCantBeStolenFrom, true);
      aiTaskMoveUnit(heroID, gRelicPosition);
      xsSetRuleMinInterval("relicCollectionMonitor", 5);
      break;
   }
}

//==============================================================================
// relicGarrisonedHandler
//==============================================================================
void relicGarrisonedHandler(int techID = -1)
{
   sendStatementToEnemies(cAICommPromptToEnemyITookARelic);
   if (gMapInfo.mIsIslandMap == true)
   {
      if (aiPlanGetIsIDValid(gRelicReservePlanID) == true || gRelicHeroID != -1)
      {
         resetRelicData();
      }
   }
   else if (aiPlanGetIsIDValid(gRelicReservePlanID) == true)
   {
      resetRelicData();
   }
   debugExploration("Relic garrisoned: " + kbTechGetName(techID) + ".");
}

//==============================================================================
// relicPickedUpHandler
//==============================================================================
void relicPickedUpHandler(int techID = -1)
{
   sendStatementToAllies(cAICommPromptToAllyITookARelic);
   debugExploration("Relic picked up: " + kbTechGetName(techID) + ".");
   if (gMapInfo.mIsIslandMap == false)
   {
      if (aiPlanGetIsIDValid(gRelicReservePlanID) == false)
      {
         return;
      }
      if (aiPlanGetNumberUnits(gRelicReservePlanID) == 0)
      {
         debugExploration("We instantly lost the hero we picked up the relic with.");
         resetRelicData();
         return;
      }
      // If we're here we're going to assume this plan picked up a Relic.
      int heroID = aiPlanGetUnitIDByIndex(gRelicReservePlanID, 0);
      int templeID = getSuitableTempleIDForRelic(aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0));
      if (templeID == -1)
      {
         debugExploration("We must've lost our Temple while we were collecting our Relic...");
         aiTaskUngarrisonUnit(heroID, kbUnitGetPosition(heroID));
         resetRelicData();
      }
      else
      {
         debugExploration("Tasking hero " + heroID + " back to Temple " + templeID + ".");
         aiTaskWorkUnit(heroID, templeID);
         gWalkingBackToTemple = true;
         gRelicTempleID = templeID;
      }
      return;
   }

   if (gRelicHeroID == -1)
   {
      return;
   }
   int heroID = gRelicHeroID;
   if (kbUnitGetIsIDValid(heroID) == false || kbUnitGetPlayerID(heroID) != cMyID)
   {
      debugExploration("We instantly lost the hero we picked up the island relic with.");
      resetRelicData();
      return;
   }
   int templeID = getRelicHomeTempleID();
   if (templeID == -1)
   {
      debugExploration("We must've lost our home Temple while we were collecting our island Relic...");
      aiTaskUngarrisonUnit(heroID, kbUnitGetPosition(heroID));
      resetRelicData();
   }
   else
   {
      debugExploration("Tasking hero " + heroID + " back to home Temple " + templeID + ".");
      gWalkingBackToTemple = true;
      gRelicTempleID = templeID;
      createIslandRelicPlan(heroID, kbUnitGetProtoUnitID(heroID), kbUnitGetPosition(templeID), templeID,
         "Relic return plan");
      xsSetRuleMinInterval("relicCollectionMonitor", 5);
   }
}