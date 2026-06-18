# Design: auto-relic-delivery (event-driven)

## 1. Architecture overview

New mod tree for `mod/human_assist_improvements/game/ai/human_assist/`:

```text
├── human_assist.xs            (modify: add include + registration call)
├── auto_repair.xs             (deploy-time copy from idle_auto_repair; unchanged)
├── auto_scout.xs              (deploy-time copy from intelligent_auto_scout; unchanged)
└── auto_relic_delivery.xs     (new: feature logic)
```

`auto_repair.xs` and `auto_scout.xs` remain shared sources of truth; this change does not touch them.

## 2. `auto_relic_delivery.xs` module anatomy

Mirrors `auto_repair.xs` layout:

- **Header constants** (`cAutoRelicDelivery_*`) for max tracker size and retry timing.
- **Globals** — parallel append-only tracker arrays and a retry flag:
  ```xs
  extern int[] gAutoRelic_heroID  = default;
  extern int[] gAutoRelic_relicID = default;
  extern bool  gAutoRelic_retryPending = false;
  ```
  Tracker pattern follows `auto_scout.xs:383` (`gAutoScout_redirectedHerdIDs`) and `auto_scout.xs:1389-1398` (linear scan). No per-hero state array is needed because the pair tracker itself records which deliveries have already fired.
- **`autoRelicDelivery_register()`** — called from `human_assist.xs::main()`; early-returns if `kbPlayerIsHuman(cMyID)` is false; initializes the shared hero/temple queries; registers `aiSetHandler("autoRelicDelivery_onPickedUp", cXSRelicPickedUpHandler)` (`core/setup.xs:181`).
- **Callback `autoRelicDelivery_onPickedUp(int techID)`** — scans alive heroes (`cUnitTypeHero`, keeping the query broad so the retry can see carrying-but-busy heroes). For each hero with `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic) > 0` it reads the carried relic ID via `kbUnitGetContainedUnitByIndex(heroID, 0)`, defensively correlates it with the event payload via `kbRelicGetTechID(relicID)`, checks the `(heroID, relicID)` tracker, and — if the hero is idle and the pair is new — finds the nearest temple with space and issues `aiTaskWorkUnit(heroID, templeID)` (`core/exploration.xs:2028`). It also arms the one-shot retry.
- **Retry rule** — `minInterval 2` rule that runs only while `gAutoRelic_retryPending` is true. It clears the flag and reruns the same scan once. This catches heroes still in pickup animation when the event fires, without continuous polling.
- **Nearest-temple helper** — query `cUnitTypeTemple`, `cMyID`, `cUnitStateAlive`, sorted ascending by distance from hero; pick first where `kbUnitGetNumberContained(templeID) < kbPlayerGetProtoStatInt(cMyID, cUnitTypeTemple, cProtoStatMaxContained)` (`core/utilities/unit_queries.xs:529-538`).

## 3. Trigger-once model

The feature is driven by `(heroID, relicID)` pair tracking rather than a per-hero state machine:

```text
pickup event
    |
    v
scan alive heroes
    |
    +-- hero not carrying a relic (kbUnitGetNumberContainedOfType == 0) --> skip
    |
    +-- read relic ID = kbUnitGetContainedUnitByIndex(heroID, 0)
        |
        +-- techID mismatch (kbRelicGetTechID(relicID) != eventTechID) --> skip
        |
        +-- pair already tracked --> skip
        |
        +-- hero busy (pickup animation or player override)
            |
            +-- immediate event scan --> arm one-shot retry
            |
            +-- retry scan --> mark pair tracked and skip permanently
        |
        +-- hero idle --> aiTaskWorkUnit(heroID, templeID), track pair
```

- Immediate scan runs in the handler; retry scans once 1–2 s later only if the first scan found a carrying-but-busy hero.
- The per-hero state machine (`NO_RELIC`/`AUTO_DELIVERING`/`PLAYER_OVERRIDE`) used in the prior implementation is **not** used; the `(heroID, relicID)` pair tracker is the active runtime path.

## 4. Decisions with rationale

| ID | Decision | Rationale |
|---|---|---|
| D1 | Register from `human_assist.xs::main()` at the passive-init point, not from `enableAutoScouting` | Matches `auto_repair.xs` passive-feature pattern; relic delivery is global, not per-scout UI |
| D2 | Event-driven scan on `cXSRelicPickedUpHandler` | Rare events keep per-tick cost ~0; `techID`-only payload still permits a focused idle-hero scan |
| D3 | Parallel arrays `gAutoRelic_heroID[]` / `gAutoRelic_relicID[]` for trigger-once | Follows `auto_scout.xs` pattern; XS has no native map/dictionary |
| D4 | Immediate scan + one-shot retry (1–2 s) | Handles pickup-animation edge case without fighting later player orders; retry re-tests idle before issuing |
| D5 | Singleton-handler caveat in README; chain future handlers through this module if needed | `cXSRelicPickedUpHandler` is exclusive; documentation mitigates conflict |
| D6 | One-shot retry remains flag-gated only | The retry is bounded by clearing `gAutoRelic_retryPending`; a separate counter was removed to eliminate dead state |
| D7 | Use `kbUnitGetContainedUnitByIndex(heroID, 0)` to obtain the relic unit ID at runtime | Replaces the prior per-hero fallback that assumed no carried-relic API existed (exploration error corrected by re-investigation) |
| D8 | Use `kbRelicGetTechID(relicID)` for defensive correlation against the event `techID` payload | Confirms the scanned relic matches the pickup event before issuing a delivery; skipped on retry because rapid events can overwrite the single retry slot |
| D9 | Use `kbUnitGetNumberContainedOfType(heroID, cUnitTypeRelic)` for type-safe carrying check | More precise than `kbUnitGetNumberContained(heroID)`, which would count any contained unit, not just relics |

## 5. Deploy-script delta decision

`scripts/deploy-mods.sh:87-90` lists individual files for the 4th mod. Therefore a new line is required:

```bash
deploy "$HUMAN_SRC/auto_relic_delivery.xs"  "$DEPLOY_ROOT/Human Assist Improvements/game/ai/human_assist/auto_relic_delivery.xs"
```

## 6. Shared-file impact

`auto_repair.xs` and `auto_scout.xs` are not touched. They continue to deploy from their canonical sibling directories. The shared-file source-of-truth invariant is preserved.

## 7. D1 byte-identity broken (intentional)

The 4th mod's `human_assist.xs` was byte-identical to the combined mod after bootstrap. This change intentionally breaks that by adding the include and registration call. Add inline maintainer comments at the include site and registration site noting the divergence and pointing to the combined-mod version as the upstream baseline for patch rebases.

## 8. Patch-maintenance / rollback

- **Rebase procedure:** when vanilla patches `human_assist.xs`, start from the patched vanilla file, re-add the three feature includes (`auto_repair.xs`, `auto_scout.xs`, `auto_relic_delivery.xs`) and their registration/hook calls.
- **Rollback:** delete `auto_relic_delivery.xs`, revert the two edits in the 4th mod's `human_assist.xs`, remove the new deploy line, and revert README changes.

## 9. Future-extensibility hook

The next feature follows the same pattern: add `auto_<feature>.xs` in `mod/human_assist_improvements/game/ai/human_assist/`, add one include, and add one registration call in `human_assist.xs::main()`. If another feature needs `cXSRelicPickedUpHandler`, it should chain through `autoRelicDelivery_onPickedUp` (add a hook function later if needed).
