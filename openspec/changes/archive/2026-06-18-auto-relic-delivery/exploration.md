# Exploration: Automatic hero relic delivery

## Change id

auto-relic-delivery

## Status

- [x] Phase: explore
- [ ] Next: propose

## Goal

Evaluate how to implement a passive human-assist feature that, whenever a hero picks up a relic, issues **one** delivery order to the nearest player-owned temple that has available relic space. The feature must:

- trigger at most once per `(hero unit ID, relic unit ID)` pair,
- skip heroes that are not idle,
- run cheaply even when the player has many heroes,
- fit the existing feature-file pattern used by `auto_repair.xs` and the bootstrap extensibility plan for the 4th mod.

## Method

1. Read project conventions and the bootstrap design/exploration artifacts.
2. Read `auto_repair.xs` as the closest passive analog and `auto_scout.xs` for query/tracker patterns.
3. Read the shipped AoM:R AI source under `~/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/`:
   - `core/exploration.xs` — vanilla relic collection logic.
   - `core/utilities/unit_queries.xs` — `getSuitableTempleIDForRelic` and query utilities.
   - `core/setup.xs` / `core/main.xs` — XS handler registration and init order.
   - `human_assist/human_assist.xs` and `human_assist/human_assist_unit_queries.xs` — what helpers the VPS overlay already has.
4. Search the vanilla tree for constants/functions related to relics, heroes, temples, events, inventory, and plan types.
5. Compare architectures A–F on CPU cost, scaling, complexity, and engine support.

## Findings

### 1. AoM:R engine API surface

| Capability | Available API | Source proof |
|------------|---------------|--------------|
| **Detect carrying a relic** | `kbUnitGetNumberContained(unitID)` appears to be the engine primitive: it returns how many units are garrisoned/contained inside a unit. Vanilla uses it for temples (`kbUnitGetNumberContained(templeID) >= kbPlayerGetProtoStatInt(..., cProtoStatMaxContained)`), and a hero carrying a relic is the symmetric case. | `core/exploration.xs:1876`, `core/utilities/unit_queries.xs:534` |
| **Temple stored-relic count / max** | `kbUnitGetNumberContained(templeID)` vs `kbPlayerGetProtoStatInt(cMyID, cUnitTypeTemple, cProtoStatMaxContained)`. | `core/utilities/unit_queries.xs:529-538` |
| **Idle detection** | `kbUnitQuerySetActionType(queryID, cActionTypeIdle)` filters the query pool; `kbUnitGetActionType(unitID) == cActionTypeIdle` checks a single unit. | `auto_repair.xs:42`, `auto_repair.xs:251` |
| **Issue delivery order** | `aiTaskWorkUnit(heroID, templeID)` is what vanilla uses to deposit a relic at a temple. | `core/exploration.xs:2028` |
| **Pickup order (if needed)** | `aiTaskWorkUnit(heroID, relicID)`. | `core/exploration.xs:1845` |
| **Relic-pickup event** | `aiSetHandler("autoRelicDelivery_onPickedUp", cXSRelicPickedUpHandler)` is valid and consistent with the existing `onAutoPlanCreate` handler already set by `human_assist.xs`. | `core/setup.xs:180-181`, `human_assist.xs:957` |
| **Event payload** | The handler receives only an `int techID`; no hero/relic unit IDs are passed. | `core/exploration.xs:1989`, `core/exploration.xs:2002` |
| **Hero unit-type constant** | `cUnitTypeHero` is the constant vanilla relic collection uses for eligible carriers. | `core/exploration.xs:1906` |
| **Temple unit-type constant** | `cUnitTypeTemple`. | `core/utilities/unit_queries.xs:514` |
| **Cheap global "relic changed" signal** | **Not found** — no `kbPlayer*RelicCount`, no `cPlanRelic` plan type, no one-shot query filter for "units carrying a relic". | Search across `~/.steam/.../game/ai/` returned nothing |
| **Dedicated relic-deposit plan** | **Not found** — `cPlanReserve` can hold a hero but still requires an explicit `aiTaskWorkUnit` to deposit. | `core/exploration.xs:1974-2028` |

**Key takeaway:** There is no zero-query solution. The best empirical signal for “this hero is carrying a relic” is `kbUnitGetNumberContained(heroID) > 0`, but this needs an in-game smoke test (see Open questions).

### 2. Existing feature patterns — mirror `auto_repair.xs`

`auto_repair.xs` is the closer analog for a passive feature:

- File header + constants, then lazy-created shared query globals (`auto_repair.xs:30-66`).
- `kbPlayerIsHuman(cMyID)` guards at the entry points so AI players are untouched (`auto_repair.xs:111`, `auto_repair.xs:238`).
- A `minInterval 3` active rule (`auto_repair.xs:293-323`) scans a filtered pool (`cActionTypeIdle` + type filter) and iterates results.
- No registration call is needed because the rule is `active` from load.

The new feature should follow the same layout: constants, shared queries, a passive `minInterval` rule, and human-only guards. Because it may also register an XS event handler, it should expose one public function — e.g. `autoRelicDelivery_register()` — called from a **global init hook**, not from `enableAutoScouting` (which is UI-triggered per scout).

The right registration point is the end of `human_assist.xs::main()`, after `disableVillagerAssist()` (`human_assist_improvements/game/ai/human_assist/human_assist.xs:948-994`). This mirrors how the scout hooks into `enableAutoScouting` for per-unit UI events, but the relic feature is global/per-player, so it belongs in the one-time startup path.

### 3. Vanilla AI relic handling

`core/exploration.xs:1799-2031` already implements full relic collection for the civ AI:

- Picks a hero from the primary land defend plan.
- Queries relics with `useSimpleUnitQuery(cUnitTypeRelic, 0, cUnitStateAlive, ...)`.
- Creates a `cPlanReserve` and adds the hero.
- Uses `aiTaskMoveUnit(heroID, relicPosition)` then `aiTaskWorkUnit(heroID, gRelicID)` to pick up.
- On pickup, calls `getSuitableTempleIDForRelic(...)` and `aiTaskWorkUnit(heroID, templeID)` to deposit.
- Tracks whether it is walking to the relic vs. temple with a state flag.

This confirms the exact subset of engine primitives we need and shows that the hard part for the mod is **not** the command API but the **detection/tracking model** for a human player’s hero that the vanilla AI does not control.

### 4. Performance comparison — approaches A to F

| Approach | How it works | Per-tick cost | Scaling | Complexity | Robustness | Verdict |
|----------|--------------|---------------|---------|------------|------------|---------|
| **A — Naive per-hero polling** | Every N seconds query all player heroes (`cUnitTypeHero`, alive) and test `kbUnitGetNumberContained`. | 1 query + O(H) contained checks per interval (H = total heroes). | O(H). With dozens of heroes this is the case the user explicitly wants to avoid. | Low | High (no event handler conflicts) | **Reject** — matches the user’s “too expensive” concern. |
| **B — Event-driven** | Register `cXSRelicPickedUpHandler`. On event, scan (idle) heroes to find who just picked up. | O(1) when no event; O(idle heroes) or O(H) per event. | O(event frequency × scan size). Best standby cost. | Medium | Medium — handler payload only gives `techID`; must scan to identify carrier; one handler per type can conflict with future mods. | **Viable if carrier identification works**. |
| **C — Filter-pool polling** (idle heroes) | Query `cUnitTypeHero` + `cActionTypeIdle`; test contained count only on idle heroes. | 1 query + O(idle heroes) per interval. Engine does the idle filtering. | O(idle heroes) ≤ O(H), usually much smaller. | Low | High — no handler overrides, mirrors `auto_repair.xs`. | **Recommended baseline**. |
| **D — Cooperative shared query** | Reuse an existing mod query (`auto_repair` villagers or `auto_scout` scouts) and post-filter by hero + contained. | Reuses query, but post-filter still O(results). | No cheaper than C because none of the shared queries target heroes. | Medium | Low — creates cross-feature coupling. | **Reject** — no suitable shared pool exists. |
| **E — Hybrid cheap signal + lazy scan** | Poll a cheap global “relic state changed” counter; only scan when it changes. | O(1) most ticks. | O(returned heroes) only on pickup ticks. | Low | High if signal exists. | **Infeasible** — no such global counter/plan was found. |
| **F — Plan-based** | Create a `cPlanReserve` per relic/hero and let the engine handle deposit. | 1 plan create + O(1) maintenance. | O(active relic deliveries). | Medium | Low — `cPlanReserve` still needs explicit `aiTaskWorkUnit`; adds plan bookkeeping for no clear gain. | **Reject** — no deposit-on-arrival plan type exists. |

**Cost summary in Big-O notation:**
- A: `O(H)` every interval.
- B: `O(1)` idle, `O(k)` per event where `k` is the number of heroes scanned (ideally idle heroes only).
- C: `O(I)` every interval, where `I` = idle heroes.
- D: same or worse than C.
- E: `O(1)` idle, but no known signal.
- F: plan overhead dominates; no engine support.

### 5. Trigger-once tracker design

XS has no maps/dictionaries; the standard pattern in this repo is parallel arrays:

- `auto_scout.xs` tracks per-scout state with `gAutoScout_unitID[]`, `gAutoScout_state[]`, etc. (`auto_scout.xs:210-218`).
- For one-shot marks it uses `gAutoScout_redirectedHerdIDs[]`, an append-only array linear-scanned to avoid re-issuing herd moves (`auto_scout.xs:2952-2998`).

**Recommended tracker:** two parallel append-only arrays `gAutoRelic_heroID[]` and `gAutoRelic_relicID[]`.

- On each candidate, linear-scan both arrays; if `(heroID, relicID)` exists, skip.
- Append the pair when an order is issued.
- Game relic counts are small (< 20 on most maps), so linear scan is fine.
- If the relic ID cannot be reliably obtained, fall back to a per-hero boolean tracker (weaker semantics) until the detection API is verified.

### 6. Idle / not-idle check

Mirror `auto_repair.xs` exactly:

1. Create the shared hero query with `kbUnitQuerySetActionType(gAutoRelic_heroQuery, cActionTypeIdle)` to let the engine pre-filter.
2. Before issuing a delivery, double-check `kbUnitGetActionType(heroID) == cActionTypeIdle`.
3. Also guard with `kbPlayerIsHuman(cMyID)`.

A hero that just picked up a relic may be in a non-idle action for a frame or two; the next poll will catch it. If event-driven B is used, the event handler can defer to the same idle check before issuing.

### 7. Nearest temple with available space

Use the same logic as `getSuitableTempleIDForRelic` in `core/utilities/unit_queries.xs:495-543`:

```xs
query = kbUnitQueryCreate("autoRelic_temples");
kbUnitQuerySetPlayerID(query, cMyID, false);
kbUnitQuerySetUnitType(query, cUnitTypeTemple);
kbUnitQuerySetState(query, cUnitStateAlive);
kbUnitQuerySetPosition(query, heroPos);
kbUnitQuerySetMaximumDistance(query, cMaxFloat);
kbUnitQuerySetAscendingSort(query, true);
```

Iterate results and pick the first temple where `kbUnitGetNumberContained(templeID) < kbPlayerGetProtoStatInt(cMyID, cUnitTypeTemple, cProtoStatMaxContained)`. This is the vanilla definition of “has available relic slots” and already accounts for the `< 5` cap.

### 8. Movement / deposit model

Deposit is automatic on arrival when the hero is tasked to the temple via `aiTaskWorkUnit(heroID, templeID)`. No separate “drop relic” command is needed. If the chosen temple becomes full between selection and arrival, the engine will fail to deposit; the next tick will pick a different temple (or do nothing if none exist).

### 9. Scope — heroes only

Use `cUnitTypeHero` as the query filter. This is the same constant the vanilla relic collector uses (`core/exploration.xs:1906`) and excludes the scout/Oracle pool used by `auto_scout.xs` (`cUnitTypeAbstractOracle`). If in-game testing reveals that Chinese Pioneers, Egyptian Pharaohs, or other sub-types are not covered by `cUnitTypeHero`, extend the query with culture-specific types exactly as vanilla does (`core/exploration.xs:1907-1923`).

### 10. Modular integration

Per the bootstrap design (`design.md:106-112`), the new feature lives in the 4th mod only:

- New file: `mod/human_assist_improvements/game/ai/human_assist/auto_relic_delivery.xs`.
- Add include next to the existing feature includes in `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs:14-15`:
  ```xs
  include "human_assist/auto_repair.xs";        // Idle Auto-Repair mod
  include "human_assist/auto_scout.xs";         // Intelligent Auto-Scout mod
  include "human_assist/auto_relic_delivery.xs"; // Auto-relic-delivery mod
  ```
- Add one registration call at the end of `human_assist.xs::main()` (e.g. after line 993):
  ```xs
  autoRelicDelivery_register();
  ```
- Add a deploy line in `scripts/deploy-mods.sh` so the file is copied from `$HUMAN_SRC/` (`scripts/deploy-mods.sh:87-90`). Existing repair/scout files are still copied from their canonical sibling directories; only the new file is native to the 4th mod.

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| `kbUnitGetNumberContained(heroID)` may not represent a carried relic | High (wrong feature behavior) | Verify with `aiEcho` before proposal; fall back to polling for relics near the hero if needed. |
| `cXSRelicPickedUpHandler` gives only `techID`; cannot identify carrier without a scan | Medium (approach B less valuable) | Document as open question; C still works regardless. |
| Only one handler per `cXSRelicPickedUpHandler` type can be registered | Medium (future mod/feature conflict) | Polling architecture C avoids this entirely. If B is used, guard registration and body with `kbPlayerIsHuman`. |
| `cUnitTypeHero` may miss culture-specific relic carriers | Medium | Extend query per vanilla special cases if testing shows gaps. |
| No XS test runner; all verification is manual | High (regressions slip through) | Provide explicit in-game checklist (see below). |
| Wrong hook placement in `human_assist.xs` could break VPS startup | Medium | Place the call at end of `main()`, after existing init, with early return if not human. |

## Manual verification checklist

- [ ] Deploy the 4th mod via `scripts/deploy-mods.sh` while AoM:R is closed.
- [ ] Confirm `mods/local/Human Assist Improvements/game/ai/human_assist/auto_relic_delivery.xs` exists.
- [ ] Start a match with a hero and a relic; pick up the relic with the hero.
- [ ] Observe `aiEcho` that a delivery order was issued once, to the nearest temple with space.
- [ ] Confirm the hero deposits the relic automatically on arrival.
- [ ] Issue a manual move/attack order to the hero immediately after pickup; confirm the mod does **not** re-issue a delivery order for that `(hero, relic)` pair.
- [ ] Pick up another relic with the same hero (if possible) and confirm it triggers for the new pair.
- [ ] Fill a temple to 5 relics and confirm the mod picks the next-nearest non-full temple.
- [ ] Spawn many idle heroes (e.g. via scenario editor) and verify no noticeable script stutter; `aiEcho` cost diagnostics if needed.

## Open questions

These should be resolved before/during the proposal phase:

1. **Does `kbUnitGetNumberContained(heroID) > 0` reliably mean the hero is carrying a relic?** If not, what is the cheapest reliable detection method?
2. **What payload does `cXSRelicPickedUpHandler` actually provide?** Can `techID` be mapped to the hero and/or relic, or will every event still require a hero scan?
3. **Does `cUnitTypeHero` cover all human-player relic carriers?** Are Chinese Pioneers, Egyptian Pharaohs, etc. included, or do we need the culture-specific branches from `core/exploration.xs:1907-1923`?
4. **What is the relic lifecycle after a player override?** If a hero is carrying a relic and receives a new order, does the remain “contained” until deposited/dropped, and does it become a new pickup event if dropped and re-picked?

## Artifacts cited

- `openspec/sdd-init/aom_retold_mod_idle_auto_repair.md` — project conventions, no XS test runner, manual verification workflow.
- `openspec/config.yaml` — strict TDD false, verification by manual deploy/log.
- `openspec/changes/archive/2026-06-18-bootstrap-human-assist-improvements-mod/design.md` — 4th-mod extensibility pattern, reuse-at-deploy decision.
- `openspec/changes/archive/2026-06-18-bootstrap-human-assist-improvements-mod/exploration.md` — prior mod layout, `human_assist.xs` include/hook pattern.
- `mod/idle_auto_repair/game/ai/human_assist/auto_repair.xs:30-66`, `:111`, `:238`, `:251`, `:293-323` — passive-feature pattern, idle filter, human guard, rule cadence.
- `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:210-218`, `:2880-2912`, `:2952-2998` — parallel-array tracker and registration pattern.
- `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs:14-15`, `:948-994` — include block and `main()` hook point.
- `scripts/deploy-mods.sh:87-90` — 4th mod deploy block.
- `mod/Extra Ai + AoModAi/README.md` — confirms `extra ai` relic logic lives in `core/exploration.xs`.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/exploration.xs:1799-2031` — vanilla relic API usage.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/utilities/unit_queries.xs:495-543` — temple-with-space selection.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/core/setup.xs:180-181` — relic handler registration.
- `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/game/ai/human_assist/human_assist.xs:957` — existing handler use in VPS overlay.

## Recommendation matrix

| Rank | Approach | When to adopt | Rationale |
|------|----------|---------------|-----------|
| **1** | **C — Filter-pool idle-hero polling** | **Default / safest choice** | Mirrors `auto_repair.xs`, needs no event-handler overrides, complexity is low, and the engine pre-filters idle heroes. Cost is bounded by the number of idle heroes, not total heroes. |
| **2** | **B — Event-driven** | **If manual testing proves we can identify the carrier from the handler** | Gives instant response and near-zero cost when nothing happens. Adopt only after answering Open question #1 and #2; otherwise the event still forces a hero scan and adds handler-conflict risk. |
| **3** | **B+C hybrid** | **If B is adopted** | Keep the idle-hero poll at a slower cadence (e.g. 5–10 s) as a safety net for events the handler may miss, while B handles the common case instantly. |

**Net recommendation for the proposer:** Start with **Approach C** (idle-hero filter-pool polling). It satisfies all constraints, follows the existing `auto_repair.xs` architecture, and avoids event-handler exclusivity risks. Add **Approach B** later only if in-game tests confirm that `cXSRelicPickedUpHandler` lets us identify the picking hero (and its relic) without a full scan.
