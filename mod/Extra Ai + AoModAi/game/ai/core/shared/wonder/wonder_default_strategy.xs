//==============================================================================
// initDefaultWonderStrategy
//==============================================================================
void initDefaultWonderStrategy()
{
   gWonderStrategy.mID = cStrategyWonder;
   gWonderStrategy.mAge = cAge5;
   gWonderStrategy.mInitFunc = [](ref StrategyData data) -> bool
   {
      data.enableAllStrategyFlags();
      // Keep Mythic age attack timings.

      data.mWallCircleAmount = 1;

      return true;
   };
   gWonderStrategy.mGetCurrentScore = []() -> float
   {
      debugStrategy(gWonderStrategy.mName + " has a value of " + cStrategyDefaultStrategyValue + ".");
      return cStrategyDefaultStrategyValue;
   };
   gWonderStrategy.mUpdateFunc = [](ref StrategyData data) -> bool
   {
      return kbPlayerGetAge(cMyID) == cAge5;
   };
   gWonderStrategy.mName = "Default Wonder";
}