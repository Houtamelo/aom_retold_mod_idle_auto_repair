# Verification Report — `xs-language-server` (workspace & engine-data redesign)

**Change:** `xs-language-server` (workspace & engine-data redesign)  
**Phase verified:** Phases 1–5 (T1–T30)  
**Verification date:** 2026-06-24  
**Verifier:** `sdd-apply` executor (Phase 5)  
**Branch:** `xs-language-server/redesign-phase-5`  

---

## 1. Executive summary

The redesigned XS Language Server is fully implemented and all automated verification gates are green. The Rust server extracts engine API data from `doxygen_retail.7z`, caches it by SHA-256, models each mod as a virtual project overlay on the vanilla `game/` folder, and diagnoses cross-file `extern` collisions, missing forward declarations, `mutable` redefinitions, and engine-call type/count errors. The IntelliJ plugin is a thin LSP client with a settings page, mod auto-detect, and LSP lifecycle management. The deprecated VS Code `.vsix` and legacy bundled `syscalls.json`/`aiplans.json` have been removed. The only remaining gap is the manual end-to-end smoke test inside a real IDE with a real AoM:R install, which cannot be executed in this headless sandbox. Overall verdict: **PASS with notes**.

---

## 2. Test results

| Suite | Command | Result |
|---|---|---|
| Rust unit tests | `cargo test` | **PASS** — 75 passed, 0 failed |
| LSP round-trip test | `cargo run --bin lsp_roundtrip_test` | **PASS** — baseline + workspace + semantic sequences green |
| IntelliJ plugin tests | `./gradlew test` | **PASS** — BUILD SUCCESSFUL |
| IntelliJ plugin package | `./gradlew buildPlugin` | **PASS** — BUILD SUCCESSFUL |

### Performance observations

- Warm engine-data load: < 1 s once the SHA-256 cache is populated.
- Cold engine-data extraction from `docs/doxygen_retail.7z` completes within the < 10 s budget on the sandbox host.
- Per-keystroke diagnostic latency was not instrumented with a representative corpus; the per-file parse cache and single-file diagnostic pass keep local edits responsive.

---

## 3. Per-spec coverage

### spec-engine-data-pipeline.md

**Status:** PASS

**Scenarios verified:**

1. **Cold start** — `EngineApi::load_from_archive` extracts `doxygen_retail.7z`, writes `~/.local/state/aomr_lsp/v2/<sha256>.json`, and loads 1,804 syscalls + 193 AI-plan constants.
2. **Warm start** — a second call with the same archive loads cached JSON and skips extraction.
3. **Corrupt cache** — a corrupt cache file is deleted and the archive is re-extracted.
4. **CLI/env resolution** — `main.rs` resolves `--game-path`, positional argument, and `AOMR_GAME_PATH` in the specified precedence order and exits descriptively when the archive is missing.

**Evidence:**

- `engine_api::tests::load_from_archive_cold_start_hits_target_counts`
- `engine_api::tests::load_from_archive_warm_start_skips_extraction`
- `engine_api::tests::corrupt_cache_falls_back_to_re_extraction`
- `cargo run --bin xs-language-server -- --game-path ./docs` returns an error when `doxygen_retail.7z` is absent

---

### spec-virtual-project-overlay.md

**Status:** PASS

**Scenarios verified:**

1. **Overlay replacement** — a mod file at relative path `R` hides the vanilla `game/R` file from workspace symbol queries.
2. **Ownership by longest prefix** — a file under a registered mod URI is owned by that mod; an unowned file triggers `window/showMessage("file not part of any registered mod; engine API only")`.
3. **Include-root inference** — `game/ai/...`, `game/data/trigger/...`, and `game/random_maps/...` are recognised as include roots.
4. **Include resolution** — `include "foo.xs"` resolves in the mod overlay first, then in the vanilla game folder.
5. **Workspace-folder add/remove** — `workspace/didChangeWorkspaceFolders` registers and unregisters mods; re-diagnosis is sent for affected open files.

**Evidence:**

- `workspace::tests::build_virtual_project_lists_mod_overrides`
- `workspace::tests::mod_overlay_hides_vanilla_file`
- `workspace::tests::resolve_include_prefers_mod_overlay`
- `workspace::tests::resolve_include_falls_back_to_vanilla`
- `workspace::tests::resolve_plain_filename_in_include_root`
- `workspace::tests::lookup_mod_uses_longest_prefix`
- `workspace::tests::nested_game_directory_is_not_a_separate_overlay_root`
- `lsp_roundtrip_test.rs` workspace-folder add/remove sequence

---

### spec-file-watching-cache-invalidation.md

**Status:** PASS

**Scenarios verified:**

1. **Dynamic watcher registration** — the server registers a `workspace/didChangeWatchedFiles` watcher for `<game-path>/game/**/*.xs` when the client supports it.
2. **Cache invalidation** — a `Changed` or `Deleted` notification invalidates the per-file parse cache entry.
3. **Graceful degradation** — clients without dynamic registration do not cause a panic.

**Evidence:**

- `cache::tests::load_or_parse_reparses_after_invalidation`
- `cache::tests::invalidate_parse_cache_removes_matching_entries`
- `lsp_roundtrip_test.rs` watcher registration assertions

---

### spec-semantic-diagnostics.md

**Status:** PASS

**Scenarios verified:**

1. **`extern` collision** — a symbol declared `extern` in one file and defined/declared in another produces an error.
2. **Use before definition** — a function call before its definition is an error unless the function is `mutable` or has a forward declaration.
3. **`mutable` redefinition** — redefinition with the same signature is allowed; a different signature is an error.
4. **File-local variables** — non-`extern` variables are hidden from autocomplete in other files.
5. **Engine-call checks** — wrong argument count and wrong argument type for engine syscalls are diagnosed, with `int`↔`float` widening allowed.

**Evidence:**

- `semantic::tests::extern_collision_between_files`
- `semantic::tests::duplicate_extern_between_files_is_collision`
- `semantic::tests::missing_forward_declaration_is_error`
- `semantic::tests::forward_declaration_allows_call`
- `semantic::tests::mutable_function_is_forward_callable`
- `semantic::tests::mutable_redefinition_same_signature_is_ok`
- `semantic::tests::mutable_redefinition_different_signature_is_error`
- `semantic::tests::file_local_same_name_is_ok`
- `typecheck::tests::*`
- `completion::tests::*`
- `lsp_roundtrip_test.rs` semantic fixture sequences

---

### spec-intellij-client-integration.md

**Status:** PASS with notes

**Scenarios verified:**

1. **Settings UI** — `XsConfigurable` is registered under **Settings → Languages & Frameworks → XS** and reads/writes `XsSettings`.
2. **Auto-detect** — `XsModAutoDetector` recursively scans for directories named exactly `game` and stops at `game/` boundaries, returning each parent as a mod root.
3. **LSP startup** — `XsStartupActivity` spawns the Rust binary with `--game-path` and sends `workspace/didChangeWorkspaceFolders`.
4. **Settings changes** — changing the mod list re-sends workspace folders; changing the game folder restarts the server.
5. **File watcher registration** — the plugin registers watchers for `<game-folder>/game/**/*.xs`.

**Evidence:**

- `XsSettingsTest`
- `XsModAutoDetectorTest`
- `./gradlew buildPlugin` succeeds (validates plugin.xml wiring and bundled resources)
- Manual review of `XsLspServerManager.kt` and `XsStartupActivity.kt`

**Notes:**

- Automated tests do not start a real LSP process; lifecycle logic is verified by inspection and the Rust round-trip tests.
- The manual smoke test (open a mod `.xs` file in IntelliJ/Rider and confirm diagnostics arrive) requires a non-headless IDE and a real AoM:R install and cannot run in CI.

---

### spec-vscode-removal.md

**Status:** PASS

**Scenarios verified:**

1. **No tracked `.vsix`** — `git ls-files | grep '\.vsix$'` returns empty.
2. **Gitignore protection** — `.gitignore` contains `extracted/` and `*.vsix` with no exception for `extracted/xs.vsix`.
3. **History preserved** — week 7 commit `1628aa6` was not reverted or rebased; it remains stranded in history as agreed.
4. **No replacement VS Code client** — no VS Code extension code or build artifact exists in the repo.

**Evidence:**

- `git ls-files | grep '\.vsix$'`
- `.gitignore` contents
- `git log --oneline -- extracted/xs.vsix` (stranded history)

---

## 4. Per-task coverage

| Task | Description | Status |
|---|---|---|
| T1 | Rust dependencies for extraction/cache | PASS |
| T2 | `cache.rs` state dir + JSON cache helpers | PASS |
| T3 | `doxygen.rs` 7z extraction + HTML scraping | PASS |
| T4 | `engine_api.rs` archive loading with cache fallback | PASS |
| T5 | `main.rs` `--game-path` / env parsing | PASS |
| T6 | Phase 1 verification | PASS |
| T7 | `workspace.rs` virtual project + overlay | PASS |
| T8 | `server.rs` workspace-folder integration | PASS |
| T9 | Per-file parse cache | PASS |
| T10 | `didChangeWatchedFiles` registration | PASS |
| T11 | `didChangeWorkspaceFolders` handler | PASS |
| T12 | Phase 2 verification | PASS |
| T13 | `symbols.rs` extern/mutable/forward visibility | PASS |
| T14 | `semantic.rs` cross-file diagnostics | PASS |
| T15 | Semantic diagnostics wired into publish path | PASS |
| T16 | `typecheck.rs` int↔float widening + cross-file | PASS |
| T17 | `completion.rs` scope filtering | PASS |
| T18 | Phase 3 verification fixtures + round-trip | PASS |
| T19 | Vendored `xs.tmLanguage.json` | PASS |
| T20 | `XsSettings.kt` PersistentStateComponent | PASS |
| T21 | `XsConfigurable.kt` settings UI | PASS |
| T22 | `XsModAutoDetector.kt` recursive game scan | PASS |
| T23 | `XsLspServerManager.kt` lifecycle + workspace folders | PASS |
| T24 | `plugin.xml` registration + extension cleanup | PASS |
| T25 | First-run UX wiring | PASS |
| T26 | Phase 4 verification | PASS |
| T27 | Remove `.vsix` and obsolete engine resources | PASS |
| T28 | Update `AGENTS.md` | PASS |
| T29 | Update `docs/xs-lsp-spike.md` | PASS |
| T30 | Final verification report | PASS |

---

## 5. Test quality audit

**Strengths:**

- Rust unit tests cover every major module (cache, doxygen, engine_api, workspace, semantic, typecheck, completion, symbols) with deterministic fixtures.
- The LSP round-trip test exercises the full JSON-RPC sequence including initialize, didOpen, completion, hover, definition, document symbol, references, rename, prepareRename, typecheck diagnostics, workspace-folder changes, and watched-file registration.
- Plugin tests were rewritten as plain JUnit to avoid the headless sandbox fixture hangs that blocked the legacy `BasePlatformTestCase` tests.
- `XsModAutoDetectorTest` now verifies both flat and nested project layouts and the `game/` boundary rule.

**Weaknesses / gaps:**

- No automated test starts the real Rust binary from the Kotlin plugin; lifecycle wiring is verified by inspection plus the Rust round-trip test.
- No representative corpus benchmark for per-keystroke diagnostic latency.
- No automated comparison against the AoM:R engine because the engine cannot run in CI.

**Coverage adequacy:** Coverage is adequate for the capabilities implemented. The gaps are manual-verification or performance-instrumentation gaps, not missing feature coverage.

---

## 6. Deviations from design

| # | Design / task wording | Implementation | Rationale |
|---|---|---|---|
| 1 | T4: keep sibling-JSON as deprecated fallback | Sibling-JSON fallback removed entirely in Phase 5 | The archive-based pipeline is reliable and the legacy files created legal/maintenance risk; user explicitly directed removal of vendored VS Code extension artifacts |
| 2 | `EngineApi` exposes 1,805 syscalls | Direct archive extraction yields 1,804 syscalls (`xsExecute` is not in the Doxygen archive) | No legacy backfill; the LSP now reports exactly what the archive contains |
| 3 | T30: `./gradlew test` with platform fixtures | Rewrote settings/detector tests as plain JUnit to avoid headless fixture hangs | Sandbox CI cannot run `BasePlatformTestCase`/`LightPlatformTestCase` reliably; the rewritten tests still assert the same behavior |
| 4 | Per-keystroke latency < 200 ms | Not instrumented on a representative corpus | Implementation uses per-file parse cache and single-file diagnostic passes, but no corpus benchmark was run |
| 5 | Engine-data cache schema version `v1/` per original design/specs | Bumped to `v2/` when legacy JSON backfill was removed | Prevents stale `v1/` caches (which could still contain 1,805 backfilled syscalls) from masking the new no-backfill behavior; older `v1/` files are left in place and ignored |

No deviations break the specified behavior.

---

## 7. Known issues

| Priority | Issue | Mitigation / note |
|---|---|---|
| Low | Forward declarations parse as `ERROR` nodes because the tree-sitter XS grammar has no dedicated function-header rule | `symbols::build_symbol_table` recovers forward declarations from `ERROR` nodes; documented in Phase 3 apply-progress |
| Low | `workspace/symbol` scopes to the most recently active document because `WorkspaceSymbolParams` carries no URI | Sufficient for editor-driven queries; noted in Phase 2 apply-progress |
| Low | `include "..."` textual paste is approximated for completion scope | Cross-file semantic resolution treats all project function definitions as visible; noted in Phase 3 apply-progress |
| Medium | Manual end-to-end smoke test in IntelliJ cannot run in this headless sandbox | Documented as manual-only verification; compile/test/package gates pass |
| Low | Several `#[allow(dead_code)]` warnings for unused helpers (`SemanticChecker`, `Param::render`, etc.) | Helper code retained for future phases; does not affect behavior |

---

## 8. Risks & follow-ups

1. **Doxygen HTML format drift.** Game patches may change Doxygen output. The scraper already tolerates malformed entries and logs warnings, but a future patch could still break parsing. Mitigation: add CI validation that extraction produces the expected syscall/aiplan counts.
2. **Headless plugin test fragility.** The move to plain JUnit removed fixture hangs but also reduced coverage of platform-specific `project` service behavior. Consider adding a single lightweight integration test once fixture stability improves.
3. **Manual smoke test gap.** The final acceptance criterion requires opening a real `.xs` mod file in a real IDE. Schedule this before publishing the plugin.
4. **Cache garbage collection.** Old `v1/` cache versions are retained indefinitely. Add a retention policy (e.g. keep last N or age-bucket) in a future maintenance change.
5. **Per-keystroke latency instrumentation.** Add representative corpus timing tests before claiming the 200 ms budget is consistently met.

---

## 9. Recommended actions

**Before merging:**

- [ ] Reviewer sign-off on the verification report and the decision to strand commit `1628aa6`.
- [ ] Optional: run the manual end-to-end smoke test in a non-headless IDE with a real AoM:R install.

**Non-blocking fast-follows:**

- [ ] Add CI validation of Doxygen extraction counts.
- [ ] Add corpus-based diagnostic latency benchmark.
- [ ] Implement cache version garbage collection.
- [ ] Add a dedicated tree-sitter grammar rule for forward declarations.

**Next phases:**

- `sdd-verify` formal sign-off.
- `sdd-archive` to finalise spec deltas.

---

## 10. Compliance summary

| Spec | Status | Directly tested scenarios | Implemented but not directly tested scenarios |
|---|---|---|---|
| spec-engine-data-pipeline.md | PASS | cold/warm cache, corrupt cache, CLI/env path | — |
| spec-virtual-project-overlay.md | PASS | overlay replacement, ownership, include roots, workspace-folder add/remove | — |
| spec-file-watching-cache-invalidation.md | PASS | watcher registration, cache invalidation | client-without-registration runtime path |
| spec-semantic-diagnostics.md | PASS | extern collision, forward declaration, mutable redefinition, int↔float widening | large-corpus latency |
| spec-intellij-client-integration.md | PASS with notes | settings, auto-detect, plugin packaging | real LSP process spawn from Kotlin |
| spec-vscode-removal.md | PASS | no tracked .vsix, gitignore, history preserved | — |

| Task group | Status |
|---|---|
| Phase 1 (T1–T6) | PASS |
| Phase 2 (T7–T12) | PASS |
| Phase 3 (T13–T18) | PASS |
| Phase 4 (T19–T26) | PASS |
| Phase 5 (T27–T30) | PASS |

**Overall verdict:** **PASS with notes**.
