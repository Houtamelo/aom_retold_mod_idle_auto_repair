//==============================================================================
// initHeroicTurtlerStrategy
//==============================================================================
void initHeroicTurtlerStrategy()
{
   gHeroicTurtlerStrategy.mID = cStrategyHeroicTurtler;
   gHeroicTurtlerStrategy.mAge = cAge3;
   gHeroicTurtlerStrategy.mInitFunc = [](ref StrategyData data) -> bool
   {
      data.enableAllStrategyFlags();

      // Low attack interval.
      int interval = selectByDifficulty(18, 14, 12, 10, 8, 8) * 60;
      gAttackManager.mBaseAttackInterval = interval;
      gAttackManager.mAttackInterval = interval;
      gNavalAttackManager.mBaseAttackInterval = interval;
      gNavalAttackManager.mAttackInterval = interval;

      data.mWallCircleAmount = 1;
      return true;
   };
   gHeroicTurtlerStrategy.mGetCurrentScore = []() -> float
   {
      const float rVal = cStrategyDefaultStrategyValue + 10.0; // Pick it over the default strategy.
      debugStrategy(gHeroicTurtlerStrategy.mName + " has a value of " + rVal + ".");
      return rVal;
   };
   gHeroicTurtlerStrategy.mUpdateFunc = [](ref StrategyData data) -> bool 
   {
      patrolBaseGates(data); 
      return kbPlayerGetAge(cMyID) == cAge3; 
   };   
   gHeroicTurtlerStrategy.mName = "Heroic Turtler";
}