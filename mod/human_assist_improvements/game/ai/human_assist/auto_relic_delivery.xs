//==============================================================================
// auto_relic_delivery.xs (Human Assist Improvements mod)
//
// Passive one-shot relic delivery for human players. When a hero picks up a
// relic, it is automatically tasked once to the nearest player-owned temple
// that still has relic space. Player commands always win: delivery only fires
// while the hero is idle, and a single one-shot retry catches the pickup-
// animation edge case without continuous polling.
//
// Prior versions fell back to a per-hero state machine because it was believed
// AoM:R exposed no way to obtain the relic unit ID carried by a hero. A re-
// investigation of the engine KB docs revealed it does:
//     kbUnitGetContainedUnitByIndex(heroID, 0) -> relic unit ID
//     kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) -> carrying check
//     kbRelicGetTechID(relicID) -> techID payload from cXSRelicPickedUpHandler
// This implementation therefore uses the ideal (heroID, relicID) pair tracker
// as the active runtime path.
//
// Loaded from human_assist.xs via:
//     include "human_assist/auto_relic_delivery.xs";
//==============================================================================

// Maximum tracked (hero, relic) pairs. Hero counts are small (< 20 in a
// typical match); 64 leaves comfortable headroom for scenario-editor spam.
const int cAutoRelic_MaxTrackerPairs = 64;

// One-shot retry cadence, in seconds. Fires only after a pickup event that
// could not immediately deliver because the carrying hero was still busy.
const int cAutoRelic_RetryInterval = 2;

// Lazy-created shared queries.
int gAutoRelic_heroQuery = -1;   // alive heroes (kept broad for retry visibility)
int gAutoRelic_templeQuery = -1; // player-owned temples near a hero

// Trigger-once tracker: parallel arrays indexed by slot. A pair is appended
// the first time a delivery order is issued for that (heroID, relicID), so any
// subsequent event or retry for the same pair is silently skipped.
extern int[] gAutoRelic_heroID  = default;
extern int[] gAutoRelic_relicID = default;

// One-shot retry state, armed when a pickup event fires while every carrying
// hero is still in the pickup animation (or has been manually ordered).
extern bool gAutoRelic_retryPending  = false;

//==============================================================================
// Query setup
//==============================================================================

void autoRelicDelivery_setupHeroQuery()
{
   if (gAutoRelic_heroQuery != -1) { return; }
   gAutoRelic_heroQuery = kbUnitQueryCreate("autoRelic_aliveHeroes");
   kbUnitQuerySetPlayerID(gAutoRelic_heroQuery, cMyID, false);
   kbUnitQuerySetUnitType(gAutoRelic_heroQuery, cUnitTypeHero);
   kbUnitQuerySetState(gAutoRelic_heroQuery, cUnitStateAlive);
   // Note: we do NOT set cActionTypeIdle on the query. The retry logic must
   // see carrying-but-busy heroes (pickup animation or player override) so it
   // can either wait or mark the pair triggered. Delivery itself still only
   // fires when kbUnitGetActionType(heroID) == cActionTypeIdle.
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

   aiEcho("autoRelicDelivery: no temple with space for hero " + heroID);
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
      // Per spec requirement 4: no-op if no temple has space. The pair is NOT
      // recorded, so a future event (or retry after a temple is built) can
      // still fire for this (hero, relic).
      return;
   }

   aiEcho("autoRelicDelivery: delivering hero " + heroID + " -> temple " + templeID
      + " (relic " + relicID + ")");
   aiTaskWorkUnit(heroID, templeID);
   autoRelicDelivery_addPair(heroID, relicID);
}

//==============================================================================
// Core scan: find idle carrying heroes and deliver, with retry fallback
//==============================================================================

void autoRelicDelivery_scanAndDeliver(bool isRetry = false, int eventTechID = -1)
{
   // Guard all paths for human players only.
   if (kbPlayerIsHuman(cMyID) == false) { return; }

   autoRelicDelivery_setupHeroQuery();
   kbUnitQueryResetResults(gAutoRelic_heroQuery);

   // Query every alive hero so we can detect carrying-but-busy heroes
   // (pickup animation or player override) for the one-shot retry.
   int heroCount = kbUnitQueryExecute(gAutoRelic_heroQuery);
   bool carryingButBusy = false;

   for (int i = 0; i < heroCount; i = i + 1)
   {
      int heroID = kbUnitQueryGetResult(gAutoRelic_heroQuery, i);
      if (heroID < 0) { continue; }

      // Type-safe carrying check: only count contained units of type Relic.
      if (kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) <= 0)
      {
         continue;
      }

      // Retrieve the actual relic unit ID carried by this hero.
      int relicID = kbUnitGetContainedUnitByIndex(heroID, 0);
      if (relicID < 0)
      {
         aiEcho("autoRelicDelivery: hero " + heroID + " reports a relic but kbUnitGetContainedUnitByIndex returned " + relicID);
         continue;
      }

      // Defensive correlation against the event payload. If this carrier does
      // not match the pickup event's techID, skip it and let a later event
      // handle it. Correlation is skipped on retry (eventTechID == -1) because
      // multiple rapid-fire events can overwrite the single retry slot.
      if (eventTechID >= 0 && kbRelicGetTechID(relicID) != eventTechID)
      {
         aiEcho("autoRelicDelivery: techID mismatch for hero " + heroID + " relic " + relicID
            + " (eventTechID=" + eventTechID + ", relicTechID=" + kbRelicGetTechID(relicID)
            + "), skipping");
         continue;
      }

      // One-shot guard: never issue a second delivery order for the same pair.
      if (autoRelicDelivery_hasPair(heroID, relicID) == true)
      {
         continue;
      }

      int action = kbUnitGetActionType(heroID);
      if (action != cActionTypeIdle)
      {
         // Carrying but busy. During the immediate event scan this is usually
         // the pickup animation, so arm the one-shot retry. During the retry
         // scan, treat it as a player override and mark the pair triggered
         // so the feature does not fight the player's order later.
         if (isRetry == false)
         {
            carryingButBusy = true;
         }
         else
         {
            aiEcho("autoRelicDelivery: player override detected for hero " + heroID + " relic " + relicID);
            autoRelicDelivery_addPair(heroID, relicID);
         }
         continue;
      }

      // Idle, carrying, and not yet handled: issue one delivery order.
      autoRelicDelivery_issueDelivery(heroID, relicID);
   }

   // Arm one-shot retry if an event fired while a carrying hero was busy.
   if (isRetry == false && carryingButBusy == true)
   {
      gAutoRelic_retryPending = true;
      xsEnableRule("autoRelicDelivery_tickRetry");
      aiEcho("autoRelicDelivery: pickup event while hero busy, arming retry");
   }
}

//==============================================================================
// XS event handler for cXSRelicPickedUpHandler
//==============================================================================

void autoRelicDelivery_onPickedUp(int techID = -1)
{
   if (kbPlayerIsHuman(cMyID) == false) { return; }

   aiEcho("autoRelicDelivery: relic picked up event (techID=" + techID + ")");
   xsSetContextPlayer(cMyID);
   autoRelicDelivery_scanAndDeliver(false, techID);
   xsSetContextPlayer(-1);
}

//==============================================================================
// Registration (called once from human_assist.xs::main())
//==============================================================================

void autoRelicDelivery_register()
{
   // AI players keep vanilla behaviour; this is a human-assist mod feature.
   if (kbPlayerIsHuman(cMyID) == false) { return; }

   aiEcho("autoRelicDelivery: registering handler");

   // Arrays declared with = default start at zero length. The tracker resets
   // each game because globals are reloaded with the script.
   gAutoRelic_retryPending = false;

   // cXSRelicPickedUpHandler is exclusive: only one handler function may be
   // registered for this event type. Future human-assist features that need it
   // should extend autoRelicDelivery_onPickedUp rather than call aiSetHandler
   // again. This caveat is documented in README.md.
   aiSetHandler("autoRelicDelivery_onPickedUp", cXSRelicPickedUpHandler);
}

//==============================================================================
// One-shot retry rule
//==============================================================================

rule autoRelicDelivery_tickRetry
minInterval 2
inactive
{
   if (gAutoRelic_retryPending == false)
   {
      xsDisableRule("autoRelicDelivery_tickRetry");
      return;
   }

   xsSetContextPlayer(cMyID);
   aiEcho("autoRelicDelivery: retry scan");
   autoRelicDelivery_scanAndDeliver(true, -1);

   gAutoRelic_retryPending = false;
   xsSetContextPlayer(-1);
   xsDisableRule("autoRelicDelivery_tickRetry");
}
