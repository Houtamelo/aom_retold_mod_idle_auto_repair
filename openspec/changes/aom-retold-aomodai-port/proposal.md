# Proposal: Port AoModAi layered walls behavior into Extra Ai + AoModAi

**Status:** Approved by user on 2026-06-18. Scope narrowed to layered walls (3 sub-features). Forward firebases, GP combo, and tower/fort recycling are explicitly deferred. Playtest cadence is handled by the user (not part of this spec).

## Intent

Port the AoModAi "layered walls" behavior set into the existing AoM:Retold mod `Extra Ai + AoModAi`. AoModAi builds a 2nd wall ring around its main base once the first ring is in place and the base has matured; it also delays wall-building for rusher personalities and cancels wall plans if the base comes under heavy attack. AoM:Retold's built-in `wallManager` only does 1 ring (2 rings on turtler personality + Large/Giant maps) and has no attack-cancellation; the mod already extends the 1st ring to default strategies. This proposal ports the 2nd ring, the rusher delay, and the attack-cancellation as a single integrated behavior.

## Scope

### In scope
- **2nd wall ring for default-strategy AIs** — add a polled rule that creates a 2nd `cPlanBuildWall` plan (ring type, radius 50) centered on the main base when the 1st ring is active and other conditions hold.
- **Wall-building rusher delay** — gate ALL wall-building (existing 1st ring + the new 2nd ring) on the AoModAi `mRusher` flag: rusher AIs wait until Age 3 + 15 min before building any walls.
- **Attack-cancellation** — destroy the 2nd-ring wall plan if 2× enemy units are near the base AND the base has been under attack for > 25 s. Prevents villagers getting trapped building walls during a push.
- **12-min plan lifetime** — matches AoModAi's `mainBaseAreaWallTeam2`: destroy the 2nd-ring plan after 12 min of activity.
- Targeted patch to the existing `mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs`. No whole-file replacement. No new files.

### Out of scope (deferred)
- **Forward firebases / siege support** — separate behavior; can be added later.
- **GP combo (Vision → Meteor/Tornado)** — separate behavior; can be added later.
- **Tower/fort recycling** — separate behavior; can be added later.
- **Per-secondary-base ring walls** (`otherBaseRingWallTeam1/2`) — default AI doesn't manage multiple TC bases well; would be wasted work.
- **Ally base wall rings** (`WallAllyMB`) — mod doesn't extend to allies; staying consistent.
- **Modifying the existing 1st-ring behavior** — the mod's `mWallCircleAmount = 1` in default strategies is correct and stays as-is.
- **Playtest cadence** — user handles manually; not part of this spec.

## Approach

- Reuse the Retold base AI file `core/buildings/buildings.xs` as the working copy (the mod overlays it).
- Add ONE polled rule (named `secondRingWallPlanMonitor` or similar) that handles all 3 sub-features.
- Port the relevant logic from AoModAi's `mainBaseAreaWallTeam2` (`extracted/mods_AoModAi/AoModAi\ai2\AoModAIBuild.xs:803`), adapt to Retold API.
- Wrap inserted code in audit markers: `// === AoModAi: layered walls begin ===` / `// === end ===`.
- Keep CRLF line endings to match the existing mod convention.
- XS syntax validated by enabling the rule and watching the in-game AI log for parse errors. User handles the actual in-game playtest.

## The behavior in detail

### Sub-feature 1: 2nd wall ring for default-strategy AIs

- **What it does:** When the AI's main base has a 1st-ring wall plan (created by `wallManager` from `mWallCircleAmount = 1`) AND all gating conditions hold, create a 2nd `cPlanBuildWall` plan with `cBuildWallPlanWallTypeRing`, `cBuildWallPlanWallRingCenterPoint = kbBaseGetLocation(mainBaseID)`, and `cBuildWallPlanWallRingRadius = 50`.
- **Why default-strategy AIs need this:** Their `mWallCircleAmount` is statically `1`, so `wallManager` only ever creates the 1st ring. The 2nd ring is gated to `*_turtler_strategy` AND only on Large/Giant maps in Retold base.
- **Why a separate rule (not bumping `mWallCircleAmount`):** `wallManager` (Retold's `core/buildings/buildings.xs:933`) explicitly skips a base that already has a wall plan. Bumping `mWallCircleAmount` from 1 to 2 mid-game would not create the 2nd ring — the wallManager would see the 1st ring and skip. The clean port is a separate polled rule that creates the 2nd plan directly.

### Sub-feature 2: Wall-building rusher delay

- **What it does:** Detect the `mRusher` flag (AoModAi's personality marker for rushers) and delay wall-building until Age 3 + 15 min for rusher AIs. Non-rusher AIs build walls normally.
- **Why this matters:** Rusher AIs are designed to be aggressive and rely on early military, not walls. Building walls too early wastes villagers and delays the rush. The mod's current behavior (Retold's `wallManager`) starts walling as soon as the strategy is set (Age 2/3/4/5) with no personality-aware delay.
- **AoModAi gating logic to port:**
  - `mRusher` flag check (already used elsewhere in the mod? need to verify)
  - If rusher: wait until `kbGetAge() >= cAge3 && xsGetTime() >= 15 * 60 * 1000`
  - If non-rusher: build normally after the other gating holds

### Sub-feature 3: Attack-cancellation

- **What it does:** When the 2nd-ring plan is active and the base is under attack, track how long the attack has been sustained. If the attack has lasted > 25 s (and we're past 19 min in game time), destroy the plan. Also recreate the plan after the attack ends.
- **Why this matters:** Without cancellation, villagers keep building walls during a push, get trapped, and the player wipes the base. AoModAi's choice to cancel walls under attack is part of what makes the behavior look "smart" rather than "scripted".
- **Attack signal source:** AoModAi uses `kbBaseGetTimeUnderAttack(cMyID, baseID)`, but **this API does not exist in Retold**. Instead, we use Retold's pre-computed extern arrays `gDefendTCBases` + `gEnemyPowerInBases` (declared in `core/globals.xs:163-164`, populated by `military_defend.xs` every defense frame). A static timestamp `gSecondRingAttackStartTime` tracks the sustained duration locally.
- **Simplification vs AoModAi:** AoModAi had two checks: (1) sustained > 25 s, (2) enemy units > 2× own+ally units AND enemy > 4. With Retold's power-based signal (single value, not unit count), the 2x ratio check is awkward to replicate. **The first port uses only the sustained check.** A future enhancement can add a power-ratio check: `gEnemyPowerInBases[index] > availablePowerToDefendWith * gWinningArmyPercentage` (the same idiom Retold uses in `military_defend.xs:731`).

## Files to modify

| File | Change |
|------|--------|
| `mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs` | Add ONE polled rule (`secondRingWallPlanMonitor` or similar) that handles 2nd-ring creation, rusher delay, and attack-cancellation. ~150–250 lines including comments and markers. |

No new files. No whole-file replacement.

## Open questions for the user

- **Gating thresholds:** match AoModAi's defaults (8 min min game time, Age 2+, ≥ 10 villagers, ≥ 150 gold) or be more aggressive (start earlier, lower thresholds) so the 2nd ring is visible in shorter playtest matches?
- **Radius for the 2nd ring:** use 50 (Retold `wallManager` uses `30.0 + iCircle * 20.0`, so 50 is the natural second-circle radius) or pick a different value?

## Review workload forecast

| Item | Estimate |
|------|----------|
| Files to modify | 1 (`mod/Extra Ai + AoModAi/game/ai/core/buildings/buildings.xs`) |
| New files | 0 |
| Estimated changed lines | 150–250 (single polled rule + helpers + markers) |
| Risk level | Medium |
| Decision needed before apply? | No — proposal is approved, only the 2 open questions above are non-blocking |
| Chained PRs recommended? | No — single PR for the whole wall behavior (the 3 sub-features are tightly coupled in one rule) |

## Out of scope / not ported (verified)

- **Migration (generic overrun relocation):** Retold already handles island migration; the full scorer is too coupled to port.
- **Smarter military decisions:** Retold `attackManager` + Extra AI's doubled `mMinimumAttackSize` cover this.
- **Layered walls (1st ring):** already covered by the mod's `mWallCircleAmount = 1` in default strategies and Retold base's `*_turtler_strategy` files.
- **Allied base defense / donations / multi-base economy / Wonder / KOTH / nomad:** all have substantial existing coverage in Retold base AI.
- **Obelisk-clearing / raiding parties:** civ-specific or require a new attack-plan subsystem; too large for a targeted patch.
- **Per-secondary-base ring walls:** default AI doesn't manage multiple TC bases well; wasted work.
- **Ally base wall rings:** mod doesn't extend to allies; staying consistent.
- **Forward firebases / siege support:** deferred to a future change.
- **GP combo (Vision → Meteor/Tornado):** deferred to a future change.
- **Tower/fort recycling:** deferred to a future change.
- **Playtest cadence:** user handles manually; not part of this spec.

## Rollback plan

Remove the marked `// === AoModAi: layered walls begin ===` block from `core/buildings/buildings.xs`. Because the change is additive and marked, rollback is a simple deletion.

## Success criteria (in-game, observed by user)

- [ ] XS syntax: rule loads without parse errors in a real match (visible in AI log on game start).
- [ ] Rusher delay: a rusher-AI match shows no wall construction in the first 15 min (or until Age 3, whichever is later).
- [ ] Non-rusher: a non-rusher-AI match on a small/medium map shows a 2nd wall ring forming around the AI's main base after the 1st ring is in place.
- [ ] Attack-cancellation: when the AI's main base comes under heavy attack, the 2nd-ring wall plan is destroyed; villagers stop building walls and the base is no longer trapped.
- [ ] No regression in existing Extra AI themes (attack sizes, GP fixes, etc.).
