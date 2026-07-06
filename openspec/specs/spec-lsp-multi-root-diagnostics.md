# LSP Multi-Root Diagnostics Specification

<!-- delta-spec for archived change 2026-07-05-lsp-include-graph-aware-diagnostics -->

> **Added/updated by change:** `lsp-include-graph-aware-diagnostics`
> **Archived:** 2026-07-05
> **Change verdict:** PASS

## Purpose

When an `.xs` file is opened in the LSP, the server SHALL discover every workspace root that includes that file, run diagnostics once per root chain, and aggregate equivalent results into one LSP `Diagnostic` per distinct issue. This aligns edit-time diagnostics with the AoM:R engine's link-time include-paste semantics and removes the false-positive `Error 0310` that occurs when a callee is defined in a sibling file included by the same root.

## Requirements

### R1 — Root classification

The server SHALL classify an opened file `F` as either an orphan (no includers) or as reachable from one or more roots, using the reverse-include graph.

- GIVEN file `F` has zero includers in the workspace, WHEN `textDocument/didOpen` arrives for `F`, THEN the server SHALL treat `F` as its own root and run exactly one diagnostic pass against `F`.
- GIVEN file `F` has one or more includers, WHEN `textDocument/didOpen` arrives for `F`, THEN the server SHALL treat each includer that has no dependents as a root for `F`.
- GIVEN file `F` is part of an include cycle, WHEN roots are computed, THEN the server SHALL fall back to treating `F` as its own root.

### R2 — Multi-pass analysis

For each root that includes `F`, the server SHALL build the root's merged view and run a full diagnostic pass.

- GIVEN roots `R1..Rn` include `F`, WHEN diagnostics are published for `F`, THEN the server SHALL run one pass per root and collect per-root diagnostics.
- GIVEN the same diagnostic issue is produced from multiple roots, WHEN results are aggregated, THEN the server SHALL emit exactly one `Diagnostic` grouped by `(uri, range, category, base message)`.
- GIVEN two roots produce the same issue at different ranges, WHEN results are aggregated, THEN the server SHALL emit two distinct diagnostics because range is part of the equality key.
- GIVEN two roots produce diagnostics for different identifiers at the same range, WHEN results are aggregated, THEN the server SHALL emit two distinct diagnostics because message is part of the equality key.

### R3 — Aggregation format

Aggregated diagnostic messages SHALL indicate root provenance only when the issue is not universal across all roots.

- GIVEN diagnostic `D` appears in all root closures of `F` and there are at least two roots, WHEN `D` is published, THEN the message SHALL NOT contain `as seen from` or any root name.
- GIVEN diagnostic `D` appears in only some root closures and `2 ≤ total_roots ≤ 100`, WHEN `D` is published, THEN the message SHALL list the producing root names after `as seen from:`.
- GIVEN diagnostic `D` appears in only some root closures and `total_roots > 100`, WHEN `D` is published, THEN the message SHALL list at most the first three producing root names followed by `...and N more`.

### R4 — Producing-roots ordering

When root names are listed, they SHALL be ordered as currently-open buffers first (in open order), then mod-overlay roots alphabetically, then all remaining roots alphabetically.

- GIVEN producing roots include an open buffer, a mod-overlay root, and a game-folder root, WHEN the suffix is formatted, THEN open buffers appear first, mod-overlay roots second, and alphabetical remainder last.

### R5 — Lazy local-pass short-circuit

The server SHALL run a single-file pass with `F` as root first and skip the multi-root loop when no cross-file symbol issues remain.

- GIVEN the local pass for `F` produces no unresolved or use-before-declaration diagnostics, WHEN `F` is diagnosed, THEN the server SHALL publish the local-pass diagnostics directly without building per-root views.
- GIVEN the local pass for `F` reports an unresolved symbol `X` and `X` is reachable from at least one root chain, WHEN the multi-root pass runs, THEN the server SHALL suppress the local diagnostic for `X` and emit at most one aggregated diagnostic per distinct issue.

### R6 — Forward declarations across the include chain

Symbols reachable later in a root chain SHALL NOT require a forward declaration in the file being diagnosed.

- GIVEN `A.xs` is reachable from root `R` as `R → A → ... → B`, AND `A.xs` calls a function defined in `B.xs` without forward-declaring it, WHEN diagnostics run for `A.xs`, THEN no `undefined symbol` or `used before declaration` diagnostic SHALL be emitted for that call.

### R7 — Workspace-wide fallback rescoping

The typecheck workspace-callee fallback SHALL be limited to symbols reachable from a root that includes the current file.

- GIVEN `resolve_workspace_function` cannot find a callee in the file's merged view, WHEN it performs fallback, THEN it SHALL search symbols in the root chain only and SHALL NOT scan the whole `VirtualProject`.

### R8 — Test coverage

The anchor RED test and new root-aware diagnostics test SHALL pass.

- GIVEN `tools/xs-language-server/lsp/tests/indirect_include_symbol_repro.rs::test_indirect_include_does_not_emit_unresolved_when_used_as_callee`, WHEN the full change is applied, THEN the test SHALL pass.
- GIVEN `tools/xs-language-server/lsp/tests/root_aware_diagnostics_repro.rs`, WHEN it runs, THEN it SHALL cover orphan files, single-root files, multi-root partial/universal/truncated coverage, dead-code orphan behavior, forward-declaration across the chain, and local-pass short-circuit.

## Scenarios

### S1 — Orphan file treated as its own root

- GIVEN file `O.xs` has zero includers
- WHEN `textDocument/didOpen` arrives for `O.xs`
- THEN the server SHALL treat `O.xs` as its own root
- AND SHALL run exactly one diagnostic pass against `O.xs`
- AND SHALL NOT consult any other root for `O.xs`

### S2 — Single-root file emits root provenance

- GIVEN file `F.xs` has exactly one root `R.xs`
- WHEN diagnostics are published for `F.xs`
- THEN the server SHALL run diagnostics against `R.xs`'s include chain
- AND the message for non-universal diagnostics SHALL mention `as seen from: R` (or omit the suffix when `R` is the only root and the diagnostic is universal)

### S3 — Multi-root universal coverage omits tree names

- GIVEN file `F.xs` has roots `R1`, `R2`, and `R3`
- AND diagnostic `D` appears in all three root closures
- WHEN diagnostics are published
- THEN `D` SHALL be emitted exactly once
- AND the message SHALL NOT contain `as seen from`

### S4 — Multi-root partial coverage lists producing trees

- GIVEN file `F.xs` has `5` roots
- AND diagnostic `D` appears in only `R2` and `R4`
- WHEN diagnostics are published
- THEN `D` SHALL be emitted exactly once
- AND the message SHALL contain `as seen from: R2, R4`

### S5 — Multi-root truncated coverage for more than 100 includers

- GIVEN file `F.xs` has `110` roots
- AND diagnostic `D` appears in `20` roots
- WHEN diagnostics are published
- THEN `D` SHALL be emitted exactly once
- AND the message SHALL list the first three producing root names followed by `...and 17 more`

### S6 — Local short-circuit skips multi-root work

- GIVEN file `F.xs` has multiple roots
- AND the local single-file pass produces no unresolved or use-before-declaration diagnostics
- WHEN diagnostics are published
- THEN the server SHALL skip the per-root loop
- AND SHALL emit the local-pass diagnostics directly

### S7 — Cross-chain call without forward declaration

- GIVEN root chain `R → A → B`
- AND `A.xs` calls `helper()` defined in `B.xs`
- AND `A.xs` does not forward-declare `helper`
- WHEN diagnostics run for `A.xs`
- THEN no `undefined symbol` diagnostic SHALL be emitted for the call to `helper`

### S8 — Workspace callee fallback limited to root chain

- GIVEN `F.xs` calls `remoteFn()` not defined in `F.xs`'s merged view
- AND `remoteFn` is defined only in a file reachable from one root that includes `F.xs`
- AND `remoteFn` is also defined in an unrelated file not reachable from any root of `F.xs`
- WHEN `resolve_workspace_function` runs
- THEN it SHALL find `remoteFn` in the reachable root chain
- AND SHALL NOT treat the unrelated-file definition as a valid fallback

## Out of scope

- Incremental invalidation beyond eager watched-file notifications.
- Symbol pre-resolution map or `@root` annotations.
- LSP client UI changes (code actions, settings page).
- Proving equivalence with every AoM:R include-link edge case.

## Verification approach

- Rust integration tests in `tools/xs-language-server/lsp/tests/root_aware_diagnostics_repro.rs` and `indirect_include_symbol_repro.rs`.
- Full workspace test: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --workspace`.
- Manual smoke test in IntelliJ/Rider against a multi-root mod file to confirm aggregated suffix format.

## Acceptance criteria

1. The server SHALL treat an orphan file as its own root.
2. The server SHALL run one diagnostic pass per root that includes the opened file.
3. The server SHALL aggregate equivalent diagnostics into one `Diagnostic` per distinct `(uri, range, category, base message)`.
4. Universal diagnostics (produced by all roots) SHALL omit root names from the message.
5. Partial diagnostics (`2..=100` roots) SHALL list every producing root name.
6. Truncated diagnostics (`>100` roots) SHALL list the first three producing names followed by `...and N more`.
7. Producing-root names SHALL be ordered as currently-open → mod-overlay → alphabetical remainder.
8. The server SHALL short-circuit the multi-root loop when the local pass has no cross-file symbol issues.
9. Cross-chain calls without forward declarations SHALL NOT produce use-before-definition errors.
10. Workspace callee fallback SHALL search the root chain only, not the full `VirtualProject`.
11. The anchor RED test `indirect_include_symbol_repro::test_indirect_include_does_not_emit_unresolved_when_used_as_callee` SHALL pass.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `lsp-include-graph-aware-diagnostics` | 2026-07-05 | PASS | New spec consolidating root classification, multi-pass analysis, aggregation format, lazy short-circuit, forward-declaration-across-chain, and workspace-fallback rescoping. |
