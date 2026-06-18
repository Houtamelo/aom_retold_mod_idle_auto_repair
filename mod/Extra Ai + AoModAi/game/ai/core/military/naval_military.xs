//==============================================================================
/* naval_military.xs

   This file is intended for any naval military unit handling.

*/
//==============================================================================

class NavalAttackManager
{
   // States.
   int mState = cStateNormal;
   int mScoutingState = cNoScoutingNeeded;

   void updateState()
   {
      if (mScoutingState == cScoutingForEnemies || mScoutingState == cNoEnemies)
      {
         mState = cStateNeedScouting;
         return;
      }
      if (gDefensivelyOverrun == true ||
          (aiPlanGetIsIDValid(gAgeUpResearchPlan) == true && aiPlanGetPriority(gAgeUpResearchPlan) > 50 &&
          kbTechGetPercentComplete(aiPlanGetVariableInt(gAgeUpResearchPlan, cResearchPlanTechID, 0)) == 0.0))
      {
         mState = cStateForcedCantAttack;
         return;
      }
      mState = cStateNormal;
   }

   // Timers.
   int mLastAttackTime = 0;
   int mBaseAttackInterval = 180;
   int mAttackInterval = 180;
   bool waitedLongEnough()
   {
      if (mLastAttackTime + mAttackInterval < xsGetTime())
      {
         return true;
      }
      return false;
   }
};
extern NavalAttackManager gNavalAttackManager;

//==============================================================================
// getEnemyDocksArray
// Search for enemy Docks that we can potentially attack.
//==============================================================================
int[] getEnemyDocksArray()
{
   static int dockQueryID = -1;
   if (dockQueryID == -1) // First run.
   {
      dockQueryID = kbUnitQueryCreate("getEnemyDocksArray");
      kbUnitQuerySetPlayerRelation(dockQueryID, cPlayerRelationEnemyNotGaia, false);
      kbUnitQuerySetUnitType(dockQueryID, cUnitTypeAbstractDock);
      kbUnitQuerySetState(dockQueryID, cUnitStateABQ);
   }
   // Keep resetting the areaGroupID since it can change.
   kbUnitQuerySetConnectedAreaGroupID(dockQueryID, gDockAreaGroupID, cPassabilityWater);
   kbUnitQueryResetResults(dockQueryID);
   kbUnitQueryExecute(dockQueryID);
   return kbUnitQueryGetResults(dockQueryID);
}

//==============================================================================
// getEnemyDocksArraySpecificPlayer
// Search for enemy Docks that we can potentially attack.
//==============================================================================
int[] getEnemyDocksArraySpecificPlayer(int playerID = -1)
{
   static int dockQueryID = -1;
   if (dockQueryID == -1) // First run.
   {
      dockQueryID = kbUnitQueryCreate("getEnemyDocksArraySpecificPlayer");
      kbUnitQuerySetUnitType(dockQueryID, cUnitTypeAbstractDock);
      kbUnitQuerySetState(dockQueryID, cUnitStateABQ);
   }
   kbUnitQuerySetPlayerID(dockQueryID, playerID, false);
   // Keep resetting the areaGroupID since it can change.
   kbUnitQuerySetConnectedAreaGroupID(dockQueryID, gDockAreaGroupID, cPassabilityWater);
   kbUnitQueryResetResults(dockQueryID);
   kbUnitQueryExecute(dockQueryID);
   return kbUnitQueryGetResults(dockQueryID);
}

//==============================================================================
// mostHatedNavalEnemy
// Determine who we should attack on water, checking gOverrideNavalTargetPlayer too.
//==============================================================================
rule mostHatedNavalEnemy
minInterval 60
priority 84
group defaultClassicalRules
inactive
{
   if (gMapInfo.mHasWater == false)
   {
      debugNavalMilitary("Disabling mostHatedNavalEnemy because there is no water on the map.");
      xsDisableRule("mostHatedNavalEnemy");
      return;
   }
   if (cPersonalityCurrent == cPersonalityPassive)
   {
      xsDisableRule("mostHatedNavalEnemy");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutomaticNavalTargetPlayerPicking) == false)
   {
      return;
   }

   debugNavalMilitary("--- Running Rule mostHatedNavalEnemy. ---");

   if (gShouldBuildDock == false)
   {
      debugExploration("Currently we don't want to build a Dock, no point in picking a most hated naval enemy.");
      return;
   }

   if (gOverrideNavalTargetPlayer != cNoTargetPlayerOverrideUsed)
   {
      debugNavalMilitary("Override found: changing MostHatedNavalPlayerID to: " + gOverrideTargetPlayerID + ", " +
         cOverrideDontAttackPlayerID + " means don't attack");
      aiSetMostHatedNavalPlayerID(gOverrideTargetPlayerID);
      if (gOverrideTargetPlayerID > 0)
      {
         int[] targetPlayerDocks = getEnemyDocksArraySpecificPlayer(gOverrideTargetPlayerID);
         if (targetPlayerDocks.size() > 0)
         {
            gNavalAttackManager.mScoutingState = cNoScoutingNeeded;
         }
         else
         {
            gNavalAttackManager.mScoutingState = cScoutingForEnemies;
         }
      }
      return;
   }

   int[] enemyDocks = new int(0, 0);
   if (cPersonalityCurrent == cPersonalityRetaliator)
   {
      for (int i = 0; i < gEnemiesThatHaveAttackedUsNaval.size(); i++)
      {
         int[] playerDocks = getEnemyDocksArraySpecificPlayer(gEnemiesThatHaveAttackedUsNaval[i]);
         for (int iDock = 0; iDock < playerDocks.size(); iDock++)
         {
            enemyDocks.add(playerDocks[iDock]);
         }
      }
   }
   else
   {
      enemyDocks = getEnemyDocksArray();
   }
   int numEnemyDocks = enemyDocks.size();
   if (numEnemyDocks == 0)
   {
      if (getNumberEnemies() > 0)
      {
         debugNavalMilitary("We currently have enemies, we just have no Docks of them scouted, starting scouting logic.");
         gNavalAttackManager.mScoutingState = cScoutingForEnemies;
         aiSetMostHatedNavalPlayerID(-1);
      }
      else
      {
         debugNavalMilitary("We currently have no enemies, waiting.");
         gNavalAttackManager.mScoutingState = cNoEnemies;
         aiSetMostHatedNavalPlayerID(-1);
      }
      return;
   }

   gNavalAttackManager.mScoutingState = cNoScoutingNeeded;
   // Randomly take a Dock from our array to attack.
   int rand = xsRandInt(0, numEnemyDocks - 1);
   int targetPlayerID = kbUnitGetPlayerID(enemyDocks[rand]);

   debugNavalMilitary("Setting our MostHatedNavalPlayerID to " + targetPlayerID + ", determined via enemy Docks array.");
   aiSetMostHatedNavalPlayerID(targetPlayerID);
}

//==============================================================================
// metRequirementsToAttackNaval
//==============================================================================
bool metRequirementsToAttackNaval(int currentDefendPlanPop = 0)
{
   bool metRequirements = true;
   // If our defend plan is already in combat we won't send our navy to attack yet.
   if (aiPlanGetState(gPrimaryNavalDefendPlan) == cPlanStateAttack)
   {
      debugNavalMilitary("We can't attack because our naval defend plan is in state attack, meaning we're in danger already.");
      metRequirements = false;
   }

   int targetNavalMilitaryPop = aiGetNavalMilitaryPop();
   float neededNavalRatio = 0.30;
   if (cPersonalityCurrent == cPersonalityAttacker)
   {
      neededNavalRatio = 0.10;
   }
   if (cPersonalityCurrent == cPersonalityOverwhelmer || cPersonalityCurrent == cPersonalityEconomist)
   {
      neededNavalRatio = 0.50;
   }
   if (currentDefendPlanPop < targetNavalMilitaryPop * neededNavalRatio)
   {
      debugNavalMilitary("We can't attack because we have too few naval units. We have: " + currentDefendPlanPop + ", pop " + 
         "and we need: " + targetNavalMilitaryPop * neededNavalRatio + ".");
      metRequirements = false;
   }
   
   // We can potentially skip the time check.
   bool mustPerformTimeCheck = true;
   // Attackers and hard and above can potentially skip the timecheck based on army size / excess resources.
   // Conqueror / Supporter / Defender / Builder can't skip the timecheck, so they stay true to "attack less frequently" design.
   if (cPersonalityCurrent == cPersonalityAttacker || (cDifficultyCurrent >= cDifficultyHard &&
       cPersonalityCurrent != cPersonalityConqueror && cPersonalityCurrent != cPersonalitySupporter &&
       cPersonalityCurrent != cPersonalityDefender && cPersonalityCurrent != cPersonalityBuilder))
   {
      // Attackers require lower army percentage to skip the time check.
      float minRatioToSkipTimeCheck = cPersonalityCurrent == cPersonalityAttacker ? 0.3 : 0.5;
      int popRequiredToSkipTimeCheck = ceil(targetNavalMilitaryPop * minRatioToSkipTimeCheck);
      if (currentDefendPlanPop > popRequiredToSkipTimeCheck)
      {
         debugNavalMilitary("Skipping the time check because we have enough army: " + currentDefendPlanPop + "/" + 
            popRequiredToSkipTimeCheck + ".");
         mustPerformTimeCheck = false;
      }
      else
      {
         debugNavalMilitary("Skipping the time check based on army is not possible since we don't have enough army: "
            + currentDefendPlanPop + "/" + popRequiredToSkipTimeCheck + ".");
      }
      if (haveExcessResourceAmount(500, cAllResources) == true)
      {
         debugNavalMilitary("Skipping the time check because we have enough excess resources.");
         mustPerformTimeCheck = false;
      }
   }
   else
   {
      debugNavalMilitary("We are not allowed to potentially skip the time check.");
   }
   if (mustPerformTimeCheck == true)
   {
      if (gNavalAttackManager.waitedLongEnough() == false)
      {
         debugNavalMilitary("It's too early to launch an attack.");
         debugNavalMilitary("Next attack will happen at: " + turnNumberIntoTimeDisplay(gNavalAttackManager.mLastAttackTime +
                                                                                       gNavalAttackManager.mAttackInterval) + ".");
         metRequirements = false;
      }
      else
      {
         debugNavalMilitary ("We have waited long enough, we can attack.");
      }
   }

   debugNavalMilitary("metRequirementsToAttackNaval returned " + xsBoolToString(metRequirements) + ".");
   return metRequirements;
}

//==============================================================================
// navalAttackManager
// This rule analyzes the current situation in the game and decides if we should
// attack an enemy.
//==============================================================================
rule navalAttackManager
inactive
group defaultClassicalRules
minInterval 15
{
   if (gMapInfo.mHasWater == false)
   {
      debugNavalMilitary("Disabling navalAttackManager because there is no water on the map.");
      xsDisableRule("navalAttackManager");
      return;
   }
   if (cPersonalityCurrent == cPersonalityPassive)
   {
      xsDisableRule("navalAttackManager");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagCanNavalAttack) == false)
   {
      return;
   }
   
   debugNavalMilitary("--- Running navalAttackManager. ---");

   if (gShouldBuildDock == false)
   {
      debugNavalMilitary("Currently we don't want to build a Dock, can't attack on the water for now.");
      return;
   }
   if (aiPlanGetIsIDValid(gPrimaryNavalDefendPlan) == false)
   {
      debugNavalMilitary("gPrimaryNavalDefendPlan isn't valid yet, waiting for the defend logic to run.");
      return;
   }

   int targetPlayer = aiGetMostHatedNavalPlayerID();
   if (targetPlayer == cOverrideDontAttackPlayerID)
   {
      debugNavalMilitary("Can't attack because of global override.");
      return;
   }
   // We run the mostHatedEnemy rule less frequent than the attackManager. Things could've changed inbetween.
   if (targetPlayer != -1 && kbPlayerHasLost(targetPlayer) == true)
   {
      debugNavalMilitary("Can't attack because our target player has already lost: " + targetPlayer + ".");
      xsRuleIgnoreIntervalOnce("mostHatedNavalEnemy");
      return;
   }
   if (targetPlayer != -1 && kbPlayerIsAlly(targetPlayer) == true)
   {
      debugNavalMilitary("Can't attack because our target player is an ally of ours: " + targetPlayer + ".");
      xsRuleIgnoreIntervalOnce("mostHatedNavalEnemy");
      return;
   }

   int[] units = aiPlanGetUnits(gPrimaryNavalDefendPlan);
   // Calling kbUnitsGetPower with true for ignoreCurrentHealth we get back what is practically our pop value in the plan.
   // We use this and not aiPlanGetCurrentPopulation because there are military out there that have 0 pop cost.
   // With kbUnitsGetPower those units get a representation still.
   int currentDefendPlanPop = kbUnitsGetPower(units, true);
   if (currentDefendPlanPop <= 0)
   {
      debugNavalMilitary("We have 0 units in our defend plan, we must quit regardless of state.");
      return;
   }

   // Based on all information that has been fed into the attack manager we now determine what state we're in.
   gNavalAttackManager.updateState();

   switch (gNavalAttackManager.mState)
   {
      case cStateNormal:
      {
         debugNavalMilitary("We're in state cStateNormal, performing the regular checks now.");
         // Default checking.
         if (metRequirementsToAttackNaval(currentDefendPlanPop) == false)
         {
            return;
         }
         break;
      }

      case cStateForcedAttack:
      {
         debugNavalMilitary("We're in state cStateForcedAttack.");
         break;
      }

      case cStateForcedCantAttack:
      {
         debugNavalMilitary("We're in state cStateForcedCantAttack, quiting.");
         return;
      }

      case cStateNeedScouting:
      {
         debugNavalMilitary("We're in state cStateNeedScouting, details:");
         switch (gNavalAttackManager.mScoutingState)
         {
            // We're not targeting specific ships here, we will just perform aggro scouting to find those and kill them.
            case cScoutingForEnemies:
            {
               debugNavalMilitary("Can't attack because we're still in need of scouting new targets.");
            }
            case cNoEnemies:
            {
               debugNavalMilitary("Can't attack because we have no enemies at this moment.");
            }
            case cNoScoutingNeeded:
            {
               aiEchoWarning("gNavalAttackManager.mScoutingState == cNoScoutingNeeded and gNavalAttackManager.mState == cStateNeedScouting.");
            }
         }
         return;
      }
   }

   int[] targetPlayerDocks = getEnemyDocksArraySpecificPlayer(targetPlayer);
   if (targetPlayerDocks.size() == 0)
   {
      debugNavalMilitary("We can't find any Docks belonging to player " + targetPlayer + ", quiting.");
      xsRuleIgnoreIntervalOnce("mostHatedNavalEnemy");
      return;
   }
   
   int targetDockID = targetPlayerDocks[xsRandInt(0, targetPlayerDocks.size() - 1)];
   int targetAreaID = kbAreaGetIDByPosition(kbBaseGetLocation(targetPlayer, kbUnitGetBaseID(targetDockID)));
   if (kbAreaGetType(targetAreaID) != cAreaTypeWater && kbAreaGetType(targetAreaID) != cAreaTypeAmphibious)
   {
      aiEchoWarning("navalAttackManager - we found a Dock to attack but the base it's in is not on the water/amphibious.");
      return;
   }
   
   // If we're here we're clear to make an attack!
   int planID = aiPlanCreate("Naval Attack Player: " + targetPlayer + ", Dock: " + targetDockID, cPlanAttack, -1,
      gNavalMilitaryCategoryID);
   aiPlanSetVariableInt(planID, cAttackPlanTargetPlayerID, 0, targetPlayer);
   aiPlanSetVariableInt(planID, cAttackPlanTargetBaseID, 0, kbUnitGetBaseID(targetDockID));

   aiPlanSetVariableInt(planID, cAttackPlanTargetMode, 0, cAttackPlanTargetModeBase);
   aiPlanSetVariableInt(planID, cAttackPlanDoneMode, 0, cAttackPlanDoneModeCantFindMoreEnemyBases);
   
   aiPlanSetVariableBool(planID, cAttackPlanNaval, 0, true);
   aiPlanSetVariableVector(planID, cAttackPlanGatherPoint, 0, gWaterDefendPoint);
   aiPlanSetVariableFloat(planID, cAttackPlanGatherDistance, 0, 15.0);
   aiPlanSetVariableInt(planID, cAttackPlanGatherWaitTime, 0, 30 * 1000);
   setDefaultAttackPlanTargetUnitTypes(planID);

   aiPlanAddUnitType(planID, cUnitTypeLogicalTypeNavalMilitary, 0, 0, 200);
   // Manually add all our naval military to this attack plan to make sure they all go attack.
   transferAllUnitsBetweenTwoPlans(gPrimaryNavalDefendPlan, planID);
   aiPlanSetPriority(planID, 30);
   aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true);

   addPlanToAllSuitableGodpowerPlans(planID);

   sendStatementToAlliesWithVector(cAICommPromptToAllyIAmAttackingHere, kbUnitGetPosition(targetDockID));
   sendStatementToEnemies(cAICommPromptToEnemyIStartAnAttack);
   debugNavalMilitary("***** LAUNCHING NAVAL ATTACK on player: " + targetPlayer + ", Dock: " + targetDockID + ", Base: " +
      kbBaseGetNameByID(targetPlayer, kbUnitGetBaseID(targetDockID)) + ".");

   gNavalAttackManager.mLastAttackTime = xsGetTime();
   // We don't want to keep attacking in the same interval, that's too predicatable, offset a little using the base time.
   int randTime = xsRandInt(-30, 30);
   debugNavalMilitary("Randomly adjusting our attack interval by " + randTime + ".");
   gNavalAttackManager.mAttackInterval = gNavalAttackManager.mBaseAttackInterval + randTime;
   debugNavalMilitary("Next attack will happen at: " + turnNumberIntoTimeDisplay(gNavalAttackManager.mLastAttackTime +
                                                                                 gNavalAttackManager.mAttackInterval) + ".");
}

////////////////////////////////////////////////////////////////////////////////
// DEFENDING.
////////////////////////////////////////////////////////////////////////////////

//==============================================================================
// createPrimaryNavalDefendPlan
// Create our main naval defend plan.
//==============================================================================
void createPrimaryNavalDefendPlan()
{
   debugNavalMilitary("Creating primary naval defend plan.");
   gPrimaryNavalDefendPlan = aiPlanCreate("Primary naval Defend", cPlanDefend, -1, gNavalMilitaryCategoryID);
   aiPlanSetVariableBool(gPrimaryNavalDefendPlan, cDefendPlanNaval, 0, true);
   aiPlanSetVariableInt(gPrimaryNavalDefendPlan, cDefendPlanTargetMode, 0, cDefendPlanTargetModePoint);
   aiPlanSetVariableInt(gPrimaryNavalDefendPlan, cDefendPlanTargetPlayerID, 0, cMyID);
   aiPlanSetVariableFloat(gPrimaryNavalDefendPlan, cDefendPlanEngageRange, 0, 50.0);
   setDefaultDefendPlanTargetUnitTypes(gPrimaryNavalDefendPlan);
   aiPlanAddUnitType(gPrimaryNavalDefendPlan, cUnitTypeLogicalTypeNavalMilitary, 0, 0, 200);
   aiPlanSetPriority(gPrimaryNavalDefendPlan, 10); // Very low priority, don't steal from attack plans.
   addPlanToAllSuitableGodpowerPlans(gPrimaryNavalDefendPlan);
}

//==============================================================================
// navalDefendManager
//==============================================================================
rule navalDefendManager
inactive
group defaultArchaicRules
minInterval 5
{
   if (gMapInfo.mHasWater == false)
   {
      debugNavalMilitary("Disabling navalDefendManager because there is no water on the map.");
      xsDisableRule("navalDefendManager");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagCanNavalDefend) == false)
   {
      return;
   }

   debugNavalMilitary("--- Running navalDefendManager. ---");

   if (gShouldBuildDock == false)
   {
      debugNavalMilitary("Currently we don't want to build a Dock, can't defend on the water for now.");
      return;
   }

   // If our primary naval defend plan is invalid we need to set up again.
   if (aiPlanGetIsIDValid(gPrimaryNavalDefendPlan) == false)
   {
      createPrimaryNavalDefendPlan();
   }
   else if (cPersonalityCurrent == cPersonalityRetaliator && aiPlanGetState(gPrimaryNavalDefendPlan) == cPlanStateAttack)
   {
      int[] enemyUnits = new int(0, 0);
      int numEnemyUnits = aiPlanGetNumberVariableValues(gPrimaryNavalDefendPlan, cDefendPlanTargetIDs);
      for (int i = 0; i < numEnemyUnits; i++)
      {
         enemyUnits.add(aiPlanGetVariableInt(gPrimaryNavalDefendPlan, cDefendPlanTargetIDs, i));
      }
      int[] playersAttackingOnWater = kbGetPlayersThatAreAttackingUsFromUnits(enemyUnits);
      for (int i = 0; i < playersAttackingOnWater.size(); i++)
      {
         debugNavalMilitary("We're being attacked by player " + playersAttackingOnWater[i] + " on the water.");
         if (gEnemiesThatHaveAttackedUsNaval.find(playersAttackingOnWater[i]) != -1)
         {
            debugNavalMilitary("   We already have this player tracked to retaliate against.");
            continue;
         }
         if (areWeAlreadyAttackingPlayer(playersAttackingOnWater[i], true) == true)
         {
            debugNavalMilitary("   We are already attacking this player, don't consider him valid yet.");
            continue;
         }
         aiCommsSendStatement(playersAttackingOnWater[i], cAICommPromptToEnemyICanAttackYouNowNaval);
         gEnemiesThatHaveAttackedUsNaval.add(playersAttackingOnWater[i]);
         debugNavalMilitary("   We can retaliate against this player now.");
      }
   }
   
   static vector lastNavalDefendPoint = cInvalidVector;
   // If we've changed water bodies all units need to be removed from the plan since they can't reach the new group.
   if (lastNavalDefendPoint != cInvalidVector)
   {
      int lastAreaGroupID = kbAreaGroupGetIDByPosition(lastNavalDefendPoint);
      int newAreaGroupID = kbAreaGroupGetIDByPosition(gWaterDefendPoint);
      if (kbPathAreAreaGroupsConnected(lastAreaGroupID, newAreaGroupID, cPassabilityWater) == false)
      {
         int[] units = aiPlanGetUnits(gPrimaryNavalDefendPlan, -1, true);
         for (int i = 0; i < units.size(); i++)
         {
            aiPlanRemoveUnitFromAllPlans(units[i]);
         }
      }
   }
   // Dynamic data gets reset each time.
   // We patrol along all Docks that are far away from gWaterDefendPoint.
   lastNavalDefendPoint = gWaterDefendPoint;
   aiPlanSetVariableVector(gPrimaryNavalDefendPlan, cDefendPlanGatherPoint, 0, gWaterDefendPoint);
   aiPlanSetVariableVector(gPrimaryNavalDefendPlan, cDefendPlanTargetPoint, 0, gWaterDefendPoint);
   aiPlanSetVariableVector(gPrimaryNavalDefendPlan, cDefendPlanPatrolWaypoints, 0, gWaterDefendPoint);
   aiPlanSetVariableBool(gPrimaryNavalDefendPlan, cDefendPlanPatrol, 0, false);
   int numValues = 1;
   for (int i = 0; i < gDockManager.mValidDockIDs.size(); i++)
   {
      if (kbUnitGetIsIDValid(gDockManager.mValidDockIDs[i]) == false)
      {
         continue; // This array is updated in another rule.
      }
      vector dockPosition = kbUnitGetPosition(gDockManager.mValidDockIDs[i]);
      if (xsVectorLength(gWaterDefendPoint - dockPosition) > 30.0)
      {
         aiPlanSetNumberVariableValues(gPrimaryNavalDefendPlan, cDefendPlanPatrolWaypoints, numValues + 1);
         aiPlanSetVariableVector(gPrimaryNavalDefendPlan, cDefendPlanPatrolWaypoints, numValues, dockPosition);
         aiPlanSetVariableBool(gPrimaryNavalDefendPlan, cDefendPlanPatrol, 0, true);
         numValues++;
      }
   }
}