//==============================================================================
// initDefaultMythicStrategy
//==============================================================================
void initDefaultMythicStrategy()
{
   // Init Mythic Age strategy.
   gMythicStrategy.mID = cStrategyMythic;
   gMythicStrategy.mAge = cAge4;
   gMythicStrategy.mInitFunc = [](ref StrategyData data) -> bool
   {
      data.enableAllStrategyFlags();

      int interval = 0;
      // Attack less frequently for these personalities.
      if (cPersonalityCurrent == cPersonalityConqueror || cPersonalityCurrent == cPersonalitySupporter ||
          cPersonalityCurrent == cPersonalityBuilder)
      {
         interval = selectByDifficulty(16, 13, 10, 9, 8, 8) * 60;
      }
      else
      {
         interval = selectByDifficultyFloat(10, 8, 7, 7, 6, 6) * 60.0;
      }
      gAttackManager.mBaseAttackInterval = interval;
      gAttackManager.mAttackInterval = interval;
      gNavalAttackManager.mBaseAttackInterval = interval;
      gNavalAttackManager.mAttackInterval = interval;
	  
      data.mWallCircleAmount = 1;
	  
      return true;
   };
   gMythicStrategy.mGetCurrentScore = []() -> float
   {
      debugStrategy(gMythicStrategy.mName + " has a value of " + cStrategyDefaultStrategyValue + ".");
      return cStrategyDefaultStrategyValue;
   };
   gMythicStrategy.mUpdateFunc = [](ref StrategyData data) -> bool
   {
      return kbPlayerGetAge(cMyID) == cAge4;
   };
   gMythicStrategy.mName = "Default Mythic";
}