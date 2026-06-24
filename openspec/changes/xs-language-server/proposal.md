# SDD Proposal — `xs-language-server` (workspace & engine-data redesign)

**Status:** Ready for spec  
**Project:** `aom_retold_mod_idle_auto_repair`  
**Change name:** `xs-language-server`  
**Current dev branch:** `xs-language-server/spike`  
**Redesign branch:** new branch off `spike` (proposed `xs-language-server/redesign`)  

---

## 1. Title and summary

**Redesign the Rust XS Language Server so it can diagnose Age of Mythology: Retold mod scripts using a real multi-source workspace — engine API extracted from Doxygen, the vanilla game folder, and per-mod overlays — without booting the game.**

The existing spike proves tree-sitter + tower-lsp can parse XS and power completions, hover, and diagnostics. What it cannot do is understand how a mod file overlays the vanilla game folder, share symbols across files under XS linking rules, or validate calls against the real engine API. This change replaces the server's data-loading and workspace models while preserving its parser and most LSP handlers. The result is an LSP that finds missing forward declarations, `extern` collisions, and unknown engine calls at edit time.

---

## 2. Why

- **User-facing problem:** XS errors such as `Error 0310: invalid symbol lookup` or missing forward declarations only appear after AoM:R boots. Modders currently validate by launching the game.
- **Why the current design fails:** It loads engine data from sibling JSON resources, analyses files in isolation, has no concept of the game folder or mod overlay, and relied on incorrect naming-prefix heuristics.
- **Why a redesign now:** The parser and core LSP handlers are proven. The missing pieces — Doxygen extraction, a hash-based cache, virtual projects, and `const`/`extern`/`mutable` semantics — are well-understood and user-confirmed.

---

## 3. What changes

| Area | Change |
|------|--------|
| Engine data pipeline | Extract API from `<game_path>/doxygen_retail.7z` at runtime, cache by SHA-256 in `~/.local/state/aomr_lsp/v1/<sha>.json`. |
| Game path resolution | Server reads `--game-path`, positional arg, or `AOMR_GAME_PATH`. Path points to the AoM:R **install root** (parent of `game/`); server expects `<game_path>/doxygen_retail.7z` and `<game_path>/game/**/*.xs`. Missing archive → immediate exit. |
| Workspace model | 3-source virtual project per mod: extracted Doxygen API + vanilla `game/` folder + one `mod/<name>/game/` overlay. |
| Multi-mod support | Standard `workspace/didChangeWorkspaceFolders`; each workspace folder is an independent virtual project. |
| Overlay semantics | Mod file at relative path `R` hides vanilla file at `game/R` for analysis, completion, and diagnostics. |
| File watching | Client-side `workspace/didChangeWatchedFiles` for `<game_path>/game/**/*.xs`; no server polling. |
| Semantics | Diagnose `extern` collisions, use-before-definition, missing forward declarations, and engine-call type/count errors using `const`, `extern`, `mutable`, and Doxygen. No prefix-based classification. |
| IntelliJ plugin | Re-focus as a thin LSP client: keep file type / TextMate / brace matcher; add game-folder settings, auto-detect mods, LSP lifecycle, and file watchers. |

---

## 4. What does NOT change

- `tree-sitter-xs` grammar and `src/parser.rs`.
- Week 1–5 LSP handlers (`completion`, `hover`, `definition`, `documentSymbol`, `references`, `rename`, `prepareRename`).
- Core data models in `src/engine_api.rs` (`Syscall`, `Param`, `AiplanConstant`).
- IntelliJ file-type registration, TextMate grammar (`xs.tmLanguage.json`), brace matcher, commenter, quote handler.
- Mod packages under `mod/` and deploy tooling (`scripts/deploy-mods.sh`).

---

## 5. Scope

### 5.1 In scope

- Rust LSP server under `tools/xs-language-server/`:
  - CLI/env parsing for game path.
  - 7z extraction and SHA-256-cached engine API JSON.
  - Virtual-project workspace with overlay resolution.
  - Cross-file symbol index and XS semantic diagnostics.
  - `workspace/didChangeWorkspaceFolders` and `workspace/didChangeWatchedFiles` handling.
- IntelliJ plugin under `tools/intellij-xs-plugin/`:
  - Settings page for game folder and mod list.
  - Auto-detect mod roots by scanning for `game/` folders.
  - LSP client lifecycle and file watchers.
- Vendored TextMate grammar at `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`.

### 5.2 Out of scope

- VS Code client (remove `extracted/xs.vsix`; week 7 commit `1628aa6` is reverted or stranded).
- Mod packaging, deployment, or in-game log parsing.
- Full type inference, constant folding, or proof-of-equivalence with the XS engine beyond the listed semantic rules.
- Marketplace publishing for the IntelliJ plugin.

### 5.3 Capabilities introduced

New specs to create:

- `xs-lsp-engine-data-loading`: 7z extraction, Doxygen scraping, hash-based cache, CLI/env game-path resolution.
- `xs-lsp-workspace-overlay`: virtual-project construction, file ownership, overlay lookup, `didChangeWorkspaceFolders`, `didChangeWatchedFiles`.
- `xs-lsp-semantic-diagnostics`: forward-declaration, `extern` collision, cross-file visibility, mutable redefinition, engine-call type checking.
- `xs-lsp-intellij-client`: settings UI, auto-detect, LSP lifecycle, watcher registration.

Modified capabilities: None.

---

## 6. Success criteria

- [ ] Cold start (cache miss) completes in **< 10 s** on a representative machine.
- [ ] Warm start (cache hit) completes in **< 1 s**.
- [ ] Per-keystroke diagnostic latency stays **< 200 ms** for the representative mod files.
- [ ] Engine-enforced semantics produce correct diagnostics: forward declarations, `extern` collisions, use-before-definition, file-local variables, `mutable` redefinition, engine-call argument count/type.
- [ ] Opening a `.xs` file inside a registered mod shows diagnostics in IntelliJ/Rider via the LSP client.
- [ ] Opening a `.xs` file outside any registered mod shows an engine-API-only message and still offers engine-based completion/hover.

---

## 7. Non-goals

- A VS Code extension or client.
- Replacing the existing deploy script or packaging mods.
- Achieving 100 % type-system parity with the engine (only the listed semantics are enforced).
- Polling the filesystem from the server.
- Global cross-mod symbol sharing.
- Merging multiple mods into a single analysis project.

---

## 8. User experience

1. Install the IntelliJ plugin from disk.
2. Open **Settings → Languages & Frameworks → XS** and set the AoM:R install root (the folder containing `game/` and `doxygen_retail.7z`).
3. Click **Auto-detect mods**: the plugin scans the project for `game/` directories and adds each parent folder as a mod workspace folder.
4. (Optional) Add or remove mod folders manually.
5. Open any `.xs` file under a registered mod.
6. Diagnostics, completion, hover, and go-to-definition appear automatically using the engine API + game folder + mod overlay.

If the file is not under any registered mod, the IDE shows a non-blocking warning and the server falls back to engine-API-only features.

---

## 9. Open questions

These need user confirmation or a spec-time decision:

1. **Include-root context.** Should the LSP infer context from the file's relative path (`game/ai/...`, `game/data/trigger/...`, `game/random_maps/...`) or require a per-mod include-root declaration?
2. **`mutable` redefinition equality.** Does "same signature" for `mutable` redefinition include default parameter values, or only name + parameter types?
3. **Combined mod shared sources.** `mod/intelligent_auto_repair_and_scout/` reuses sibling feature files at deploy time. Should the LSP support adding sibling mod folders as extra workspace folders, or should the combined mod overlay include symlinks/copies?
4. **Cache garbage collection.** Keep the last N engine-data cache versions, keep only the current hash, or time-bucket by age?
5. **Int/float strictness.** Should the LSP allow implicit `int` ↔ `float` widening in engine calls and arithmetic (matching the engine's behavior) or remain strict?

Resolved for the proposal: the game path points to the AoM:R install root; `doxygen_retail.7z` and the `game/` folder are expected beneath it.

---

## 10. Rollback strategy

- **Branch state:** Keep current work on `xs-language-server/spike`. Do redesign work on a new branch (e.g. `xs-language-server/redesign`).
- **IntelliJ plugin fallback:** If the LSP client changes break the plugin, revert the Kotlin changes. The last known-good plugin state is week 6 commit `1e9d594`, which provides file type + TextMate highlighting only.
- **VS Code artifact:** `extracted/xs.vsix` and related week 7 commit `1628aa6` are removed/stranded; reverting them is a simple git operation if needed.
- **No mod source changes:** This change does not modify `mod/*/game/` or `human_assist.xs` overlays. Deleting or reverting `tools/xs-language-server/` and `tools/intellij-xs-plugin/` client changes leaves the deployed mods untouched.

---

## 11. Dependencies

- `sevenz-rust` — decompress `doxygen_retail.7z`.
- `scraper` — parse Doxygen HTML into engine API JSON.
- `dirs` — cross-platform cache/state directory resolution.
- `sha2` — hash the 7z archive for cache keys.
- `lsp-types` — already in use; supports workspace folders and watched-file notifications.

---

## 12. References

- `openspec/changes/xs-language-server/explore.md` — locked architecture and exploration analysis.
- `openspec/changes/intellij-xs-plugin/proposal.md` — prior SDD proposal format reference.
- `AGENTS.md` — project stack, XS language references, deploy/verify loop.
- `docs/xs-lsp-spike.md` — original pivot (note: contains outdated `k`/`g`/`s` prefix guidance).
- `docs/xs-language-syntax.md` — XS syntax, modifiers, forward-declaration rules, `include` semantics.
- `docs/doxygen_retail/` and `docs/doxygen_retail.7z` — engine API documentation source.
- `tools/xs-language-server/src/` — current Rust LSP implementation.
- `tools/xs-language-server/tree-sitter-xs/grammar.js` — current XS grammar.
- `tools/intellij-xs-plugin/src/main/kotlin/` — current IntelliJ plugin (becomes thin client).
- `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` — vendored TextMate grammar.
- `mod/*/game/ai/human_assist/human_assist.xs` — representative mod overlay files.
- `openspec/config.yaml` — SDD project rules.
