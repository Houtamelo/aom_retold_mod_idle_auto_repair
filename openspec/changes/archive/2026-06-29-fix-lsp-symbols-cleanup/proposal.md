# Proposal: `fix-lsp-symbols-cleanup` — Remove dead `Param::render` and misleading test in `symbols.rs`

## Intent

Remove the unused `Param::render` method and the misleadingly-named `recovers_function_definition_from_error_node_with_body` test in `tools/xs-language-server/src/symbols.rs`. Both were flagged during an audit of R1 findings: `Param::render` has no callers and a contradictory interface, while the test name claims it exercises ERROR-recovery but actually exercises the regular `function_definition` path. This change is pure internal cleanup with no behavior change. R1-F-02 (forward-declaration grammar) was refuted in this audit and is explicitly out of scope; it needs its own separate change against `tree-sitter-xs` grammar/parser.

## Scope

### In Scope
- Delete `Param::render` (`tools/xs-language-server/src/symbols.rs:84-94`).
- Delete the `impl Param` block that becomes empty.
- Delete the `is_false` helper (`symbols.rs:80-82`) that only supports `Param::is_ref`'s `skip_serializing_if` attribute, replacing it with an inline equivalent.
- Update the `serde(default, skip_serializing_if = ...)` annotation on `is_ref` so serialization remains unchanged after `is_false` is removed.
- Rename `recovers_function_definition_from_error_node_with_body` to reflect that it tests the regular `function_definition` extraction path, or remove it if equivalent coverage already exists.
- Update audit-grade reproduction tests in `tools/xs-language-server/tests/symbols_cleanup_repro.rs` to assert the post-fix state.

### Out of Scope
- The R1-F-02 forward-declaration grammar/parser bug in `tree-sitter-xs` (forward declarations produce `ERROR` nodes). This requires a separate `fix-tree-sitter-xs-forward-declarations` change.
- R1-F-08 (`trim_matches` strip), R1-F-09 (init comment), R1-F-10 (`_source` rename), R1-F-12 (`extract_modifiers` test gap), and any other findings in `symbols.rs`.

## Capabilities

### New Capabilities
None. This is a pure internal cleanup with no new user-facing behavior.

### Modified Capabilities
None. No spec-level behavior changes; this change removes dead code and fixes a misleading test name.

## Approach

1. Strict TDD: keep the six existing audit-grade tests in `tests/symbols_cleanup_repro.rs`, then add two new strict-TDD RED→GREEN tests that assert the post-fix state.
2. For the `Param::render` removal, replace the audit test with a compile-time or runtime contract verifying `Param` no longer carries the `render` method.
3. For the misleading test, keep the useful assertion body and rename it to match the regular extraction path it exercises.
4. Replace `skip_serializing_if = "is_false"` with an inline closure or switch `is_ref` to `Option<bool>` so the helper can be deleted without changing serialization output.
5. Test runner: `cargo test --manifest-path tools/xs-language-server/Cargo.toml`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `tools/xs-language-server/src/symbols.rs` | Modified | Delete `Param::render`, the `impl Param` block, the `is_false` helper, and replace `skip_serializing_if` with an inline equivalent. Rename the misleading test. |
| `tools/xs-language-server/tests/symbols_cleanup_repro.rs` | Modified | Update existing audit tests and add strict-TDD assertions for the post-fix state. |
| `tools/xs-language-server/Cargo.lock` | None expected | No dependency changes. |
| `mod/*` | None | No impact on deployed XS mod packages. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| `Param::render` has a hidden caller we missed during the audit | Very low | `cargo test` and `cargo build` will catch any breakage; production binary does not link against `tests/`. |
| Removing `is_false` breaks `is_ref` serialization | Low | Replace it with an inline `skip_serializing_if` closure or an `Option<bool>` form that preserves the exact same JSON shape. |

## Review Workload Forecast

- Estimated delta: `-25 / +5` LOC in a single file (`symbols.rs` only).
- Decision needed before apply: No.
- Chained PRs recommended: No.
- 400-line budget risk: Low.

## Rollback Plan

`git revert <change-sha>` is fully safe. This change is a pure deletion of dead code plus a test rename, with no data migration, protocol change, or schema change.

## Dependencies

None.

## Success Criteria

- [ ] `cargo test --manifest-path tools/xs-language-server/Cargo.toml` passes.
- [ ] `cargo build --manifest-path tools/xs-language-server/Cargo.toml` compiles cleanly (no new clippy warnings).
- [ ] `Param::render` no longer exists in `tools/xs-language-server/src/symbols.rs`.
- [ ] The misleading test has been renamed to reflect its actual behavior (or removed if equivalent coverage already exists).
- [ ] All audit-grade reproduction tests in `symbols_cleanup_repro.rs` are updated to assert the new (correct) state.

## Open Questions

None anticipated.
