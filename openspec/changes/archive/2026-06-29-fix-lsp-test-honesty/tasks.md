# Tasks: fix-lsp-test-honesty — JSON-RPC frame parsing for the LSP roundtrip test

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~180 (+180/-25 in binary, ~80 in tests) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Chain strategy | pending |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Replace substring assertions with JSON-RPC frame parsing and add strict-TDD contract tests | PR 1 | Against `main`; single-file refactor in `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` plus transformed tests in `tools/xs-language-server/tests/r5_test_honesty_repro.rs` |

## Phase 1: RED — Write the failing contract tests

### Task 1.1: Write `r5_f01_completion_count_equals_parsed_result_count`
**TDD Cycle:** RED
- [ ] In `tools/xs-language-server/tests/r5_test_honesty_repro.rs`, transform `r5_f01_completion_count_inflated_by_unrelated_log_message` into a contract test.
- [ ] Build a stream with one `textDocument/completion` response containing an `aiEcho`-labeled entry and a later `window/logMessage` notification whose `params.message` is `"aiEcho"`.
- [ ] Assert that the roundtrip binary's completion count equals the count from parsing only the completion response `result` array (expected: 1).
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f01_completion_count_equals_parsed_result_count`. Expect RED on current code.

### Task 1.2: Write `r5_f01_completion_count_zero_when_no_completions`
**TDD Cycle:** RED
- [ ] In `tools/xs-language-server/tests/r5_test_honesty_repro.rs`, transform `r5_f01_completion_count_passes_with_zero_real_completions` into a contract test.
- [ ] Build a stream with NO completion response and only a `window/logMessage` containing `"aiEcho"`.
- [ ] Assert that both the roundtrip binary's completion count and the parsed completion count are zero.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f01_completion_count_zero_when_no_completions`. Expect RED on current code.

### Task 1.3: Write `r5_f03_clean_diag_derived_from_parsed_notifications`
**TDD Cycle:** RED
- [ ] In `tools/xs-language-server/tests/r5_test_honesty_repro.rs`, transform `r5_f03_has_clean_diag_lies_on_injected_substring_noise` into a contract test.
- [ ] Use a `textDocument/publishDiagnostics` notification for `file:///tmp/test.xs` with empty `diagnostics`.
- [ ] Assert that `has_clean_diag` is true iff a parsed notification with that URI and empty diagnostics exists; it must not be satisfied by raw-byte substring noise.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f03_clean_diag_derived_from_parsed_notifications`. Expect RED on current code.

### Task 1.4: Write `r5_f03_bad_diag_derived_from_parsed_notifications`
**TDD Cycle:** RED
- [ ] In `tools/xs-language-server/tests/r5_test_honesty_repro.rs`, transform `r5_f03_has_bad_diag_lies_on_free_text_substring` into a contract test.
- [ ] Use a `textDocument/publishDiagnostics` notification for `file:///tmp/bad.xs` with non-empty `diagnostics`.
- [ ] Assert that `has_bad_diag` is true iff a parsed notification with that URI and non-empty diagnostics exists; it must not be satisfied by free-text substring matches.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f03_bad_diag_derived_from_parsed_notifications`. Expect RED on current code.

### Task 1.5: Write `r5_f02_references_elapsed_measures_only_response_window`
**TDD Cycle:** RED
- [ ] Add a new live integration test that spawns the `xs-language-server` binary, sends initialize → initialized → didOpen → references, and measures wall-clock time.
- [ ] Start the timer immediately after the references request is flushed; stop it immediately after the `id=2001` response is read but before shutdown/exit is sent.
- [ ] Assert that `production_elapsed` is within 5 ms of the actual response window and `<= 500 ms`.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f02_references_elapsed_measures_only_response_window`. Expect RED on current code because the current window includes the post-write 50 ms sleep.

### Task 1.6: Remove `r5_f02_production_timing_window_includes_50ms_post_write_sleep`
**TDD Cycle:** RED cleanup
- [ ] Delete the audit-only structural test `r5_f02_production_timing_window_includes_50ms_post_write_sleep` from `tools/xs-language-server/tests/r5_test_honesty_repro.rs`; it is replaced by the live contract test in Task 1.5.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- --list` no longer lists the removed test.

### Task 1.7: Confirm RED state across all new tests
**TDD Cycle:** RED confirmation
- [ ] Run all rewritten and added contract tests against the unfixed code.
- [ ] Capture failing test names, elapsed values, and assertion output for use in Phase 4 verification.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- --nocapture`. All new tests must fail on current code.

## Phase 2: GREEN — Implement `wire_helpers` and rewrite production assertions

### Task 2.1: Add `wire_helpers` module to the roundtrip binary
**TDD Cycle:** GREEN
- [ ] Insert `mod wire_helpers { ... }` near the top of `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs`, immediately after the `use` statements.
- [ ] Define `pub enum LspMessage { Request { ... }, Response { ... }, Notification { ... } }` using `serde_json::Value` payloads.
- [ ] Implement `pub fn parse_lsp_messages(bytes: &[u8]) -> Vec<LspMessage>`, `pub fn find_response(...) -> Option<&LspMessage>`, and `pub fn find_notifications(...) -> impl Iterator<Item = &LspMessage>`.
- [ ] Reuse the Content-Length framing logic already present in `read_framed_message` (lines 84-133) so `read_response_with_id` and the new batch parser share the same boundary handling.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro`. Tests must compile and begin turning GREEN.

### Task 2.2: Replace the completion substring sum with a parsed query
**TDD Cycle:** GREEN
- [ ] In `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs`, replace lines 401-411 with a query over parsed messages.
- [ ] Use `parse_lsp_messages(&all_bytes)` and `find_response(..., completion_id)` to locate the completion response; count entries in `result` whose `label` is `aiEcho`, `aiEchoCategory`, or `aiEchoWarning`.
- [ ] Preserve the existing `>= 3` pass threshold; only the counting source changes.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f01_completion_count_equals_parsed_result_count r5_f01_completion_count_zero_when_no_completions`. Expect GREEN.

### Task 2.3: Replace diagnostic substring checks with parsed notification queries
**TDD Cycle:** GREEN
- [ ] In `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs`, replace lines 362-380 with queries over notifications from `parse_lsp_messages(&all_bytes)`.
- [ ] Set `has_clean_diag = true` iff a `textDocument/publishDiagnostics` notification has `params.uri == "file:///tmp/test.xs"` and `params.diagnostics` is empty.
- [ ] Set `has_bad_diag = true` iff such a notification for `"file:///tmp/bad.xs"` has non-empty diagnostics.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f03_clean_diag_derived_from_parsed_notifications r5_f03_bad_diag_derived_from_parsed_notifications`. Expect GREEN.

### Task 2.4: Fix the references timing window at lines 1659-1676
**TDD Cycle:** GREEN
- [ ] Move `let start = Instant::now();` to immediately after the references request `stdin.flush()` at line 1661.
- [ ] Read the `id=2001` response and capture `let elapsed = start.elapsed();` before writing shutdown/exit, so the interval excludes the 50 ms sleep, shutdown roundtrip, and `child.wait()` cleanup.
- [ ] Change the console label from `"returned in X ms"` to `"responded in X ms (request to response)"`.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f02_references_elapsed_measures_only_response_window`. Expect GREEN.

### Task 2.5: Confirm GREEN state across all new tests
**TDD Cycle:** GREEN confirmation
- [ ] Run the full roundtrip test file after the three production-site rewrites.
- [ ] Verify that every transformed R5-F-01, R5-F-02, and R5-F-03 contract test now passes.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- --nocapture`. All new tests must pass.

## Phase 3: REFACTOR — Tighten, deduplicate, and document

### Task 3.1: Extract a completion assertion helper
**TDD Cycle:** REFACTOR
- [ ] Extract a private helper `assert_completion_returns_aiEcho(messages: &[LspMessage], expected_min: usize)` used by both R5-F-01 contract tests.
- [ ] The helper should locate the completion response, count aiEcho-family labels, and assert `>= expected_min` with a clear failure message.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f01_completion_count_equals_parsed_result_count r5_f01_completion_count_zero_when_no_completions`. Still GREEN.

### Task 3.2: Extract a diagnostics assertion helper
**TDD Cycle:** REFACTOR
- [ ] Extract a private helper `assert_diagnostics_published(messages: &[LspMessage], uri: &str, expected_non_empty: bool)` used by both R5-F-03 contract tests.
- [ ] The helper should search `textDocument/publishDiagnostics` notifications for `params.uri == uri` and check whether `params.diagnostics` is empty or non-empty as expected.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r5_test_honesty_repro -- r5_f03_clean_diag_derived_from_parsed_notifications r5_f03_bad_diag_derived_from_parsed_notifications`. Still GREEN.

### Task 3.3: Document the `parse_lsp_messages` framing invariant
**TDD Cycle:** REFACTOR
- [ ] Add a rustdoc comment on `pub fn parse_lsp_messages` describing the Content-Length framing invariant: input is a sequence of `Content-Length: N\r\n\r\n<N bytes>` frames; malformed frames are skipped; the function returns one `LspMessage` per successfully parsed JSON body.
- [ ] Note that the function is lenient and does not fail the caller on stray bytes between frames.
- **Verification:** `cargo doc --manifest-path tools/xs-language-server/Cargo.toml --no-deps` builds without warnings.

### Task 3.4: Enforce formatting and a clean build
**TDD Cycle:** REFACTOR
- [ ] Run `cargo fmt --manifest-path tools/xs-language-server/Cargo.toml`.
- [ ] Run `cargo build --manifest-path tools/xs-language-server/Cargo.toml` to confirm the crate and binary compile cleanly.
- **Verification:** Both commands exit with status 0.

## Phase 4: VERIFY — Prove the rewrite is correct

### Task 4.1: Run the full Rust test suite
**TDD Cycle:** VERIFY
- [ ] Run all unit and integration tests for the LSP server crate.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml`. Expect all tests green, including the new contract tests, the unchanged deadlock-reproduction tests, and existing LSP unit tests.

### Task 4.2: Run the roundtrip binary end-to-end
**TDD Cycle:** VERIFY
- [ ] Execute the roundtrip binary as a standalone process.
- [ ] Confirm it exits 0 and prints the updated `"responded in X ms (request to response)"` label for engine references.
- **Verification:** `cargo run --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test`. Exit code must be 0.

### Task 4.3: Mutation-test spot-check for completion isolation
**TDD Cycle:** VERIFY
- [ ] Construct a synthetic byte stream that injects a `window/logMessage` notification with `params.message = "aiEcho"` between the completion response and the shutdown message.
- [ ] Confirm that the new completion query does not count the log message as a completion entry.
- [ ] Document the check as an inline code comment in `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` or `tools/xs-language-server/tests/r5_test_honesty_repro.rs` if it is not automated.
- **Verification:** The spot-check output shows `completion_count == parsed_count` with the injected log message present, or the attached comment clearly describes the scenario.

## Review Workload Forecast
- Decision needed before apply: No
- Chained PRs recommended: No
- 400-line budget risk: Low
- Justification: ~180 LOC delta in a single file; no protocol or schema changes; existing tests are replaced not added (per ADR-4 in design).
