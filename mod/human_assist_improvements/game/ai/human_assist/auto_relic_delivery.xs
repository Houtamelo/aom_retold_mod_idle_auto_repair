//==============================================================================
// auto_relic_delivery.xs (Human Assist Improvements mod)
//
// Passive one-shot relic delivery for human players.
//
// Algorithm:
//   Every 2 seconds, snapshot all ground relics via a player-0 query
//   (cUnitTypeRelic, cUnitStateAlive) and diff against the previous tick.
//   For each relic that was on the ground last tick but is not this tick:
//     1. Query all player-owned alive units within cAutoRelic_ProximityRadius
//        meters of the relic's last-seen position, filtering on cUnitTypeUnit
//        (the broadest "any concrete unit" tag, present on every hero type
//        including Miko, Atlantean villager-heroes, Major/Minor/Villager
//        subtypes, Norse/Egyptian/Greek/Chinese heroes, plus non-hero units
//        that the per-unit carrying check filters out cheaply). Worst case
//        ~40 units in a 10 m radius; per-unit cost is one API call.
//     2. For each candidate, check if the unit carries that SPECIFIC relic
//        unit ID (via kbUnitGetNumberContainedOfType > 0 then
//        kbUnitGetContainedUnitByIndex). Non-heroes fail this check
//        immediately.
//     3. If carrying, idle (cActionTypeIdle), and plan-free (-1, the
//        "no plan" return value of kbUnitGetPlanID),
//        issue aiTaskWorkUnit(hero, nearestTempleWithSpace).
//
// No state is retained across ticks except the diff snapshot. Each tick is
// independent. Stale carries or player overrides are not retried.
//
// The three cases for "missing relic" are exhaustive:
//   - Picked up by another player: hero query filters by player ID, no
//     candidate found, no action.
//   - Picked up by our hero but hero is busy: idle check rejects, no action.
//     Player commands take precedence.
//   - Picked up by our hero, hero is idle: deliver.
//
// New ground relics that appear (temple destroyed, hero died and dropped) are
// tracked in the snapshot but not acted on; if they later disappear the
// missing-relic path handles them.
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

// Hero proximity radius, in meters.
const float cAutoRelic_ProximityRadius = 10.0;

//==============================================================================
// Queries and snapshot
//==============================================================================

int gAutoRelic_relicQuery    = -1; // alive ground relics (player 0 / gaia)
int gAutoRelic_unitQuery     = -1; // all alive player-owned units within
                                   // cAutoRelic_ProximityRadius of a
                                   // disappearance. Filtered by type Unit
                                   // (cUnitTypeUnit = 889, present on every
                                   // concrete unit). Per-unit carrying
                                   // check filters out non-heroes.
int gAutoRelic_templeQuery    = -1; // player-owned temples

// Previous-tick snapshot of ground relics, used for the disappearance diff.
extern int[]    gAutoRelic_prevRelicIDs       = default;
extern vector[] gAutoRelic_prevRelicPositions = default;

//==============================================================================
// Query setup
//==============================================================================

void autoRelicDelivery_setupRelicQuery()
{
   if (gAutoRelic_relicQuery != -1) { return; }
   gAutoRelic_relicQuery = kbUnitQueryCreate("autoRelic_relicsOnGround");
   // Ground relics are owned by gaia (player 0).
   kbUnitQuerySetPlayerID(gAutoRelic_relicQuery, 0, false);
   kbUnitQuerySetUnitType(gAutoRelic_relicQuery, cUnitTypeRelic);
   kbUnitQuerySetState(gAutoRelic_relicQuery, cUnitStateAlive);
}

void autoRelicDelivery_setupUnitQuery()
{
   if (gAutoRelic_unitQuery != -1) { return; }
   gAutoRelic_unitQuery = kbUnitQueryCreate("autoRelic_unitsInRange");
   kbUnitQuerySetPlayerID(gAutoRelic_unitQuery, cMyID, false);
   // cUnitTypeUnit (= 889) is the broadest "any concrete unit" tag in the
   // proto hierarchy. Every unit we care about (Miko, Ajax, Mikos, all the
   // Major/Minor/Villager hero subtypes, Norse/Egyptian/Greek/Chinese
   // heroes, plus non-hero units that the carrying check filters cheaply)
   // carries <unittype>Unit</unittype>. The per-unit carrying check in
   // handleDisappearance rejects non-carriers in one cheap API call.
   kbUnitQuerySetUnitType(gAutoRelic_unitQuery, cUnitTypeUnit);
   kbUnitQuerySetState(gAutoRelic_unitQuery, cUnitStateAlive);
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
// Per-disappearance helpers
//==============================================================================

// Returns the IDs of all player-owned alive units within
// cAutoRelic_ProximityRadius meters of the given position. Filters by
// cUnitTypeUnit (broadest "any unit" tag — Miko, all hero types, plus
// non-hero units). The per-unit carrying check in handleDisappearance
// rejects non-heroes in one cheap API call, so the extra candidates cost
// almost nothing.
int[] autoRelicDelivery_findUnitsInRange(vector pos = cInvalidVector)
{
   autoRelicDelivery_setupUnitQuery();

   kbUnitQuerySetPosition(gAutoRelic_unitQuery, pos);
   kbUnitQuerySetMaximumDistance(gAutoRelic_unitQuery, cAutoRelic_ProximityRadius);
   kbUnitQueryResetResults(gAutoRelic_unitQuery);
   kbUnitQueryExecute(gAutoRelic_unitQuery);
   return(kbUnitQueryGetResults(gAutoRelic_unitQuery));
}

// True if heroID carries the specific relic unit ID. Slot 0 fast path with a
// defensive scan over all contained slots in case future hero types hold
// relics in higher slots.
bool autoRelicDelivery_heroCarriesRelic(int heroID = -1, int relicID = -1)
{
   if (heroID < 0 || relicID < 0) { return(false); }
   if (kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) <= 0) { return(false); }

   int slot0 = kbUnitGetContainedUnitByIndex(heroID, 0);
   if (slot0 == relicID && kbUnitIsType(slot0, cUnitTypeRelic) == true)
   {
      return(true);
   }

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

// True if the hero is idle and has no active AI plan. The plan check
// compares against -1 because that is the value kbUnitGetPlanID returns
// when the unit has no plan; the shipped AoM:R AI source uses -1
// literally everywhere (see core/economy/economic_units.xs:138,
// core/exploration.xs:655). The reference doc
// docs/MythRMConstants.txt:16 declares `cInvalidID = -1` but the
// constant is not actually exposed to the XS runtime.
bool autoRelicDelivery_heroIsDeliverable(int heroID = -1)
{
   if (heroID < 0) { return(false); }
   return(kbUnitGetActionType(heroID) == cActionTypeIdle
          && kbUnitGetPlanID(heroID) == -1);
}

// Returns the player-owned temple with available relic space nearest to the
// hero, or -1 if none has space.
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
// Handle one disappeared relic: scan nearby heroes and deliver to the first
// valid carrier.
//==============================================================================

void autoRelicDelivery_handleDisappearance(int relicID = -1, vector lastPos = cInvalidVector)
{
   int[] candidates = autoRelicDelivery_findUnitsInRange(lastPos);
   aiEcho("autoRelicDelivery: candidate units within 10m: " + candidates.size());

   for (int h = 0; h < candidates.size(); h = h + 1)
   {
      int heroID = candidates[h];
      if (autoRelicDelivery_heroCarriesRelic(heroID, relicID) == false) { continue; }
      if (autoRelicDelivery_heroIsDeliverable(heroID) == false) { continue; }

      int templeID = autoRelicDelivery_findNearestTempleWithSpace(heroID);
      if (templeID < 0) { continue; }

      aiEcho("autoRelicDelivery: hero " + heroID + " carrying target relic -> delivering to temple " + templeID);
      aiTaskWorkUnit(heroID, templeID);
      return;
   }
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

   // Snapshot current ground relics.
   kbUnitQueryResetResults(gAutoRelic_relicQuery);
   int relicCount = kbUnitQueryExecute(gAutoRelic_relicQuery);

   // Local snapshot of current ground relics. Use `new int(0, 0)` /
   // `new vector(0, cInvalidVector)` for local empty arrays; the
   // shipped AoM:R XS runtime only accepts `= default` for `extern`
   // globals, `static` class members, and parameter defaults — NOT
   // for local variables inside function bodies (see
   // core/exploration.xs for the canonical `new int(0, 0)` pattern).
   int[]    curRelicIDs       = new int(0, 0);
   vector[] curRelicPositions = new vector(0, cInvalidVector);
   for (int i = 0; i < relicCount; i = i + 1)
   {
      int relicID = kbUnitQueryGetResult(gAutoRelic_relicQuery, i);
      curRelicIDs.add(relicID);
      curRelicPositions.add(kbUnitGetPosition(relicID));
   }

   aiEcho("autoRelicDelivery: tick start (relicsOnGround=" + relicCount + ")");

   // Find relics that disappeared since last tick and handle each one.
   int prevCount = gAutoRelic_prevRelicIDs.size();
   for (int i = 0; i < prevCount; i = i + 1)
   {
      int prevID = gAutoRelic_prevRelicIDs[i];
      bool found = false;
      for (int j = 0; j < relicCount; j = j + 1)
      {
         if (curRelicIDs[j] == prevID) { found = true; break; }
      }
      if (found == true) { continue; }

      vector prevPos = gAutoRelic_prevRelicPositions[i];
      aiEcho("autoRelicDelivery: disappearance detected (relicID=" + prevID
             + ", pos=(" + prevPos.x + "," + prevPos.z + "))");
      autoRelicDelivery_handleDisappearance(prevID, prevPos);
   }

   // Save current snapshot for next tick.
   gAutoRelic_prevRelicIDs.clear();
   gAutoRelic_prevRelicPositions.clear();
   for (int i = 0; i < curRelicIDs.size(); i = i + 1)
   {
      gAutoRelic_prevRelicIDs.add(curRelicIDs[i]);
      gAutoRelic_prevRelicPositions.add(curRelicPositions[i]);
   }

   xsSetContextPlayer(-1);
}