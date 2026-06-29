# Spec: LSP symbol-extractor module honesty

The LSP symbol-extractor module MUST NOT retain public methods or tests whose names no longer match their behavior. Dead helpers and misleading test labels MUST be removed in the same change that surfaces them.

## Rationale

Code review found `Param::render` in `tools/xs-language-server/src/symbols.rs` with zero callers and a body that discards its parameter via `let _ = source`, and the test `recovers_function_definition_from_error_node_with_body` whose name claims ERROR-recovery behavior but which actually exercises the regular `function_definition` path. Both are documentation bugs that hide real intent from future maintainers. A reviewer doing static analysis can be misled into "fixing" working code based on the misleading names. This spec formalizes that dead public items and misleading test labels must be removed when they are surfaced.

## Scenarios

### Scenario: dead public method is removed on discovery

- GIVEN a `pub fn` or `impl` block method whose name documents one purpose but whose body implements another (for example, it discards a parameter via `let _ = param;`, has zero callers, or contradicts its own doc-comments)
- WHEN a review or strict-TDD audit surfaces the deadness
- THEN the method MUST be deleted in the change that surfaces it
- AND any private helpers that exist solely to support the dead method MUST also be deleted or inlined
- AND the audit-grade reproduction tests that documented the lie MUST be inverted to strict-TDD RED→GREEN contracts

### Scenario: misleading test label is renamed or removed on discovery

- GIVEN a `#[test] fn` whose name documents a code path (for example, "recovers from error node") but whose actual exercise hits a different code path (for example, the regular function-definition rule)
- WHEN a review or strict-TDD audit surfaces the label-behavior mismatch
- THEN the test MUST be renamed to reflect what it actually exercises OR removed if the actual code path is already covered by other tests
- AND if the test is removed, the audit-grade reproduction that documented the lie MUST be inverted to a strict-TDD test that asserts the rename/absence

## Cross-references

- `openspec/specs/spec-lsp-server-lifecycle.md:116` — R3-F-01 "Single-Mutex Hold Across Await Points" is the same flavour of internal-hygiene invariant for the LSP server.
- `openspec/specs/spec-game-folder-test-coverage.md:61` — "string matching SHALL NOT be the primary classifier" is the broader test-honesty principle.

## Out of scope

- The forward-declaration grammar/parser bug (R1-F-02) in `tree-sitter-xs`; this needs a separate `fix-tree-sitter-xs-forward-declarations` change.
- Other `symbols.rs` findings: R1-F-08 (`trim_matches` strip), R1-F-09 (`init` comment), R1-F-10 (`_source` rename), R1-F-12 (`extract_modifiers` test gap).
- Any plugin-side findings.
- Any change to deployed XS mod packages.

## Verification approach

- `cargo test --manifest-path tools/xs-language-server/Cargo.toml` — full suite green.
- `cargo build --manifest-path tools/xs-language-server/Cargo.toml` — compiles cleanly.
- The audit-grade tests in `tools/xs-language-server/tests/symbols_cleanup_repro.rs` are inverted to strict-TDD contracts during `sdd-apply` and prove the post-fix state.

## Acceptance criteria

1. `Param::render` no longer exists in `tools/xs-language-server/src/symbols.rs`.
2. The `is_false` helper is deleted or inlined; `Param::is_ref`'s `#[serde(skip_serializing_if = ...)]` annotation compiles to equivalent behavior.
3. The misleading test `recovers_function_definition_from_error_node_with_body` is renamed to reflect actual behavior OR removed (with the regular-path coverage already existing in the test file).
