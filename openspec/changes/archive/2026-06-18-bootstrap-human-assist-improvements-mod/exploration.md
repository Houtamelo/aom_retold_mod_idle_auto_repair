# Exploration: Bootstrap a 4th bundled mod — HumanAssistImprovements

## Change id

bootstrap-human-assist-improvements-mod

## Status

- [x] Phase: explore
- [ ] Next: propose

## Goal

Understand how the existing three deployed mods are structured and wired into the shared AoM:R file `game/ai/human_assist/human_assist.xs`, so we can bootstrap a fourth mod (`HumanAssistImprovements`) that bundles the existing Auto-Repair + Intelligent Auto-Scout features **and serves as the single home for all future human-assist features**, eliminating the combinatorial explosion of combined mods.

## Method

- Read the project init report (`openspec/sdd-init/aom_retold_mod_idle_auto_repair.md`) and `openspec/config.yaml` for conventions, testing limits, and shared-file rules.
- Listed every tracked file under `mod/` to map mod directory layouts.
- Read and diffed `human_assist.xs` in the three deployed mods to identify the exact overlay deltas.
- Read `auto_repair.xs` (full) and skimmed `auto_scout.xs` to confirm how feature files are included and what hook registration they need.
- Read `scripts/deploy-mods.sh` to understand deploy names, source resolution, and shared-file copying.
- Read each existing `README.md` and the top-level `README.md` to extract mod-description conventions and patch-maintenance notes.
- Searched `docs/` and `extracted/` for mod manifests, packaging metadata, or `.mod`/`.mrtmod`/`*mod*.xml` files.
- Checked `docs/path3_research.md` to confirm the file-overlay mechanism (a local mod is just a directory tree under `mods/local/<name>/`).

## Findings

### 1. Existing mod layout

| Mod directory | Deployed folder name | Tracked source files | Note |
| --- | --- | --- | --- |
| `mod/idle_auto_repair/` | `Idle Auto-Repair` | `game/ai/human_assist/human_assist.xs`, `game/ai/human_assist/auto_repair.xs` | Standalone repair mod |
| `mod/intelligent_auto_scout/` | `Intelligent Auto-Scout` | `game/ai/human_assist/human_assist.xs`, `game/ai/human_assist/auto_scout.xs` | Standalone scout mod |
| `mod/intelligent_auto_repair_and_scout/` | `Intelligent Auto-Repair and Scout` | `game/ai/human_assist/human_assist.xs` only | Combined mod; feature files copied from siblings at deploy time |
| `mod/aom_autorepair_test/` | (not deployed) | `game/ai/human_assist/human_assist.xs` | Historical proof-of-concept; no mod include lines |

All live under `mod/<dir>/game/ai/human_assist/`. No `.xs` files exist deeper or elsewhere in the tracked mod trees.

### 2. How `human_assist.xs` differs across the three deployed mods

The vanilla base is the proof-of-concept file in `mod/aom_autorepair_test/game/ai/human_assist/human_assist.xs`, which ends its include block at lines `11-13`:

```xs
include "human_assist/human_assist_debug.xs";
include "human_assist/human_assist_unit_queries.xs";
include "human_assist/human_assist_resource_breakdown_system.xs";
```

Each deployed mod adds **exactly one include** right after that block:

- `mod/idle_auto_repair/game/ai/human_assist/human_assist.xs:14`:
  ```xs
  include "human_assist/auto_repair.xs"; // Idle Auto-Repair mod
  ```
- `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:14`:
  ```xs
  include "human_assist/auto_scout.xs"; // Intelligent Auto-Scout mod
  ```
- `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs:14-15`:
  ```xs
  include "human_assist/auto_repair.xs"; // Idle Auto-Repair mod
  include "human_assist/auto_scout.xs";  // Intelligent Auto-Scout mod
  ```

The scout mod also injects a single registration call inside `enableAutoScouting`:

- `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:109` and `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs:110`:
  ```xs
  autoScout_register(planID, unitID); // Intelligent Auto-Scout mod (no-op for AI players)
  ```

The call is placed immediately after the Oracle-specific block (`human_assist.xs:103-108`) and before the `aiPlanSetFlag(planID, cPlanFlagNoMoreUnits, true)` line.

This is the combinatorial bottleneck: both mods need to edit the same `human_assist.xs`, but AoM:R file overlays are whole-file replacements, so only one mod's overlay can be active per `human_assist.xs` path.

### 3. Feature files: `auto_repair.xs` and `auto_scout.xs`

Both are self-contained files placed next to `human_assist.xs` and pulled in via `include`:

- `mod/idle_auto_repair/game/ai/human_assist/auto_repair.xs:323`
  - Defines `autoRepair_setupQueries()`, `autoRepair_tryAssign()`, `autoRepair_watchdog()`, the `autoRepair_watchdogRule` highFrequency rule, and the `autoRepair` `minInterval 3` rule.
  - Guards on `kbPlayerIsHuman(cMyID)` so it does nothing for AI players (`auto_repair.xs:111`, `auto_repair.xs:238`).
- `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:3054`
  - Defines `autoScout_register()`, `autoScout_tickHeavy`, and `autoScout_tickFast` rules.
  - Also guards on `kbPlayerIsHuman(cMyID)` (`auto_scout.xs:2902`).

Because they rely on globals such as `gReservePlan` declared in `human_assist.xs`, they must be included after that declaration (line 10) and after the sibling helper includes.

### 4. `scripts/deploy-mods.sh` deploy flow

`scripts/deploy-mods.sh:34-43` defines a `deploy()` helper that copies a source file to a destination under `AOMR_LOCAL_MODS` (default: the Proton prefix Steam user mods path).

Source roots (`scripts/deploy-mods.sh:45-47`):

```bash
REPAIR_SRC="$SRC_ROOT/idle_auto_repair/game/ai/human_assist"
SCOUT_SRC="$SRC_ROOT/intelligent_auto_scout/game/ai/human_assist"
COMBINED_SRC="$SRC_ROOT/intelligent_auto_repair_and_scout/game/ai/human_assist"
```

Deployment blocks (`scripts/deploy-mods.sh:49-60`):

1. **Idle Auto-Repair** copies `auto_repair.xs` and `human_assist.xs` to `Idle Auto-Repair/game/ai/human_assit/`.
2. **Intelligent Auto-Scout** copies `auto_scout.xs` and `human_assist.xs` to `Intelligent Auto-Scout/game/ai/human_assist/`.
3. **Intelligent Auto-Repair and Scout** copies `auto_repair.xs` from `REPAIR_SRC`, `auto_scout.xs` from `SCOUT_SRC`, and `human_assist.xs` from `COMBINED_SRC` to `Intelligent Auto-Repair and Scout/game/ai/human_assist/`.

The combined mod therefore has **no own copies** of the feature files; the deploy script resolves them from sibling directories at deploy time.

To add a 4th mod, the script needs a fourth deploy block, e.g.:

```bash
HUMAN_SRC="$SRC_ROOT/human_assist_improvements/game/ai/human_assist"
...
deploy "$REPAIR_SRC/auto_repair.xs"        "$DEPLOY_ROOT/Human Assist Improvements/game/ai/human_assist/auto_repair.xs"
deploy "$SCOUT_SRC/auto_scout.xs"          "$DEPLOY_ROOT/Human Assist Improvements/game/ai/human_assist/auto_scout.xs"
deploy "$HUMAN_SRC/human_assist.xs"        "$DEPLOY_ROOT/Human Assist Improvements/game/ai/human_assist/human_assist.xs"
```

If the 4th mod adds its own feature files (e.g. `auto_something.xs`), those would be deployed from `$HUMAN_SRC/` too.

### 5. Mod metadata / manifest

- `glob` for `**/*.mod`, `**/*.mrtmod`, `**/*mod*.xml`, and `**/extracted/**/*` returned **no tracked manifest files**.
- `docs/path3_research.md:182-224` confirms AoM:R local mods work by **directory tree overlay**: a folder under `mods/local/<name>/game/...` shadows vanilla files by relative path. No dropdown selection is required, and there is no additive layering for `.xs` files.
- `docs/proto_mods_syntax.md` explains additive-merge XML for unit data (`proto_mods.xml`, `techtree_mods.xml`, etc.), but these are data files, not mod package manifests.
- The existing thumbnails (`thumbnail_idle-auto-repair.png`, etc.) are loose in the repo root, named by mod. No metadata file references them.

**Conclusion:** The source repo does not carry a mod manifest. Local installation works purely by folder presence and tree structure; the Age of Mythology Retold mod platform likely adds its own packaging manifest when the folder is uploaded.

### 6. README files

| README | Purpose | Key sections |
| --- | --- | --- |
| `README.md` | Project landing | Mod list, project layout, high-level changelogs |
| `mod/idle_auto_repair/README.md` | Per-mod player doc | Feature description, layout, local testing, patch maintenance, changelog |
| `mod/intelligent_auto_scout/README.md` | Per-mod player doc | Feature description, layout, local testing, patch maintenance (include + hook), changelog |
| `mod/intelligent_auto_repair_and_scout/README.md` | Combined changelog | Bundled feature list, local testing, combined changelog |

A 4th mod README should follow the combined README pattern: describe the bundle, link to the standalone READMEs for deep behaviour docs, explain the conflict (do not install alongside the standalone or combined mods), provide patch-maintenance steps for `human_assist.xs`, and list a unified changelog.

### 7. Naming recommendations

Existing conventions:

- Directory: `mod/idle_auto_repair/`, `mod/intelligent_auto_scout/`, `mod/intelligent_auto_repair_and_scout/` — all lower_snake_case.
- Deployed folder: `Idle Auto-Repair`, `Intelligent Auto-Scout`, `Intelligent Auto-Repair and Scout` — word-cased with spaces.
- File names inside: lower_snake_case (`auto_repair.xs`, `human_assist.xs`).

Recommendation for the 4th mod:

- **Directory:** `mod/human_assist_improvements/`
- **Deployed folder:** `Human Assist Improvements` (matches the user's name and the existing word-cased pattern)
- **Internal files:** lower_snake_case, e.g. `auto_repair.xs`, `auto_scout.xs`, `auto_<feature>.xs`

### 8. The bundling solution

#### Recommended structure

The 4th mod follows the same `game/ai/human_assist/` shape as the existing three mods:

```text
mod/human_assist_improvements/
├── README.md
└── game/ai/human_assist/
    ├── human_assist.xs          # vanilla copy + both existing includes + future includes
    └── auto_<future>.xs         # future features live here (one file per self-contained feature)
```

#### What `human_assist.xs` should contain

Mirror the combined mod:

- Lines `14-15`: include both `auto_repair.xs` and `auto_scout.xs`.
- Line `110`: call `autoScout_register(planID, unitID)` inside `enableAutoScouting`.
- As new features are added, add their `include` lines next to the existing feature includes.
- If a future feature needs a hook inside `enableAutoScouting`, `main()`, or another vanilla function, add the call there.

This gives one and only one place where all human-assist overlays compose, making future composition linear instead of combinatorial.

#### Own copies vs. reuse-at-deploy for existing feature files

| Approach | Pros | Cons |
| --- | --- | --- |
| **Reuse at deploy** (copy `auto_repair.xs`/`auto_scout.xs` from sibling dirs into the 4th deploy target) | Single source of truth for repair/scout logic across all four maintained variants | 4th mod source dir is incomplete by itself; packaging needs the sibling dirs |
| **Own copies** in `mod/human_assist_improvements/game/ai/human_assist/` | 4th mod is fully self-contained; packaging is trivial | Four copies of repair/scout to keep in sync whenever the standalone mods are updated |

**Recommendation: reuse at deploy for existing `auto_repair.xs` and `auto_scout.xs`.**

Rationale:

- The user explicitly intends to continue maintaining the standalone repair/scout mods. Keeping one source file per feature reduces regression risk and patch-maintenance burden.
- The existing combined mod already uses this pattern; it's proven and keeps the repo DRY.
- The 4th mod's source will still be self-contained for **future** features, which is the actual growth area.

Future features should live as source files **inside** `mod/human_assist_improvements/game/ai/human_assist/` and be deployed directly from there.

#### How this eliminates combinatorial explosion

With repair (R) and scout (S), the existing three mods are `{R, S, R+S}`. Adding a future feature F without a bundled mod would require `{F, R+F, S+F, R+S+F}` — four new mods. If a second feature G is added, the combinations explode further.

The 4th mod is the single target that already contains `{R, S, ...}`. Any future feature only needs to be added **inside** this mod's `human_assist.xs` include list. No additional combined mods are needed because the 4th mod is the canonical combined package.

### 9. Relationship to the existing 3 mods

The 4th mod is **additive and a strict superset** of the combined mod at launch:

- It includes all current repair/scout behaviour.
- It keeps the standalone repair and scout mods untouched, as requested.
- It is **not** intended to be installed alongside the other mods because they all overlay the same `game/ai/human_assist/human_assist.xs` file; only one can win.

Consistency strategy:

- Keep `auto_repair.xs` canonical in `mod/idle_auto_repair/game/ai/human_assist/`.
- Keep `auto_scout.xs` canonical in `mod/intelligent_auto_scout/game/ai/human_assist/`.
- Reuse those two files at deploy time for both `mod/intelligent_auto_repair_and_scout/` and the new `mod/human_assist_improvements/`.
- If the hook wiring in `human_assist.xs` changes (e.g., a new scout registration signature), update both combined overlays.

### 10. Risks / unknowns

| Risk | Impact | Mitigation |
| --- | --- | --- |
| **Vanilla patch invalidates `human_assist.xs`** | High; every overlay mod breaks on patch day | Document patch-maintenance steps in the 4th README; re-derive overlay from fresh vanilla copy |
| **`scripts/deploy-mods.sh` must change** | Medium; forgetting to add the new block leaves the 4th mod undeployed | Add block to deploy script as part of bootstrap; verify with a dry-run |
| **No automated tests / no XS runner** | High; regressions only caught in-game | Manual deploy + `aiEcho` verification checklist per feature |
| **Load-order conflict with existing 3 mods** | High if player enables multiple | README should clearly state "do not install alongside Idle Auto-Repair, Intelligent Auto-Scout, or Intelligent Auto-Repair and Scout" |
| **Manifest / thumbnail for mod platform** | Medium; packaging UI may require a thumbnail | Provide `thumbnail_human-assist-improvements.png` and confirm platform naming convention during upload |
| **Future feature hook collisions** | Medium | Keep all feature includes and hook registrations in one `human_assist.xs`; use distinct prefixes per feature |

## Open questions

- Does AoM:R support a mod loading a feature `.xs` file from its own subdirectory via `include`?
  - Yes, confirmed by the existing three mods: `include "human_assist/auto_repair.xs";` works because the whole `game/ai/` subtree is resolved relative to the active overlay path (`docs/path3_research.md:182-203`).
- Does the 4th mod need any entry in a mod manifest/packaging step beyond its directory, or is directory presence sufficient?
  - Directory presence with the correct `game/...` tree is sufficient for local loading (`docs/path3_research.md:220-225`). The Steam/mod-platform packaging step may generate its own manifest; that requirement should be confirmed against the platform's upload workflow, but no manifest file is created in this repo.
- Should the deploy script copy the 4th mod's feature files from siblings (current pattern) or should the 4th mod contain its own copies going forward?
  - **Existing repair/scout files should continue to be copied from sibling dirs** to preserve a single source of truth. **Future feature files should live in the 4th mod directory** and be deployed directly from there.

## Artifacts cited

- `openspec/sdd-init/aom_retold_mod_idle_auto_repair.md` — project structure, conventions, testing constraints, shared-file architecture notes.
- `openspec/config.yaml` — openspec rules: strict TDD false, verification via manual deploy/log, note to update combined mod wiring.
- `mod/idle_auto_repair/game/ai/human_assist/human_assist.xs:14` — repair include line.
- `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:14` — scout include line.
- `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:109` — `autoScout_register` hook.
- `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs:14-15` — combined includes.
- `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs:110` — combined `autoScout_register` hook.
- `mod/aom_autorepair_test/game/ai/human_assist/human_assist.xs:11-13` — vanilla include baseline.
- `mod/idle_auto_repair/game/ai/human_assist/auto_repair.xs:111`, `auto_repair.xs:238` — human-player guards.
- `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs:2902` — human-player guard for scout.
- `scripts/deploy-mods.sh:45-60` — source resolution and shared-file copy logic.
- `docs/path3_research.md:182-225` — file-overlay mechanism and no-additive-modding note.
- `docs/proto_mods_syntax.md:1-146` — additive-merge XML for game data (not a mod manifest).
- `README.md`, `mod/idle_auto_repair/README.md`, `mod/intelligent_auto_scout/README.md`, `mod/intelligent_auto_repair_and_scout/README.md` — README conventions.

## Risks / unknowns (summary)

- `human_assist.xs` is a whole-file vanilla overlay; any game patch that touches it requires manual rebasing of the include/hook edits.
- The deploy script must be extended for the 4th mod or it will not deploy.
- No automated XS test runner exists; correctness is verified by manual deploy and in-game `aiEcho` log inspection.
- The 4th mod must be marked as conflicting with the existing three mods because they all shadow the same `human_assist.xs` path.
- Mod-platform packaging requirements (thumbnail, manifest, naming constraints) should be confirmed during the upload step; no such metadata file is present in source.
