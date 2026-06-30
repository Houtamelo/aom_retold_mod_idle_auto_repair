# Tasks: fix-goto-definition-on-include-statements

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~120 (+100 / -5 in tracked files; ~50 in new LSP tests; ~15 in plugin test) |
| New test files | 1 (`tools/xs-language-server/tests/include_goto_definition_repro.rs`) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Decision needed before apply | No |

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | LSP include-aware go-to-definition + regression tests | PR 1 | Against `main`; changes in `server.rs`, `parser.rs`, `workspace.rs`, new integration test file |
| 2 | Plugin forwarding regression test + version bump + docs | PR 1 | Kotlin test in `XsGotoDeclarationHandlerTest.kt`, `gradle.properties` 0.3.0 → 0.4.0, runtime issues doc update |

---

## Phase 0: Setup

### Task 0.1: Confirm branch state
**TDD Cycle:** Setup
- [x] Verify the working tree is on a branch dedicated to `fix-goto-definition-on-include-statements` and is clean of unrelated changes (`git status --short`).
- [x] Confirm the parent `HEAD` contains the resolved Issues 1, 2, and 3 changes so Issue #4 is the only outstanding item.
- **Verification:** `git status` shows only expected files; `git log --oneline -5` includes the recent SDD changes.

### Task 0.2: Confirm `cargo build --tests` clean
**TDD Cycle:** Setup
- [x] Build the LSP crate with tests: `cargo build --manifest-path tools/xs-language-server/Cargo.toml --tests`.
- [x] Record the baseline test count by listing tests.
- **Verification:**
  - Build exits with status 0.
  - Test list totals **215** existing tests (`cargo test --manifest-path tools/xs-language-server/Cargo.toml -- --list 2>&1 | grep -c ': test'`).
  - Accept any pre-existing warnings (e.g., workspace test `game_root` unused, `DID_CHANGE` const in `lsp_roundtrip_test`); do not attempt to fix them in this change.

### Task 0.3: Note current `pluginVersion`
**TDD Cycle:** Setup
- [x] Read `tools/intellij-xs-plugin/gradle.properties` and record `pluginVersion = 0.3.0` as the starting value.
- [x] Confirm `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandler.kt` exists and is the existing Issue #1 handler.
- **Verification:** `grep '^pluginVersion' tools/intellij-xs-plugin/gradle.properties` returns `0.3.0`.

---

## Phase 1: RED — Write failing LSP tests first

### Task 1.1: Create `tools/xs-language-server/tests/include_goto_definition_repro.rs`
**TDD Cycle:** RED
- [x] Create a new integration test file at `tools/xs-language-server/tests/include_goto_definition_repro.rs`.
- [x] Reuse the proven helpers from `tests/r5_test_honesty_repro.rs` for LSP `Content-Length` framing and JSON-RPC parsing.
- [x] Add a `Fixture` helper that:
  - Builds a temporary `game/` tree and optional mod overlay.
  - Copies `docs/doxygen_retail.7z` into the temp game root.
  - Spawns `CARGO_BIN_EXE_xs-language-server --game-path <temp_game>`.
  - Sends `initialize` with optional workspace folders.
  - Opens the includer file via `textDocument/didOpen`.
- [x] Add the following five tests, each asserting the `textDocument/definition` response:
  1. `test_direct_include_resolves_to_target` (R1)
     - Vanilla `game/ai/core/main.xs` contains `include "debug.xs";`.
     - `game/ai/core/debug.xs` exists.
     - Cursor on the `d` of `debug.xs`.
     - Assert `result.uri` ends with `game/ai/core/debug.xs` and range is `(0,0)-(0,0)`.
  2. `test_mod_overlay_resolves_to_mod_file` (R2)
     - Vanilla `game/ai/core/debug.xs` exists.
     - Mod `mod_a/game/ai/core/debug.xs` overlays it.
     - Includer is `mod_a/game/ai/core/main.xs` with `include "debug.xs";`.
     - Register `mod_a` as a workspace folder.
     - Assert URI ends with `mod_a/game/ai/core/debug.xs`.
  3. `test_vanilla_fallback_when_mod_missing` (R3)
     - Vanilla `game/ai/core/debug.xs` exists.
     - Mod `mod_a/game/ai/core/main.xs` does **not** overlay `debug.xs`.
     - Includer is `mod_a/game/ai/core/main.xs` with `include "debug.xs";`.
     - Register `mod_a`.
     - Assert URI ends with `game/ai/core/debug.xs`.
  4. `test_not_found_returns_null` (R4)
     - `game/ai/core/main.xs` contains `include "does/not/exist.xs";`.
     - Assert `result` is `null` (or an empty array, depending on serialization).
  5. `test_cursor_outside_path_token_uses_existing_logic` (R5)
     - `game/ai/core/main.xs` contains `include "debug.xs"; void foo() {}`.
     - Cursor on the `include` keyword (line 0, character ~2).
     - Assert `result` is `null`.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test include_goto_definition_repro --no-run` compiles successfully.

### Task 1.2: Run tests and confirm all 5 FAIL
**TDD Cycle:** RED confirmation
- [x] Run the new integration tests against the current implementation.
- [x] Capture the failing test names and assertions (every test should fail because the server currently returns `null` for include paths).
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test include_goto_definition_repro -- --nocapture`. All five tests must fail, each expecting a resolved URI but receiving `null`.

### Task 1.3: Document the failure as evidence
**TDD Cycle:** RED documentation
- [x] Record the RED output in the verify report placeholder or a temporary note file.
- [x] Save the failing command and count (5/5 RED) for the final verify report.
- **Verification:** Evidence file exists and clearly shows all tests failing before implementation.

---

## Phase 2: GREEN — Implement the minimal LSP fix

### Task 2.1: Add `detect_include_path_at_position` helper in `parser.rs`
**TDD Cycle:** GREEN
- [x] Open `tools/xs-language-server/src/parser.rs`.
- [x] Add a new public function `detect_include_path_at_position(_file: &Path, source: &str, line: u32, col: u32) -> Option<String>` using tree-sitter parsing.
- [x] Add private helper `range_contains(node, line, col)` that checks whether an LSP position lies inside a tree-sitter node's range.
- [x] Functionality:
  - Parse the source.
  - Walk only top-level children.
  - Find an `include_directive` whose range contains the cursor.
  - Ensure its `path` child is a `string_literal` containing the cursor.
  - Return the path text with surrounding quotes stripped.
- **Verification:** `cargo build --manifest-path tools/xs-language-server/Cargo.toml` succeeds.

### Task 2.2: Expose `Workspace::resolve_include_for_file`
**TDD Cycle:** GREEN
- [x] Open `tools/xs-language-server/src/workspace.rs`.
- [x] Add public method `resolve_include_for_file(project: &VirtualProject, from_file: &Path, include_target: &str) -> Option<PathBuf>`.
- [x] Add private helper `relative_path_for_file(project: &VirtualProject, abs_path: &Path) -> String` that mirrors the precedence used by the merged view: overlay key lookup → `game_relative_path` → absolute path fallback.
- [x] Ensure `resolve_include_for_file` delegates to the existing `resolve_include(project, &from_rel, include_target)`.
- **Verification:** `cargo build --manifest-path tools/xs-language-server/Cargo.toml` succeeds.

### Task 2.3: Extend `goto_definition` in `server.rs` with the include branch
**TDD Cycle:** GREEN
- [x] Open `tools/xs-language-server/src/server.rs` at the existing `goto_definition` handler.
- [x] Insert a **Phase 0** include-aware branch immediately after the `text` lookup and **before** `word::identifier_at_cursor` (around line 649).
- [x] The branch must:
  - Convert the document URI to a `file` path; skip the branch if not `file://`.
  - Call `parser::detect_include_path_at_position(&current_file, &text, pos.line, pos.character)`.
  - On a detected include path, build or fetch the active `VirtualProject` via `self.workspace.lock().await` and `ws.lookup_mod(uri)` (falling back to `VirtualProject::default()`).
  - Call `ws.resolve_include_for_file(&project, &current_file, &include_target)`.
  - Convert the resolved `PathBuf` to a `Url`, build a `Location` with range `(0,0)-(0,0)`, and return `GotoDefinitionResponse::Scalar(location)`.
  - If resolution returns `None`, return `Ok(None)`.
- **Verification:** `cargo build --manifest-path tools/xs-language-server/Cargo.toml` succeeds.

### Task 2.4: Run the 5 RED tests and confirm they PASS
**TDD Cycle:** GREEN confirmation
- [x] Run the new integration tests again.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test include_goto_definition_repro -- --nocapture`. All five tests must PASS.

### Task 2.5: Run the full Rust test suite
**TDD Cycle:** GREEN confirmation
- [x] Run all LSP tests with `--no-fail-fast`.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast`. Total count should be **220** (215 existing + 5 new), all green.

---

## Phase 3: GREEN — Plugin forwarding regression test

### Task 3.1: Add `testIncludeStringLiteralDelegatesToResolver` in `XsGotoDeclarationHandlerTest.kt`
**TDD Cycle:** GREEN
- [x] Open `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt`.
- [x] Add the following test:
  ```kotlin
  fun testIncludeStringLiteralDelegatesToResolver() {
      val psiFile = myFixture.configureByText("a.xs", "include \"b.xs\";\n")
      val includeStringOffset = psiFile.text.indexOf('"') + 2
      val sourceElement = psiFile.findElementAt(includeStringOffset)!!

      val externalFile = myFixture.configureByText("b.xs", "void helper() {}\n")
      val externalTarget = externalFile.findElementAt(externalFile.text.indexOf("helper"))!!

      val handler = XsGotoDeclarationHandler(TestResolver(listOf(externalTarget)))
      val result = handler.getGotoDeclarationTargets(
          sourceElement,
          sourceElement.textOffset,
          myFixture.editor
      )

      assertSize(1, result ?: emptyArray())
      assertEquals("b.xs", result!![0].containingFile.name)
  }
  ```
- **Verification:** `./gradlew :compileTestKotlin` succeeds.

### Task 3.2: Run `./gradlew :test` (excluding the pre-existing env failure)
**TDD Cycle:** GREEN confirmation
- [x] Run the plugin unit-test suite.
- [x] If an environment-related test fails (e.g., missing IDE runtime or SDK path) unrelated to this change, capture it separately.
- **Verification:** `./gradlew :test`. `XsGotoDeclarationHandlerTest` passes; any env failure is documented as pre-existing.

### Task 3.3: Confirm the new plugin test passes
**TDD Cycle:** GREEN confirmation
- [x] Filter specifically to the goto-declaration handler tests if the broader suite has pre-existing failures.
- **Verification:** `./gradlew :test --tests "com.aomr.xs.navigation.XsGotoDeclarationHandlerTest"` reports `testIncludeStringLiteralDelegatesToResolver` green.

---

## Phase 4: REFACTOR — Document the new branches

### Task 4.1: Add doc-comment to `detect_include_path_at_position`
**TDD Cycle:** REFACTOR
- [x] In `tools/xs-language-server/src/parser.rs`, add a rustdoc comment above `detect_include_path_at_position` describing:
  - Purpose: detect whether the cursor is inside the path token of an `include_directive` and return the unquoted include target.
  - Parameters and return value.
  - The `_file` parameter's role (symmetry/diagnostics, currently unused).
- **Verification:** `cargo doc --manifest-path tools/xs-language-server/Cargo.toml --no-deps` builds without adding new warnings.

### Task 4.2: Add doc-comment to `Workspace::resolve_include_for_file`
**TDD Cycle:** REFACTOR
- [x] In `tools/xs-language-server/src/workspace.rs`, add a rustdoc comment above `resolve_include_for_file` describing:
  - That it resolves an include target from an absolute source file path.
  - How it computes the relative path and delegates to `resolve_include`.
  - What `None` means.
- **Verification:** `cargo doc --manifest-path tools/xs-language-server/Cargo.toml --no-deps` still clean.

### Task 4.3: Add doc-comment to the `goto_definition` include branch
**TDD Cycle:** REFACTOR
- [x] In `tools/xs-language-server/src/server.rs`, add a brief inline comment at the start of the new Phase 0 block.
- [x] Mention that the branch runs before identifier resolution and that `Ok(None)` falls back to existing behavior.
- **Verification:** `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml --all-targets` shows no new warnings attributable to this branch.

---

## Phase 5: VERIFY — Prove the fix is complete

### Task 5.1: Run `cargo test --no-fail-fast`
**TDD Cycle:** VERIFY
- [x] Run the full LSP test suite.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast` exits 0 with **220** tests passing.

### Task 5.2: Run `cargo clippy --all-targets`
**TDD Cycle:** VERIFY
- [x] Check for new warnings introduced by the change.
- **Verification:** `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml --all-targets` exits 0. Any warnings must match the pre-existing baseline (Task 0.2); new warnings are not allowed.

### Task 5.3: Run `./gradlew :test`
**TDD Cycle:** VERIFY
- [x] Run the plugin test suite.
- **Verification:** `./gradlew :test` passes, except for any pre-existing environment failures captured in Phase 3.

### Task 5.4: Bump `pluginVersion` 0.3.0 → 0.4.0
**TDD Cycle:** VERIFY
- [x] Open `tools/intellij-xs-plugin/gradle.properties`.
- [x] Change `pluginVersion = 0.3.0` to `pluginVersion = 0.4.0` (MINOR bump per AGENTS.md new-LSP-feature policy).
- **Verification:** `grep '^pluginVersion' tools/intellij-xs-plugin/gradle.properties` returns `0.4.0`.

### Task 5.5: Run `./gradlew :buildPlugin` and confirm the `.zip` artifact
**TDD Cycle:** VERIFY
- [x] Build the plugin distribution.
- **Verification:** `./gradlew :buildPlugin` succeeds and produces `tools/intellij-xs-plugin/build/distributions/intellij-xs-plugin-0.4.0.zip`.

### Task 5.6: Write the verify report
**TDD Cycle:** VERIFY documentation
- [x] Create `openspec/changes/fix-goto-definition-on-include-statements/verify-report.md`.
- [x] Include:
  - Summary of change.
  - Test results (cargo: 220/220; plugin: new regression test green).
  - Clippy warning status.
  - `pluginVersion` after bump.
  - Deviations from design (expected: none).
  - Manual verification notes (optional; note that real-editor Ctrl+Click is manual and not automated).
- **Verification:** Report file exists and data matches the actual verification outputs.

---

## Phase 6: DOCUMENT — Mark Issue #4 resolved

### Task 6.1: Update `docs/issues/2026-06-29-runtime-issues.md` Issue 4
**TDD Cycle:** DOCUMENT
- [x] Open `docs/issues/2026-06-29-runtime-issues.md`.
- [x] At Issue 4, change status from **Open** to **Resolved 2026-06-29** by `openspec/changes/fix-goto-definition-on-include-statements/`.
- [x] Add a one-line note that include-path go-to-definition now resolves through the LSP's `Workspace` pipeline and is forwarded by the existing `XsGotoDeclarationHandler`.
- **Verification:** `grep -A2 'Issue 4' docs/issues/2026-06-29-runtime-issues.md` shows the resolved status and cross-reference.

---

## Phase 7: COMMIT

### Task 7.1: Stage only intended files
**TDD Cycle:** COMMIT
- [x] Add the exact set of files expected for this change:
  - `tools/xs-language-server/src/server.rs` (modified)
  - `tools/xs-language-server/src/parser.rs` (modified)
  - `tools/xs-language-server/src/workspace.rs` (modified)
  - `tools/xs-language-server/tests/include_goto_definition_repro.rs` (new)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` (modified, +include test)
  - `tools/intellij-xs-plugin/gradle.properties` (version bump)
  - `docs/issues/2026-06-29-runtime-issues.md` (resolved note)
  - SDD artifacts: `openspec/changes/fix-goto-definition-on-include-statements/verify-report.md`, and this `tasks.md` if project convention commits SDD artifacts.
- [x] Ensure `scripts/deploy-mods.sh` is **not** staged.
- [x] Ensure any `cargo fmt` churn outside the touched lines is discarded or not staged.
- **Verification:** `git diff --cached --name-only` lists exactly the intended files.

### Task 7.2: Prepare commit message
**TDD Cycle:** COMMIT
- [x] Write commit message following Conventional Commits:
  ```
  feat(xs-lsp): navigate from include statement path token via textDocument/definition (Issue #4)
  ```
  Body (optional, but recommended):
  - Add include-aware branch in `goto_definition`.
  - Detect cursor in `include_directive` path token and resolve via `Workspace::resolve_include_for_file`.
  - Add LSP integration tests (R1-R5) and plugin forwarding regression test.
  - Bump plugin version 0.3.0 → 0.4.0.
- **Verification:** Commit message is ready; actual commit performed by the orchestrator after `sdd-verify`.

---

## Review Workload Forecast

- Files changed: ~7 files (3 LSP source files, 1 new LSP integration test, 1 plugin test file, `gradle.properties`, runtime issues doc, SDD verify report)
- New test files: 1
- LOC delta: ~120 (+100 / -5 tracked, +50 tests, +15 plugin test)
- Chained PRs: No
- 400-line budget risk: Low
- Decision needed: No
