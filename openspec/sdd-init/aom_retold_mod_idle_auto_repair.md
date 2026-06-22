# SDD Init Report — aom_retold_mod_idle_auto_repair

> Generated during SDD init. Persisted so the orchestrator and downstream sub-agents can read project context without re-running detection.

## Project Metadata

| Field | Value |
| --- | --- |
| Project name | Age of Mythology: Retold mods — Idle Auto-Repair & Intelligent Auto-Scout |
| Project path | `/home/houtamelo/Documents/projects/aom_retold_mod` |
| Project key | `aom_retold_mod_idle_auto_repair` |
| Default branch | `master` |
| Primary language | XS (Age of Mythology: Retold AI scripting language) |
| Runtime | Age of Mythology: Retold game engine (proprietary, Windows/Proton) |
| License | MIT |
| Remote origin | `git@github.com:Houtamelo/aom_retold_mod_idle_auto_repair.git` |
| Commit count | ~118 |

## Stack

| Layer | Tool / Technology | Notes |
| --- | --- | --- |
| Language | XS | C-like scripting language used by AoM:R for AI / scenario scripts. Loaded and executed by the game engine. |
| Source formats | `.xs` scripts, Markdown docs | No XML mod manifests detected in the tracked source (the mod platform may add its own packaging). |
| Build system | None | No Makefile, package.json, Cargo.toml, go.mod, pyproject.toml, or CI pipeline. |
| Package manager | None | Not applicable. |
| Deploy tooling | Bash | `scripts/deploy-mods.sh` copies source files into the AoM:R `mods/local/` folder. |
| Framework | AoM:R engine APIs | Heavy use of `kb*` (knowledge base) and `ai*` (AI planner) functions, e.g. `kbUnitQueryCreate`, `aiTaskUnitMove`, `aiPlanCreate`. |

### Mod packages

| Mod directory | Deployed name | Source files |
| --- | --- | --- |
| `mod/idle_auto_repair/` | `Idle Auto-Repair` | `auto_repair.xs` (323 lines), `human_assist.xs` (991 lines) |
| `mod/intelligent_auto_scout/` | `Intelligent Auto-Scout` | `auto_scout.xs` (3054 lines), `human_assist.xs` (992 lines) |
| `mod/intelligent_auto_repair_and_scout/` | `Intelligent Auto-Repair and Scout` | `human_assist.xs` (993 lines); reuses `auto_repair.xs` and `auto_scout.xs` from the sibling mods at deploy time |
| `mod/aom_autorepair_test/` | (reference only, not deployed) | Historical proof-of-concept `human_assist.xs` (1270 lines) |

## Conventions

### Lint / formatter

- **None detected.** No `.editorconfig`, `rustfmt.toml`, `.prettierrc`, `.eslintrc`, `flake8`, or similar configuration exists.
- Manual style observed in source:
  - Three-space indentation in `.xs` files.
  - Block headers with `//=====` separators.
  - Inline comments use `//`.
  - Constants use `c<Module>_<Name>` or `g<Name>` for globals.

### Naming conventions

- **Constants:** `cAutoScout_StateIdle`, `cAutoRepair_MaxBuilders`
- **Globals:** `gAutoScout_unitID[]`, `gAutoRepair_villagerQuery`
- **Functions:** `autoScout_tickUnit`, `autoRepair_tryAssign`
- **Files:** lower_snake_case (`auto_repair.xs`, `human_assist.xs`)
- **Mod folders:** lower_snake_case matching the deployed title (`idle_auto_repair`, `intelligent_auto_scout`)

### Commit message style

Conventional Commits, scoped where relevant:

```text
fix: AI players get vanilla scout/repair behaviour, not nothing
feat: restrict both mods to human players only
feat(scout): safe-corridor pathing + score refactor + release-ready READMEs
feat(scout): heat-map fixes + oracle scoring overhaul
tune(scout): area-centroid padding on heat flood-fill
chore(scout): drop dead Oracle-investigation diagnostics
```

### Branch strategy

- Default branch is `master`.
- No long-lived feature branches observed in recent history; appears to be trunk-based development with direct commits to `master`.
- No protected-branch or PR workflow detected (no `.github/` directory).

## Architecture

### Layering and dependency direction

```text
mod/{name}/game/ai/human_assist/
├── human_assist.xs      <-- vanilla copy + feature include(s) + hook call(s)
├── auto_repair.xs       <-- feature logic (included by repair and combined mods)
└── auto_scout.xs        <-- feature logic (included by scout and combined mods)
```

- `human_assist.xs` is a near-copy of the vanilla AoM:R file. Each mod adds one `include` line (and the scout mod adds one registration call inside `enableAutoScouting`).
- Feature logic lives entirely in self-contained `auto_repair.xs` and `auto_scout.xs`.
- `mod/intelligent_auto_repair_and_scout/` does **not** contain its own copies of the feature files; `scripts/deploy-mods.sh` copies them from the sibling directories at deploy time. Editing the shared files affects both the standalone and combined deployments.
- Dependency direction: feature files depend on the globals and functions declared in the vanilla `human_assist.xs` (e.g. `gReservePlan`), and on the AoM:R engine API surface.

### Public API surface

There is no textual / programmatic public API in the traditional sense. The mod's observable surface is:

- The auto-scout button behavior for `AbstractScout` units (and Atlantean Oracles).
- Idle repair behavior for eligible units when they become idle.
- The `scripts/deploy-mods.sh` entry point that copies files into the game's `mods/local/` folder.

### Test directory structure

- **No `tests/`, `__tests__/`, or `spec/` directory exists.**
- Design documentation is organized under `docs/superpowers/`:
  - `specs/` — design specs that preceded implementation.
  - `plans/` — implementation plans with checkbox task tracking.

## Testing Capability

| Capability | Status | Details |
| --- | --- | --- |
| Test runner | ❌ None | No `cargo test`, `npm test`, `pytest`, `go test`, `dotnet test`, or equivalent exists. XS has no standalone interpreter or unit-test harness. |
| Unit tests | ❌ Unavailable | AoM:R XS scripts can only be parsed/executed by the game engine. |
| Integration tests | ❌ Unavailable | Engine-required; cannot be run headlessly in this environment. |
| E2E tests | ❌ Unavailable | Would require launching the game, which is not supported in a headless workflow. |
| Coverage | ❌ Unavailable | No tooling exists. |
| Linter | ❌ None | No XS linter configured. |
| Type checker | ❌ None | XS is dynamically typed; the engine reports syntax/runtime errors only. |
| Formatter | ❌ None | Manual three-space style is used consistently. |

### Manual verification workflow

The project explicitly relies on manual verification as documented in `docs/superpowers/plans/`:

1. Edit the `.xs` source files.
2. Run `scripts/deploy-mods.sh` to copy files into the AoM:R `mods/local/` folder (game must **not** be running).
3. Launch AoM:R and load the local mod.
4. Start a match and inspect in-game behavior and `aiEcho` log output.

### Sandboxed TDD support

**Supported: false.**

A closed edit → run → assert loop inside a single shell is impossible because:

- No XS interpreter or compiler exists outside the game.
- The game engine is required to parse, load, and execute the scripts.
- Assertions can only be made by observing in-game behavior or `aiEcho` logs.

### Strict TDD decision

**`strict_tdd: false`**

Rationale:

- The project is game-modding / hack work on a proprietary data/script format.
- No deterministic, fast unit-test harness exists.
- Verification requires launching a AAA game title and observing runtime behavior.
- The existing development workflow (documented in `docs/superpowers/plans/`) is spec → plan → implement → manual deploy/verify, not red-green-refactor.

Recommended downstream workflow: spec-driven manual implementation with explicit verification checklists and in-game playback steps, not strict TDD.

## Skill Registry Snapshot

Registry source: `.atl/skill-registry.md` (already present, last updated 2026-06-06).

Skills relevant to this project and their triggers:

| Skill | Trigger | Scope | Path |
| --- | --- | --- | --- |
| `brainstorming` | Design before coding | user | `~/.config/opencode/skills/brainstorming/SKILL.md` |
| `cognitive-doc-design` | Guides, READMEs, RFCs, specs | user | `~/.config/opencode/skills/cognitive-doc-design/SKILL.md` |
| `comment-writer` | Collaboration comments | user | `~/.config/opencode/skills/comment-writer/SKILL.md` |
| `github-pr` | Pull requests and descriptions | user | `~/.config/opencode/skills/github-pr/SKILL.md` |
| `issue-creation` | Issues and bug reports | user | `~/.config/opencode/skills/issue-creation/SKILL.md` |
| `systematic-debugging` | Bugs, test failures, unexpected behavior | user | `~/.config/opencode/skills/systematic-debugging/SKILL.md` |
| `writing-plans` | Implementation plans from specs | user | `~/.config/opencode/skills/writing-plans/SKILL.md` |
| `work-unit-commits` | Reviewable commit units | user | `~/.config/opencode/skills/work-unit-commits/SKILL.md` |
| `requesting-code-review` | Fresh perspective / review | user | `~/.config/opencode/skills/requesting-code-review/SKILL.md` |
| `receiving-code-review` | Processing feedback | user | `~/.config/opencode/skills/receiving-code-review/SKILL.md` |

Project-local skills (none in `.opencode/skills/`, `.claude/skills/`, `.atl/skills/`, etc.).

## OpenSpec Layout

Created during this init:

```text
openspec/
├── config.yaml
├── specs/
├── changes/
│   └── archive/
└── sdd-init/
    └── aom_retold_mod_idle_auto_repair.md   (this file)
```

No existing `openspec/` directory was present before this run.

## Notes for Orchestrator

1. **Strict TDD is off.** The project has no automated test runner. Planning and verification must rely on specs, implementation plans, and manual deploy/log inspection checklists.
2. **Shared source files.** `auto_scout.xs` and `auto_repair.xs` are reused by the combined mod at deploy time. Any change to those files affects both the standalone and combined packages; update `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` only when the hook wiring itself changes.
3. **Vanilla overlay risk.** `human_assist.xs` in each mod is a copy of a vanilla engine file. Future game patches may invalidate the overlay; patch-maintenance notes are part of the READMEs and should be reflected in rollback plans.
4. **No CI / build / lint.** There is no automated gate. Verification artifacts (screenshots, aiEcho logs, match replays) become the audit trail.
5. **AGENTS.md exists at the project root** as the canonical agent-facing context file (stack, XS language references, mod layout, shared-file rule, deploy/verify workflow). Downstream agents should read it first; it defers to this report for deeper SDD context.
6. **Skill resolution.** The `.atl/skill-registry.md` cache is current; prefer loading skills by exact path before work. No project-local skills exist.
7. **Next step recommendation.** Because the project is established and already has a documented change workflow, the next step for a new change is `sdd-new` (propose a change). If the change is large or uncertain, route through `sdd-explore` first.
