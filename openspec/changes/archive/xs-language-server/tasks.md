# Tasks: XS Language Server (workspace & engine-data redesign)

## Review Workload Forecast

Total estimated LOC across all tasks: ~6,000 lines
- Rust: ~4,800 lines
- Kotlin: ~1,000 lines
- Other (TextMate, docs, `.gitignore`): ~50 lines

Chained PRs recommended: YES
- Reason: The change replaces the engine-data pipeline, reworks the workspace model, adds cross-file semantic diagnostics, and converts the IntelliJ plugin into an LSP client. The total surface is far larger than a comfortable single PR even with an unlimited review budget, and the phases are sequentially dependent.

400-line budget risk: MEDIUM
- Reasoning: `T3` (Doxygen scraping), `T7` (workspace overlay), `T14` (semantic diagnostics), and `T23` (IntelliJ LSP client lifecycle) are dense and will likely exceed 300 lines each once fixtures and tests land. They are still autonomous modules, so the risk is manageable with per-phase PRs.

Decision needed before apply: YES
- Confirm the chain strategy: feature-branch-chain with a `xs-language-server/redesign` tracker branch, or stacked-to-main PRs merging sequentially to `main`.

---

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: feature-branch-chain
400-line budget risk: Medium

---

## Phase 1 — Doxygen extraction + cache

#### T1 — Add Rust dependencies
- **Phase:** 1
- **Depends on:** —
- **Specs referenced:** `spec-engine-data-pipeline.md`
- **Files affected:** `tools/xs-language-server/Cargo.toml`
- **Acceptance criteria:**
  1. [x] Add `sevenz-rust`, `scraper`, `sha2`, `dirs`, and `anyhow` to `Cargo.toml`.
  2. [x] `cargo build` succeeds.
  3. [x] `cargo test` passes all existing tests.
- **Estimated lines changed:** ~20
- **Test command:** `cargo build && cargo test`

#### T2 — Implement `cache.rs`
- **Phase:** 1
- **Depends on:** T1
- **Specs referenced:** `spec-engine-data-pipeline.md`, `spec-file-watching-cache-invalidation.md`
- **Files affected:** `tools/xs-language-server/src/cache.rs`
- **Acceptance criteria:**
  1. [x] Resolve `dirs::state_dir().join("aomr_lsp")` with fallback to `~/.aomr_lsp`.
  2. [x] Create `v1/` subdirectory; read/write JSON files.
  3. [x] Provide SHA-256 helper for 7z archives and content hashing.
  4. [x] Provide `game_parse/v1/<mtime>-<sha256>.json` helpers.
  5. [x] Unit tests cover path resolution and read/write round-trips.
- **Estimated lines changed:** ~250
- **Test command:** `cargo test cache`

#### T3 — Implement `doxygen.rs`
- **Phase:** 1
- **Depends on:** T1
- **Specs referenced:** `spec-engine-data-pipeline.md`
- **Files affected:** `tools/xs-language-server/src/doxygen.rs`
- **Acceptance criteria:**
  1. [x] Extract `doxygen_retail.7z` to a temp dir with `sevenz-rust`.
  2. [x] Parse relevant `*_8cpp.html` pages with `scraper`.
  3. [x] Emit JSON matching existing `syscalls.json` and `aiplans.json` shapes.
  4. [x] Unit test extracts `docs/doxygen_retail.7z` and asserts counts match committed JSON (1,804 syscalls parsed directly; `EngineApi` backfills the missing legacy `xsExecute` to reach 1,805; 193 aiplans).
  5. [x] Tolerate minor HTML drift; warn and skip malformed entries but never emit empty output.
- **Estimated lines changed:** ~400
- **Test command:** `cargo test doxygen`

#### T4 — Wire `cache.rs` + `doxygen.rs` into `engine_api.rs`
- **Phase:** 1
- **Depends on:** T2, T3
- **Specs referenced:** `spec-engine-data-pipeline.md`
- **Files affected:** `tools/xs-language-server/src/engine_api.rs`
- **Acceptance criteria:**
  1. [x] Remove sibling-crate `load_default()` JSON loading path (kept as deprecated fallback).
  2. [x] Provide `EngineApi::load_from_archive(Path)` that checks cache by SHA-256 → on miss extracts and writes cache → loads JSON.
  3. [x] Corrupt cache JSON falls back to re-extraction.
  4. [x] Unit tests cover cold start (miss → extract → load) and warm start (hit → load).
- **Estimated lines changed:** ~150
- **Test command:** `cargo test engine_api`

#### T5 — Update `main.rs` for game-path CLI/env parsing
- **Phase:** 1
- **Depends on:** T1, T4
- **Specs referenced:** `spec-engine-data-pipeline.md`
- **Files affected:** `tools/xs-language-server/src/main.rs`, `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs`, `tools/xs-language-server/src/server.rs`
- **Acceptance criteria:**
  1. [x] Parse `--game-path <PATH>`, then positional argument, then `AOMR_GAME_PATH` env var.
  2. [x] Exit with a descriptive error if the game path is missing or `doxygen_retail.7z` is absent.
  3. [x] Bootstrap `EngineApi`, then start `LspService` with the game path.
  4. [x] Update `lsp_roundtrip_test.rs` to pass a test game path to the binary.
- **Estimated lines changed:** ~200
- **Test command:** `cargo test && cargo run -- --game-path ./docs` returns an error if 7z missing

#### T6 — Phase 1 verification
- **Phase:** 1
- **Depends on:** T5
- **Specs referenced:** `spec-engine-data-pipeline.md`
- **Files affected:** (verification only)
- **Acceptance criteria:**
  1. [x] Full LSP round-trip passes with the new engine-data pipeline.
  2. [x] `cargo test` is green.
  3. [x] Cold start and warm start complete within performance budgets on a representative machine.
- **Estimated lines changed:** ~50
- **Test command:** `cargo test && cargo run --bin lsp_roundtrip_test`

## Phase 2 — Workspace + virtual project

#### T7 — Implement `workspace.rs`
- **Phase:** 2
- **Depends on:** T6
- **Specs referenced:** `spec-virtual-project-overlay.md`
- **Files affected:** `tools/xs-language-server/src/workspace.rs`
- **Acceptance criteria:**
  1. [x] Represent a virtual project as engine API + game folder + mod `game/` overlay.
  2. [x] Resolve file ownership by longest workspace-folder URI prefix.
  3. [x] Hide vanilla file at relative path `R` when mod provides `game/R`.
  4. [x] Infer include roots (`game/ai/`, `game/data/trigger/`, `game/random_maps/`) from the file path.
  5. [x] Resolve `include "X"` first in mod include-root directory, then in game include-root directory.
  6. [x] Unit tests cover overlay replacement, include resolution, include-root inference, and mod-in-mod boundaries.
- **Estimated lines changed:** ~600
- **Test command:** `cargo test workspace`

#### T8 — Integrate `workspace.rs` into `server.rs`
- **Phase:** 2
- **Depends on:** T7
- **Specs referenced:** `spec-virtual-project-overlay.md`
- **Files affected:** `tools/xs-language-server/src/server.rs`
- **Acceptance criteria:**
  1. [x] Track registered workspace folders and map each to a `VirtualProject`.
  2. [x] On `didOpen`, determine owning mod; for unowned files emit `window/showMessage("file not part of any registered mod; engine API only")`.
  3. [x] Scope `workspace/symbol` queries to the owning virtual project.
  4. [x] Keep existing handlers (`completion`, `hover`, `definition`, etc.) functional.
  5. [x] Extend `lsp_roundtrip_test.rs` with virtual-project selection cases.
- **Estimated lines changed:** ~400
- **Test command:** `cargo test && cargo run --bin lsp_roundtrip_test`

#### T9 — Per-file parse cache
- **Phase:** 2
- **Depends on:** T7
- **Specs referenced:** `spec-file-watching-cache-invalidation.md`
- **Files affected:** `tools/xs-language-server/src/cache.rs`, `tools/xs-language-server/src/workspace.rs`
- **Acceptance criteria:**
  1. [x] Store parsed game-folder files at `game_parse/v1/<mtime>-<sha256>.json`.
  2. [x] Parse-tree + per-file symbol table loaded on first access.
  3. [x] If mtime/SHA-256 unchanged, reuse cache across restarts.
  4. [x] Unit tests cover cache key derivation and warm load.
- **Estimated lines changed:** ~200
- **Test command:** `cargo test parse_cache`

#### T10 — `didChangeWatchedFiles` registration
- **Phase:** 2
- **Depends on:** T8, T9
- **Specs referenced:** `spec-file-watching-cache-invalidation.md`
- **Files affected:** `tools/xs-language-server/src/server.rs`
- **Acceptance criteria:**
  1. [x] In `initialized`, dynamically register `workspace/didChangeWatchedFiles` for `<game-path>/game/**/*.xs` when the client supports it.
  2. [x] On `Changed`/`Deleted` notifications, invalidate the parse cache for the URI and dependent files.
  3. [x] Re-diagnose affected open mod files and publish updated diagnostics.
  4. [x] If the client does not support dynamic registration, degrade gracefully.
  5. [x] Add LSP round-trip test simulating a watched-file change.
- **Estimated lines changed:** ~300
- **Test command:** `cargo test watched_files && cargo run --bin lsp_roundtrip_test`

#### T11 — `didChangeWorkspaceFolders` handler
- **Phase:** 2
- **Depends on:** T8
- **Specs referenced:** `spec-virtual-project-overlay.md`
- **Files affected:** `tools/xs-language-server/src/server.rs`
- **Acceptance criteria:**
  1. [x] Implement `workspace/didChangeWorkspaceFolders` method.
  2. [x] Add/remove mod roots; rebuild virtual-project overlay maps.
  3. [x] Re-evaluate ownership of currently open files and emit showMessage/unowned warning as needed.
  4. [x] Add LSP round-trip test for add/remove workspace folders.
- **Estimated lines changed:** ~250
- **Test command:** `cargo test workspace_folders && cargo run --bin lsp_roundtrip_test`

#### T12 — Phase 2 verification
- **Phase:** 2
- **Depends on:** T10, T11
- **Specs referenced:** `spec-virtual-project-overlay.md`, `spec-file-watching-cache-invalidation.md`
- **Files affected:** (verification only)
- **Acceptance criteria:**
  1. [x] `cargo test` is green.
  2. [x] Round-trip tests cover include resolution, file replacement, and workspace-folder add/remove.
  3. [x] Per-keystroke diagnostic latency stays under 200 ms on representative mod files (amortised by per-file parse cache).
- **Estimated lines changed:** ~50
- **Test command:** `cargo test && cargo run --bin lsp_roundtrip_test`

## Phase 3 — Semantic diagnostics

#### T13 — Extend `symbols.rs`
- **Phase:** 3
- **Depends on:** T12
- **Specs referenced:** `spec-semantic-diagnostics.md`
- **Files affected:** `tools/xs-language-server/src/symbols.rs`
- **Acceptance criteria:**
  1. [x] Record `extern` and `mutable` flags on functions and variables.
  2. [x] Record forward-only declarations (function header without body) from `function_definition` where the parser emits them.
  3. [x] Distinguish file-local (`static`/no `extern`) symbols from exported (`extern`) symbols.
  4. [x] Expose a per-file `SymbolTable` API suitable for a cross-file index.
  5. [x] Unit tests cover `extern`, `mutable`, and forward-decl extraction.
- **Estimated lines changed:** ~250
- **Test command:** `cargo test symbols`

#### T14 — Implement `semantic.rs`
- **Phase:** 3
- **Depends on:** T13
- **Specs referenced:** `spec-semantic-diagnostics.md`
- **Files affected:** `tools/xs-language-server/src/semantic.rs`
- **Acceptance criteria:**
  1. [x] Detect `extern` collisions: any file has `extern X`, another file declares/defines `X` → diagnostic.
  2. [x] Validate use-before-definition: function call before its definition is an error unless the function is `mutable` or a forward declaration precedes the call.
  3. [x] Validate `mutable` redefinition equality: same name, parameter types, and default values.
  4. [x] Respect `include "..."` textual semantics for visibility.
  5. [x] Unit tests with fixtures cover forward-decl, extern collision, mutable redefinition, and file-local shadowing.
- **Estimated lines changed:** ~600
- **Test command:** `cargo test semantic`

#### T15 — Wire semantic diagnostics into `diagnostics.rs`
- **Phase:** 3
- **Depends on:** T14, T8
- **Specs referenced:** `spec-semantic-diagnostics.md`
- **Files affected:** `tools/xs-language-server/src/diagnostics.rs`, `tools/xs-language-server/src/server.rs`
- **Acceptance criteria:**
  1. [x] On `didOpen`/`didChange`, after parse, run `semantic.rs` over the file’s virtual project.
  2. [x] Merge semantic diagnostics with parse diagnostics.
  3. [x] Publish combined diagnostics with source `"xs-language-server"`.
  4. [x] Include engine-style `Error 0310` message for unresolved symbols.
- **Estimated lines changed:** ~150
- **Test command:** `cargo test diagnostics && cargo run --bin lsp_roundtrip_test`

#### T16 — Extend `typecheck.rs`
- **Phase:** 3
- **Depends on:** T13, T8
- **Specs referenced:** `spec-semantic-diagnostics.md`
- **Files affected:** `tools/xs-language-server/src/typecheck.rs`
- **Acceptance criteria:**
  1. [x] Allow implicit `int` ↔ `float` widening in arithmetic, comparisons, and engine-call arguments.
  2. [x] Resolve workspace function types across files via the virtual-project symbol index.
  3. [x] Keep existing engine-call count/type checks.
  4. [x] Unit tests cover `int` passed where `float` expected and cross-file function argument checks.
- **Estimated lines changed:** ~250
- **Test command:** `cargo test typecheck`

#### T17 — Update `completion.rs`
- **Phase:** 3
- **Depends on:** T13, T16
- **Specs referenced:** `spec-semantic-diagnostics.md`
- **Files affected:** `tools/xs-language-server/src/completion.rs`
- **Acceptance criteria:**
  1. [x] Combine engine API items with exported project symbols (`extern` variables/functions and all symbols from included files).
  2. [x] Hide file-local variables outside their declaring file.
  3. [x] Prefer exact/prefix matching without prefix-based heuristics.
  4. [x] Unit tests verify scope filtering.
- **Estimated lines changed:** ~250
- **Test command:** `cargo test completion`

#### T18 — Phase 3 verification
- **Phase:** 3
- **Depends on:** T15, T16, T17
- **Specs referenced:** `spec-semantic-diagnostics.md`
- **Files affected:** `tools/xs-language-server/tests/fixtures/**/*` (new fixtures)
- **Acceptance criteria:**
  1. [x] Fixtures for forward-decl errors, extern collisions, mutable redefinition, int/float widening, and autocompletion scope.
  2. [x] LSP round-trip tests cover each semantic scenario.
  3. [x] `cargo test` is green.
- **Estimated lines changed:** ~300
- **Test command:** `cargo test && cargo run --bin lsp_roundtrip_test`

## Phase 4 — IntelliJ client integration

#### T19 — Vendor `xs.tmLanguage.json`
- **Phase:** 4
- **Depends on:** T12
- **Specs referenced:** `spec-intellij-client-integration.md`
- **Files affected:** `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`, `tools/intellij-xs-plugin/build.gradle.kts`
- **Acceptance criteria:**
  1. TextMate grammar is committed at `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`.
  2. If missing, extract it from the cached VS Code grammar source.
  3. Update `build.gradle.kts` resource validation to require only `syntaxes/xs.tmLanguage.json` (no longer `syscalls.json`/`aiplans.json`).
  4. TextMate highlighting works in IntelliJ after `./gradlew buildPlugin`.
- **Estimated lines changed:** ~20
- **Test command:** `./gradlew buildPlugin`

#### T20 — Implement `XsSettings.kt`
- **Phase:** 4
- **Depends on:** T19
- **Specs referenced:** `spec-intellij-client-integration.md`
- **Files affected:** `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/XsSettings.kt`
- **Acceptance criteria:**
  1. `PersistentStateComponent` stores `gamePath: String` and `modPaths: MutableList<String>`.
  2. Defaults are empty strings/lists.
  3. State is persisted per project and restored on reopen.
  4. Unit/component test verifies persistence round-trip.
- **Estimated lines changed:** ~100
- **Test command:** `./gradlew test --tests "com.aomr.xs.settings.*"`

#### T21 — Implement `XsConfigurable.kt`
- **Phase:** 4
- **Depends on:** T20
- **Specs referenced:** `spec-intellij-client-integration.md`
- **Files affected:** `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/XsConfigurable.kt`
- **Acceptance criteria:**
  1. Settings UI appears under Settings → Languages & Frameworks → XS.
  2. Game-folder text field with file chooser.
  3. Mod list with `+`/`-` buttons and drag-reorder.
  4. Auto-detect button wired to `XsModAutoDetector`.
  5. UI reads/writes `XsSettings`.
- **Estimated lines changed:** ~300
- **Test command:** `./gradlew test --tests "com.aomr.xs.settings.*"`

#### T22 — Implement `XsModAutoDetector.kt`
- **Phase:** 4
- **Depends on:** T20
- **Specs referenced:** `spec-intellij-client-integration.md`
- **Files affected:** `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/XsModAutoDetector.kt`
- **Acceptance criteria:**
  1. Recursively scan project directories for folders named exactly `game`.
  2. Stop recursion at each `game/` boundary.
  3. Return each `game/` parent as a mod root.
  4. Unit test with a synthetic project fixture covers nested `game/` boundaries.
- **Estimated lines changed:** ~100
- **Test command:** `./gradlew test --tests "com.aomr.xs.settings.XsModAutoDetectorTest"`

#### T23 — Extend `XsLspServerManager.kt`
- **Phase:** 4
- **Depends on:** T20, T22
- **Specs referenced:** `spec-intellij-client-integration.md`
- **Files affected:** `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerManager.kt`
- **Acceptance criteria:**
  1. Spawn the Rust LSP binary with `--game-path <gamePath>`.
  2. After spawn, send `workspace/didChangeWorkspaceFolders` with detected mod URIs.
  3. Register `didChangeWatchedFiles` watcher for `<gamePath>/game/**/*.xs`.
  4. On mod list change, re-send `workspace/didChangeWorkspaceFolders`.
  5. On game-folder change, restart the LSP server.
  6. Handle process failure with a non-blocking error notification.
- **Estimated lines changed:** ~400
- **Test command:** `./gradlew test --tests "com.aomr.xs.lsp.*"`

#### T24 — Update `plugin.xml`
- **Phase:** 4
- **Depends on:** T21
- **Specs referenced:** `spec-intellij-client-integration.md`
- **Files affected:** `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`
- **Acceptance criteria:**
  1. Add `<projectConfigurable>` for `XsConfigurable`.
  2. Remove engine-facing extension points (`completion.contributor`, `documentationProvider`, `parameterInfoHandler`, `psi.referenceContributor`).
  3. Keep file-type, TextMate, editor helper registrations.
  4. Plugin builds and loads without ClassNotFound errors.
- **Estimated lines changed:** ~20
- **Test command:** `./gradlew buildPlugin`

#### T25 — First-run UX wiring
- **Phase:** 4
- **Depends on:** T23, T24
- **Specs referenced:** `spec-intellij-client-integration.md`
- **Files affected:** `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/startup/XsStartupActivity.kt`, `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`
- **Acceptance criteria:**
  1. `StartupActivity.DumbAware` reads `XsSettings` on IDE startup.
  2. If `gamePath` is empty, open Settings prompting for it.
  3. If `modPaths` is empty, run `XsModAutoDetector.scan()` and persist results.
  4. If detection finds nothing, show a non-blocking `NotificationGroup.WARNING`.
  5. Start the LSP server and send workspace folders.
- **Estimated lines changed:** ~300
- **Test command:** `./gradlew test && ./gradlew runIde` (manual check)

#### T26 — Phase 4 verification
- **Phase:** 4
- **Depends on:** T25
- **Specs referenced:** `spec-intellij-client-integration.md`
- **Files affected:** (verification only)
- **Acceptance criteria:**
  1. `./gradlew buildPlugin` succeeds.
  2. `./gradlew test` is green.
  3. Manual test: install plugin, open a mod `.xs` file, and verify diagnostics arrive from the Rust server.
  4. Remove obsolete engine-facing Kotlin classes (`XsEngineApi`, `XsAiPlans`, `XsCompletionContributor`, `XsDocumentationProvider`, `XsParameterInfoHandler`, `XsEngineReferenceContributor`, `XsEngineStubGenerator`, `XsCallContextDetector`) so the plugin is a true thin client.
- **Estimated lines changed:** ~-700 (deletions) + 50 tests
- **Test command:** `./gradlew buildPlugin && ./gradlew test`

## Phase 5 — VS Code removal + housekeeping

#### T27 — Remove `extracted/xs.vsix` and obsolete engine resources
- **Phase:** 5
- **Depends on:** T26
- **Specs referenced:** `spec-vscode-removal.md`
- **Files affected:** `extracted/xs.vsix`, `.gitignore`, `tools/intellij-xs-plugin/src/main/resources/syscalls.json`, `tools/intellij-xs-plugin/src/main/resources/aiplans.json`, `tools/xs-language-server/src/engine_api.rs`, `tools/xs-language-server/src/typecheck.rs`, `tools/xs-language-server/src/doxygen.rs`
- **Acceptance criteria:**
  1. [x] `git rm extracted/xs.vsix` (or `git rm --ignore-unmatch` if absent).
  2. [x] Ensure `.gitignore` ignores `extracted/` and contains no `!extracted/xs.vsix` exception.
  3. [x] No `.vsix` files are tracked (`git ls-files | grep '\.vsix$'` returns empty).
  4. [x] Week 7 commit `1628aa6` left stranded; no history rewrite performed.
  5. [x] Removed deprecated `syscalls.json`/`aiplans.json` from plugin resources.
  6. [x] Removed sibling-JSON fallback from `EngineApi`.
- **Estimated lines changed:** ~5
- **Test command:** `git ls-files | grep '\.vsix$'` returns empty

#### T28 — Update `AGENTS.md`
- **Phase:** 5
- **Depends on:** T26, T27
- **Specs referenced:** `spec-intellij-client-integration.md`, `spec-vscode-removal.md`
- **Files affected:** `AGENTS.md`
- **Acceptance criteria:**
  1. [x] Describe the new LSP architecture (Rust server + thin IntelliJ client).
  2. [x] Document cache directory (`~/.local/state/aomr_lsp/` or `~/.aomr_lsp`).
  3. [x] Describe IntelliJ settings scope and mod auto-detect behavior.
  4. [x] Remove references to the VS Code client path.
- **Estimated lines changed:** ~50
- **Test command:** manual review

#### T29 — Update `docs/xs-lsp-spike.md`
- **Phase:** 5
- **Depends on:** T27
- **Specs referenced:** `spec-vscode-removal.md`
- **Files affected:** `docs/xs-lsp-spike.md`
- **Acceptance criteria:**
  1. [x] Fix the incorrect prefix examples (`k[A-Z]\w*`, `g[A-Z]\w*`, `s[A-Z]\w*`) and replace with the actual `const`/`extern`/`mutable` + Doxygen architecture.
  2. [x] Add a banner marking the doc as historical (post-spike plan, superseded by `openspec/changes/xs-language-server/`).
  3. [x] Update project layout to reflect the real Rust/Kotlin split.
- **Estimated lines changed:** ~40
- **Test command:** manual review

#### T30 — Final verification
- **Phase:** 5
- **Depends on:** All previous tasks
- **Specs referenced:** all six specs
- **Files affected:** (verification only)
- **Acceptance criteria:**
  1. [x] `cargo test` passes (75 tests).
  2. [x] `cargo run --bin lsp_roundtrip_test` passes.
  3. [x] `./gradlew test` passes (sandbox-friendly plain-JUnit tests).
  4. [x] `./gradlew buildPlugin` succeeds.
  5. [x] Manual end-to-end smoke test in IntelliJ documented as CI-only; requires non-headless IDE and AoM:R install.
  6. [x] Verification report created at `openspec/changes/xs-language-server/verify-report.md`.
- **Estimated lines changed:** ~30
- **Test command:** `cargo test && cargo run --bin lsp_roundtrip_test && ./gradlew test && ./gradlew buildPlugin`
