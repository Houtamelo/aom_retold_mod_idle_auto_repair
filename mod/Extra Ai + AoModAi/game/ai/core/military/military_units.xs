//==============================================================================
/* military_units.xs

   This file is intended for land/air military unit training.

*/
//==============================================================================

//==============================================================================
// greekHeroTraining
//==============================================================================
void greekHeroTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   int currentAge = kbPlayerGetAge(cMyID);
   int archaicHeroPUID = cUnitTypeJason;
   int classicalHeroPUID = cUnitTypeHeracles;
   int heroicHeroPUID = cUnitTypeOdysseus;
   int mythicHeroPUID = cUnitTypeBellerophon;
   if (cMyCiv == cCivHades)
   {
      archaicHeroPUID = cUnitTypeAjax;
      classicalHeroPUID = cUnitTypeAchilles;
      heroicHeroPUID = cUnitTypeChiron;
      mythicHeroPUID = cUnitTypePerseus;
   }
   else if (cMyCiv == cCivPoseidon)
   {
      archaicHeroPUID = cUnitTypeTheseus;
      classicalHeroPUID = cUnitTypeAtalanta;
      heroicHeroPUID = cUnitTypeHippolyta;
      mythicHeroPUID = cUnitTypePolyphemus;
   }
   else if (cMyCiv == cCivDemeter)
   {
      archaicHeroPUID = cUnitTypeOrpheus;
      classicalHeroPUID = cUnitTypeIolaus;
      heroicHeroPUID = cUnitTypeIcarus;
      mythicHeroPUID = cUnitTypeMidas;
   }
   int archaicPopCost = kbPlayerGetProtoStatInt(cMyID, archaicHeroPUID, cProtoStatPopCost);
   int classicalPopCost = kbPlayerGetProtoStatInt(cMyID, classicalHeroPUID, cProtoStatPopCost);
   int heroicPopCost = kbPlayerGetProtoStatInt(cMyID, heroicHeroPUID, cProtoStatPopCost);
   int mythicPopCost = kbPlayerGetProtoStatInt(cMyID, mythicHeroPUID, cProtoStatPopCost);
   bool alreadyHaveArchaic = false;
   bool alreadyHaveClassical = false; // Rule starts running in Classical, this will always be available.
   bool alreadyHaveHeroic = currentAge <= cAge2; // Not available yet.
   bool alreadyHaveMythic = currentAge <= cAge3; // Not available yet.

   int currentHeroPop = 0;
   if (aiPlanGetIDByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, archaicHeroPUID) != -1 ||
       kbUnitCount(archaicHeroPUID, cMyID) >= 1)
   {
      alreadyHaveArchaic = true;
      currentHeroPop += archaicPopCost;
   }
   if (aiPlanGetIDByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, classicalHeroPUID) != -1 ||
       kbUnitCount(classicalHeroPUID, cMyID) >= 1)
   {
      alreadyHaveClassical = true;
      currentHeroPop += classicalPopCost;
   }
   if (alreadyHaveHeroic == false && aiPlanGetIDByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, heroicHeroPUID) != -1 ||
       kbUnitCount(heroicHeroPUID, cMyID) >= 1)
   {
      alreadyHaveHeroic = true;
      currentHeroPop += heroicPopCost;
   }
   if (alreadyHaveMythic == false && aiPlanGetIDByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, mythicHeroPUID) != -1 ||
       kbUnitCount(mythicHeroPUID, cMyID) >= 1)
   {
      alreadyHaveMythic = true;
      currentHeroPop += mythicPopCost;
   }
   if (alreadyHaveArchaic == true && alreadyHaveClassical == true && alreadyHaveHeroic == true && alreadyHaveMythic == true)
   {
      totalAvailablePop -= currentHeroPop;
      debugMilitaryTraining("Skipping greekHeroTraining because we already have or are training all the available heroes.");
      return;
   }

   int availableHeroPop = originalTotalMilitaryPop * gArmyHeroPercentage;
   int minimumHeroPop = originalTotalMilitaryPop * (gArmyHeroPercentage / 2);
   debugMilitaryTraining("availableHeroPop is: " + availableHeroPop + ", minimumHeroPop is: " + minimumHeroPop + ".");
   debugMilitaryTraining("We're starting with currentHeroPop: " + currentHeroPop + ".");
   if (availableHeroPop <= currentHeroPop)
   {
      totalAvailablePop -= currentHeroPop;
      debugMilitaryTraining("We already have currentHeroPop, skipping. availableHeroPop: " + availableHeroPop + " currentHeroPop: "
         + currentHeroPop + ".");
      return;
   }

   int targetPlayer = aiGetMostHatedPlayerID();
   if (targetPlayer > 0)
   {
      int enemyMythPop = kbGetPopulationSlotsByUnitTypeID(cUnitTypeMythUnit, targetPlayer);
      debugMilitaryTraining("Scouted a total of enemyMythPop: " + enemyMythPop + ".");
      // We match enemy myth pop or cap at our original.
      availableHeroPop = min(availableHeroPop, enemyMythPop);
      // If our matching of the enemy pop turned out to be a really low number we use our minimum instead.
      if (availableHeroPop < minimumHeroPop)
      {
         availableHeroPop = minimumHeroPop;
         debugMilitaryTraining("Using minimumHeroPop because we haven't scouted enough enemy myth units to warrant many heroes.");
      }
      else
      {
         debugMilitaryTraining("Adjusting our availableHeroPop to: " + availableHeroPop + ", because of our scouting reports.");
      }
   }

   if (availableHeroPop <= currentHeroPop)
   {
      totalAvailablePop -= currentHeroPop;
      debugMilitaryTraining("We already have enough currentHeroPop, skipping. availableHeroPop: " + availableHeroPop + 
         " currentHeroPop: " + currentHeroPop + ".");
      return;
   }

   int heroPopToTrain = availableHeroPop - currentHeroPop;
   debugMilitaryTraining("We're going to train heroes for heroPopToTrain: " + heroPopToTrain + 
      ". Calculation: availableHeroPop(" + availableHeroPop + ") - currentHeroPop(" + currentHeroPop + ").");
   int rand = xsRandInt(0, 3);
   int oldHeroPopToTrain = heroPopToTrain;
   for (int i = 0; i < 4; i++)
   {
      switch((i + rand) % 4)
      {
         case 0:
         {
            if (alreadyHaveArchaic == false)
            {
               createSimpleTrainPlan(archaicHeroPUID, 1, gLandAreaGroupID, gMilitaryTrainingCategoryID);
               heroPopToTrain -= archaicPopCost;
               currentHeroPop += archaicPopCost;
            }
            break;
         }
         case 1:
         {
            if (alreadyHaveClassical == false)
            {
               createSimpleTrainPlan(classicalHeroPUID, 1, gLandAreaGroupID, gMilitaryTrainingCategoryID);
               heroPopToTrain -= classicalPopCost;
               currentHeroPop += classicalPopCost;
            }
            break;
         }
         case 2:
         {
            if (alreadyHaveHeroic == false)
            {
               createSimpleTrainPlan(heroicHeroPUID, 1, gLandAreaGroupID, gMilitaryTrainingCategoryID);
               heroPopToTrain -= heroicPopCost;
               currentHeroPop += heroicPopCost;
            }
            break;
         }
         case 3:
         {
            if (alreadyHaveMythic == false)
            {
               createSimpleTrainPlan(mythicHeroPUID, 1, gLandAreaGroupID, gMilitaryTrainingCategoryID);
               heroPopToTrain -= mythicPopCost;
               currentHeroPop += mythicPopCost;
            }
            break;
         }
      }
      if (heroPopToTrain <= 0)
      {
         totalAvailablePop -= currentHeroPop;
         return;
      }
   }
   if (oldHeroPopToTrain == heroPopToTrain)
   {
      debugMilitaryTraining("We couldn't create a train plan for any hero, maybe we already have all that are available to us.");
   }
   totalAvailablePop -= currentHeroPop;
}

//==============================================================================
// egyptianHeroTraining
//==============================================================================
void egyptianHeroTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   int planID = gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex];
   if (aiPlanGetIsIDValid(planID) == false)
   {
      planID = createSimpleMaintainPlan(cUnitTypePriest, 0, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
      gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex] = planID;
   }
   
   int availableHeroPop = originalTotalMilitaryPop * gArmyHeroPercentage;
   int minimumHeroPop = originalTotalMilitaryPop * (gArmyHeroPercentage / 2);
   debugMilitaryTraining("availableHeroPop is: " + availableHeroPop + ", minimumHeroPop is: " + minimumHeroPop + ".");

   int targetPlayer = aiGetMostHatedPlayerID();
   if (targetPlayer > 0)
   {
      int enemyMythPop = kbGetPopulationSlotsByUnitTypeID(cUnitTypeMythUnit, targetPlayer);
      debugMilitaryTraining("Scouted a total of enemyMythPop: " + enemyMythPop + ".");
      // We match enemy myth pop or cap at our original.
      availableHeroPop = min(availableHeroPop, enemyMythPop);
      // If our matching of the enemy pop turned out to be a really low number we use our minimum instead.
      if (availableHeroPop < minimumHeroPop)
      {
         availableHeroPop = minimumHeroPop;
         debugMilitaryTraining("Using minimumHeroPop because we haven't scouted enough enemy myth units to warrant many heroes.");
      }
      else
      {
         debugMilitaryTraining("Adjusting our availableHeroPop to: " + availableHeroPop + ", because of our scouting reports.");
      }
   }

   int priestPopCost = kbPlayerGetProtoStatInt(cMyID, cUnitTypePriest, cProtoStatPopCost);
   int numberToMaintain = ceil(xsIntToFloat(availableHeroPop) / xsIntToFloat(priestPopCost));
   // It could be that this plan is already perfectly set up, then don't do anything.
   if (aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) == numberToMaintain)
   {
      debugMilitaryTraining("Maintain plan for " + numberToMaintain + " Priest doesn't require any changes.");
   }
   else
   {
      aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, numberToMaintain);
      aiPlanSetName(planID, planID + ": Hero maintain: " + numberToMaintain + " Priest");
      debugMilitaryTraining("Adjusting maintain plan for Priest to maintain " + numberToMaintain + ".");
   }
   totalAvailablePop -= numberToMaintain * priestPopCost;
}

//==============================================================================
// norseHeroTraining
//==============================================================================
void norseHeroTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   int hersirPlanID = gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex];
   if (aiPlanGetIsIDValid(hersirPlanID) == false)
   {
      hersirPlanID = createSimpleMaintainPlan(cUnitTypeHersir, 0, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
      gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex] = hersirPlanID;
   }

   int godiPlanID = gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex + 1];
   bool godiEnabled = false;
   if (kbProtoUnitAvailable(cUnitTypeGodi) == true)
   {
      if (aiPlanGetIsIDValid(godiPlanID) == false)
      {
         godiPlanID = createSimpleMaintainPlan(cUnitTypeGodi, 0, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
      }
      gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex + 1] = godiPlanID;
      godiEnabled = true;
   }

   int availableHeroPop = originalTotalMilitaryPop * gArmyHeroPercentage;
   int minimumHeroPop = originalTotalMilitaryPop * (gArmyHeroPercentage / 2);
   debugMilitaryTraining("availableHeroPop is: " + availableHeroPop + ", minimumHeroPop is: " + minimumHeroPop + ".");

   int targetPlayer = aiGetMostHatedPlayerID();
   if (targetPlayer > 0)
   {
      int enemyMythPop = kbGetPopulationSlotsByUnitTypeID(cUnitTypeMythUnit, targetPlayer);
      debugMilitaryTraining("Scouted a total of enemyMythPop: " + enemyMythPop + ".");
      // We match enemy myth pop or cap at our original.
      availableHeroPop = min(availableHeroPop, enemyMythPop);
      // If our matching of the enemy pop turned out to be a really low number we use our minimum instead.
      if (availableHeroPop < minimumHeroPop)
      {
         availableHeroPop = minimumHeroPop;
         debugMilitaryTraining("Using minimumHeroPop because we haven't scouted enough enemy myth units to warrant many heroes.");
      }
      else
      {
         debugMilitaryTraining("Adjusting our availableHeroPop to: " + availableHeroPop + ", because of our scouting reports.");
      }
   }

   int numMilitaryBuildings = gMilitaryBuildings.size();
   int hersirPopCost = kbPlayerGetProtoStatInt(cMyID, cUnitTypeHersir, cProtoStatPopCost);
   int godiPopCost = kbPlayerGetProtoStatInt(cMyID, cUnitTypeGodi, cProtoStatPopCost);

   if (godiEnabled == false)
   {
      int numberToMaintain = ceil(xsIntToFloat(availableHeroPop) / xsIntToFloat(hersirPopCost));
      // It could be that this plan is already perfectly set up, then don't do anything.
      if (aiPlanGetVariableInt(hersirPlanID, cTrainPlanNumberToMaintain, 0) == numberToMaintain)
      {
         debugMilitaryTraining("Maintain plan for " + numberToMaintain + " Hersir doesn't require any changes.");
      }
      else
      {
         aiPlanSetVariableInt(hersirPlanID, cTrainPlanNumberToMaintain, 0, numberToMaintain);
         aiPlanSetName(hersirPlanID, hersirPlanID + ": Hero maintain: " + numberToMaintain + " Hersir");
         debugMilitaryTraining("Adjusting maintain plan for Hersir to maintain " + numberToMaintain + ".");
      }
      for (int j = 0; j < numMilitaryBuildings; j++)
      {
         if (kbProtoUnitCanTrain(gMilitaryBuildings[j], cUnitTypeHersir) == true)
         {
            gArmyUnitBuildings[gMaintainPlanHeroStartIndex] = gMilitaryBuildings[j];
            break;
         }
      }
      totalAvailablePop -= numberToMaintain * hersirPopCost;
      return;
   }
   else
   {
      int maintainHersirAmount = 0;
      int maintainGodiAmount = 0;
      while (availableHeroPop > 0)
      {
         int rand = xsRandInt(0, 2);
         if (rand == 0)
         {
            maintainHersirAmount++;
            availableHeroPop -= hersirPopCost;
         }
         else if (rand == 1)
         {
            if (cMyCiv == cCivLoki)
            {
               maintainHersirAmount++;
               availableHeroPop -= hersirPopCost;
            }
            else
            {
               maintainGodiAmount++;
               availableHeroPop -= godiPopCost;
            }
         }
         else
         {
            maintainGodiAmount++;
            availableHeroPop -= godiPopCost;
         }
      }

      // It could be that this plan is already perfectly set up, then don't do anything.
      if (aiPlanGetVariableInt(hersirPlanID, cTrainPlanNumberToMaintain, 0) == maintainHersirAmount)
      {
         debugMilitaryTraining("Maintain plan for " + maintainHersirAmount + " Hersir doesn't require any changes.");
      }
      else
      {
         aiPlanSetVariableInt(hersirPlanID, cTrainPlanNumberToMaintain, 0, maintainHersirAmount);
         aiPlanSetName(hersirPlanID, hersirPlanID + ": Hero maintain: " + maintainHersirAmount + " Hersir");
         debugMilitaryTraining("Adjusting maintain plan for Hersir to maintain " + maintainHersirAmount + ".");
      }
      for (int j = 0; j < numMilitaryBuildings; j++)
      {
         if (kbProtoUnitCanTrain(gMilitaryBuildings[j], cUnitTypeHersir) == true)
         {
            gArmyUnitBuildings[gMaintainPlanHeroStartIndex] = gMilitaryBuildings[j];
            break;
         }
      }

      // It could be that this plan is already perfectly set up, then don't do anything.
      if (aiPlanGetVariableInt(godiPlanID, cTrainPlanNumberToMaintain, 0) == maintainGodiAmount)
      {
         debugMilitaryTraining("Maintain plan for " + maintainGodiAmount + " Godi doesn't require any changes.");
      }
      else
      {
         aiPlanSetVariableInt(godiPlanID, cTrainPlanNumberToMaintain, 0, maintainGodiAmount);
         aiPlanSetName(godiPlanID, godiPlanID + ": Hero maintain: " + maintainGodiAmount + " Godi");
         debugMilitaryTraining("Adjusting maintain plan for Godi to maintain " + maintainGodiAmount + ".");
      }
      for (int j = 0; j < numMilitaryBuildings; j++)
      {
         if (kbProtoUnitCanTrain(gMilitaryBuildings[j], cUnitTypeGodi) == true)
         {
            gArmyUnitBuildings[gMaintainPlanHeroStartIndex + 1] = gMilitaryBuildings[j];
            break;
         }
      }

      totalAvailablePop -= maintainHersirAmount * hersirPopCost;
      totalAvailablePop -= maintainGodiAmount * godiPopCost;
   }
}

//==============================================================================
// atlanteanHeroTraining
//==============================================================================
void atlanteanHeroTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   int murmilloHeroPop = kbPlayerGetProtoStatInt(cMyID, cUnitTypeMurmillo, cProtoStatPopCost);
   int contariusHeroPop = kbPlayerGetProtoStatInt(cMyID, cUnitTypeContarius, cProtoStatPopCost);
   int katapeltesHeroPop = kbPlayerGetProtoStatInt(cMyID, cUnitTypeKatapeltes, cProtoStatPopCost);
   int destroyerHeroPop = kbPlayerGetProtoStatInt(cMyID, cUnitTypeDestroyer, cProtoStatPopCost);
   int fanaticHeroPop = kbPlayerGetProtoStatInt(cMyID, cUnitTypeFanatic, cProtoStatPopCost);
   int arcusHeroPop = kbPlayerGetProtoStatInt(cMyID, cUnitTypeArcus, cProtoStatPopCost);
   int turmaHeroPop = kbPlayerGetProtoStatInt(cMyID, cUnitTypeTurma, cProtoStatPopCost);
   int cheiroballistaHeroPop = kbPlayerGetProtoStatInt(cMyID, cUnitTypeCheiroballista, cProtoStatPopCost);

   int queryID = useSimpleUnitQuery(cUnitTypeHero);
   kbUnitQueryExecute(queryID);
   int currentHeroPop = kbUnitQueryGetPopulationSlots(queryID);
   int numPlans = 0;

   // Tally up all the hero research plans we've got going on.
   numPlans = aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechMurmilloToHero);
   currentHeroPop += numPlans * murmilloHeroPop;
   numPlans = aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechContariusToHero);
   currentHeroPop += numPlans * contariusHeroPop;
   numPlans = aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechKatapeltesToHero);
   currentHeroPop += numPlans * katapeltesHeroPop;
   numPlans = aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechDestroyerToHero);
   currentHeroPop += numPlans * destroyerHeroPop;
   numPlans = aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechFanaticToHero);
   currentHeroPop += numPlans * fanaticHeroPop;
   numPlans = aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechArcusToHero);
   currentHeroPop += numPlans * arcusHeroPop;
   numPlans = aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechTurmaToHero);
   currentHeroPop += numPlans * turmaHeroPop;
   numPlans = aiPlanGetNumberByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechCheiroballistaToHero);
   currentHeroPop += numPlans * cheiroballistaHeroPop;

   // If we're too close to attacking we don't do anything because we only want to heroize idle units in our gPrimaryLandDefendPlan.
   // If we can't attack due to strategy flags we don't check this because the attack time may be unset.
   // And also don't do this if we're in need of scouting, because then our attack time also never gets reset.
   if (checkStrategyFlag(cStrategyFlagCanAttack) == true &&
       gAttackManager.mState != cStateNeedScouting &&
       (xsGetTime() + 30 > gAttackManager.mLastAttackTime + gAttackManager.mAttackInterval))
   {
      debugMilitaryTraining("Quiting atlanteanHeroTraining early because we're too close to launching an attack.");
      totalAvailablePop -= currentHeroPop;
      return; 
   }

   int availableHeroPop = 3 * originalTotalMilitaryPop * gArmyHeroPercentage;
   int minimumHeroPop = 3 * originalTotalMilitaryPop * (gArmyHeroPercentage / 2);
   debugMilitaryTraining("availableHeroPop is: " + availableHeroPop + ", minimumHeroPop is: " + minimumHeroPop + ".");

   if (availableHeroPop <= currentHeroPop)
   {
      totalAvailablePop -= currentHeroPop;
      debugMilitaryTraining("We already have enough currentHeroPop, skipping. availableHeroPop: " + availableHeroPop + 
         " currentHeroPop: " + currentHeroPop + ".");
      return;
   }

   int targetPlayer = aiGetMostHatedPlayerID();
   if (targetPlayer > 0)
   {
      int enemyMythPop = kbGetPopulationSlotsByUnitTypeID(cUnitTypeMythUnit, targetPlayer);
      debugMilitaryTraining("Scouted a total of enemyMythPop: " + enemyMythPop + ".");
      // We match enemy myth pop or cap at our original.
      availableHeroPop = min(availableHeroPop, enemyMythPop);
      // If our matching of the enemy pop turned out to be a really low number we use our minimum instead.
      if (availableHeroPop < minimumHeroPop)
      {
         availableHeroPop = minimumHeroPop;
         debugMilitaryTraining("Using minimumHeroPop because we haven't scouted enough enemy myth units to warrant many heroes.");
      }
      else
      {
         debugMilitaryTraining("Adjusting our availableHeroPop to: " + availableHeroPop + ", because of our scouting reports.");
      }
   }

   if (availableHeroPop <= currentHeroPop)
   {
      totalAvailablePop -= currentHeroPop;
      debugMilitaryTraining("We already have enough currentHeroPop, skipping. availableHeroPop: " + availableHeroPop + 
         " currentHeroPop: " + currentHeroPop + ".");
      return;
   }

   int heroPopToTrain = availableHeroPop - currentHeroPop;
   debugMilitaryTraining("We're going to train heroes for heroPopToTrain: " + heroPopToTrain + 
      ". Calculation: availableHeroPop(" + availableHeroPop + ") - currentHeroPop(" + currentHeroPop + ").");

   int[] reserveUnits = aiPlanGetUnits(gPrimaryLandDefendPlan, cUnitTypeHumanSoldier);
   int[] validMeleeUnits = new int(0, -1);
   int[] validRangedUnits = new int(0, -1);
   for (int i = 0; i < reserveUnits.size(); i++)
   {
      int unitID = reserveUnits[i];
      // Only want to heroize 100% HP units.
      if (kbUnitGetStatFloat(unitID, cUnitStatHPRatio) < 1.0)
      {
         continue;
      }
      // Don't pick a unit we already have a plan for.
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanResearcherID, unitID) >= 0)
      {
         continue;
      }
      // Don't pick a unit we're going to use Valor on soon.
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanGodPower, cGodPowerPlanTargetUnit, unitID) >= 0)
      {
         continue;
      }
      // Don't pick units that are already being heroized, this can happen outside of a plan via Valor.
      if (kbUnitGetActionType(unitID) == cActionTypeCommandResearch)
      {
         continue;
      }
      int puid = kbUnitGetProtoUnitID(unitID);
      bool valid = false;
      bool melee = true;
      if (puid == cUnitTypeMurmillo || puid == cUnitTypeContarius || 
          puid == cUnitTypeKatapeltes ||  puid == cUnitTypeDestroyer || puid == cUnitTypeFanatic)
      {
         valid = true;
      }
      else if (puid == cUnitTypeArcus || puid == cUnitTypeTurma || puid == cUnitTypeCheiroballista)
      {
         valid = true;
         melee = false;
      }
      if (valid == true)
      {
         // Only if there are no enemies within range of this unit do we start the heroization, otherwise we may die during animation.
         // EXCEPTION: if we have 0 currentHeroPop we always let a unit go through so we always get at least 1 hero.
         if (currentHeroPop == 0 || getUnitCountByLocation(cUnitTypeMilitaryUnit, cPlayerRelationEnemyNotGaia, cUnitStateAlive,
             kbUnitGetPosition(unitID), 20.0, cUnitQueryVisibleStateVisible) == 0)
         {
            if (melee == true)
            {
               validMeleeUnits.add(unitID);
            }
            else
            {
               validRangedUnits.add(unitID);
            }
         }
         else
         {
            debugMilitaryTraining("Can't heroize " + unitID + " because there are enemies close.");
         }
      }
   }
   int numMeleeUnits = validMeleeUnits.size();
   int numRangedUnits = validRangedUnits.size();
   int totalUnits = numMeleeUnits + numRangedUnits;
   if (totalUnits <= 0)
   {
      totalAvailablePop -= currentHeroPop;
      debugMilitaryTraining("Quiting atlanteanHeroTraining early because there are no units in our defend plan that we can heroize.");
      return;
   }
   int meleeHeroesMade = 0;
   int rangedHeroesMade = 0;
   int unitsProcessed = 0;
   while (heroPopToTrain > 0)
   {
      static int i = 0;
      if (i % 2 == 0)
      {
         if (meleeHeroesMade < numMeleeUnits)
         {
            int prio = 50;
            if (meleeHeroesMade == 0)
            {
               prio = 51; // Put some emphasis on the first hero each run.
            }
            int unitID = validMeleeUnits[meleeHeroesMade];
            int puid = kbUnitGetProtoUnitID(unitID);
            switch (puid)
            {
               case cUnitTypeMurmillo:
               {
                  createSimpleResearchPlanSpecificResearcher(cTechMurmilloToHero, unitID, prio, true);
                  currentHeroPop += murmilloHeroPop;
                  heroPopToTrain -= murmilloHeroPop;
                  break;
               }
               case cUnitTypeContarius:
               {
                  createSimpleResearchPlanSpecificResearcher(cTechContariusToHero, unitID, prio, true);
                  currentHeroPop += contariusHeroPop;
                  heroPopToTrain -= contariusHeroPop;
                  break;
               }
               case cUnitTypeKatapeltes:
               {
                  createSimpleResearchPlanSpecificResearcher(cTechKatapeltesToHero, unitID, prio, true);
                  currentHeroPop += katapeltesHeroPop;
                  heroPopToTrain -= katapeltesHeroPop;
                  break;
               }
               case cUnitTypeDestroyer:
               {
                  createSimpleResearchPlanSpecificResearcher(cTechDestroyerToHero, unitID, prio, true);
                  currentHeroPop += destroyerHeroPop;
                  heroPopToTrain -= destroyerHeroPop;
                  break;
               }
               case cUnitTypeFanatic:
               {
                  createSimpleResearchPlanSpecificResearcher(cTechFanaticToHero, unitID, prio, true);
                  currentHeroPop += fanaticHeroPop;
                  heroPopToTrain -= fanaticHeroPop;
                  break;
               }
               default:
               {
                  aiEchoWarning("Bug in what melee units enter validMeleeUnits, this one is " + kbProtoUnitGetName(puid) + ".");
                  break;
               }
            }
            meleeHeroesMade++;
         }
      }
      else
      {
         if (rangedHeroesMade < numRangedUnits)
         {
            int prio = 50;
            if (rangedHeroesMade == 0)
            {
               prio = 51; // Put some emphasis on the first hero each run.
            }
            int unitID = validRangedUnits[rangedHeroesMade];
            int puid = kbUnitGetProtoUnitID(unitID);
            switch (puid)
            {
               case cUnitTypeArcus:
               {
                  createSimpleResearchPlanSpecificResearcher(cTechArcusToHero, unitID, prio, true);
                  currentHeroPop += arcusHeroPop;
                  heroPopToTrain -= arcusHeroPop;
                  break;
               }
               case cUnitTypeTurma:
               {
                  createSimpleResearchPlanSpecificResearcher(cTechTurmaToHero, unitID, prio, true);
                  currentHeroPop += turmaHeroPop;
                  heroPopToTrain -= turmaHeroPop;
                  break;
               }
               case cUnitTypeCheiroballista:
               {
                  createSimpleResearchPlanSpecificResearcher(cTechCheiroballistaToHero, unitID, prio, true);
                  currentHeroPop += cheiroballistaHeroPop;
                  heroPopToTrain -= cheiroballistaHeroPop;
                  break;
               }
               default:
               {
                  aiEchoWarning("Bug in what melee units enter validRangedUnits, this one is " + kbProtoUnitGetName(puid) + ".");
                  break;
               }
            }
            rangedHeroesMade++;
         }
      }
      i++;

      unitsProcessed++;
      if (unitsProcessed == totalUnits && heroPopToTrain > 0)
      {
         debugMilitaryTraining("We don't have enough units to heroize to fully satisfy our needs.");
         break;
      }
   }
   // Finally we're done.
   totalAvailablePop -= currentHeroPop;
}

//==============================================================================
// chineseHeroTraining
//==============================================================================
void chineseHeroTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   int currentAge = kbPlayerGetAge(cMyID);
   int numPioneersToMaintain = 0;
   bool noHeroPopToTrain = false;
   int pioneerPopCost = kbPlayerGetProtoStatInt(cMyID, cUnitTypePioneer, cProtoStatPopCost);
   int availableHeroPop = originalTotalMilitaryPop * gArmyHeroPercentage;
   int minimumHeroPop = originalTotalMilitaryPop * (gArmyHeroPercentage / 2);

   // First we have to figure out how many heroes we already have. Which is a bit cumbersome with the train/maintain plan combi.
   int[] availableHeroes = new int(0, -1);
   if (currentAge >= cAge3)
   {
      availableHeroes.add(cUnitTypeSage);
      availableHeroes.add(cUnitTypeJiangZiYa);
      if (currentAge == cAge3)
      {
         if (cMyCiv == cCivFuxi)
         {
            availableHeroes.add(cUnitTypeNezhaYouth);
         }
      }
      else // Mythic/Wonder.
      {
         if (cMyCiv == cCivShennong)
         {
            availableHeroes.add(cUnitTypeWenZhong);
         }
         else if (cMyCiv == cCivNuwa)
         {
            availableHeroes.add(cUnitTypeLiJing);
         }
         else
         {
            availableHeroes.add(cUnitTypeYangJian);
            availableHeroes.add(cUnitTypeNezha);
         }
      }
   }

   int currentHeroPop = kbGetPopulationSlotsByUnitTypeID(cUnitTypeHero, cMyID, cUnitStateAlive);
   for (int i = 0; i < availableHeroes.size(); i++)
   {
      int puid = availableHeroes[i];
      int popCost = kbPlayerGetProtoStatInt(cMyID, puid, cProtoStatPopCost);
      int numPlans = aiPlanGetNumberByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, puid);
      // We only train 1 unit per plan for this logic, so this simple multiplication works.
      currentHeroPop += popCost * numPlans;
   }

   int queuedPioneerPop = kbGetPopulationSlotsByUnitTypeID(cUnitTypePioneer, cMyID, cUnitStateQueued);
   // Pioneers are not part of the availableHeroes array, so take them into account here.
   currentHeroPop += queuedPioneerPop;
   int totalPioneerPop = queuedPioneerPop + kbGetPopulationSlotsByUnitTypeID(cUnitTypePioneer, cMyID, cUnitStateAlive);

   // We also want one without Pioneers, so we know how much space is left for them since they have no BL.
   int currentHeroPopWithoutPioneers = currentHeroPop - totalPioneerPop;

   debugMilitaryTraining("availableHeroPop is: " + availableHeroPop + ", minimumHeroPop is: " + minimumHeroPop + ".");
   debugMilitaryTraining("We're starting with currentHeroPop: " + currentHeroPop + ", and currentHeroPopWithoutPioneers: " +
      currentHeroPopWithoutPioneers + ", which means we have totalPioneerPop: " + totalPioneerPop + ".");

   int targetPlayer = aiGetMostHatedPlayerID();
   if (targetPlayer > 0)
   {
      int enemyMythPop = kbGetPopulationSlotsByUnitTypeID(cUnitTypeMythUnit, targetPlayer);
      debugMilitaryTraining("Scouted a total of enemyMythPop: " + enemyMythPop + ".");
      // We match enemy myth pop or cap at our original.
      availableHeroPop = min(availableHeroPop, enemyMythPop);
      // If our matching of the enemy pop turned out to be a really low number we use our minimum instead.
      if (availableHeroPop < minimumHeroPop)
      {
         availableHeroPop = minimumHeroPop;
         debugMilitaryTraining("Using minimumHeroPop because we haven't scouted enough enemy myth units to warrant many heroes.");
      }
      else
      {
         debugMilitaryTraining("Adjusting our availableHeroPop to: " + availableHeroPop + ", because of our scouting reports.");
      }
   }

   int heroPopToTrain = availableHeroPop - currentHeroPop;
   debugMilitaryTraining("We're going to train heroes for heroPopToTrain: " + heroPopToTrain + 
      ". Calculation: availableHeroPop(" + availableHeroPop + ") - currentHeroPop(" + currentHeroPop + ").");
   // If previous run we wanted to maintain 100 Pioneers and now we already have enough heroes it doesn't mean that our Pioneer
   // maintain plan should remain at 100. Maybe our eco took a big hit and now we only want 80. So we can't early out.
   if (heroPopToTrain <= 0)
   {
      debugMilitaryTraining("We already have enough currentHeroPop. Skipping the training of new heroes, must still adjust " +
         "Pioneer maintain plan though.");
      int availablePioneerPop = max(0, availableHeroPop - currentHeroPopWithoutPioneers);
      if (availablePioneerPop > 0)
      {
         numPioneersToMaintain = ceil(xsIntToFloat(availablePioneerPop) / xsIntToFloat(pioneerPopCost));
      }
      debugMilitaryTraining("We have this much pop available for Pioneers: " + availablePioneerPop +
         " (availableHeroPop - currentHeroPopWithoutPioneers), this leaves room for numPioneersToMaintain: " +
         numPioneersToMaintain + ".");
      noHeroPopToTrain = true;
   }

   // If we're in Heroic we can train some unique heroes with 1 BL.
   // No Nezha in Classical to simplify this stuff.
   if (heroPopToTrain > 0 && currentAge >= cAge3)
   {
      bool trainedUniqueHero = false;
      // Just randomly train 2 of these if it's possible. Yes we can pick the same index twice...
      for (int i = 0; i < 2; i++)
      {
         int rand = xsRandInt(0, availableHeroes.size() - 1);
         int puid = availableHeroes[rand];
         int numAlive = kbUnitCount(puid, cMyID, cUnitStateAlive);
         int numPlanned = aiPlanGetNumberByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, puid);
         int buildLimit = kbPlayerGetProtoStatInt(cMyID, puid, cProtoStatBuildLimit);
         if (numAlive + numPlanned > buildLimit)
         {
            debugMilitaryTraining("Not making another train plan for " + kbUnitTypeGetName(puid) + " because we're at our BL.");
            continue;
         }
         int popCost = kbPlayerGetProtoStatInt(cMyID, puid, cProtoStatPopCost);
         createSimpleTrainPlan(puid, 1, gLandAreaGroupID, gMilitaryTrainingCategoryID);
         currentHeroPop += popCost;
         currentHeroPopWithoutPioneers += popCost;
         heroPopToTrain -= popCost;
         trainedUniqueHero = true;
         if (heroPopToTrain <= 0)
         {
            break;
         }
      }
      if (trainedUniqueHero == false)
      {
         debugMilitaryTraining("We didn't create a plan for a limited Hero, instead just maintaining more Pioneers.");
      }
   }
   else if (heroPopToTrain > 0 && currentAge < cAge3)
   {
      debugMilitaryTraining("We have population room for more heroes but aren't in Heroic yet, can only maintain more Pioneers now.");
   }

   // If noHeroPopToTrain is true we already have our amount to train.
   if (noHeroPopToTrain == false)
   {
      // If we're here we know that we should at least maintain the amount of Pioneers we currently have.
      // This number was already incorporated into currentHeroPop above, so don't need to add to it again.
      int currentPioneerCount = kbUnitCount(cUnitTypePioneer, cMyID, cUnitStateABQ);
      debugMilitaryTraining("We have " + currentPioneerCount + " Pioneers already alive/queued, we will keep maintaining those " +
         "at the least.");
      numPioneersToMaintain = currentPioneerCount;
      // If we still have heroPopToTrain left we can add that to our existing number.
      if (heroPopToTrain > 0)
      {
         int extraPioneers = ceil(xsIntToFloat(heroPopToTrain) / xsIntToFloat(pioneerPopCost));
         debugMilitaryTraining("We have " + heroPopToTrain + " pop left for Pioneers. Which adds " + extraPioneers + " Pioneers.");
         numPioneersToMaintain += extraPioneers;
         currentHeroPop += extraPioneers * pioneerPopCost;
      }
      else
      {
         debugMilitaryTraining("No heroPopToTrain left to train maintain more Pioneers.");
      }
   }

   int pioneerPlanID = gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex];
   if (aiPlanGetIsIDValid(pioneerPlanID) == false)
   {
      pioneerPlanID = createSimpleMaintainPlan(cUnitTypePioneer, numPioneersToMaintain, gLandAreaGroupID,
         gMilitaryTrainingCategoryID, 50, -1, -1, true);
      gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex] = pioneerPlanID;
   }
   // It could be that this plan is already perfectly set up, then don't do anything.
   else if (aiPlanGetVariableInt(pioneerPlanID, cTrainPlanNumberToMaintain, 0) == numPioneersToMaintain)
   {
      debugMilitaryTraining("Maintain plan for " + numPioneersToMaintain + " Pioneers doesn't require any changes.");
   }
   else
   {
      aiPlanSetVariableInt(pioneerPlanID, cTrainPlanNumberToMaintain, 0, numPioneersToMaintain);
      aiPlanSetName(pioneerPlanID, pioneerPlanID + ": Hero maintain: " + numPioneersToMaintain + " Pioneer");
      debugMilitaryTraining("Adjusting maintain plan for Pioneers to maintain " + numPioneersToMaintain + ".");
   }

   for (int i = 0; i < gMilitaryBuildings.size(); i++)
   {
      if (kbProtoUnitCanTrain(gMilitaryBuildings[i], cUnitTypePioneer) == true)
      {
         gArmyUnitBuildings[gMaintainPlanHeroStartIndex] = gMilitaryBuildings[i];
         break;
      }
   }

   totalAvailablePop -= currentHeroPop;
}

//==============================================================================
// japaneseHeroTraining
//==============================================================================
void japaneseHeroTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   int bushiPlanID = gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex];
   if (aiPlanGetIsIDValid(bushiPlanID) == false)
   {
      bushiPlanID = createSimpleMaintainPlan(cUnitTypeBushi, 0, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
      gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex] = bushiPlanID;
   }

   int onnaMushaPlanID = gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex + 1];
   if (aiPlanGetIsIDValid(onnaMushaPlanID) == false)
   {
      onnaMushaPlanID = createSimpleMaintainPlan(cUnitTypeOnnaMusha, 0, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
   }
   gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex + 1] = onnaMushaPlanID;

   int daimyoPlanID = gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex + 2];
   bool daimyoEnabled = false;
   if (kbProtoUnitAvailable(cUnitTypeDaimyo) == true)
   {
      if (aiPlanGetIsIDValid(daimyoPlanID) == false)
      {
         daimyoPlanID = createSimpleMaintainPlan(cUnitTypeDaimyo, 0, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
      }
      gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex + 2] = daimyoPlanID;
      daimyoEnabled = true;
   }

   int onmyojiPlanID = gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex + 3];
   bool onmyojiEnabled = false;
   if (kbProtoUnitAvailable(cUnitTypeOnmyoji) == true)
   {
      if (aiPlanGetIsIDValid(onmyojiPlanID) == false)
      {
         onmyojiPlanID = createSimpleMaintainPlan(cUnitTypeOnmyoji, 0, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
      }
      gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex + 3] = onmyojiPlanID;
      onmyojiEnabled = true;
   }

   int availableHeroPop = originalTotalMilitaryPop * gArmyHeroPercentage;
   int minimumHeroPop = originalTotalMilitaryPop * (gArmyHeroPercentage / 2);
   debugMilitaryTraining("availableHeroPop is: " + availableHeroPop + ", minimumHeroPop is: " + minimumHeroPop + ".");

   int targetPlayer = aiGetMostHatedPlayerID();
   if (targetPlayer > 0)
   {
      int queryID = useSimpleUnitQuery(cUnitTypeMythUnit, targetPlayer);
      kbUnitQueryExecute(queryID);
      int enemyMythPop = kbUnitQueryGetPopulationSlots(queryID);
      debugMilitaryTraining("Scouted a total of enemyMythPop: " + enemyMythPop);
      // We match enemy myth pop or cap at our original.
      availableHeroPop = min(availableHeroPop, enemyMythPop);
      // If our matching of the enemy pop turned out to be a really low number we use our minimum instead.
      if (availableHeroPop < minimumHeroPop)
      {
         availableHeroPop = minimumHeroPop;
         debugMilitaryTraining("Using minimumHeroPop because we haven't scouted enough enemy myth units to warrant many heroes.");
      }
      else
      {
         debugMilitaryTraining("Adjusting our availableHeroPop to: " + availableHeroPop + ", because of our scouting reports.");
      }
   }

   int numMilitaryBuildings = gMilitaryBuildings.size();
   int bushiPopCost = kbPlayerGetProtoStatInt(cMyID, cUnitTypeBushi, cProtoStatPopCost);
   int onnaMushaPopCost = kbPlayerGetProtoStatInt(cMyID, cUnitTypeOnnaMusha, cProtoStatPopCost);
   int daimyoPopCost = kbPlayerGetProtoStatInt(cMyID, cUnitTypeDaimyo, cProtoStatPopCost);
   int onmyojiPopCost = kbPlayerGetProtoStatInt(cMyID, cUnitTypeOnmyoji, cProtoStatPopCost);

   int maintainBushiAmount = 0;
   int maintainOnnaMushaAmount = 0;
   int maintainDaimyoAmount = 0;
   int maintainOnmyojiAmount = 0;

   int maxRoll = 1;
   if (daimyoEnabled == true)
   {
      maxRoll++;
   }
   if (onmyojiEnabled == true)
   {
      maxRoll++;
   }
   if (daimyoEnabled == false && onmyojiEnabled == true)
   {
      aiEchoWarning("Our enabling logic is not how we expect it to be, Japanese hero training will be broken now.");
   }

   while (availableHeroPop > 0)
   {
      int rand = xsRandInt(0, maxRoll);
      switch (rand)
      {
         case 0:
         {
            maintainBushiAmount++;
            availableHeroPop -= bushiPopCost;
            break;
         }
         case 1:
         {
            maintainOnnaMushaAmount++;
            availableHeroPop -= onnaMushaPopCost;
            break;
         }
         case 2:
         {
            maintainDaimyoAmount++;
            availableHeroPop -= daimyoPopCost;
            break;
         }
         case 3:
         {
            maintainOnmyojiAmount++;
            availableHeroPop -= onmyojiPopCost;
            break;
         }
      }
   }

   // It could be that this plan is already perfectly set up, then don't do anything.
   if (aiPlanGetVariableInt(bushiPlanID, cTrainPlanNumberToMaintain, 0) == maintainBushiAmount)
   {
      debugMilitaryTraining("Maintain plan for " + maintainBushiAmount + " Bushi doesn't require any changes.");
   }
   else
   {
      aiPlanSetVariableInt(bushiPlanID, cTrainPlanNumberToMaintain, 0, maintainBushiAmount);
      aiPlanSetName(bushiPlanID, bushiPlanID + ": Hero maintain: " + maintainBushiAmount + " Bushi");
      debugMilitaryTraining("Adjusting maintain plan for Bushi to maintain " + maintainBushiAmount + ".");
   }
   for (int i = 0; i < numMilitaryBuildings; i++)
   {
      if (kbProtoUnitCanTrain(gMilitaryBuildings[i], cUnitTypeBushi) == true)
      {
         gArmyUnitBuildings[gMaintainPlanHeroStartIndex] = gMilitaryBuildings[i];
         break;
      }
   }

   // It could be that this plan is already perfectly set up, then don't do anything.
   if (aiPlanGetVariableInt(onnaMushaPlanID, cTrainPlanNumberToMaintain, 0) == maintainOnnaMushaAmount)
   {
      debugMilitaryTraining("Maintain plan for " + maintainOnnaMushaAmount + " Onna-Musha doesn't require any changes.");
   }
   else
   {
      aiPlanSetVariableInt(onnaMushaPlanID, cTrainPlanNumberToMaintain, 0, maintainOnnaMushaAmount);
      aiPlanSetName(onnaMushaPlanID, onnaMushaPlanID + ": Hero maintain: " + maintainOnnaMushaAmount + " Onna-Musha");
      debugMilitaryTraining("Adjusting maintain plan for Onna-Musha to maintain " + maintainOnnaMushaAmount + ".");
   }
   for (int i = 0; i < numMilitaryBuildings; i++)
   {
      if (kbProtoUnitCanTrain(gMilitaryBuildings[i], cUnitTypeOnnaMusha) == true)
      {
         gArmyUnitBuildings[gMaintainPlanHeroStartIndex + 1] = gMilitaryBuildings[i];
         break;
      }
   }

   if (daimyoEnabled == true)
   {
      // It could be that this plan is already perfectly set up, then don't do anything.
      if (aiPlanGetVariableInt(daimyoPlanID, cTrainPlanNumberToMaintain, 0) == maintainDaimyoAmount)
      {
         debugMilitaryTraining("Maintain plan for " + maintainDaimyoAmount + " Daimyo doesn't require any changes.");
      }
      else
      {
         aiPlanSetVariableInt(daimyoPlanID, cTrainPlanNumberToMaintain, 0, maintainDaimyoAmount);
         aiPlanSetName(daimyoPlanID, daimyoPlanID + ": Hero maintain: " + maintainDaimyoAmount + " Daimyo");
         debugMilitaryTraining("Adjusting maintain plan for Daimyo to maintain " + maintainDaimyoAmount + ".");
      }
      for (int i = 0; i < numMilitaryBuildings; i++)
      {
         if (kbProtoUnitCanTrain(gMilitaryBuildings[i], cUnitTypeDaimyo) == true)
         {
            gArmyUnitBuildings[gMaintainPlanHeroStartIndex + 2] = gMilitaryBuildings[i];
            break;
         }
      }
   }

   if (onmyojiEnabled == true)
   {
      // It could be that this plan is already perfectly set up, then don't do anything.
      if (aiPlanGetVariableInt(onmyojiPlanID, cTrainPlanNumberToMaintain, 0) == maintainOnmyojiAmount)
      {
         debugMilitaryTraining("Maintain plan for " + maintainOnmyojiAmount + " Onmyoji doesn't require any changes.");
      }
      else
      {
         aiPlanSetVariableInt(onmyojiPlanID, cTrainPlanNumberToMaintain, 0, maintainOnmyojiAmount);
         aiPlanSetName(onmyojiPlanID, onmyojiPlanID + ": Hero maintain: " + maintainOnmyojiAmount + " Onmyoji");
         debugMilitaryTraining("Adjusting maintain plan for Onmyoji to maintain " + maintainOnmyojiAmount + ".");
      }
      for (int i = 0; i < numMilitaryBuildings; i++)
      {
         if (kbProtoUnitCanTrain(gMilitaryBuildings[i], cUnitTypeOnmyoji) == true)
         {
            gArmyUnitBuildings[gMaintainPlanHeroStartIndex + 3] = gMilitaryBuildings[i];
            break;
         }
      }
   }

   totalAvailablePop -= maintainBushiAmount * bushiPopCost;
   totalAvailablePop -= maintainOnnaMushaAmount * onnaMushaPopCost;
   totalAvailablePop -= maintainDaimyoAmount * daimyoPopCost;
   totalAvailablePop -= maintainOnmyojiAmount * onmyojiPopCost;
}

//==============================================================================
// aztecHeroTraining
//==============================================================================
void aztecHeroTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   int planID = gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex];
   if (aiPlanGetIsIDValid(planID) == false)
   {
      planID = createSimpleMaintainPlan(cUnitTypeWarriorPriest, 0, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
      gArmyUnitMaintainPlans[gMaintainPlanHeroStartIndex] = planID;
   }
   
   int availableHeroPop = originalTotalMilitaryPop * gArmyHeroPercentage;
   int minimumHeroPop = originalTotalMilitaryPop * (gArmyHeroPercentage / 2);
   debugMilitaryTraining("availableHeroPop is: " + availableHeroPop + ", minimumHeroPop is: " + minimumHeroPop + ".");

   int targetPlayer = aiGetMostHatedPlayerID();
   if (targetPlayer > 0)
   {
      int enemyMythPop = kbGetPopulationSlotsByUnitTypeID(cUnitTypeMythUnit, targetPlayer);
      debugMilitaryTraining("Scouted a total of enemyMythPop: " + enemyMythPop + ".");
      // We match enemy myth pop or cap at our original.
      availableHeroPop = min(availableHeroPop, enemyMythPop);
      // If our matching of the enemy pop turned out to be a really low number we use our minimum instead.
      if (availableHeroPop < minimumHeroPop)
      {
         availableHeroPop = minimumHeroPop;
         debugMilitaryTraining("Using minimumHeroPop because we haven't scouted enough enemy myth units to warrant many heroes.");
      }
      else
      {
         debugMilitaryTraining("Adjusting our availableHeroPop to: " + availableHeroPop + ", because of our scouting reports.");
      }
   }

   int priestPopCost = kbPlayerGetProtoStatInt(cMyID, cUnitTypePriest, cProtoStatPopCost);
   int numberToMaintain = ceil(xsIntToFloat(availableHeroPop) / xsIntToFloat(priestPopCost));
   // It could be that this plan is already perfectly set up, then don't do anything.
   if (aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) == numberToMaintain)
   {
      debugMilitaryTraining("Maintain plan for " + numberToMaintain + " Warrior Priest doesn't require any changes.");
   }
   else
   {
      aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, numberToMaintain);
      aiPlanSetName(planID, planID + ": Hero maintain: " + numberToMaintain + " Warrior Priest");
      debugMilitaryTraining("Adjusting maintain plan for Warrior Priest to maintain " + numberToMaintain + ".");
   }
   totalAvailablePop -= numberToMaintain * priestPopCost;
}

//==============================================================================
// heroMilitaryTraining
//==============================================================================
void heroMilitaryTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   switch (cMyCulture)
   {
      case cCultureGreek:
      {
         greekHeroTraining(totalAvailablePop, originalTotalMilitaryPop);
         break;
      }
      case cCultureEgyptian:
      {
         egyptianHeroTraining(totalAvailablePop, originalTotalMilitaryPop);
         break;
      }
      case cCultureNorse:
      {
         norseHeroTraining(totalAvailablePop, originalTotalMilitaryPop);
         break;
      }
      case cCultureAtlantean:
      {
         atlanteanHeroTraining(totalAvailablePop, originalTotalMilitaryPop);
         break;
      }
      case cCultureChinese:
      {
         chineseHeroTraining(totalAvailablePop, originalTotalMilitaryPop);
         break;
      }
      case cCultureJapanese:
      {
         japaneseHeroTraining(totalAvailablePop, originalTotalMilitaryPop);
         break;
      }
      case cCultureAztec:
      {
         aztecHeroTraining(totalAvailablePop, originalTotalMilitaryPop);
         break;
      }
   }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

//==============================================================================
// capUnwantedMythUnits
//==============================================================================
int capUnwantedMythUnits(int mythPUID = -1, int trainAmount = -1)
{
   if (mythPUID == cUnitTypeCaladria)
   {
      int existingUnits = kbUnitCount(mythPUID, cMyID, cUnitStateAlive);
      int[] planIDs = aiPlanGetIDsByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, mythPUID);
      for (int j = 0; j < planIDs.size(); j++)
      {
         existingUnits += aiPlanGetVariableInt(planIDs[j], cTrainPlanNumberToTrain, 0) -
                          aiPlanGetVariableInt(planIDs[j], cTrainPlanNumberTrained, 0);
      }
      return min(max(0, 2 - existingUnits), trainAmount);
   }
   if (mythPUID == cUnitTypeKitsune)
   {
      int existingUnits = kbUnitCount(mythPUID, cMyID, cUnitStateAlive);
      int[] planIDs = aiPlanGetIDsByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, mythPUID);
      for (int j = 0; j < planIDs.size(); j++)
      {
         existingUnits += aiPlanGetVariableInt(planIDs[j], cTrainPlanNumberToTrain, 0) -
                          aiPlanGetVariableInt(planIDs[j], cTrainPlanNumberTrained, 0);
      }
      return min(max(0, 2 - existingUnits), trainAmount);
   }
   return trainAmount;
}

//==============================================================================
// mythMilitaryTraining
//==============================================================================
void mythMilitaryTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   // Clear out this array of plans that are done.
   for (int i = gMythUnitTrainPlans.size() - 1; i >= 0; i--)
   {
      if (aiPlanGetIsIDValid(gMythUnitTrainPlans[i]) == false)
      {
         gMythUnitTrainPlans.removeIndex(i);
      }
   }

   int[] enabledMythUnits = getAllEnabledMilitaryMythUnits(cUnitTypeTemple);
   if (enabledMythUnits.size() == 0)
   {
      debugMilitaryTraining("mythMilitaryTraining - no enabled myth units to train.");
      return;
   }

   int currentMythPop = 0;
   for (int i = 0; i < enabledMythUnits.size(); i++)
   {
      int mythPUID = enabledMythUnits[i];

      debugMilitaryTraining("mythMilitaryTraining - analyzing mythPUID: " + kbProtoUnitGetName(mythPUID) + ".");
      int popCost = kbPlayerGetProtoStatInt(cMyID, mythPUID, cProtoStatPopCost);
      currentMythPop += kbUnitCount(mythPUID, cMyID, cUnitStateAlive) * popCost;
      int[] planIDs = aiPlanGetIDsByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, mythPUID);
      for (int j = 0; j < planIDs.size(); j++)
      {
         int toTrain = aiPlanGetVariableInt(planIDs[j], cTrainPlanNumberToTrain, 0) -
                       aiPlanGetVariableInt(planIDs[j], cTrainPlanNumberTrained, 0);
         currentMythPop += toTrain * popCost;
      }
      debugMilitaryTraining("mythMilitaryTraining - currentMythPop: " + currentMythPop + ".");
   }

   float currentMythPopPercentage = gArmyEarlyGameMythPercentage;
   if (kbPlayerGetAge(cMyID) >= cAge3) // From Heroic we start training more Myth.
   {
      currentMythPopPercentage = gArmyLateGameMythPercentage;
   }
   int availableMythPop = originalTotalMilitaryPop * currentMythPopPercentage;
   int minimumMythPop = originalTotalMilitaryPop * (gArmyHeroPercentage / 2);
   debugMilitaryTraining("availableMythPop is: " + availableMythPop + ", minimumMythPop is: " + minimumMythPop + ".");
   debugMilitaryTraining("We're starting with currentMythPop: " + currentMythPop + ".");
   if (availableMythPop <= currentMythPop)
   {
      totalAvailablePop -= currentMythPop;
      debugMilitaryTraining("We already have enough myth pop, skipping. Available availableMythPop: " + availableMythPop +
         " current: " + currentMythPop + ".");
      return;
   }
   
   int targetPlayer = aiGetMostHatedPlayerID();
   if (targetPlayer > 0)
   {
      int enemyHeroPop = kbGetPopulationSlotsByUnitTypeID(cUnitTypeHero, targetPlayer);
      if (kbPlayerGetCulture(targetPlayer) == cCultureGreek)
      {
         enemyHeroPop *= 1.5;
         debugMilitaryTraining("Target player is Greek, multiplying scouted hero pop by 1.5 to account for their increased " + 
            " strength, new enemyHeroPop: " + enemyHeroPop + ".");
      }
      // If the enemy has more hero pop than 75% of our availableMythPop we use our minimumMythPop instead.
      // Because clearly the enemy has a lot of heroes, we don't want to get countered too hard.
      int threshold = availableMythPop * 0.75;
      debugMilitaryTraining("Scouted a total of enemyHeroPop: " + enemyHeroPop + ". If this number is >= " + 
         threshold + " we will use minimumMythPop.");
      if (enemyHeroPop >= threshold)
      {
         availableMythPop = minimumMythPop;
         debugMilitaryTraining("Using minimumMythPop because the enemy has too many heroes.");
      }
   }

   if (availableMythPop <= currentMythPop)
   {
      totalAvailablePop -= currentMythPop;
      debugMilitaryTraining("We already have enough currentMythPop, skipping. availableMythPop: " + availableMythPop + 
         " currentMythPop: " + currentMythPop + ".");
      return;
   }
   
   int mythPopToTrain = availableMythPop - currentMythPop;
   debugMilitaryTraining("We're going to train myth units for maximum mythPopToTrain: " + mythPopToTrain + 
      ". Calculation: availableMythPop(" + availableMythPop + ") - currentMythPop(" + currentMythPop + ").");

   int index = 0;
   if (enabledMythUnits.size() > 1)
   {
      if (cPersonalityCurrent == cPersonalitySieger)
      {
         // Add all myth units that are good at sieging to a new array, we will rand from that array instead.
         // Don't just pick the best siege myth unit all the time, then it gets monotone.
         int[] siegeMythUnits = new int(0, 0);
         for (int i = 0; i < enabledMythUnits.size(); i++)
         {
            float siegePower = kbProtoUnitGetSiegePower(enabledMythUnits[i]);
            if (siegePower >= 4.0)
            {
               debugMilitaryTraining("Valid siege myth unit: " + kbProtoUnitGetName(enabledMythUnits[i]) + ".");
               siegeMythUnits.add(enabledMythUnits[i]);
            }
         }
         // Do we even have good siegers?
         if (siegeMythUnits.size() == 0)
         {
            index = xsRandInt(0, enabledMythUnits.size() - 1); // Standard logic.
         }
         else if (siegeMythUnits.size() == 1)
         {
            index = enabledMythUnits.find(siegeMythUnits[0]);
         }
         else
         {
            // Random siege myth unit.
            int chosenSiegePUID = siegeMythUnits[xsRandInt(0, siegeMythUnits.size() - 1)];
            index = enabledMythUnits.find(chosenSiegePUID);
         }
      }
      else
      {
         // Randomly choose a myth unit to make a train plan for.
         index = xsRandInt(0, enabledMythUnits.size() - 1);
      }
   }
   int mythPUID = enabledMythUnits[index];
   int popCost = kbPlayerGetProtoStatInt(cMyID, mythPUID, cProtoStatPopCost);
   int trainAmount = mythPopToTrain / popCost; // Don't overshoot this time.
   if (trainAmount == 0)
   {
      trainAmount = 1; // But always want 1 at least :D
   }
   else if (trainAmount > 3)
   {
      debugMilitaryTraining("Clamping our train amount to 3 because we wanted too many myth units at once.");
      trainAmount = 3; // Don't train too many of one type at once. Wait until next iteration to train more (maybe other type).
   }
   trainAmount = capUnwantedMythUnits(mythPUID, trainAmount);
   // TrainAmount could've been set to 0 by capUnwantedMythUnits.
   if (trainAmount >= 1)
   {
      int planID = createSimpleTrainPlan(mythPUID, trainAmount, gLandAreaGroupID, gMilitaryTrainingCategoryID);
      gMythUnitTrainPlans.add(planID);
   }
   totalAvailablePop -= currentMythPop;
   totalAvailablePop -= trainAmount * popCost;
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

//==============================================================================
// siegeMilitaryTraining
//==============================================================================
void siegeMilitaryTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   static int siegePUID1 = -1;
   static int siegePUID2 = -1;
   static int siege1PopCost = -1;
   static int siege2PopCost = -1;
   if (siegePUID1 == -1) // First run.
   {
      switch (cMyCulture)
      {
         case cCultureGreek:
         {
            siegePUID1 = cUnitTypePetrobolos;
            siegePUID2 = cUnitTypeHelepolis;
            siege1PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID1, cProtoStatPopCost);
            siege2PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID2, cProtoStatPopCost);
            break;
         }
         case cCultureEgyptian:
         {
            siegePUID1 = cUnitTypeSiegeTower;
            siegePUID2 = cUnitTypeCatapult;
            siege1PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID1, cProtoStatPopCost);
            siege2PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID2, cProtoStatPopCost);
            break;
         }
         case cCultureNorse:
         {
            siegePUID1 = cUnitTypePortableRam;
            siegePUID2 = cUnitTypeBallista;
            siege1PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID1, cProtoStatPopCost);
            siege2PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID2, cProtoStatPopCost);
            break;
         }
         case cCultureAtlantean:
         {
            siegePUID1 = cUnitTypeDestroyer;
            siegePUID2 = cUnitTypeFireSiphon;
            siege1PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID1, cProtoStatPopCost);
            siege2PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID2, cProtoStatPopCost);
            break;
         }
         case cCultureChinese:
         {
            siegePUID1 = cUnitTypeAxeCart;
            siegePUID2 = cUnitTypeSiegeCrossbow;
            siege1PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID1, cProtoStatPopCost);
            siege2PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID2, cProtoStatPopCost);
            break;
         }
         case cCultureJapanese:
         {
            // Only has 1 siege unit.
            siegePUID1 = cUnitTypeOyumi;
            siege1PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID1, cProtoStatPopCost);
            break;
         }
         case cCultureAztec:
         {
            siegePUID1 = cUnitTypeOtontin;
            siegePUID2 = cUnitTypeQuinametzin;
            siege1PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID1, cProtoStatPopCost);
            siege2PopCost = kbPlayerGetProtoStatInt(cMyID, siegePUID2, cProtoStatPopCost);
         }
      }
   }

   bool siege1Enabled = false;
   if (kbProtoUnitAvailable(siegePUID1) == true)
   {
      if (aiPlanGetIsIDValid(gArmyUnitMaintainPlans[gMaintainPlanSiegeStartIndex]) == false)
      {
         gArmyUnitMaintainPlans[gMaintainPlanSiegeStartIndex] = createSimpleMaintainPlan(siegePUID1, 0, gLandAreaGroupID,
            gMilitaryTrainingCategoryID, 50, -1, -1, true);
      }
      siege1Enabled = true;
   }
   bool siege2Enabled = false;
   if (cMyCulture != cCultureJapanese) // Japan only has 1 siege unit.
   {
      if (kbProtoUnitAvailable(siegePUID2) == true)
      {
         if (aiPlanGetIsIDValid(gArmyUnitMaintainPlans[gMaintainPlanSiegeStartIndex + 1]) == false)
         {
            gArmyUnitMaintainPlans[gMaintainPlanSiegeStartIndex + 1] = createSimpleMaintainPlan(siegePUID2, 0, gLandAreaGroupID,
               gMilitaryTrainingCategoryID, 50, -1, -1, true);
         }
         siege2Enabled = true;
      }
   }
   if (siege1Enabled == false && siege2Enabled == false)
   {
      debugMilitaryTraining("Quiting siegeMilitaryTraining because we're not in a high enough age yet to train siege.");
      return;
   }

   int numMilitaryBuildings = gMilitaryBuildings.size();
   int buildingPUID = -1;
   int availableSiegePop = originalTotalMilitaryPop * gArmySiegePercentage;
   if (gDefensivelyOverrun == true)
   {
      debugMilitaryTraining("Not training siege because we're in a defense panic, we have no use for siege now.");
      availableSiegePop = 0;
   }
   debugMilitaryTraining("availableSiegePop is: " + availableSiegePop + ".");

   if (siege1Enabled == true && siege2Enabled == false)
   {
      int planID = gArmyUnitMaintainPlans[gMaintainPlanSiegeStartIndex];
      #if (cMyCulture == cCultureJapanese)
      // First siege weapon will be our only siege weapon, we're allowed to overshoot.
      int numberToMaintain = ceil(xsIntToFloat(availableSiegePop) / xsIntToFloat(siege1PopCost));
      #else
      int numberToMaintain = availableSiegePop / siege1PopCost; // Undershoot in early ages.
      #endif
      // It could be that this plan is already perfectly set up, then don't do anything.
      if (aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) == numberToMaintain)
      {
         debugMilitaryTraining("Maintain plan for " + numberToMaintain + " " + kbProtoUnitGetName(siegePUID1) +
            " doesn't require any changes.");
      }
      else
      {
         aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, numberToMaintain);
         aiPlanSetName(planID, planID + ": Siege maintain: " + numberToMaintain + " " + kbProtoUnitGetName(siegePUID1));
         debugMilitaryTraining("Adjusting maintain plan for " + kbProtoUnitGetName(siegePUID1) + " to maintain " +
            numberToMaintain + ".");
      }
      for (int j = 0; j < numMilitaryBuildings; j++)
      {
         buildingPUID = gMilitaryBuildings[j];
         if (kbProtoUnitCanTrain(buildingPUID, siegePUID1) == true)
         {
            gArmyUnitBuildings[gMaintainPlanSiegeStartIndex] = buildingPUID;
            break;
         }
      }
      totalAvailablePop -= numberToMaintain * siege1PopCost;
   }
   else if (siege1Enabled == false && siege2Enabled == true)
   {
      int planID = gArmyUnitMaintainPlans[gMaintainPlanSiegeStartIndex + 1];
      int numberToMaintain = availableSiegePop / siege2PopCost; // Undershoot in early ages.
      // It could be that this plan is already perfectly set up, then don't do anything.
      if (aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) == numberToMaintain)
      {
         debugMilitaryTraining("Maintain plan for " + numberToMaintain + " " + kbProtoUnitGetName(siegePUID2) +
            " doesn't require any changes.");
      }
      else
      {
         aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, numberToMaintain);
         aiPlanSetName(planID, planID + ": Siege maintain: " + numberToMaintain + " " + kbProtoUnitGetName(siegePUID2));
         debugMilitaryTraining("Adjusting maintain plan for " + kbProtoUnitGetName(siegePUID2) + " to maintain " + numberToMaintain + ".");
      }
      for (int j = 0; j < numMilitaryBuildings; j++)
      {
         buildingPUID = gMilitaryBuildings[j];
         if (kbProtoUnitCanTrain(buildingPUID, siegePUID2) == true)
         {
            gArmyUnitBuildings[gMaintainPlanSiegeStartIndex + 1] = buildingPUID;
            break;
         }
      }
      totalAvailablePop -= numberToMaintain * siege2PopCost;
   }
   else // Both are enabled.
   {
      int maintainSiege1Amount = 0;
      int maintainSiege2Amount = 0;
      while (availableSiegePop > 0)
      {
         if (xsRandBool() == true)
         {
            maintainSiege1Amount++; // Overshoot in later ages.
            availableSiegePop -= siege1PopCost;
         }
         else
         {
            maintainSiege2Amount++; // Overshoot in later ages.
            availableSiegePop -= siege2PopCost;
         }
      }

      int planID = gArmyUnitMaintainPlans[gMaintainPlanSiegeStartIndex];
      // It could be that this plan is already perfectly set up, then don't do anything.
      if (aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) == maintainSiege1Amount)
      {
         debugMilitaryTraining("Maintain plan for " + maintainSiege1Amount + " " + kbProtoUnitGetName(siegePUID1) +
            " doesn't require any changes.");
      }
      else
      {
         aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, maintainSiege1Amount);
         aiPlanSetName(planID, planID + ": Siege maintain: " + maintainSiege1Amount + " " + kbProtoUnitGetName(siegePUID1));
         debugMilitaryTraining("Adjusting maintain plan for " + kbProtoUnitGetName(siegePUID1) + " to maintain " +
            maintainSiege1Amount + ".");
      }
      for (int j = 0; j < numMilitaryBuildings; j++)
      {
         buildingPUID = gMilitaryBuildings[j];
         if (kbProtoUnitCanTrain(buildingPUID, siegePUID1) == true)
         {
            gArmyUnitBuildings[gMaintainPlanSiegeStartIndex] = buildingPUID;
            break;
         }
      }

      planID = gArmyUnitMaintainPlans[gMaintainPlanSiegeStartIndex + 1];
      // It could be that this plan is already perfectly set up, then don't do anything.
      if (aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) == maintainSiege2Amount)
      {
         debugMilitaryTraining("Maintain plan for " + maintainSiege2Amount + " " + kbProtoUnitGetName(siegePUID2) +
            " doesn't require any changes.");
      }
      else
      {
         aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, maintainSiege2Amount);
         aiPlanSetName(planID, planID + ": Siege maintain: " + maintainSiege2Amount + " " + kbProtoUnitGetName(siegePUID2));
         debugMilitaryTraining("Adjusting maintain plan for " + kbProtoUnitGetName(siegePUID2) + " to maintain " + 
            maintainSiege2Amount + ".");
      }
      for (int j = 0; j < numMilitaryBuildings; j++)
      {
         buildingPUID = gMilitaryBuildings[j];
         if (kbProtoUnitCanTrain(buildingPUID, siegePUID2) == true)
         {
            gArmyUnitBuildings[gMaintainPlanSiegeStartIndex + 1] = buildingPUID;
            break;
         }
      }

      totalAvailablePop -= maintainSiege1Amount * siege1PopCost;
      totalAvailablePop -= maintainSiege2Amount * siege2PopCost;
   }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

//==============================================================================
// sharedHumanArcherUnitPickerSetup
//==============================================================================
bool sharedHumanArcherUnitPickerSetup(int upID = -1)
{
   kbUnitPickResetAll(upID);
   const int minimumPopForCounterMode = 15;
   const int attackUnitType = cUnitTypeLogicalTypeLandMilitary;
   kbUnitPickSetMinimumCounterModePop(upID, minimumPopForCounterMode);
   // Default to scanning for enemy land military.
   kbUnitPickSetAttackUnitType(upID, attackUnitType);
   // We however don't want to compare against the enemy myth units/siege.
   // These human units aren't meant to counter the myth units and will all do very poorly in a comparison.
   // That data just ain't useful for us, we want to compare against enemy heroes/humans which we expect to fight.
   // The data from comparing against siege is also not useful for us. We're archers, we already know we will have incredibly low
   // efficiency. And taking that data into account will give all the archers a very low score always.
   // Animals of set are another edge case, cost very little so we're always very inefficient and we can't counter them anyway.
   // TODO: we could skip archer training if there are many siege units in our base.
   int[] excludeTypes = new int(3, -1);
   excludeTypes[0] = cUnitTypeMythUnit;
   excludeTypes[1] = cUnitTypeSiegeLineUpgraded; // Cheiro isn't part of this, all other siege is.
   excludeTypes[2] = cUnitTypeAnimalOfSet;
   kbUnitPickSetCounterModeExcludeTypes(upID, excludeTypes);

   int targetPlayerID = aiGetMostHatedPlayerID();
   bool haveValidEnemyPlayer = true;
   bool inCounterMode = false;
   if (targetPlayerID <= 0 || kbPlayerHasLost(targetPlayerID) == true)
   {
      haveValidEnemyPlayer = false;
   }
   if (haveValidEnemyPlayer == true)
   {
      kbUnitPickSetEnemyPlayerID(upID, targetPlayerID);
      // This query will also happen in the source, we just find out manually here if we would go into counter mode or not.
      int queryID = useSimpleUnitQuery(attackUnitType, targetPlayerID, cUnitStateAlive);
      kbUnitQuerySetExcludeTypes(queryID, excludeTypes);
      kbUnitQueryExecute(queryID);
      int popCount = kbUnitQueryGetPopulationSlots(queryID);
      if (popCount >= minimumPopForCounterMode)
      {
         inCounterMode = true;
      }
   }

   // Our Human Archers should be able to move over land.
   kbUnitPickSetMovementType(upID, cPassabilityLand);

   // We want to analyze all our Human Archers.
   kbUnitPickSetPreferenceFactor(upID, cUnitTypeAbstractArcher, 0.01);
   kbUnitPickSetPreferenceFactor(upID, cUnitTypeHero, 0.0);

   kbUnitPickSetCombatEfficiencyWeight(upID, 1.0);

   // Set the default target types and weights, for use until we've seen enough actual units to counter them.
   // So we basically try to build a raiding army for now.
   if (haveValidEnemyPlayer == true && inCounterMode == false)
   {
      int culture = kbPlayerGetCulture(targetPlayerID);
      switch (culture)
      {
         case cCultureGreek:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerGreek, 1.0);
            break;
         }
         case cCultureEgyptian:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerEgyptian, 1.0);
            break;
         }
         case cCultureNorse:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerNorse, 1.0);
            break;
         }
         case cCultureAtlantean:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerAtlantean, 1.0);
            break;
         }
         case cCultureChinese:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerChinese, 1.0);
            break;
         }
         case cCultureAztec:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerAztec, 1.0);
            break;
         }
      }
   }
   return inCounterMode;
}

//==============================================================================
// greekHumanArcherUnitPicker
//==============================================================================
bool greekHumanArcherUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanArcherUnitPickerSetup(upID);
   if (inCounterMode == false && kbPlayerGetAge(cMyID) >= cAge3)
   {
      debugMilitaryTraining("We're not in counter mode, removing Peltast from our unitPicker.");
      kbUnitPickSetPreferenceFactor(upID, cUnitTypePeltast, 0.0);
   }
   return inCounterMode;
}

//==============================================================================
// egyptianHumanArcherUnitPicker
//==============================================================================
bool egyptianHumanArcherUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanArcherUnitPickerSetup(upID);
   // Remove Slingers only if we already have Chariots.
   if (inCounterMode == false && kbPlayerGetAge(cMyID) >= cAge3 && kbUnitCount(cUnitTypeMigdolStronghold, cMyID, cUnitStateAlive) >= 1)
   {
      debugMilitaryTraining("We're not in counter mode and could train Chariot Archers, removing Slingers from our unitPicker.");
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeSlinger, 0.0);
   }
   return inCounterMode;
}

//==============================================================================
// norseHumanArcherUnitPicker
//==============================================================================
bool norseHumanArcherUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanArcherUnitPickerSetup(upID);
   // We only have Throwing Axemen, never cancel them.
   return inCounterMode;
}

//==============================================================================
// atlanteanHumanArcherUnitPicker
//==============================================================================
bool atlanteanHumanArcherUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanArcherUnitPickerSetup(upID);
   if (kbPlayerGetAge(cMyID) >= cAge3)
   {
      if (inCounterMode == false)
      {
         debugMilitaryTraining("We're not in counter mode and could train Arcus, removing Turma/Cheiroballista from our unitPicker.");
         kbUnitPickSetPreferenceFactor(upID, cUnitTypeTurma, 0.0);
         kbUnitPickSetPreferenceFactor(upID, cUnitTypeCheiroballista, 0.0);
      }
      else
      {
         // 80% Chance in heroic to just ignore Turma, those units really fall off.
         if (xsRandBool(0.8) == true)
         {
            debugMilitaryTraining("Randomly removing Turma from our unit picker because we just like Arcus more.");
            kbUnitPickSetPreferenceFactor(upID, cUnitTypeTurma, 0.0);
         }
         // Arcus are just much stronger than Turma/Cheiro normally. Give them an artificial boost.
         kbUnitPickSetPreferenceFactor(upID, cUnitTypeArcus, 0.2);
      }
   }
   return inCounterMode;
}

//==============================================================================
// chineseHumanArcherUnitPicker
//==============================================================================
bool chineseHumanArcherUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanArcherUnitPickerSetup(upID);
   return inCounterMode;
}

//==============================================================================
// japaneseHumanArcherUnitPicker
//==============================================================================
bool japaneseHumanArcherUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanArcherUnitPickerSetup(upID);
   return inCounterMode;
}

//==============================================================================
// azteceHumanArcherUnitPicker
//==============================================================================
bool azteceHumanArcherUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanArcherUnitPickerSetup(upID);
   return inCounterMode;
}

//==============================================================================
// humanArcherMilitaryTraining
//==============================================================================
void humanArcherMilitaryTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   static int archerUnitPickerID = -1;
   if (kbUnitPickGetIsIDValid(archerUnitPickerID) == false)
   {
      archerUnitPickerID = kbUnitPickCreate("Human archer military units");
   }

   bool counterMode = false;
   // Update preferences in case something changed inbetween.
   switch (cMyCulture)
   {
      case cCultureGreek:
      {
         counterMode = greekHumanArcherUnitPicker(archerUnitPickerID);
         break;
      }
      case cCultureEgyptian:
      {
         counterMode = egyptianHumanArcherUnitPicker(archerUnitPickerID);
         break;
      }
      case cCultureNorse:
      {
         counterMode = norseHumanArcherUnitPicker(archerUnitPickerID);
         break;
      }
      case cCultureAtlantean:
      {
         counterMode = atlanteanHumanArcherUnitPicker(archerUnitPickerID);
         break;
      }
      case cCultureChinese:
      {
         counterMode = chineseHumanArcherUnitPicker(archerUnitPickerID);
         break;
      }
      case cCultureJapanese:
      {
         counterMode = japaneseHumanArcherUnitPicker(archerUnitPickerID);
         break;
      }
      case cCultureAztec:
      {
         counterMode = azteceHumanArcherUnitPicker(archerUnitPickerID);
         break;
      }
   }

   kbUnitPickRun(archerUnitPickerID);

   int totalArcherPop = totalAvailablePop * gHumanArmyArcherPercentage;
   debugMilitaryTraining("Available pop is: " + totalAvailablePop + " and gHumanArmyArcherPercentage is: " +
      gHumanArmyArcherPercentage + ", which gives " + totalArcherPop + " pop for archers.");

   // Reset the building(s) we want to use for archers.
   for (int i = gMaintainPlanHumanArcherStartIndex; i < gNumHumanArcherUnitTypes; i++)
   {
      gArmyUnitBuildings[i] = -1;
   }
   
   float totalFactor = 0.0;
   int puid = -1;
   int existingPlanID = -1;
   int planID = -1;
   int[] temp = new int(gNumHumanArcherUnitTypes, -1);
   int[] unitPickResults = kbUnitPickGetResults(archerUnitPickerID);
   float[] unitPickResultFactors = kbUnitPickGetResultFactors(archerUnitPickerID);
   int numUnitPickerResults = unitPickResults.size();
   if (numUnitPickerResults <= 0)
   {
      aiEchoWarning("Human archer unit picker was unable to get any valid results!!!");
      return;
   }

   debugMilitaryTraining("Unit picker results:");
   for (int i = 0; i < numUnitPickerResults; i++)
   {
      debugMilitaryTraining("   # " + i + " = " + kbProtoUnitGetName(unitPickResults[i]) + ", factor = " +
         unitPickResultFactors[i] + ".");
   }

   // We have a unique case where Fortress buildings are quite expensive but also have the best units.
   // So upon aging up to Heroic it's very possible that we suddenly want to train those units but have 0 Fortresses, especially Eggy.
   // We need to guard against this by checking if the wanted unit has only a Fortress trainer, and if we have 0 of those -> remove.
   if (kbUnitCount(cUnitTypeAbstractFortress, cMyID, cUnitStateAlive) == 0)
   {
      for (int i = numUnitPickerResults - 1; i >= 0; i--)
      {
         puid = unitPickResults[i];
         int[] trainers = kbProtoUnitGetTrainers(puid);
         if (trainers.size() == 1 && trainers[0] == gFortressUnit)
         {
            debugMilitaryTraining("Skipping the training of " + kbProtoUnitGetName(puid) + ", because it can only be trained in " +
               "a " + kbProtoUnitGetName(gFortressUnit) + " and we have none of those now.");
            unitPickResults.removeIndex(i);
            unitPickResultFactors.removeIndex(i);
            numUnitPickerResults--;
         }
      }
   }

   if (counterMode == true)
   {
      if (unitPickResultFactors[0] < 0.6)
      {
         debugMilitaryTraining("Our most wanted archer has too low of a result factor, not training any archers now.");
         for (int i = gMaintainPlanHumanArcherStartIndex; i < gNumHumanArcherUnitTypes; i++)
         {
            if (aiPlanGetIsIDValid(gArmyUnitMaintainPlans[i]) == true)
            {
               aiPlanDestroy(gArmyUnitMaintainPlans[i]);
            }
            gArmyUnitMaintainPlans[i] = -1;
         }
         return;
      }
   }

   // We loop through all the units our human archer unit picker returned.
   // We see if we already have a maintain plan for said unit and save that information.
   // Also calculate the total factor.
   for (int i = 0; i < min(gNumHumanArcherUnitTypes, numUnitPickerResults); i++)
   {
      totalFactor += unitPickResultFactors[i];
      puid = unitPickResults[i];
      for (int j = gMaintainPlanHumanArcherStartIndex; j < gNumHumanArcherUnitTypes; j++)
      {
         existingPlanID = gArmyUnitMaintainPlans[j];
         // If we already have a plan for this unit, re-use it.
         if (aiPlanGetIsIDValid(existingPlanID) == true && puid == aiPlanGetVariableInt(existingPlanID, cTrainPlanUnitType, 0))
         {
            // Offset j for the temp array, since that array's index starts at 0.
            temp[j - gMaintainPlanHumanArcherStartIndex] = existingPlanID;
            break;
         }
      }
   }

   // We destroy any maintain plans that are no longer needed because our unit picker preference has changed.
   for (int i = gMaintainPlanHumanArcherStartIndex; i < gNumHumanArcherUnitTypes; i++)
   {
      // If temp[i] == -1 it means that we looped through our unit picker results and the plan at that index was training
      // a unit that we no longer want to train, thus we should destroy the plan.
      existingPlanID = gArmyUnitMaintainPlans[i];
      // Offset i for the temp array, since that array's index starts at 0.
      if (temp[i - gMaintainPlanHumanArcherStartIndex] == -1 && aiPlanGetIsIDValid(existingPlanID) == true)
      {
         debugMilitaryTraining("Destroying plan " + aiPlanGetName(existingPlanID) + ", because we no longer want that unit.");
         aiPlanDestroy(existingPlanID);
      }
   }

   debugMilitaryTraining("Start creating new/updating existing train plans, total factor: " + totalFactor + ".");
   for (int i = 0; i < min(gNumHumanArcherUnitTypes, numUnitPickerResults); i++)
   {
      puid = unitPickResults[i];

      // If we still have a maintain plan for this puid, re-use it.
      for (int j = 0; j < temp.size(); j++)
      {
         planID = temp[j];
         if (planID != -1 && puid == aiPlanGetVariableInt(planID, cTrainPlanUnitType, 0))
         {
            break;
         }
         planID = -1;
      }

      int numberToMaintain = 0;
      int popCount = kbPlayerGetProtoStatInt(cMyID, puid, cProtoStatPopCost);
      if (popCount > 0)
      {
         // Round up, never get into a situation where we don't train enough.
         numberToMaintain = ceil(((unitPickResultFactors[i] / totalFactor) * totalArcherPop) / popCount);
         debugMilitaryTraining("Unit: " + kbProtoUnitGetName(puid) + ", number to maintain: " + numberToMaintain + ".");
         debugMilitaryTraining("Factor: " + unitPickResultFactors[i] + ", popCount: " + popCount + ".");
      }
      else
      {
         aiEchoWarning("Unit: " + kbProtoUnitGetName(puid) + " has no population cost, it can't be used in standard military training.");
         continue;
      }
      
      if (planID < 0)
      {
         planID = createSimpleMaintainPlan(puid, numberToMaintain, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
         aiPlanSetName(planID, planID + ": Land military archer maintain: " + numberToMaintain + " " + kbProtoUnitGetName(puid));
         debugMilitaryTraining("Creating maintain plan for " + kbProtoUnitGetName(puid) + ".");
      }
      else
      {
         // It could be that this plan is already perfectly set up, then don't do anything.
         if (aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) == numberToMaintain)
         {
            debugMilitaryTraining("Existing maintain plan for " + kbProtoUnitGetName(puid) + " doesn't require changes.");
         }
         else
         {
            aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, numberToMaintain);
            aiPlanSetName(planID, planID + ": Land military archer maintain: " + numberToMaintain + " " + kbProtoUnitGetName(puid));
            debugMilitaryTraining("Adjusting existing maintain plan for " + kbProtoUnitGetName(puid) + ".");
         }
      }
      
      // Finally update our real array.
      gArmyUnitMaintainPlans[i + gMaintainPlanHumanArcherStartIndex] = planID;

      int trainBuildingPUID = -1;
      int numMilitaryBuildings = gMilitaryBuildings.size();
      for (int j = 0; j < numMilitaryBuildings; j++)
      {
         int buildingPUID = gMilitaryBuildings[j];
         if (kbProtoUnitCanTrain(buildingPUID, puid) == true)
         {
            trainBuildingPUID = buildingPUID;
            break;
         }
      }

      gArmyUnitBuildings[i + gMaintainPlanHumanArcherStartIndex] = trainBuildingPUID;

      totalAvailablePop -= numberToMaintain * popCount;
   }
}

//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

//==============================================================================
// sharedHumanUnitPickerSetup
//==============================================================================
bool sharedHumanUnitPickerSetup(int upID = -1)
{
   kbUnitPickResetAll(upID);
   const int minimumPopForCounterMode = 15;
   const int attackUnitType = cUnitTypeLogicalTypeLandMilitary;
   kbUnitPickSetMinimumCounterModePop(upID, minimumPopForCounterMode);
   // Default to scanning for enemy land military.
   kbUnitPickSetAttackUnitType(upID, attackUnitType);
   // We however don't want to compare against the enemy myth units/siege.
   // These human units aren't meant to counter the myth units and will all do very poorly in a comparison.
   // That data just ain't useful for us, we want to compare against enemy heroes/humans which we expect to fight.
   // All of these units will do well against siege, insane efficiency probably. That data is also not relevant to us at all.
   // Animals of set are another edge case, cost very little so we're always very inefficient and we can't counter them anyway.
   int[] excludeTypes = new int(3, -1);
   excludeTypes[0] = cUnitTypeMythUnit;
   excludeTypes[1] = cUnitTypeSiegeLineUpgraded; // Cheiro isn't part of this, all other siege is.
   excludeTypes[2] = cUnitTypeAnimalOfSet;
   kbUnitPickSetCounterModeExcludeTypes(upID, excludeTypes);

   int targetPlayerID = aiGetMostHatedPlayerID();
   bool haveValidEnemyPlayer = true;
   bool inCounterMode = false;
   if (targetPlayerID <= 0 || kbPlayerHasLost(targetPlayerID) == true)
   {
      haveValidEnemyPlayer = false;
   }
   if (haveValidEnemyPlayer == true)
   {
      kbUnitPickSetEnemyPlayerID(upID, targetPlayerID);
      // This query will also happen in the source, we just find out manually here if we would go into counter mode or not.
      int queryID = useSimpleUnitQuery(attackUnitType, targetPlayerID, cUnitStateAlive);
      kbUnitQuerySetExcludeTypes(queryID, excludeTypes);
      kbUnitQueryExecute(queryID);
      int popCount = kbUnitQueryGetPopulationSlots(queryID);
      if (popCount >= minimumPopForCounterMode)
      {
         inCounterMode = true;
      }
   }

   // Our Human Soldiers should be able to move over land.
   kbUnitPickSetMovementType(upID, cPassabilityLand);

   // We want to analyze all our Human Soldiers.
   kbUnitPickSetPreferenceFactor(upID, cUnitTypeHumanSoldier, 0.01);

   // We need to then remove all the archers since those are handled seperately.
   kbUnitPickSetPreferenceFactor(upID, cUnitTypeAbstractArcher, 0.0);

   kbUnitPickSetCombatEfficiencyWeight(upID, 1.0);

   // Set the default target types and weights, for use until we've seen enough actual units to counter them.
   // So we basically try to build a raiding army for now.
   if (haveValidEnemyPlayer == true && inCounterMode == false)
   {
      int culture = kbPlayerGetCulture(targetPlayerID);
      switch (culture)
      {
         case cCultureGreek:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerGreek, 1.0);
            break;
         }
         case cCultureEgyptian:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerEgyptian, 1.0);
            break;
         }
         case cCultureNorse:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerNorse, 1.0);
            break;
         }
         case cCultureAtlantean:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerAtlantean, 1.0);
            break;
         }
         case cCultureChinese:
         {
            kbUnitPickAddCombatEfficiencyType(upID, cUnitTypeVillagerChinese, 1.0);
            break;
         }
      }
   }
   return inCounterMode;
}

//==============================================================================
// greekHumanUnitPicker
//==============================================================================
bool greekHumanUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanUnitPickerSetup(upID);
   if (inCounterMode == false && kbPlayerGetAge(cMyID) >= cAge3)
   {
      debugMilitaryTraining("We're not in counter mode, removing Hypaspist/Prodromos from our unitPicker.");
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeHypaspist, 0.0);
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeProdromos, 0.0);
   }
   return inCounterMode;
}

//==============================================================================
// egyptianHumanUnitPicker
//==============================================================================
bool egyptianHumanUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanUnitPickerSetup(upID);
   kbUnitPickSetPreferenceFactor(upID, cUnitTypeMercenary, 0.0);
   kbUnitPickSetPreferenceFactor(upID, cUnitTypeMercenaryCavalry, 0.0);

   // Our Barracks units are all counter units but we can't exclude them since then it's only Migdol and we don't want that either!

   // War Elephants are always considered to be very strong by the AI, but don't keep training them too often since they're so expensive.
   if (xsRandBool(0.66) == true) // 66% chance true.
   {
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeWarElephant, 0.0);
   }
   return inCounterMode;
}

//==============================================================================
// norseHumanUnitPicker
//==============================================================================
bool norseHumanUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanUnitPickerSetup(upID);
   if (inCounterMode == false)
   {
      debugMilitaryTraining("We're not in counter mode, removing Hirdman/Huskarl from our unitPicker.");
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeHirdman, 0.0);
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeHuskarl, 0.0);
   }

   // We can build Huskarls earlier when we're Ullr, but if we lost the Asgardian Hill Fort we can't train them anymore in Classical.
   if (cMyCiv == cCivFreyr && kbPlayerGetAge(cMyID) == cAge2 && kbTechGetStatus(cTechClassicalAgeUllr) == cTechStatusActive &&
       getUnit(cUnitTypeAsgardianHillFort) == -1)
   {
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeHuskarl, 0.0);
   }

   return inCounterMode;
}

//==============================================================================
// atlanteanHumanUnitPicker
//==============================================================================
bool atlanteanHumanUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanUnitPickerSetup(upID);
   kbUnitPickSetPreferenceFactor(upID, cUnitTypeDestroyer, 0.0); // Trained via Siege training.
   kbUnitPickSetPreferenceFactor(upID, cUnitTypeOracle, 0.0); // oracleMaintainMonitor does this stuff.
   if (inCounterMode == false)
   {
      debugMilitaryTraining("We're not in counter mode, removing Katapeltes from our unitPicker.");
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeKatapeltes, 0.0);
   }
   return inCounterMode;
}

//==============================================================================
// chineseHumanUnitPicker
//==============================================================================
bool chineseHumanUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanUnitPickerSetup(upID);
   if (inCounterMode == false)
   {
      debugMilitaryTraining("We're not in counter mode, removing Ge Halberdier from our unitPicker.");
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeGeHalberdier, 0.0);
   }
   return inCounterMode;
}

//==============================================================================
// japaneseHumanUnitPicker
//==============================================================================
bool japaneseHumanUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanUnitPickerSetup(upID);
   if (inCounterMode == false)
   {
      debugMilitaryTraining("We're not in counter mode, removing Yari Spearman from our unitPicker.");
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeYariSpearman, 0.0);
   }
   return inCounterMode;
}

//==============================================================================
// aztecHumanUnitPicker
//==============================================================================
bool aztecHumanUnitPicker(int upID = -1)
{
   bool inCounterMode = sharedHumanUnitPickerSetup(upID);
   kbUnitPickSetPreferenceFactor(upID, cUnitTypeOtontin, 0.0); // Trained via Siege training.
   // We only have counter infantry in Classical, can only start excluding them from Heroic onwards.
   if (inCounterMode == false && kbPlayerGetAge(cMyID) >= cAge3)
   {
      debugMilitaryTraining("We're not in counter mode, removing Tlamanih Spearman and Coyote Warrior from our unitPicker.");
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeTlamanihSpearman, 0.0);
      kbUnitPickSetPreferenceFactor(upID, cUnitTypeCoyoteWarrior, 0.0);
   }
   return inCounterMode;
}

//==============================================================================
// humanMilitaryTraining
//==============================================================================
void humanMilitaryTraining(ref int totalAvailablePop, int originalTotalMilitaryPop = -1)
{
   static int humanUnitPickerID = -1;
   if (kbUnitPickGetIsIDValid(humanUnitPickerID) == false)
   {
      humanUnitPickerID = kbUnitPickCreate("Human military units");
   }
   
   // Update preferences in case something changed inbetween.
   switch (cMyCulture)
   {
      case cCultureGreek:
      {
         greekHumanUnitPicker(humanUnitPickerID);
         break;
      }
      case cCultureEgyptian:
      {
         egyptianHumanUnitPicker(humanUnitPickerID);
         break;
      }
      case cCultureNorse:
      {
         norseHumanUnitPicker(humanUnitPickerID);
         break;
      }
      case cCultureAtlantean:
      {
         atlanteanHumanUnitPicker(humanUnitPickerID);
         break;
      }
      case cCultureChinese:
      {
         chineseHumanUnitPicker(humanUnitPickerID);
         break;
      }
      case cCultureJapanese:
      {
         japaneseHumanUnitPicker(humanUnitPickerID);
         break;
      }
      case cCultureAztec:
      {
         aztecHumanUnitPicker(humanUnitPickerID);
         break;
      }
   }
   kbUnitPickRun(humanUnitPickerID);

   // Reset the building(s) we want to use for melee units.
   for (int i = gMaintainPlanHumanMeleeStartIndex; i < gNumHumanMeleeUnitTypes; i++)
   {
      gArmyUnitBuildings[i] = -1;
   }

   float totalFactor = 0.0;
   int puid = -1;
   int existingPlanID = -1;
   int planID = -1;
   int numFortresses = kbUnitCount(gFortressUnit, cMyID, cUnitStateAlive);
   int[] temp = new int(gNumHumanMeleeUnitTypes, -1);
   int[] unitPickResults = kbUnitPickGetResults(humanUnitPickerID);
   float[] unitPickResultFactors = kbUnitPickGetResultFactors(humanUnitPickerID);
   int numUnitPickerResults = unitPickResults.size();
   if (numUnitPickerResults <= 0)
   {
      aiEchoWarning("Human unit picker was unable to get any valid results!!!");
      return;
   }

   debugMilitaryTraining("Unit picker results:");
   for (int i = 0; i < numUnitPickerResults; i++)
   {
      debugMilitaryTraining("   # " + i + " = " + kbProtoUnitGetName(unitPickResults[i]) + ", factor = " +
         unitPickResultFactors[i] + ".");
   }

   // We have a unique case where Fortress buildings are quite expensive but also have the best units.
   // So upon aging up to Heroic it's very possible that we suddenly want to train those units but have 0 Fortresses, especially Eggy.
   // We need to guard against this by checking if the wanted unit has only a Fortress trainer, and if we have 0 of those -> remove.
   if (numFortresses == 0)
   {
      for (int i = numUnitPickerResults - 1; i >= 0; i--)
      {
         puid = unitPickResults[i];
         int[] trainers = kbProtoUnitGetTrainers(puid);
         if (trainers.size() == 1 && trainers[0] == gFortressUnit)
         {
            debugMilitaryTraining("Skipping the training of " + kbProtoUnitGetName(puid) + ", because it can only be trained in " +
               "a " + kbProtoUnitGetName(gFortressUnit) + " and we have none of those now.");
            unitPickResults.removeIndex(i);
            unitPickResultFactors.removeIndex(i);
            numUnitPickerResults--;
         }
      }
   }

   // We have a potential problem with Egyptians. Since the Migdol units are so strong the unit picker may select all of them.
   // We will often not have enough Migdols to support such a big need and it would be better if we utilised our Barracks as well.
   // We need 1 Migdol per 1 unit type we have to train from there. If we have 0 Migdols we would've removed them all already above.
   if (cMyCulture == cCultureEgyptian && numFortresses != 0 && numFortresses < 3)
   {
      int numAvailableMigdols = numFortresses;
      debugMilitaryTraining("We have " + numAvailableMigdols + " available Migdol Strongholds.");
      for (int i = gMaintainPlanHumanArcherStartIndex; i < gNumHumanArcherUnitTypes; i++)
      {
         planID = gArmyUnitMaintainPlans[i];
         if (aiPlanGetIsIDValid(planID) == true && aiPlanGetVariableInt(planID, cTrainPlanUnitType, 0) == cUnitTypeChariotArcher)
         {
            debugMilitaryTraining("Found that we are also planning to train Chariot Archers, this takes up 1 Migdol Stronghold.");
            numAvailableMigdols--;
            break;
         }
      }

      for (int i = 0; i < numUnitPickerResults; i++)
      {
         puid = unitPickResults[i];
         if (puid != cUnitTypeCamelRider && puid != cUnitTypeWarElephant)
         {
            continue;
         }
         if (numAvailableMigdols <= 0)
         {
            debugMilitaryTraining("Deleting " + kbProtoUnitGetName(puid) + " from our unit picker results due to a lack of " +
               "available Migdol Strongholds.");
            unitPickResults.removeIndex(i);
            unitPickResultFactors.removeIndex(i);
            numUnitPickerResults--;
            i--;
         }
         else
         {
            debugMilitaryTraining("Found that we're training  " + kbProtoUnitGetName(puid) + ", this takes up 1 Migdol Stronghold.");
            numAvailableMigdols--;
         }
      }
   }

   // We loop through all the units our human unit picker returned.
   // We see if we already have a maintain plan for said unit and save that information.
   // Also calculate the total factor.
   for (int i = 0; i < min(gNumHumanMeleeUnitTypes, numUnitPickerResults); i++)
   {
      totalFactor += unitPickResultFactors[i];
      puid = unitPickResults[i];
      for (int j = gMaintainPlanHumanMeleeStartIndex; j < gMaintainPlanHumanMeleeStartIndex + gNumHumanMeleeUnitTypes; j++)
      {
         existingPlanID = gArmyUnitMaintainPlans[j];
         // If we already have a plan for this unit, re-use it.
         if (aiPlanGetIsIDValid(existingPlanID) == true && puid == aiPlanGetVariableInt(existingPlanID, cTrainPlanUnitType, 0))
         {
            // Offset j for the temp array, since that array's index starts at 0.
            temp[j - gMaintainPlanHumanMeleeStartIndex] = existingPlanID;
            break;
         }
      }
   }

   // We destroy any maintain plans that are no longer needed because our unit picker preference has changed.
   for (int i = gMaintainPlanHumanMeleeStartIndex; i < gMaintainPlanHumanMeleeStartIndex + gNumHumanMeleeUnitTypes; i++)
   {
      // If temp[i] == -1 it means that we looped through our unit picker results and the plan at that index was training
      // a unit that we no longer want to train, thus we should destroy the plan.
      existingPlanID = gArmyUnitMaintainPlans[i];
      // Offset i for the temp array, since that array's index starts at 0.
      if (temp[i - gMaintainPlanHumanMeleeStartIndex] == -1 && aiPlanGetIsIDValid(existingPlanID) == true)
      {
         debugMilitaryTraining("Destroying plan " + aiPlanGetName(existingPlanID) + ", because we no longer want that unit.");
         aiPlanDestroy(existingPlanID);
      }
   }

   debugMilitaryTraining("Start creating new/updating existing train plans, total factor: " + totalFactor + ".");
   for (int i = 0; i < min(gNumHumanMeleeUnitTypes, numUnitPickerResults); i++)
   {
      puid = unitPickResults[i];

      // If we still have a maintain plan for this puid, re-use it.
      for (int j = 0; j < temp.size(); j++)
      {
         planID = temp[j];
         if (aiPlanGetIsIDValid(planID) == true && puid == aiPlanGetVariableInt(planID, cTrainPlanUnitType, 0))
         {
            break;
         }
         planID = -1;
      }

      int numberToMaintain = 0;
      int popCount = kbPlayerGetProtoStatInt(cMyID, puid, cProtoStatPopCost);
      if (popCount > 0)
      {
         // Round up, never get into a situation where we don't train enough.
         numberToMaintain = ceil(((unitPickResultFactors[i] / totalFactor) * totalAvailablePop) / popCount);
         debugMilitaryTraining("Unit: " + kbProtoUnitGetName(puid) + ", number to maintain: " + numberToMaintain + ".");
         debugMilitaryTraining("Factor: " + unitPickResultFactors[i] + ", popCount: " + popCount + ".");
      }
      else
      {
         aiEchoWarning("Unit: " + kbProtoUnitGetName(puid) + " has no population cost, it can't be used in standard military training.");
         continue;
      }

      if (planID < 0)
      {
         planID = createSimpleMaintainPlan(puid, numberToMaintain, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1, -1, true);
         aiPlanSetName(planID, planID + ": Land military maintain: " + numberToMaintain + " " + kbProtoUnitGetName(puid));
         debugMilitaryTraining("Creating maintain plan for " + kbProtoUnitGetName(puid) + ".");
      }
      else
      {
         // It could be that this plan is already perfectly set up, then don't do anything.
         if (aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0) == numberToMaintain)
         {
            debugMilitaryTraining("Existing maintain plan for " + kbProtoUnitGetName(puid) + " doesn't require changes.");
         }
         else
         {
            aiPlanSetVariableInt(planID, cTrainPlanNumberToMaintain, 0, numberToMaintain);
            aiPlanSetName(planID, planID + ": Land military maintain: " + numberToMaintain + " " + kbProtoUnitGetName(puid));
            debugMilitaryTraining("Adjusting existing maintain plan for " + kbProtoUnitGetName(puid) + ".");
         }
      }
      
      // Finally update our real array.
      gArmyUnitMaintainPlans[i + gMaintainPlanHumanMeleeStartIndex] = planID;

      int trainBuildingPUID = -1;
      int numMilitaryBuildings = gMilitaryBuildings.size();
      for (int j = 0; j < numMilitaryBuildings; j++)
      {
         int buildingPUID = gMilitaryBuildings[j];
         if (kbProtoUnitCanTrain(buildingPUID, puid) == true)
         {
            trainBuildingPUID = buildingPUID;
            break;
         }
      }

      gArmyUnitBuildings[i + gMaintainPlanHumanMeleeStartIndex] = trainBuildingPUID;
   }

   if (cMyCulture == cCultureAztec && checkStrategyFlag(cStrategyFlagBuildTraps) == true)
   {
      // We want to build traps but may not have chosen to maintain the right human units that can build them.
      // Just train 1 of the Tlamanih Spearman so our traps aren't deadlocked if need be.
      if (kbUnitCount(cUnitTypeTlamanihSpearman, cMyID, cUnitStateAlive) != 0 ||
          kbUnitCount(cUnitTypeTequihuaArcher, cMyID, cUnitStateAlive) != 0)
      {
         return;
      }
      if (aiPlanGetNumberByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, cUnitTypeTlamanihSpearman) != 0 ||
          aiPlanGetNumberByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, cUnitTypeTequihuaArcher) != 0)
      {
         return;
      }
      debugMilitaryTraining("We're Aztec, want to build traps, but aren't maintaining any human units we can do so, force one.");
      // Just force train 1 Tlamanih Spearman, easy.
      createSimpleTrainPlan(cUnitTypeTlamanihSpearman, 1, gLandAreaGroupID, gMilitaryTrainingCategoryID,);
   }
}

//==============================================================================
// dryadTraining
//==============================================================================
void dryadTraining(int originalTotalMilitaryPop = -1)
{
   int hesperidesTreeID = getUnit(cUnitTypeHesperidesTree);
   if (hesperidesTreeID == -1)
   {
      debugMilitaryTraining("We have no Hesperides Tree in our possession, can't train Dryads.");
      return;
   }
   int currentDryadCount = kbUnitCount(cUnitTypeDryad, cMyID, cUnitStateAlive);
   int[] planIDs = aiPlanGetIDsByTypeAndVariableIntValue(cPlanTrain, cTrainPlanUnitType, cUnitTypeDryad);
   for (int j = 0; j < planIDs.size(); j++)
   {
      currentDryadCount += aiPlanGetVariableInt(planIDs[j], cTrainPlanNumberToTrain, 0) -
                           aiPlanGetVariableInt(planIDs[j], cTrainPlanNumberTrained, 0);
   }
   debugMilitaryTraining("dryadTraining - currentDryadCount: " + currentDryadCount + ".");

   int wantedDryadCount = originalTotalMilitaryPop / 10;
   if (cDifficultyCurrent <= cDifficultyModerate && wantedDryadCount > 1)
   {
      wantedDryadCount = 1;
   }
   debugMilitaryTraining("dryadTraining - wantedDryadCount: " + wantedDryadCount + ".");
   int dryadCountToTrain = max(0, wantedDryadCount - currentDryadCount);
   int buildLimit = kbPlayerGetProtoStatInt(cMyID, cUnitTypeDryad, cProtoStatBuildLimit);
   if (currentDryadCount + dryadCountToTrain > buildLimit)
   {
      dryadCountToTrain = buildLimit - currentDryadCount;
      debugMilitaryTraining("dryadTraining - needing to clamp dryadCountToTrain because of BL to: " + dryadCountToTrain + ".");
   }
   if (dryadCountToTrain <= 0)
   {
      debugMilitaryTraining("dryadTraining - dryadCountToTrain: " + dryadCountToTrain + ", do nothing.");
   }
   else
   {
      // We must set the buildingPUID because the Hesperides Tree can't be found by the automatic train plan systems.
      createSimpleTrainPlan(cUnitTypeDryad, dryadCountToTrain, gLandAreaGroupID, gMilitaryTrainingCategoryID, 50, -1,
         cUnitTypeHesperidesTree);
   }
}

///////////////////////////////
const int cMilitaryManagerStateBeginAndHero = 0;
const int cMilitaryManagerStateMyth = 1;
const int cMilitaryManagerStateSiege = 2;
const int cMilitaryManagerStateHumanArcher = 3;
const int cMilitaryManagerStateHumanAndDryad = 4;
//==============================================================================
/* militaryManager
   Manages how much military population should be trained.
   And then makes sure that actually gets trained.
*/
//==============================================================================
rule militaryManager
inactive
group defaultClassicalRules
minInterval 30
priority 70
{
   static int currentState = cMilitaryManagerStateBeginAndHero;
   if (checkStrategyFlag(cStrategyFlagAutoTrainMilitaryUnits) == false)
   {
      currentState = cMilitaryManagerStateBeginAndHero;
      // Destroy all plans.
      for (int i = 0; i < gMythUnitTrainPlans.size(); i++)
      {
         int planID = gMythUnitTrainPlans[i];
         if (aiPlanGetIsIDValid(planID) == true)
         {
            aiPlanDestroy(planID);
         }
      }
      gMythUnitTrainPlans.clear();
      for (int i = 0; i < gNumTotalArmyUnitTypes; i++)
      {
         int planID = gArmyUnitMaintainPlans[i];
         if (aiPlanGetIsIDValid(planID) == true)
         {
            aiPlanDestroy(planID);
            gArmyUnitMaintainPlans[i] = -1;
         }
         gArmyUnitBuildings[i] = -1;
      }
      return;
   }

   debugMilitaryTraining("--- Running Rule militaryManager. ---");
   static int originalTotalMilitaryPop = 0;
   static int currentMilitaryPop = 0;
   static int remainingMilitaryPop = 0;

   switch (currentState)
   {
      case cMilitaryManagerStateBeginAndHero:
      {
         debugMilitaryTraining("Current maintain plans:");
         for (int i = 0; i < gNumTotalArmyUnitTypes; i++)
         {
            int planID = gArmyUnitMaintainPlans[i];
            if (aiPlanGetIsIDValid(planID) == true)
            {
               debugMilitaryTraining("   " + aiPlanGetName(planID));
            }
            else
            {
               debugMilitaryTraining("   Currently Invalid plan.");
            }
         }

         if (gOverrideMaxMilitaryPop >= cUnlimitedMilitaryPop)
         {
            debugMilitaryTraining("Override - Setting our military pop to: " + gOverrideMaxMilitaryPop + ", -1 means unlimited.");
            aiSetMilitaryPop(gOverrideMaxMilitaryPop);
         }
         else
         {
            // Updating of military pop to have realistic maintain numbers.
            bool unlockMilitaryPop = false;
            int currentLandEconomicPop = aiGetCurrentEconomyPop();
            int currentNavalEconomicPop = aiGetCurrentNavalEconomyPop();
            int militaryPop = (currentLandEconomicPop + currentNavalEconomicPop) * gMilitaryToEcoRatio;
            debugMilitaryTraining("Initial data: currentLandEconomicPop = " + currentLandEconomicPop + ", currentNavalEconomicPop = " +
               currentNavalEconomicPop + ", calculated militaryPop = " + militaryPop + ".");

            // If we're >= Titan and Classic and have > 30% of our wanted economic pop we unlock our military pop.
            if (cDifficultyCurrent >= cDifficultyTitan && kbPlayerGetAge(cMyID) >= cAge2)
            {
               // Deliberately ignoring Fishing Ships here since if we can't get back on the water it shouldn't hinder unlocking land army.
               float percentageLandEconomyPopAlive = 0.0;
               int maxLandEconomicPop = aiGetEconomyPop();
               if (maxLandEconomicPop > 0)
               {
                  percentageLandEconomyPopAlive = xsIntToFloat(currentLandEconomicPop) / xsIntToFloat(maxLandEconomicPop);
                  debugMilitaryTraining("We have " + percentageLandEconomyPopAlive + " percentage land economy pop alive (" +
                     currentLandEconomicPop + "/" + maxLandEconomicPop + "). We need at least 90 percent to unlock our militaryPop.");
               }
               if (percentageLandEconomyPopAlive >= 0.3)
               {
                  unlockMilitaryPop = true;
                  debugMilitaryTraining("Unlocking military population because we're >= Mythic and nearly have >= 90 percent " +
                     " of our wanted land economic pop alive.");
               }
            }
            if (haveExcessResourceAmount(500, cAllResources) == true)
            {
               unlockMilitaryPop = true;
               debugMilitaryTraining("Unlocking military population because we have 500 excess in all resources (excluding favor).");
            }
            if (unlockMilitaryPop == true)
            {
               militaryPop = kbPlayerGetPopCap(cMyID) - currentLandEconomicPop - currentNavalEconomicPop;
               militaryPop = max(0, militaryPop); // This is needed because we could lose a lot of Houses and hit this.
               debugMilitaryTraining("We have a pop cap of " + kbPlayerGetPopCap(cMyID) + ", total economy pop: " + 
                  (currentLandEconomicPop + currentNavalEconomicPop) + ", resulting in a militaryPop of " + militaryPop + ".");
               if (cDifficultyCurrent <= cDifficultyHard)
               {
                  debugMilitaryTraining("Seeing if we need to take gMaxMilitaryPop (" + gMaxMilitaryPop + 
                     ") into account because we're on a lower difficulty.");
                  militaryPop = min(gMaxMilitaryPop, militaryPop); // Cap this appropriately again.
               }
            }
            else
            {
               debugMilitaryTraining("Not unlocking our military pop numbers, didn't meet the requirements.");
            }
            gMilitaryPopIsUnlocked = unlockMilitaryPop;
            aiSetMilitaryPop(militaryPop);
            debugMilitaryTraining("Setting our military pop to: " + militaryPop + ".");
         }

         originalTotalMilitaryPop = aiGetMilitaryPop();
         if (originalTotalMilitaryPop == 0)
         {
            debugMilitaryTraining("We are allowed 0 max military pop, can't train anything now.");
            return;
         }
         currentMilitaryPop = aiGetCurrentMilitaryPop();
         if (originalTotalMilitaryPop <= currentMilitaryPop)
         {
            debugMilitaryTraining("We are allowed " + originalTotalMilitaryPop + " max military pop but already have " +
               currentMilitaryPop + " military pop, not training any more now.");
            return;
         }
         remainingMilitaryPop = originalTotalMilitaryPop;
         debugMilitaryTraining("*** remainingMilitaryPop before any training: " + remainingMilitaryPop + ". ***");

         if (cPersonalityCurrent == cPersonalityHumanoid)
         {
            debugMilitaryTraining("We're humanoid, not training any heroes.");
         }
         else
         {
            heroMilitaryTraining(remainingMilitaryPop, originalTotalMilitaryPop);
         }
         debugMilitaryTraining("*** remainingMilitaryPop after hero training: " + remainingMilitaryPop + ". ***");
         break;
      }

      case cMilitaryManagerStateMyth:
      {
         debugMilitaryTraining("*** remainingMilitaryPop for myth training: " + remainingMilitaryPop + ". ***");
         if (cPersonalityCurrent == cPersonalityHumanoid)
         {
            debugMilitaryTraining("We're humanoid, not training any myth units.");
         }
         else
         {
            mythMilitaryTraining(remainingMilitaryPop, originalTotalMilitaryPop);
         }
         debugMilitaryTraining("*** remainingMilitaryPop after myth training: " + remainingMilitaryPop + ". ***");
         break;
      }

      case cMilitaryManagerStateSiege:
      {
         debugMilitaryTraining("*** remainingMilitaryPop for siege training: " + remainingMilitaryPop + ". ***");
         siegeMilitaryTraining(remainingMilitaryPop, originalTotalMilitaryPop);
         debugMilitaryTraining("*** remainingMilitaryPop after siege training: " + remainingMilitaryPop + ". ***");
         break;
      }

      case cMilitaryManagerStateHumanArcher:
      {
         debugMilitaryTraining("*** remainingMilitaryPop for human archer training: " + remainingMilitaryPop + ". ***");
         humanArcherMilitaryTraining(remainingMilitaryPop, originalTotalMilitaryPop);
         debugMilitaryTraining("*** remainingMilitaryPop after human archer training: " + remainingMilitaryPop + ". ***");
         break;
      }

      case cMilitaryManagerStateHumanAndDryad:
      {
         debugMilitaryTraining("*** remainingMilitaryPop for human training: " + remainingMilitaryPop + ". ***");
         humanMilitaryTraining(remainingMilitaryPop, originalTotalMilitaryPop);
         if (cPersonalityCurrent == cPersonalityHumanoid)
         {
            debugMilitaryTraining("We're humanoid, not training any Dryads.");
         }
         else
         {
            dryadTraining(originalTotalMilitaryPop);
         }
         // Make sure we instantly react to updated human maintain plans.
         xsRuleIgnoreIntervalOnce("militaryBuildingManager");
         break;
      }
   }

   if (currentState == cMilitaryManagerStateHumanAndDryad)
   {
      // Reset.
      currentState = cMilitaryManagerStateBeginAndHero;
   }
   else
   {
      // Advance through the options.
      currentState++;
      xsRuleIgnoreIntervalOnce("militaryManager");
   }
}

//==============================================================================
// forceMigdolStrongholdUnits
// The problem we have is that Migdol units are much more expensive than Barracks units.
// Thus we run the risk of our Barracks units draining all our resources before we can stack up enough for a Migdol unit.
// To try and work around that issue we start putting a train delay on Barracks plans if they have already trained a lot of units.
//==============================================================================
rule forceMigdolStrongholdUnits
inactive
group defaultHeroicRules
minInterval 10
{
   if (cMyCulture != cCultureEgyptian)
   {
      xsDisableRule("forceMigdolStrongholdUnits");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoTrainMilitaryUnits) == false)
   {
      return;
   }

   debugMilitaryTraining("--- Running Rule forceMigdolStrongholdUnits. ---");

   const float threshold = 0.50;
   const int trainDelay = 3;
   bool haveMigdolPlanBelow50 = false;
   int[] plansThatNeedTrainDelay = new int(0, 0);
   int[] barracksPlans = new int(0, 0);
   for (int i = 0; i < gMaintainPlanHeroStartIndex; i++)
   {
      int planID = gArmyUnitMaintainPlans[i];
      if (aiPlanGetIsIDValid(planID) == false)
      {
         continue;
      }
      int puid = aiPlanGetVariableInt(planID, cTrainPlanUnitType, 0);
      int numToMaintain = aiPlanGetVariableInt(planID, cTrainPlanNumberToMaintain, 0);
      if (numToMaintain == 0)
      {
         continue;
      }
      int numAlive = kbUnitCount(puid, cMyID, cUnitStateAlive);
      if (kbProtoUnitCanTrain(cUnitTypeBarracks, puid) == true)
      {
         barracksPlans.add(planID);
         if (xsIntToFloat(numAlive) / xsIntToFloat(numToMaintain) >= threshold)
         {
            // This plan has enough units alive and doesn't currently have a train delay yet.
            if (aiPlanGetVariableBool(planID, cTrainPlanUseMultipleBuildings, 0) == true)
            {
               debugMilitaryTraining(aiPlanGetName(planID) +
                  " has more than 50 percent of its wanted units alive and could be delayed.");
               plansThatNeedTrainDelay.add(planID);
            }
         }
         else
         {
            // This plan does not have enough units alive (anymore), remove a potential train delay now.
            if (aiPlanGetVariableBool(planID, cTrainPlanUseMultipleBuildings, 0) == false)
            {
               debugMilitaryTraining("Removing the train delay from " + aiPlanGetName(planID) +
                  " because it's below the threshold.");
               aiPlanSetVariableInt(planID, cTrainPlanFrequency, 0, -1);
               aiPlanSetVariableBool(planID, cTrainPlanUseMultipleBuildings, 0, true);
            }
         }
      }
      // This is a Migdol unit, just find one that would be below the threshold that's enough.
      else if (haveMigdolPlanBelow50 == false)
      {
         if (xsIntToFloat(numAlive) / xsIntToFloat(numToMaintain) < threshold)
         {
            debugMilitaryTraining(aiPlanGetName(planID) + " has fewer than 50 percent of its wanted units alive.");
            haveMigdolPlanBelow50 = true;
         }
      }
   }

   // Set all train delays on plans that don't have it already.
   if (haveMigdolPlanBelow50 == true)
   {
      for (int i = 0; i < plansThatNeedTrainDelay.size(); i++)
      {
         debugMilitaryTraining("Setting a train delay on " + aiPlanGetName(plansThatNeedTrainDelay[i]) + ".");
         aiPlanSetVariableInt(plansThatNeedTrainDelay[i], cTrainPlanFrequency, 0, trainDelay);
         aiPlanSetVariableBool(plansThatNeedTrainDelay[i], cTrainPlanUseMultipleBuildings, 0, false);
      }
   }
   // No Migdol plans that need prio? Remove all train delays.
   else
   {
      for (int i = 0; i < barracksPlans.size(); i++)
      {
         if (aiPlanGetVariableBool(barracksPlans[i], cTrainPlanUseMultipleBuildings, 0) == false)
         {
            debugMilitaryTraining("Removing the train delay from " + aiPlanGetName(barracksPlans[i]) + ".");
            aiPlanSetVariableInt(barracksPlans[i], cTrainPlanFrequency, 0, -1);
            aiPlanSetVariableBool(barracksPlans[i], cTrainPlanUseMultipleBuildings, 0, true);
         }
      }
   }
}

//==============================================================================
// forceSiegeUnits
// Siege units are expensive, especially early game. Play around with priorities to forcibly train them for our personality trait.
//==============================================================================
rule forceSiegeUnits
inactive
group defaultHeroicRules
minInterval 10
{
   if (cPersonalityCurrent != cPersonalitySieger)
   {
      xsDisableRule("forceSiegeUnits");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagAutoTrainMilitaryUnits) == false)
   {
      return;
   }

   debugMilitaryTraining("--- Running Rule forceSiegeUnits. ---");

   const int threshold = 2;
   int prio = 50;
   if (gDefensivelyOverrun == false)
   {
      int numExistingSiegeUnits = kbUnitCount(cUnitTypeAbstractSiegeWeapon, cMyID, cUnitStateAlive);
      #if (cMyCulture == cCultureAtlantean)
      numExistingSiegeUnits -= kbUnitCount(cUnitTypeCheiroballista, cMyID, cUnitStateAlive);
      numExistingSiegeUnits += kbUnitCount(cUnitTypeDestroyer, cMyID, cUnitStateAlive);
      numExistingSiegeUnits += kbUnitCount(cUnitTypeDestroyerHero, cMyID, cUnitStateAlive);
      #endif
      debugMilitaryTraining("We currently have " + numExistingSiegeUnits + " siege units alive, if it's below " + threshold +
         " we increase the priority of all siege maintain plans to 51 if they don't have it already, otherwise priority must be 50.");
      if (numExistingSiegeUnits < threshold)
      {
         prio = 51;
      }
   }
   for (int i = gMaintainPlanSiegeStartIndex; i < gNumTotalArmyUnitTypes; i++)
   {
      int planID = gArmyUnitMaintainPlans[i];
      if (aiPlanGetIsIDValid(planID) == false)
      {
         continue;
      }
      if (aiPlanGetPriority(planID) != prio)
      {
         aiPlanSetPriority(planID, prio);
         debugMilitaryTraining("Adjusting priority on " + aiPlanGetName(planID) + " to " + prio + ".");
      }
   }
}

//==============================================================================
// deleteMythUnits
// Humanoid personality uses this to get rid of those unwanted myth units.
//==============================================================================
rule deleteMythUnits
inactive
group defaultArchaicRules
minInterval 1
{
   if (cPersonalityCurrent != cPersonalityHumanoid)
   {
      xsDisableRule("deleteMythUnits");
      return;
   }

   debugMilitaryTraining("--- Running Rule deleteMythUnits. ---");
   int queryID = useSimpleUnitQuery(cUnitTypeMythUnit, cMyID, cUnitStateAlive);
   int numResults = kbUnitQueryExecute(queryID);
   for (int i = 0; i < numResults; i++)
   {
      int unitID = kbUnitQueryGetResult(queryID, i);
      debugMilitaryTraining("Deleting myth unit: " + kbProtoUnitGetName(kbUnitGetProtoUnitID(unitID)) + " " + unitID + ".");
      aiTaskDeleteUnit(unitID);
   }
}

//==============================================================================
// villagerToBerserk
// If we have no more builders as Norse we need to check if we can still train some, and otherwise convert a Villager/Dwarf.
//==============================================================================
rule villagerToBerserk
inactive
group defaultArchaicRules
minInterval 30
{
   if (cMyCulture != cCultureNorse)
   {
      xsDisableRule("villagerToBerserk");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagConvertVillagerToBerserk) == false)
   {
      return;
   }
   // Wait for starting units to spawn.
   if (xsGetTime() < 5)
   {
      return;
   }
   debugMilitaryTraining("--- Running Rule villagerToBerserk. ---");

   if (aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechVillagerNorseToBerserk) >= 0 ||
       aiPlanGetIDByTypeAndVariableIntValue(cPlanResearch, cResearchPlanTechID, cTechVillagerDwarfToBerserk) >= 0)
   {
      debugMilitaryTraining("Already have a research plan going to convert a Villager to a Berserk.");
      return;
   }

   // We can't just query for existing train plans. Because what if we have a train plan for Throwing Axeman but have no Longhouses
   // left, then we would wrongly early out. Due to the interval of this rule and the prio 100 of the train plans the chance
   // of creating duplicate train plans isn't high, and also wouldn't be a big problem...

   // Still have one?
   if (kbUnitCount(cUnitTypeLogicalTypeNorseSoldierThatBuilds, cMyID, cUnitStateABQ) >= 1)
   {
      debugMilitaryTraining("Still have an alive builder.");
      return;
   }

   // We have no builders alive but maybe do have buildings to create them.
   // We most likely already have a maintain plan lying around for a builder due to automatic military etc...
   // But still we create a new train plan with highest prio to at least crank out 1 builder.

   // Can still train one? Here we need to do a lot of checks because different builders can be built from different buildings.
   // Don't do a gLandAreaGroupID check here or we can get stuck on Migration.
   int tcCount = kbUnitCount(cUnitTypeAbstractSocketedTownCenter, cMyID, cUnitStateAlive);
   int villageCenterCount =  kbUnitCount(cUnitTypeVillageCenter, cMyID, cUnitStateAlive);
   int longhouseCount = kbUnitCount(cUnitTypeLonghouse, cMyID, cUnitStateAlive);
   int greatHallCount = kbUnitCount(cUnitTypeGreatHall, cMyID, cUnitStateAlive);
   int templeCount = kbUnitCount(cUnitTypeTemple, cMyID, cUnitStateAlive);
   int hillFortCount = kbUnitCount(cUnitTypeHillFort, cMyID, cUnitStateAlive);
   if (tcCount + villageCenterCount + longhouseCount >= 1) // Chose Berserk from TC/VC/Longhouse.
   {
      createSimpleTrainPlan(cUnitTypeBerserk, 1, -1, gMilitaryTrainingCategoryID, 100);
      debugMilitaryTraining("Training a Berserk with highest prio.");
      return;
   }
   if (longhouseCount >= 1) // Chose TA from Longhouse.
   {
      createSimpleTrainPlan(cUnitTypeThrowingAxeman, 1, -1, gMilitaryTrainingCategoryID, 100);
      debugMilitaryTraining("Training a Throwing Axeman with highest prio.");
      return;
   }
   if (templeCount + greatHallCount >= 1) // Chose Hersir from Temple/GreatHall.
   {
      createSimpleTrainPlan(cUnitTypeHersir, 1, -1, gMilitaryTrainingCategoryID, 100);
      debugMilitaryTraining("Training a Hersir with highest prio.");
      return;
   }
   if (hillFortCount >= 1)
   {
      createSimpleTrainPlan(cUnitTypeThrowingAxeman, 1, -1, gMilitaryTrainingCategoryID, 100);
      debugMilitaryTraining("Training a Huskarl with highest prio.");
      return;
   }

   // Actually no buildings left to train with either, panic!!!

   bool dwarf = false;
   int toTransformID = getUnit(cUnitTypeVillagerNorse);
   if (toTransformID == -1)
   {
      toTransformID = getUnit(cUnitTypeVillagerDwarf);
      dwarf = true;
      if (toTransformID == -1)
      {
         debugMilitaryTraining("Can't find any Gatherer/Dwarf to transform.");
         return;
      }
   }

   // Make sure this Villager doesn't linger in a gather plan.
   aiPlanRemoveUnitFromAllPlans(toTransformID);
   if (dwarf == true)
   {
      createSimpleResearchPlanSpecificResearcher(cTechVillagerDwarfToBerserk, toTransformID, 100, true);
   }
   else
   {
      createSimpleResearchPlanSpecificResearcher(cTechVillagerNorseToBerserk, toTransformID, 100, true);
   }
}