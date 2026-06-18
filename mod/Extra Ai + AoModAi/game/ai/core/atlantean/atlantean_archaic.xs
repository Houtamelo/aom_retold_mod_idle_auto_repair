void boAtlanteanStandardArchaic()
{
   debugBO("Picked BO: boAtlanteanStandardArchaic");
   
   // Starting Villagers, 2 food.
   boVillager(cUnitTypeVillagerAtlantean, cResourceFood);
   boVillager(cUnitTypeVillagerAtlantean, cResourceFood);
   
   boActivateRule("HeroizeStartingOracles");
   boActivateRule("HeroizeStartingMurmillo");
   
   boIncreaseTimeout(220);
   if (cMyCiv == cCivGaia)
   {
      boExecute(boUseUnusedGodPowers); // Cast Gaia Forest.
   }

   boVillager(cUnitTypeVillagerAtlantean, cResourceWood);
   boVillager(cUnitTypeVillagerAtlantean, cResourceGold);
   boBuild(cUnitTypeManor, cUnitTypeVillagerAtlantean, 1, loanVillagerFromWoodPlan, "internalBOBuildManorComplete");
   boVillager(cUnitTypeVillagerAtlantean, cResourceFood);
   boVillager(cUnitTypeVillagerAtlantean, cResourceWood);
   boBuild(cUnitTypeTemple, cUnitTypeVillagerAtlantean, 1, loanVillagerFromGoldPlan);
   boVillager(cUnitTypeVillagerAtlantean, cResourceFood);
   boConditionalWait([]()->bool {return kbUnitCount(cUnitTypeTemple, cMyID, cUnitStateABQ) >= 1;}, 10);
   boVillager(cUnitTypeVillagerAtlantean, cResourceGold);
   boVillager(cUnitTypeVillagerAtlantean, cResourceWood);
   boConditionalWait([]()->bool {return kbUnitCount(cUnitTypeTemple, cMyID) >= 1;}, 10);
   // Villager Distribution: 4 - 3 - 2 - 0.
}

void boAtlanteanGaiaEcoArchaic()
{
   debugBO("Picked BO: boAtlanteanGaiaEcoArchaic");
   
   // Starting Villagers, 2 food.
   boVillager(cUnitTypeVillagerAtlantean, cResourceFood);
   boVillager(cUnitTypeVillagerAtlantean, cResourceFood);
   
   boActivateRule("HeroizeStartingOracles");
   boActivateRule("HeroizeStartingMurmillo");
   
   boIncreaseTimeout(250);
   boExecute(boUseUnusedGodPowers); // Cast Gaia Forest.

   boVillager(cUnitTypeVillagerAtlantean, cResourceWood);
   boVillager(cUnitTypeVillagerAtlantean, cResourceGold);
   boBuild(cUnitTypeManor, cUnitTypeVillagerAtlantean, 1, loanVillagerFromWoodPlan, "internalBOBuildManorComplete");
   boVillager(cUnitTypeVillagerAtlantean, cResourceFood);
   boVillager(cUnitTypeVillagerAtlantean, cResourceWood);
   boBuild(cUnitTypeTemple, cUnitTypeVillagerAtlantean, 1, loanVillagerFromGoldPlan);
   boVillager(cUnitTypeVillagerAtlantean, cResourceWood);
   boConditionalWait([]()->bool {return kbUnitCount(cUnitTypeTemple, cMyID, cUnitStateABQ) >= 1;}, 10);
   boVillager(cUnitTypeVillagerAtlantean, cResourceGold);
   boBuild(cUnitTypeEconomicGuild, cUnitTypeVillagerAtlantean, 1, loanVillagerFromWoodPlan);
   boVillager(cUnitTypeVillagerAtlantean, cResourceGold);
   boConditionalWait([]()->bool {return kbUnitCount(cUnitTypeTemple, cMyID) >= 1;}, 10);
   boVillager(cUnitTypeVillagerAtlantean, cResourceFood);
   // Villager Distribution: 4 - 3 - 3 - 0.
}