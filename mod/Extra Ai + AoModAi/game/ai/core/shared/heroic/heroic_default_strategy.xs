//==============================================================================
// initDefaultHeroicStrategy
//==============================================================================
void initDefaultHeroicStrategy()
{
   // Init Heroic Age strategy.
   gHeroicStrategy.mID = cStrategyHeroic;
   gHeroicStrategy.mAge = cAge3;
   gHeroicStrategy.mInitFunc = [](ref StrategyData data) -> bool
   {
      data.enableAllStrategyFlags();

      int interval = 0;
      // Attack less frequently for these personalities.
      if (cPersonalityCurrent == cPersonalityConqueror || cPersonalityCurrent == cPersonalitySupporter ||
          cPersonalityCurrent == cPersonalityBuilder)
      {
         interval = selectByDifficulty(18, 14, 12, 10, 8, 8) * 60;
      }
      else
      {
         interval = selectByDifficultyFloat(10, 9, 8, 7, 6, 6) * 60.0;
      }
      gAttackManager.mBaseAttackInterval = interval;
      gAttackManager.mAttackInterval = interval;
      gNavalAttackManager.mBaseAttackInterval = interval;
      gNavalAttackManager.mAttackInterval = interval;
	  
      data.mWallCircleAmount = 1;

      return true;
   };
   gHeroicStrategy.mGetCurrentScore = []() -> float
   {
      debugStrategy(gHeroicStrategy.mName + " has a value of " + cStrategyDefaultStrategyValue + ".");
      return cStrategyDefaultStrategyValue;
   };
   gHeroicStrategy.mUpdateFunc = [](ref StrategyData data) -> bool
   {
      return kbPlayerGetAge(cMyID) == cAge3;
   };
   gHeroicStrategy.mName = "Default Heroic";
}