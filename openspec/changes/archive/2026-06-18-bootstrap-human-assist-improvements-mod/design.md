# Design: Bootstrap the HumanAssistImprovements bundled mod

## Technical Approach

Add a fourth AoM:R local mod, **Human Assist Improvements**, as the canonical bundle for all future human-assist features. It is deployed as a directory-tree overlay over the vanilla path `game/ai/human_assist/`, the same mechanism used by the three existing mods (`docs/path3_research.md:182-225`).

At bootstrap, the new mod behaves identically to the existing combined mod:

- Its `human_assist.xs` mirrors the combined mod (`mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`), including both `auto_repair.xs` and `auto_scout.xs` and calling `autoScout_register(planID, unitID)` inside `enableAutoScouting`.
- The feature files `auto_repair.xs` and `auto_scout.xs` are not duplicated; `scripts/deploy-mods.sh` resolves them from the canonical sibling directories at deploy time, matching the current combined-mod pattern (`exploration.md:181-189`).

Future features will add a single `auto_<feature>.xs` file inside `mod/human_assist_improvements/game/ai/human_assist/` and add one `include` line (and any hook registration call) to this mod's `human_assist.xs`, keeping composition linear instead of combinatorial.

## Layering

```text
repository
├── mod/
│   ├── idle_auto_repair/
│   │   └── game/ai/human_assist/
│   │       ├── auto_repair.xs          ← canonical repair source
│   │       └── human_assist.xs
│   ├── intelligent_auto_scout/
│   │   └── game/ai/human_assist/
│   │       ├── auto_scout.xs           ← canonical scout source
│   │       └── human_assist.xs
│   ├── intelligent_auto_repair_and_scout/
│   │   └── game/ai/human_assist/
│   │       └── human_assist.xs         ← combined overlay (today)
│   └── human_assist_improvements/      ← NEW: same shape as combined
│       └── game/ai/human_assist/
│           ├── human_assist.xs         ← combined overlay + future includes
│           └── auto_<future>.xs        ← future features live here
└── scripts/deploy-mods.sh              ← 4th deploy block added here
```

At deploy time, the 4th mod's target folder contains copies of `auto_repair.xs` and `auto_scout.xs` (reused from siblings) plus the 4th mod's `human_assist.xs`:

```text
AoM:R mods/local/
└── Human Assist Improvements/
    └── game/ai/human_assist/
        ├── auto_repair.xs   (deploy-time copy from idle_auto_repair/)
        ├── auto_scout.xs    (deploy-time copy from intelligent_auto_scout/)
        └── human_assist.xs  (from human_assist_improvements/)
```

## Architecture Decisions

### D1: Reuse existing feature files at deploy time

| Approach | Tradeoff | Decision |
|---|---|---|
| Reuse `auto_repair.xs` / `auto_scout.xs` from siblings at deploy | 4th mod source is not self-contained for existing features, but preserves single source of truth | Chosen |
| Private copies inside the 4th mod | Self-contained packaging; four copies to keep in sync | Rejected |

The existing combined mod already uses reuse-at-deploy (`scripts/deploy-mods.sh:57-60`). Keeping one repair/scout source reduces regression risk while the standalone mods remain maintained. Future `auto_<feature>.xs` files will live directly in the 4th mod directory because they have no standalone sibling.

### D2: Directory and deployed names

- **Directory:** `mod/human_assist_improvements/` (lower_snake_case, matching existing mod folders).
- **Deployed folder:** `Human Assist Improvements` (word-cased with spaces, matching `Idle Auto-Repair`, `Intelligent Auto-Scout`, etc.).

### D3: Conflict handling

All four mods shadow the same vanilla file `game/ai/human_assist/human_assist.xs`. AoM:R overlays are whole-file replacements with no additive layering, so only one can be active (`exploration.md:75`, `docs/path3_research.md:203-225`). The README for the 4th mod will warn players not to install it alongside the other three mods. No runtime detection is proposed; documentation is the mitigation.

### D4: Placeholder thumbnail

Use the existing combined-mod thumbnail (`thumbnail_idle-auto-repair-and-intelligent-auto-scout.png`) copied to `thumbnail_human-assist-improvements.png`. This satisfies the bootstrap packaging UI while leaving a dedicated asset as future work.

## `human_assist.xs` construction

The new overlay is a copy of the combined mod's `human_assist.xs` with identical edits:

```xs
// Lines 14-15 — feature includes (after vanilla sibling includes)
include "human_assist/auto_repair.xs"; // Idle Auto-Repair mod
include "human_assist/auto_scout.xs";  // Intelligent Auto-Scout mod
```

```xs
// Inside enableAutoScouting, immediately after the Oracle-specific block
// (human_assist.xs:103-108) and before aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true)
autoScout_register(planID, unitID); // Intelligent Auto-Scout mod (no-op for AI players)
```

Citations: combined-mod includes at `exploration.md:60-64`; combined-mod hook at `exploration.md:68-71`.

## Data Flow / State Machines

There is no new runtime state machine for this bootstrap. The 4th mod inherits the existing `auto_repair.xs` and `auto_scout.xs` state machines unchanged.

```text
Engine loads human_assist.xs overlay
          │
          ├── include auto_repair.xs ──→ autoRepair watchdog / minInterval rules
          │                                  (state: idle → find damaged building → assign villager)
          │
          └── include auto_scout.xs  ──→ autoScout tickHeavy / tickFast rules
                                           (state: scout idle → register explore plan → pathing)
```

### Extensibility hook

To add a future feature `auto_something.xs`:

1. Place `auto_something.xs` in `mod/human_assist_improvements/game/ai/human_assist/`.
2. Add one `include "human_assist/auto_something.xs";` line to the 4th mod's `human_assist.xs`.
3. Add any required hook call (e.g., inside `enableAutoScouting`, `main()`, etc.).
4. Extend `scripts/deploy-mods.sh` to deploy `auto_something.xs` from `$HUMAN_SRC/`.

This pattern is the single mechanism that eliminates combinatorial combined-mod proliferation.

## File Changes

| File | Action | Description |
|---|---|---|
| `mod/human_assist_improvements/game/ai/human_assist/human_assist.xs` | Create | Vanilla copy + both existing includes + scout hook + future extension anchors |
| `mod/human_assist_improvements/README.md` | Create | Bundle description, install steps, conflict warning, patch-maintenance note |
| `thumbnail_human-assist-improvements.png` | Create | Placeholder copied from the combined-mod thumbnail |
| `scripts/deploy-mods.sh` | Modify | Fourth deploy block for `Human Assist Improvements`; does not alter existing three blocks |
| `README.md` | Modify | Add 4th mod to mod list and conflict note |

## Interfaces / Contracts

No new XS API functions or engine contracts are introduced. The 4th mod relies on the existing feature-file contracts:

- `auto_repair.xs` exposes `autoRepair_setupQueries()` (called implicitly via rule setup) and the `autoRepair` rule.
- `auto_scout.xs` exposes `autoScout_register(int planID, int unitID)`, called from `enableAutoScouting`.
- Both feature files guard behavior with `kbPlayerIsHuman(cMyID)`.

## Testing Strategy

| Layer | What to Test | Approach |
|---|---|---|
| Deploy | 4th mod files copied to correct `mods/local/Human Assist Improvements/...` paths | Run `scripts/deploy-mods.sh` and inspect output |
| Overlay | `human_assist.xs` contains both includes and the scout hook | Diff against combined mod's `human_assist.xs` |
| E2E | Repair and scout behave as in combined mod | Manual AoM:R match; check `aiEcho` output for both features |

## Migration / Rollout

No migration required. The change is purely additive.

Rollback plan:

1. Delete `mod/human_assist_improvements/`.
2. Revert the new block and header comment in `scripts/deploy-mods.sh`.
3. Revert `README.md` list/conflict additions.
4. Delete `thumbnail_human-assist-improvements.png`.

## Open Questions

- Final custom thumbnail asset for the 4th mod is pending; placeholder is acceptable for bootstrap.
- Platform packaging manifest requirements, if any, must be confirmed during upload; no manifest file is added in source.
