//==============================================================================
/* godpowers.xs

   This file contains all logic for the management of god powers.

*/

//==============================================================================
// isUnneededGodPowerDueToResources
//
// Returns true only when a resource-granting god power should be SKIPPED
// (its added resources would be wasted). That is only ever the case on
// INFINITE starting resources -- on any finite setting (standard / low /
// medium / high) the powers remain valuable and must NOT be skipped.
//
// NOTE: the operator on cStartingResourcesCurrent is intentionally `!=`.
// The base Retold AI ships this check as `==`, which inverts the function's
// semantics and made the AI skip GreatHunt / Lure / Rain / Prosperity /
// DwarvenMine / GaiaForest / PlentyVault / PeachBlossomSpring /
// ProsperousSeeds on every standard game (ISSUE-03 from the 2026-06-19
// playtest). This overlay corrects the inversion. Do NOT "fix" the operator
// back to `==` -- that re-introduces ISSUE-03.
//==============================================================================
bool isUnneededGodPowerDueToResources(int protoPowerID = -1)
{
   // Finite resources: resource powers are useful, do NOT skip.
   if (cStartingResourcesCurrent != cStartingResourcesInfinite)
   {
      return false;
   }
   // Infinite resources: skip resource-granting powers (their output is worthless).
   if (protoPowerID == cProtoPowerLure ||
       protoPowerID == cProtoPowerPlentyVault ||
       protoPowerID == cProtoPowerRain ||
       protoPowerID == cProtoPowerProsperity ||
       protoPowerID == cProtoPowerDwarvenMine ||
       protoPowerID == cProtoPowerGreatHunt ||
       protoPowerID == cProtoPowerGaiaForest ||
       protoPowerID == cProtoPowerThePeachBlossomSpring ||
       protoPowerID == cProtoPowerProsperousSeeds)
   {
      return true;
   }
   return false;
}

//==============================================================================
// setUpGodPowerPlan
//==============================================================================
bool setUpGodPowerPlan(int protoPowerID = -1, ref int planID)
{
   // Unsupported god powers for now.
   if (cPersonalityCurrent == cPersonalityHumanoid)
   {
      debugGodPowers("We're humanoid, we don't use god powers!");
      return true;
   }
   if (isUnsupportedGodPower(protoPowerID) == true)
   {
      debugGodPowers("Unsupported god power " + kbGodPowerGetName(protoPowerID) + ", exiting early.");
      return true;
   }
   if (isUnneededGodPowerDueToResources(protoPowerID) == true)
   {
      debugGodPowers("We're on infinite resources and " + kbGodPowerGetName(protoPowerID) + " provides resources, " +
         "have no use for it, exiting early.");
      return true;
   }

   planID = aiPlanCreate("GodPower " + kbGodPowerGetName(protoPowerID), cPlanGodPower, -1, gGodpowersCategoryID);
   aiPlanSetVariableInt(planID, cGodPowerPlanPowerProtoID, 0, protoPowerID);
   aiPlanSetEventHandler(planID, cPlanEventStateChange, "godPowerStateChangeHandler");
   
   switch (protoPowerID)
   {
      case cProtoPowerBolt:
      case cProtoPowerSentinel:
      case cProtoPowerLure:
      case cProtoPowerWither:
      case cProtoPowerRestoration:
      case cProtoPowerCeaseFire:
      case cProtoPowerPestilence:
      case cProtoPowerArcadianMeadow:
      case cProtoPowerBronze:
      case cProtoPowerCurse:
      case cProtoPowerUnderworldPassage:
      case cProtoPowerCommunalHearth:
      case cProtoPowerPlentyVault:
      case cProtoPowerLightningStorm:
      case cProtoPowerEarthquake:
      case cProtoPowerUnderworldInvasion:
      {
         setupGreekGodPowerPlan(planID, protoPowerID);
         break;
      }
      case cProtoPowerVision:
      case cProtoPowerRain:
      case cProtoPowerProsperity:
      case cProtoPowerEclipse:
      case cProtoPowerShiftingSands:
      case cProtoPowerPlagueOfSerpents:
      case cProtoPowerLocustSwarm:
      case cProtoPowerAncestors:
      case cProtoPowerCitadel:
      case cProtoPowerSonOfOsiris:
      case cProtoPowerMeteor:
      case cProtoPowerTornado:
      {
         setupEgyptianGodPowerPlan(planID, protoPowerID);
         break;
      }
      case cProtoPowerDwarvenMine:
      case cProtoPowerSpy:
      case cProtoPowerGreatHunt:
      case cProtoPowerGullinbursti:
      case cProtoPowerForestFire:
      case cProtoPowerHealingSpring:
      case cProtoPowerUndermine:
      case cProtoPowerAsgardianBastion:
      case cProtoPowerFrost:
      case cProtoPowerFlamingWeapons:
      case cProtoPowerWalkingWoods:
      case cProtoPowerTempest:
      case cProtoPowerRagnarok:
      case cProtoPowerFimbulwinter:
      case cProtoPowerNidhogg:
      case cProtoPowerInferno:
      {
         setupNorseGodPowerPlan(planID, protoPowerID);
         break;
      }
      case cProtoPowerDeconstruction:
      case cProtoPowerShockwave:
      case cProtoPowerGaiaForest:
      case cProtoPowerCarnivora:
      case cProtoPowerValor:
      case cProtoPowerSpiderLair:
      case cProtoPowerTraitor:
      case cProtoPowerChaos:
      case cProtoPowerHesperidesTree:
      case cProtoPowerVortex:
      case cProtoPowerTartarianGate:
      case cProtoPowerImplode:
      {
         setupAtlanteanGodPowerPlan(planID, protoPowerID);
         break;
      }
      case cProtoPowerCreation:
      case cProtoPowerThePeachBlossomSpring:
      case cProtoPowerProsperousSeeds:
      case cProtoPowerVenomBeast:
      case cProtoPowerEarthWall:
      case cProtoPowerVanish:
      case cProtoPowerForestProtection:
      case cProtoPowerDroughtLand:
      case cProtoPowerLightningWeapons:
      case cProtoPowerGreatFlood:
      case cProtoPowerYinglongsWrath:
      case cProtoPowerBlazingPrairie:
      {
         setupChineseGodPowerPlan(planID, protoPowerID);
         break;
      }
      case cProtoPowerSolarShield:
      case cProtoPowerKusanagi:
      case cProtoPowerNewMoon:
      case cProtoPowerShrineOfTheHunt:
      case cProtoPowerSwampland:
      case cProtoPowerGoshinboku:
      case cProtoPowerShogun:
      case cProtoPowerSmitingGust:
      case cProtoPowerThunderBurst:
      case cProtoPowerSacredGate:
      case cProtoPowerDragonTyphoon:
      case cProtoPowerDivineSlash:
      {
         setupJapaneseGodPowerPlan(planID, protoPowerID);
         break;
      }
      case cProtoPowerBloodPact:
      case cProtoPowerTailwind:
      case cProtoPowerObsidianMirror:
      case cProtoPowerLullaby:
      case cProtoPowerInfestation:
      case cProtoPowerAgaveBloom:
      case cProtoPowerEarthMonster:
      case cProtoPowerStarfall:
      case cProtoPowerPurge:
      case cProtoPowerCorruptedGround:
      case cProtoPowerMonolithOfTlaloc:
      case cProtoPowerVolcano:
      {
         setupAztecGodPowerPlan(planID, protoPowerID);
         break;
      }
      default:
      {
         aiEchoWarning("setUpGodPowerPlan - Received an unrecognized protoPowerID: " + protoPowerID + ", name: " +
            kbGodPowerGetName(protoPowerID) + ".");
         // Prevent endless error messages, destroy the plan and return true.
         aiPlanDestroy(planID);
         return true;
      }
   }

   if (aiPlanGetVariableBool(planID, cGodPowerPlanRequiresCombatPlan, 0) == true)
   {
      debugGodPowers(aiPlanGetName(planID) + " requires a combat plan, seeing if we have any that we can link.");
      int[] plans = aiPlanGetIDsByType(cPlanAttack);
      int[] defendPlans = aiPlanGetIDsByType(cPlanDefend);
      int[] explorePlans = aiPlanGetIDsByType(cPlanExplore);
      for (int i = plans.size() - 1; i >= 0; i--)
      {
         // No reinforcement plans.
         if (aiPlanGetParentID(plans[i]) != -1)
         {
            plans.removeIndex(i);
         }
      }
      for (int i = 0; i < defendPlans.size(); i++)
      {
         plans.add(defendPlans[i]);
      }
      for (int i = 0; i < explorePlans.size(); i++)
      {
         // Only aggro scout plans.
         if (aiPlanGetVariableBool(explorePlans[i], cExplorePlanAggressiveScouts, 0) == true)
         {
            plans.add(explorePlans[i]);
         }
      }
      if (aiPlanGetUserVariableIndex(planID, "Not Naval") != -1)
      {
         removeNavalPlansFromArray(plans);
      }
      if (aiPlanGetUserVariableIndex(planID, "Not Titan") != -1)
      {
         removeTitanPlansFromArray(plans);
      }
      aiPlanSetNumberVariableValues(planID, cGodPowerPlanCombatPlanID, plans.size());
      for (int i = 0; i < plans.size(); i++)
      {
         aiPlanSetVariableInt(planID, cGodPowerPlanCombatPlanID, i, plans[i]);
         debugGodPowers("   Added " + aiPlanGetName(plans[i]) + ".");
      }
   }

   debugGodPowers("Created a god power plan for: " + kbGodPowerGetName(protoPowerID) + ".");
   return true;
}

//==============================================================================
// useUnusedGodPowers
// Create god power plans for each god power that we have in the bank.
//==============================================================================
void useUnusedGodPowers()
{
   // Use all god powers we have a charge for but no plan for.
   for (int i = 0; i < 4; i++)
   {
      int protoPowerID = kbGodPowerGetIDInSlot(i, cMyID);
      if (protoPowerID == -1 || isUnsupportedGodPower(protoPowerID) == true)
      {
         continue;
      }
      int originalProtoPowerID = protoPowerID;
      // Obsidian Mirror will identify as something else potentially, but we gotta ask some other syscalls with ObsidianMirror.
      if (cMyCiv == cCivTezcatlipoca && i == 0)
      {
         originalProtoPowerID = cProtoPowerObsidianMirror;
      }
      // No charges obviously we can't make a plan. And if it's still on cooldown we can't cast it either.
      debugGodPowers("We have " + kbGodPowerGetNumCharges(originalProtoPowerID, cMyID) + " " + kbGodPowerGetName(protoPowerID) + " charges.");
      if (kbGodPowerGetNumCharges(originalProtoPowerID, cMyID) <= 0 || kbGodPowerIsOnCooldown(protoPowerID, cMyID) == true)
      {
         continue;
      }
      // Don't stack plans, doesn't work for 95% of the GPs.
      if (aiPlanGetNumberByTypeAndVariableIntValue(cPlanGodPower, cGodPowerPlanPowerProtoID, originalProtoPowerID) > 0)
      {
         continue;
      }
      debugGodPowers("We have an unhandled charge of " + kbGodPowerGetName(protoPowerID) + ", using it now.");
      int planID = -1;
      setUpGodPowerPlan(protoPowerID, planID);
      if (protoPowerID != originalProtoPowerID)
      {
         aiPlanSetVariableInt(planID, cGodPowerPlanPowerProtoID, 0, originalProtoPowerID);
      }
   }
}

//==============================================================================
// godPowerGrantedHandler
// We can't just create plans for each god power that we get because our strategies may want to manually control them.
// This gets called AFTER ageUpEventHandler, they are however both called in the same frame when aging up.
//==============================================================================
void godPowerGrantedHandler(int protoPowerID = -1)
{
   if (protoPowerID == cProtoPowerTitanGate)
   {
      if (cPersonalityCurrent == cPersonalityHumanoid)
      {
         debugGodPowers("Received a Titan Gate god power charge, we're humanoid though so not doing anything with it.");
         return;
      }
      debugGodPowers("Received a Titan Gate god power charge, starting the placement logic now.");
      xsEnableRule("titanGateConstructionMonitor");
      xsRuleIgnoreIntervalOnce("titanGateConstructionMonitor");
      return;
   }
   debugGodPowers("We got granted a charge of " + kbGodPowerGetName(protoPowerID) + ".");
}

//==============================================================================
// godPowerStateChangeHandler
// God powers can be blocked by various mechanics in the game.
// We can't check for all of them when we make our god power plans.
// So it's very plausible some of our GPs fail to cast.
// And sometimes we need to do something when we succeed in casting a GP too.
//==============================================================================
void godPowerStateChangeHandler(int planID = -1)
{
   int protoPowerID = aiPlanGetVariableInt(planID, cGodPowerPlanPowerProtoID, 0);
   if (kbGodPowerGetIsIDValid(protoPowerID) == false)
   {
      return; // We could've accidentally made an invalid plan.
   }
   int planState = aiPlanGetState(planID);
   if (planState == cPlanStateFailed)
   {
      debugGodPowers(kbGodPowerGetName(protoPowerID) + " failed to be cast.");
      // We will potentially try to recast via useUnusedGodPowersMonitor.
      return;
   }

   if (planState == cPlanStateDone)
   {
      switch (protoPowerID)
      {
         // Greek.
         case cProtoPowerUnderworldInvasion:
         {
            setLastUnderworldInvasionCastTime(xsGetTime());
            xsRuleIgnoreIntervalOnce("attackManager");
            break;
         }

         // Egyptian.
         case cProtoPowerShiftingSands:
         {
            // We've shifted forwards to an attack plan, put units in that plan.
            if (aiPlanGetIsIDValid(gShiftingSandsAttackPlanID) == true)
            {
               xsEnableRule("addShiftedUnitsToAttackPlanMonitor");
            }
            break;
         }
         case cProtoPowerAncestors:
         {
            // Ancestors cast offensively will have this bool set to true.
            // We must now add these Minions to the attack plan.
            if (aiPlanGetVariableBool(planID, cGodPowerPlanRequiresCombatPlan, 0) == true)
            {
               // We don't know what attack plan we just cast Ancestors for. Our best bet is to find the closest military unit
               // to the casting location and take that planID.
               int unitID = getClosestUnitByLocation(cUnitTypeLogicalTypeLandMilitary, cMyID, cUnitStateAlive,
                  aiPlanGetVariableVector(planID, cGodPowerPlanTargetLocation, 0), 30.0);
               if (unitID == -1)
               {
                  debugGodPowers("Couldn't find a close unit with which to figure out our attack plan for Ancestors.");
                  return;
               }
               int unitPlanID = kbUnitGetPlanID(unitID);
               if (unitPlanID == -1)
               {
                  debugGodPowers("Closest unit with which to figure out our attack plan for Ancestors has no valid unitPlanID.");
                  return;
               }
               if (aiPlanGetType(unitPlanID) != cPlanAttack && aiPlanGetType(unitPlanID) != cPlanExplore)
               {
                  debugGodPowers("Closest unitPlanID with which to figure out our attack plan for Ancestors isn't of the right type.");
                  return;
               }
               gAncestorsAttackPlanID = unitPlanID;
            }
            xsEnableRule("addMinionsToPlanMonitor");
            break;
         }
         case cProtoPowerSonOfOsiris:
         {
            int sooID = getUnit(cUnitTypeSonOfOsiris);
            if (sooID == -1)
            {
               return;
            }
            int empowerPlanID = kbUnitGetPlanID(sooID);
            if (empowerPlanID >= 0)
            {
               // Free the SoO for combat.
               if (aiPlanGetType(empowerPlanID) == cPlanEmpower)
               {
                  aiPlanRemoveUnit(empowerPlanID, sooID);
               }
            }
            break;
         }

         // Norse.
         case cProtoPowerGullinbursti:
         {
            xsEnableRule("gullinburstiDefendMonitor");
            break;
         }
         case cProtoPowerWalkingWoods:
         {
            xsEnableRule("addWalkingWoodsToAttackPlanMonitor");
            break;
         }
         case cProtoPowerRagnarok:
         {
            xsRuleIgnoreIntervalOnce("attackManager");
            // These House build plans will still be searching for a Villager, add military too it as well.
            int[] buildPlans = aiPlanGetIDsByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, gHouseUnit);
            for (int i = 0; i < buildPlans.size(); i++)
            {
               aiPlanAddUnitType(buildPlans[i], cUnitTypeLogicalTypeNorseSoldierThatBuilds, 1, 1, 1);
            }
            break;
         }

         // Atlantean.
         case cProtoPowerGaiaForest:
         {
            if (isBuildOrderDone() == false)
            {
               xsEnableRule("gaiaForestTransition");
            }
            break;
         }
         case cProtoPowerTraitor:
         {
            // Traitor removes the unit completely and then instantiates a new one, we can't save the ID.
            if (aiPlanGetIsIDValid(gTraitorAssignToPlanID) == true)
            {
               xsEnableRule("addTraitorToPlanMonitor");
            }
            break;
         }

         // Chinese.
         case cProtoPowerVenomBeast:
         {
            // Fei Beasts cast offensively will have this bool set to true.
            // We must now add these beasts to the attack plan.
            if (aiPlanGetVariableBool(planID, cGodPowerPlanRequiresCombatPlan, 0) == true)
            {
               // Save what plan we need to add them to.
               gFeiBeastsAttackPlanID = aiPlanGetVariableInt(planID, cGodPowerPlanCombatPlanID, 0);
            }
            xsEnableRule("addFeiBeastsToPlanMonitor");
            break;
         }

         // Aztec.
         case cProtoPowerAgaveBloom:
         {
            if (gResourceNeeds[cResourceFood] < -5000 || gTimeToFarm == true)
            {
               // This has to be on a delay.
               xsEnableRule("forbidAgaveBloom");
            }
         }
      }
   }
}

//==============================================================================
// useUnusedGodPowersMonitor
// Keep clearing out our bank if we're allowed to.
// We don't do this too often because if a GP instantly fails and we then
// instantly create another plan for it, most likely it will fail again.
// So to prevent a casting fail loop we wait a bit.
//==============================================================================
rule useUnusedGodPowersMonitor
group defaultArchaicRules
inactive
minInterval 10
{
   debugGodPowers("--- Running Rule useUnusedGodPowersMonitor ---");
   // If we're allowed create another plan for this GP.
   if (checkStrategyFlag(cStrategyFlagAutomaticGodPowerUsage) == true)
   {
      useUnusedGodPowers();
   }
}

//==============================================================================
// godPowerCastedHandler
//==============================================================================
void godPowerCastedHandler(int index = -1)
{
   debugGodPowers("God power was cast!");
   //debugGodPowers("Index: " + index);
   //debugGodPowers("Location: " + kbGodPowerCastEventGetCastLocation(index));
   int casterID = kbGodPowerCastEventInfoGetCaster(index);
   debugGodPowers("Caster: " + casterID);
   int godPowerID = kbGodPowerCastEventInfoGetProtoPower(index);
   debugGodPowers("Proto Power: " + kbGodPowerGetName(godPowerID));

   if (casterID == cMyID)
   {
      // Generic / offensive / eco god powers all have unique chats.
      switch (godPowerID)
      {
         case cProtoPowerSentinel:
         case cProtoPowerRestoration:
         case cProtoPowerCeaseFire:
         case cProtoPowerBronze:
         case cProtoPowerUnderworldPassage:
         case cProtoPowerArcadianMeadow:
         case cProtoPowerVision:
         case cProtoPowerEclipse:
         case cProtoPowerShiftingSands:
         case cProtoPowerPlagueOfSerpents:
         case cProtoPowerCitadel:
         case cProtoPowerSonOfOsiris:
         case cProtoPowerSpy:
         case cProtoPowerGullinbursti:
         case cProtoPowerHealingSpring:
         case cProtoPowerAsgardianBastion:
         case cProtoPowerRagnarok:
         case cProtoPowerDeconstruction:
         case cProtoPowerValor:
         case cProtoPowerSpiderLair:
         case cProtoPowerHesperidesTree:
         case cProtoPowerVanish:
         case cProtoPowerForestProtection:
         case cProtoPowerNewMoon:
         case cProtoPowerGoshinboku:
         case cProtoPowerSacredGate:
         case cProtoPowerObsidianMirror:
         case cProtoPowerTailwind:
         {
            sendStatementToEverybody(cAICommPromptToEverybodyGenericGodPower);
            break;
         }
         case cProtoPowerBolt:
         case cProtoPowerWither:
         case cProtoPowerPestilence:
         case cProtoPowerCurse:
         case cProtoPowerLightningStorm:
         case cProtoPowerEarthquake:
         case cProtoPowerUnderworldInvasion:
         case cProtoPowerLocustSwarm:
         case cProtoPowerAncestors:
         case cProtoPowerMeteor:
         case cProtoPowerTornado:
         case cProtoPowerForestFire:
         case cProtoPowerUndermine:
         case cProtoPowerFrost:
         case cProtoPowerFlamingWeapons:
         case cProtoPowerWalkingWoods:
         case cProtoPowerTempest:
         case cProtoPowerFimbulwinter:
         case cProtoPowerNidhogg:
         case cProtoPowerInferno:
         case cProtoPowerShockwave:
         case cProtoPowerCarnivora:
         case cProtoPowerTraitor:
         case cProtoPowerChaos:
         case cProtoPowerVortex:
         case cProtoPowerTartarianGate:
         case cProtoPowerImplode:
         case cProtoPowerEarthWall:
         case cProtoPowerLightningWeapons:
         case cProtoPowerDroughtLand:
         case cProtoPowerVenomBeast:
         case cProtoPowerGreatFlood:
         case cProtoPowerBlazingPrairie:
         case cProtoPowerYinglongsWrath:
         case cProtoPowerSolarShield:
         case cProtoPowerKusanagi:
         case cProtoPowerSwampland:
         case cProtoPowerShogun:
         case cProtoPowerSmitingGust:
         case cProtoPowerThunderBurst:
         case cProtoPowerDragonTyphoon:
         case cProtoPowerDivineSlash:
         case cProtoPowerBloodPact:
         case cProtoPowerLullaby:
         case cProtoPowerInfestation:
         case cProtoPowerEarthMonster:
         case cProtoPowerStarfall:
         case cProtoPowerPurge:
         case cProtoPowerCorruptedGround:
         case cProtoPowerMonolithOfTlaloc:
         case cProtoPowerVolcano:
         {
            if (godPowerID == cProtoPowerUnderworldInvasion)
            {
               lastUnderworldInvasionCastTime = xsGetTime();
            }
            sendStatementToEnemies(cAICommPromptToEnemyOffensiveGodPower);
            break;
         }
         case cProtoPowerLure:
         case cProtoPowerCommunalHearth:
         case cProtoPowerPlentyVault:
         case cProtoPowerRain:
         case cProtoPowerProsperity:
         case cProtoPowerDwarvenMine:
         case cProtoPowerGreatHunt:
         case cProtoPowerGaiaForest:
         case cProtoPowerCreation:
         case cProtoPowerThePeachBlossomSpring:
         case cProtoPowerProsperousSeeds:
         case cProtoPowerShrineOfTheHunt:
         case cProtoPowerAgaveBloom:
         {
            sendStatementToEverybody(cAICommPromptToEverybodyEconomicGodPower);
            break;
         }
      }
   }
}

//==============================================================================
// valorRebuyMonitor
// Valor is a special GP that we want to get a lot early on and thus isn't suited for the other logic.
//==============================================================================
rule valorRebuyMonitor
group defaultClassicalRules
inactive
minInterval 30
{
   if (kbTechGetStatus(cTechClassicalAgePrometheus) != cTechStatusActive)
   {
      xsDisableRule("valorRebuyMonitor");
      return;
   }
   if (checkStrategyFlag(cStrategyFlagRebuysGodPowers) == false)
   {
      return;
   }
   debugGodPowers("--- Running Rule valorRebuyMonitor ---");

   float rebuyCost = kbGodPowerGetPrePurchaseCost(cProtoPowerValor, 1, cMyID);
   if (rebuyCost >= 15.0)
   {
      debugGodPowers("Valor now costs " + rebuyCost + " which is too expensive, never rebuying again.");
      xsDisableRule("valorRebuyMonitor");
      return;
   }
   if (kbGodPowerIsOnCooldown(cProtoPowerValor, cMyID) == true)
   {
      debugGodPowers("Valor is still on cooldown, can't rebuy.");
      return;
   }
   if (kbGodPowerIsRepeatable(cProtoPowerValor, cMyID) == false)
   {
      debugGodPowers("Valor can't be rebought at all.");
      return;
   }
   if (kbGodPowerGetCost(cProtoPowerValor, cMyID) == 0.0)
   {
      if (aiPlanGetIDByTypeAndVariableIntValue(cPlanGodPower, cGodPowerPlanPowerProtoID, cProtoPowerValor, 0) == -1)
      {
         godPowerGrantedHandler(cProtoPowerValor);
         debugGodPowers("Valor still had a free charge but didn't have a plan, created one now.");
         return;
      }
      debugGodPowers("We still have charges on Valor, can't rebuy.");
      return;
   }
   if (kbGodPowerCanPrePurchase(cProtoPowerValor, cMyID) == false)
   {
      debugGodPowers("Valor can't be rebought at this moment.");
      return;
   }
   if (rebuyCost > (-gResourceNeeds[cResourceFavor]))
   {
      debugGodPowers("Don't have enough excess favor to buy Valor.");
      return;
   }

   debugGodPowers("Decided to buy another charge for Valor.");
   aiPrePurchaseGodPower(cProtoPowerValor, 1);
   godPowerGrantedHandler(cProtoPowerValor);
}

//==============================================================================
// wantToRebuyGodPower
// This idea is that the really strong GPs like Lightning Storm + Son of Osiris are always valid.
// And stuff like Bolt + Undermine become invalid on cost after some time.
// Thus we re-use a lot of GPs, but in the end only the most powerful remain.
//==============================================================================
bool wantToRebuyGodPower(int protoPowerID = -1, float rebuyCost = 0.0)
{
   bool haveSocketedTCAlive = getRandomTownCenterBaseID() >= 0; // Use this func since it keeps gLandAreaGroupID into account.
   switch (protoPowerID)
   {
      case cProtoPowerSentinel:
      {
         if (haveSocketedTCAlive == false)
         {
            debugGodPowers("   Not rebuying Sentinels since we have no TC to cast it on.");
            return false;
         }
         break;
      }
      case cProtoPowerUnderworldPassage:
      {
         if (kbUnitCount(cUnitTypeUnderworldPassage, cMyID, cUnitStateAlive) >= 1)
         {
            debugGodPowers("   Not rebuying Underworld since we already have one set alive.");
            return false;
         }
         break;
      }
      case cProtoPowerCommunalHearth:
      {
         if (getBaseIDForCommunalHearth(false) == -1)
         {
            debugGodPowers("   Not Rebuying Communcal Hearth because we found na valid TC base to cast it on.");
            return false;
         }
         break;
      }
      case cProtoPowerPlentyVault:
      {
         if (haveSocketedTCAlive == false)
         {
            debugGodPowers("   Not rebuying Plenty Vault since we have no TC base cast it in.");
            return false;
         }
         // Don't crowd our base too much with these.
         if (kbUnitCount(cUnitTypePlentyVault, cMyID, cUnitStateAlive) >= 5)
         {
            debugGodPowers("   Not rebuying Plenty Vault since we already have 5 or more alive.");
            return false;
         }
         break;
      }
      case cProtoPowerUnderworldInvasion:
      {
         if (isItWorthToCastUnderworldInvasion(false) == false)
         {
            debugGodPowers("   Not rebuying Underworld Invasion because the TC to Eidolons spawned ratio isn't right.");
            return false;
         }
         break;
      }

      // Egyptian.
      case cProtoPowerPlagueOfSerpents:
      {
         // Otherwise we spam the entire map with these.
         if (kbUnitCount(cUnitTypeSerpent, cMyID, cUnitStateAlive) > 0)
         {
            debugGodPowers("   Not rebuying Plague Of Serpents since we still have some alive.");
            return false;
         }
         break;
      }
      case cProtoPowerCitadel:
      {
         if (kbUnitCount(cUnitTypeTownCenter, cMyID, cUnitStateAlive) == 0)
         {
            debugGodPowers("   Not rebuying Citadel since we have no TC to cast it on.");
            return false;
         }
         break;
      }

      // Norse.
      case cProtoPowerGullinbursti:
      {
         if (haveSocketedTCAlive == false)
         {
            debugGodPowers("   Not rebuying Gullinburstie since we have no TC to cast it on.");
            return false;
         }
         break;
      }
      case cProtoPowerHealingSpring:
      {
         if (getBaseIDForHealingSpring(false) == -1)
         {
            debugGodPowers("   Not Rebuying Healing Spring because we found na valid TC base to cast it on.");
            return false;
         }
         break;
      }
      case cProtoPowerAsgardianBastion:
      {
         if (haveSocketedTCAlive == false)
         {
            debugGodPowers("   Not rebuying Asgardian Bastion since we have no TC base to cast it in.");
            return false;
         }
         break;
      }
      case cProtoPowerRagnarok:
      {
         if (shouldCastRagnarok(false) == false)
         {
            debugGodPowers("   Not rebuying Ragnarok because we don't have enough Villagers alive to cast Ragnarok effectively.");
            return false;
         }
         break;
      }

      // Atlantean.
      case cProtoPowerGaiaForest:
      {
         if (haveSocketedTCAlive == false)
         {
            debugGodPowers("   Not rebuying Gaia Forest since we have no TC base left.");
            return false;
         }
         int baseID = getMostDefendedTCBase();
         vector location = kbBaseGetLocation(cMyID, baseID);
         float range = kbBaseGetDistance(cMyID, baseID) + 10.0;
         if (getUnitCountByLocation(cUnitTypeTreeGaia, cPlayerMotherNatureID, cUnitStateAlive, location, range) >= 10)
         {
            debugGodPowers("   Not rebuying Gaia Forest since we have enough trees left for now.");
            return false;
         }
         break;
      }
      case cProtoPowerCarnivora:
      {
         if (haveSocketedTCAlive == false)
         {
            debugGodPowers("   Not rebuying Carnivora since we have no TC base left.");
            return false;
         }
         // Don't crowd our base too much with these.
         if (kbUnitCount(cUnitTypeCarnivora, cMyID, cUnitStateAlive) >= 5)
         {
            debugGodPowers("   Not rebuying Carnivora since we already have 5 or more alive.");
            return false;
         }
         break;
      }
      case cProtoPowerSpiderLair:
      {
         // Too many of these is actually a performance drain.
         if (kbUnitCount(cUnitTypeSpider, cMyID, cUnitStateAlive) + kbUnitCount(cUnitTypeSpiderEgg, cMyID, cUnitStateAlive) >= 15)
         {
            debugGodPowers("   Not rebuying Spider Lair since we already have 15 or more hiding.");
            return false;
         }
         break;
      }
      case cProtoPowerHesperidesTree:
      {
         // Only need 1.
         if (kbUnitCount(cUnitTypeHesperidesTree, cMyID, cUnitStateAlive) >= 1)
         {
            debugGodPowers("   Not rebuying Hesperides Tree since we already have 1 or more alive.");
            return false;
         }
         break;
      }

      // Chinese.
      case cProtoPowerCreation:
      {
         // Output is in the func.
         if (haveFewerEcoPopThanCreationThreshold(true) == false)
         {
            return false;
         }
         break;
      }
      case cProtoPowerProsperousSeeds:
      {
         if (findFarmForProsperousSeeds(false) == -1)
         {
            debugGodPowers("   Not rebuying Prosperous Seeds since we have no Farms to cast it on.");
            return false;
         }
         break;
      }
      case cProtoPowerForestProtection:
      {
         if (getValidForestProtectionTarget(false) == -1)
         {
            debugGodPowers("   Not rebuying Forest Protection since we have no TC/Fortress to cast it on.");
            return false;
         }
         break;
      }

      // Japanese.
      case cProtoPowerNewMoon:
      {
         float totalValue = 0.0;
         int buildingPUID = getHighestValueNewMoonPUID(totalValue);
         if (totalValue < gNewMoonMinimumValue)
         {
            debugGodPowers("Not rebuying New Mon since the best projected value is " +  kbProtoUnitGetName(buildingPUID) +
               " with " + totalValue + ", which is below the " + gNewMoonMinimumValue + " minimum.");
            return false;
         }
         break;
      }
      case cProtoPowerGoshinboku:
      {
         if (haveSocketedTCAlive == false)
         {
            debugGodPowers("   Not rebuying Goshinboku since we have no TC base left.");
            return false;
         }
         int shinbokuCount = kbUnitCount(cUnitTypeGoshinboku, cMyID, cUnitStateAlive);
         int threshold = 1;
         if (cPersonalityCurrent == cPersonalityMythical)
         {
            threshold++;
         }
         if (shinbokuCount >= threshold)
         {
            debugGodPowers("Not rebuying Goshinboku since we already have " + shinbokuCount + "/" + threshold + " Shinbokus.");
            return false;
         }
         break;
      }
      case cProtoPowerShrineOfTheHunt:
      {
         int shrineID = findShrineForShrineOfTheHunt();
         if (shrineID == -1)
         {
            debugGodPowers("Not rebuying Shrine of the Hunt since we have no suitable Shrines to cast it on.");
            return false;
         }
         break;
      }
      case cProtoPowerSacredGate:
      {
         if (haveSocketedTCAlive == false)
         {
            debugGodPowers("   Not rebuying Sacred Gate since we have no TC base left.");
            return false;
         }
         if (kbUnitCount(cUnitTypeSacredGate, cMyID, cUnitStateAlive) != 0)
         {
            debugGodPowers("Not rebuying Sacred Gate since we have already have at least one alive.");
            return false;
         }
         break;
      }
   }
   return true;
}

//==============================================================================
// godPowerRebuyMonitor
//==============================================================================
rule godPowerRebuyMonitor
#if (cMyCulture == cCultureAtlantean)
group defaultArchaicRules
minInterval 30
#else
group defaultMythicRules
minInterval 120
#endif
inactive
{
   if (checkStrategyFlag(cStrategyFlagRebuysGodPowers) == false)
   {
      return;
   }
   
   // We want to quickly rebuy Archaic + Classical Atty GPs, afterwards we slow it all down to normal level.
   if (cMyCulture == cCultureAtlantean && kbPlayerGetAge(cMyID) >= cAge3)
   {
      xsSetRuleMinInterval("godPowerRebuyMonitor", 120);
   }
   debugGodPowers("--- Running Rule godPowerRebuyMonitor ---");

   int[] buyableGodPowers = new int(0, 0);
   for (int i = 0; i < 4; i++)
   {
      int protoPowerID = kbGodPowerGetIDInSlot(i, cMyID);
      if (protoPowerID == -1)
      {
         continue; // Needed against invalid entries + Atlanteans who activate this early on.
      }
      // Obsidian Mirror will identify as something else potentially, but we gotta ask some other syscalls with ObsidianMirror.
      if (cMyCiv == cCivTezcatlipoca && i == 0)
      {
         protoPowerID = cProtoPowerObsidianMirror;
      }
      debugGodPowers("Analyzing slot " + i + " which contains " + kbGodPowerGetName(protoPowerID) + " to potentially rebuy.");
      // Forbid list.
      if (protoPowerID == cProtoPowerLure || protoPowerID == cProtoPowerVision || protoPowerID == cProtoPowerSpy ||
          protoPowerID == cProtoPowerGreatHunt || protoPowerID == cProtoPowerForestFire || protoPowerID == cProtoPowerValor ||
          protoPowerID == cProtoPowerEarthWall || protoPowerID == cProtoPowerVanish || protoPowerID == cProtoPowerThePeachBlossomSpring || 
          protoPowerID == cProtoPowerSmitingGust)
      {
         debugGodPowers("   We never want to rebuy this GP.");
         continue;
      }
      if (kbGodPowerIsRepeatable(protoPowerID, cMyID) == false)
      {
         debugGodPowers("   GP can't be rebought at all.");
         continue;
      }
      if (kbGodPowerIsOnCooldown(protoPowerID, cMyID) == true)
      {
         debugGodPowers("   GP is still on cooldown, can't rebuy.");
         continue;
      }
      if (kbGodPowerGetNumCharges(protoPowerID, cMyID) > 0)
      {
         debugGodPowers("   We still have a charge of this GP.");
         continue;
      }
      float rebuyCost = kbGodPowerGetPrePurchaseCost(protoPowerID, 1, cMyID);
      if (wantToRebuyGodPower(protoPowerID, rebuyCost) == false)
      {
         continue;
      }
      if (kbGodPowerCanPrePurchase(protoPowerID, cMyID) == false)
      {
         debugGodPowers("   GP can't be rebought at this moment.");
         continue;
      }
      if (rebuyCost > (-gResourceNeeds[cResourceFavor]))
      {
         debugGodPowers("   Don't have enough excess favor to buy this GP.");
         debugGodPowers("We can only rebuy a god power if we have enough favor to buy all we would potentially want, quiting.");
         return;
      }
      buyableGodPowers.add(protoPowerID);
   }

   if (buyableGodPowers.size() == 0)
   {
      debugGodPowers("Found no god powers we can rebuy this time.");
      return;
   }
   int toBuyID = buyableGodPowers[xsRandInt(0, buyableGodPowers.size() - 1)];
   debugGodPowers("Decided to buy another charge for: " + kbGodPowerGetName(toBuyID) + ".");
   aiPrePurchaseGodPower(toBuyID, 1);
   godPowerGrantedHandler(toBuyID);
}

//==============================================================================
// titanGateRebuyMonitor
//==============================================================================
rule titanGateRebuyMonitor
group defaultWonderRules
inactive
minInterval 60
{
   if (checkStrategyFlag(cStrategyFlagRebuysGodPowers) == false)
   {
      return;
   }
   debugGodPowers("--- Running Rule titanGateRebuyMonitor ---");

   if (kbGodPowerGetCost(cProtoPowerTitanGate, cMyID) == 0.0)
   {
      debugGodPowers("Titan Gate still has a charge left, can't rebuy.");
      return;
   }
   if (kbGodPowerIsOnCooldown(cProtoPowerTitanGate, cMyID) == true)
   {
      debugGodPowers("Titan Gate is still on cooldown, can't rebuy.");
      return;
   }
   if (kbGodPowerCanPrePurchase(cProtoPowerTitanGate, cMyID) == false)
   {
      debugGodPowers("Titan Gate can't be rebought at this moment.");
      return;
   }
   aiPrePurchaseGodPower(cProtoPowerTitanGate, 1);
   debugGodPowers("Buying another charge for the Titan Gate.");
   godPowerGrantedHandler(cProtoPowerTitanGate);
}

//////////////////////
int gTitanGatePlanID = -1;
int gTitanGateRepairPlanID = -1;
const int cStateBegin = 0;
const int cStateWaitingForFoundation = 1;
const int cStateFoundationPlaced = 2;
const int cStateBuilding = 3;
const int cStateDone = 4;
int gTitanGateCurrentState = cStateBegin;
int gTitanGateID = -1;
//==============================================================================
// titanGateStateChangeHandler
//==============================================================================
void titanGateStateChangeHandler(int planID = -1)
{
   int state = aiPlanGetState(planID);
   switch (state)
   {
      case cPlanStateDone:
      {
         debugGodPowers("Titan Gate placement succeeded, going to build it now!");
         gTitanGatePlanID = -1;
         gTitanGateCurrentState = cStateFoundationPlaced;
         break;
      }
      case cPlanStateFailed:
      {
         debugGodPowers("Titan Gate placement failed, restarting the chain!");
         gTitanGatePlanID = -1;
         gTitanGateCurrentState = cStateBegin;
         break;
      }
   }
}

//==============================================================================
// titanGateRepairStateChangeHandler
//==============================================================================
void titanGateRepairStateChangeHandler(int planID = -1)
{
   int state = aiPlanGetState(planID);
   switch (state)
   {
      case cPlanStateDone:
      {
         debugGodPowers("Titan Gate repair succeeded, we have a Titan now!");
         gTitanGateRepairPlanID = -1;
         gTitanGateCurrentState = cStateDone;
         xsEnableRule("titanManager");
         break;
      }
      case cPlanStateFailed:
      {
         debugGodPowers("Titan Gate repair failed, titanGateConstructionMonitor will reset us soon!");
         gTitanGateRepairPlanID = -1;
         break;
      }
   }
}

//==============================================================================
// titanGateConstructionMonitor
// Place -> Construct
//==============================================================================
rule titanGateConstructionMonitor
inactive
minInterval 5
{
   debugGodPowers("--- Running Rule titanGateConstructionMonitor ---");
   switch (gTitanGateCurrentState)
   {
      case cStateBegin:
      {
         if (gTitanGatePlanID != -1)
         {
            aiEchoWarning("titanGateConstructionMonitor - in cStateBegin gTitanGatePlanID should be -1.");
            if (aiPlanGetIsIDValid(gTitanGatePlanID) == true)
            {
               aiPlanDestroy(gTitanGatePlanID);
            }
            gTitanGatePlanID = -1;
         }
         int safestBaseID = getMostDefendedTCBase();
         if (safestBaseID == -1)
         {
            debugGodPowers("We currently have no TC base");
            return;
         }
         gTitanGatePlanID = aiPlanCreate("Titan Gate Placement", cPlanGodPower, -1, gMilitaryBuildingsCategoryID);
         aiPlanSetVariableInt(gTitanGatePlanID, cGodPowerPlanPowerProtoID, 0, cProtoPowerTitanGate);

         aiPlanSetVariableInt(gTitanGatePlanID, cGodPowerPlanEvaluationModel, 0, cGodPowerPlanEvaluationModelNone);
         aiPlanSetVariableInt(gTitanGatePlanID, cGodPowerPlanTargetingModel, 0, cGodPowerPlanTargetingModelBuildingPlacement);
         int bpID = kbBuildingPlacementCreate(aiPlanGetName(gTitanGatePlanID) + " Titan Gate Placement");
         kbBuildingPlacementSetBuildingPUID(bpID, cUnitTypeTitanGate);
         
         addSafeBackAreasToBuildingPlacement(bpID, safestBaseID, gGodpowersCategoryID);
         // Risky cuz it can block builders but this thing is hard enough to place as it is.
         kbBuildingPlacementSetBufferSpace(bpID, 0.0);
         kbBuildingPlacementSetRequiresCompletelyUnobstructed(bpID, true);
         aiPlanSetVariableInt(gTitanGatePlanID, cGodPowerPlanBPID, 0, bpID);
         
         aiPlanSetEventHandler(gTitanGatePlanID, cPlanEventStateChange, "titanGateStateChangeHandler");
         gTitanGateCurrentState = cStateWaitingForFoundation;
         debugGodPowers("Created: " + aiPlanGetName(gTitanGatePlanID) + ".");
         break;
      }
      
      case cStateWaitingForFoundation:
      {
         debugGodPowers("Waiting for the Titan Gate foundation to be placed.");
         break;
      }

      case cStateFoundationPlaced:
      {
         if (gTitanGateRepairPlanID != -1)
         {
            aiEchoWarning("titanGateConstructionMonitor - in cStateFoundationPlaced gTitanGateRepairPlanID should be -1.");
            if (aiPlanGetIsIDValid(gTitanGateRepairPlanID) == true)
            {
               aiPlanDestroy(gTitanGateRepairPlanID);
            }
            gTitanGateRepairPlanID = -1;
         }
         // Sometimes we get here too quickly after placing the Gate and we can't see it yet, don't fail.
         static bool firstRunInFoundation = true;
         gTitanGateID = getUnit(cUnitTypeTitanGate, cMyID, cUnitStateBuilding);
         if (gTitanGateID == -1)
         {
            if (firstRunInFoundation == true)
            {
               firstRunInFoundation = false;
               return;
            }
            aiEchoWarning("titanGateConstructionMonitor - in cStateFoundationPlaced gTitanGateID isn't valid! Resetting " + 
               "everything and quiting.");
            gTitanGateID = -1;
            xsDisableRule("titanGateConstructionMonitor");
            return;
         }

         gTitanGateRepairPlanID = aiPlanCreate("Repair Titan Gate", cPlanRepair, -1, gMilitaryBuildingsCategoryID);
         aiPlanSetVariableInt(gTitanGateRepairPlanID, cRepairPlanTargetID, 0, gTitanGateID);
         aiPlanSetPriority(gTitanGateRepairPlanID, 100); // GOOOOO.
         int unitType = cUnitTypeAbstractVillager;
         int amount = 0;
         if (cMyCulture == cCultureNorse)
         {
            unitType = cUnitTypeLogicalTypeNorseSoldierThatBuilds;
            amount = max(5, kbUnitCount(unitType, cMyID, cUnitStateAlive) / 2); // 50%.
         }
         else if (cMyCulture == cCultureChinese)
         {
            unitType = cUnitTypeVillagerChinese;
            amount = max(5, kbUnitCount(unitType, cMyID, cUnitStateAlive) / 5); // 20%.
            // Custom Kuafu code.
            int kuafuAmount = max(5, kbUnitCount(cUnitTypeKuafu, cMyID, cUnitStateAlive) / 5); // 20%.
            aiPlanAddUnitType(gTitanGateRepairPlanID, cUnitTypeKuafu, kuafuAmount, kuafuAmount, kuafuAmount);
         }
         else
         {
            amount = max(5, kbUnitCount(unitType, cMyID, cUnitStateAlive) / 5); // 20%.
         }

         aiPlanAddUnitType(gTitanGateRepairPlanID, unitType, amount, amount, amount);
         aiPlanSetBaseID(gTitanGateRepairPlanID, kbUnitGetBaseID(gTitanGateID));
         aiPlanSetEventHandler(gTitanGateRepairPlanID, cPlanEventStateChange, "titanGateRepairStateChangeHandler");

         firstRunInFoundation = true; // Reset this.
         debugGodPowers("Created: " + aiPlanGetName(gTitanGateRepairPlanID));
         gTitanGateCurrentState = cStateBuilding;
         break;
      }

      case cStateBuilding:
      {
         if (kbUnitGetIsIDValid(gTitanGateID) == false || kbUnitGetPlayerID(gTitanGateID) != cMyID)
         {
            debugGodPowers("In cStateBuilding gTitanGateID isn't valid anymore, we must've " + 
               "lost the Gate! Resetting everything and quiting.");
            gTitanGateID = -1;
            xsDisableRule("titanGateConstructionMonitor");
            return;
         }
         debugGodPowers("Waiting for the Gate to be completed.");
         break;
      }

      case cStateDone:
      {
         debugGodPowers("Titan has been successfully unleashed, disabling now to wait for a potential next run.");
         gTitanGateID = -1;
         gTitanGateCurrentState = cStateBegin;
         xsDisableRule("titanGateConstructionMonitor");
         break;
      }
   }
}