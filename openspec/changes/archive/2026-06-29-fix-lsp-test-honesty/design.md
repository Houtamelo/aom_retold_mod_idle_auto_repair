# Design: `fix-lsp-test-honesty` — JSON-RPC frame parsing for the LSP roundtrip test

## Overview

This change rewrites the assertion logic in `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` so the end-to-end LSP gate inspects parsed JSON-RPC frames instead of raw-byte substrings. A new inline `wire_helpers` module provides Content-Length-framed parsing and typed queries; three dishonest assertions (R5-F-01 completion counting, R5-F-02 timing, R5-F-03 diagnostics flags) are replaced with parsed-message contracts. No production server code is touched and no public API is added. Expected delta: ~+180/-25 LOC in the binary and ~80 new LOC in the test file.

## Architecture Decisions

### ADR-1: Module placement

**Choice**: Add `wire_helpers` as an inline `mod wire_helpers { ... }` block at the top of `bin/lsp_roundtrip_test.rs`, immediately after the `use` statements.

**Rationale**: The binary is self-contained and the helpers are test-only. Keeping them in the same file avoids a new crate module, public surface area, and `Cargo.toml` changes.

### ADR-2: Reuse the existing `read_response_with_id` parsing path

**Choice**: `parse_lsp_messages(bytes)` walks Content-Length frames using the same byte-by-byte header/body logic already present in `read_framed_message` (lines 84-133). The existing `read_response_with_id` helper remains and shares that path; the new parser generalizes it to return notifications too.

**Rationale**: Two parsers would drift. Refactoring the existing frame reader into a shared primitive guarantees that `read_response_with_id` and the new batch parser agree on header boundaries, UTF-8 handling, and EOF semantics.

### ADR-3: `LspMessage` enum shape

**Choice**:

```rust
enum LspMessage {
    Request { id: i64, method: String, params: serde_json::Value },
    Response { id: i64, result_or_error: Result<serde_json::Value, serde_json::Value> },
    Notification { method: String, params: serde_json::Value },
}
```

`params`, `result`, and `error` are kept as `serde_json::Value`.

**Rationale**: The test only needs structural classification (id, method, URI, diagnostics array). Avoiding a full LSP type lattice keeps the refactor small and avoids dependencies on `tower_lsp::lsp_types` for every message kind.

### ADR-4: TDD transformation of the audit reproductions

The tests in `tests/r5_test_honesty_repro.rs` currently demonstrate the lie (they pass on buggy code). For strict TDD they become contract tests that fail before the fix and pass after:

| Current test | Transformed test | Assertion after fix |
|---|---|---|
| `r5_f01_completion_count_inflated_by_unrelated_log_message` | `r5_f01_completion_count_equals_parsed_result_count` | `assert_eq!(production_count, parsed_count)` on a stream where parsed count is 1 |
| `r5_f01_completion_count_passes_with_zero_real_completions` | same file, inverted | `assert_eq!(production_count, 0)` and `assert_eq!(parsed_count, 0)` |
| `r5_f02_production_timing_window_includes_50ms_post_write_sleep` | **drop** | Replaced by a new live integration test (see below) |
| `r5_f03_has_clean_diag_lies_on_injected_substring_noise` | `r5_f03_clean_diag_derives_from_parsed_notification` | `assert_eq!(production_has_clean_diag(raw), parsed_notification_exists(uri="file:///tmp/test.xs", empty=true))` |
| `r5_f03_has_bad_diag_lies_on_free_text_substring` | `r5_f03_bad_diag_derives_from_parsed_notification` | `assert_eq!(production_has_bad_diag(raw), parsed_notification_exists(uri="file:///tmp/bad.xs", non_empty=true))` |

The new R5-F-02 live test spawns `xs-language-server`, sends the references request, captures elapsed from post-flush to parsed-response-return, and asserts it is within 5 ms of the actual server turnaround. It will be RED on the current code because the current code includes the 50 ms sleep, and GREEN after the fix.

### ADR-5: Timing fix precision

**Choice**: Move `let start = Instant::now()` to immediately after `stdin.flush()` for the references request, then read the response with `read_response_with_id`, then `let elapsed = start.elapsed()` before writing shutdown/exit. Print label changes from `"returned in X ms"` to `"responded in X ms (request to response)"`.

**Rationale**: The elapsed interval must bracket [request written & flushed, response fully received]. The current window includes a 50 ms sleep, the shutdown/exit roundtrip, and `child.wait()`; the new window does not.

### ADR-6: No new public API

**Choice**: Keep all new types and helpers inside `bin/lsp_roundtrip_test.rs`. Do not add modules or exports to the LSP server crate library.

**Rationale**: The change is a test-harness refactor. Exposing helpers from `src/lib.rs` would create maintenance obligations and public semver surface for code that is only expected to be used by this binary.

## Data Flow / Sequence

```text
                    bin/lsp_roundtrip_test.rs
                           │
                           ▼
              ┌─────────────────────────────┐
              │  frame(request) ──► stdin   │
              │  start = Instant::now()     │
              └─────────────────────────────┘
                           │
                           ▼
              xs-language-server processes request
                           │
                           ▼
              stdout ◄── framed JSON-RPC response
                           │
                           ▼
              ┌─────────────────────────────┐
              │  parse_lsp_messages(&all_bytes)
              │       LspMessage::Response    │
              │       LspMessage::Notification│
              └─────────────────────────────┘
                           │
              ┌────────────┼────────────┐
              ▼            ▼            ▼
        find_response  find_notifications  elapsed = start.elapsed()
        (id=3 / 10)    (publishDiagnostics)
              │            │
              ▼            ▼
           assert      assert
```

## Code Structure Sketch

```rust
// At the top of bin/lsp_roundtrip_test.rs, after `use`:
mod wire_helpers {
    use serde_json::Value;

    pub enum LspMessage {
        Request { id: i64, method: String, params: Value },
        Response { id: i64, result_or_error: Result<Value, Value> },
        Notification { method: String, params: Value },
    }

    pub fn parse_lsp_messages(bytes: &[u8]) -> Vec<LspMessage> { /* Content-Length loop */ }
    pub fn find_response<'a>(messages: &'a [LspMessage], id: i64) -> Option<&'a LspMessage> { /* ... */ }
    pub fn find_notifications<'a>(messages: &'a [LspMessage], method: &str)
        -> impl Iterator<Item = &'a LspMessage> + 'a { /* ... */ }
}

// Sites using the helpers:
// - completion_count: count aiEcho-family labels in the Response with id=3.
// - has_clean_diag: any publishDiagnostics notification for file:///tmp/test.xs with empty diagnostics.
// - has_bad_diag: any publishDiagnostics notification for file:///tmp/bad.xs with non-empty diagnostics.
// - engine references timing: start after references flush, end after read_response_with_id(2001).
```

## Test Strategy

1. **R5-F-01 contract tests** (2 tests in `tests/r5_test_honesty_repro.rs`):
   - Rename and invert the two completion-count reproductions so they assert `production_count == parsed_count`.
   - These become RED when run against the current substring matcher and GREEN after the rewrite.

2. **R5-F-03 contract tests** (2 tests in the same file):
   - Rename and invert the two diagnostics reproductions so they assert that the production boolean equals a parsed-notification predicate.

3. **R5-F-02 live integration test** (new test file or appended to the same file):
   - Spawn the `xs-language-server` binary (mirrors `tests/game_folder_parse.rs`).
   - Send initialize → initialized → didOpen → references.
   - Capture `start` after the references flush and `elapsed` after `read_response_with_id(2001)` returns.
   - Assert `elapsed` is within 5 ms of the wall-clock response window and `<= 500 ms`.
   - Timeout: 5 seconds; deterministic because it does not measure cold-start.

4. **Full verification**:
   - `cargo test --manifest-path tools/xs-language-server/Cargo.toml` passes.
   - All transformed tests fail on the unfixed binary and pass on the fixed binary.

## File-by-File Change List

| File | Change |
|---|---|
| `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` | +180/-25: add inline `wire_helpers` module; rewrite completion, diagnostics, and references-timing assertions; fix print labels. |
| `tools/xs-language-server/tests/r5_test_honesty_repro.rs` | Rename and invert 4 reproductions per ADR-4; drop the always-passing R5-F-02 structural test; add ~80 LOC live server-spawn test. |

## Migration / Rollback

No migration required. `git revert <change-sha>` is fully safe; the change is a pure test-code refactor with no protocol, schema, cache-version, or deployed-mod behavior change.

## Open Questions

None anticipated.

## Risks

- **Parser edge cases**: the existing byte-by-byte Content-Length reader is already used for `read_response_with_id`; sharing it reduces but does not eliminate boundary-bug risk. Mitigation: the transformed R5-F-01/R5-F-03 tests exercise synthetic multi-frame streams.
- **Live R5-F-02 test flakiness on slow CI**: spawning a real server and measuring wall-clock time can be noisy. Mitigation: bound the test to 5 seconds, compare elapsed against the pre-shutdown response window (not cold-start), and allow a 5 ms tolerance.
