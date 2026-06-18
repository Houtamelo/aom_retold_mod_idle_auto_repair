//==============================================================================
// initMythicTurtlerStrategy
//==============================================================================
void initMythicTurtlerStrategy()
{
   gMythicTurtlerStrategy.mID = cStrategyMythicTurtler;
   gMythicTurtlerStrategy.mAge = cAge4;
   gMythicTurtlerStrategy.mInitFunc = [](ref StrategyData data) -> bool
   {
      data.enableAllStrategyFlags();

      // Low attack interval.
      int interval = selectByDifficulty(16, 13, 10, 9, 8, 8) * 60;
      gAttackManager.mBaseAttackInterval = interval;
      gAttackManager.mAttackInterval = interval;
      gNavalAttackManager.mBaseAttackInterval = interval;
      gNavalAttackManager.mAttackInterval = interval;

      data.mWallCircleAmount = 1;
      // Only do multiple layers on large/giant map sizes.
      if (cMapSizeCurrent >= cMapSizeLarge)
      {
         data.mWallCircleAmount = 2;
      }

      return true;
   };
   gMythicTurtlerStrategy.mGetCurrentScore = []() -> float
   {
      const float rVal = cStrategyDefaultStrategyValue + 10.0; // Pick it over the default strategy.
      debugStrategy(gMythicTurtlerStrategy.mName + " has a value of " + rVal + ".");
      return rVal;
   };
   gMythicTurtlerStrategy.mUpdateFunc = [](ref StrategyData data) -> bool 
   { 
      patrolBaseGates(data); 
      return kbPlayerGetAge(cMyID) == cAge4;
   };   
   gMythicTurtlerStrategy.mName = "Mythic Turtler";
}