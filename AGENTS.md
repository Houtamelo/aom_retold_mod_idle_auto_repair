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

- `tools/xs-language-server/` — Rust LSP workspace (now `tower-lsp` + **lelwel typed AST**; tree-sitter fully dropped on 2026-07-03). Three members:
  - `tools/xs-language-server/xs-parser/` — typed AST crate (`xs_parser`). Grammar is `src/xs.llw` (lelwel). Output: 18-variant `Expr`, `Statement`, `Declaration`, `TopLevelItem` hierarchies plus `TranslationUnit::from_cst`. 176 unit tests.
  - `tools/xs-language-server/lsp/` — LSP server crate. Handlers walk the typed AST via `xs_parser` (path dep). 157 unit tests passing + 35 environmental failures.
  - `tools/xs-language-server/tree-sitter-xs/` — **legacy** tree-sitter grammar. Orphaned after Phase 4 PR-C; can be deleted in a separate cleanup commit. New work targets `xs-parser/`.
  
  The LSP server diagnoses mod scripts using a 3-source workspace: engine API extracted from `doxygen_retail.7z`, the vanilla AoM:R `game/` folder, and per-mod `game/` overlays. See `openspec/changes/archive/2026-07-02-rich-typed-ast-layer/` for the typed AST layer change and `openspec/changes/archive/2026-07-03-2026-07-04-phase4-lsp-typed-ast-wiring/` for the LSP-wiring change.
- `tools/intellij-xs-plugin/` — IntelliJ Platform plugin (Kotlin/Gradle) for XS. Now a thin LSP client that provides file-type registration, TextMate syntax highlighting, brace matching, and editor helpers. It also provides a settings page for the LSP server connection and can auto-detect mod roots.

## XS language reference

XS is the C-like scripting language loaded by the AoM:R engine. Consult these references before writing or reviewing `.xs`:

- `docs/xs-language-syntax.md` — language syntax (types, control flow, vector semantics, built-in operators)
- `docs/MythRMConstants.txt` and `docs/MythTRConstants.txt` — full lists of XS global constants (unit types, ages, god powers, resources, plan states, etc.). Grep these to discover valid IDs and ranges
- `~/.steam/steam/steamapps/common/Age of Mythology Retold/game` — every `.xs` script shipped by the developers, including `game/ai/human_assist/`, `game/ai/core/`, `game/ai/campaign/`, etc. Inspect freely for research, inspiration, and live examples of engine API usage. Read-only from this repo's perspective
- `docs/doxygen_retail/` — official developer-built doxygen documentation of most built-in game functions, including descriptions, signatures, and parameter semantics. When unsure about a `kb*`, `ai*`, `tr*`, or `xs*` call, search here first
- `docs/xs-lsp-spike.md` — historical spike plan that led to the Rust LSP pivot. Now superseded by `openspec/changes/archive/xs-language-server/`; kept as a record of the original investigation.
- `openspec/changes/archive/xs-language-server/` — SDD artifacts for the redesigned LSP (proposal, design, specs, apply progress, verify report).

## XS quirks and pitfalls

- **Forward declarations are required.** XS does NOT support implicit forward declarations like C/C++. A function must be either defined before it is called, OR declared (signature only) with a trailing semicolon earlier in the file. A function marked `mutable` is the only exception — it can be redefined later and is forward-callable. See `docs/xs-language-syntax.md` → "Forward declarations" for the full rule.
  - **How to detect** before deploying: for every user-defined function in a touched `.xs`, find its definition line and check that no call site has a smaller line number. A one-shot scan:
    ```bash
    file=mod/<name>/game/ai/human_assist/<name>.xs
    grep -nE '^(void|int|bool|float|string|extern|mutable)[ \t]+[A-Za-z_][A-Za-z0-9_]*[ \t]*\(' "$file" \
      | awk -F'[ \t:()]+' '{print $2, $1}' \
      | sort -k1,1 -u \
      | while read fn def; do
        early=$(grep -nE "[^A-Za-z0-9_]$fn[ \t]*\(" "$file" | awk -F: -v d="$def" '$1 < d')
        [ -n "$early" ] && echo "MISSING FWD-DECL: $fn (def line $def)"
      done
    ```
  - **Fix pattern:** add a forward-declaration block after the `extern` block, mirroring every signature with a trailing `;`:
    ```xs
    extern int gMyGlobal = 0;

    void autoScout_helperA(int x = -1);
    int  autoScout_helperB(int y = -1, int z = -1);

    // ... original code, possibly calling helperA/helperB ...

    void autoScout_helperA(int x = -1) { /* ... */ }
    int  autoScout_helperB(int y = -1, int z = -1) { /* ... */ }
    ```
  - **Failure mode when missed:** engine refuses to load the mod with `Error 0310: invalid symbol lookup` at game start. Only visible by booting AoM:R — there is no standalone XS compiler.
  - **Lesson learned (engine-explore-migration):** when an SDD apply produces 7+ commits across 1000+ lines of XS, run the scan above before declaring "Deviations from Design: None." That change claimed no deviations but actually introduced 8 forward-declaration bugs in `mod/intelligent_auto_scout/game/ai/human_assist/auto_scout.xs`.

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

The plugin is now a thin LSP client. Settings live under **Settings → Languages & Frameworks → XS**:

- **Game folder** is stored globally (per IDE installation). Point it at the AoM:R install root (the directory containing `game/` and `doxygen_retail.7z`).
- **Mod paths** are stored project-local. Add them manually or click **Auto-detect mods**.
- **Auto-detect** recursively scans the project for directories named exactly `game` and stops recursion at each `game/` boundary. The parent of each found `game/` directory is added as a mod root.

### Plugin version bump policy

**Always bump `pluginVersion` in `tools/intellij-xs-plugin/gradle.properties` for any commit that affects the bundled plugin artifact.** The plugin artifact is affected by changes to **any** of:

- `tools/intellij-xs-plugin/` — Kotlin sources, `plugin.xml`, `build.gradle.kts`, `gradle.properties`, `package.json`, `syntaxes/xs.tmLanguage.json`
- `tools/xs-language-server/` — Rust sources under `lsp/src/` and `xs-parser/src/`, `Cargo.toml`, `Cargo.lock`
- `tools/intellij-xs-plugin/src/main/resources/bin/` — bundled LSP binary staging area (gitignored; replaced at build by `copyLspServerToResources`)

The `copyLspServerToResources` Gradle task always picks up whatever LSP release binary is on disk. Without a version bump, two `.zip` files can have the same `pluginVersion=0.1.2` but contain different LSP binaries — users have no way to tell which is which.

**Bump strategy:**

| Change kind                                   | Bump           | Example       |
| -------------------------------------------- | -------------- | ------------- |
| Bug fix (no new features, no API change)        | Patch           | 0.1.2 → 0.1.3 |
| New LSP feature, new settings UI, new grammar scope, new test coverage | Minor | 0.1.x → 0.2.0 |
| Breaking API change, Kotlin/Gradle/IntelliJ Platform major version bump | Major | 0.x.0 → 1.0.0 |

The plugin's `build.gradle.kts:12` picks up the version via `providers.gradleProperty("pluginVersion").get()`, so changing `gradle.properties` is enough — no other file needs editing.

#### Plugin test fixture gotcha

`BasePlatformTestCase` tests for `.xs` files should identify buffers by **file extension** (`virtualFile.extension == "xs"`) rather than `PsiElement.language`. In the headless fixture used by `./gradlew :test`, `configureByText("a.xs", ...)` produces PSI leaves whose `language` is reported as the platform `TEXT` language, even though the file parses through the XS `ParserDefinition`.

### For `tools/xs-language-server/`

**Rust toolchain**: this workspace requires Rust **nightly**. `tools/xs-language-server/rust-toolchain.toml` pins `channel = "nightly"`; cargo auto-activates it from anywhere in that subdirectory. Reasons: `scraper 0.27` and `fluent-uri 0.4` use `let`-chains (`if let X && let Y`), stabilized in Rust 1.88, so the system's `apt`-installed rustc 1.85 cannot compile this crate. The sandbox installs rustup + nightly system-wide via `.claude-sandbox.deps.sh`; outside the sandbox, install rustup and let `rust-toolchain.toml` do the rest.

1. Edit Rust sources under:
   - `tools/xs-language-server/lsp/src/` — LSP handlers (symbols, semantic_tokens, references, definition_check, typecheck, diagnostics, server). Public crate name `xs_language_server`.
   - `tools/xs-language-server/xs-parser/src/` — typed AST + grammar. Public crate name `xs_parser`.
2. `cargo build --manifest-path tools/xs-language-server/Cargo.toml` to build
3. `cargo test --manifest-path tools/xs-language-server/Cargo.toml` for the workspace suite. As of 2026-07-03 (post-Phase 4): **157 passed / 35 environmental failures** in the LSP crate + **176 passed** in the xs-parser crate. The 35 environmental failures are cwd-relative `tools/docs/doxygen_retail.7z` lookups in some unit tests that resolve correctly only from the repo root. They are pre-existing and not regressions.
4. `cargo run --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test` for the end-to-end LSP message sequence (`initialize` / `initialized` / `didOpen` / `shutdown` / `exit`). Requires `AOMR_GAME_PATH` to be set to the install root.
5. **Game folder integration test** (optional, requires the game installed):
   ```bash
   AOMR_GAME_PATH=/path/to/Age\ of\ Mythology\ Retold \
     cargo test --manifest-path tools/xs-language-server/Cargo.toml \
       --test game_folder_parse -- --nocapture
   ```
   Walks `game/**/*.xs` via the typed AST, asserts no unexpected parse errors, asserts every file resolves through the `Workspace`. After Phase 4, the test uses the typed AST's `xs_parser::Diagnostic` severity instead of tree-sitter ERROR-node counting. The test skips cleanly if `AOMR_GAME_PATH` is not set, so plain `cargo test` on CI still passes.
6. Integration test: open the IntelliJ plugin and verify diagnostics arrive for a mod `.xs` file. The LSP writes its log to `<project>/.idea/xs-lsp.log` (stderr is redirected there — was previously merged into stdout and silently dropped).

The server caches extracted engine API data under `~/.local/state/aomr_lsp/v2/` (or `~/.aomr_lsp/v2/` if `XDG_STATE_HOME` is unavailable). The cache key is the SHA-256 of `doxygen_retail.7z`; warm starts load the cached JSON directly instead of re-extracting the archive. The `v2/` schema was introduced when the legacy JSON backfill was removed in Phase 5; older `v1/` cache files are ignored.

**Smoke-testing the LSP end-to-end against a synthetic file:**

```bash
AOMR_GAME_PATH="/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold" \
  cargo build --release --manifest-path tools/xs-language-server/Cargo.toml --bin xs-language-server
# Then in another shell, pipe LSP JSON-RPC frames (Content-Length framing) to the binary.
# A round-trip test (initialize → didOpen → documentSymbol → definition → shutdown → exit)
# confirms every typed-AST handler responds correctly.
```

## Where to look for more

- `README.md` — player-facing changelog + compatibility matrix.
- `mod/<name>/README.md` — per-mod design notes, patch-maintenance caveats, rollback notes.
- `docs/xs-lsp-spike.md` — historical spike plan that led to the Rust LSP pivot.
- `openspec/config.yaml` — SDD rules, proposal/spec/design conventions, verification method.
- `openspec/sdd-init/aom_retold_mod_idle_auto_repair.md` — full SDD-init context (stack, conventions, testing capability).
- `openspec/changes/intellij-xs-plugin/` — completed Kotlin plugin planning (P0, P0.5, P1). P1 was fully verified (PASS, 36 tests green). P2-P5 paused in favor of the LSP pivot. The Kotlin plugin is now a thin LSP client.
- `openspec/changes/archive/xs-language-server/` — historical SDD changes for the original tree-sitter LSP.
- `openspec/changes/archive/2026-07-02-rich-typed-ast-layer/` — Phase 1-3 of the tree-sitter → lelwel migration. Adds the typed AST layer (`xs-parser` crate, 176 unit tests).
- `openspec/changes/archive/2026-07-03-fix-class-specifier-no-trailing-semi/` — small grammar fix unblocking class definitions.
- `openspec/changes/archive/2026-07-03-fix-rule-body-extraction/` — small grammar fix unblocking rule body parsing + introducing `TypeTable` API for Phase 4.
- `openspec/changes/archive/2026-07-03-2026-07-04-phase4-lsp-typed-ast-wiring/` — Phase 4: chained PRs (PR-A symbols.rs rewrite + xs-parser rename, PR-B handlers + span_to_range, PR-C drop tree-sitter). 9 commits on branch `xs-lsp-roundtrip-followup`.
- `docs/` — proto_mods syntax, BANG docs, research notes, playtest records, XS language syntax notes.
- skill: `playtest-log-analysis/` — project-local skill for parsing AI playtest logs.
