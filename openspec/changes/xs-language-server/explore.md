# SDD Explore — `xs-language-server` (redesigned architecture)

**Goal:** Define the design space and locked architecture for a redesigned Rust LSP server that can diagnose Age of Mythology: Retold XS scripts without booting the game, using a real multi-source workspace (engine API + vanilla game folder + per-mod overlay) and proper `const`/`extern`/`mutable` semantics.

**Locked decisions (not re-litigated):**

- Crate stays at `tools/xs-language-server/` and remains the source of truth for XS language semantics.
- Engine API data is extracted from `doxygen_retail.7z` at runtime, cached by SHA-256 under `~/.local/state/aomr_lsp/v1/`.
- The LSP works in a 3-source workspace: extracted Doxygen API + the user's AoM:R game folder + one mod overlay per registered workspace folder.
- Multi-mod support is via standard `workspace/didChangeWorkspaceFolders`; each mod is independent and not merged.
- Game-folder file changes are invalidated through standard `workspace/didChangeWatchedFiles`; the server does not poll the filesystem.
- No prefix-based heuristics (`k...`, `g...`, `s...`) for semantics; use `const`, `extern`, `mutable`, and Doxygen source.
- VS Code client is out of scope; the `extracted/xs.vsix` artifact from commit `1628aa6` is removed.

**Executive summary**

The existing Rust LSP spike (weeks 1–5 on branch `xs-language-server/spike`) proves that tree-sitter + tower-lsp can parse XS, publish diagnostics, and provide completion/hover/definition. But its engine data is hard-wired to JSON files in the IntelliJ plugin resources, it has no concept of the vanilla game folder or mod overlays, and it analyses each file in isolation. This explore documents a redesigned architecture that keeps the parser and most LSP handlers but replaces the data-loading and workspace models with a Doxygen-extracted engine API, a hash-based cache, and per-mod virtual projects formed by overlaying `mod/<name>/game/` onto the game folder. The redesign also moves semantic decisions away from naming prefixes and onto XS keywords (`const`, `extern`, `mutable`). The remaining unknowns are mostly about include-root context, `mutable` redefinition rules, and how the "game folder" relates to the repo's `docs/doxygen_retail.7z`.

---

## 1. Problem statement

AoM:R XS scripts can only be validated by the game engine itself. A missing forward declaration, an `extern` collision, or a call to an unknown engine function surfaces as `Error 0310: invalid symbol lookup` when the mod loads — after the game has booted. The goal of the LSP is to run these error checks at edit time, inside the IDE, without launching AoM:R.

Real XS code is not a single-file language. A mod file such as `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`:

- Calls engine functions defined in Doxygen (`aiEcho`, `kbUnitCount`, `aiPlanCreate`, ...).
- Includes vanilla files from the game folder (`game/ai/core/core.xs`).
- Is overlaid onto the vanilla game folder at runtime, so a mod file *replaces* the vanilla file at the same relative path.
- May share symbols across files only when they are marked `extern`.
- Requires forward declarations unless the function is `mutable`.

Any useful diagnosis must therefore understand three sources (engine API, game folder, mod overlay) and XS-specific linking rules. The previous design loaded engine data from static JSON and analysed files one at a time, which cannot model these interactions.

---

## 2. Current state

### 2.1 What is already built (weeks 1–5, branch `xs-language-server/spike`)

The project under `tools/xs-language-server/` exists and compiles. The following is in place:

| Component | File(s) | What it does today |
|---|---|---|
| tree-sitter XS grammar | `tools/xs-language-server/tree-sitter-xs/grammar.js` | Forked from `tree-sitter-c`. Parses functions, rules, `extern`/`static`/`mutable`, `const`/`ref`, classes, arrays, `include "..."`, and the core C-like expression/statement grammar. It parses `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` with no syntax errors. |
| Parser wrapper | `src/parser.rs` | Thin `parse(source)` helper around the tree-sitter parser. |
| LSP server skeleton | `src/main.rs`, `src/server.rs` | `tower-lsp` over stdio. Implements `initialize`, `initialized`, `shutdown`, `didOpen`/`didChange`/`didClose`, `textDocument/completion`, `hover`, `definition`, `documentSymbol`, `references`, `rename`, and `prepareRename`. |
| Engine API models | `src/engine_api.rs` | `Syscall`, `Param`, `AiplanConstant`, `EngineApi` with exact and prefix lookups. |
| Completion | `src/completion.rs` | Prefix-matches `syscalls.json` + `aiplans.json` for engine/API completion items. |
| Hover / go-to-definition | `src/server.rs` | Engine symbols return generated `xs-stub://engine/<name>` URIs; workspace symbols resolve to real locations. |
| Per-file symbol table | `src/symbols.rs` | Extracts rules, functions, variables, and `const` declarations from a parse tree. |
| Diagnostics | `src/diagnostics.rs`, `src/typecheck.rs` | Reports `ERROR`/`MISSING` parse nodes; type-checks argument count and argument type for engine syscalls (`ai*`, `kb*`, `tr*`, `xs*`, `rm*`). |
| References / rename | `src/references.rs`, `src/word.rs` | Finds identifier occurrences in one file and produces rename edits. |
| Test binaries | `src/bin/*.rs` | Probes (`day1_probe.rs`, `inspect_tree.rs`, `dump_top_level.rs`) and an end-to-end `lsp_roundtrip_test.rs` that exercises the full LSP message sequence. |

Cargo dependencies already include `tower-lsp`, `tokio`, `serde`/`serde_json`, `tree-sitter`, and local `tree-sitter-xs`.

### 2.2 What is wrong with the current design

1. **Engine data is vendored and loaded from a sibling-crate path.**
   - `EngineApi::load_default()` reads `../intellij-xs-plugin/src/main/resources/{syscalls,aiplans}.json`.
   - Those JSON files are remnants of the VS Code extension / IntelliJ plugin, not generated by the LSP.
   - If the plugin folder is absent or the JSON is stale, the server silently starts with an empty engine API (`unwrap_or_default()`).

2. **No game-folder integration.**
   - The server does not know where the AoM:R game folder is.
   - It cannot resolve `include "core/core.xs"`, cannot see vanilla symbols, and cannot model the mod-overlay behavior.
   - Diagnostics therefore miss cross-file forward-declaration errors and `extern` collisions.

3. **No mod-overlay / multi-mod model.**
   - All analysis is per-file inside a flat `HashMap<Url, SymbolTable>`.
   - There is no concept of "this file belongs to mod X", so the server cannot compute a virtual project of `game folder + mod/game/overlay`.
   - Multiple mods would be merged rather than isolated.

4. **Prefix-based assumptions in early documentation still linger.**
   - `docs/xs-lsp-spike.md` incorrectly documents `k[A-Z]\w*` as game-state constants, `g[A-Z]\w*` as globals, and `s[A-Z]\w*` as statics.
   - The actual XS engine does not enforce these prefixes. The redesigned server must derive constants from the `const` keyword or from Doxygen, and cross-file sharing from `extern`.
   - The current `engine_api.rs` / `completion.rs` were shaped under that prefix model and do not yet implement the `const`/`extern`/`mutable` semantic rules.

5. **No runtime invalidation of game files.**
   - The server never receives `workspace/didChangeWatchedFiles` and does not cache per-file parse results.
   - Editing a vanilla file would leave stale diagnostics in dependent mod files.

### 2.3 What is frozen or to be removed

- **`extracted/xs.vsix` and the VS Code client are removed from scope.**
  - Commit `1628aa6` on `master` added `extracted/xs.vsix` (a VS Code extension converted to an LSP client).
  - The redesign does not include a VS Code client; the file should be removed or left stranded in history.
- **No server-side filesystem polling.**
  - All game-folder change notifications come from the LSP client via `workspace/didChangeWatchedFiles`.
- **No implicit forward declarations.**
  - The LSP must enforce the same forward-declaration rule as the engine.
- **No global cross-mod sharing.**
  - Workspace folders for different mods are analysed as independent virtual projects.

---

## 3. User-confirmed architecture (restatement)

### 3.1 LSP server startup and engine-data pipeline

- The server obtains the "game folder" path from, in order:
  1. a `--game-path` CLI argument,
  2. a positional CLI argument,
  3. the `AOMR_GAME_PATH` environment variable.
- The path is **required**. If it is missing, or if `doxygen_retail.7z` is not found beneath it, the server exits immediately with a descriptive error.
- The SHA-256 hash of `doxygen_retail.7z` is the cache key.
- On cache miss, the server extracts the 7z archive and parses the relevant Doxygen HTML pages (`aifuncs_8cpp.html`, `kbfuncs_8cpp.html`, `randommapfuncs_8cpp.html`, `triggerfuncs_8cpp.html`, `triggerfuncs__mainthread_8cpp.html`, `xsfuncs_8cpp.html`, `aiplans_8cpp.html`, etc.) into a single JSON document containing syscalls, AI-plan constants, and help text.
- The extracted JSON is stored at `~/.local/state/aomr_lsp/v1/<sha256>.json` using the `dirs` crate.
- On cache hit, the server loads the JSON directly and skips extraction.

### 3.2 Workspace model

- The server speaks LSP 3.17 and supports `workspace/didChangeWorkspaceFolders`.
- Each workspace folder URI represents one mod root, typically `mod/<name>/`.
- The server internally maps `mod/<name>/game/` to that mod's overlay directory.
- Recursive scanning stops at `game/` boundaries; there are no mod-in-mod nests.
- Each mod is an independent **virtual project**:
  - sources in `mod/<name>/game/` overlay the vanilla game folder by relative path,
  - an overlaid file hides the matching vanilla file for every analysis consumer,
  - symbols are not shared across mods.

### 3.3 Per-file analysis

- When a `.xs` file is opened or changed, the server determines which registered mod URI prefix owns the file.
- If no mod owns the file, the server emits `window/showMessage`: "file not part of any registered mod; engine API only", and only offers engine-API based features.
- If a mod owns the file, the server builds a virtual project = game folder + that mod's `game/` overlay and runs:
  - parse diagnostics,
  - multi-file symbol resolution,
  - type checking,
  - forward-declaration validation,
  - `extern` collision detection.
- `include "foo.xs"` resolves in the mod's directory first, then in the game folder.
- Workspace-symbol queries (`workspace/symbol`) are scoped to the owning mod.

### 3.4 Engine semantics enforced by the LSP

| Engine rule | LSP behavior |
|---|---|
| `extern` collision | **Error** when any file declares `extern X` and another file declares/definies `X`. |
| Use before definition | **Error** unless the function is `mutable` or has a forward declaration before the call site. |
| Missing forward declaration | **Error**, matching engine `Error 0310: invalid symbol lookup`. |
| Variable visibility | Non-`extern` variables are file-local; autocompletion must hide them from other files. |
| Same identifier in different files | `static`/file-local definitions with the same name in different files are **OK**. |
| `mutable` | Forward-callable and may be redefined later with the same signature. |
| Constants | Derived from Doxygen or `const` declarations only; naming prefixes are ignored. |

### 3.5 File watching and caching

- The server registers a `workspace/didChangeWatchedFiles` watcher for `<game>/**/*.xs`.
- The IntelliJ client watches the game folder and forwards change notifications.
- Each notification invalidates the cached parse result for the changed file (`game_parse/v1/<mtime>-<sha256>.json`).
- The server performs no server-side file-system polling.

### 3.6 Performance budgets

- Cold start: < 10 s (one-time 7z extraction + Doxygen parse + JSON serialization).
- Warm start: < 1 s (hash + cache load).
- Per-keystroke diagnostic latency: < 200 ms.

### 3.7 IntelliJ plugin responsibilities

The Kotlin plugin under `tools/intellij-xs-plugin/` becomes a thin LSP client:

- Keep: `XsLanguage`, `XsFileType`, file-type registration, TextMate bundle (`syntaxes/xs.tmLanguage.json`), brace matcher, commenter, quote handler, surrounding pairs.
- Remove or no longer maintain: engine data classes, completion contributor, documentation provider, parameter-info handler, engine reference/stub contributors, PSI lexer/parser work.
- Add:
  - Settings UI: game folder text field, mod list (+ / – buttons), auto-detect button.
  - Auto-detect: recursively scan the project for folders named exactly `game`; stop recursion at each `game/` boundary; each found `game` folder's parent is added as a mod root. Runs only on an empty mod list. Warn if nothing found.
  - LSP lifecycle: `StartupActivity.DumbAware` spawns the Rust LSP with `--game-path` and sends `workspace/didChangeWorkspaceFolders` with the detected mod URIs.
  - Register `didChangeWatchedFiles` watchers for `<game>/**/*.xs`.
  - Restart the LSP or send updated workspace-folder events when settings change.

---

## 4. Existing code to preserve vs. rework

| Code | Verdict | Notes |
|---|---|---|
| `tree-sitter-xs` grammar and `parser.rs` | **Preserve** | Already supports `include`, `extern`/`static`/`mutable`, `const`/`ref`, classes, rules, and C-like expressions. May need minor adjustments for include-root hints. |
| `server.rs` LSP handlers | **Preserve + rework** | `completion`, `hover`, `definition`, `documentSymbol`, `references`, `rename`, `prepareRename` logic stays; workspace-folder tracking, virtual-project selection, watched-file invalidation, and `window/showMessage` for unowned files must be added. |
| `main.rs` | **Rework** | Replace the bare stdio setup with CLI/env parsing for `--game-path` and mandatory engine-data load. |
| `engine_api.rs` data models | **Preserve** | `Syscall`, `Param`, `AiplanConstant` structures are still the right shape for Doxygen-extracted data. |
| `engine_api.rs` loader | **Replace** | Move from JSON-in-sibling-path to 7z extraction + hash-based cache. |
| `symbols.rs` | **Extend** | Per-file extraction is solid, but it must record `extern`, `mutable`, and forward declarations, distinguish file-local vs. exported symbols, and feed a cross-file index. |
| `diagnostics.rs` | **Extend** | Add `extern` collision, missing forward declaration, and undefined-symbol diagnostics in addition to parse errors. |
| `typecheck.rs` | **Extend** | Existing engine-call count/type checks are reusable; need cross-file function type lookup, int↔float compatibility, and integration into the virtual project. |
| `completion.rs` | **Rework** | Must combine engine API + exported project symbols, hide non-`extern` variables outside their file, and resolve includes for workspace completion. |
| `references.rs` / `word.rs` | **Extend** | Per-file identifier walks are fine; expand to workspace-wide references once cross-file indexing exists. |
| `tools/intellij-xs-plugin/` Kotlin code | **Rework as client** | Keep file type + TextMate + editor helpers. Drop engine-facing PSI contributors. Add settings + LSP lifecycle. |

### 4.1 Known technical debt from the old prefix model

- `engine_api.rs` currently indexes syscalls/aiplans by name prefix for completion. That is fine for filtering, but the redesign must not use `k`/`g`/`s` prefixes to decide whether an identifier is a constant, global, or static.
- `completion.rs` returns all engine API items for any identifier prefix. It does not yet suppress file-local variables outside their file or prefer `extern` symbols. That scope logic must be added.
- `symbols.rs` classifies `const` declarations as constants but does not yet treat `extern` as a visibility/export marker across files. The new indexer must.

---

## 5. Open questions remaining

These decisions are locked unless the user pushes back, but the following details still need clarification before spec/design:

1. **Game-folder vs. Doxygen 7z location.** The locked architecture says `--game-path` must contain `doxygen_retail.7z`. In this repo the 7z lives at `docs/doxygen_retail.7z`, not inside the AoM:R install. Should `--game-path` actually point to the repository root (or a directory containing `docs/doxygen_retail.7z`), or do we expect a copy/symlink of the 7z inside the game folder?

2. **Include-root context.** XS `include` roots differ by runtime context: AI scripts resolve under `game/ai`, trigger scripts under `game/data/trigger`, random-map scripts under `game/random_maps`. How does the LSP know the context of a file? Should it infer from the relative path (`game/ai/...` vs `game/data/trigger/...`), or should the mod/workspace configuration declare an include root?

3. **`mutable` redefinition equality.** The engine allows `mutable` functions to be redefined "with the same signature." Does "same signature" include default parameter values, or only name + parameter types? The LSP needs a precise rule to avoid false positives.

4. **`extern` collision edge cases.** If file A has `extern int gX = -1;` and file B has `int gX = -1;` (no `extern`), is that an error? The locked rule says yes. What if both files have `extern int gX = -1;` — is that allowed, or does it count as multiple definitions?

5. **Cross-file symbol resolution order for includes.** If `fileA.xs` includes `fileB.xs`, do symbols defined in `fileB` become visible in `fileA` regardless of `extern`? The XS engine requires `extern` for variables but functions are visible across files if defined before use. The LSP should model this exactly; confirm the rule for included files vs. workspace-wide files.

6. **Combined mod and shared source files.** `mod/intelligent_auto_repair_and_scout/` reuses `auto_repair.xs` and `auto_scout.xs` from sibling mods at deploy time, but at source time those files live under `mod/idle_auto_repair/game/` and `mod/intelligent_auto_scout/game/`. How should the LSP present these shared files to the combined mod's virtual project? Should the user add the source folders of the sibling mods as additional workspace folders, or should the combined mod's overlay include symlinks/copies?

7. **Watcher registration fallback.** If the LSP client (or a particular IntelliJ version) does not support dynamic watcher registration or `didChangeWatchedFiles`, should the server degrade gracefully (stale game-folder diagnostics) or force a less efficient full-project re-parse on focus?

8. **Cache garbage-collection policy.** Old cache versions are GC'd automatically. Should we keep the last N versions, delete everything except the current hash, or time-bucket by age?

9. **Doxygen extraction schema.** Should the extracted JSON keep the existing `syscalls.json` / `aiplans.json` shape, or do we want a merged schema with a single top-level object? Keeping the current shape minimizes changes to `EngineApi` consumers.

10. **Type-checking int vs. float.** The current `typecheck.rs` requires exact type matches. The XS engine allows `int` in `float` contexts (and vice versa) for arithmetic. Should the LSP allow implicit int↔float widening, or stay strict?

---

## 6. Risks

| # | Risk | Impact | Mitigation |
|---|---|---|---|
| 1 | **7z extraction is unproven.** We have not validated that `sevenz-rust` can decompress `docs/doxygen_retail.7z` within the cold-start budget, nor that `scraper` can reliably parse the Doxygen 1.15 HTML. | High until spiked | Add aDay-0 spike task: extract and parse the 7z, compare output counts against the committed JSON, measure time. Keep `syscalls.json` / `aiplans.json` as a build-time fallback only if extraction fails. |
| 2 | **Doxygen HTML format drift.** Game patches may change Doxygen output, breaking the scraper. | Medium | Tolerant scraping: skip malformed entries, log warnings, never emit empty output. Add validation that critical source files produce expected syscall counts. |
| 3 | **Virtual-project overlay complexity.** Correctly hiding replaced vanilla files and resolving `include` across game + mod requires careful path bookkeeping. | Medium | Separate overlay resolution into a dedicated module with unit tests for relative-path mapping, file replacement, and include lookup. |
| 4 | **Forward-declaration / `extern` false positives.** If the LSP's semantic model diverges from the engine, users see errors that the game does not, or miss real errors. | High | Extensive fixtures covering forward declarations, `mutable`, `extern` collision, cross-file includes, and static file-local symbols. Compare against real game load behavior. |
| 5 | **Per-keystroke latency on large virtual projects.** A mod overlay plus the full game folder could be thousands of files. | Medium | Cache per-file parse results (`game_parse/v1/<mtime>-<sha256>.json`), only re-parse changed files, and compute symbol tables incrementally. |
| 6 | **IntelliJ client capability gaps.** Not all IntelliJ/Rider builds support dynamic watcher registration or workspace-folder changes the same way. | Medium | Feature-detect client capabilities at `initialize`; degrade to manual "refresh" action or LSP restart when a capability is missing. |
| 7 | **No automated XS execution for verification.** We cannot run the game in CI, so semantic diagnostics cannot be compared against the engine automatically. | Medium | Build a fixture corpus from known-good and known-bad game/mod files, capture real engine error messages, and assert the LSP produces matching diagnostics manually. |
| 8 | **Cache directory permissions / portability.** `dirs::state_dir()` may not exist or may not be writable in all environments. | Low | Fall back to `~/.aomr_lsp` if `dirs` returns `None`; document the path; make extraction idempotent. |
| 9 | **Legacy prefix assumptions in existing code/tests.** Even though the redesign removes prefix heuristics, old tests and docs may still assume `k`/`g`/`s` semantics. | Low | Audit `tools/xs-language-server/src/` for any prefix-based filters and update test fixtures to use `const`/`extern`/`mutable` explicitly. |

---

## 7. References

- `AGENTS.md` — project overview, stack, XS language references, deploy/verify loop.
- `docs/xs-lsp-spike.md` — original pivot plan. **Note:** contains incorrect `k`/`g`/`s` prefix descriptions; use the locked architecture in this doc instead.
- `docs/xs-language-syntax.md` — XS syntax, modifiers, forward-declaration rule, `include` semantics.
- `docs/doxygen_retail/` and `docs/doxygen_retail.7z` — upstream engine API documentation and archive.
- `tools/xs-language-server/src/` — current Rust LSP implementation.
- `tools/xs-language-server/Cargo.toml` — current dependencies.
- `tools/xs-language-server/tree-sitter-xs/grammar.js` — current tree-sitter XS grammar.
- `tools/intellij-xs-plugin/src/main/kotlin/` — current IntelliJ plugin (to become LSP client).
- `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` — vendored TextMate grammar.
- `mod/*/game/ai/human_assist/human_assist.xs` — representative mod overlay files.
- `scripts/deploy-mods.sh` — shows how combined mod reuses sibling feature files.
- `openspec/changes/intellij-xs-plugin/` — prior SDD artifacts for the IntelliJ plugin (format/style reference).
- `openspec/config.yaml` — SDD rules for this project.
- `openspec/sdd-init/aom_retold_mod_idle_auto_repair.md` — full SDD-init context and testing capability.

---

## 8. Recommendation

The design space is understood and the architecture is locked. The next SDD phase is **`sdd-propose`**, because the user has already confirmed the high-level design and the remaining work is to document intent, scope, success criteria, and rollback before writing specs. If the open questions around the 7z location or include-root context change the design materially, revisit them during the proposal; otherwise they can be resolved in spec phase.
