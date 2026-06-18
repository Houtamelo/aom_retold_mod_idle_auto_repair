//==============================================================================
/* dropsite_placement.xs

   This file is intended for picking the best spots to build a dropsites

*/
//==============================================================================

//==============================================================================
// calculateFoodDropsitePlacement
//==============================================================================
void calculateFoodDropsitePlacement(int buildPlanID = -1, int bpID = -1, int kbResourceID = -1)
{
   if (kbResourceGetIsIDValid(kbResourceID) == false)
   {
      aiEchoWarning(aiPlanGetName(buildPlanID) + ", calling calculateFoodDropsitePlacement with kbResourceID being -1, placement " +
         "can't function.");
      return;
   }

   // Very custom logic for Farms.
   if (kbResourceGetSubType(kbResourceID) == cAIResourceSubTypeFarm)
   {
      int farmID = kbResourceGetFarmIDWithFurthestAwayDropsite(kbResourceID);
      // We will use the Farm's position as our center position.
      vector farmPosition = kbUnitGetPosition(farmID);
      kbBuildingPlacementSetCenterPosition(bpID, farmPosition, 8.0);
      // Orient towards a TC if we have one.
      int closestTC = getClosestUnitByLocation(cUnitTypeAbstractTownCenter, cMyID, cUnitStateABQ, farmPosition, 55.0);
      if (closestTC != -1)
      {
         kbBuildingPlacementAddUnitInfluence(bpID, cUnitTypeAbstractTownCenter, 1000.0, 55.0, cFalloffLinear);
         debugBuildingPlacements(aiPlanGetName(buildPlanID) + ", calculateFoodDropsitePlacement Farm found a TC as influence.");
      }
      // 0.5 obstruction to make sure Villagers can still path around it always.
      kbBuildingPlacementSetBufferSpace(bpID, 0.5);
      kbBuildingPlacementSetStepSize(bpID, 1.0);
      kbBuildingPlacementAddPositionInfluence(bpID, farmPosition, 350.0, 10.0, cFalloffLinear);
      return;
   }

   int unitID = -1;
   for (int i = 0; i < kbResourceGetNumberUnits(kbResourceID); i++)
   {
      if (kbUnitGetIsIDValid(kbResourceGetUnit(kbResourceID, i), true) == true)
      {
         unitID = kbResourceGetUnit(kbResourceID, i);
         break;
      }
   }

   // Build right on top of the center of the resource.
   kbBuildingPlacementSetBufferSpace(bpID, 0.0);
   if (unitID != -1 && kbUnitGetProtoUnitID(unitID) == cUnitTypeBerryBush)
   {
      // This must happen or we run a high risk of needing to walk all the way around to reach the berry at index 0.
      debugBuildingPlacements(aiPlanGetName(buildPlanID) + ", calculateFoodDropsitePlacement using 1.5 buffer space because of berries.");
      kbBuildingPlacementSetBufferSpace(bpID, 1.5);
   }
   vector placeGranaryHere = kbResourceGetPosition(kbResourceID);

   kbBuildingPlacementSetCenterPosition(bpID, placeGranaryHere, 8.0);
   kbBuildingPlacementSetStepSize(bpID, 1.0);

   kbBuildingPlacementAddPositionInfluence(bpID, placeGranaryHere, 100.0, 8.0, cFalloffLinear);
}

//==============================================================================
// calculateWoodDropsitePlacement
//==============================================================================
void calculateWoodDropsitePlacement(int buildPlanID = -1, int bpID = -1, int kbResourceID = -1, bool atlantean = false)
{
   if (kbResourceID == -1)
   {
      aiEchoWarning(aiPlanGetName(buildPlanID) + ", calling calculateWoodDropsitePlacement with kbResourceID being -1, placement " +
         "can't function.");
      return;
   }

   // We build a little bit away from the trees to prevent us from not being able to reach tree index 0.
   // And to prevent our Villagers from getting stuck when a Villager in the middle goes to another plan.
   kbBuildingPlacementSetBufferSpace(bpID, 2.0);
   vector closestDropsiteLocation = cInvalidVector;
   // We always orient towards TC during BO, we know for a fact we don't have an existing dropsite already.
   // This prevents Greek picking up the Gold Storehouse as a valid influence for the wood Storehouse.
   if (isBuildOrderDone() == true && kbUnitGetIsIDValid(kbResourceGetClosestDropsiteID(kbResourceID)) == true)
   {
      closestDropsiteLocation = kbUnitGetPosition(kbResourceGetClosestDropsiteID(kbResourceID));
   }
   vector basePosition = cInvalidVector;

   // It could be that our first unit in the KB Resource has just been harvested before we call this, then it's invalid.
   // So let's search for the first valid one.
   int unitID = -1;
   int numTrees = kbResourceGetNumberUnits(kbResourceID);
   for (int i = 0; i < numTrees; i++)
   {
      if (kbUnitGetIsIDValid(kbResourceGetUnit(kbResourceID, i), true) == true)
      {
         unitID = kbResourceGetUnit(kbResourceID, i);
         break;
      }
   }
   // On high game speeds the resource could already be gone before we reach this point, guard against that. (unlikely but still)
   if (unitID == -1)
   {
      debugBuildingPlacements("Found no valid units inside the KB Resource (" + kbResourceID + ") during calculateWoodDropsitePlacement.");
      return;
   }

   // If we have no existing dropsite we will use a TC for orientation.
   if (closestDropsiteLocation == cInvalidVector)
   {
      int closestTownCenterID = getClosestUnitByLocation(cUnitTypeAbstractTownCenter, cMyID, cUnitStateAlive,
         kbUnitGetPosition(unitID), 50.0);
      if (closestTownCenterID != -1)
      {
         basePosition = kbUnitGetPosition(closestTownCenterID);
      }
   }
   // If we also have no TC we try any building for orientation.
   if (closestDropsiteLocation == cInvalidVector && basePosition == cInvalidVector)
   {
      int closestBuildingID = getClosestUnitByLocation(cUnitTypeBuilding, cMyID, cUnitStateAlive,
         kbUnitGetPosition(unitID), 50.0);
      if (closestBuildingID != -1)
      {
         basePosition = kbUnitGetPosition(closestBuildingID);
      }
   }

   // If we lack any buildings in the vicinity we just search a good spot without orienting to any side.
   if (closestDropsiteLocation == cInvalidVector && basePosition == cInvalidVector)
   {
      debugBuildingPlacements(aiPlanGetName(buildPlanID) + ", calculateWoodDropsitePlacement found no closest dropsite or base. " + 
         "We will build around the center of the resource now.");
      // Since we scan from the center of the forest we need a big distance.
      kbBuildingPlacementSetCenterPosition(bpID, kbResourceGetPosition(kbResourceID), 20.0);
      kbBuildingPlacementSetStepSize(bpID, 1.0);
   }
   else if (closestDropsiteLocation != cInvalidVector) // Can never hit this as Atlantean.
   {
      // If we already have a dropsite it means that we will base our new dropsite off the old dropsite's location.
      // We do this because we assume that the old dropsite was placed correctly since we placed it manually using the
      // code in the else block below. The KB resource has just been sorted by proximity (in the source)  towards the old dropsite
      // and the unit at index 0 will be closest to our old dropsite.
      debugBuildingPlacements(aiPlanGetName(buildPlanID) + ", calculateWoodDropsitePlacement we already had a dropsite, using the closest " + 
         "tree towards our old dropsite now as the main influence.");
      kbBuildingPlacementSetCenterPosition(bpID, kbUnitGetPosition(unitID), 10.0);
      kbBuildingPlacementSetStepSize(bpID, 1.0);

      // Step away from the closest tree towards our old dropsite first.
      vector influencePosition = kbUnitGetPosition(unitID) -
                                 (xsVectorNormalize((kbUnitGetPosition(unitID) - closestDropsiteLocation) * 5));
      kbBuildingPlacementAddPositionInfluence(bpID, influencePosition, 350.0, 7.0, cFalloffNone);
      // Orient a bit around the closest tree, this makes sure that if we can't build close enough to the resource to get any
      // other stricter influences to take effect we at least build towards the forest.
      kbBuildingPlacementAddPositionInfluence(bpID, kbUnitGetPosition(unitID), 25.0, 10.0, cFalloffLinear);
   }
   else // basePosition != cInvalidVector.
   {
      debugBuildingPlacements(aiPlanGetName(buildPlanID) + ", calculateWoodDropsitePlacement found no dropsite but did find a base. " + 
         "Using this base's location now to scan for the closest tree.");
      kbResourceSortTowardsPosition(kbResourceID, basePosition);
      int closestTreeID = kbResourceGetUnit(kbResourceID, 0);
      debugBuildingPlacements(aiPlanGetName(buildPlanID) + ", calculateWoodDropsitePlacement found closest treeID: " + closestTreeID);
      kbBuildingPlacementSetCenterPosition(bpID, kbUnitGetPosition(closestTreeID), 10.0);
      kbBuildingPlacementSetStepSize(bpID, 1.0);

      // Step away from the closest tree towards our base first.
      vector influencePosition = kbUnitGetPosition(closestTreeID) -
                                 (xsVectorNormalize((kbUnitGetPosition(closestTreeID) - basePosition) * 5));
      if (atlantean == false)
      {
         kbBuildingPlacementAddPositionInfluence(bpID, influencePosition, 350.0, 7.0, cFalloffNone);
         // Orient a bit around the closest tree, this makes sure that if we can't build close enough to the resource to get any
         // other stricter influences to take effect we at least build towards the forest.
         kbBuildingPlacementAddPositionInfluence(bpID, kbUnitGetPosition(closestTreeID), 25.0, 10.0, cFalloffLinear);
      }
      else // For Atlantean we just need to build close towards the closest tree, no really strict rules which can block a big Manor.
      {
         kbBuildingPlacementAddPositionInfluence(bpID, influencePosition, 350.0, 10.0, cFalloffLinear);
      }
   }

   // Search a good spot with many trees next to it.
   kbBuildingPlacementAddUnitInfluence(bpID, cUnitTypeTree, 40.0, 7.0, cFalloffLinear, kbResourceID);
}

//==============================================================================
// calculateGoldDropsitePlacement
//==============================================================================
void calculateGoldDropsitePlacement(int buildPlanID = -1, int bpID = -1, int kbResourceID = -1, bool atlantean = false)
{
   if (kbResourceID == -1)
   {
      aiEchoWarning(aiPlanGetName(buildPlanID) + ", calling calculateWoodDropsitePlacement with kbResourceID being -1, placement " +
         "can't function.");
      return;
   }

   vector resourceLocation = kbResourceGetPosition(kbResourceID);
   vector placementCenter = resourceLocation;
   bool resourceNearAmphibiousTerrain = false;

   int resourceAreaID = kbAreaGetIDByPosition(resourceLocation);
   if (kbAreaGetIsIDValid(resourceAreaID) == true)
   {
      int resourceAreaType = kbAreaGetType(resourceAreaID);
      int resourceAreaGroupID = kbAreaGetGroupID(resourceAreaID);
      if (resourceAreaType == cAreaTypeAmphibious ||
          (resourceAreaGroupID != -1 && kbAreaGroupGetType(resourceAreaGroupID) == cAreaGroupTypeAmphibious))
      {
         resourceNearAmphibiousTerrain = true;
      }
   }

   if (resourceNearAmphibiousTerrain == false)
   {
      int[] amphibiousTypes = new int(1, -1);
      amphibiousTypes[0] = cAreaTypeAmphibious;
      int[] amphibiousAreas = kbAreaGetIDsByPositionAndRange(resourceLocation, 18.0, amphibiousTypes, true,
         cPassabilityAmphibious);
      resourceNearAmphibiousTerrain = amphibiousAreas.size() > 0;
   }

   if (resourceNearAmphibiousTerrain == true)
   {
      vector passablePosition = kbFindClosestPassablePoint(resourceLocation, cPassabilityLand, 18.0);
      if (passablePosition == cInvalidVector)
      {
         passablePosition = kbFindClosestPassablePoint(resourceLocation, cPassabilityAmphibious, 18.0);
      }
      if (passablePosition != cInvalidVector)
      {
         placementCenter = passablePosition;
      }
      debugBuildingPlacements(aiPlanGetName(buildPlanID) + ", calculateGoldDropsitePlacement found amphibious terrain near " +
         "the gold mine.");
   }

   if (atlantean == false)
   {
      // Avoid the mine, we need to leave a lot of space so miners can't get stuck.
      kbBuildingPlacementSetBufferSpace(bpID, 1.5);
   }

   // 8.0 Atty, 6.5 everybody else. Manors can be built a little further away from the resource, dropsites we don't want that.
   // Unique Aztec dropsite is very large, we need 12.0 so it can even be built at all.
   float centerDistance = 6.5;
   if (cMyCulture == cCultureAtlantean)
   {
      centerDistance = 8.0;
   }
   else if (cMyCulture == cCultureAztec)
   {
      centerDistance = 7.5;
   }
   if (resourceNearAmphibiousTerrain == true)
   {
      centerDistance = 18.0;
   }
   kbBuildingPlacementSetCenterPosition(bpID, placementCenter, centerDistance);

   if (resourceNearAmphibiousTerrain == true)
   {
      int[] validTypes = new int(4, -1);
      validTypes[0] = cAreaTypePassableLand;
      validTypes[1] = cAreaTypeGold;
      validTypes[2] = cAreaTypeSettlement;
      validTypes[3] = cAreaTypeAmphibious;
      int[] areaIDs = kbAreaGetIDsByPositionAndRange(resourceLocation, centerDistance, validTypes, true,
         cPassabilityAmphibious);
      for (int i = 0; i < areaIDs.size(); i++)
      {
         kbBuildingPlacementAddAreaID(bpID, areaIDs[i], 0, false);
      }
      int[] landAreaIDs = kbAreaGetIDsByPositionAndRange(placementCenter, centerDistance, validTypes, true,
         cPassabilityLand);
      for (int i = 0; i < landAreaIDs.size(); i++)
      {
         kbBuildingPlacementAddAreaID(bpID, landAreaIDs[i], 0, false);
      }
   }

   if (placementCenter != resourceLocation)
   {
      kbBuildingPlacementAddPositionInfluence(bpID, placementCenter, 125.0, 10.0, cFalloffLinear);
   }
   kbBuildingPlacementSetStepSize(bpID, 0.5);

   // We want the dropsite as close to the mine as possible. So the center of the mine has an influence.
   // It's strong enough to make us build close to it but the side we chose should still be determined by the TC influence below.
   kbBuildingPlacementAddPositionInfluence(bpID, resourceLocation, resourceNearAmphibiousTerrain == true ? 75.0 : 10.0,
      10.0, cFalloffLinear);

   // If we have a Town Center within 75 range we will build towards that TC.
   int townCenterID = getClosestUnitByLocation(cUnitTypeAbstractTownCenter, cMyID, cUnitStateAlive, resourceLocation, 75.0);
   if (townCenterID != -1)
   {
      debugBuildingPlacements(aiPlanGetName(buildPlanID) + ", calculateGoldDropsitePlacement found a TC to use for influence.");
      if (resourceNearAmphibiousTerrain == false)
      {
         kbBuildingPlacementAddPositionInfluence(bpID, kbUnitGetPosition(townCenterID), 100.0, 75.0, cFalloffLinear);
      }
   }
}


//==============================================================================
// selectDropsitePlacement
// This is the default code that gets ran whenever a gather plan decides it wants
// to create another dropsite via the automatic system that is source driven.
//==============================================================================
void selectDropsitePlacement(int planID = -1)
{
   aiPlanSetPriority(planID, 100);
   
   int parentPlanID = aiPlanGetParentID(planID);
   if (parentPlanID == -1)
   {
      aiEchoWarning("Calling selectDropsitePlacement - Our parentPlanID is invalid somehow? Calling for " + aiPlanGetName(planID));
      aiPlanSetState(planID, cPlanStateFailed);
      return;
   }
   // We set this flag to true in the source to prevent our Villagers getting stolen before we run this handler, unset it now.
   aiPlanSetFlag(parentPlanID, cPlanFlagCantBeStolenFrom, false);

   aiPlanSetVariableBool(planID, cBuildPlanFailOnUnaffordable, 0, true);
   int bpID = -1;
   // If this is a failsafe build there still exists another building placement with the plan's name, can't create duplicates.
   if (aiPlanGetVariableBool(parentPlanID, cGatherPlanFailsafeBuild, 0) == true)
   {
      bpID = kbBuildingPlacementCreate(aiPlanGetName(planID) + " failsafe");
   }
   else
   {
      bpID = kbBuildingPlacementCreate(aiPlanGetName(planID));
   }
   if (bpID == -1)
   {
      aiPlanSetState(planID, cPlanStateFailed);
      return;
   }
   kbBuildingPlacementSetBuildingPUID(bpID, aiPlanGetVariableInt(planID, cBuildPlanBuildingTypeID, 0));
   aiPlanSetVariableInt(planID, cBuildPlanBuildingPlacementID, 0, bpID);

   
   int kbResourceID = aiPlanGetVariableInt(parentPlanID, cGatherPlanKBResourceID, 0);
   if (kbResourceGetIsIDValid(kbResourceID) == false)
   {
      aiEchoWarning("Calling selectDropsitePlacement with a plan that has an invalid kbResourceID. Plan: " + aiPlanGetName(planID) + 
         ", kbResourceID " + kbResourceID + ".");
      aiPlanSetState(planID, cPlanStateFailed);
      return;
   }

   debugBuildingPlacements("Automatic placement of a new dropsite for gather plan: " + aiPlanGetName(parentPlanID) + ", build plan: "
      + aiPlanGetName(planID) + ", kbResourceID: " + kbResourceID);

   int[] units = aiPlanGetUnits(parentPlanID, cUnitTypeAbstractVillager);
   if (units.size() == 0)
   {
      aiEcho("Calling selectDropsitePlacement but we have no Villagers in the plan: " + aiPlanGetName(parentPlanID) + ".");
      aiPlanSetState(planID, cPlanStateFailed);
      return;
   }

   // Only try once.
   aiPlanSetVariableInt(planID, cBuildPlanMaxRetries, 0);

   // Take any units already assigned to the plan to help out, but don't do this for Farms since they're so spread out.
   if (kbResourceGetSubType(kbResourceID) != cAIResourceSubTypeFarm)
   {
      for (int i = 0; i < units.size(); i++)
      {
         aiPlanAddUnit(planID, units[i]);
      }
   }
   else
   {
      // We build the Granary with the 2 closest Villagers.
      vector scanPosition = kbUnitGetPosition(kbResourceGetFarmIDWithFurthestAwayDropsite(kbResourceID));
      float closestDistance = cMaxFloat;
      float secondClosestDistance = cMaxFloat;
      int closestVillagerID = -1;
      int secondClosestVillagerID = -1;
      for (int i = 0; i < units.size(); i++)
      {
         float distance = xsVectorDistanceXZ(scanPosition, kbUnitGetPosition(units[i]));
         if (distance < closestDistance)
         {
            secondClosestVillagerID = closestVillagerID;
            secondClosestDistance = closestDistance;
            closestDistance = distance;
            closestVillagerID = units[i];
         }
         else if (distance < secondClosestDistance)
         {
            secondClosestVillagerID = units[i];
            secondClosestDistance = distance;
         }
      }
      // Must be valid since we have at least 1 unit in this plan.
      aiPlanAddUnit(planID, closestVillagerID);
      if (secondClosestVillagerID != -1)
      {
         aiPlanAddUnit(planID, secondClosestVillagerID);
      }
   }

   int resourceType = kbResourceGetType(kbResourceID);
   if (resourceType == cResourceGold)
   {
      calculateGoldDropsitePlacement(planID, bpID, kbResourceID, false);
   }
   else if (resourceType == cResourceWood)
   {
      calculateWoodDropsitePlacement(planID, bpID, kbResourceID);
   }
   else // Food.
   {
      calculateFoodDropsitePlacement(planID, bpID, kbResourceID);
   }

   if (aiPlanGetVariableBool(parentPlanID, cGatherPlanFailsafeBuild, 0) == true)
   {
      debugBuildingPlacements("This is a failsafe dropsite build, no buffer space.");
      kbBuildingPlacementSetBufferSpace(bpID, 0.0);
   }
}
