# LSP Roundtrip Test Honesty Specification

> **Added/updated by change:** `fix-lsp-test-honesty`

## Capability summary

The LSP end-to-end roundtrip test SHALL exercise server response assertions by parsing JSON-RPC frames instead of scanning the byte stream with substring matches; timing assertions SHALL measure only the request/response interval.
This capability applies the principle established in `openspec/specs/spec-game-folder-test-coverage.md:61` that **string matching SHALL NOT be the primary classifier**.

## Rationale

`tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` is the only gate that exercises the LSP server over a real transport.
Its assertions currently pass by scanning raw captured stdout for byte substrings, so a completion entry and a `window/logMessage` payload that happens to contain the same bytes are indistinguishable.
The references-request timing assertion is similarly dishonest: it captures `Instant::now()` before the write, then reads `start.elapsed()` after the embedded 50 ms sleep, shutdown/exit roundtrip, and `child.wait()` cleanup have all completed.
These three bugs (R5-F-01, R5-F-02, R5-F-03) accumulated into the only automated gate, allowing it to report green while masking real regressions.

## Scenarios

### Scenario: completion assertion derives from parsed completion response

- GIVEN a `textDocument/completion` response with one aiEcho-labeled entry at id=N
- AND a separate `window/logMessage` notification whose `params.message` is the literal string `"aiEcho"` appearing anywhere later in the captured byte stream
- WHEN the roundtrip test asserts that the completion response included aiEcho family items
- THEN the assertion SHALL count exactly the entries in the `textDocument/completion` response's `result` array (1 in this case)
- AND SHALL NOT count the `"aiEcho"` substring in the log-message payload as a completion entry

### Scenario: diagnostics flags derived from parsed publishDiagnostics notifications

- GIVEN a `textDocument/publishDiagnostics` notification whose `params.uri` is `"file:///tmp/test.xs"` and `params.diagnostics` is empty
- AND another `textDocument/publishDiagnostics` for `"file:///tmp/bad.xs"` with non-empty diagnostics
- WHEN the roundtrip test asserts diagnostics were published
- THEN `has_clean_diag` SHALL be true iff a parsed notification with the expected URI AND empty diagnostics exists
- AND `has_bad_diag` SHALL be true iff a parsed notification with the expected URI AND non-empty diagnostics exists
- AND SHALL NOT be satisfied by raw-byte substring matches against unrelated frames

### Scenario: elapsed time measures only the references request/response interval

- GIVEN the LSP server is spawned and the references request is written and flushed at time T0
- WHEN the roundtrip test measures elapsed time
- THEN elapsed SHALL be computed as `t_response_received - T0` (post-write, pre-cleanup)
- AND SHALL NOT include the post-write 50 ms sleep, the shutdown/exit roundtrip, or the `child.wait()` OS cleanup
- AND the printed label SHALL be `"responded in X ms"` or similar accurate wording — NOT `"returned in X ms"` if the measurement includes shutdown/cleanup

## XS-engine / cross-cutting constraints

- The classifier rule in `spec-game-folder-test-coverage.md:61` (`string matching SHALL NOT be the primary classifier`) applies to every LSP test gate, including the roundtrip binary.
- The `wire_helpers` parser is a pure test-side refactor; it SHALL NOT require changes to LSP server request handling, notification payloads, or diagnostic serialization.

## Out of scope

- Reading the `diagnostics.raw` payload as text (structural JSON-RPC classification only).
- Performance benchmarks and server start-up timing assertions.
- New LSP requests or protocol coverage.
- Changes to deployed XS mod packages under `mod/`.

## Verification approach

- Audit reproduction tests live in `tools/xs-language-server/tests/r5_test_honesty_repro.rs`
  and prove each lie structurally.
- Additional strict-TDD `RED→GREEN` tests are added during `sdd-apply`
  and target the production `bin/lsp_roundtrip_test.rs` directly.
- Local verification command: `cargo test --manifest-path tools/xs-language-server/Cargo.toml` passes.
- The reproduction tests in `r5_test_honesty_repro.rs` produce the documented RED state on the unfixed binary; the strict-TDD tests added by `sdd-apply` must FAIL on the unfixed binary and PASS on the fixed binary.

## Acceptance criteria

1. The rewrite of `bin/lsp_roundtrip_test.rs` introduces a `wire_helpers` module that exposes `parse_lsp_messages(bytes) -> Vec<LspMessage>` and uses Content-Length-framed JSON-RPC parsing.
2. The three substring matchers at lines 401-411 and 362-380 are replaced with parsed queries; the timing harness at lines 1659-1676 captures elapsed from post-write to response-received, excluding post-response housekeeping.
3. All audit reproduction tests in `tests/r5_test_honesty_repro.rs` are preserved and produce the documented RED→GREEN state on the rewritten binary.
4. `cargo test --manifest-path tools/xs-language-server/Cargo.toml` passes — including the new strict-TDD tests added during `sdd-apply`.
