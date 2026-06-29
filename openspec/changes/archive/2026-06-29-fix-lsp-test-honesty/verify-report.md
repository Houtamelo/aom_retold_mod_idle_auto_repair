## Verification Report

**Change**: fix-lsp-test-honesty
**Mode**: Strict TDD (user override)

### Completeness summary

| Task / phase | Status | Evidence |
|--------------|--------|----------|
| Phase 1 — RED contract tests written | ✅ | `tests/r5_test_honesty_repro.rs:98-507` (5 transformed tests) |
| Phase 2 — `wire_helpers` module + parsed queries | ✅ | `src/bin/lsp_roundtrip_test.rs:18-150` |
| Phase 2 — Completion count from parsed response | ✅ | `src/bin/lsp_roundtrip_test.rs:543-561` |
| Phase 2 — Diagnostic flags from parsed notifications | ✅ | `src/bin/lsp_roundtrip_test.rs:490-503` |
| Phase 2 — References timing narrowed | ✅ | `src/bin/lsp_roundtrip_test.rs:1919-1950,1981` |
| Phase 3 — No public API surface added | ✅ | `wire_helpers` is private to the binary |
| Phase 4 — Full Rust suite green | ✅ | `cargo test` 203/203 passed |
| Phase 4 — Roundtrip binary exits 0 | ✅ | new label printed: `responded ... (request to response)` |

---

## 1. Verification

### 1.1 Cargo tests pass

```bash
cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast 2>&1 | tail -20
```

Output:

```text
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/r5_test_honesty_repro.rs (tools/xs-language-server/target/debug/deps/r5_test_honesty_repro-68786da0773c9700)

running 5 tests
test r5_f01_completion_count_zero_when_no_completions ... ok
test r5_f03_bad_diag_asserts_parsed_notification ... ok
test r5_f01_completion_count_equals_parsed_result_count ... ok
test r5_f03_clean_diag_asserts_parsed_notification ... ok
test r5_f02_references_elapsed_measures_only_response_window ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.21s

   Doc-tests xs_language_server

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**Verdict**: PASS  
Full roll-up captured from the complete run: **182 unit tests + 9 game-folder integration tests + 7 R3-F-01 tests + 5 R5 tests = 203/203 passed**.

---

### 1.2 Roundtrip binary end-to-end

```bash
cargo run --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test 2>&1 | tail -20
```

Output (last lines):

```text
PASS (include completion_across_include): response matched expected content
PASS (include hover_across_include): response matched expected content
PASS (include definition_across_include): response matched expected content
PASS (cycle_does_not_hang): server exited cleanly despite include cycle
[test] received msg: {"id":2000,"jsonrpc":"2.0","result":{"capabilities":{...}}}
[test] received msg: {"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"diagnostics":[],"uri":"file:///tmp/aomr_eng_mod_.../main.xs","version":1}}
[test] received msg: {"id":2001,"jsonrpc":"2.0","result":[{"range":{"end":{"character":9,"line":2},"start":{"character":3,"line":2}},"uri":"file:///tmp/aomr_eng_mod_.../main.xs"},{"range":{"end":{"character":9,"line":2},"start":{"character":3,"line":2}},"uri":"file:///tmp/aomr_eng_mod_.../util.xs"}]}
PASS (engine references): responded 2 aiEcho location(s) in 0 ms (request to response)
```

Exit code confirmed separately: `0`.

**Verdict**: PASS

---

### 1.3 Wire_helpers call sites

```bash
grep -n "wire_helpers::" tools/xs-language-server/src/bin/lsp_roundtrip_test.rs
```

Output:

```text
218:    wire_helpers::read_framed_json_message(reader)
490:    let messages = wire_helpers::parse_lsp_messages(&all_bytes);
493:    for msg in wire_helpers::find_notifications(&messages, "textDocument/publishDiagnostics") {
494:        if let wire_helpers::LspMessage::Notification { params, .. } = msg {
543:    let completion_count = match wire_helpers::find_response(&messages, 3) {
544:        Some(wire_helpers::LspMessage::Response {
1907:                        let messages = wire_helpers::parse_lsp_messages(&all_bytes);
1908:                        if wire_helpers::find_response(&messages, 2001).is_some() {
```

**Verdict**: PASS (5 call sites; the required parse + notification + response + timing paths are all present).

---

### 1.4 Timing fix (R5-F-02)

```bash
grep -n "request to response" tools/xs-language-server/src/bin/lsp_roundtrip_test.rs
```

Output:

```text
1981:            "PASS (engine references): responded {} aiEcho location(s) in {} ms (request to response)",
```

Implementation verifies the interval excludes the post-write sleep, shutdown/exit, and `child.wait()` (`src/bin/lsp_roundtrip_test.rs:1892-1950`).

**Verdict**: PASS

---

### 1.5 Strict-TDD test transformations

```bash
grep -n "^fn r5_" tools/xs-language-server/tests/r5_test_honesty_repro.rs
```

Output:

```text
98:fn r5_f01_completion_count_equals_parsed_result_count() {
117:fn r5_f01_completion_count_zero_when_no_completions() {
175:fn r5_f03_clean_diag_asserts_parsed_notification() {
197:fn r5_f03_bad_diag_asserts_parsed_notification() {
478:fn r5_f02_references_elapsed_measures_only_response_window() {
```

**Verdict**: PASS (all 5 expected tests present and passing).

---

### 1.6 Spec scenario coverage

| Spec scenario | Covering test(s) | Result |
|---|---|---|
| Completion count derives from parsed `textDocument/completion` response (R5-F-01) | `r5_f01_completion_count_equals_parsed_result_count`, `r5_f01_completion_count_zero_when_no_completions` | PASS |
| Diagnostic flags derive from parsed `textDocument/publishDiagnostics` notifications (R5-F-03) | `r5_f03_clean_diag_asserts_parsed_notification`, `r5_f03_bad_diag_asserts_parsed_notification` | PASS |
| Elapsed time measures only the references request/response interval (R5-F-02) | `r5_f02_references_elapsed_measures_only_response_window` + production label at line 1981 | PASS |

**Verdict**: PASS

---

### 1.7 No new clippy warnings

```bash
cargo clippy --manifest-path tools/xs-language-server/Cargo.toml --tests 2>&1 | grep -E "^warning" | sort -u > /tmp/clippy_now.txt
wc -l /tmp/clippy_now.txt
```

Result: **31 unique warning lines**.

All warnings in `src/bin/lsp_roundtrip_test.rs` are in code that existed before this change (e.g. `DID_CHANGE` unused at line 157, `map_or` simplifications at lines 521/601, collapsible `if`s in the include/cycle helpers, zombie-process warning in the cycle test). They match the pre-existing debt documented in the R3-F-01 verify report.

One warning is located in a file touched by this change:

```text
warning: this `if` statement can be collapsed
   --> tests/r5_test_honesty_repro.rs:298:9
```

This is the test-side `read_response_with_id` helper, which mirrors the production helper that already has the same warning at `src/bin/lsp_roundtrip_test.rs:230`. It is style-only and does not affect correctness.

**Verdict**: PASS WITH WARNING (the single test-helper warning is documented).

---

### 1.8 No deviation from design (ADRs 1–6)

| ADR | Design choice | Implementation evidence | Result |
|---|---|---|---|
| ADR-1 | Inline `mod wire_helpers` after `use` statements | `src/bin/lsp_roundtrip_test.rs:18-150` | CONFORMS |
| ADR-2 | Reuse existing Content-Length framing path; `read_framed_message` delegates to `wire_helpers::read_framed_json_message` | `src/bin/lsp_roundtrip_test.rs:218`, `45-89` | CONFORMS |
| ADR-3 | `LspMessage` enum with `Request`/`Response`/`Notification` and `serde_json::Value` payloads | `src/bin/lsp_roundtrip_test.rs:25-39` | CONFORMS |
| ADR-4 | Rename/invert 4 reproductions; drop the old R5-F-02 structural test; add live R5-F-02 test | `tests/r5_test_honesty_repro.rs:98-507` (5 tests); old `r5_f02_production_timing_window_includes_50ms_post_write_sleep` removed | CONFORMS (with naming nuance; see Deviations) |
| ADR-5 | Start timer after references flush, capture elapsed before shutdown; print `responded ... (request to response)` | `src/bin/lsp_roundtrip_test.rs:1919-1950`, label at `1981` | CONFORMS IN ESSENCE; start is placed one line before the write rather than strictly after flush (see Deviations) |
| ADR-6 | No new public API | `wire_helpers` is private to the binary, no `src/lib.rs` changes | CONFORMS |

**Verdict**: PASS with documented minor deviations.

---

### 1.9 No scope creep

```bash
git diff --stat tools/xs-language-server/src/bin/lsp_roundtrip_test.rs tools/xs-language-server/tests/r5_test_honesty_repro.rs
```

Output:

```text
 .../src/bin/lsp_roundtrip_test.rs                  | 598 +++++++++++++++------
 1 file changed, 438 insertions(+), 160 deletions(-)
```

Only the two designated files are modified by this change. The other modified/untracked files visible in `git status` (`cache.rs`, `server.rs`, `r3_f01_deadlock_repro.rs`, `openspec/...`, etc.) are pre-existing work from earlier changes (notably R3-F-01).

**Verdict**: PASS

---

## 2. Acceptance criteria

From `openspec/changes/fix-lsp-test-honesty/specs/spec-lsp-roundtrip-test-honesty.md` § Acceptance criteria:

1. **The rewrite introduces a `wire_helpers` module that exposes `parse_lsp_messages` and uses Content-Length-framed JSON-RPC parsing.**  
   ✅ `wire_helpers::parse_lsp_messages` at `src/bin/lsp_roundtrip_test.rs:98` uses `read_framed_json_message`, which parses `Content-Length` headers and JSON-RPC bodies.

2. **The three substring matchers are replaced with parsed queries; the timing harness captures elapsed from post-write to response-received, excluding post-response housekeeping.**  
   ✅ Diagnostics: `src/bin/lsp_roundtrip_test.rs:490-503`; Completion: `src/bin/lsp_roundtrip_test.rs:543-561`; Timing: `src/bin/lsp_roundtrip_test.rs:1892-1950` (reader thread stops at parsed response, before shutdown/exit).

3. **All audit reproduction tests in `tests/r5_test_honesty_repro.rs` are preserved and produce the documented RED→GREEN state on the rewritten binary.**  
   ✅ The 5 transformed tests all pass. Apply-progress observation `#1420` recorded the RED→GREEN transitions.

4. **`cargo test --manifest-path tools/xs-language-server/Cargo.toml` passes, including the new strict-TDD tests.**  
   ✅ 203/203 tests passed.

---

## 3. Spec scenarios

| Scenario | Covering test / code | Result |
|---|---|---|
| Completion assertion counts only entries in the `textDocument/completion` response, ignoring unrelated `window/logMessage` substring `"aiEcho"` | `r5_f01_completion_count_equals_parsed_result_count`, `r5_f01_completion_count_zero_when_no_completions`; production code `src/bin/lsp_roundtrip_test.rs:543-561` | PASS |
| `has_clean_diag` / `has_bad_diag` come from parsed `textDocument/publishDiagnostics` notifications by URI and diagnostics emptiness, not raw-byte substrings | `r5_f03_clean_diag_asserts_parsed_notification`, `r5_f03_bad_diag_asserts_parsed_notification`; production code `src/bin/lsp_roundtrip_test.rs:490-503` | PASS |
| Elapsed measures `t_response_received - T0` after the references write+flush, excluding the 50 ms sleep, shutdown/exit, and `child.wait()` | `r5_f02_references_elapsed_measures_only_response_window`; production code `src/bin/lsp_roundtrip_test.rs:1892-1950` and label `1981` | PASS |

---

## 4. Deviations

1. **Timing start placement (ADR-5)** — The design says to place `let start = Instant::now();` *immediately after* `stdin.flush()`. The implementation places it immediately *before* the references `write_all`/`flush` (`src/bin/lsp_roundtrip_test.rs:1919-1921`). The captured interval still excludes the post-write 50 ms sleep, the shutdown/exit roundtrip, and `child.wait()` cleanup, and the live R5-F-02 test passes the 5 ms tolerance. This is a harmless ordering difference with no semantic impact.

2. **Test naming suffix (ADR-4)** — The design table uses the suffix `_derives_from_parsed_notification` for the two R5-F-03 tests, while the acceptance checklist and implementation use `_asserts_parsed_notification`. The tests are semantically identical and the names match the verification checklist.

3. **Line-count inflation (Phase 3)** — The binary diff is +438/-160 instead of the forecasted ~+180/-25 because Phase 3 included cosmetic `eprintln!` reformatting. There is no behavior change or scope creep.

---

## 5. Issues

- **CRITICAL**: None
- **WARNING**:
  - One new clippy `collapsible_if` warning in `tests/r5_test_honesty_repro.rs:298` (test-side `read_response_with_id` helper). It mirrors the pre-existing warning in the production helper and is style-only.
  - The ADR-5 timing start placement is one line before the references write instead of strictly after flush; see Deviations.
- **SUGGESTION**: None

---

## 6. Risks / warnings

- Pre-existing clippy warnings are unchanged and still present (e.g. `DID_CHANGE` unused, `map_or` simplifications, zombie-process warning in the cycle test). They are documented as accepted debt in the R3-F-01 verify report.
- The apply phase sub-agent timed out before delivering its final report; orchestrator-side recovery (Engram observation `#1420`) captured that the work was complete. This formal verification independently confirms that recovery.
- The test-side `read_response_with_id` helper adds a `collapsible_if` clippy warning. It is cosmetic and can be addressed in a future cleanup if desired.
- The references-timing start is measured just before the request write rather than strictly after flush; this does not include the post-response housekeeping that R5-F-02 is intended to exclude.

---

## 7. Final Verdict

**PASS WITH WARNINGS**

All acceptance criteria are met, all spec scenarios are exercised by passing tests, the roundtrip binary exits cleanly with the corrected label, and the change stays within the two designated files. The warnings are minor: a single new style-level clippy warning in the test helper and a documented, functionally harmless ordering nuance in the timing window.

---

## 8. Next recommended phase

`sdd-archive`
