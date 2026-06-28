# Verify Report: xs-lsp-0.1.7-diagnostics-ux

## Change

| Field | Value |
|-------|-------|
| Change ID | `xs-lsp-0.1.7-diagnostics-ux` |
| Branch / commit | `xs-lsp-0.1.7-diagnostics-ux` @ `462516a` |
| Verify mode | Strict TDD (tooling override in `openspec/config.yaml`) |
| Test runner | `cargo test --manifest-path tools/xs-language-server/Cargo.toml` |
| Test command used | `cargo test --manifest-path tools/xs-language-server/Cargo.toml --lib` and full `cargo test ...` plus `cargo run --bin lsp_roundtrip_test` |

## Completeness

| Task | Status | Evidence |
|------|--------|----------|
| 1.1 Engine-definition test expects `null` | ✅ Complete | `lsp_roundtrip_test.rs` updated to assert `is_null && no_stub_uri` |
| 1.2 `goto_definition` returns `Ok(None)` for engine API | ✅ Complete | `server.rs` engine-only branch returns `Ok(None)` |
| 1.3 Verify engine definition | ✅ Passing | Roundtrip prints `PASS: definition returned null for engine-API symbol (aiEcho)` |
| 2.1 RED test for empty publishDiagnostics | ✅ Complete | `lsp_roundtrip_test.rs` `DID_CHANGE_BAD_FIX` + empty publish assertion |
| 2.2 `collect_all` inserts empty entry | ✅ Complete | `diagnostics.rs:110-114` `diags.entry(uri.clone()).or_default();` |
| 2.3 Refactor capability comments | ✅ Complete | `server.rs:239-249` comments aligned |
| 3.1-3.2 RED tests for parse message shapes | ✅ Complete | `diagnostics.rs` tests for missing `;`, `}`, unexpected identifier, missing RHS, unterminated string, stray `;` |
| 3.3 Rewrite `to_diagnostic` | ✅ Complete | `diagnostics.rs` uses `missing_token_message`, `unexpected_token_message`, `friendly_kind` |
| 3.4 Verify game folder parse | ✅ Passing | `cargo test --test game_folder_parse`: 9/9 pass |
| 4.1 RED test for engine-symbol references | ✅ Complete | `lsp_roundtrip_test.rs` `run_engine_references_test` |
| 4.2 Workspace scan for engine references | ✅ Complete | `server.rs::references` walks `project.visible_files` and uses `find_identifier_uses` |
| 4.3 Time budgets | ✅ Satisfied | Engine references returned 2 locations in 52 ms (≤500 ms) |
| 4.4 Verify references | ✅ Passing | Roundtrip prints `PASS (engine references): returned 2 aiEcho location(s) in 52 ms` |
| 5.1 Version bump | ✅ Complete | `tools/intellij-xs-plugin/gradle.properties:12` → `pluginVersion = 0.1.7` |
| 5.2 Final full tests | ✅ Passing | `cargo test`: 187 pass; roundtrip: 44 PASS / 2 pre-existing FAIL |

## Build / Test / Coverage Evidence

### Command 1 — Library unit tests
```bash
cargo test --manifest-path tools/xs-language-server/Cargo.toml --lib 2>&1 | tee /tmp/lib_tests.log
```
Result: **178 passed; 0 failed; 0 ignored**. New parse-error tests all green.

### Command 2 — Full `cargo test`
```bash
cargo test --manifest-path tools/xs-language-server/Cargo.toml
```
Result: **all green** (178 lib + 9 integration + 0 doc tests).

### Command 3 — Game-folder integration test
```bash
AOMR_GAME_PATH="$HOME/.steam/steamapps/common/Age of Mythology Retold" \
  cargo test --manifest-path tools/xs-language-server/Cargo.toml --test game_folder_parse
```
Result: **9 passed; 0 failed**. Zero unexpected parse errors and zero unresolved symbols.

### Command 4 — LSP roundtrip binary
```bash
cargo run --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test > /tmp/roundtrip_exit.log 2>&1; echo EXIT:$?
```
Result: **EXIT:1** due to two pre-existing semantic fixture failures (`float_to_int_loss`, `extern_collision`).

Line counts from `/tmp/roundtrip.log`:
- `^PASS`: 44
- `^FAIL`: 2

The two failing assertion groups are the same failures present on `master`; they are **not** touched by this change (`git diff master..xs-lsp-0.1.7-diagnostics-ux -- tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` does not modify those test cases).

### Coverage
No coverage tool is configured for this project. Coverage analysis skipped.

## Spec Compliance Matrix

| Spec | Requirement / Scenario | Test Evidence | Status |
|------|------------------------|---------------|--------|
| `spec-stale-diagnostic-clearing` | Empty publish when last issue fixed | `lsp_roundtrip_test.rs` `bad_clean_empty` assertion passes: `"diagnostics":[]` + `"version":2` for `/tmp/bad.xs` | ✅ PASS |
| `spec-stale-diagnostic-clearing` | Partial fix still publishes remaining errors | Covered by design (`diagnostics.rs` retains per-category entries) but not by an explicit scenario test | ⚠️ SUGGESTION: add direct test |
| `spec-stale-diagnostic-clearing` | Clean file after previous diagnostics | Same mechanism as above exercised in roundtrip | ✅ PASS |
| `spec-actionable-parse-errors` | Missing `;` → `Missing ';'` | `parse_message_for_missing_semicolon` passes | ✅ PASS |
| `spec-actionable-parse-errors` | Missing `}` → `Missing '}'` | `parse_message_for_missing_closing_brace` passes | ✅ PASS |
| `spec-actionable-parse-errors` | Unexpected identifier → `Unexpected identifier '<name>'` | `parse_message_for_unexpected_identifier` passes | ✅ PASS |
| `spec-actionable-parse-errors` | Messages MUST NOT contain `MISSING`, `ERROR`, `node`, `column(s)`, grammar non-terminals | `assert_no_internals` in all parse-message tests passes; `bad.xs` diagnostics show `Unexpected token 'int'`, `Unexpected token ';'`, `Missing ')'` only | ✅ PASS |
| `spec-engine-symbol-definition` | Engine symbol `aiEcho` definition returns `null` | Roundtrip `definition returned null for engine-API symbol (aiEcho)` passes | ✅ PASS |
| `spec-engine-symbol-definition` | Workspace symbol still resolves | Roundtrip `definition returned real file:line for workspace function` passes | ✅ PASS |
| `spec-engine-symbol-definition` | No `xs-stub://` URI in any definition | Verified by `no_stub_uri` checks and grep returns no `xs-stub` in `server.rs` | ✅ PASS |
| `spec-engine-symbol-references` | `aiEcho` references return workspace use sites | `run_engine_references_test` passes with 2 locations | ✅ PASS |
| `spec-engine-symbol-references` | `includeDeclaration: false` excludes declaration | Engine symbols have no workspace declaration; returned locations are use sites only | ✅ PASS |
| `spec-engine-symbol-references` | Completes within 500 ms for ≤10 files | Measured 52 ms | ✅ PASS |

## Design Coherence

| Design Decision | Implementation | Status |
|-----------------|----------------|--------|
| AD-1 Empty-publish for clean files | `diagnostics.rs:110-114` unconditional `or_default()`; `publish_diagnostics` iterates map | ✅ Aligned |
| AD-2 Parse-error message formatting | `missing_token_message`, `unexpected_token_message`, `friendly_kind` helpers; fallback to line/column | ✅ Aligned |
| AD-3 Engine-symbol references via workspace scan | `server.rs::references` uses `project.visible_files`, `find_identifier_uses`, `seen` dedupe, per-file/total budget guards | ✅ Aligned |
| AD-4 Engine-symbol definition returns `null` | `server.rs::goto_definition` returns `Ok(None)` after engine-only resolution | ✅ Aligned |

## Issues

### CRITICAL
None.

### WARNING
None.

### SUGGESTION / OUT-OF-SCOPE
- **Pre-existing `lsp_roundtrip_test` failures**: `semantic float_to_int_loss` and `semantic extern_collision` fail on this branch, but the same failures exist on `master` and are not modified by this change. They should be fixed separately.
- **`cargo` warnings**: 7 compiler warnings (unused imports/variables, deprecated fields, dead code) remain. None block the build or tests; the one in a changed file is `server.rs:1071` unused variable `path`.
- **Spec gap**: `spec-stale-diagnostic-clearing` “partial diagnostic fix” scenario has no explicit test. The implementation naturally handles it, but a targeted regression test would strengthen coverage.

## TDD Compliance

| Check | Result | Details |
|-------|--------|---------|
| TDD Evidence reported | ✅ | Found in Engram apply-progress observation #1383 |
| All tasks have tests | ✅ | 5 phase tasks + follow-ups all have RED/GREEN evidence |
| RED confirmed (tests exist) | ✅ | All RED commits add or update test assertions/files still present |
| GREEN confirmed (tests pass) | ✅ | 187 `cargo test` cases pass; roundtrip PASS lines = 44 |
| Triangulation adequate | ✅ | Parse-error behavior covered by 6 unit tests across distinct error shapes |
| Safety Net for modified files | ✅ | Full `cargo test` and `game_folder_parse` exercise existing code paths |

**TDD Compliance**: strict

## Test Layer Distribution

| Layer | Tests / Functions | Files | Tool |
|-------|-------------------|-------|------|
| Unit | 178 (including 11 in `diagnostics.rs`) | `tools/xs-language-server/src/diagnostics.rs` | `cargo test --lib` |
| Integration | 9 (`game_folder_parse.rs`) + 5 scenario functions in `lsp_roundtrip_test.rs` | `tests/game_folder_parse.rs`, `src/bin/lsp_roundtrip_test.rs` | `cargo test`, `cargo run --bin lsp_roundtrip_test` |
| E2E | 0 | — | — |
| **Total** | **187 cargo tests + 44 roundtrip PASS assertions** | | |

## Changed File Coverage

Coverage analysis skipped — no coverage tool detected.

## Quality Metrics

| Tool | Result |
|------|--------|
| **Linter** | Not configured separately; `cargo build/test` emits 7 warnings, 0 errors |
| **Type Checker** | `cargo test` compiles successfully; no type errors |

## Previous Fail Follow-up

The prior verify pass reported FAIL for these items. Current status:

| Prior Item | Current Status | Notes |
|------------|----------------|-------|
| Issue #1: stale diagnostics / empty publish | **FIXED** | `diagnostics.rs:110-114` inserts empty entry; roundtrip `clean follow-up change publishes empty diagnostics for /tmp/bad.xs` passes |
| Issue #6: actionable parse errors leaking internals | **FIXED** | New unit tests pass; `bad.xs` diagnostics use `Unexpected token '...'` / `Missing ')'` with no `ERROR`/`MISSING`/`node`/`column(s)`/`primitive_type` |
| Integration test asserts old `"Parse error"` string | **FIXED** | `lsp_roundtrip_test.rs:365-366` now checks `"Parse error" \|\| "Missing" \|\| "Unexpected"` |
| Doc comments at `server.rs:240-245` and `:569-571` | **FIXED** | Comments describe engine symbols returning `null` / references scanning workspace; no `xs-stub://engine/<name>` or "virtual URI" wording remains |
| Pre-existing `semantic float_to_int_loss` and `semantic extern_collision` | **OUT-OF-SCOPE** | Confirmed on `master`; diff of `lsp_roundtrip_test.rs` does not touch those test cases |

## Final Verdict

**Status: PASS**

All spec-actionable items from the prior FAIL report are resolved. `cargo test` passes, game-folder integration passes, and the new/changed LSP roundtrip assertions pass. The `lsp_roundtrip_test` binary still exits 1 only because of two pre-existing semantic fixture failures that are unrelated to this change and reproducible on `master`. The implementation is ready for archive.
