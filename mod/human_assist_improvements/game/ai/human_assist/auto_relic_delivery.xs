//==============================================================================
// auto_relic_delivery.xs (Human Assist Improvements mod)
//
// Poll-based auto-relic-delivery for human players. Every 2 seconds we snapshot
// all alive ground relics and compare unit IDs with the previous tick. Each
// relic ID that disappears is treated as a probable pickup; we search player
// heroes within 10 meters of the relic's last position and task the first hero
// that carries the exact relic unit ID, is idle, and has no active AI plan to
// the nearest player-owned temple with relic space.
//
// This replaces the previous event-driven design based on
// cXSRelicPickedUpHandler, which does not fire for human players.
//
// Canonical architecture and requirements:
//   openspec/changes/auto-relic-delivery-reinvestigation/design.md
//   openspec/changes/auto-relic-delivery-reinvestigation/specs/auto-relic-delivery/spec.md
//
// Loaded from human_assist.xs via:
//     include "human_assist/auto_relic_delivery.xs";
//==============================================================================

// Maximum tracked (hero, relic) pairs. Hero counts are small (< 20 in a
// typical match); 64 leaves comfortable headroom for scenario-editor spam.
const int cAutoRelic_MaxTrackerPairs = 64;

// Pending-disappearance age-out, in scan ticks. Each tick is ~2 seconds.
const int cAutoRelic_PendingMaxAge = 4;

// Shared queries.
int gAutoRelic_heroQuery = -1;   // reusable hero proximity query
int gAutoRelic_templeQuery = -1; // player-owned temples near a hero
int gAutoRelic_relicQuery = -1;  // alive ground relics

// Previous-tick snapshot of ground relics.
extern int[]    gAutoRelic_prevRelicIDs       = default;
extern vector[] gAutoRelic_prevRelicPositions = default;

// Pending disappearances that failed delivery because the carrier was busy
// (pickup animation or player override). Retried on subsequent ticks.
extern int[]    gAutoRelic_pendingRelicIDs       = default;
extern vector[] gAutoRelic_pendingRelicPositions = default;
extern int[]    gAutoRelic_pendingAge            = default;
// Parallel hero ID is stored so we can mark the pair triggered on age-out.
extern int[]    gAutoRelic_pendingHeroIDs        = default;

// Trigger-once tracker: parallel arrays indexed by slot. A pair is appended
// the first time a delivery order is issued for that (heroID, relicID), so any
// subsequent scan for the same pair is silently skipped.
extern int[] gAutoRelic_heroID  = default;
extern int[] gAutoRelic_relicID = default;

//==============================================================================
// Query setup
//==============================================================================

void autoRelicDelivery_setupHeroProximityQuery()
{
   if (gAutoRelic_heroQuery != -1) { return; }
   gAutoRelic_heroQuery = kbUnitQueryCreate("autoRelic_heroProximity");
   kbUnitQuerySetPlayerID(gAutoRelic_heroQuery, cMyID, false);
   kbUnitQuerySetUnitType(gAutoRelic_heroQuery, cUnitTypeHero);
   kbUnitQuerySetState(gAutoRelic_heroQuery, cUnitStateAlive);
}

void autoRelicDelivery_setupRelicQuery()
{
   if (gAutoRelic_relicQuery != -1) { return; }
   gAutoRelic_relicQuery = kbUnitQueryCreate("autoRelic_relicsOnGround");
   // Ground relics are owned by gaia (player 0).
   kbUnitQuerySetPlayerID(gAutoRelic_relicQuery, 0, false);
   kbUnitQuerySetUnitType(gAutoRelic_relicQuery, cUnitTypeRelic);
   kbUnitQuerySetState(gAutoRelic_relicQuery, cUnitStateAlive);
}

void autoRelicDelivery_setupTempleQuery()
{
   if (gAutoRelic_templeQuery != -1) { return; }
   gAutoRelic_templeQuery = kbUnitQueryCreate("autoRelic_temples");
   kbUnitQuerySetPlayerID(gAutoRelic_templeQuery, cMyID, false);
   kbUnitQuerySetUnitType(gAutoRelic_templeQuery, cUnitTypeTemple);
   kbUnitQuerySetState(gAutoRelic_templeQuery, cUnitStateAlive);
   kbUnitQuerySetAscendingSort(gAutoRelic_templeQuery, true);
}

//==============================================================================
// (heroID, relicID) trigger-once tracker helpers
//==============================================================================

bool autoRelicDelivery_hasPair(int heroID = -1, int relicID = -1)
{
   if (heroID < 0 || relicID < 0) { return(false); }
   int n = gAutoRelic_heroID.size();
   for (int i = 0; i < n; i = i + 1)
   {
      if (gAutoRelic_heroID[i] == heroID && gAutoRelic_relicID[i] == relicID)
      {
         return(true);
      }
   }
   return(false);
}

void autoRelicDelivery_addPair(int heroID = -1, int relicID = -1)
{
   if (heroID < 0 || relicID < 0) { return; }
   if (gAutoRelic_heroID.size() >= cAutoRelic_MaxTrackerPairs)
   {
      aiEcho("autoRelicDelivery: tracker overflow, skipping pair (" + heroID + "," + relicID + ")");
      return;
   }
   gAutoRelic_heroID.add(heroID);
   gAutoRelic_relicID.add(relicID);
}

//==============================================================================
// Relic-snapshot diff
//==============================================================================

int autoRelicDelivery_computeDisappearances(int[] prevIDs = default, vector[] prevPos = default,
                                            int[] curIDs = default, vector[] curPos = default,
                                            int[] outIDs = default, vector[] outPos = default)
{
   outIDs.clear();
   outPos.clear();

   int prevCount = prevIDs.size();
   int curCount = curIDs.size();
   for (int i = 0; i < prevCount; i = i + 1)
   {
      int candidateID = prevIDs[i];
      bool found = false;
      for (int j = 0; j < curCount; j = j + 1)
      {
         if (curIDs[j] == candidateID)
         {
            found = true;
            break;
         }
      }
      if (found == false)
      {
         outIDs.add(candidateID);
         outPos.add(prevPos[i]);
      }
   }
   return(outIDs.size());
}

//==============================================================================
// Hero-carrier helpers
//==============================================================================

int[] autoRelicDelivery_findCarrierInRange(vector pos = cInvalidVector)
{
   autoRelicDelivery_setupHeroProximityQuery();

   kbUnitQuerySetPosition(gAutoRelic_heroQuery, pos);
   kbUnitQuerySetMaximumDistance(gAutoRelic_heroQuery, 10.0);
   kbUnitQueryResetResults(gAutoRelic_heroQuery);

   kbUnitQueryExecute(gAutoRelic_heroQuery);
   return(kbUnitQueryGetResults(gAutoRelic_heroQuery));
}

bool autoRelicDelivery_heroCarriesRelic(int heroID = -1, int relicID = -1)
{
   if (heroID < 0 || relicID < 0) { return(false); }
   if (kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) <= 0) { return(false); }

   // Fast path: slot 0 is the normal carrying slot for a hero.
   int slot0 = kbUnitGetContainedUnitByIndex(heroID, 0);
   if (slot0 == relicID && kbUnitIsType(slot0, cUnitTypeRelic) == true)
   {
      return(true);
   }

   // Defensive: scan all contained slots.
   int count = kbUnitGetNumberContained(heroID);
   for (int i = 0; i < count; i = i + 1)
   {
      int containedID = kbUnitGetContainedUnitByIndex(heroID, i);
      if (containedID == relicID && kbUnitIsType(containedID, cUnitTypeRelic) == true)
      {
         return(true);
      }
   }
   return(false);
}

bool autoRelicDelivery_heroIsDeliverable(int heroID = -1)
{
   if (heroID < 0) { return(false); }
   return(kbUnitGetActionType(heroID) == cActionTypeIdle
          && kbUnitGetPlanID(heroID) == cInvalidID);
}

//==============================================================================
// Nearest temple with available relic space
//==============================================================================

int autoRelicDelivery_findNearestTempleWithSpace(int heroID = -1)
{
   if (heroID < 0) { return(-1); }
   autoRelicDelivery_setupTempleQuery();

   vector heroPos = kbUnitGetPosition(heroID);
   kbUnitQuerySetPosition(gAutoRelic_templeQuery, heroPos);
   kbUnitQuerySetMaximumDistance(gAutoRelic_templeQuery, cMaxFloat);
   kbUnitQueryResetResults(gAutoRelic_templeQuery);

   int templeCount = kbUnitQueryExecute(gAutoRelic_templeQuery);
   int maxContained = kbPlayerGetProtoStatInt(cMyID, cUnitTypeTemple, cProtoStatMaxContained);

   for (int i = 0; i < templeCount; i = i + 1)
   {
      int templeID = kbUnitQueryGetResult(gAutoRelic_templeQuery, i);
      if (templeID < 0) { continue; }
      if (kbUnitGetNumberContained(templeID) < maxContained)
      {
         return(templeID);
      }
   }

   aiEcho("autoRelicDelivery: no temple with space -> skip");
   return(-1);
}

//==============================================================================
// Issue delivery order and record the pair
//==============================================================================

void autoRelicDelivery_issueDelivery(int heroID = -1, int relicID = -1)
{
   if (heroID < 0 || relicID < 0) { return; }
   if (autoRelicDelivery_hasPair(heroID, relicID) == true) { return; }

   int templeID = autoRelicDelivery_findNearestTempleWithSpace(heroID);
   if (templeID < 0)
   {
      return;
   }

   aiEcho("autoRelicDelivery: hero " + heroID + " carrying target relic -> delivering to temple " + templeID);
   aiTaskWorkUnit(heroID, templeID);
   autoRelicDelivery_addPair(heroID, relicID);
}

//==============================================================================
// Registration (called once from human_assist.xs::main())
//==============================================================================

void autoRelicDelivery_register()
{
   // CRITICAL: human_assist.xs::main() calls disableVillagerAssist() immediately
   // before us, and that function ends with xsSetContextPlayer(-1). Set the
   // context explicitly so diagnostic banners and rule enablement land in the
   // human player's context, then restore to -1 on exit.
   xsSetContextPlayer(cMyID);

   aiEcho("autoRelicDelivery: register() called cMyID=" + cMyID
          + " kbPlayerIsHuman=" + kbPlayerIsHuman(cMyID));

   // AI players keep vanilla behaviour; this is a human-assist mod feature.
   if (kbPlayerIsHuman(cMyID) == false)
   {
      aiEcho("autoRelicDelivery: skipping registration (AI player context)");
      xsSetContextPlayer(-1);
      return;
   }

   aiEcho("autoRelicDelivery: enabling scan rule");

   // Arrays declared with = default start at zero length. The tracker and
   // snapshot arrays reset each game because globals are reloaded with the script.
   xsEnableRule("autoRelicDelivery_scanRelics");

   xsSetContextPlayer(-1);
}

//==============================================================================
// Core poll rule: 2-second ground-relic scan
//==============================================================================

rule autoRelicDelivery_scanRelics
minInterval 2
inactive
{
   if (kbPlayerIsHuman(cMyID) == false) { return; }
   xsSetContextPlayer(cMyID);

   autoRelicDelivery_setupRelicQuery();
   autoRelicDelivery_setupHeroProximityQuery();

   // Snapshot current ground relics and their positions.
   kbUnitQueryResetResults(gAutoRelic_relicQuery);
   int relicCount = kbUnitQueryExecute(gAutoRelic_relicQuery);

   int[] curRelicIDs = default;
   vector[] curRelicPositions = default;
   for (int i = 0; i < relicCount; i = i + 1)
   {
      int relicID = kbUnitQueryGetResult(gAutoRelic_relicQuery, i);
      curRelicIDs.add(relicID);
      curRelicPositions.add(kbUnitGetPosition(relicID));
   }

   aiEcho("autoRelicDelivery: tick start (relicsOnGround=" + relicCount + ")");

   // Detect relic IDs that were present last tick but are gone now.
   int[] disappearedIDs = default;
   vector[] disappearedPositions = default;
   autoRelicDelivery_computeDisappearances(gAutoRelic_prevRelicIDs,
                                           gAutoRelic_prevRelicPositions,
                                           curRelicIDs,
                                           curRelicPositions,
                                           disappearedIDs,
                                           disappearedPositions);

   for (int i = 0; i < disappearedIDs.size(); i = i + 1)
   {
      vector pos = disappearedPositions[i];
      aiEcho("autoRelicDelivery: disappearance detected (relicID=" + disappearedIDs[i]
             + ", pos=(" + pos.x + "," + pos.z + "))");
   }

   //--------------------------------------------------------------------------
   // Age pending entries first; entries that reached max age are dropped and
   // their (hero, relic) pair is marked triggered so we do not fight the player
   // indefinitely.
   //--------------------------------------------------------------------------
   int pendingCount = gAutoRelic_pendingRelicIDs.size();
   for (int i = 0; i < pendingCount; i = i + 1)
   {
      gAutoRelic_pendingAge[i] = gAutoRelic_pendingAge[i] + 1;
   }

   int[] newPendingIDs    = default;
   vector[] newPendingPos = default;
   int[] newPendingAge    = default;
   int[] newPendingHero   = default;

   for (int i = 0; i < pendingCount; i = i + 1)
   {
      int relicID = gAutoRelic_pendingRelicIDs[i];
      vector pos  = gAutoRelic_pendingRelicPositions[i];
      int age     = gAutoRelic_pendingAge[i];
      int heroID  = gAutoRelic_pendingHeroIDs[i];

      if (age >= cAutoRelic_PendingMaxAge)
      {
         aiEcho("autoRelicDelivery: pending relic " + relicID + " aged out after "
                + age + " ticks");
         if (heroID >= 0)
         {
            aiEcho("autoRelicDelivery: marking pair (" + heroID + "," + relicID
                   + ") triggered after timeout");
            autoRelicDelivery_addPair(heroID, relicID);
         }
         continue;
      }

      // Retry: look for a carrier near the relic's last known position.
      int[] heroes = autoRelicDelivery_findCarrierInRange(pos);
      int carrier = -1;
      for (int h = 0; h < heroes.size(); h = h + 1)
      {
         if (autoRelicDelivery_heroCarriesRelic(heroes[h], relicID) == true)
         {
            carrier = heroes[h];
            break;
         }
      }

      if (carrier < 0)
      {
         // Carrier is no longer in range or has dropped the relic.
         aiEcho("autoRelicDelivery: pending relic " + relicID + " carrier lost, dropping");
         continue;
      }

      if (autoRelicDelivery_heroIsDeliverable(carrier) == true)
      {
         autoRelicDelivery_issueDelivery(carrier, relicID);
         continue; // resolved; do not re-add to pending
      }

      // Still carrying but still not deliverable; keep pending.
      newPendingIDs.add(relicID);
      newPendingPos.add(pos);
      newPendingAge.add(age);
      newPendingHero.add(carrier);
      aiEcho("autoRelicDelivery: pending relic " + relicID + " still not deliverable (hero "
             + carrier + ")");
   }

   //--------------------------------------------------------------------------
   // Process fresh disappearances.
   //--------------------------------------------------------------------------
   for (int i = 0; i < disappearedIDs.size(); i = i + 1)
   {
      int relicID = disappearedIDs[i];
      vector pos  = disappearedPositions[i];

      // Skip if this relic is already pending from an earlier tick.
      bool alreadyPending = false;
      for (int p = 0; p < newPendingIDs.size(); p = p + 1)
      {
         if (newPendingIDs[p] == relicID)
         {
            alreadyPending = true;
            break;
         }
      }
      if (alreadyPending == true) { continue; }

      int[] heroes = autoRelicDelivery_findCarrierInRange(pos);
      aiEcho("autoRelicDelivery: candidate heroes within 10m: " + heroes.size());

      bool handled = false;
      for (int h = 0; h < heroes.size(); h = h + 1)
      {
         int heroID = heroes[h];
         if (autoRelicDelivery_heroCarriesRelic(heroID, relicID) == true)
         {
            if (autoRelicDelivery_heroIsDeliverable(heroID) == true)
            {
               autoRelicDelivery_issueDelivery(heroID, relicID);
            }
            else
            {
               int action = kbUnitGetActionType(heroID);
               int planID = kbUnitGetPlanID(heroID);

               if (action != cActionTypeIdle)
               {
                  aiEcho("autoRelicDelivery: hero " + heroID
                         + " carrying target relic but not idle (action=" + action + ") -> skip");
               }
               else if (planID != cInvalidID)
               {
                  aiEcho("autoRelicDelivery: hero " + heroID
                         + " carrying target relic but has active plan " + planID + " -> skip");
               }

               newPendingIDs.add(relicID);
               newPendingPos.add(pos);
               newPendingAge.add(0);
               newPendingHero.add(heroID);
            }
            handled = true;
            break; // first matching carrier wins
         }
         else if (kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) > 0)
         {
            aiEcho("autoRelicDelivery: hero " + heroID + " carrying different relic -> skip");
         }
         else
         {
            aiEcho("autoRelicDelivery: hero " + heroID + " not carrying any relic -> skip");
         }
      }

      // No hero in range: this is either a non-hero pickup or a scenario-script
      // removal. Nothing to deliver and no pair to track.
      if (handled == false && heroes.size() > 0)
      {
         // All nearby heroes were inspected and none carry this relic.
      }
   }

   // Commit updated pending state.
   gAutoRelic_pendingRelicIDs.clear();
   gAutoRelic_pendingRelicPositions.clear();
   gAutoRelic_pendingAge.clear();
   gAutoRelic_pendingHeroIDs.clear();
   for (int i = 0; i < newPendingIDs.size(); i = i + 1)
   {
      gAutoRelic_pendingRelicIDs.add(newPendingIDs[i]);
      gAutoRelic_pendingRelicPositions.add(newPendingPos[i]);
      gAutoRelic_pendingAge.add(newPendingAge[i]);
      gAutoRelic_pendingHeroIDs.add(newPendingHero[i]);
   }

   // Save current snapshot for the next tick.
   gAutoRelic_prevRelicIDs.clear();
   gAutoRelic_prevRelicPositions.clear();
   for (int i = 0; i < curRelicIDs.size(); i = i + 1)
   {
      gAutoRelic_prevRelicIDs.add(curRelicIDs[i]);
      gAutoRelic_prevRelicPositions.add(curRelicPositions[i]);
   }

   xsSetContextPlayer(-1);
}
