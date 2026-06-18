//==============================================================================
/* resource_breakdown_system.xs

   This file is intended for selecting what resources are valid to gather from and how many gather plans we will make.

*/

const float remoteFoodRange = 100.0;
const float remoteWoodRange = 250.0;
const float remoteGoldRange = 250.0;

//==============================================================================
// Builds our mAll*type*Resources arrays. The actual KB Resource IDs are saved in here and can then later be fetched using the offsets.
int internalAppendValidResourcesByBase(ref int[] array, int baseID = -1, int resourceType = -1, int resourceSubType = -1,
   float dangerThreshold = 100.0)
{
   int totalResourcesFound = 0;
   int[] resources = kbGetValidResourcesByPosition(kbBaseGetLocation(cMyID, baseID), resourceType, resourceSubType,
      kbBaseGetDistance(cMyID, baseID), dangerThreshold);
   for (int iResource = 0; iResource < resources.size(); iResource++)
   {
      // If this add was actually unique, increment totalResourcesFound.
      int oldArraySize = array.size();
      array.uniqueAdd(resources[iResource]);
      if (array.size() > oldArraySize)
      {
         totalResourcesFound++;
      }
   }
   return totalResourcesFound;
}

int internalAppendNearbyGoldResourcesByBase(ref int[] array, int baseID = -1, float dangerThreshold = 100.0)
{
   if (kbBaseGetIsIDValid(cMyID, baseID) == false)
   {
      return 0;
   }

   vector baseLocation = kbBaseGetLocation(cMyID, baseID);
   int baseAreaGroupID = kbAreaGroupGetIDByPosition(baseLocation);
   float baseDistance = kbBaseGetDistance(cMyID, baseID);
   int totalResourcesFound = 0;

   static int goldQueryID = -1;
   if (goldQueryID == -1)
   {
      goldQueryID = kbUnitQueryCreate("RBD amphibious gold resources");
      kbUnitQuerySetPlayerID(goldQueryID, cPlayerMotherNatureID);
      kbUnitQuerySetUnitType(goldQueryID, cUnitTypeGoldResource);
      kbUnitQuerySetState(goldQueryID, cUnitStateAlive);
      kbUnitQuerySetVisibleState(goldQueryID, cUnitQueryVisibleStateRecentPositionKnown);
   }
   if (goldQueryID == -1)
   {
      return 0;
   }

   kbUnitQuerySetPosition(goldQueryID, baseLocation);
   kbUnitQuerySetMaximumDistance(goldQueryID, baseDistance);
   kbUnitQueryResetResults(goldQueryID);
   int numResults = kbUnitQueryExecute(goldQueryID);
   for (int i = 0; i < numResults; i++)
   {
      int unitID = kbUnitQueryGetResult(goldQueryID, i);
      int kbResourceID = kbUnitGetKBResourceID(unitID);
      if (kbResourceGetIsIDValid(kbResourceID) == false ||
          kbResourceGetType(kbResourceID) != cResourceGold ||
          kbResourceGetSubType(kbResourceID) != cAIResourceSubTypeEasy)
      {
         continue;
      }

      vector resourceLocation = kbUnitGetPosition(unitID);
      int resourceAreaID = kbAreaGetIDByPosition(resourceLocation);
      if (resourceAreaID != -1 && kbAreaGetDangerLevel(resourceAreaID) > dangerThreshold)
      {
         continue;
      }

      int resourceAreaGroupID = kbAreaGroupGetIDByPosition(resourceLocation);
      if (baseAreaGroupID != -1 && resourceAreaGroupID != -1 &&
          kbPathAreAreaGroupsConnected(baseAreaGroupID, resourceAreaGroupID, cPassabilityAmphibious) == false)
      {
         continue;
      }

      int oldArraySize = array.size();
      array.uniqueAdd(kbResourceID);
      if (array.size() > oldArraySize)
      {
         totalResourcesFound++;
         debugResourceBreakdown("Added amphibious-accessible gold resource " + kbResourceID + " to base " +
            kbBaseGetNameByID(cMyID, baseID) + ".");
      }
   }
   return totalResourcesFound;
}

// The last index in the array holds the number of resources that we found in total, first index is always 0.
// If we have 5 bases and each base had 2 resources the array looks like this: 0-2-4-6-8-10.
// If you now ask how many resources are in gather base at index 3 (4th base) it goes like this:
// Take the value at index 3 = 6, this is the value of how many resources we found after analyzing 3 bases, but we want base 4.
// To calculate that we take the value at index 4 and reduce it by the value at index 3, which would be 8 - 6 = 2.
// The arrays that are used for this are called *type*ResourcesOffset.
int internalBaseNumberResources(ref int[] offsetArray, int index = -1)
{
   int start = offsetArray[index];
   if (index + 1 < offsetArray.size())
   {
      return offsetArray[index + 1] - start;
   }
   aiEchoWarning("internalBaseNumberResources - something is off with the logic as we should never reach here.");
   return 0;
}

// start should be *type*ResourcesOffset[iBase]. This start point is effectively how many resources
// were found by all the bases before us, which is our starting index. Then we loop over num resources and tally up how many units they have.
int internalBaseNumberResourceUnits(ref int[] resourceArray, int start = -1, int num = -1)
{
   int ret = 0;
   int end = min(start + num, resourceArray.size());
   for (int i = start; i < end; i++)
   {
      // There can be 1/2 frames delay between populating this array and using it, guard against resources that went away.
      if (kbResourceGetIsIDValid(resourceArray[i]) == true)
      {
         ret += kbResourceGetNumberUnits(resourceArray[i]);
      }
   }
   return ret;
}

// start should be *type*ResourcesOffset[iBase]. This start point is effectively how many resources
// were found by all the bases before us, which is our starting index. Then we loop over num resources and return their IDs.
void internalBaseKBResourceIDs(ref int[] resourceArray, ref int[] kbResourceIDS, int start = -1, int num = -1)
{
   int end = min(start + num, resourceArray.size());
   for (int i = start; i < end; i++)
   {
      kbResourceIDS.add(resourceArray[i]);
   }
}

// Helper to remove all breakdowns from a base.
void removeAllBreakdownsFromBase(int baseID = -1)
{
   aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeEasy, baseID);
   aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeHerdable, baseID);
   aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeHunt, baseID);
   aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeHuntAggressive, baseID);
   aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeFarm, baseID);
   aiRemoveResourceBreakdown(cResourceWood, cAIResourceSubTypeEasy, baseID);
   aiRemoveResourceBreakdown(cResourceGold, cAIResourceSubTypeEasy, baseID);
   aiRemoveResourceBreakdown(cResourceFavor, cAIResourceSubTypeEasy, baseID);
}

int internalPrio = -1;
int internalReturnAndDecrement()
{
   int ret = internalPrio;
   internalPrio = internalPrio -1;
   return ret;
}

class ResourceBreakdownSystem
{
   int mMaxFarmsPerBase = 15;
   void setMaxFarmsPerBase(int newAmount = -1)
   {
      mMaxFarmsPerBase = newAmount;
   }
   int mMaxFarmsPerIteration = 3;
   void setMaxFarmsPerIteration(int newAmount = -1)
   {
      mMaxFarmsPerIteration = newAmount;
   }

   int mPrioGoldEasy = 0;
   int mPrioWoodEasy = 0;
   int mPrioFavorEasy = 0;

   int mPrioFoodEasy = 0;
   int mPrioFoodHerdable = 0;
   int mPrioFoodHunt = 0;
   int mPrioFoodAggressive = 0;
   int mPrioFoodFarm = 0;

   void internalSetFoodPrio(bool prioritizeFarms = false)
   {
      if (prioritizeFarms == true)
      {
         mPrioFoodFarm = internalReturnAndDecrement();
         mPrioFoodAggressive = internalReturnAndDecrement();
         mPrioFoodHunt = internalReturnAndDecrement();
         mPrioFoodEasy = internalReturnAndDecrement();
         mPrioFoodHerdable = internalReturnAndDecrement();
      }
      else
      {
         mPrioFoodAggressive = internalReturnAndDecrement();
         mPrioFoodHunt = internalReturnAndDecrement();
         mPrioFoodEasy = internalReturnAndDecrement();
         mPrioFoodHerdable = internalReturnAndDecrement();
         mPrioFoodFarm = internalReturnAndDecrement();
      }
   }

   void resourceBreakdownUpdateGatherPlanPriorities(bool prioritizeFarms = false)
   {
      // This prio is lower than build plans so that they can always steal from us in their Need section.
      internalPrio = 30;
      float foodPercentage = aiGetResourcePercentage(cResourceFood);
      float woodPercentage = aiGetResourcePercentage(cResourceWood);
      float goldPercentage = aiGetResourcePercentage(cResourceGold);
      float favorPercentage = aiGetResourcePercentage(cResourceFavor);
      debugResourceBreakdown("Food: " + foodPercentage + " Wood: " + woodPercentage
       + " Gold: " + goldPercentage + " Favor: " + favorPercentage);
      if (goldPercentage > woodPercentage && goldPercentage > foodPercentage)
      {
         mPrioGoldEasy = internalReturnAndDecrement();
         if (woodPercentage > foodPercentage)// Gold > wood > food
         { 
            mPrioWoodEasy = internalReturnAndDecrement();
            internalSetFoodPrio(prioritizeFarms);
         }
         else// Gold > food >= wood
         { 
            internalSetFoodPrio(prioritizeFarms);
            mPrioWoodEasy = internalReturnAndDecrement();
         }
      }
      else if (woodPercentage > foodPercentage)
      {
         mPrioWoodEasy = internalReturnAndDecrement();
         if (goldPercentage > foodPercentage)// Wood > gold > food
         { 
            mPrioGoldEasy = internalReturnAndDecrement();
            internalSetFoodPrio(prioritizeFarms);
         }
         else// wood > food >= gold
         { 
            internalSetFoodPrio(prioritizeFarms);
            mPrioGoldEasy = internalReturnAndDecrement();
         }
      }
      else
      {
         if (goldPercentage >= woodPercentage)// food >= gold > wood
         { 
            internalSetFoodPrio(prioritizeFarms);
            mPrioGoldEasy = internalReturnAndDecrement();
            mPrioWoodEasy = internalReturnAndDecrement();
         }
         else
         { // food >= wood > gold
            internalSetFoodPrio(prioritizeFarms);
            mPrioWoodEasy = internalReturnAndDecrement();
            mPrioGoldEasy = internalReturnAndDecrement();
         }
      }
      mPrioFavorEasy = internalReturnAndDecrement();
   }

   bool mGatherFood = true;
   bool mGatherWood = true;
   bool mGatherGold = true;
   bool mGatherFavor = true;

   int mLastAssignedFoodGathererNoFarmersCount = 0;
   int mLastAssignedNumberFarmers = 0;
   int mLastAssignedMaxFoodGatherers = 0;
   bool mOverride = false;

   int[] mSortedTCBases = default;

   bool mAllowAutoRemoteCreation = true;
   int[] mGatherBases = default;
   int[] mInternalRemoteGatherBases = default;
   int[] mAllHuntResources = default; 
   int[] mAllAggressiveResources = default; 
   int[] mAllEasyResources = default; 
   int[] mAllHerdableResources = default; 
   int[] mSafeFarmResources = default; 
   int[] mAllEasyWoodResources = default; 
   int[] mAllEasyGoldResources = default; 

   int[] mHuntResourceOffset = default; 
   int[] mAggressiveResourcesOffset = default; 
   int[] mEasyResourcesOffset = default; 
   int[] mHerdableResourcesOffset = default; 
   int[] mSafeFarmResourcesOffset = default; 
   int[] mEasyWoodResourcesOffset = default; 
   int[] mEasyGoldResourcesOffset = default; 

   //==============================================================================
   // internalCalculateBaseResourceOffsets
   //==============================================================================
   void internalCalculateBaseResourceOffsets()
   {
      mHuntResourceOffset.add(mAllHuntResources.size());
      mAggressiveResourcesOffset.add(mAllAggressiveResources.size());
      mEasyResourcesOffset.add(mAllEasyResources.size());
      mHerdableResourcesOffset.add(mAllHerdableResources.size());
      mSafeFarmResourcesOffset.add(mSafeFarmResources.size());

      mEasyGoldResourcesOffset.add(mAllEasyGoldResources.size());
      mEasyWoodResourcesOffset.add(mAllEasyWoodResources.size());
   }

   //==============================================================================
   // internalFixupBaseResourceOffsets
   //==============================================================================
   void internalFixupBaseResourceOffsets()
   {
      mHuntResourceOffset.removeIndex(mHuntResourceOffset.size() - 1);
      mAggressiveResourcesOffset.removeIndex(mAggressiveResourcesOffset.size() - 1);
      mEasyResourcesOffset.removeIndex(mEasyResourcesOffset.size() - 1);
      mHerdableResourcesOffset.removeIndex(mHerdableResourcesOffset.size() - 1);
      mSafeFarmResourcesOffset.removeIndex(mSafeFarmResourcesOffset.size() - 1);

      mEasyGoldResourcesOffset.removeIndex(mEasyGoldResourcesOffset.size() - 1);
      mEasyWoodResourcesOffset.removeIndex(mEasyWoodResourcesOffset.size() - 1);
   }

   //==============================================================================
   // internalClear
   //==============================================================================
   void internalClear()
   {
      for (int i = mSortedTCBases.size() - 1; i >= 0 ; i--)
      {
         int baseID = mSortedTCBases[i];
         // Remove invalid bases as well as bases that are no longer a TC base.
         if (kbBaseGetIsIDValid(cMyID, baseID) == false ||
            // If we lose our last TC base but it's still the main base we don't remove it, otherwise all gathering is done for.
            (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false && kbBaseIsFlagSet(cMyID, baseID, cBaseFlagMain) == false))
         {
            removeAllBreakdownsFromBase(baseID);
            mSortedTCBases.removeIndex(i);
         }
         // Also remove bases that are no longer connected to our prefered area group.
         else if (gLandAreaGroupID != -1 &&
            kbPathAreAreaGroupsConnected(gLandAreaGroupID,
            kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityAmphibious) == false)
         {
            removeAllBreakdownsFromBase(baseID);
            mSortedTCBases.removeIndex(i);
         }
      }

      for (int i = mInternalRemoteGatherBases.size() - 1; i >= 0 ; i--)
      {
         int baseID = mInternalRemoteGatherBases[i];
         // Remove invalid bases.
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            removeAllBreakdownsFromBase(baseID);
            mInternalRemoteGatherBases.removeIndex(i);
         }
      }
      mGatherBases.clear();
      mAllHuntResources.clear();
      mAllAggressiveResources.clear();
      mAllEasyResources.clear();
      mAllHerdableResources.clear();
      mSafeFarmResources.clear();
      mAllEasyGoldResources.clear();
      mAllEasyWoodResources.clear();
      mHuntResourceOffset.clear();
      mAggressiveResourcesOffset.clear();
      mEasyResourcesOffset.clear();
      mHerdableResourcesOffset.clear();
      mSafeFarmResourcesOffset.clear();
      mEasyGoldResourcesOffset.clear();
      mEasyWoodResourcesOffset.clear();
   }

   //==============================================================================
   // internalScanBaseResources
   //==============================================================================
   int internalScanBaseResources(int baseID = -1)
   {
      int totalResourcesFound = 0;
      if (mGatherFood == true)
      {
         totalResourcesFound += internalAppendValidResourcesByBase(mAllHuntResources, baseID, cResourceFood, cAIResourceSubTypeHunt);
         totalResourcesFound += internalAppendValidResourcesByBase(mAllAggressiveResources, baseID, cResourceFood, cAIResourceSubTypeHuntAggressive);
         totalResourcesFound += internalAppendValidResourcesByBase(mAllEasyResources, baseID, cResourceFood, cAIResourceSubTypeEasy);
         totalResourcesFound += internalAppendValidResourcesByBase(mAllHerdableResources, baseID, cResourceFood, cAIResourceSubTypeHerdable);
         totalResourcesFound += internalAppendValidResourcesByBase(mSafeFarmResources, baseID, cResourceFood, cAIResourceSubTypeFarm);
      }

      if (mGatherGold == true)
      {
         totalResourcesFound += internalAppendValidResourcesByBase(mAllEasyGoldResources, baseID, cResourceGold, cAIResourceSubTypeEasy);
         totalResourcesFound += internalAppendNearbyGoldResourcesByBase(mAllEasyGoldResources, baseID);
      }
      if (mGatherWood == true)
      {
         totalResourcesFound += internalAppendValidResourcesByBase(mAllEasyWoodResources, baseID, cResourceWood, cAIResourceSubTypeEasy);
      }
      debugResourceBreakdown("Adding " + kbBaseGetNameByID(cMyID, baseID) + " to mGatherBases.");
      mGatherBases.add(baseID);
      return totalResourcesFound;
   }

   //==============================================================================
   // internalCreateRemoteBase
   // This function has 2 parts.
   // 1. Searching within 100.0 meters of any TC/Mainbase with a giga influence towards edge of map.
   // 2. Searching within the provided maxDistance with regular influences.
   // We do it this way so that we try to grab safe resources to the back first always. Nothing worse then going out to the center
   // of the map for resources when we had save stuff in the back...
   // Imperfection: this distance to edge also applies to sides of the map ofc, so it doesn't per se take true back resources
   // it can also take side resources, just not forward...
   //==============================================================================
   int internalCreateRemoteGoldBaseFallback(float maxDistance = 250.0, int gatherBaseTypeFlag = cBaseFlagRemoteGoldGatherBase)
   {
      static int goldQueryID = -1;
      if (goldQueryID == -1)
      {
         goldQueryID = kbUnitQueryCreate("RBD remote amphibious gold");
         kbUnitQuerySetPlayerID(goldQueryID, cPlayerMotherNatureID);
         kbUnitQuerySetUnitType(goldQueryID, cUnitTypeGoldResource);
         kbUnitQuerySetState(goldQueryID, cUnitStateAlive);
         kbUnitQuerySetVisibleState(goldQueryID, cUnitQueryVisibleStateRecentPositionKnown);
         kbUnitQuerySetAscendingSort(goldQueryID, true);
      }
      if (goldQueryID == -1)
      {
         return -1;
      }

      for (int iBase = 0; iBase < mSortedTCBases.size(); iBase++)
      {
         int baseID = mSortedTCBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue;
         }

         vector baseLocation = kbBaseGetLocation(cMyID, baseID);
         int baseAreaGroupID = kbAreaGroupGetIDByPosition(baseLocation);
         kbUnitQuerySetPosition(goldQueryID, baseLocation);
         kbUnitQuerySetMaximumDistance(goldQueryID, maxDistance);
         kbUnitQueryResetResults(goldQueryID);
         int numResults = kbUnitQueryExecute(goldQueryID);
         for (int i = 0; i < numResults; i++)
         {
            int unitID = kbUnitQueryGetResult(goldQueryID, i);
            int kbResourceID = kbUnitGetKBResourceID(unitID);
            if (kbResourceGetIsIDValid(kbResourceID) == false ||
                kbResourceGetType(kbResourceID) != cResourceGold ||
                kbResourceGetSubType(kbResourceID) != cAIResourceSubTypeEasy ||
                mAllEasyGoldResources.find(kbResourceID) != -1)
            {
               continue;
            }

            vector resourceLocation = kbUnitGetPosition(unitID);
            int resourceAreaGroupID = kbAreaGroupGetIDByPosition(resourceLocation);
            if (baseAreaGroupID != -1 && resourceAreaGroupID != -1 &&
                kbPathAreAreaGroupsConnected(baseAreaGroupID, resourceAreaGroupID, cPassabilityAmphibious) == false)
            {
               continue;
            }

            vector remoteBaseLocation = kbFindClosestPassablePoint(resourceLocation, cPassabilityLand, 18.0);
            if (remoteBaseLocation == cInvalidVector)
            {
               remoteBaseLocation = kbFindClosestPassablePoint(resourceLocation, cPassabilityAmphibious, 18.0);
            }
            if (remoteBaseLocation == cInvalidVector)
            {
               remoteBaseLocation = resourceLocation;
            }

            int remoteBaseID = kbBaseCreate(cMyID, kbBaseGetNextID() + ": RemoteGoldBase", remoteBaseLocation, 24.0);
            if (kbBaseGetIsIDValid(cMyID, remoteBaseID) == false)
            {
               continue;
            }

            kbBaseSetDistance(cMyID, remoteBaseID, 24.0);
            kbBaseSetFlag(cMyID, remoteBaseID, cBaseFlagRemoteGatherBase, true);
            kbBaseSetFlag(cMyID, remoteBaseID, cBaseFlagEconomy, true);
            kbBaseSetFlag(cMyID, remoteBaseID, gatherBaseTypeFlag, true);
            mInternalRemoteGatherBases.add(remoteBaseID);
            internalCalculateBaseResourceOffsets();
            internalScanBaseResources(remoteBaseID);
            debugResourceBreakdown("           *** Created fallback amphibious remote gold base: " +
               kbBaseGetNameByID(cMyID, remoteBaseID) + ". ***");
            return remoteBaseID;
         }
      }
      return -1;
   }

   int internalCreateRemoteBase(int resourceType = -1, int resourceSubtype = -1, float maxDistance = 250.0)
   {
      // Change the factor now, reset it each time before we hit a return statement.
      kbSetResourceSelectorFactor(cTSFactorDistanceToEdge, resourceType, -200.0);
      int gatherBaseTypeFlag = cBaseFlagRemoteFoodGatherBase;
      if (resourceType == cResourceWood)
      {
         gatherBaseTypeFlag = cBaseFlagRemoteWoodGatherBase;
      }
      else if (resourceType == cResourceGold)
      {
         gatherBaseTypeFlag = cBaseFlagRemoteGoldGatherBase;
      }

      for (int i = 0; i < mSortedTCBases.size(); i++)
      {
         int baseID = mSortedTCBases[i];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue;
         }
         // 100.0/maxDistance.
         int remoteBaseID = kbBaseFindOrCreateResourceBase(kbBaseGetLocation(cMyID, baseID), resourceType, resourceSubtype,
            min(100.0, maxDistance), 100.0);
         if (kbBaseGetIsIDValid(cMyID, remoteBaseID) == true)
         {
            // Reset influence.
            kbSetResourceSelectorFactor(cTSFactorDistanceToEdge, resourceType, -10.0);
            if (mGatherBases.find(remoteBaseID) == -1)
            {
               kbBaseSetDistance(cMyID, remoteBaseID, 20.0);
               kbBaseSetFlag(cMyID, remoteBaseID, cBaseFlagRemoteGatherBase, true);
               kbBaseSetFlag(cMyID, remoteBaseID, cBaseFlagEconomy, true);
               kbBaseSetFlag(cMyID, remoteBaseID, gatherBaseTypeFlag, true);
               mInternalRemoteGatherBases.add(remoteBaseID);
               // Add this base to our internal tracking.
               internalCalculateBaseResourceOffsets();
               internalScanBaseResources(remoteBaseID);
               return remoteBaseID;
            }
            else
            {
               // If we already have the base we need to set the appropriate remote gather base type.
               kbBaseSetFlag(cMyID, remoteBaseID, gatherBaseTypeFlag, true);
               debugResourceBreakdown("           *** " + kbBaseGetNameByID(cMyID, remoteBaseID) + " of size: " +
                  kbBaseGetDistance(cMyID, remoteBaseID) + " is now also tagged as a " + kbGetResourceName(resourceType) +
                  " remote base.");
               return remoteBaseID;
            }
         }
      }

      // Reset influence.
      kbSetResourceSelectorFactor(cTSFactorDistanceToEdge, resourceType, -10.0);
      // Found nothing and our maxDistance isn't higher than what we already analyzed, we've failed.
      if (maxDistance <= 100.0)
      {
         if (resourceType == cResourceGold)
         {
            return internalCreateRemoteGoldBaseFallback(maxDistance, gatherBaseTypeFlag);
         }
         return -1;
      }

      for (int i = 0; i < mSortedTCBases.size(); i++)
      {
         int baseID = mSortedTCBases[i];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue;
         }
         int remoteBaseID = kbBaseFindOrCreateResourceBase(kbBaseGetLocation(cMyID, baseID), resourceType, resourceSubtype,
            maxDistance, 100.0);
         if (kbBaseGetIsIDValid(cMyID, remoteBaseID) == true)
         {
            if (mGatherBases.find(remoteBaseID) == -1)
            {
               kbBaseSetDistance(cMyID, remoteBaseID, 20.0);
               kbBaseSetFlag(cMyID, remoteBaseID, cBaseFlagRemoteGatherBase, true);
               kbBaseSetFlag(cMyID, remoteBaseID, cBaseFlagEconomy, true);
               kbBaseSetFlag(cMyID, remoteBaseID, gatherBaseTypeFlag, true);
               mInternalRemoteGatherBases.add(remoteBaseID);
               // Add this base to our internal tracking.
               internalCalculateBaseResourceOffsets();
               internalScanBaseResources(remoteBaseID);
               return remoteBaseID;
            }
            else
            {
               // If we already have the base we need to set the appropriate remote gather base type.
               kbBaseSetFlag(cMyID, remoteBaseID, gatherBaseTypeFlag, true);
               debugResourceBreakdown("           *** " + kbBaseGetNameByID(cMyID, remoteBaseID) + " of size: " +
                  kbBaseGetDistance(cMyID, remoteBaseID) + " is now also tagged as a " + kbGetResourceName(resourceType) +
                  " remote base.");
               return remoteBaseID;
            }
         }
      }
      if (resourceType == cResourceGold)
      {
         return internalCreateRemoteGoldBaseFallback(maxDistance, gatherBaseTypeFlag);
      }
      return -1;
   }

   //==============================================================================
   // internalSortBases
   //==============================================================================
   void internalSortBases(ref int[] bases, string arrayName = "bases")
   {
      debugResourceBreakdown("Previous order of the " + arrayName + " array:");
      for (int i = 0; i < bases.size(); i++)
      {
         debugResourceBreakdown("   " + kbBaseGetNameByID(cMyID, bases[i]));
      }
      bool firstShuffle = true;
      int numReshuffles = 1; // Start with not analyzing the last base since it makes no sense.
      debugResourceBreakdown("Analyzing if we should make changes to the " + arrayName + " array:");
      for (int i = 0; i < bases.size() - numReshuffles; i++)
      {
         int areaID = kbAreaGetIDByPosition(kbBaseGetLocation(cMyID, bases[i]));
         if (kbAreaGetDangerLevel(areaID) <= 100.0)
         {
            debugResourceBreakdown("   " + kbBaseGetNameByID(cMyID, bases[i]) + " is not dangerous, don't reshuffle it.");
            continue;
         }
         // This is a dangerous base, put it at the back of our array.
         debugResourceBreakdown("   " + kbBaseGetNameByID(cMyID, bases[i]) + " is dangerous, adding it at the back.");
         bases.add(bases[i]);
         bases.removeIndex(i);
         i--;
         if (firstShuffle == false)
         {
            numReshuffles++; // Don't analyze the same base twice.
         }
         firstShuffle = false;
      }
      if (firstShuffle == false) // We have shuffled.
      {
         debugResourceBreakdown("Current order of the " + arrayName + " array:");
         for (int i = 0; i < bases.size(); i++)
         {
            debugResourceBreakdown("   " + kbBaseGetNameByID(cMyID, bases[i]));
         }
      }
   }

   //==============================================================================
   // scanForBases
   //==============================================================================
   void scanForBases()
   {
      internalClear();
      // If we're overriden in a scenario then scan just remote bases
      if (mOverride == true)
      {
         if (mInternalRemoteGatherBases.size() > 1)
         {
            internalSortBases(mInternalRemoteGatherBases);
         }

         debugResourceBreakdown("Gather base override num bases: " + mInternalRemoteGatherBases.size());
         for (int i = 0; i < mInternalRemoteGatherBases.size(); i++)
         {
            int baseID = mInternalRemoteGatherBases[i];
            debugResourceBreakdown("Gather baseID: " + baseID);
            internalCalculateBaseResourceOffsets();
            // Assume that override bases are not remote
            internalScanBaseResources(baseID);
         }
         // Add one last entry so the last base can also easily calculate it
         internalCalculateBaseResourceOffsets();
         return;
      }

      // Add bases to our TC array if we didn't have them already.
      int numberBases = kbBaseGetNumber(cMyID);
      for (int i = 0; i < numberBases; i++)
      {
         int baseID = kbBaseGetIDByIndex(cMyID, i);
         if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
         {
            continue;
         }
         if (gLandAreaGroupID != -1 &&
             kbPathAreAreaGroupsConnected(gLandAreaGroupID,
             kbAreaGroupGetIDByPosition(kbBaseGetLocation(cMyID, baseID)), cPassabilityAmphibious) == false)
         {
            continue;
         }
         if (mSortedTCBases.find(baseID) != -1)
         {
            continue;
         }
         mSortedTCBases.add(baseID);
         debugResourceBreakdown(kbBaseGetNameByID(cMyID, baseID) + " adding this base to mSortedTCBases.");
      }

      // Only sort them if we have more than 1 base.
      if (mSortedTCBases.size() > 1)
      {
         internalSortBases(mSortedTCBases);
      }

      for (int i = 0; i < mSortedTCBases.size(); i++)
      {
         internalCalculateBaseResourceOffsets();
         internalScanBaseResources(mSortedTCBases[i]);
      }

      // Only sort them if we have more than 1 remote base.
      if (mInternalRemoteGatherBases.size() > 1)
      {
         internalSortBases(mInternalRemoteGatherBases);
      }

      for (int i = 0; i < mInternalRemoteGatherBases.size(); i++)
      {
         internalCalculateBaseResourceOffsets();
         int numResources = internalScanBaseResources(mInternalRemoteGatherBases[i]);
         // Remove the remote gather base flag from remote bases that have 0 resources left.
         // Only do this if the area isn't currently dangerous because that would block internalScanBaseResources from finding
         // still existing resources.
         if (numResources <= 0 &&
             kbAreaGetDangerLevel(kbAreaGetIDByPosition(kbBaseGetLocation(cMyID, mInternalRemoteGatherBases[i]))) <= 100.0)
         {
            kbBaseSetFlag(cMyID, mInternalRemoteGatherBases[i], cBaseFlagRemoteGatherBase, false);
            kbBaseSetFlag(cMyID, mInternalRemoteGatherBases[i], cBaseFlagRemoteFoodGatherBase, false);
            kbBaseSetFlag(cMyID, mInternalRemoteGatherBases[i], cBaseFlagRemoteWoodGatherBase, false);
            kbBaseSetFlag(cMyID, mInternalRemoteGatherBases[i], cBaseFlagRemoteGoldGatherBase, false);
            // internalScanBaseResources added this base to mGatherBases, remove it again.
            mGatherBases.removeValue(mInternalRemoteGatherBases[i]);
            internalFixupBaseResourceOffsets(); // Remove the offset we just added for this base since it has been removed.
            debugResourceBreakdown(kbBaseGetNameByID(cMyID, mInternalRemoteGatherBases[i]) + " no longer has resources in it, " + 
               "removing remote gather base flags and removing it from the gather bases.");
            removeAllBreakdownsFromBase(mInternalRemoteGatherBases[i]);
            mInternalRemoteGatherBases.removeIndex(i);
            i--;
         }
      }
      
      // Add one last entry so the last base can also easily calculate it
      internalCalculateBaseResourceOffsets();

      // Assert that our arrays are alligned.
      if (mHuntResourceOffset.size() - 1 != mGatherBases.size())
      {
         debugResourceBreakdown("mHuntResourceOffset.size() : " + mHuntResourceOffset.size() + ", mGatherBases.size() : " +
            mGatherBases.size());
         aiEchoWarning("Resource arrays are going out of sync with the offset arrays, offset is meant to be 1 bigger.");
      }
   }

   //==============================================================================
   // applyResourceFlags
   //==============================================================================
   void applyResourceFlags(bool food = true, bool wood = true, bool gold = true, bool favor = true)
   {
      mGatherFood = food;
      mGatherWood = wood;
      mGatherGold = gold;
      mGatherFavor = favor;
      // We might need to remove some breakdowns from our bases
      int numberBases = mGatherBases.size();
      for (int i = 0; i < numberBases; i++)
      {
         int baseID = mGatherBases[i];
         if (mGatherFood == false)
         {
            aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeEasy, baseID);
            aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeHerdable, baseID);
            aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeHunt, baseID);
            aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeHuntAggressive, baseID);
            aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeFarm, baseID);
         }
         if (mGatherWood == false)
         {
            aiRemoveResourceBreakdown(cResourceWood, cAIResourceSubTypeEasy, baseID);
         }
         if (mGatherGold == false)
         {
            aiRemoveResourceBreakdown(cResourceGold, cAIResourceSubTypeEasy, baseID);
         }
         if (mGatherFavor == false)
         {
            aiRemoveResourceBreakdown(cResourceFavor, cAIResourceSubTypeEasy, baseID);
         }
      }
   }
   
   //==============================================================================
   // nullPercentageForBase
   //==============================================================================
   void nullPercentageForBase(int index = 0, int resourceType = -1, int resourceSubtype = cAIResourceSubTypeEasy)
   {
      int baseID = mGatherBases[index];
      if (kbBaseGetIsIDValid(cMyID, baseID) == false)
      {
         return; // Base array is made and used in consecutives frames, bases can go invalid.
      }
      if (aiGetResourceBreakdownID(resourceType, resourceSubtype, baseID) == -1)
      {
         return; // Don't set a null state if we don't have the breakdown to begin with.
      }
      // Set our wanted plans + percentage to 0.
      aiSetResourceBreakdown(resourceType, resourceSubtype, 0, 50, baseID);
   }

   //==============================================================================
   // nullPercentageForBases
   //==============================================================================
   void nullPercentageForBases(int startIndex = 0, int resourceType = -1, int resourceSubtype = cAIResourceSubTypeEasy)
   {
      int numberBases = mGatherBases.size();
      for (int i = startIndex; i < numberBases; i++)
      {
         nullPercentageForBase(i, resourceType, resourceSubtype);
      }
   }

   //==============================================================================
   // resourceBreakdownUpdateExistingFood
   // In this function we try to keep our existing food gather plans alive, even if "better" food sources came available.
   // We prefer staying on berries that we know we're successfully gathering from than walking to the next hunt that we scout.
   // Stability over opportunism basically.
   // We do not re-assign farms here since they're #1 prio in resourceBreakdownUpdateFood already so they will be properly handled.
   // We must however take our last num Farmers into account here so that we don't backfill plans with gatherers that are now farming.
   //==============================================================================
   int resourceBreakdownUpdateExistingFood(int totalGathererCount = -1, bool canMicro = false, ref int totalAssigned)
   {
      debugResourceBreakdown("--- Trying to assign food gatherers to existing plans ---");
      int totalFoodGathererCount = round(aiGetResourcePercentage(cResourceFood) * totalGathererCount);
      if (gOverrideOkToGatherFood == false)
      {
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeEasy);
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeFarm);
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeHerdable);
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeHunt);
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeHuntAggressive);
         debugResourceBreakdown("     Not allowed to gather food via breakdowns because of gOverrideOkToGatherFood == false.");
         return totalFoodGathererCount;
      }
      if (totalFoodGathererCount == 0)
      {
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeEasy);
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeFarm);
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeHerdable);
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeHunt);
         nullPercentageForBases(0, cResourceFood, cAIResourceSubTypeHuntAggressive);
         debugResourceBreakdown("     No food gatherers to assign.");
         return totalFoodGathererCount;
      }

      // foodGathererCount will equal the amount of gatherers we want to assign in this function, it will be less than
      // total food gatherers if we're already farming, since those farmers need to persist.
      int foodGathererCount = max(0, totalFoodGathererCount - mLastAssignedNumberFarmers);
      // Compare our new food gatherer count against how many gatherers we assigned last run minus farmers.
      // That should give a number which indicates if we have more/fewer gatherers to assign this run.
      int deltaGatherers = foodGathererCount - mLastAssignedFoodGathererNoFarmersCount;
      int totalAssignedFoodGatherers = 0;
      // Farmers get prio over everything, if we had this many farmers before we don't want to potentially put them in these plans.
      debugResourceBreakdown("     totalFoodGathererCount: " + totalFoodGathererCount + ", deltaGatherers: " + deltaGatherers +
         ", mLastAssignedNumberFarmers: " + mLastAssignedNumberFarmers + ", will try to re-assign " + foodGathererCount + ".");
      int unassigned = foodGathererCount;
      int numBases = mGatherBases.size();

      // Preference order is:
      // - Existing plans (this function only deals with this part)
      // - Existing farms (but not in age 1)
      // - Aggressive hunt
      // - Passive Hunt
      // - Berries / Chickens.
      // - Herdables
      // - New farms
      
      //////////////
      // Reactive hunt.
      //////////////
      // IF YOU CHANGE THESE NUMBERS ALSO CHANGE THEM IN resourceBreakdownUpdateFood.
      int numRequiredReactiveHunters = 0;
      int maxLeftoverReactiveHunters = 0;
      if (canMicro == true)
      {
         numRequiredReactiveHunters = cMyCulture == cCultureAtlantean ? 3 : 5;
         maxLeftoverReactiveHunters = cMyCulture == cCultureAtlantean ? 4 : 6;
      }
      else
      {
         numRequiredReactiveHunters = cMyCulture == cCultureAtlantean ? 4 : 8;
         maxLeftoverReactiveHunters = cMyCulture == cCultureAtlantean ? 3 : 3;
      }
      // We already have big numbers above, we can't put more or we pathblock ourself.
      int maxOverflowReactiveHunters = 0;
      for (int iBase = 0; iBase < numBases; iBase++)
      {
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         debugResourceBreakdown("     *** Reactive Hunts processing existing gather base: " + kbBaseGetNameByID(cMyID, baseID) +
            " of size: " + kbBaseGetDistance(cMyID, baseID) + ".");
            
         int breakDownID = aiGetResourceBreakdownID(cResourceFood, cAIResourceSubTypeHuntAggressive, baseID);
         if (breakDownID == -1)
         {
            debugResourceBreakdown("           Base had no existing reactive hunt gather plans.");
            continue;
         }
         int[] reactiveHuntPlans = aiPlanGetIDsByTypeAndVariableIntValue(cPlanGather, cGatherPlanBreakDownID, breakDownID);
         int numExistingReactiveHuntPlans = reactiveHuntPlans.size();
         
         int numBaseHunters = 0;
         int numHuntPlans = 0;
         int[] numNeeded = new int(0, 0);
         int[] numWanted = new int(0, 0);
         int[] numMax = new int(0, 0);
         // Assign per plan.
         for (int iPlan = 0; iPlan < numExistingReactiveHuntPlans; iPlan++)
         {
            int planID = reactiveHuntPlans[iPlan];
            int kbResourceID = aiPlanGetVariableInt(planID, cGatherPlanKBResourceID, 0);
            int numWantedUnits = aiPlanGetNumberWantedUnits(planID, cUnitTypeAbstractVillager);
            // No resource most likely means that we can't find any more, if we're precisely in a transition then the
            // default assignment will re-create us, no problem.
            if (kbResourceGetIsIDValid(kbResourceID) == false)
            {
               deltaGatherers += numWantedUnits; // Potentially give these to other existing plans.
               debugResourceBreakdown("           We don't have a valid KB Resource ID: " + aiPlanGetName(planID) + ", giving " +
                  " these units back to our deltaGatherers.");
               continue;
            }
            int areaID = kbAreaGetIDByPosition(kbResourceGetPosition(kbResourceID));
            if (areaID != -1)
            {
               if (kbAreaGetDangerLevel(areaID, false) > 100.0) // TODO unhardcode this value and unify with source + top of this file.
               {
                  deltaGatherers += numWantedUnits; // Potentially give these to other existing plans.
                  debugResourceBreakdown("           Danger rating for: " + aiPlanGetName(planID) + " is too high, giving " +
                     " these units back to our deltaGatherers.");
                  continue;
               }
            }

            if (unassigned < numWantedUnits)
            {
               if (unassigned >= numRequiredReactiveHunters)
               {
                  numNeeded.add(max(numRequiredReactiveHunters, unassigned - 2));
                  numWanted.add(unassigned);
                  numMax.add(unassigned + maxOverflowReactiveHunters);
                  debugResourceBreakdown("           We have to reduce the amount of gatherers assigned to: " +
                     aiPlanGetName(planID) + " from " + numWantedUnits + " to " + unassigned + ".");
               }
               else
               {
                  debugResourceBreakdown("           We don't have enough remaining food gatherers for: " + aiPlanGetName(planID)
                     + ".");
                  continue;
               }
            }
            else
            {
               if (deltaGatherers > 0)
               {
                  if (numWantedUnits < numRequiredReactiveHunters + maxLeftoverReactiveHunters)
                  {
                     int numWantedHunters = min(numRequiredReactiveHunters + maxLeftoverReactiveHunters, numWantedUnits + deltaGatherers);
                     // Calculate difference from what we had and what we will now assign, subtract that from deltaGatherers
                     // since we're claiming extra now.
                     deltaGatherers -= (numWantedHunters - numWantedUnits);
                     numNeeded.add(max(numRequiredReactiveHunters, numWantedHunters - 2));
                     numWanted.add(numWantedHunters);
                     numMax.add(numWantedHunters + maxOverflowReactiveHunters);
                     debugResourceBreakdown("           We have extra gatherers and " + aiPlanGetName(planID) +
                        " wasn't as max capacity yet so we filled it up with " + (numWantedHunters - numWantedUnits) +
                        " extra units. New wanted total: " + numWantedHunters + ".");
                  }
                  else
                  {
                     numNeeded.add(aiPlanGetNumberNeededUnits(planID, cUnitTypeAbstractVillager));
                     numWanted.add(numWantedUnits);
                     numMax.add(aiPlanGetNumberMaxUnits(planID, cUnitTypeAbstractVillager));
                     debugResourceBreakdown("           We have extra gatherers but " + aiPlanGetName(planID) +
                        " was already at max capacity of " + numWantedUnits + ", no changes.");
                  }
               }
               else
               {
                  numNeeded.add(aiPlanGetNumberNeededUnits(planID, cUnitTypeAbstractVillager));
                  numWanted.add(numWantedUnits);
                  numMax.add(aiPlanGetNumberMaxUnits(planID, cUnitTypeAbstractVillager));
                  debugResourceBreakdown("           We have don't have extra gatherers but " + aiPlanGetName(planID) +
                        " doesn't need to be reduced from " + numWantedUnits + " size.");
               }
            }

            numHuntPlans++;
            int index = numWanted.size() - 1;
            unassigned -= numWanted[index];
            numBaseHunters += numWanted[index];
            totalAssignedFoodGatherers += numWanted[index];
         }
   	   debugResourceBreakdown("           Num reactive hunters: " + numBaseHunters + ", keeping " + numHuntPlans + "/" +
            numExistingReactiveHuntPlans + " existing plans.");
         if (numHuntPlans > 0)
         {
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeHuntAggressive, numHuntPlans, mPrioFoodAggressive,
                                    baseID, numNeeded, numWanted, numMax);
         }
         else
         {
            aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeHuntAggressive, baseID);
         }
      }

      //////////////
      // Passive hunt.
      //////////////
      // IF YOU CHANGE THESE NUMBERS ALSO CHANGE THEM IN resourceBreakdownUpdateFood.
      int numTargetHunters = 4;
      int maxLeftoverHunters = 6;
      int maxOverflowHunters = 2;
      if (cMyCulture == cCultureAtlantean)
      {
         numTargetHunters = 2;
         maxLeftoverHunters = 5;
      }

      for (int iBase = 0; iBase < numBases; iBase++)
      {
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         debugResourceBreakdown("     *** Passive Hunts processing existing gather base: " + kbBaseGetNameByID(cMyID, baseID) +
            " of size: " + kbBaseGetDistance(cMyID, baseID) + ".");
            
         int breakDownID = aiGetResourceBreakdownID(cResourceFood, cAIResourceSubTypeHunt, baseID);
         if (breakDownID == -1)
         {
            debugResourceBreakdown("           Base had no existing passive hunt gather plans.");
            continue;
         }
         int[] huntPlans = aiPlanGetIDsByTypeAndVariableIntValue(cPlanGather, cGatherPlanBreakDownID, breakDownID);
         int numExistingHuntPlans = huntPlans.size();
         
         int numBaseHunters = 0;
         int numHuntPlans = 0;
         int[] numNeeded = new int(0, 0);
         int[] numWanted = new int(0, 0);
         int[] numMax = new int(0, 0);
         // Assign per plan.
         for (int iPlan = 0; iPlan < numExistingHuntPlans; iPlan++)
         {
            int planID = huntPlans[iPlan];
            int kbResourceID = aiPlanGetVariableInt(planID, cGatherPlanKBResourceID, 0);
            int numWantedUnits = aiPlanGetNumberWantedUnits(planID, cUnitTypeAbstractVillager);
            // No resource most likely means that we can't find any more, if we're precisely in a transition then the
            // default assignment will re-create us, no problem.
            if (kbResourceGetIsIDValid(kbResourceID) == false)
            {
               deltaGatherers += numWantedUnits; // Potentially give these to other existing plans.
               debugResourceBreakdown("           We don't have a valid KB Resource ID: " + aiPlanGetName(planID) + ", giving " +
                  " these units back to our deltaGatherers.");
               continue;
            }
            int areaID = kbAreaGetIDByPosition(kbResourceGetPosition(kbResourceID));
            if (areaID != -1)
            {
               if (kbAreaGetDangerLevel(areaID, false) > 100.0) // TODO unhardcode this value and unify with source + top of this file.
               {
                  deltaGatherers += numWantedUnits; // Potentially give these to other existing plans.
                  debugResourceBreakdown("           Danger rating for: " + aiPlanGetName(planID) + " is too high, giving " +
                     " these units back to our deltaGatherers.");
                  continue;
               }
            }

            if (unassigned < numWantedUnits)
            {
               // Cap at our unassigned, since we always keep a plan around for passive hunt.
               if (unassigned > 0)
               {
                  numNeeded.add(max(1, unassigned - 2));
                  numWanted.add(unassigned);
                  numMax.add(unassigned + maxOverflowHunters);
                  debugResourceBreakdown("           We have to reduce the amount of gatherers assigned to: " +
                     aiPlanGetName(planID) + " from " + numWantedUnits + " to " + unassigned + ".");
               }
               else
               {
                  debugResourceBreakdown("           We don't have enough remaining food gatherers for: " + aiPlanGetName(planID)
                     + ".");
                  continue;
               }
            }
            else
            {
               if (deltaGatherers > 0)
               {
                  if (numWantedUnits < numTargetHunters + maxLeftoverHunters)
                  {
                     int numWantedHunters = min(numTargetHunters + maxLeftoverHunters, numWantedUnits + deltaGatherers);
                     deltaGatherers -= numWantedHunters - numWantedUnits;
                     numNeeded.add(max(1, numWantedHunters - 2));
                     numWanted.add(numWantedHunters);
                     numMax.add(numWantedHunters + maxOverflowHunters);
                     debugResourceBreakdown("           We have extra gatherers and " + aiPlanGetName(planID) +
                        " wasn't as max capacity yet so we filled it up with " + (numWantedHunters - numWantedUnits) +
                        " extra units. New wanted total: " + numWantedHunters + ".");
                  }
                  else
                  {
                     numNeeded.add(aiPlanGetNumberNeededUnits(planID, cUnitTypeAbstractVillager));
                     numWanted.add(numWantedUnits);
                     numMax.add(aiPlanGetNumberMaxUnits(planID, cUnitTypeAbstractVillager));
                     debugResourceBreakdown("           We have extra gatherers but " + aiPlanGetName(planID) +
                        " was already at max capacity of " + (numTargetHunters + maxLeftoverHunters) +
                        ", no changes.");
                  }
               }
               else
               {
                  numNeeded.add(aiPlanGetNumberNeededUnits(planID, cUnitTypeAbstractVillager));
                  numWanted.add(numWantedUnits);
                  numMax.add(aiPlanGetNumberMaxUnits(planID, cUnitTypeAbstractVillager));
                  debugResourceBreakdown("           We have don't have extra gatherers but " + aiPlanGetName(planID) +
                        " does't need to be reduced from " + numWantedUnits + " size.");
               }
            }

            numHuntPlans++;
            int index = numWanted.size() - 1;
            unassigned -= numWanted[index];
            numBaseHunters += numWanted[index];
            totalAssignedFoodGatherers += numWanted[index];
         }
   	   debugResourceBreakdown("           Num passive hunters: " + numBaseHunters + ", keeping " + numHuntPlans + "/" +
            numExistingHuntPlans + " existing plans.");
         if (numHuntPlans > 0)
         {
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeHunt, numHuntPlans, mPrioFoodHunt,
                                   baseID, numNeeded, numWanted, numMax);
         }
         else
         {
            aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeHunt, baseID);
         }
      }

      //////////////
      // Berries / Chickens.
      //////////////
      // IF YOU CHANGE THESE NUMBERS ALSO CHANGE THEM IN resourceBreakdownUpdateFood.
      int numTargetEasy = 4;
      int maxLeftoverEasy = 3;
      int maxOverflowEasy = 3;
      if (cMyCulture == cCultureAtlantean)
      {
         numTargetEasy = 3;
      }

      for (int iBase = 0; iBase < numBases; iBase++)
      {
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         debugResourceBreakdown("     *** Berries/Chickens processing existing gather base: " + kbBaseGetNameByID(cMyID, baseID) +
            " of size: " + kbBaseGetDistance(cMyID, baseID) + ".");
            
         int breakDownID = aiGetResourceBreakdownID(cResourceFood, cAIResourceSubTypeEasy, baseID);
         if (breakDownID == -1)
         {
            debugResourceBreakdown("           Base had no existing Berries/Chickens gather plans.");
            continue;
         }
         int[] easyPlans = aiPlanGetIDsByTypeAndVariableIntValue(cPlanGather, cGatherPlanBreakDownID, breakDownID);
         int numExistingEasyPlans = easyPlans.size();
         
         int numBaseEasy= 0;
         int numEasyPlans = 0;
         int[] numNeeded = new int(0, 0);
         int[] numWanted = new int(0, 0);
         int[] numMax = new int(0, 0);
         // Assign per plan.
         for (int iPlan = 0; iPlan < numExistingEasyPlans; iPlan++)
         {
            int planID = easyPlans[iPlan];
            int kbResourceID = aiPlanGetVariableInt(planID, cGatherPlanKBResourceID, 0);
            int numWantedUnits = aiPlanGetNumberWantedUnits(planID, cUnitTypeAbstractVillager);
            // No resource most likely means that we can't find any more, if we're precisely in a transition then the
            // default assignment will re-create us, no problem.
            if (kbResourceGetIsIDValid(kbResourceID) == false)
            {
               deltaGatherers += numWantedUnits; // Potentially give these to other existing plans.
               debugResourceBreakdown("           We don't have a valid KB Resource ID: " + aiPlanGetName(planID) + ", giving " +
                  " these units back to our deltaGatherers.");
               continue;
            }
            int areaID = kbAreaGetIDByPosition(kbResourceGetPosition(kbResourceID));
            if (areaID != -1)
            {
               if (kbAreaGetDangerLevel(areaID, false) > 100.0) // TODO unhardcode this value and unify with source + top of this file.
               {
                  deltaGatherers += numWantedUnits; // Potentially give these to other existing plans.
                  debugResourceBreakdown("           Danger rating for: " + aiPlanGetName(planID) + " is too high, giving " +
                     " these units back to our deltaGatherers.");
                  continue;
               }
            }

            if (unassigned < numWantedUnits)
            {
               // Cap at our unassigned, since we always keep a plan around for Berries/Chickens.
               if (unassigned > 0)
               {
                  numNeeded.add(max(1, unassigned - 2));
                  numWanted.add(unassigned);
                  numMax.add(unassigned + maxOverflowEasy);
                  debugResourceBreakdown("           We have to reduce the amount of gatherers assigned to: " +
                     aiPlanGetName(planID) + " from " + numWantedUnits + " to " + unassigned + ".");
               }
               else
               {
                  debugResourceBreakdown("           We don't have enough remaining food gatherers for: " + aiPlanGetName(planID)
                     + ".");
                  continue;
               }
            }
            else
            {
               if (deltaGatherers > 0)
               {
                  if (numWantedUnits < numTargetEasy + maxLeftoverEasy)
                  {
                     int numWantedGatherers = min(numTargetEasy + maxLeftoverEasy, numWantedUnits + deltaGatherers);
                     deltaGatherers -= numWantedGatherers - numWantedUnits;
                     numNeeded.add(max(1, numWantedGatherers - 2));
                     numWanted.add(numWantedGatherers);
                     numMax.add(numWantedGatherers + maxOverflowEasy);
                     debugResourceBreakdown("           We have extra gatherers and " + aiPlanGetName(planID) +
                        " wasn't as max capacity yet so we filled it up with " + (numWantedGatherers - numWantedUnits) + " extra units. " + 
                        "New wanted total: " + numWantedGatherers + ".");
                  }
                  else
                  {
                     numNeeded.add(aiPlanGetNumberNeededUnits(planID, cUnitTypeAbstractVillager));
                     numWanted.add(numWantedUnits);
                     numMax.add(aiPlanGetNumberMaxUnits(planID, cUnitTypeAbstractVillager));
                     debugResourceBreakdown("           We have extra gatherers but " + aiPlanGetName(planID) +
                        " was already at max capacity of " + (numTargetEasy + maxLeftoverEasy) + ", no changes.");
                  }
               }
               else
               {
                  numNeeded.add(aiPlanGetNumberNeededUnits(planID, cUnitTypeAbstractVillager));
                  numWanted.add(numWantedUnits);
                  numMax.add(aiPlanGetNumberMaxUnits(planID, cUnitTypeAbstractVillager));
                  debugResourceBreakdown("           We have don't have extra gatherers but " + aiPlanGetName(planID) +
                        " does't need to be reduced from " + numWantedUnits + " size.");
               }
            }

            numEasyPlans++;
            int index = numWanted.size() - 1;
            unassigned -= numWanted[index];
            numBaseEasy += numWanted[index];
            totalAssignedFoodGatherers += numWanted[index];
         }
   	   debugResourceBreakdown("           Num Berries/Chickens: " + numBaseEasy + ", keeping " + numEasyPlans + "/" +
            numExistingEasyPlans + " existing plans.");
         if (numEasyPlans > 0)
         {
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeEasy, numEasyPlans, mPrioFoodEasy,
                                   baseID, numNeeded, numWanted, numMax);
         }
         else
         {
            aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeEasy, baseID);
         }
      }

      //////////////
      // Herdables.
      //////////////
      // IF YOU CHANGE THESE NUMBERS ALSO CHANGE THEM IN resourceBreakdownUpdateFood.
      int numTargetHerders = 8;
      int maxOverflowHerders = 2;

      for (int iBase = 0; iBase < numBases; iBase++)
      {
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         debugResourceBreakdown("     *** Herdables processing existing gather base: " + kbBaseGetNameByID(cMyID, baseID) +
            " of size: " + kbBaseGetDistance(cMyID, baseID) + ".");
            
         int breakDownID = aiGetResourceBreakdownID(cResourceFood, cAIResourceSubTypeHerdable, baseID);
         if (breakDownID == -1)
         {
            debugResourceBreakdown("           Base had no existing Herdables gather plans.");
            continue;
         }
         int[] herdablePlans = aiPlanGetIDsByTypeAndVariableIntValue(cPlanGather, cGatherPlanBreakDownID, breakDownID);
         int numExistingHerdablePlans = herdablePlans.size();
         
         int numBaseHerders = 0;
         int numHerdablePlans = 0;
         int[] numNeeded = new int(0, 0);
         int[] numWanted = new int(0, 0);
         int[] numMax = new int(0, 0);
         // Assign per plan.
         for (int iPlan = 0; iPlan < numExistingHerdablePlans; iPlan++)
         {
            int planID = herdablePlans[iPlan];
            int kbResourceID = aiPlanGetVariableInt(planID, cGatherPlanKBResourceID, 0);
            int numWantedUnits = aiPlanGetNumberWantedUnits(planID, cUnitTypeAbstractVillager);
            // No resource most likely means that we can't find any more, if we're precisely in a transition then the
            // default assignment will re-create us, no problem.
            if (kbResourceGetIsIDValid(kbResourceID) == false)
            {
               deltaGatherers += numWantedUnits; // Potentially give these to other existing plans.
               debugResourceBreakdown("           We don't have a valid KB Resource ID: " + aiPlanGetName(planID) + ", giving " +
                  " these units back to our deltaGatherers.");
               continue;
            }
            int areaID = kbAreaGetIDByPosition(kbResourceGetPosition(kbResourceID));
            if (areaID != -1)
            {
               if (kbAreaGetDangerLevel(areaID, false) > 100.0) // TODO unhardcode this value and unify with source + top of this file.
               {
                  deltaGatherers += numWantedUnits; // Potentially give these to other existing plans.
                  debugResourceBreakdown("           Danger rating for: " + aiPlanGetName(planID) + " is too high, giving " +
                     " these units back to our deltaGatherers.");
                  continue;
               }
            }

            if (unassigned < numWantedUnits)
            {
               // Cap at our unassigned, since we always keep a plan around for Herdables.
               if (unassigned > 0)
               {
                  numNeeded.add(max(1, unassigned - 2));
                  numWanted.add(unassigned);
                  numMax.add(unassigned + maxOverflowHerders);
                  debugResourceBreakdown("           We have to reduce the amount of gatherers assigned to: " +
                     aiPlanGetName(planID) + " from " + numWantedUnits + " to " + unassigned + ".");
               }
               else
               {
                  debugResourceBreakdown("           We don't have enough remaining food gatherers for: " + aiPlanGetName(planID)
                     + ".");
                  continue;
               }
            }
            else
            {
               if (deltaGatherers > 0)
               {
                  if (numWantedUnits < numTargetHerders)
                  {
                     int numWantedGatherers = min(numTargetHerders, numWantedUnits + deltaGatherers);
                     deltaGatherers -= numWantedGatherers - numWantedUnits;
                     numNeeded.add(max(1, numWantedGatherers - 2));
                     numWanted.add(numWantedGatherers);
                     numMax.add(numWantedGatherers + maxOverflowHerders);
                     debugResourceBreakdown("           We have extra gatherers and " + aiPlanGetName(planID) +
                        " wasn't as max capacity yet so we filled it up with " + (numWantedGatherers - numWantedGatherers) +
                        " extra units. New wanted total: " + numWantedGatherers + ".");
                  }
                  else
                  {
                     numNeeded.add(aiPlanGetNumberNeededUnits(planID, cUnitTypeAbstractVillager));
                     numWanted.add(numWantedUnits);
                     numMax.add(aiPlanGetNumberMaxUnits(planID, cUnitTypeAbstractVillager));
                     debugResourceBreakdown("           We have extra gatherers but " + aiPlanGetName(planID) +
                        " was already at max capacity of " + numTargetHerders + ", no changes.");
                  }
               }
               else
               {
                  numNeeded.add(aiPlanGetNumberNeededUnits(planID, cUnitTypeAbstractVillager));
                  numWanted.add(numWantedUnits);
                  numMax.add(aiPlanGetNumberMaxUnits(planID, cUnitTypeAbstractVillager));
                  debugResourceBreakdown("           We have don't have extra gatherers but " + aiPlanGetName(planID) +
                        " does't need to be reduced from " + numWantedUnits + " size.");
               }
            }

            numHerdablePlans++;
            int index = numWanted.size() - 1;
            unassigned -= numWanted[index];
            numBaseHerders += numWanted[index];
            totalAssignedFoodGatherers += numWanted[index];
         }
   	   debugResourceBreakdown("           Num Herdables: " + numBaseHerders + ", keeping " + numHerdablePlans + "/" +
            numExistingHerdablePlans + " existing plans.");
         if (numHerdablePlans > 0)
         {
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeHerdable, numHerdablePlans, mPrioFoodHerdable,
                                   baseID, numNeeded, numWanted, numMax);
         }
         else
         {
            aiRemoveResourceBreakdown(cResourceFood, cAIResourceSubTypeHerdable, baseID);
         }
      }

      // If after assigning everything into their Wanted we still have unassigned left and we had units in the MAX part,
      // put them back in there.
      int unassignedComparedToLastRun = foodGathererCount - totalAssignedFoodGatherers;
      int puttingInMax = 0;
      if (unassignedComparedToLastRun > 0 && mLastAssignedMaxFoodGatherers > 0)
      {
         puttingInMax = min(unassignedComparedToLastRun, mLastAssignedMaxFoodGatherers);
      }
      totalAssignedFoodGatherers += puttingInMax;
      debugResourceBreakdown("           We're keeping " + totalAssignedFoodGatherers + "/" + foodGathererCount +
         " assigned to existing food plans, " + puttingInMax + " of those will be in the MAX part. Didn't assign " +
         mLastAssignedNumberFarmers + " gatherers who were farmers last time.");
      totalAssigned = totalAssignedFoodGatherers;
      // This is the amount that resourceBreakdownUpdateFood still needs to assign,
      // which can be fully comprised of farmers or we need new spots.
      return totalFoodGathererCount - totalAssignedFoodGatherers;
   }

   //==============================================================================
   // resourceBreakdownUpdateFood
   // In this function we try to assign all food gatherers that need a new spot. In resourceBreakdownUpdateExistingFood we already
   // tried to perserve existing food plans. Since Farms are the #1 priority they are not assigned already in 
   // resourceBreakdownUpdateExistingFood but this function still needs to do that.
   //==============================================================================
   int resourceBreakdownUpdateFood(bool allowFarming = false, int maxFarmCount = -1, int numNeedsNewPlan = -1,
      int alreadyAssigned = 0, bool canMicro = false)
   {
      debugResourceBreakdown("--- Trying to assign food gatherers ---");
      int foodGathererCount = numNeedsNewPlan;
      if (foodGathererCount <= 0)
   	{
         // If we have no gatherers left to assign to new spots we must only make sure Farms have no breakdowns left.
         // Any other breakdown will already be handled in resourceBreakdownUpdateExistingFood.
         nullPercentageForBase(0, cResourceFood, cAIResourceSubTypeFarm);
         debugResourceBreakdown("     No food gatherers to assign, quiting.");
         mLastAssignedMaxFoodGatherers = 0;
         mLastAssignedNumberFarmers = 0;
         mLastAssignedFoodGathererNoFarmersCount = alreadyAssigned;
         return 0;
      }
      if (gOverrideOkToGatherFood == false)
      {
         // resourceBreakdownUpdateExistingFood already removed all the breakdowns at this point.
         debugResourceBreakdown("     Not allowed to gather food via breakdowns because of gOverrideOkToGatherFood == false.");
         mLastAssignedMaxFoodGatherers = 0;
         mLastAssignedNumberFarmers = 0;
         mLastAssignedFoodGathererNoFarmersCount = alreadyAssigned;
         return foodGathererCount;
      }
      
      // Preference order is:
      // - Existing plans -> in resourceBreakdownUpdateExistingFood.
      // - Existing farms (but not in age 1)
      // - Aggressive hunt
      // - Passive Hunt
      // - Berries / Chickens.
      // - Herdables
      // - New farms
      int totalReactiveHunters = 0;
      int totalPassiveHunters = 0;
      int totalEasy = 0;
      int totalHerders = 0;
      int totalFarmers = 0;

      debugResourceBreakdown("     Trying to assign: " + foodGathererCount + " gatherers.");
      debugResourceBreakdown("     Are we allowed to micro: " + xsBoolToString(canMicro) + ".");
      debugResourceBreakdown("     Are we allowed to farm: " + xsBoolToString(allowFarming) + ".");
      int unassigned = foodGathererCount;
      int maxCapacity = 0;

      int currentAge = kbPlayerGetAge(cMyID);
      // Egyptians/Aztecs unlock Farms in Archaic already.
      bool canBuildFarm = kbProtoUnitAvailable(gFarmUnit);
      int numFarms = kbUnitCount(cUnitTypeAbstractFarm, cMyID, cUnitStateABQ); // TODO we should only count Farms in the bases we actually use.
      int numPlannedFarms = aiPlanGetNumberByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, gFarmUnit);
      int numTotalFarms = numFarms + numPlannedFarms;
   	debugResourceBreakdown("     We have: " + numFarms + " Farms and are planning for: " + numPlannedFarms + " more, totaling: "
         + numTotalFarms + ".");

      // We're going to process every base and its resources until we run out of gatherers.

      // Farmers.
      // This part of the Farm logic is purely to indentify what bases already have Farms, later on we build new ones.
      int[] farmsInBase = new int(0, 0);
      int[] baseFarmers = new int(0, 0);
      for (int iBase = 0; iBase < mGatherBases.size(); iBase++)
      {
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            // Need to add empty entry to have the index lookup on iBase remain correct later.
            farmsInBase.add(0);
            baseFarmers.add(0);
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         debugResourceBreakdown("     *** Analyzing Farms processing gather base: " + kbBaseGetNameByID(cMyID, baseID) +
            " of size: " + kbBaseGetDistance(cMyID, baseID) + ".");
         farmsInBase.add(0);
         baseFarmers.add(0);
         int numSafeFarmResources = internalBaseNumberResources(mSafeFarmResourcesOffset, iBase);
         int numSafeBaseFarms = internalBaseNumberResourceUnits(mSafeFarmResources, mSafeFarmResourcesOffset[iBase], numSafeFarmResources);
   	   debugResourceBreakdown("           Has: " + numSafeFarmResources + " Farm resources, totaling: " + numSafeBaseFarms +
            " safe Farms.");
         // We only already reserve these Farmers if we're past Archaic, otherwise these Farms can still end up being used
         // but then all other food sources must've not provided enough spots for us.
         if (currentAge > cAge1) 
         {        
            // Do not overshoot our villagers.
            int prioBaseFarmers = 0;
            if (cMyCiv == cCivDemeter)
            {
               prioBaseFarmers = min(numSafeBaseFarms * 2, unassigned);
            }
            else
            {
               prioBaseFarmers = min(numSafeBaseFarms, unassigned);
            }
            debugResourceBreakdown("           Reserved: " + prioBaseFarmers + " farmers.");
            baseFarmers[iBase] = prioBaseFarmers;
            totalFarmers += prioBaseFarmers;
   	      unassigned -= prioBaseFarmers;
            int availableFarms = 0;
            if (cMyCiv == cCivDemeter)
            {
               availableFarms = numSafeBaseFarms - (prioBaseFarmers / 2);
            }
            else
            {
               availableFarms = numSafeBaseFarms - prioBaseFarmers;
            }
            if (availableFarms > 0)
            {
               debugResourceBreakdown("           " + availableFarms + " Unused Farms remain.");
            }
         }
         farmsInBase[iBase] = numSafeBaseFarms;
      }

      //////////////
      // Reactive hunt.
      //////////////
      // IF YOU CHANGE THESE NUMBERS ALSO CHANGE THEM IN resourceBreakdownUpdateExistingFood.
      int numRequiredReactiveHunters = 0;
      int maxLeftoverReactiveHunters = 0;
      if (canMicro == true)
      {
         numRequiredReactiveHunters = cMyCulture == cCultureAtlantean ? 3 : 5;
         maxLeftoverReactiveHunters = cMyCulture == cCultureAtlantean ? 4 : 6;
      }
      else
      {
         numRequiredReactiveHunters = cMyCulture == cCultureAtlantean ? 4 : 8;
         maxLeftoverReactiveHunters = cMyCulture == cCultureAtlantean ? 3 : 3;
      }
      // We already have big numbers above, we can't put more or we pathblock ourself.
      int maxOverflowReactiveHunters = 0;
      for (int iBase = 0; iBase < mGatherBases.size(); iBase++)
      {
         if (unassigned <= 0)
         {
            break;
         }
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         // We're now analyzing remote gather bases, we can gather reactive hunt in there, if the base is of the correct resource type.
         if (iBase >= mSortedTCBases.size() && mOverride == false)
         {
            if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagRemoteFoodGatherBase) == false)
            {
               debugResourceBreakdown("     *** Analyzing reactive hunt, SKIPPING gather base: " + kbBaseGetNameByID(cMyID, baseID) +
                             " of size: " + kbBaseGetDistance(cMyID, baseID) + " because it's not the right remote type.");
               continue;
            }
         }
         debugResourceBreakdown("     *** Reactive Hunts processing gather base: " + kbBaseGetNameByID(cMyID, baseID) +
            " of size: " + kbBaseGetDistance(cMyID, baseID) + ".");
         int numAggressiveSpots = internalBaseNumberResources(mAggressiveResourcesOffset, iBase);
         int numTakenAggressiveSpots = aiGetResourceBreakdownNumberPlans(cResourceFood, cAIResourceSubTypeHuntAggressive, baseID);
         if (numTakenAggressiveSpots > 0)
         {
            debugResourceBreakdown("           We found " + numAggressiveSpots + " reactive hunt spots of which " +
               numTakenAggressiveSpots + " are already taken by plans that persist.");
            numAggressiveSpots -= numTakenAggressiveSpots;
         }
         if (numAggressiveSpots <= 0)
         {
            continue;
         }
         int numHuntPlans = min(numAggressiveSpots, unassigned / numRequiredReactiveHunters);
         int[] numNeeded = new int(numHuntPlans, 0);
         int[] numWanted = new int(numHuntPlans, 0);
         int[] numMax = new int(numHuntPlans, 0);
         // Assign per plan.
         for (int iPlan = 0; iPlan < numHuntPlans; iPlan++)
         {
            // We always know we have the available after the min() above.
            numNeeded[iPlan] = numRequiredReactiveHunters;
            unassigned -= numRequiredReactiveHunters;
         }
         // Try adding some leftovers
         int numBaseHunters = 0;
         for (int iPlan = 0; iPlan < numHuntPlans; iPlan++)
         {
            int leftovers = min(unassigned, maxLeftoverReactiveHunters);
            int wantedHunters = numNeeded[iPlan] + leftovers;
   	      debugResourceBreakdown("           Plan wants num gatherers: " + wantedHunters + ".");
            numNeeded[iPlan] = max(numRequiredReactiveHunters, wantedHunters - 2);
            numWanted[iPlan] = wantedHunters;
            numMax[iPlan] = wantedHunters + maxOverflowReactiveHunters;
            maxCapacity += maxOverflowReactiveHunters;
            totalReactiveHunters += wantedHunters;
            unassigned -= leftovers;
            numBaseHunters += wantedHunters;
         }
   	   debugResourceBreakdown("           Num plans: " + numHuntPlans + ", num gatherers: " + numBaseHunters +
            ", num reactive hunt spots: " + numAggressiveSpots + ".");
         if (numTakenAggressiveSpots > 0)
         {
            aiAddToResourceBreakdown(cResourceFood, cAIResourceSubTypeHuntAggressive, numHuntPlans, baseID, numNeeded,
                                     numWanted, numMax);
         }
         else
         {
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeHuntAggressive, numHuntPlans, mPrioFoodAggressive,
                                   baseID, numNeeded, numWanted, numMax);
         }
      }

      // Still have unassigned? Try creating a remote base.
      while (unassigned >= numRequiredReactiveHunters && mAllowAutoRemoteCreation == true)
      {
         // NOTE: If we somehow find the same remote base twice in this loop, first creation and then finding it again we have an issue.
         // Because then our second aiSetResourceBreakdown will overwrite the first one.
         // The chances of this are super low because these bases are small.
         // We will not handle this situation because it solves itself the next run where we will scan the remote base properly.
         int remoteBaseID = internalCreateRemoteBase(cResourceFood, cAIResourceSubTypeHuntAggressive, remoteFoodRange);
         if (remoteBaseID != -1)
         {
            // Increase these arrays so we don't out of bounds.
            farmsInBase.add(0);
            baseFarmers.add(0);
            // Instantly include leftovers.
            int numRemote = min(numRequiredReactiveHunters + maxLeftoverReactiveHunters, unassigned);
            unassigned -= numRemote;
            totalReactiveHunters += numRemote;
            debugResourceBreakdown("           *** Created remote food reactive hunt base: " +
               kbBaseGetNameByID(cMyID, remoteBaseID) + ", this remote base gets " + numRemote + " gatherers. ***");
            int[] numNeeded = new int(1, max(numRequiredReactiveHunters, numRemote - 2));
            int[] numWanted = new int(1, numRemote);
            int[] numMax = new int(1,  numRemote + maxOverflowReactiveHunters);
            maxCapacity += maxOverflowReactiveHunters;
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeHuntAggressive, 1, mPrioFoodAggressive, 
                                   remoteBaseID, numNeeded, numWanted, numMax);
         }
         else
         {
            debugResourceBreakdown("           *** Failed to create a remote reactive hunt base. ***");
            break;
         }
      }

      //////////////
      // Passive hunt.
      //////////////
      // IF YOU CHANGE THESE NUMBERS ALSO CHANGE THEM IN resourceBreakdownUpdateExistingFood.
      int numTargetHunters = 4;
      int maxLeftoverHunters = 6;
      int maxOverflowHunters = 2;
      if (cMyCulture == cCultureAtlantean)
      {
         numTargetHunters = 2;
         maxLeftoverHunters = 5;
      }
      int targetPassiveHuntGroupSize = numTargetHunters + maxLeftoverHunters;

      for (int iBase = 0; iBase < mGatherBases.size(); iBase++)
      {
         if (unassigned <= 0)
         {
            break;
         }
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         // We're now analyzing remote gather bases, we can gather passive hunt in there, if the base is of the correct resource type.
         if (iBase >= mSortedTCBases.size() && mOverride == false)
         {
            if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagRemoteFoodGatherBase) == false)
            {
               debugResourceBreakdown("     *** Analyzing passive hunt, SKIPPING gather base: " + kbBaseGetNameByID(cMyID, baseID) +
                                " of size: " + kbBaseGetDistance(cMyID, baseID) + " because it's not the right remote type.");
               continue;
            }
         }
         debugResourceBreakdown("     *** Passive Hunts processing gather base: " + kbBaseGetNameByID(cMyID, baseID) + " of size: " +
                                kbBaseGetDistance(cMyID, baseID) + ".");
         int numHuntSpots = internalBaseNumberResources(mHuntResourceOffset, iBase);
         int numTakenHuntSpots = aiGetResourceBreakdownNumberPlans(cResourceFood, cAIResourceSubTypeHunt, baseID);
         if (numTakenHuntSpots > 0)
         {
            debugResourceBreakdown("           We found " + numHuntSpots + " passive hunt spots of which " +
               numTakenHuntSpots + " are already taken by plans that persist.");
            numHuntSpots -= numTakenHuntSpots;
         }
         if (numHuntSpots <= 0)
         {
            continue;
         }
         // Always make 1 hunt plan if have the spots for it.
         int numHuntPlans = min(numHuntSpots, ceil(xsIntToFloat(unassigned) / xsIntToFloat(targetPassiveHuntGroupSize)));
         int[] numNeeded = new int(numHuntPlans, 0);
         int[] numWanted = new int(numHuntPlans, 0);
         int[] numMax = new int(numHuntPlans, 0);
         // Assign per plan and try adding some leftovers.
         for (int iPlan = 0; iPlan < numHuntPlans; iPlan++)
         {
            int neededHunters = min(unassigned, numTargetHunters);
            numNeeded[iPlan] = neededHunters;
            unassigned -= neededHunters;
         }
         // Try adding some leftovers.
         int numBaseHunters = 0;
         for (int iPlan = 0; iPlan < numHuntPlans; iPlan++)
         {
            int leftovers = min(unassigned, maxLeftoverHunters);
            int wantedHunters = numNeeded[iPlan] + leftovers;
   	      debugResourceBreakdown("           Plan wants num gatherers: " + wantedHunters + ".");
            numNeeded[iPlan] = max(1, wantedHunters - 2);
            numWanted[iPlan] = wantedHunters;
            numMax[iPlan] = wantedHunters + maxOverflowHunters;
            maxCapacity += maxOverflowHunters;
            totalPassiveHunters += wantedHunters;
            unassigned -= leftovers;
            numBaseHunters += wantedHunters;
         }
   	   debugResourceBreakdown("           Num plans: " + numHuntPlans + ", num gatherers: " + numBaseHunters + ", num passive hunt spots: " +
            numHuntSpots + ".");
         if (numTakenHuntSpots > 0)
         {
            aiAddToResourceBreakdown(cResourceFood, cAIResourceSubTypeHunt, numHuntPlans, baseID, numNeeded, numWanted, numMax);
         }
         else
         {
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeHunt, numHuntPlans, mPrioFoodHunt,
                                   baseID, numNeeded, numWanted, numMax);
         }   
      }

      // Still have unassigned? Try creating a remote base.
      while (unassigned >= numTargetHunters && mAllowAutoRemoteCreation == true)
      {
         int remoteBaseID = internalCreateRemoteBase(cResourceFood, cAIResourceSubTypeHunt, remoteFoodRange);
         if (remoteBaseID != -1)
         {
            // Increase these arrays so we don't out of bounds.
            farmsInBase.add(0);
            baseFarmers.add(0);
            // Instantly include leftovers.
            int numRemote = min(targetPassiveHuntGroupSize, unassigned);
            unassigned -= numRemote;
            totalPassiveHunters += numRemote;
            debugResourceBreakdown("           *** Created remote food hunt base: " +
               kbBaseGetNameByID(cMyID, remoteBaseID) + ", this remote base gets " + numRemote + " gatherers. ***");
            int[] numNeeded = new int(1, max(1, numRemote - 2));
            int[] numWanted = new int(1, numRemote);
            int[] numMax = new int(1,  numRemote + maxOverflowHunters);
            maxCapacity += maxOverflowHunters;
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeHunt, 1, mPrioFoodHunt, 
                                   remoteBaseID, numNeeded, numWanted, numMax);
         }
         else
         {
            debugResourceBreakdown("           *** Failed to create a remote hunt base. ***");
            break;
         }
      }

      //////////////
      // Berries / Chickens.
      //////////////
      // IF YOU CHANGE THESE NUMBERS ALSO CHANGE THEM IN resourceBreakdownUpdateExistingFood.
      int numTargetEasy = 4;
      int maxLeftoverEasy = 3;
      int maxOverflowEasy = 3;
      if (cMyCulture == cCultureAtlantean)
      {
         numTargetEasy = 3;
      }
      int targetEasyGroupSize = numTargetEasy + maxLeftoverEasy;
      for (int iBase = 0; iBase < mGatherBases.size(); iBase++)
      {
         if (unassigned <= 0)
         {
            break;
         }
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         // We're now analyzing remote gather bases, we can't gather Berries / Chickens in those.
         // We can't because we don't want to risk our gatherers in remote bases for arguably "bad" resource subtypes.
         if (iBase >= mSortedTCBases.size() && mOverride == false)
         {
            continue;
         }
         debugResourceBreakdown("     *** Berries/Chickens processing gather base: " + kbBaseGetNameByID(cMyID, baseID) + " of size: " +
                                kbBaseGetDistance(cMyID, baseID) + ".");
         int numEasySpots = internalBaseNumberResources(mEasyResourcesOffset, iBase);
         int numTakenEasySpots = aiGetResourceBreakdownNumberPlans(cResourceFood, cAIResourceSubTypeEasy, baseID);
         if (numTakenEasySpots > 0)
         {
            debugResourceBreakdown("           We found " + numEasySpots + " berry/chicken spots of which " +
               numTakenEasySpots + " are already taken by plans that persist.");
            numEasySpots -= numTakenEasySpots;
         }
         if (numEasySpots <= 0)
         {
            continue;
         }
         // We always create 1 plan.
         int numEasyPlans = min(numEasySpots, ceil(xsIntToFloat(unassigned) / xsIntToFloat(targetEasyGroupSize)));
         int[] numNeeded = new int(numEasyPlans, 0);
         int[] numWanted = new int(numEasyPlans, 0);
         int[] numMax = new int(numEasyPlans, 0);
         // Assign per plan and try adding some leftovers.
         for (int iPlan = 0; iPlan < numEasyPlans; iPlan++)
         {
            int neededGatherers = min(unassigned, numTargetEasy);
            numNeeded[iPlan] = neededGatherers;
            unassigned -= neededGatherers;
         }
         // Try adding some leftovers.
         int numBaseGatherers = 0;
         for (int iPlan = 0; iPlan < numEasyPlans; iPlan++)
         {
            int leftovers = min(unassigned, maxLeftoverEasy);
            int wantedGatherers = numNeeded[iPlan] + leftovers;
   	      debugResourceBreakdown("           Plan wants num gatherers: " + wantedGatherers + ".");
            numNeeded[iPlan] = max(1, wantedGatherers - 2);
            numWanted[iPlan] = wantedGatherers;
            numMax[iPlan] = wantedGatherers + maxOverflowEasy;
            maxCapacity += maxOverflowEasy;
            totalEasy += wantedGatherers;
            unassigned -= leftovers;
            numBaseGatherers += wantedGatherers;
         }
   	   debugResourceBreakdown("           Num plans: " + numEasyPlans + ", num gatherers: " + numBaseGatherers +
            ", num berry/chicken spots: " + numEasySpots + ".");
         if (numTakenEasySpots > 0)
         {
            aiAddToResourceBreakdown(cResourceFood, cAIResourceSubTypeEasy, numEasyPlans, baseID, numNeeded, numWanted, numMax);
         }
         else
         {
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeEasy, numEasyPlans, mPrioFoodEasy,
                                   baseID, numNeeded, numWanted, numMax);
         }
      }

      //////////////
      // Herdables.
      //////////////
      // IF YOU CHANGE THESE NUMBERS ALSO CHANGE THEM IN resourceBreakdownUpdateExistingFood.
      // No required number so we can dump leftovers easily.
      int targetAmountOfHerders = 8;
      int maxOverflowHerders = 2;
      if (cMyCulture == cCultureAtlantean)
      {
         targetAmountOfHerders = 6;
         maxOverflowHerders = 2;
      }
      for (int iBase = 0; iBase < mGatherBases.size(); iBase++)
      {
         if (cMyCiv == cCivDemeter)
         {
            // Don't take herdables when we're Demeter, they can gather favor.
            break;
         }
         // We don't keep herdables in non TC bases. So if we hit the index in mGatherBases where the TC bases end we quit, unless override.
         if (unassigned <= 0 || (iBase >= mSortedTCBases.size() && mOverride == false))
         {
            break;
         }
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         
         debugResourceBreakdown("     *** Herdables processing gather base: " + kbBaseGetNameByID(cMyID, baseID) + " of size: " +
                                kbBaseGetDistance(cMyID, baseID) + ".");
         int numHerdableSpots = internalBaseNumberResources(mHerdableResourcesOffset, iBase);
         int numTakenHerdableSpots = aiGetResourceBreakdownNumberPlans(cResourceFood, cAIResourceSubTypeHerdable, baseID);
         if (numTakenHerdableSpots > 0)
         {
            debugResourceBreakdown("           We found " + numHerdableSpots + " herdable spots of which " +
               numTakenHerdableSpots + " are already taken by plans that persist. Since we only allow 1 herdable plan per base we skip.");
            continue;
         }
         if (numHerdableSpots <= 0)
         {
            continue;
         }
         // Only 1 herdable plan at most per base
         int numHerdablePlans = 1;
         int numBaseHerders = min(targetAmountOfHerders, unassigned);
         int[] numNeeded = new int(1, max(1, numBaseHerders - 2));
         int[] numWanted = new int(1, numBaseHerders);
         int[] numMax = new int(1, numBaseHerders + maxOverflowHerders);
         maxCapacity += maxOverflowHerders;
         totalHerders += numBaseHerders;
         unassigned -= numBaseHerders;
   	   debugResourceBreakdown("           Num plans: " + numHerdablePlans + ", num gatherers: " + numBaseHerders + ", num herdable spots: " +
            numHerdableSpots + ".");
         if (numTakenHerdableSpots > 0)
         {
            aiAddToResourceBreakdown(cResourceFood, cAIResourceSubTypeHerdable, numHerdablePlans, baseID, numNeeded, numWanted, numMax);
         }
         else
         {
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeHerdable, numHerdablePlans, mPrioFoodHerdable,
                                   baseID, numNeeded, numWanted, numMax);
         }
      }

      bool allowedToBuildMoreFarms = true;
      // Only output all of this stuff when we can actually farm.
      if (allowFarming == true)
      {
         if (maxFarmCount >= 0)
         {
            debugResourceBreakdown("We have a hard limit of " + maxFarmCount + " max TOTAL Farms we can own.");
            if (numTotalFarms >= maxFarmCount)
            {
               debugResourceBreakdown("We're already at/past our max farm limit, not building any more. We have " + numFarms +
                  " existing Farms and " + numPlannedFarms + " ongoing Farm build plans.");
               allowedToBuildMoreFarms = false;
            }
         }
         debugResourceBreakdown("We have a limit of " + mMaxFarmsPerBase + " Farms that we can have PER base.");
      }

      // Farms.
      for (int iBase = 0; iBase < mGatherBases.size(); iBase++)
      {
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         int currentBaseFarmers = baseFarmers[iBase]; // Reserved farmers from earlier.
         if (unassigned <= 0 && currentBaseFarmers == 0)
         {
            nullPercentageForBase(iBase, cResourceFood, cAIResourceSubTypeFarm);
            continue;
         }
         
         // If we're here and are in Archaic it means we must incorporate our pre-existing Farms.
         // We know for a fact that currentBaseFarmers == 0 because we can't assign gatherers to farms in Archaic at the start.
         if (currentAge == cAge1)
         {
            if (cMyCiv == cCivDemeter)
            {
               currentBaseFarmers = min(farmsInBase[iBase] * 2, unassigned);
            }
            else
            {
               currentBaseFarmers = min(farmsInBase[iBase], unassigned);
            }
            totalFarmers += currentBaseFarmers;
            unassigned -= currentBaseFarmers;
         }

         if (currentBaseFarmers >= 1)
         {
            debugResourceBreakdown("     *** " + kbBaseGetNameByID(cMyID, baseID) + " setting breakdown for its " +
               currentBaseFarmers + " farmers.");
            int[] kbResourceIDs = new int(0, 0);
            internalBaseKBResourceIDs(mSafeFarmResources , kbResourceIDs, mSafeFarmResourcesOffset[iBase],
               internalBaseNumberResources(mSafeFarmResourcesOffset, iBase));
            // We already set the breakdown now, below this we only plan for new Farms but those don't get a gatherer yet.
            // This is because once we start a build plan it could be a while before the foundation is there.
            // Cost is always a concern in a big transition + placement can take a while with overlaps being common.
            // So prevent gatherers just standing around idle and trying to work on already occupied Farms.
            int[] numNeeded = new int(0, 0);
            int[] numWanted = new int(0, 0);
            int[] numMax = new int(0, 0);
            int[] breakdownResources = new int(0, 0);
            int totalAssigned = 0;
            int totalFarmGatherPlans = 0;
            // Loop over all Farm resources we have, assign all our gatherers. It can be that we have more Farms available
            // than that we have gatherers, guard against that by early outing.
            for (int i = 0; i < kbResourceIDs.size(); i++)
            {
               if (totalAssigned >= currentBaseFarmers)
               {
                  break;
               }
               int numFarmsThisResource = kbResourceGetNumberUnits(kbResourceIDs[i]);
               int numToAssign = 0;
               if (cMyCiv == cCivDemeter)
               {
                  numToAssign = min(numFarmsThisResource * 2, currentBaseFarmers - totalAssigned);
               }
               else
               {
                  numToAssign = min(numFarmsThisResource, currentBaseFarmers - totalAssigned);
               }
               debugResourceBreakdown("           Farm KB Resource " + kbResourceIDs[i] + " gets " + numToAssign + " farmers.");
               totalAssigned += numToAssign;
               numNeeded.add(max(1, numToAssign - 2));
               numWanted.add(numToAssign);
               // Never have a bigger max than we have Farms, we can't do anything with the excess then.
               numMax.add(numFarmsThisResource);
               breakdownResources.add(kbResourceIDs[i]);
               totalFarmGatherPlans++;
            }
            if (totalAssigned != currentBaseFarmers)
            {
               aiEchoWarning("The amount of Farmers we predicted earlier doesn't equal the amount of Farms we found during assignment.");
            }
            aiSetResourceBreakdown(cResourceFood, cAIResourceSubTypeFarm, totalFarmGatherPlans, mPrioFoodFarm, 
                                   baseID, numNeeded, numWanted, numMax, breakdownResources);
         }

         // Next part is about construction new Farms, quit if we're not allowed to or can't.
         if (allowFarming == false || canBuildFarm == false || allowedToBuildMoreFarms == false ||
             kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue;
         }

         // numTotalFarms can increase below due to building more Farms, so check again.
         if (maxFarmCount >= 0 && numTotalFarms >= maxFarmCount)
         {
            continue;
         }

         // We don't build Farms in not TC bases. So if we hit the index in mGatherBases where the TC bases end we quit, unless override.
         if (unassigned <= 0 || (iBase >= mSortedTCBases.size() && mOverride == false))
         {
            continue;
         }

         // Do not farm in dangerous bases.
         if (kbAreaGetDangerLevel(kbAreaGetIDByPosition(kbBaseGetLocation(cMyID, baseID))) > 100.0)
         {
            debugResourceBreakdown("     *** " + kbBaseGetNameByID(cMyID, baseID) + " is not suited for farming. ***");
            continue;
         }
         
         debugResourceBreakdown("     *** Farms processing gather base: " + kbBaseGetNameByID(cMyID, baseID) + " of size: " +
            kbBaseGetDistance(cMyID, baseID) + ".");

         // Now we can have unassigned left with also build plans left.
         // We need to properly reduce our unassigned number by the amount of build plans we have.
         int numBasePlannedFarms = 0;
         for (int iPlan = 0; iPlan < numPlannedFarms; iPlan++)
         {
            int planID = aiPlanGetIDByTypeAndVariableIntValue(cPlanBuild, cBuildPlanBuildingTypeID, gFarmUnit, iPlan);
            if (baseID == aiPlanGetBaseID(planID))
            {
               numBasePlannedFarms++;
            }
         }
         debugResourceBreakdown("           Has: " + numBasePlannedFarms + " Farm build plans already in progress.");
         int futureFarmers = min(numBasePlannedFarms, unassigned);
         if (futureFarmers > 0)
         {
            // These gatherers will be taken up by the MAX room in the highest prio plans, cuz they get no breakdown of their own.
            unassigned -= futureFarmers;
            totalFarmers += futureFarmers;
            debugResourceBreakdown("           Reducing unassigned by " + futureFarmers + ", these gatherers will hopefully have "
               + "a Farm foundation next update to really assign to.");
         }

         // Max of 15 Farms per base.
         int amountFarmsToBuild = mMaxFarmsPerBase - farmsInBase[iBase] - numBasePlannedFarms;
         if (amountFarmsToBuild <= 0)
         {
            debugResourceBreakdown("           Base already has max amount of Farms.");
            continue;
         }
         // Cap at how many unassigned are left.
         if (cMyCiv == cCivDemeter)
         {
            amountFarmsToBuild = min(amountFarmsToBuild, unassigned / 2);
         }
         else
         {
            amountFarmsToBuild = min(amountFarmsToBuild, unassigned);
         }
         // Cap the max amount of Farms we place down per iteration per base to 3 default.
         // This prevents an excessive increase for Wood/Gold needs.
         // And it prevents the Farm build plans having to restart constantly because another plan took its best spot.
         amountFarmsToBuild = min(amountFarmsToBuild, mMaxFarmsPerIteration);
         // If we have a max farm limit we need to make sure we don't overshoot that.
         if (maxFarmCount >= 0)
         {
            amountFarmsToBuild = min(maxFarmCount - numTotalFarms, amountFarmsToBuild);
            numTotalFarms += amountFarmsToBuild;
            if (numTotalFarms >= maxFarmCount)
            {
               debugResourceBreakdown("           We're going to construct new Farms and with these additions we've hit our " +
                  "TOTAL max cap.");
            }
         }
         
         if (cMyCiv == cCivDemeter)
         {
            unassigned -= amountFarmsToBuild * 2;
            totalFarmers += amountFarmsToBuild * 2;
         }
         else
         {
            unassigned -= amountFarmsToBuild;
            totalFarmers += amountFarmsToBuild;
         }
         debugResourceBreakdown("           Base gets " + amountFarmsToBuild + " Farm build plans.");
         for (int i = 0; i < amountFarmsToBuild; i++)
         {
            createSimpleBuildPlan(gFarmUnit, 1, 70, gEconomicBuildingsCategoryID, baseID, 0, -1);
         }
      }

      // The number of gatherers that will fill up the MAX part of our plans.
      int numMAXGatherers = unassigned > maxCapacity ? maxCapacity : unassigned;
      debugResourceBreakdown("We have " + maxCapacity + " spots in the MAX part of our food plans. And " + numMAXGatherers +
         " gatherers will actually be distributed to those spots.");
      unassigned -= numMAXGatherers;
      mLastAssignedNumberFarmers = totalFarmers;
      mLastAssignedMaxFoodGatherers = numMAXGatherers;
      if (unassigned > 0)
      {
         debugResourceBreakdown("ATTENTION: Don't know what to do with " + unassigned + " gatherers.");
      }
      if (allowFarming == false && canBuildFarm == true)
      {
         if (unassigned > 0)
         {
            debugResourceBreakdown("We're not farming and didn't manage to assign all our food gatherers, think about starting that now.");
            alertRanOutOfFoodResources();
         }
         else
         {
            debugResourceBreakdown("We're not farming and still managed to assign all our food gatherers, don't start farming now.");
            alertFoundFoodResources();
         }
      }
      debugResourceBreakdown("Assignments of " + (foodGathererCount - unassigned) + "/" + foodGathererCount +
         " food gatherers are: Reactive Hunt: " + totalReactiveHunters + ", Passive Hunt: " + totalPassiveHunters +
         ", Berries/Chickens: " + totalEasy + ", Herders: " + totalHerders + ", Farmers: " + totalFarmers +
         ", MAX: " + numMAXGatherers + ".");
      // Save how many gatherers we assigned this run, excluding the farmers.
      mLastAssignedFoodGathererNoFarmersCount = alreadyAssigned + (foodGathererCount - unassigned) - totalFarmers;
      return unassigned;
   }
   

   //==============================================================================
   // resourceBreakdownUpdateGold
   //==============================================================================
   int resourceBreakdownUpdateGold(int gathererCount = -1, int carryoverUnassigned = -1)
   {
      debugResourceBreakdown("--- Trying to assign gold gatherers ---");
      int goldGathererCount = round(aiGetResourcePercentage(cResourceGold) * gathererCount);
      if (carryoverUnassigned > 0)
      {
         debugResourceBreakdown("     Adding: " + carryoverUnassigned + " gatherers that couldn't be assigned to food earlier.");
         goldGathererCount += carryoverUnassigned;
      }
      if (goldGathererCount == 0)
      {
         nullPercentageForBases(0, cResourceGold);
         debugResourceBreakdown("     No gold gatherers to assign, quiting.");
         return 0;
      }
      if (gOverrideOkToGatherGold == false)
      {
         nullPercentageForBases(0, cResourceGold);
         debugResourceBreakdown("     Not allowed to gather gold via breakdowns because of gOverrideOkToGatherGold == false.");
         return goldGathererCount;
      }

      int totalMiners = 0;
      int maxCapacity = 0;
      debugResourceBreakdown("     Trying to assign: " + goldGathererCount + " miners.");
      int unassigned = goldGathererCount;
      int numRequiredVillagers = 9;
      int maxLeftoverSize = 3;
      int maxOverflowSize = 4;
      if (cMyCulture == cCultureAtlantean)
      {
         numRequiredVillagers = 3;
         maxLeftoverSize = 2;
      }
      int targetGroupSize = numRequiredVillagers + maxLeftoverSize;
      // We're going to process every base and its resources until we run out of gatherers.
      for (int iBase = 0; iBase < mGatherBases.size(); iBase++)
      {
         if (unassigned <= 0)
         {
            nullPercentageForBases(iBase, cResourceGold, cAIResourceSubTypeEasy);
            break;
         }
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         // We're now analyzing remote gather bases, we can gather gold in there, if the base is of the correct resource type.
         if (iBase >= mSortedTCBases.size() && mOverride == false)
         {
            if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagRemoteGoldGatherBase) == false)
            {
               debugResourceBreakdown("     *** Analyzing gold, SKIPPING gather base: " + kbBaseGetNameByID(cMyID, baseID) +
                                " of size: " + kbBaseGetDistance(cMyID, baseID) + " because it's not the right remote type.");
               continue;
            }
         }
         debugResourceBreakdown("     *** Analyzing gold, processing gather base: " + kbBaseGetNameByID(cMyID, baseID) +
                                " of size: " + kbBaseGetDistance(cMyID, baseID) + ".");
         int numGoldMines = internalBaseNumberResources(mEasyGoldResourcesOffset, iBase);
         if (numGoldMines == 0)
         {
            aiRemoveResourceBreakdown(cResourceGold, cAIResourceSubTypeEasy, baseID);
            continue;
         }

         // We always want to make use of all our mines if we can afford it.
         int numGoldPlans = min(numGoldMines, ceil(xsIntToFloat(unassigned) / xsIntToFloat(targetGroupSize)));
         int[] numNeeded = new int(numGoldPlans, 0);
         int[] numWanted = new int(numGoldPlans, 0);
         int[] numMax = new int(numGoldPlans, 0);
         // Assign per plan and try adding some leftovers
         for (int iPlan = 0; iPlan < numGoldPlans; iPlan++)
         {
            int neededGatherers = min(unassigned, numRequiredVillagers);
            numNeeded[iPlan] = neededGatherers;
            unassigned -= neededGatherers;
         }
         // Try adding some leftovers
         int numBaseMiners = 0;
         for (int iPlan = 0; iPlan < numGoldPlans; iPlan++)
         {            
            int leftovers = min(unassigned, maxLeftoverSize);
            int wantedMiners = numNeeded[iPlan] + leftovers;
   	      debugResourceBreakdown("           Plan wants num gatherers: " + wantedMiners + ".");
            numNeeded[iPlan] = max(1, wantedMiners - 2);
            numWanted[iPlan] = wantedMiners;
            numMax[iPlan] = wantedMiners + maxOverflowSize;
            maxCapacity += maxOverflowSize;
            unassigned -= leftovers;
            numBaseMiners += wantedMiners;
         }
         debugResourceBreakdown("           Num plans: " + numGoldPlans + ", num miners: " + numBaseMiners + ", num gold mines: " +
            numGoldMines + ".");
         aiSetResourceBreakdown(cResourceGold, cAIResourceSubTypeEasy, numGoldPlans,  mPrioGoldEasy, 
                                baseID, numNeeded, numWanted, numMax);
         totalMiners += numBaseMiners;
      }

      // Still have unassigned? Try create a remote base.
      if (unassigned > 0 && mAllowAutoRemoteCreation == true)
      {
         int remoteBaseID = internalCreateRemoteBase(cResourceGold, cAIResourceSubTypeEasy, remoteGoldRange);
         if (remoteBaseID != -1)
         {
            // Instantly include leftovers.
            int numRemote = min(targetGroupSize + 3, unassigned);
            totalMiners += numRemote;
            unassigned -= numRemote;
            int[] numNeeded = new int(1, max(1, numRemote - 2));
            int[] numWanted = new int(1, numRemote);
            int[] numMax = new int(1, numRemote + maxOverflowSize);
            maxCapacity += maxOverflowSize;
            debugResourceBreakdown("           *** Created remote gold base: " + kbBaseGetNameByID(cMyID, remoteBaseID) + 
               ", this remote base gets " + numRemote + " miners. ***");

            aiSetResourceBreakdown(cResourceGold, cAIResourceSubTypeEasy, 1, mPrioGoldEasy, 
                                   remoteBaseID, numNeeded, numWanted, numMax);
         }
         else
         {
            debugResourceBreakdown("           *** Failed to create a remote gold base. ***");
         }
      }

      // The number of gatherers that will fill up the MAX part of our plans.
      int numMAXGatherers = unassigned > maxCapacity ? maxCapacity : unassigned;
      debugResourceBreakdown("We have " + maxCapacity + " spots in the MAX part of our gold plans. And " + numMAXGatherers +
         " gatherers will actually be distributed to those spots.");
      unassigned -= numMAXGatherers;

      if (unassigned > 0)
      {
         debugResourceBreakdown("ATTENTION: Don't know what to do with " + unassigned + " miners.");
         alertRanOutOfGoldResources();
      }
      else
      {
         alertFoundGoldResources();
      }
      debugResourceBreakdown("We've managed to assign " + totalMiners + "/" + goldGathererCount + " of our miners.");
      return unassigned;
   }
   

   //==============================================================================
   // resourceBreakdownUpdateWood
   //==============================================================================
   int resourceBreakdownUpdateWood(int gathererCount = -1, int carryoverUnassigned = -1)
   {
      debugResourceBreakdown("--- Trying to assign wood gatherers ---");
      int woodGathererCount = round(aiGetResourcePercentage(cResourceWood) * gathererCount);
      if (carryoverUnassigned > 0)
      {
         debugResourceBreakdown("     Adding: " + carryoverUnassigned + " gatherers that couldn't be assigned to food/gold earlier.");
         woodGathererCount += carryoverUnassigned;
      }
      if (woodGathererCount == 0)
      {
         nullPercentageForBases(0, cResourceWood);
         debugResourceBreakdown("     No wood gatherers to assign, quiting.");
         return 0;
      }
      if (gOverrideOkToGatherWood == false)
      {
         nullPercentageForBases(0, cResourceWood);
         debugResourceBreakdown("     Not allowed to gather wood via breakdowns because of gOverrideOkToGatherWood == false.");
         return woodGathererCount;
      }

      int totalLumberjacks = 0;
      int maxCapacity = 0;
      debugResourceBreakdown("     Trying to assign: " + woodGathererCount + " lumberjacks.");
      int unassigned = woodGathererCount;
      int numTargetVillagers = 8;
      int maxLeftoverSize = 4;
      int maxOverflowSize = 10;
      if (cMyCulture == cCultureAtlantean)
      {
         numTargetVillagers = 3;
         maxLeftoverSize = 2;
      }
      int targetGroupSize = numTargetVillagers + maxLeftoverSize;
      // We're going to process every base and its resources until we run out of villagers
      for (int iBase = 0; iBase < mGatherBases.size(); iBase++)
      {
         if (unassigned <= 0)
         {
            nullPercentageForBases(iBase, cResourceWood, cAIResourceSubTypeEasy);
            break;
         }
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         // We're now analyzing remote gather bases, we can gather wood in there, if the base is of the correct resource type.
         if (iBase >= mSortedTCBases.size() && mOverride == false)
         {
            if (kbBaseIsFlagSet(cMyID, baseID, cBaseFlagRemoteWoodGatherBase) == false)
            {
               debugResourceBreakdown("     *** Analyzing wood, SKIPPING gather base: " + kbBaseGetNameByID(cMyID, baseID) +
                                " of size: " + kbBaseGetDistance(cMyID, baseID) + " because it's not the right remote type.");
               continue;
            }
         }
         debugResourceBreakdown("     *** Analyzing wood, processing gather base: " + kbBaseGetNameByID(cMyID, baseID) +
                                " of size: " + kbBaseGetDistance(cMyID, baseID) + ".");
         int numWoodlines = internalBaseNumberResources(mEasyWoodResourcesOffset, iBase);
         if (numWoodlines == 0)
         {
            aiRemoveResourceBreakdown(cResourceWood, cAIResourceSubTypeEasy, baseID);
            continue;
         }

         // We always want to make use of all our woodlines if we can afford it.
         // But we don't want to allocate more villagers than 1 per tree
         int numAvailableTrees = internalBaseNumberResourceUnits(mAllEasyWoodResources, mEasyWoodResourcesOffset[iBase], numWoodlines);
         int numWoodPlans = min(numWoodlines, ceil(xsIntToFloat(min(unassigned, numAvailableTrees)) / xsIntToFloat(targetGroupSize)));
         
         int[] numNeeded = new int(numWoodPlans, 0);
         int[] numWanted = new int(numWoodPlans, 0);
         int[] numMax = new int(numWoodPlans, 0);
         // Assign per plan and try adding some leftovers
         for (int iPlan = 0; iPlan < numWoodPlans; iPlan++)
         {
            int neededLumberjacks = min(numAvailableTrees, min(unassigned, targetGroupSize));
            numNeeded[iPlan] = neededLumberjacks;
            unassigned -= neededLumberjacks;
            numAvailableTrees -= neededLumberjacks;
         }
         // Try adding some leftovers
         int numBaseLumberjacks = 0;
         for (int iPlan = 0; iPlan < numWoodPlans; iPlan++)
         {
            int leftovers = min(numAvailableTrees,min(unassigned, 3));
            int wantedLumberjacks = numNeeded[iPlan] + leftovers;
   	      debugResourceBreakdown("           Plan wants num gatherers: " + wantedLumberjacks + ".");
            numNeeded[iPlan] = max(1, wantedLumberjacks - 2);
            numWanted[iPlan] = wantedLumberjacks;
            numMax[iPlan] = wantedLumberjacks + maxOverflowSize;
            maxCapacity += maxOverflowSize;
            unassigned -= leftovers;
            numBaseLumberjacks += wantedLumberjacks;
         }

         debugResourceBreakdown("           Num plans: " + numWoodPlans + ", num lumberjacks: " + numBaseLumberjacks + ", num woodlines: " +
            numWoodlines + ".");
         aiSetResourceBreakdown(cResourceWood, cAIResourceSubTypeEasy, numWoodPlans,  mPrioWoodEasy, 
                                baseID, numNeeded, numWanted, numMax);
         totalLumberjacks += numBaseLumberjacks;
      }

      // Still have unassigned? Try create a remote base.
      if (unassigned > 0 && mAllowAutoRemoteCreation == true)
      {
         int remoteBaseID = internalCreateRemoteBase(cResourceWood, cAIResourceSubTypeEasy, remoteWoodRange);
         if (remoteBaseID != -1)
         {
            // Instantly include leftovers.
            int numRemote = min(targetGroupSize + 3, unassigned);
            totalLumberjacks += numRemote;
            unassigned -= numRemote;
            int[] numNeeded = new int(1, max(1, numRemote - 2));
            int[] numWanted = new int(1, numRemote);
            int[] numMax = new int(1, numRemote + maxOverflowSize);
            maxCapacity += maxOverflowSize;
            debugResourceBreakdown("           *** Created remote wood base: " + kbBaseGetNameByID(cMyID, remoteBaseID) + 
               ", this remote base gets " + numRemote + " lumberjacks. ***");
            aiSetResourceBreakdown(cResourceWood, cAIResourceSubTypeEasy, 1, mPrioWoodEasy, 
                                   remoteBaseID, numNeeded, numWanted, numMax);
         }
         else
         {
            debugResourceBreakdown("           *** Failed to create a remote wood base. ***");
         }
      }

      // The number of gatherers that will fill up the MAX part of our plans.
      int numMAXGatherers = unassigned > maxCapacity ? maxCapacity : unassigned;
      debugResourceBreakdown("We have " + maxCapacity + " spots in the MAX part of our wood plans. And " + numMAXGatherers +
         " gatherers will actually be distributed to those spots.");
      unassigned -= numMAXGatherers;
      // Todo some mechanism to tell that we've ran out of resources so don't allocate more wood gatherers.
      if (unassigned > 0)
      {
         debugResourceBreakdown("ATTENTION: Don't know what to do with " + unassigned + " lumberjacks.");
      }
      debugResourceBreakdown("We've managed to assign " + totalLumberjacks + "/" + woodGathererCount + " of our lumberjacks.");
      return unassigned;
   }


   //==============================================================================
   // resourceBreakdownUpdateFavor
   //==============================================================================
   int resourceBreakdownUpdateFavor(int gathererCount = -1, int carryoverUnassigned = -1)
   {
      debugResourceBreakdown("--- Trying to assign favor gatherers ---");
      int favorGathererCount = round(aiGetResourcePercentage(cResourceFavor) * gathererCount);
      if (carryoverUnassigned > 0)
      {
         debugResourceBreakdown("     Adding: " + carryoverUnassigned + " gatherers that couldn't be assigned to food/gold/wood earlier.");
         favorGathererCount += carryoverUnassigned;
      }
      if (favorGathererCount == 0)
      {
         nullPercentageForBases(0, cResourceFavor);
         debugResourceBreakdown("     No favor gatherers to assign, quiting.");
         return favorGathererCount;
      }
      if (gOverrideOkToGatherFavor == false)
      {
         nullPercentageForBases(0, cResourceFavor);
         debugResourceBreakdown("     Not allowed to gather favor via breakdowns because of gOverrideOkToGatherFavor == false.");
         return favorGathererCount;
      }

      debugResourceBreakdown("     Trying to assign: " + favorGathererCount + " prayers.");
      // We assume 1 side of the Temple can be blocked off due to whatever/no buffer space on first Temple, around 25 fit then.
      const int maxPerTemple = 25;
      int unassigned = favorGathererCount;
      int totalPrayers = 0;
      int[] usedTemples = new int(0, 0);

      for (int iBase = 0; iBase < mGatherBases.size(); iBase++)
      {
         if (unassigned <= 0)
         {
            nullPercentageForBases(iBase, cResourceFavor, cAIResourceSubTypeEasy);
            break;
         }
         int baseID = mGatherBases[iBase];
         if (kbBaseGetIsIDValid(cMyID, baseID) == false)
         {
            continue; // Base array is made and used in consecutives frames, bases can go invalid.
         }
         // We can only gather favor in TC bases, after we hit the first remote base we can just null + out.
         if (mOverride == false && kbBaseIsFlagSet(cMyID, baseID, cBaseFlagTownCenter) == false)
         {
            nullPercentageForBases(iBase, cResourceFavor, cAIResourceSubTypeEasy);
            break;
         }

         debugResourceBreakdown("     *** Analyzing favor, processing gather base: " + kbBaseGetNameByID(cMyID, baseID) +
                                " of size: " + kbBaseGetDistance(cMyID, baseID) + ".");
         // Count of sites (Temples for Greek).
         int queryID = useSimpleUnitQuery(cUnitTypeTemple, cMyID, cUnitStateAlive, kbBaseGetLocation(cMyID, baseID),
            kbBaseGetDistance(cMyID, baseID));
         kbUnitQueryExecute(queryID);
         int[] results = kbUnitQueryGetResults(queryID);
         int numTemplesInBase = 0;
         for (int i = 0; i < results.size(); i++)
         {
            if (usedTemples.find(results[i]) != -1)
            {
               continue;
            }
            usedTemples.add(results[i]);
            numTemplesInBase++;
         }
         if (numTemplesInBase == 0)
         {
            aiRemoveResourceBreakdown(cResourceFavor, cAIResourceSubTypeEasy, baseID);
            continue;
         }
         int numFavorPlans = min(numTemplesInBase, ceil(xsIntToFloat(unassigned) / xsIntToFloat(maxPerTemple)));
         int[] numNeeded = new int(numFavorPlans, 0);
         int[] numWanted = new int(numFavorPlans, 0);
         int[] numMax = new int(numFavorPlans, maxPerTemple); // If we have idle Villagers due to whatever they can always pray.

         int numBasePrayers = 0;
         for (int iPlan = 0; iPlan < numFavorPlans; iPlan++)
         {
            int prayers = min(maxPerTemple, unassigned);
            unassigned -= prayers;
            totalPrayers += prayers;
            numBasePrayers += prayers;
            numNeeded[iPlan] = max(1, prayers - 2);
            numWanted[iPlan] = prayers;
         }
         debugResourceBreakdown("           Num plans: " + numFavorPlans + ", num prayers: " + numBasePrayers + ", num Temples: " +
            numTemplesInBase + ".");
         aiSetResourceBreakdown(cResourceFavor, cAIResourceSubTypeEasy, numFavorPlans,  mPrioFavorEasy,
                                baseID, numNeeded, numWanted, numMax);
      }
      debugResourceBreakdown("We've managed to assign " + totalPrayers + "/" + favorGathererCount + " of our prayers.");
      return unassigned;
   }
   

   //==============================================================================
   // addRemoteGatherBase
   // Used by the BO system for the transition. 
   //==============================================================================
   void addRemoteGatherBase(int baseID = -1)
   {
      debugResourceBreakdown("Adding: " + kbBaseGetNameByID(cMyID, baseID) + " to our remote gather bases array.");
      mInternalRemoteGatherBases.add(baseID);
   }

   //==============================================================================
   // createRemoteGatherBase
   // Used by the BO system for the transition. 
   //==============================================================================
   int createRemoteGatherBase(vector location = cInvalidVector, float size = -1.0)
   {
      int baseID = kbBaseCreate(cMyID, kbBaseGetNextID() + ": RemoteBase", location, size);
      kbBaseSetFlag(cMyID, baseID, cBaseFlagRemoteGatherBase, true);
      kbBaseSetFlag(cMyID, baseID, cBaseFlagEconomy, true);
      kbBaseSetDistance(cMyID, baseID, size);
      mInternalRemoteGatherBases.add(baseID);
      debugResourceBreakdown("Adding: " + kbBaseGetNameByID(cMyID, baseID) + " to our remote gather bases array.");
      return baseID;
   }
};
extern ResourceBreakdownSystem gRBDSystem;

//==============================================================================
// createOverrideGatherBase
// Campaign function to force gathering to happen in specific bases.
//==============================================================================
int createOverrideGatherBase(vector position = cInvalidVector, float size = -1.0)
{
   gRBDSystem.mAllowAutoRemoteCreation = false;
   gRBDSystem.mOverride = true;
   int baseID = gRBDSystem.createRemoteGatherBase(position, size);
   kbBaseSetFlag(cMyID, baseID, cBaseFlagCampaignGatherBase, true);
   return baseID;
}

//==============================================================================
// removeOverrideGatherBase
//==============================================================================
void removeOverrideGatherBase(int baseID = -1)
{
   int index = gRBDSystem.mInternalRemoteGatherBases.find(baseID);
   if (index == -1)
   {
      aiEchoWarning("Trying to delete a remote gather base with ID " + baseID + ", but we don't have that base in our arrays.");
      return;
   }
   removeAllBreakdownsFromBase(baseID);
   gRBDSystem.mInternalRemoteGatherBases.removeIndex(index);
}
