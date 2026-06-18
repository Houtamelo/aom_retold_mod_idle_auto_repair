# Proposal: Auto-relic-delivery (event-driven)

## Intent

Add a passive human-assist feature to the **Human Assist Improvements** mod: when a human player's hero picks up a relic, the hero is automatically tasked once to deliver it to the nearest player-owned temple with available space. The order is issued at most once per `(hero unit ID, relic unit ID)` pair, so subsequent player orders always win.

## Scope

### In Scope

- New `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` with the event handler, idle scan, trigger-once tracker, nearest-temple lookup, and delivery command.
- One include line and a `autoRelicDelivery_register()` call in `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` at the end of `main()` (`human_assist.xs:948-994`).
- One deploy line in `scripts/deploy-mods.sh` for `auto_relic_delivery.xs` from `$HUMAN_SRC/`.
- Update `mod/human_assist_improvements/README.md` to document the new feature and the singleton-handler caveat.
- Briefly update top-level `README.md` to list the feature.

### Out of Scope

- Non-hero carriers (Pioneers, Pharaohs, transport ships) — covered by verification questions, not initial implementation.
- A continuous global hero poll (rejected per the original performance constraint; only event-triggered scans are used).
- Editing `auto_repair.xs` or `auto_scout.xs`. The feature is standalone.
- Changes to the three existing standalone/combined mods.

## Capabilities

### New Capabilities

- `auto-relic-delivery`: Passive one-shot relic delivery by heroes for human players.

### Modified Capabilities

- None.

## Approach

Locked Approach B: event-driven via `cXSRelicPickedUpHandler` (`core/setup.xs:180-181`).

1. **Registration:** `autoRelicDelivery_register()` is called from the end of `human_assist.xs::main()`, after the scout registration line inside `enableAutoScouting` is unchanged. It initializes the tracker arrays and registers the handler.
2. **Handler body:** The handler receives only a `techID` (`core/exploration.xs:1989`, `:2002`), so it scans idle heroes via `kbUnitQueryCreate(... cUnitTypeHero ... cActionTypeIdle)` (mirroring `auto_repair.xs:42`, `:251`). For each idle hero with `kbUnitGetNumberContained(heroID) > 0` (carrying signal, `core/utilities/unit_queries.xs:534`), it checks whether the `(heroID, relicID)` pair is already in the tracker.
3. **Delivery:** For a new pair, it finds the nearest player-owned temple where `kbUnitGetNumberContained(templeID) < kbPlayerGetProtoStatInt(cMyID, cUnitTypeTemple, cProtoStatMaxContained)` (`core/utilities/unit_queries.xs:529-538`) and issues `aiTaskWorkUnit(heroID, templeID)` (`core/exploration.xs:2028`).
4. **Trigger-once tracker:** Two parallel append-only arrays `gAutoRelic_heroID[]` and `gAutoRelic_relicID[]` in the style of `auto_scout.xs:2952-2998`.

**Idle-vs-event timing resolution:** The handler immediately scans idle heroes and issues delivery orders. Because the event may fire during the pickup animation, the handler also arms a short one-shot retry (1–2 seconds later) that runs only after a pickup event to catch the hero once the animation settles. This avoids a continuous poll while honoring the "no fighting player orders" constraint.

## Affected Areas

| Area | Impact | Description |
|---|---|---|
| `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs` | New | Feature logic, handler, tracker, temple lookup. |
| `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` | Modified | Add include and `autoRelicDelivery_register()` in `main()`. Maintainer comments documenting the divergence from the combined mod are now appropriate. |
| `scripts/deploy-mods.sh` | Modified | Add `$HUMAN_SRC/auto_relic_delivery.xs` deploy line. |
| `mod/human_assist_improvements/README.md` | Modified | Feature description + singleton-handler caveat. |
| `README.md` | Modified | Add feature to the mod list. |

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| Singleton `cXSRelicPickedUpHandler` conflicts with future features or other mods | Med | Document in-mod entry point; future human-assist features chain through our handler instead of registering their own. |
| `kbUnitGetNumberContained(heroID) > 0` may not reliably mean "carrying a relic" | High | Manual `aiEcho` verification scenario. |
| `cUnitTypeHero` may miss culture-specific carriers (Pioneer, Pharaoh, etc.) | Med | Manual verification; extend per vanilla special cases if needed. |
| `human_assist.xs` diverges from combined mod (D1 byte-identity breaks) | Expected | Acknowledged at bootstrap; patch-rebase notes maintained in README. |
| Handler behavior differs from source reading | Med | Fallback to a low-frequency tick-rule scan is possible without an architectural rewrite. |

## Rollback Plan

1. Remove `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs`.
2. Revert the `include "human_assist/auto_relic_delivery.xs";` line and `autoRelicDelivery_register();` call in `human_assist.xs`. This restores byte-identity with the combined mod's `human_assist.xs`.
3. Revert the new deploy line in `scripts/deploy-mods.sh`.
4. Revert README updates.

No changes to `auto_repair.xs` or `auto_scout.xs` are needed.

## Dependencies

- None.

## Success Criteria

- [ ] Pickup event triggers delivery to the nearest non-full temple exactly once per `(hero, relic)` pair.
- [ ] Player-issued orders after pickup are not overridden.
- [ ] AI players are unaffected (`kbPlayerIsHuman(cMyID)` guard).
- [ ] Deploy script copies the new file to `Human Assist Improvements/game/ai/human_assist/`.
- [ ] Manual in-game checklist resolves open verification questions.

## Open Verification Questions Carried Forward

These become in-game verification scenarios in the spec:

1. Does `kbUnitGetNumberContained(heroID) > 0` reliably mean the hero is carrying a relic? (`exploration.md:190`)
2. What payload does `cXSRelicPickedUpHandler` actually provide? Can `techID` map to the hero/relic, or does every event require a hero scan? (`exploration.md:191`)
3. Does `cUnitTypeHero` cover all human-player relic carriers, or do Pioneer/Pharaoh branches from `core/exploration.xs:1907-1923` need to be added? (`exploration.md:192`)
4. What is the relic lifecycle after a player override? If a hero is carrying a relic and receives a new order, does the relic remain contained until deposited/dropped, and does it become a new pickup event if dropped and re-picked? (`exploration.md:193`)
