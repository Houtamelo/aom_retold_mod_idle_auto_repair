# What this repo is

Source for houtamelo's Age of Mythology: Retold mods published on the AoM:R mod platform, plus tooling for working with the XS scripting language. See `README.md` for the player-facing changelog and compatibility matrix.

Five deployable mod packages live under `mod/`:

- `mod/idle_auto_repair/` — Idle Auto-Repair
- `mod/intelligent_auto_scout/` — Intelligent Auto-Scout
- `mod/intelligent_auto_repair_and_scout/` — combined, overlays one `human_assist.xs`
- `mod/human_assist_improvements/` — superset (also includes auto-relic-delivery); recommended single-mod choice
- `mod/Extra Ai + AoModAi/` — curated port of Garbhus's "extra ai" mod (22 files under `game/ai/core/`)

`mod/aom_autorepair_test/` and `mod/auto_scout_test/` are historical scratch, not deployed.

# Stack

## Mods (the primary deliverable)

- Language: XS — C-like scripting language loaded by the AoM:R engine. No standalone interpreter
- Source: `.xs` scripts + Markdown docs
- Build: None
- Test runner: None. XS can only be executed by the game engine
- Deploy: `scripts/deploy-mods.sh` (bash, `set -euo pipefail`) - copies mods to the user's AoM:R installation

## Tooling under `tools/`

- `tools/intellij-xs-plugin/` — IntelliJ Platform plugin (Kotlin/Gradle) for XS. Currently a thin client + file-type + TextMate highlighting + brace matcher. Was previously intended to be a full language implementation; that work was paused in favor of the LSP approach. See `openspec/changes/intellij-xs-plugin/` for the prior Kotlin plan, and `docs/xs-lsp-spike.md` for the pivot rationale.
- `tools/xs-language-server/` (planned, not yet created) — Rust LSP server (tree-sitter + tower-lsp). The "real" language implementation. The goal is to run error checks on XS without booting the game. See `docs/xs-lsp-spike.md` for the spike plan and post-spike roadmap.

## XS language reference

XS is the C-like scripting language loaded by the AoM:R engine. Consult these references before writing or reviewing `.xs`:

- `docs/xs-language-syntax.md` — language syntax (types, control flow, vector semantics, built-in operators)
- `docs/MythRMConstants.txt` and `docs/MythTRConstants.txt` — full lists of XS global constants (unit types, ages, god powers, resources, plan states, etc.). Grep these to discover valid IDs and ranges
- `~/.steam/steam/steamapps/common/Age of Mythology Retold/game` — every `.xs` script shipped by the developers, including `game/ai/human_assist/`, `game/ai/core/`, `game/ai/campaign/`, etc. Inspect freely for research, inspiration, and live examples of engine API usage. Read-only from this repo's perspective
- `docs/doxygen_retail/` — official developer-built doxygen documentation of most built-in game functions, including descriptions, signatures, and parameter semantics. When unsure about a `kb*`, `ai*`, `tr*`, or `xs*` call, search here first
- `docs/xs-lsp-spike.md` — the current pivot toward a Rust LSP for diagnostics; explains why this approach was chosen over a pure-Kotlin implementation

## Develop & verify loop

### For mods

1. Edit `.xs` source under `mod/<name>/game/`
2. Close AoM:R . Then run `./scripts/deploy-mods.sh` — copies into the game's `mods/local/` folder
3. Launch AoM:R, enable the local mod, start a match
4. Inspect in-game behaviour and `aiEcho` log output (`scripts/extract-ai-logs.sh` helps parse logs)

`AOMR_LOCAL_MODS` env var overrides the deploy target (default: Steam Deck-style Proton prefix under `~/.steam/steam/steamapps/compatdata/1934680/...`). The `.claude-sandbox.toml` mounts `~/.steam/steamapps/` RW so the deploy script works inside the sandbox

### For `tools/intellij-xs-plugin/`

1. Edit Kotlin sources under `tools/intellij-xs-plugin/src/main/kotlin/`
2. `./gradlew buildPlugin` — produces `build/distributions/intellij-xs-plugin-*.zip`
3. Install in Rider/IDEA via Settings → Plugins → ⚙️ → Install Plugin from Disk

### For `tools/xs-language-server/` (once it exists)

1. Edit Rust sources
2. `cargo build` to build
3. `cargo test` for unit tests
4. Integration test: connect an LSP client (Neovim with LSP plugin, or `vscode-languageclient` test harness) and verify diagnostics appear

## Where to look for more

- `README.md` — player-facing changelog + compatibility matrix.
- `mod/<name>/README.md` — per-mod design notes, patch-maintenance caveats, rollback notes.
- `docs/xs-lsp-spike.md` — current state of the LSP pivot (READ THIS before working on the LSP).
- `openspec/config.yaml` — SDD rules, proposal/spec/design conventions, verification method.
- `openspec/sdd-init/aom_retold_mod_idle_auto_repair.md` — full SDD-init context (stack, conventions, testing capability).
- `openspec/changes/intellij-xs-plugin/` — completed Kotlin plugin planning (P0, P0.5, P1). P1 was fully verified (PASS, 36 tests green). P2-P5 paused in favor of the LSP pivot. The Kotlin plugin remains useful as a thin client.
- `openspec/changes/` — current in-flight and archived SDD changes. A future `xs-language-server/` change will live here.
- `docs/` — proto_mods syntax, BANG docs, research notes, playtest records, XS language syntax notes.
- skill: `playtest-log-analysis/` — project-local skill for parsing AI playtest logs.
