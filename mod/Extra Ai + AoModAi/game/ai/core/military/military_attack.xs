//==============================================================================
/* military_attack.xs

   This file is intended for any land/air military unit attack handling.

*/
//==============================================================================

class AttackManager
{
   // Must attack reasons.
   bool mExcessResources = false;
   bool mKOTHAttack = false;
   bool mKOTHPanic = false;
   int mLastUnderworldInvasionCastTime = cMinInt;

   // Can't attack reasons.
   bool mPrimaryLandDefendPlanIsEngaged = false;

   // States.
   int mState = cStateNormal;
   int mScoutingState = cNoScoutingNeeded;

   void updateState()
   {
      // Reset KOTH variables from last run.
      mKOTHAttack = false;
      mKOTHPanic = false;

      bool haveTitan = kbUnitCount(cUnitTypeAbstractTitan, cMyID, cUnitStateAlive) >= 1;
      bool haveTitanGate = kbUnitCount(cUnitTypeTitanGate, cMyID, cUnitStateBuilding) >= 1;
      bool ragnarokActive = (kbUnitCount(cUnitTypeHeroOfRagnarok, cMyID, cUnitStateAlive) +
                         kbUnitCount(cUnitTypeHeroOfRagnarokDwarf, cMyID, cUnitStateAlive)) >= 10;
      bool ceaseFireActive = kbGodPowerCheckActiveForAnyPlayer(cProtoPowerCeaseFire);
                         
      if ((cVictoryTypesCurrent & cVictoryTypeKingOfTheHill) != 0 && gKOTHIsOwnedByAllies == false)
      {
         debugMilitaryAttacking("KOTH Attack!");
         mKOTHAttack = true;
         mState = cStateNormal;
         if (getRemainingKOTHTime() < 240)
         {
            debugMilitaryAttacking("KOTH PANIC!");
            mKOTHPanic = true;
            mState = cStateForcedAttack;
         }
         return;
      }

      if (mScoutingState == cScoutingForEnemies || mScoutingState == cNoEnemies)
      {
         mState = cStateNeedScouting;
         return;
      }
      if (ceaseFireActive == true || mPrimaryLandDefendPlanIsEngaged == true || haveTitanGate == true || gDefensivelyOverrun == true)
      {
         mState = cStateForcedCantAttack;
         return;
      }
      if (kbGodPowerCheckActive(cProtoPowerFlamingWeapons, cMyID) == true || ragnarokActive == true || haveTitan == true ||
          xsGetTime() < mLastUnderworldInvasionCastTime + 10)
      {
         mState = cStateForcedAttack;
         return;
      }
      // Age up has a lower "priority" than the forced attack states.
      if (aiPlanGetIsIDValid(gAgeUpResearchPlan) == true && aiPlanGetPriority(gAgeUpResearchPlan) > 50 &&
          kbTechGetPercentComplete(aiPlanGetVariableInt(gAgeUpResearchPlan, cResearchPlanTechID, 0)) == 0.0)
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

   // Minimum attack size.
   int mMinimumAttackSize = 0;

   // Even if our allowed military pop is 5 and we have 7 pop it doesn't mean we should attack.
   // Such a situation basically indicates that we lost all our eco and we should actually rebuild instead.
   // Based on difficulty set some minimum attack sizes.
   void calculateMinAttackSizes()
   {
      // Static numbers for lower difficulties, they never unlock full eco etc...
      switch (cDifficultyCurrent)
      {
         case cDifficultyEasy:
         {
            if (cPersonalityCurrent == cPersonalityOverwhelmer || cPersonalityCurrent == cPersonalityEconomist)
            {
               mMinimumAttackSize = 17;
            }
            else
            {
               mMinimumAttackSize = 15;
            }
            return;
         }
         case cDifficultyModerate:
         {
            if (cPersonalityCurrent == cPersonalityOverwhelmer || cPersonalityCurrent == cPersonalityEconomist)
            {
               mMinimumAttackSize = 23;
            }
            else
            {
               mMinimumAttackSize = 20;
            }
            return;
         }
         case cDifficultyHard:
         {
            if (cPersonalityCurrent == cPersonalityOverwhelmer || cPersonalityCurrent == cPersonalityEconomist)
            {
               mMinimumAttackSize = 28;
            }
            else
            {
               mMinimumAttackSize = 25;
            }
            return;
         }
      }
      // Dynamic calculation for difficulties that progress through the game unchecked.
      if (cPersonalityCurrent == cPersonalityOverwhelmer || cPersonalityCurrent == cPersonalityEconomist)
      {
         mMinimumAttackSize = (20 * kbPlayerGetAge(cMyID)) + (3 * cDifficultyCurrent);
      }
      else
      {
         mMinimumAttackSize = (18 * kbPlayerGetAge(cMyID)) + (3 * cDifficultyCurrent);
      }
      // Attacker lower numbers.
      if (cPersonalityCurrent == cPersonalityAttacker)
      {
         mMinimumAttackSize -= 6 * cDifficultyCurrent;
      }
   }
};
extern AttackManager gAttackManager;

//==============================================================================
// setLastUnderworldInvasionCastTime
//==============================================================================
void setLastUnderworldInvasionCastTime(int time = 0)
{
   gAttackManager.mLastUnderworldInvasionCastTime = time;
}

//==============================================================================
// mostHatedEnemy
// Determine who we should attack, checking gOverrideTargetPlayerID too.
//==============================================================================
rule mostHatedEnemy
minInterval 60
priority 85
group defaultClassicalRules
inactive
{
   // Don't disable this rule for Passive, we want to have a mostHatedPlayer for our military units counter picker.
   if (checkStrategyFlag(cStrategyFlagAutomaticTargetPlayerPicking) == false)
   {
      return;
   }
   debugMilitaryAttacking("--- Running Rule mostHatedEnemy. ---");

   if (gOverrideTargetPlayerID != cNoTargetPlayerOverrideUsed)
   {
      debugMilitaryAttacking("Override found: changing MostHatedPlayerID to: " + gOverrideTargetPlayerID + ", " +
         cOverrideDontAttackPlayerID + " means don't attack");
      aiSetMostHatedPlayerID(gOverrideTargetPlayerID);
      if (gOverrideTargetPlayerID > 0)
      {
         if (kbBaseGetNumber(gOverrideTargetPlayerID) > 0)
         {
            gAttackManager.mScoutingState = cNoScoutingNeeded;
         }
         else
         {
            gAttackManager.mScoutingState = cScoutingForEnemies;
         }
      }
      return;
   }
   
   if (gCloseEnemyBaseID != -1)
   {
      // OwnerID can be -1 if the base just disappeared, don't need to error it will get fixed next defendManager run.
      int ownerID = kbBaseGetOwner(gCloseEnemyBaseID);
      if (ownerID != -1 && (cPersonalityCurrent != cPersonalityRetaliator || gEnemiesThatHaveAttackedUs.find(ownerID) != -1)) 
      {
         debugMilitaryAttacking("Found a valid close enemy base we must destroy first: " +
            kbBaseGetNameByID(ownerID, gCloseEnemyBaseID) + ".");
         debugMilitaryAttacking("We have chosen to attack player " +  ownerID + ".");
         aiSetMostHatedPlayerID(ownerID);
         gAttackManager.mScoutingState = cNoScoutingNeeded;
         return;
      }
   }

   int numberEnemiesWithVisibleBases = 0;
   int numberEnemies = 0;
   int[] enemyPlayers = new int(0, 0);
   int[] sameIslandEnemyPlayers = new int(0, 0);
   vector ourPosition = aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
   int ourAreaGroup = kbAreaGroupGetIDByPosition(ourPosition);
   
   // Part 1: fill up arrays.
   for (int i = 1; i <= cNumberPlayers; i++)
   {
      if (i == cMyID)
      {
         continue;
      }
      if (kbPlayerIsEnemy(i) == false)
      {
         continue;
      }
      if (kbPlayerHasLost(i) == true)
      {
         continue;
      }
      numberEnemies++;
      if (cPersonalityCurrent == cPersonalityRetaliator && gEnemiesThatHaveAttackedUs.find(i) == -1)
      {
         debugMilitaryAttacking("Player " + i + " is an enemy of ours but isn't in gEnemiesThatHaveAttackedUs, skipping.");
         continue;
      }
      int numBases = kbBaseGetNumber(i); 
      if (numBases > 0)
      {
         debugMilitaryAttacking("Including player " + i + " in the enemy array.");
         enemyPlayers.add(i);
         numberEnemiesWithVisibleBases++;
         for (int iBase = 0; iBase < numBases; iBase++)
         {
            vector loc = kbBaseGetLocation(i, kbBaseGetIDByIndex(i, iBase));
            int areaGroup = kbAreaGroupGetIDByPosition(loc);
            if (kbPathAreAreaGroupsConnected(ourAreaGroup, areaGroup, cPassabilityLand) == true)
            {
               debugMilitaryAttacking("Player " + i + " has a base on the same land mass as us.");
               sameIslandEnemyPlayers.add(i);
               break;
            }
         }
      }
      // No bases visible, but it's a random map, do some "cheating".
      else if (cGameTypeCurrent == cGameTypeRandomMap && gMapInfo.mIsNomadMap == false)
      {
         vector startingPos = kbPlayerGetStartingPosition(i);
         // If we have never scouted the starting position of the player we will still include them in the enemy array.
         // Basically we can assume the player still has a base at this location.
         // And human players also know where the players have spawned, so yes we cheat here a bit but players also know.
         if (kbLocationFogged(startingPos) == true || kbLocationVisible(startingPos) == true)
         {
            continue;
         }
         debugMilitaryAttacking("Including player " + i + " in the enemy array because we haven't scouted their starting position.");
         int areaGroup = kbAreaGroupGetIDByPosition(startingPos);
         if (kbPathAreAreaGroupsConnected(ourAreaGroup, areaGroup, cPassabilityLand) == true)
         {
            debugMilitaryAttacking("Player " + i + " starting position is on the same land mass as us.");
            sameIslandEnemyPlayers.add(i);
         }
         enemyPlayers.add(i);
         numberEnemiesWithVisibleBases++;
      }
   }

   // Part 2: early out if we have nothing to attack right now.
   if (numberEnemiesWithVisibleBases == 0)
   {
      if (numberEnemies > 0)
      {
         debugMilitaryAttacking("We currently have enemies, we just have no bases of them scouted, starting scouting logic.");
         gAttackManager.mScoutingState = cScoutingForEnemies;
         aiSetMostHatedPlayerID(-1);
      }
      else
      {
         debugMilitaryAttacking("We currently have no enemies, waiting.");
         gAttackManager.mScoutingState = cNoEnemies;
         aiSetMostHatedPlayerID(-1);
      }
      return;
   }

   // Part 3: decide if we should attack an enemy close to us or if we target a random enemy.
   gAttackManager.mScoutingState = cNoScoutingNeeded;
   bool shouldAttackClosestEnemy = false;
   if (gIsFFA == true)
   {
      shouldAttackClosestEnemy = true;
      debugMilitaryAttacking("We will attack our closest enemy because the game is FFA.");
   }
   if (numberEnemiesWithVisibleBases >= 3)
   {
      shouldAttackClosestEnemy = true;
      debugMilitaryAttacking("We will attack our closest enemy because we have >= 3 total enemies.");
   }

   // Part 4: decide who to attack based on what is in the arrays.
   int selectedEnemyPlayerID = -1;
   int numEnemiesOnSameIsland = sameIslandEnemyPlayers.size(); 
   if (numEnemiesOnSameIsland > 0)
   {
      debugMilitaryAttacking("We have enemies on our land mass, prioritizing those.");
      if (shouldAttackClosestEnemy == true)
      {
         int closestPlayerID = -1;
         float closestBaseDistance = cMaxFloat;
         for (int i = 0; i < numEnemiesOnSameIsland; i++)
         {
            int baseID = kbFindClosestBase(sameIslandEnemyPlayers[i], -1, ourPosition, cPassabilityLand, false);
            vector baseLocation = cInvalidVector;
            if (baseID == -1) // This should only be possible if we're in fact attacking the starting location.
            {
               baseLocation = kbPlayerGetStartingPosition(sameIslandEnemyPlayers[i]);
            }
            else
            {
               baseLocation = kbBaseGetLocation(sameIslandEnemyPlayers[i], baseID);
            }
            float distance = xsVectorDistance(ourPosition, baseLocation);
            if (distance < closestBaseDistance)
            {
               closestBaseDistance = distance;
               closestPlayerID = sameIslandEnemyPlayers[i];
            }
         }
         selectedEnemyPlayerID = closestPlayerID;
         debugMilitaryAttacking("Player with the closest base to us is: " + closestPlayerID + ".");
      }
      else
      {
         if (numEnemiesOnSameIsland == 1)
         {
            selectedEnemyPlayerID = sameIslandEnemyPlayers[0];
         }
         else
         {
            selectedEnemyPlayerID = sameIslandEnemyPlayers[xsRandInt(0, numEnemiesOnSameIsland - 1)];
         }
      }
   }
   else
   {
      debugMilitaryAttacking("We don't have enemies on our land mass, analyze all enemies now.");
      if (shouldAttackClosestEnemy == true)
      {
         int closestPlayerID = -1;
         float closestBaseDistance = cMaxFloat;
         for (int i = 0; i < enemyPlayers.size(); i++)
         {
            int baseID = kbFindClosestBase(enemyPlayers[i], -1, ourPosition, cPassabilityAmphibious, false);
            vector baseLocation = cInvalidVector;
            if (baseID == -1) // This should only be possible if we're in fact attacking the starting location.
            {
               baseLocation = kbPlayerGetStartingPosition(enemyPlayers[i]);
            }
            else
            {
               baseLocation = kbBaseGetLocation(enemyPlayers[i], baseID);
            }
            float distance = xsVectorDistance(ourPosition, baseLocation);
            if (distance < closestBaseDistance)
            {
               closestBaseDistance = distance;
               closestPlayerID = enemyPlayers[i];
            }
         }
         selectedEnemyPlayerID = closestPlayerID;
         debugMilitaryAttacking("Player with the closest base to us is: " + closestPlayerID + ".");
      }
      else
      {
         if (numberEnemiesWithVisibleBases == 1)
         {
            selectedEnemyPlayerID = enemyPlayers[0];
         }
         else
         {
            selectedEnemyPlayerID = enemyPlayers[xsRandInt(0, numberEnemiesWithVisibleBases - 1)];
         }
      }
   }

   debugMilitaryAttacking("We have chosen to attack player " +  selectedEnemyPlayerID + ".");
   aiSetMostHatedPlayerID(selectedEnemyPlayerID);
}

//==============================================================================
// preventSomeBuildersFromJoiningAttack
//==============================================================================
void preventSomeBuildersFromJoiningAttack(int attackPlanID = -1)
{
   if (cMyCulture != cCultureNorse)
   {
      return;
   }
   int threshold = selectByDifficulty(1, 1, 2, 2, 2, 2);
   int[] units = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypeLogicalTypeNorseSoldierThatBuilds);
   int numRemoved = 0;
   for (int i = 0; i < units.size(); i++)
   {
      debugMilitaryAttacking("Removing " + kbProtoUnitGetName(kbUnitGetProtoUnitID(units[i])) + " " + units[i] +
         " from attack plan to remain as an available builder.");
      numRemoved++;
      aiPlanRemoveUnit(gPrimaryLandDefendPlan, units[i]);
      // Make sure this builder doesn't get auto assigned to the plan if it still gathering.
      aiUnitAddForbiddenPlanID(units[i], attackPlanID);
      if (numRemoved >= threshold)
      {
         return;
      }
   }
}

//==============================================================================
// createUnitAttackPlan
//==============================================================================
int createUnitAttackPlan()
{
   vector gatherPoint = aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
   
   int planID = aiPlanCreate("Attacking Units", cPlanAttack, -1, gMilitaryAttackingCategoryID);
   aiPlanSetVariableInt(planID, cAttackPlanTargetMode, 0, cAttackPlanTargetModePoint);

   aiPlanSetVariableVector(planID, cAttackPlanGatherPoint, 0, gatherPoint);
   aiPlanSetVariableFloat(planID, cAttackPlanGatherDistance, 0, 15.0);
   aiPlanSetVariableInt(planID, cAttackPlanGatherWaitTime, 0, 30 * 1000);
   aiPlanSetVariableInt(planID, cAttackPlanDoneMode, 0, cAttackPlanDoneModeNoTarget | cAttackPlanDoneModeNoUnits);
   aiPlanAddUnitType(planID, cUnitTypeLogicalTypeLandMilitary, 0, 0, 200);

   preventSomeBuildersFromJoiningAttack(planID);
   // Manually add all our defending units to this attack plan to make sure they all go attack.
   int[] excludeTransferTypes = new int(1, cUnitTypePhoenixEgg);
   transferAllUnitsBetweenTwoPlans(gPrimaryLandDefendPlan, planID, excludeTransferTypes);
   setDefaultAttackPlanTargetUnitTypes(planID);

   // If we have god power plans that need a combat plan to function we assign them this plan now.
   addPlanToAllSuitableGodpowerPlans(planID);

   // Find our enemies and put max 5 in our plan.
   int numResults = 0;
   int[] results = new int(0, 0);
   int[] excludeTypes = new int(1, cUnitTypeNavalUnit);
   if (cPersonalityCurrent == cPersonalityRetaliator)
   {
      for (int i = 0; i < gEnemiesThatHaveAttackedUs.size(); i++)
      {
         int queryID = useSimpleUnitQuery(cUnitTypeLogicalTypeNeededForVictory, gEnemiesThatHaveAttackedUs[i], cUnitStateAlive);
         kbUnitQuerySetExcludeTypes(queryID, excludeTypes);
         kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateRecentPositionKnown);
         numResults += kbUnitQueryExecute(queryID);
         int[] tempResults = kbUnitQueryGetResults(queryID);
         for (int j = 0; j < tempResults.size(); j++)
         {
            results.add(tempResults[j]);
         }
      }
   }
   else
   {
      int queryID = useSimpleUnitQuery(cUnitTypeLogicalTypeNeededForVictory, cPlayerRelationEnemyNotGaia, cUnitStateAlive);
      kbUnitQuerySetExcludeTypes(queryID, excludeTypes);
      kbUnitQuerySetVisibleState(queryID, cUnitQueryVisibleStateRecentPositionKnown);
      numResults = kbUnitQueryExecute(queryID);
      results = kbUnitQueryGetResults(queryID);
   }
   
   if (numResults <= 0)
   {
      aiEchoWarning("Calling createUnitAttackPlan but there are no valid units to go attack, check this beforehand.");
      aiPlanDestroy(planID);
      return -1;
   }

   aiPlanSetNumberVariableValues(planID, cAttackPlanTargetPoint, min(numResults, 5));
   if (numResults <= 5)
   {
      for (int i = 0; i < numResults; i++)
      {
         vector unitPosition = kbUnitGetPosition(results[i]);
         aiPlanSetVariableVector(planID, cAttackPlanTargetPoint, i, unitPosition);
      }
   }
   else
   {
      // If we hit this again with another attack plan we hope we don't walk towards the same enemies basically.
      int startIndex = xsRandInt(0, numResults - 1);
      for (int i = 0; i < 5; i++)
      {
         if (startIndex == numResults)
         {
            startIndex = 0;
         }
         vector unitPosition = kbUnitGetPosition(results[startIndex]);
         aiPlanSetVariableVector(planID, cAttackPlanTargetPoint, i, unitPosition);
         startIndex++;
      }
   }

   aiPlanSetPriority(planID, 99);
   // Don't reduce this attack's size while underway.
   aiPlanSetFlag(planID, cPlanFlagCantBeStolenFrom, true);

   sendStatementToAllies(cAICommPromptToAllyIAmAttackingHere);
   sendStatementToEnemies(cAICommPromptToEnemyIStartAnAttack);

   return planID;
}

//==============================================================================
// createKOTHAttackPlan
//==============================================================================
int createKOTHAttackPlan()
{
   vector gatherPoint = aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
   
   int planID = aiPlanCreate("Attack: recapture KOTH", cPlanAttack, -1, gMilitaryAttackingCategoryID);
   aiPlanSetVariableInt(planID, cAttackPlanTargetMode, 0, cAttackPlanTargetModePoint);
   aiPlanSetVariableVector(planID, cAttackPlanTargetPoint, 0, gKOTHPosition);

   aiPlanSetVariableVector(planID, cAttackPlanGatherPoint, 0, gatherPoint);
   aiPlanSetVariableFloat(planID, cAttackPlanGatherDistance, 0, 15.0);
   aiPlanSetVariableInt(planID, cAttackPlanGatherWaitTime, 0, 30 * 1000);
   aiPlanSetVariableInt(planID, cAttackPlanDoneMode, 0, cAttackPlanDoneModeNoTarget | cAttackPlanDoneModeNoUnits);
   aiPlanAddUnitType(planID, cUnitTypeLogicalTypeLandMilitary, 0, 0, 200);

   preventSomeBuildersFromJoiningAttack(planID);
   // Manually add all our defending units to this attack plan to make sure they all go attack.
   int[] excludeTransferTypes = new int(1, cUnitTypePhoenixEgg);
   transferAllUnitsBetweenTwoPlans(gPrimaryLandDefendPlan, planID, excludeTransferTypes);
   setDefaultAttackPlanTargetUnitTypes(planID);

   // If we have god power plans that need a combat plan to function we assign them this plan now.
   addPlanToAllSuitableGodpowerPlans(planID);

   aiPlanSetPriority(planID, 99);
   // Don't reduce this attack's size while underway.
   aiPlanSetFlag(planID, cPlanFlagCantBeStolenFrom, true);

   sendStatementToAlliesWithVector(cAICommPromptToAllyIAmAttackingHere, gKOTHPosition);
   sendStatementToEnemies(cAICommPromptToEnemyIStartAnAttack);

   return planID;
}

//==============================================================================
// createDefaultAttackPlan
//==============================================================================
int createDefaultAttackPlan(int targetPlayer = -1, int targetBaseID = -1)
{
   vector gatherPoint = aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
   int planID = aiPlanCreate("Attack Player " + targetPlayer + " Base " + kbBaseGetNameByID(targetPlayer, targetBaseID),
                             cPlanAttack, -1, gMilitaryAttackingCategoryID);

   aiPlanSetVariableInt(planID, cAttackPlanTargetMode, 0, cAttackPlanTargetModeBase);
   aiPlanSetVariableInt(planID, cAttackPlanTargetBaseID, 0, targetBaseID);
   aiPlanSetVariableInt(planID, cAttackPlanTargetPlayerID, 0, targetPlayer);

   aiPlanSetVariableVector(planID, cAttackPlanGatherPoint, 0, gatherPoint);
   aiPlanSetVariableFloat(planID, cAttackPlanGatherDistance, 0, 15.0);
   aiPlanSetVariableInt(planID, cAttackPlanGatherWaitTime, 0, 30 * 1000);
   aiPlanSetVariableInt(planID, cAttackPlanDoneMode, 0, cAttackPlanDoneModeCantFindMoreEnemyBases | cAttackPlanDoneModeNoUnits);

   aiPlanAddUnitType(planID, cUnitTypeLogicalTypeLandMilitary, 0, 0, 200);

   preventSomeBuildersFromJoiningAttack(planID);
   // Manually add all our defending units to this attack plan to make sure they all go attack.
   int[] excludeTransferTypes = new int(1, cUnitTypePhoenixEgg);
   transferAllUnitsBetweenTwoPlans(gPrimaryLandDefendPlan, planID, excludeTransferTypes);
   setDefaultAttackPlanTargetUnitTypes(planID);

   // If we have god power plans that need a combat plan to function we assign them this plan now.
   addPlanToAllSuitableGodpowerPlans(planID);

   aiPlanSetPriority(planID, 99);
   // Don't reduce this attack's size while underway.
   aiPlanSetFlag(planID, cPlanFlagCantBeStolenFrom, true);
   return planID;
}

//==============================================================================
// createAttackPlanToPoint
//==============================================================================
int createAttackPlanToPoint(int targetPlayer = -1, vector targetPosition = cInvalidVector)
{
   vector gatherPoint = aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
   int planID = aiPlanCreate("Attack Player " + targetPlayer + " Position " + targetPosition, cPlanAttack, -1,
                             gMilitaryAttackingCategoryID);

   aiPlanSetVariableInt(planID, cAttackPlanTargetMode, 0, cAttackPlanTargetModePoint);
   aiPlanSetVariableVector(planID, cAttackPlanTargetPoint, 0, targetPosition);

   aiPlanSetVariableVector(planID, cAttackPlanGatherPoint, 0, gatherPoint);
   aiPlanSetVariableFloat(planID, cAttackPlanGatherDistance, 0, 15.0);
   aiPlanSetVariableInt(planID, cAttackPlanGatherWaitTime, 0, 30 * 1000);
   aiPlanSetVariableInt(planID, cAttackPlanDoneMode, 0, cAttackPlanDoneModeNoTarget | cAttackPlanDoneModeNoUnits);
   aiPlanAddUnitType(planID, cUnitTypeLogicalTypeLandMilitary, 0, 0, 200);

   preventSomeBuildersFromJoiningAttack(planID);
   // Manually add all our defending units to this attack plan to make sure they all go attack.
   int[] excludeTransferTypes = new int(1, cUnitTypePhoenixEgg);
   transferAllUnitsBetweenTwoPlans(gPrimaryLandDefendPlan, planID, excludeTransferTypes);
   setDefaultAttackPlanTargetUnitTypes(planID);

   // If we have god power plans that need a combat plan to function we assign them this plan now.
   addPlanToAllSuitableGodpowerPlans(planID);

   aiPlanSetPriority(planID, 99);
   // Don't reduce this attack's size while underway.
   aiPlanSetFlag(planID, cPlanFlagCantBeStolenFrom, true);
   return planID;
}

//==============================================================================
// getTargetBaseTypeChances
//==============================================================================
void getTargetBaseTypeChances(int[] units = default, bool titan = false, ref int strongestTCBaseChance, ref int weakestTCBaseChance,
   ref int marketBaseChance)
{
   // These are the base values.
   strongestTCBaseChance = 20;
   weakestTCBaseChance = 40;
   marketBaseChance = 10;

   if (titan == true)
   {
      // Gogo kill those TCs!
      strongestTCBaseChance = 100;
      weakestTCBaseChance = 0;
      marketBaseChance = 0;
      return;
   }

   // Depending on what we have sitting in our source plan we can change the values a bit.
   // We don't want to attack the strongest TC base if we barely have any sieging power.
   float siegePower = kbUnitsGetSiegePower(units, true);
   debugMilitaryAttacking("Our potential units for our attack have a total of: " + siegePower + " siege power.");
   if (siegePower >= 100.0)
   {
      strongestTCBaseChance += 20;
   }
   else if (siegePower >= 50.0)
   {
      weakestTCBaseChance += 20;
   }
   else
   {
      // Barely any siege available, Markets could be more interesting now.
      marketBaseChance += 20;
      strongestTCBaseChance = 0;
   }
}

//==============================================================================
// calculateTargetBase
//==============================================================================
int calculateTargetBase(int[] units = default, int targetPlayer = -1, bool titan = false)
{
   int numberBases = kbBaseGetNumber(targetPlayer);
   if (numberBases == 0)
   {
      // If this happens our mostHatedEnemy rule will notify the scouting code soon.
      debugMilitaryAttacking("Our targetPlayer (" + targetPlayer + ") has no bases to attack!");
      return -1;
   }

   // We can swap gCloseEnemyBaseID at a higher interval than mostHatedEnemy runs,
   // so make sure this base actually belongs to our targetPlayer.
   if (gCloseEnemyBaseID != -1 && kbBaseGetIsIDValid(targetPlayer, gCloseEnemyBaseID) == true)
   {
      debugMilitaryAttacking("Found a valid close enemy base we must attack first: " +
            kbBaseGetNameByID(targetPlayer, gCloseEnemyBaseID) + ".");
      return gCloseEnemyBaseID;
   }

   vector ownArmyPosition = aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
   int ownAreaGroupID = kbAreaGroupGetIDByPosition(ownArmyPosition);

   int strongestTCBaseID = -1;
   int strongestTCBaseHighestScore = 0.0;
   bool haveToTransportForStrongestTCBase = true;

   int weakestTCBaseID = -1;
   int weakestTCBaseLowestScore = cMaxInt;
   bool haveToTransportForWeakestTCBase = true;

   int marketBaseID = -1;
   int marketBaseLowestScore = 0.0;
   bool haveToTransportForMarketBase = true;

   int[] validBases = new int(0, 0);
   int[] validBasesNoTransporting = new int(0, 0);

   debugMilitaryAttacking("calculateTargetBase - Calculating player: " + targetPlayer + ", who has numberBases: " + numberBases + ".");
   // Go through all players' bases and calculate values for comparison.
   for (int baseIndex = 0; baseIndex < numberBases; baseIndex++)
   {
      int baseID = kbBaseGetIDByIndex(targetPlayer, baseIndex);
      vector baseLocation = kbBaseGetLocation(targetPlayer, baseID);
      int baseAssets = 0;

      int queryID = useSimpleUnitQuery(cUnitTypeTartarianGate, cPlayerRelationAlly, cUnitStateAlive, baseLocation, 45.0);
      if (kbUnitQueryExecute(queryID) >= 1)
      {
         debugMilitaryAttacking("Skipping base: " + kbBaseGetNameByID(targetPlayer, baseID) +
          ", because it has a Tartarian Gate in it.");
         continue;
      }
      
      debugMilitaryAttacking("Analyzing base: " + kbBaseGetNameByID(targetPlayer, baseID) + ".");
      int baseAreaGroup = kbAreaGroupGetIDByPosition(baseLocation);
      bool needToTransport = kbPathAreAreaGroupsConnected(ownAreaGroupID, baseAreaGroup, cPassabilityLand) == false;
      // Everything goes into the validBases array.
      validBases.add(baseID);
      if (needToTransport == false)
      {
         validBasesNoTransporting.add(baseID);
      }

      // Town Center base.
      if (kbBaseIsFlagSet(targetPlayer, baseID, cBaseFlagTownCenter) == true)
      {
         debugMilitaryAttacking("   This is a TC base.");
         int numberAttackBuildings = kbBaseGetNumberUnitsOfType(targetPlayer, baseID, cUnitTypeLogicalTypeBuildingsThatShoot);
         baseAssets += numberAttackBuildings * 300;

         int numberTrainBuildings = kbBaseGetNumberUnitsOfType(targetPlayer, baseID, cUnitTypeLogicalTypeMilitaryProductionBuilding);
         baseAssets += numberTrainBuildings * 100;

         int numberBuildings = kbBaseGetNumberUnitsOfType(targetPlayer, baseID, cUnitTypeBuilding);
         baseAssets += numberBuildings * 10;

         int numberMilitary = kbBaseGetNumberUnitsOfType(targetPlayer, baseID, cUnitTypeMilitaryUnit);
         baseAssets += numberBuildings * 10;

         if (strongestTCBaseID == -1 || baseAssets > strongestTCBaseHighestScore ||
             (haveToTransportForStrongestTCBase == true && needToTransport == false))
         {
            // If we have to transport for this base, but we already found a valid base that doesn't require it, we skip this one.
            if (haveToTransportForStrongestTCBase == false && needToTransport == true)
            {
            }
            else
            {
               strongestTCBaseID = baseID;
               strongestTCBaseHighestScore = baseAssets;
               haveToTransportForStrongestTCBase = needToTransport;
               debugMilitaryAttacking("   New best strongest TC base to attack: " + kbBaseGetNameByID(targetPlayer, baseID)
                  + ", baseAssets: " + baseAssets + ".");
            }
         }
         if (weakestTCBaseID == -1 || baseAssets < weakestTCBaseLowestScore ||
             (haveToTransportForWeakestTCBase == true && needToTransport == false))
         {
            // If we have to transport for this base, but we already found a valid base that doesn't require it, we skip this one.
            if (haveToTransportForWeakestTCBase == false && needToTransport == true)
            {
            }
            else
            {
               weakestTCBaseID = baseID;
               weakestTCBaseLowestScore = baseAssets;
               haveToTransportForWeakestTCBase = needToTransport;
               debugMilitaryAttacking("   New best weakest TC base to attack: " + kbBaseGetNameByID(targetPlayer, baseID)
                  + ", baseAssets: " + baseAssets + ".");
            }
         }
      }

      // Market base.
      else if (kbBaseGetNumberUnitsOfType(targetPlayer, baseID, cUnitTypeMarket) > 0)
      {
         debugMilitaryAttacking("   This is a Market base.");
         int numberAttackBuildings = kbBaseGetNumberUnitsOfType(targetPlayer, baseID, cUnitTypeLogicalTypeBuildingsThatShoot);
         baseAssets += numberAttackBuildings * 300;

         int numberTrainBuildings = kbBaseGetNumberUnitsOfType(targetPlayer, baseID, cUnitTypeLogicalTypeMilitaryProductionBuilding);
         baseAssets += numberTrainBuildings * 100;

         int numberBuildings = kbBaseGetNumberUnitsOfType(targetPlayer, baseID, cUnitTypeBuilding);
         baseAssets += numberBuildings * 10;

         int numberMilitary = kbBaseGetNumberUnitsOfType(targetPlayer, baseID, cUnitTypeMilitaryUnit);
         baseAssets += numberBuildings * 10;

         if (marketBaseID == -1 || baseAssets < marketBaseLowestScore ||
             (haveToTransportForMarketBase == true && needToTransport == false))
         {
            // If we have to transport for this base, but we already found a valid base that doesn't require it, we skip this one.
            if (haveToTransportForMarketBase == false && needToTransport == true)
            {
               continue;
            }
            marketBaseID = baseID;
            marketBaseLowestScore = baseAssets;
            haveToTransportForMarketBase = needToTransport;
            debugMilitaryAttacking("   New best market base to attack: " + kbBaseGetNameByID(targetPlayer, baseID)
               + ", baseAssets: " + baseAssets + ".");
         }
      }
   }

   int strongestTCBaseChance = 0;
   int weakestTCBaseChance = 0;
   int marketBaseChance = 0;
   getTargetBaseTypeChances(units, titan, strongestTCBaseChance, weakestTCBaseChance, marketBaseChance);
   debugMilitaryAttacking("Chances: strongestTCBaseChance: " + strongestTCBaseChance + ", weakestTCBaseChance: " +
      weakestTCBaseChance + ", marketBaseChance: " + marketBaseChance + ".");

   while (true)
   {
      int totalRoll = strongestTCBaseChance + weakestTCBaseChance + marketBaseChance;
      if (totalRoll == 0)
      {
         // No chances left, everything came up invalid. We just picking something random now, prefer not transporting.
         debugMilitaryAttacking("We were not able to find a target base from our preferences, just pick a random valid base.");
         if (validBasesNoTransporting.size() > 0)
         {
            int targetBaseID = validBasesNoTransporting[xsRandInt(0, validBasesNoTransporting.size() - 1)];
            debugMilitaryAttacking("Random base to attack without transporting: " +
               kbBaseGetNameByID(targetPlayer, targetBaseID) + ".");
            return targetBaseID;
         }
         if (validBases.size() > 0)
         {
            int targetBaseID = validBases[xsRandInt(0, validBases.size() - 1)];
            debugMilitaryAttacking("Random base to attack: " + kbBaseGetNameByID(targetPlayer, targetBaseID) + ".");
            return targetBaseID;
         }
         debugMilitaryAttacking("No random valid base available, returning -1.");
         return -1;
      }

      // Convert these into usable numbers.
      int weakestTCBaseRoll = weakestTCBaseChance + strongestTCBaseChance;
      int marketBaseRoll = weakestTCBaseChance + strongestTCBaseChance + marketBaseChance;

      int rand = xsRandInt(1, totalRoll);
      debugMilitaryAttacking("TotalRoll: " + totalRoll + ", rand: " + rand + ".");

      if (strongestTCBaseChance != 0 && rand <= strongestTCBaseChance)
      {
         debugMilitaryAttacking("Decided we attack the strongest TC base!");
         if (strongestTCBaseID != -1)
         {
            debugMilitaryAttacking("We have a strongest TC base to attack: " +
               kbBaseGetNameByID(targetPlayer, strongestTCBaseID) + ".");
            return strongestTCBaseID;
         }
         debugMilitaryAttacking("We don't have a strongest TC base to attack, removing the chance for rolling this.");
         strongestTCBaseChance = 0;
      }
      else if (weakestTCBaseChance != 0 && rand <= weakestTCBaseRoll)
      {
         debugMilitaryAttacking("Decided we attack the weakest TC base!");
         if (weakestTCBaseID != -1)
         {
            debugMilitaryAttacking("We have a weakest TC base to attack: " +
               kbBaseGetNameByID(targetPlayer, weakestTCBaseID) + ".");
            return weakestTCBaseID;
         }
         debugMilitaryAttacking("We don't have a weakest TC base to attack, removing the chance for rolling this.");
         weakestTCBaseChance = 0;
      }
      else if (marketBaseChance != 0 && rand <= marketBaseRoll)
      {
         debugMilitaryAttacking("Decided we attack a Market base!");
         if (marketBaseID != -1)
         {
            debugMilitaryAttacking("We have a Market base to attack: " + kbBaseGetNameByID(targetPlayer, marketBaseID) + ".");
            return marketBaseID;
         }
         debugMilitaryAttacking("We don't have a Market base to attack, removing the chance for rolling this.");
         marketBaseChance = 0;
      }
      else
      {
         aiEchoWarning("We shouldn't reach here, our chances are messed up.");
         return -1;
      }
   }

   aiEchoWarning("We shouldn't reach here, the while loop above must pick a target base or return -1.");
   return -1;
}

//==============================================================================
// metRequirementsToAttack
//==============================================================================
bool metRequirementsToAttack(int armyPop = -1, bool skipTimeCheck = false, ref int[] units)
{
   bool result = true;
   int wantedMilitaryPop = aiGetMilitaryPop();
   float neededArmyRatio = 0.50;
   if (cPersonalityCurrent == cPersonalityAttacker)
   {
      neededArmyRatio = 0.30;
   }
   if (cPersonalityCurrent == cPersonalityOverwhelmer || cPersonalityCurrent == cPersonalityEconomist)
   {
      neededArmyRatio = 0.70;
   }
   int minPop = ceil(wantedMilitaryPop * neededArmyRatio);
   // If we're Norse we're going to keep 2 infantry at home to have builders available, so need more minpop to compensate.
   if (cDifficultyCurrent >= cDifficultyHard && cMyCulture == cCultureNorse)
   {
      debugMilitaryAttacking("We're norse, keep 4 pop at home for building. Increasing pop required for the dynamic " + 
         "calculation by 4.");
      minPop += 4;
   }
   if (armyPop < minPop)
   {
      debugMilitaryAttacking("Dynamic military pop calculation: can't attack: " + armyPop + "/" + minPop + ".");
      result = false;
   }
   else
   {
      debugMilitaryAttacking("Dynamic military pop calculation: allowed to attack: " + armyPop + "/" + minPop + ".");
   }

   if (armyPop < gAttackManager.mMinimumAttackSize)
   {
      debugMilitaryAttacking("Static military pop minimum limit: can't attack: " + armyPop + "/" +
         gAttackManager.mMinimumAttackSize + ".");
      result = false;
   }
   else
   {
      debugMilitaryAttacking("Static military pop minimum limit: allowed to attack: " + armyPop + "/" +
         gAttackManager.mMinimumAttackSize + ".");
   }

   if (skipTimeCheck == false)
   {
      // We can potentially skip the time check.
      bool mustPerformTimeCheck = true;
      // Attackers and hard and above can potentially skip the timecheck based on army size / excess resources.
      // Conqueror / Supporter / Defender / Builder can't skip the timecheck, so they stay true to "attack less frequently" design.
      if (cPersonalityCurrent == cPersonalityAttacker || (cDifficultyCurrent >= cDifficultyHard &&
          cPersonalityCurrent != cPersonalityConqueror && cPersonalityCurrent != cPersonalitySupporter &&
          cPersonalityCurrent != cPersonalityDefender && cPersonalityCurrent != cPersonalityBuilder))
      {
         // Attackers require lower army percentage to skip the time check.
         float minRatioToSkipTimeCheck = cPersonalityCurrent == cPersonalityAttacker ? 0.5 : 0.7;
         int popRequiredToSkipTimeCheck = ceil(wantedMilitaryPop * minRatioToSkipTimeCheck);
         if (armyPop > popRequiredToSkipTimeCheck)
         {
            debugMilitaryAttacking("Skipping the time check because we have enough army: " + armyPop + "/" + 
               popRequiredToSkipTimeCheck + ".");
            mustPerformTimeCheck = false;
         }
         else
         {
            debugMilitaryAttacking("Skipping the time check based on army is not possible since we don't have enough army: "
               + armyPop + "/" + popRequiredToSkipTimeCheck + ".");
         }
         if (haveExcessResourceAmount(1000, cAllResources) == true)
         {
            debugMilitaryAttacking("Skipping the time check because we have enough excess resources.");
            mustPerformTimeCheck = false;
         }
      }
      else
      {
         debugMilitaryAttacking("We are not allowed to potentially skip the time check.");
      }
      if (mustPerformTimeCheck == true)
      {
         if (gAttackManager.waitedLongEnough() == false)
         {
            debugMilitaryAttacking("It's too early to launch an attack.");
            debugMilitaryAttacking("Next attack will happen at: " + turnNumberIntoTimeDisplay(gAttackManager.mLastAttackTime +
                                                                                          gAttackManager.mAttackInterval) + ".");
            result = false;
         }
         else
         {
            debugMilitaryAttacking ("We have waited long enough, we can attack.");
         }
      }
   }
   else
   {
      debugMilitaryAttacking("Skipping the time check because skipTimeCheck == true.");
   }

   if (cPersonalityCurrent == cPersonalitySieger && xsGetTime() < 900)
   {
      debugMilitaryAttacking("We're a Sieger and it's less than 15 minutes in-game, we require at least 2 units that are good " +
         "against buildings before we can attack.");
      int numSiegeUnitsFound = 0;
      for (int iUnit = 0; iUnit < units.size(); iUnit++)
      {
         if (kbUnitIsType(units[iUnit], cUnitTypeAbstractSiegeWeapon) == true)
         {
            numSiegeUnitsFound++;
         }
         #if (cMyCulture == cCultureAtlantean)
         int protoUnitID = kbUnitGetProtoUnitID(units[iUnit]);
         if (protoUnitID == cUnitTypeDestroyer || protoUnitID == cUnitTypeDestroyerHero)
         {
            numSiegeUnitsFound++;
         }
         #endif
      }
      if (numSiegeUnitsFound < 2)
      {
         debugMilitaryAttacking("Didn't find enough siege units in the defend plan, can't attack.");
         result = false;
      }
      else
      {
         debugMilitaryAttacking("Found enough siege units in the defend plan.");
      }
   }

   debugMilitaryAttacking("metRequirementsToAttack returned " + xsBoolToString(result) + ".");
   return result;
}

//==============================================================================
// modifyAttackStartTime
// The idea here is that we activate this after the BO is done.
// It will then randomly offset when we will launch our first attack to make sure it's not all synced up.
// The logic below fully expects there never having been an attack via the attackManager yet.
// So mLastAttackTime is 0 and mAttackInterval is equal to mBaseAttackInterval.
//==============================================================================
void modifyAttackStartTime()
{
   int instantAttackTime = xsGetTime() - gAttackManager.mBaseAttackInterval;
   if (instantAttackTime <= 0)
   {
      // If we ended the BO so early that we're not even past our attack interval time we must modify the attack interval.
      // This will realistically only do anything for DM since otherwise we won't have enough army anyway.
      int randInterval = xsRandInt(-30, 30); // Same as the normal offset we use in the attacking chain.
      debugMilitaryAttacking("Modifying our attack interval by " + randInterval + ".");
      // Our mAttackInterval will still be default at this point so we can just overwrite it.
      gAttackManager.mAttackInterval = gAttackManager.mBaseAttackInterval + randInterval;
      debugMilitaryAttacking("modifyAttackStartTime has determined our first attack should happen at: " +
         turnNumberIntoTimeDisplay(gAttackManager.mAttackInterval) + ".");
   }
   else
   {
      // We now have the value that if we set the gAttackManager.mLastAttackTime to that we would still instantly attack.
      // Decreasing this value makes no sense since then we just instantly attack, same result as leaving it as is.
      // What we do instead is increase the value by a random amount so that some AIs actually attack later.
      int randStart = xsRandInt(0, 60);
      debugMilitaryAttacking("Increasing our attack start time by " + randStart + ".");
      // Fake a last attack time so that we will wait our randStart from now.
      gAttackManager.mLastAttackTime = instantAttackTime + randStart;
      debugMilitaryAttacking("modifyAttackStartTime has determined our first attack should happen at: " +
         turnNumberIntoTimeDisplay(gAttackManager.mLastAttackTime + gAttackManager.mBaseAttackInterval) + ".");
   }
   gNeedOffsetAttackTimes = false;
}

int gRetaliatorValidTitanAttackPlayerID = -1;
//==============================================================================
// attackManager
// This rule analyzes the current situation in the game and decides if we should
// attack an enemy.
//==============================================================================
rule attackManager
inactive
group defaultClassicalRules
minInterval 15
{
   if (cPersonalityCurrent == cPersonalityPassive)
   {
      xsDisableRule("attackManager");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagCanAttack) == false)
   {
      return;
   }
   debugMilitaryAttacking("--- Running Rule attackManager. ---");
   if (gNeedOffsetAttackTimes == true)
   {
      modifyAttackStartTime();
   }

   gAttackManager.calculateMinAttackSizes();
   // The titanManager runs on a shorter interval than the AttackManager, so once this variable is set we're sure to have the
   // titanManager react to it. And then we reset it so the titanManager can't keep attacking this player, and also needs to wait.
   gRetaliatorValidTitanAttackPlayerID = -1;

   int targetPlayer = aiGetMostHatedPlayerID();
   if (targetPlayer == cOverrideDontAttackPlayerID)
   {
      debugMilitaryAttacking("Can't attack because of global override.");
      return;
   }
   // We run the mostHatedEnemy rule less frequent than the attackManager. Things could've changed inbetween.
   if (targetPlayer != -1 && kbPlayerHasLost(targetPlayer) == true)
   {
      debugMilitaryAttacking("Can't attack because our target player has already lost: " + targetPlayer + ".");
      return;
   }
   if (targetPlayer != -1 && kbPlayerIsAlly(targetPlayer) == true)
   {
      debugMilitaryAttacking("Can't attack because our target player is an ally of ours: " + targetPlayer + ".");
      return;
   }
   if (cPersonalityCurrent == cPersonalityRetaliator)
   {
      if (targetPlayer != -1 && gEnemiesThatHaveAttackedUs.find(targetPlayer) == -1)
      {
         // Wait for mostHatedEnemy to run again and fix this.
         debugMilitaryAttacking("Our current most hated player is not present in gEnemiesThatHaveAttackedUs, can't attack it.");
         return;
      }
   }

   int[] units = aiPlanGetUnits(gPrimaryLandDefendPlan);
   // Calling kbUnitsGetPower with true for ignoreCurrentHealth we get back what is practically our pop value in the plan.
   // We use this and not aiPlanGetCurrentPopulation because there are military out there that have 0 pop cost.
   // With kbUnitsGetPower those units get a representation still.
   int currentDefendPlanPop = kbUnitsGetPower(units, true);
   if (currentDefendPlanPop <= 0)
   {
      debugMilitaryAttacking("We have 0 units in our defend plan, we must quit regardless of state.");
      return;
   }

   // Based on all information that has been fed into the attack manager we now determine what state we're in.
   gAttackManager.updateState();

   bool targetUnits = false;
   switch (gAttackManager.mState)
   {
      case cStateNormal:
      {
         debugMilitaryAttacking("We're in state cStateNormal, performing the regular checks now.");
         // Default checking.
         if (metRequirementsToAttack(currentDefendPlanPop, false, units) == false)
         {
            return;
         }
         break;
      }

      case cStateForcedAttack:
      {
         debugMilitaryAttacking("We're in state cStateForcedAttack.");
         break;
      }

      case cStateForcedCantAttack:
      {
         debugMilitaryAttacking("We're in state cStateForcedCantAttack, quiting.");
         return;
      }

      case cStateNeedScouting:
      {
         debugMilitaryAttacking("We're in state cStateNeedScouting, details:");
         switch (gAttackManager.mScoutingState)
         {
            case cScoutingForEnemies:
            {
               // We need to make sure the we don't start attacking units if we have enemies inside our bases.
               // Because if there are and we make unit attack plans then our defend logic loses those units to defend with.
               // And then the defend logic can think we're actually overrun and cancel all these attacks again.
               for (int iBase = 0; iBase < gEnemyPowerInBases.size(); iBase++)
               {
                  if (gEnemyPowerInBases[iBase] > 0)
                  {
                     debugMilitaryAttacking("We're scouting for enemies but also see enemy units in our " +
                        "bases. First dealing with those, afterwards we can target units away from us.");
                     return;
                  }
               }

               int[] excludeTypes = new int(1, cUnitTypeNavalUnit);

               bool foundUnitToTarget = false;
               if (cPersonalityCurrent == cPersonalityRetaliator)
               {
                  for (int i = 0; i < gEnemiesThatHaveAttackedUs.size(); i++)
                  {
                     foundUnitToTarget = getUnit(cUnitTypeLogicalTypeNeededForVictory, gEnemiesThatHaveAttackedUs[i],
                        cUnitStateAlive, cUnitQueryVisibleStateRecentPositionKnown, excludeTypes) != -1;
                     if (foundUnitToTarget == true)
                     {
                        break;
                     }
                  }
               }
               else
               {
                  foundUnitToTarget = getUnit(cUnitTypeLogicalTypeNeededForVictory, cPlayerRelationEnemyNotGaia, cUnitStateAlive,
                     cUnitQueryVisibleStateRecentPositionKnown, excludeTypes) != -1;
               }
               
               if (foundUnitToTarget == true)
               {
                  targetUnits = true;
                  debugMilitaryAttacking("We're scouting for enemies and have found some, attacking those units now.");
                  // Need to have at least some pop in here to go.
                  // TODO if such a plan actually dies we know there are still many enemies and we need to send more units etc...
                  if (currentDefendPlanPop <= 10)
                  {
                     debugMilitaryAttacking("We have less than 10 pop in our defend plan, quit anyway.");
                     return;
                  }
               }
               else
               {
                  debugMilitaryAttacking("Can't attack because we're still in need of scouting new targets.");
                  return;
               }
               break;
            }
            case cNoEnemies:
            {
               debugMilitaryAttacking("Can't attack because we have no enemies at this moment.");
               return;
            }
            case cNoScoutingNeeded:
            {
               aiEchoWarning("gAttackManager.mScoutingState == cNoScoutingNeeded and gAttackManager.mState == cStateNeedScouting.");
               return;
            }
         }
      }
   }

   // Here we actually calculate what we're going to attack.
   if (gAttackManager.mKOTHAttack == true)
   {
      createKOTHAttackPlan();
      debugMilitaryAttacking("**** ATTACKING TO RECAPTURE THE KOTH!!! ****");
   }
   else if (targetUnits == true)
   {
      createUnitAttackPlan();
      debugMilitaryAttacking("**** ATTACKING UNIT POSITIONS!!! ****");
   }
   else
   {
      if (cPersonalityCurrent == cPersonalityRetaliator)
      {
         debugMilitaryAttacking("Removing " + targetPlayer + " from gEnemiesThatHaveAttackedUs since we're retaliating now.");
         gEnemiesThatHaveAttackedUs.removeValue(targetPlayer);
         gRetaliatorValidTitanAttackPlayerID = targetPlayer; // Our Titan logic can now still attack this player as well.
      }
      int numBases = kbBaseGetNumber(targetPlayer);
      if (numBases > 0)
      {
         int targetBaseID = calculateTargetBase(units, targetPlayer, false);
         if (targetBaseID == -1)
         {
            debugMilitaryAttacking("Can't find a new base to attack for player " + targetPlayer + ", waiting on mostHatedEnemy now.");
            return;
         }
         createDefaultAttackPlan(targetPlayer, targetBaseID);
         sendStatementToAlliesWithVector(cAICommPromptToAllyIAmAttackingHere, kbBaseGetLocation(targetPlayer, targetBaseID));
         sendStatementToEnemies(cAICommPromptToEnemyIStartAnAttack);
         debugMilitaryAttacking("***** LAUNCHING ATTACK on player: " + targetPlayer + ", base: " +
            kbBaseGetNameByID(targetPlayer, targetBaseID));
      }
      else
      {
         vector startingPosition = kbPlayerGetStartingPosition(targetPlayer);
         createAttackPlanToPoint(targetPlayer, startingPosition);
         sendStatementToAlliesWithVector(cAICommPromptToAllyIAmAttackingHere, startingPosition);
         sendStatementToEnemies(cAICommPromptToEnemyIStartAnAttack);
         debugMilitaryAttacking("***** LAUNCHING ATTACK on player: " + targetPlayer + ", Starting Position: " + startingPosition + ".");
      }
   }
   gAttackManager.mLastAttackTime = xsGetTime();
   // We don't want to keep attacking in the same interval, that's too predicatable, offset a little using the base time.
   int randTime = xsRandInt(-30, 30);
   debugMilitaryAttacking("Randomly adjusting our attack interval by " + randTime + ".");
   gAttackManager.mAttackInterval = gAttackManager.mBaseAttackInterval + randTime;
   debugMilitaryAttacking("Next attack will happen at: " + turnNumberIntoTimeDisplay(gAttackManager.mLastAttackTime +
                                                                                     gAttackManager.mAttackInterval) + ".");
   // Potentially cast Smart Wind.
   checkForTailwind();
}

////////////////////////////////////////////
// Titan Management
////////////////////////////////////////////

//==============================================================================
// createTitanDefendPlan
//==============================================================================
void createTitanDefendPlan(int titanID = -1)
{
   int planID = aiPlanCreate("Defend titan " + titanID, cPlanDefend, -1, gMilitaryDefendingCategoryID);
   // We mimic the mode the primary defend plan is in.
   if (aiPlanGetVariableInt(gPrimaryLandDefendPlan, cDefendPlanTargetMode, 0) == cDefendPlanTargetModeBase)
   {
      aiPlanSetVariableInt(planID, cDefendPlanTargetMode, 0, cDefendPlanTargetModeBase);
      aiPlanSetVariableInt(planID, cDefendPlanTargetPlayerID, 0, cMyID);
      int baseID = aiPlanGetBaseID(gPrimaryLandDefendPlan);
      aiPlanSetVariableInt(planID, cDefendPlanTargetBaseID, 0, baseID);
      aiPlanSetVariableFloat(planID, cDefendPlanEngageRange, 0, kbBaseGetDistance(cMyID, baseID) + 10.0);
      aiPlanSetVariableVector(planID, cDefendPlanGatherPoint, 0, kbBaseGetMilitaryGatherPoint(cMyID, baseID));
      aiPlanSetBaseID(planID, baseID);
   }
   else
   {
      aiPlanSetVariableInt(planID, cDefendPlanTargetMode, 0, cDefendPlanTargetModePoint);
      vector gatherPoint = aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
      aiPlanSetVariableVector(planID, cDefendPlanTargetPoint, 0, gatherPoint);
      aiPlanSetVariableVector(planID, cDefendPlanGatherPoint, 0, gatherPoint);
   }
   
   aiPlanAddUserVariableBool(planID, 0, "Titan Plan", 1); // Add this so we can identify this being a titan plan.
   aiPlanSetUserVariableBool(planID, 0, 0, true);
   addPlanToAllSuitableGodpowerPlans(planID);
   aiPlanAddUnitType(planID, cUnitTypeAbstractTitan, 1, 1, 1);
   aiPlanAddUnit(planID, titanID);
   setDefaultDefendPlanTargetUnitTypes(planID);
   aiPlanSetPriority(planID, 100);
   aiPlanSetFlag(planID, cPlanFlagCantBeStolenFrom, true);
   aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true);
   aiPlanSetFlag(planID, cPlanFlagDestroyWhenNoUnitsLeft, true);
}

//==============================================================================
// manageTitanDefendPlan
// In createTitanDefendPlan we just put our titan right where our gPrimaryLandDefendPlan was, see here if we need to update.
//==============================================================================
void manageTitanDefendPlan(int planID = -1, int titanID = -1)
{
   int numTCBases = gDefendTCBases.size();
   if (numTCBases == 0)
   {
      // Convert to defend point following gPrimaryLandDefendPlan;
      debugMilitaryDefending(aiPlanGetName(planID) + " we have no TC bases, defend the point that gPrimaryLandDefendPlan is " +
         "also defending.");
      aiPlanSetVariableInt(planID, cDefendPlanTargetMode, 0, cDefendPlanTargetModePoint);
      vector gatherPoint = aiPlanGetVariableVector(gPrimaryLandDefendPlan, cDefendPlanGatherPoint, 0);
      aiPlanSetVariableVector(planID, cDefendPlanTargetPoint, 0, gatherPoint);
      aiPlanSetVariableVector(planID, cDefendPlanGatherPoint, 0, gatherPoint);
      aiPlanResetBaseID(planID);
      return;
   }
   int numRegularDefendPlans = gDefendPlans.size();
   int currentBaseID = aiPlanGetBaseID(planID);
   if (currentBaseID != -1)
   {
      for (int i = 0; i < numRegularDefendPlans; i++)
      {
         if (currentBaseID == aiPlanGetBaseID(gDefendPlans[i]))
         {
            debugMilitaryDefending(aiPlanGetName(planID) + " doesn't need to move to another base.");
            return; // We're already defending a base that's still under attack.
         }
      }
   }
   if (numRegularDefendPlans == 0)
   {
      int baseID = aiPlanGetBaseID(gPrimaryLandDefendPlan);
      if (currentBaseID == baseID)
      {
         debugMilitaryDefending(aiPlanGetName(planID) + " we're not defending any base but our plan was already at the spot " +
         " where gPrimaryLandDefendPlan also already is.");
         return; // Already set up well.
      }
      aiPlanSetVariableInt(planID, cDefendPlanTargetMode, 0, cDefendPlanTargetModeBase);
      aiPlanSetVariableInt(planID, cDefendPlanTargetPlayerID, 0, cMyID);
      aiPlanSetVariableInt(planID, cDefendPlanTargetBaseID, 0, baseID);
      aiPlanSetVariableFloat(planID, cDefendPlanEngageRange, 0, kbBaseGetDistance(cMyID, baseID) + 10.0);
      aiPlanSetVariableVector(planID, cDefendPlanGatherPoint, 0, kbBaseGetMilitaryGatherPoint(cMyID, baseID));
      aiPlanSetBaseID(planID, baseID);
      debugMilitaryDefending(aiPlanGetName(planID) + " we're currently not defending any useful base, return to where " +
         "gPrimaryLandDefendPlan is stationed.");
      return;
   }
   float closestDistance = cMaxFloat;
   int closestBaseID = -1;
   vector myPosition = aiPlanGetLocation(planID, true);
   for (int i = 0; i < numRegularDefendPlans; i++)
   {
      float distance = xsVectorDistanceSqr(myPosition, kbBaseGetLocation(cMyID, aiPlanGetBaseID(gDefendPlans[i])));
      if (distance < closestDistance)
      {
         closestDistance = distance;
         closestBaseID = aiPlanGetBaseID(gDefendPlans[i]);
      }
   }

   aiPlanSetVariableInt(planID, cDefendPlanTargetMode, 0, cDefendPlanTargetModeBase);
   aiPlanSetVariableInt(planID, cDefendPlanTargetPlayerID, 0, cMyID);
   aiPlanSetVariableInt(planID, cDefendPlanTargetBaseID, 0, closestBaseID);
   aiPlanSetVariableFloat(planID, cDefendPlanEngageRange, 0, kbBaseGetDistance(cMyID, closestBaseID) + 10.0);
   aiPlanSetVariableVector(planID, cDefendPlanGatherPoint, 0, kbBaseGetMilitaryGatherPoint(cMyID, closestBaseID));
   aiPlanSetBaseID(planID, closestBaseID);
   debugMilitaryDefending(aiPlanGetName(planID) + " we're going to defend base: " + kbBaseGetNameByID(cMyID, closestBaseID) + ".");
}

//==============================================================================
// defensiveTitanManager
// Called via titanManager if we're not allowed to attack/explore with our Titans.
//==============================================================================
void defensiveTitanManager()
{
   debugMilitaryDefending("--- Running 'Rule' defensiveTitanManager. ---");
   int queryID = useSimpleUnitQuery(cUnitTypeAbstractTitan);
   int numTitans = kbUnitQueryExecute(queryID);
   if (numTitans == 0)
   {
      xsDisableRule("titanManager");
      return;
   }
   int[] titans = kbUnitQueryGetResults(queryID);
   for (int i = 0; i < numTitans; i++)
   {
      int titanID = titans[i];
      int currentPlanID = kbUnitGetPlanID(titanID);
      if (currentPlanID != -1)
      {
         // This can be a child reinforcement plan, take the defend parent instead.
         int parentPlanID = aiPlanGetParentID(currentPlanID);
         if (parentPlanID != -1)
         {
            currentPlanID = parentPlanID;
         }
      }
      bool needNewPlan = false;
      if (currentPlanID == -1)
      {
         needNewPlan = true;
      }
      // If we're in a plan that is not meant to have a Titan in it we need a new plan, but don't destroy this plan.
      if (needNewPlan == false && aiPlanGetUserVariableIndex(currentPlanID, "Titan Plan") == -1)
      {
         needNewPlan = true;
      }
      if (needNewPlan == true)
      {
         createTitanDefendPlan(titanID);
      }
      else if (aiPlanGetType(currentPlanID) == cPlanDefend) // Retaliator can end up here when some Titans are still attacking.
      {
         manageTitanDefendPlan(currentPlanID, titanID);
      }
   }
}

////////////////////////////////////////////
const int cTitanStateNeedsTarget = 0;
const int cTitanStateWalking = 1;
const int cTitanStateAttackingUnit = 2;
const int cTitanStateAttackingBuildingThatShoot = 3;
const int cTitanStateAttackingTownCenter = 4;
const int cTitanStateAttackingTitan = 5;

//==============================================================================
// assignAllTitansToExplorePlans
//==============================================================================
void assignAllTitansToExplorePlans(int queryID = -1, int numTitans = -1)
{
   for (int i = 0; i < numTitans; i++)
   {
      int titanID = kbUnitQueryGetResult(queryID, i);
      int currentPlanID = kbUnitGetPlanID(titanID);
      bool needNewPlan = false;
      bool destroyOldPlan = false;
      if (currentPlanID == -1)
      {
         needNewPlan = true;
      }
      // If we're in a plan that is not meant to have a Titan in it we need a new plan, but don't destroy this plan.
      if (needNewPlan == false && aiPlanGetNumberUserVariableValues(currentPlanID, 0) == 0)
      {
         needNewPlan = true;
      }
      // If we're currently in an attack plan (that has user variables) it's a Titan attack plan so it needs to be destroyed.
      if (needNewPlan == false && aiPlanGetType(currentPlanID) == cPlanAttack)
      {
         needNewPlan = true;
         destroyOldPlan = true;
      }
      // Clean up the Titan attack plan.
      if (destroyOldPlan == true)
      {
         aiPlanSetState(currentPlanID, cPlanStateDone);
      }
      if (needNewPlan == true)
      {
         int newPlanID = aiPlanCreate("Titan Explore " + titanID, cPlanExplore, -1, gExplorationCategoryID);
         aiPlanSetPriority(newPlanID, 100);
         aiPlanSetVariableBool(newPlanID, cExplorePlanAggressiveScouts, 0, true);
         aiPlanSetVariableBool(newPlanID, cExplorePlanAvoidingAttackedAreas, 0, false);
         setDefaultExplorePlanTargetUnitTypes(newPlanID);
         aiPlanAddUnitType(newPlanID, cUnitTypeAbstractTitan, 1, 1, 1);
         aiPlanAddUnit(newPlanID, titanID);
         aiPlanSetFlag(newPlanID, cPlanFlagNoMoreUnits, true);
         aiPlanSetFlag(newPlanID, cPlanFlagCantBeStolenFrom, true);
         aiPlanSetFlag(newPlanID, cPlanFlagDestroyWhenNoUnitsLeft, true);
         // Explore other islands if we need to.
         if (gMapInfo.mIsIslandMap == true)
         {
            helperExploreOtherIslands(newPlanID);
         }
         aiPlanAddUserVariableBool(newPlanID, 0, "Titan Plan", 1); // Add this so we can identify this being a titan plan.
         aiPlanSetUserVariableBool(newPlanID, 0, 0, true);
         addPlanToAllSuitableGodpowerPlans(newPlanID);
         debugMilitaryAttacking("Created an explore plan for Titan " + titanID + ": " + aiPlanGetName(newPlanID) + ".");
      }
   }
}

//==============================================================================
// createTitanAttackPlan
//==============================================================================
int createTitanAttackPlan(int titanID = -1, int targetPlayer = -1, int targetBaseID = -1)
{
   int planID = aiPlanCreate("Titan Attack " + titanID, cPlanAttack, -1, gMilitaryAttackingCategoryID);
   aiPlanAddUserVariableBool(planID, 0, "Titan Plan", 1); // Add this so we can identify this being a titan plan.
   aiPlanSetUserVariableBool(planID, 0, 0, true);

   aiPlanSetVariableInt(planID, cAttackPlanTargetMode, 0, cAttackPlanTargetModeBase);
   aiPlanSetVariableInt(planID, cAttackPlanTargetBaseID, 0, targetBaseID);
   aiPlanSetVariableInt(planID, cAttackPlanTargetPlayerID, 0, targetPlayer);
   aiPlanSetVariableInt(planID, cAttackPlanDoneMode, 0, cAttackPlanDoneModeCantFindMoreEnemyBases | cAttackPlanDoneModeNoUnits);

   aiPlanSetVariableVector(planID, cAttackPlanGatherPoint, 0, kbUnitGetPosition(titanID));
   aiPlanSetVariableFloat(planID, cAttackPlanGatherDistance, 0, 15.0);
   aiPlanSetVariableInt(planID, cAttackPlanGatherWaitTime, 0, 30 * 1000);

   aiPlanAddUnitType(planID, cUnitTypeAbstractTitan, 1, 1, 1);
   aiPlanAddUnit(planID, titanID);
   setDefaultAttackPlanTargetUnitTypes(planID);

   aiPlanSetPriority(planID, 100);
   aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true);
   aiPlanSetFlag(planID, cPlanFlagCantBeStolenFrom, true);
   aiPlanSetFlag(planID, cPlanFlagDestroyWhenNoUnitsLeft, true);
   addPlanToAllSuitableGodpowerPlans(planID);
   return planID;
}

//==============================================================================
// titanManager
//==============================================================================
rule titanManager
inactive
minInterval 5
{
   if (checkStrategyFlag(cStrategyFlagAutomaticTitanManagement) == false)
   {
      return;
   }
   if (cPersonalityCurrent == cPersonalityPassive)
   {
      defensiveTitanManager();
      return;
   }
   debugMilitaryAttacking("--- Running Rule titanManager. ---");

   int queryID = useSimpleUnitQuery(cUnitTypeAbstractTitan);
   int numTitans = kbUnitQueryExecute(queryID);
   if (numTitans == 0)
   {
      xsDisableRule("titanManager");
      return;
   }
   int[] titans = kbUnitQueryGetResults(queryID);

   int targetPlayer = -1;
   if (cPersonalityCurrent == cPersonalityRetaliator)
   {
      if (gRetaliatorValidTitanAttackPlayerID == -1)
      {
         debugMilitaryAttacking("Can't attack because gRetaliatorValidTitanAttackPlayerID is -1.");
         defensiveTitanManager();
         return;
      }
      else
      {
         targetPlayer = gRetaliatorValidTitanAttackPlayerID;
      }
   }
   else
   {
      targetPlayer = aiGetMostHatedPlayerID();
      bool cantAttack = false;
      if (targetPlayer == cOverrideDontAttackPlayerID)
      {
         debugMilitaryAttacking("Can't attack because of global override.");
         cantAttack = true;
      }
      if (cantAttack == false && targetPlayer == -1)
      {
         debugMilitaryAttacking("Can't attack because we see no enemies.");
         cantAttack = true;
      }
      // We run the mostHatedEnemy rule less frequent than the titanManager. Things could've changed inbetween.
      if (cantAttack == false && targetPlayer != -1 && kbPlayerHasLost(targetPlayer) == true)
      {
         debugMilitaryAttacking("Can't attack because our target player has already lost: " + targetPlayer + ".");
         cantAttack = true;
      }
      if (cantAttack == false && targetPlayer != -1 && kbPlayerIsAlly(targetPlayer) == true)
      {
         debugMilitaryAttacking("Can't attack because our target player is an ally of ours: " + targetPlayer + ".");
         cantAttack = true;
      }
      if (cantAttack == true)
      {
         debugMilitaryAttacking("We can't attack with our Titans right now, let them all explore.");
         assignAllTitansToExplorePlans(queryID, numTitans);
         return;
      }
   }

   for (int i = 0; i < numTitans; i++)
   {
      int titanID = titans[i];
      int currentPlanID = kbUnitGetPlanID(titanID);
      bool needNewPlan = false;
      bool destroyOldPlan = false;
      if (currentPlanID == -1)
      {
         needNewPlan = true;
      }
      // If we're in a plan that is not meant to have a Titan in it we need a new plan, but don't destroy this plan.
      if (needNewPlan == false && aiPlanGetUserVariableIndex(currentPlanID, "Titan Plan") == -1)
      {
         needNewPlan = true;
      }
      // If we're currently in an explore plan (that has user variables) it's a Titan explore plan so it needs to be destroyed.
      if (needNewPlan == false && aiPlanGetType(currentPlanID) == cPlanExplore)
      {
         needNewPlan = true;
         destroyOldPlan = true;
      }
      // Clean up the Titan explore plan.
      if (destroyOldPlan == true)
      {
         aiPlanSetState(currentPlanID, cPlanStateDone);
      }
      // As a retaliator the Titan would be in a defend plan or child reinforcement plan instead of explore plan.
      if (cPersonalityCurrent == cPersonalityRetaliator && needNewPlan == false)
      {
         if ((aiPlanGetType(currentPlanID) == cPlanAttack && aiPlanGetParentID(currentPlanID) != -1) ||
              aiPlanGetType(currentPlanID) == cPlanDefend)
         {
            needNewPlan = true;
            aiPlanRemoveUnitFromAllPlans(titanID);
         }
      }
      if (needNewPlan == true)
      {
         int[] dummy = new int(0, 0);
         int targetBaseID = calculateTargetBase(dummy, targetPlayer, true);
         if (targetBaseID == -1)
         {
            debugMilitaryAttacking("Can't find a new base to attack for player " + targetPlayer + ", waiting on mostHatedEnemy now.");
            continue;
         }
         currentPlanID = createTitanAttackPlan(titanID, targetPlayer, targetBaseID);
         debugMilitaryAttacking("Created an attack plan for Titan " + titanID + ": " + aiPlanGetName(currentPlanID) + ".");
      }

      // TODO
      //int state = aiPlanGetUserVariableInt(currentPlanID, 0, 0);
      //switch (state)
      //{
      //   case cTitanStateNeedsTarget:
      //   {
      //      debugMilitaryAttacking("Titan " + titanID + " is in need of a new target.");
      //      // Clean up our route if we're getting a new one.
      //      int attackRouteID = aiPlanGetVariableInt(currentPlanID, cAttackPlanAttackRouteID, 0);
      //      if (kbAttackRouteGetIsIDValid(attackRouteID) == true)
      //      {
      //         kbAttackRouteDestroy(attackRouteID);
      //      }
      //      // Find new targets.
      //      break;
      //   }
      //   
      //   case cTitanStateWalking:
      //   {
      //      debugMilitaryAttacking("Titan " + titanID + " is walking towards its target.");
      //      break;
      //   }
//
      //   case cTitanStateAttackingUnit:
      //   {
      //      debugMilitaryAttacking("Titan " + titanID + " is attacking a unit.");
      //      break;
      //   }
//
      //   case cTitanStateAttackingBuildingThatShoot:
      //   {
      //      debugMilitaryAttacking("Titan " + titanID + " is attacking a building that shoots.");
      //      break;
      //   }
//
      //   case cTitanStateAttackingTownCenter:
      //   {
      //      debugMilitaryAttacking("Titan " + titanID + " is attacking a Town Center.");
      //      break;
      //   }
//
      //   case cTitanStateAttackingTitan:
      //   {
      //      debugMilitaryAttacking("Titan " + titanID + " is attacking another Titan.");
      //      break;
      //   }
//
      //   default:
      //   {
      //      aiEchoWarning("Unrecognized state found in titanManager: " + state + ".");
      //      break;
      //   }
      //}
   }
}