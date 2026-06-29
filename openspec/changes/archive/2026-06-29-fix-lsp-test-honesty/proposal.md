# Proposal: `fix-lsp-test-honesty` — Replace substring matching with JSON-RPC frame parsing in LSP roundtrip test

## Intent

The LSP end-to-end gate `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` currently reports green by scanning raw captured stdout for byte substrings instead of parsing the JSON-RPC frames it claims to validate. The audit confirmed three dishonest assertions (R5-F-01, R5-F-02, R5-F-03) with failing reproductions in `tests/r5_test_honesty_repro.rs`. Fixing this is high leverage because the roundtrip test is the only automated gate that exercises the LSP server over a real transport.

## Scope

### In Scope
- Add a `wire_helpers` module exposing `parse_lsp_messages(bytes) -> Vec<LspMessage>` with a typed enum (`Request`, `Response`, `Notification`) that walks Content-Length-framed JSON-RPC bodies. This extends but does not replace the existing `read_response_with_id` helper at line 137.
- Rewrite the three substring matchers in `lsp_roundtrip_test.rs` to query parsed messages through `wire_helpers`: completion counts from the actual `textDocument/completion` response (R5-F-01), diagnostics flags from parsed `textDocument/publishDiagnostics` notifications (R5-F-03).
- Fix the timing assertion at lines 1659-1676 by capturing elapsed from the completion of the references write+flush until the parsed response is received, removing the inflated interval that currently includes the 50 ms sleep, shutdown roundtrip, and `child.wait()` (R5-F-02).

### Out of Scope
- Reading the `diagnostics.raw` payload as text; the fix only needs structural JSON-RPC classification.
- Performance benchmarks or server start-up timing assertions.
- Adding new LSP requests or protocol coverage beyond the existing roundtrip scenario.
- Changes to packaged XS mods under `mod/`.

## Capabilities

### New Capabilities
None. This is a test-infrastructure refactor with no user-facing behavior.

### Modified Capabilities
None. Existing `spec-game-folder-test-coverage.md` already enforces that "string matching SHALL NOT be the primary classifier" (line 61); this change applies the same principle to the roundtrip test but does not alter that spec's requirements.

## Approach

1. Introduce `wire_helpers` in `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` (or a small companion module if the binary structure allows) that reads Content-Length headers, splits stream bytes into JSON bodies, and deserializes each body into `LspMessage` with fields for `id`, `method`, and an untyped `Value` payload.
2. Keep `read_response_with_id` as-is for callers that only need a single response; add new helpers such as `find_response_by_id` and `find_notifications_with_method` to make assertions readable.
3. Update the completion check to search only the `textDocument/completion` response array for `aiEcho*`, the diagnostic checks to assert on `publishDiagnostics` notifications for the expected URIs, and the references timing to measure the request/response pair.
4. **Strict TDD is active for this change** (override of global `strict_tdd: false`). Each reproduction test must go RED before the implementation rewrite and GREEN after. Test runner: `cargo test --manifest-path tools/xs-language-server/Cargo.toml`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` | Modified | Substring checks replaced by parsed JSON-RPC queries; timing window narrowed. |
| `tools/xs-language-server/tests/r5_test_honesty_repro.rs` | None intended | Reproductions remain unchanged; they prove correctness of the rewrite. |
| `mod/*` | None | No impact on deployed XS mod packages. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Manual JSON-RPC frame parser has subtle boundary bugs | Low | Re-use the same Content-Length logic already present in `read_response_with_id`; exercise with reproduction tests. |
| Refactor changes assert sensitivity so existing failures hide | Very low | Reproduction tests go RED-then-GREEN before applying; full `cargo test` suite remains green. |

## Review Workload Forecast

- Estimated delta: `+180/-25` LOC in a single file.
- Decision needed before apply: No.
- Chained PRs recommended: No.
- 400-line budget risk: Low.

## Rollback Plan

`git revert <change-sha>` is fully safe. This is a pure test-code refactor with no data migration, protocol change, or schema change. Reverting restores the dishonest assertions but does not affect production behavior.

## Dependencies

None.

## Success Criteria

- [ ] R5-F-01: a `window/logMessage` notification containing the literal text `"aiEcho"` no longer inflates the completion counter when the completion response has zero entries.
- [ ] R5-F-02: the printed elapsed time for the references request reflects only the request/response interval, not the 50 ms sleep, shutdown, or process cleanup.
- [ ] R5-F-03: `has_clean_diag` and `has_bad_diag` are derived from parsed `textDocument/publishDiagnostics` notifications, not raw-byte substring checks.
- [ ] `cargo test --manifest-path tools/xs-language-server/Cargo.toml` passes.

## Open Questions

None anticipated.
