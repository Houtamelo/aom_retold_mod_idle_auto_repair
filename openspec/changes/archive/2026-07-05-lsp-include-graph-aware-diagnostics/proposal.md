# Proposal: LSP Include-Graph-Aware Diagnostics

## Intent

AoM:R modders write new `.xs` files before wiring them into an include chain. Today the LSP diagnoses an open file using only its own forward include closure, so a callee defined in a sibling included by the same root is falsely reported as unresolved (`Error 0310`). We will make diagnostics root-aware: discover every root that includes the open file, run diagnostics against each root chain, and aggregate the results. This fixes the RED repro and aligns edit-time diagnostics with engine link-time semantics.

## Scope

### In Scope (V1)
- Orphan = root heuristic: a file with no includers is analyzed standalone.
- Project-wide reverse-include graph to discover `roots_that_include(file)`.
- Multi-pass diagnostic pipeline: one pass per root, aggregated into one message per distinct issue.
- Aggregation format with universal coverage (no tree names) and truncation (>100 trees → first 3 + "...and N more").
- Lazy local-pass short-circuit: run single-file pass first; skip multi-root work if no cross-file symbol issues remain.
- Per-root merged-view cache keyed by `(root_path, content_hash_of_closure)`.
- Update `semantic.rs` helpers (`forward_callable_merged`, `check_forward_declarations_for_merged_view`, etc.) to use the root chain, not the full `VirtualProject`.
- Update `typecheck.rs::resolve_workspace_function` to fall back to the root chain instead of the whole project.
- New `*_repro.rs` tests: orphan file, single-root, multi-root partial coverage, multi-root universal, multi-root truncated, forward-declaration across chain, dead-code orphan.
- Existing RED test `indirect_include_symbol_repro.rs::test_indirect_include_does_not_emit_unresolved_when_used_as_callee` turns GREEN.

### Out of Scope (V2 / Deferred)
- Incremental cache invalidation (Approach 4).
- Symbol pre-resolution map (Approach 5).
- `// @root` annotation or curated root list.
- LSP UI changes (code actions, settings page, etc.).

## Capabilities

### New Capabilities
- `lsp-include-graph-aware-diagnostics`: reverse-include root discovery, per-root diagnostic runs, universal/partial/truncated aggregation, and performance mitigations.

### Modified Capabilities
- `semantic-diagnostics`: change cross-file symbol resolution from the file's merged view / whole project to the set of root chains that include the file.

## Approach

Layer 1 (data): Build a project-wide reverse-include graph on first need. For file `F`, compute `roots = roots_that_include(F)`; empty means orphan, so use `F` as its own root.

Layer 2 (analysis loop): For each root `R` in `roots`, get or build `MergedView(R)`. Run `diagnostics::collect_all(F, merged_view)` once per root. Aggregate equivalent diagnostics by `(range, category, base message)`. Emit one LSP `Diagnostic` per distinct issue: universal messages omit tree names; partial/truncated messages append `\n  as seen from: root1, root2, root3` and "...and N more" when >100 roots. Publish via `client.publish_diagnostics`.

Layer 3 (short-circuit): Always run the local pass first. If no unresolved/use-before-declaration diagnostics remain, skip the multi-root loop.

Layer 4 (cache): `MergedViewCache` keyed by `(root_path, content_hash_of_closure)`. Reuse across files reachable from the same root and across repeated opens.

## Locked Design Confirmations

1. **Orphan = root**: Files with no includers are treated as standalone roots. This matches the modder workflow (writing a file before wiring it) and is preferable to silently hiding diagnostics on new code. Dead-code over-classification is accepted for V1.
2. **Multi-root analysis**: If `F` is included by `R1..Rn`, diagnostics run once per root. A symbol reachable from any root must not be reported unresolved.
3. **Aggregation format**: One diagnostic per distinct issue. Universal coverage omits tree names. Partial/truncated coverage lists up to 3 root names plus "...and N more" when more than 100 roots exist.
4. **Forward declarations across the full chain**: `forward_callable_merged` and related helpers must evaluate the entire root chain (all files reachable from `R`), not just `F`'s forward closure.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `lsp/src/server.rs` | Modified | Root discovery, per-root analysis loop, diagnostic aggregation, cache plumbing. |
| `lsp/src/diagnostics.rs` | Modified | `collect_all` becomes multi-root aware; adds aggregation key and message formatting. |
| `lsp/src/merged_view.rs` | Modified | Build merged view from arbitrary root; expose reverse-include graph. |
| `lsp/src/semantic.rs` | Modified | `forward_callable_merged`, `check_forward_declarations_for_merged_view`, `check_extern_collisions` use root chain. |
| `lsp/src/typecheck.rs` | Modified | `resolve_workspace_function` falls back to root chain, not whole `VirtualProject`. |
| `lsp/tests/*repro.rs` | New | TDD repro tests for orphan, single-root, multi-root, forward-declaration, and dead-code cases. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Performance cliff on high-includer files (147 roots) | High if unmitigated | Lazy short-circuit + per-root cache ship in V1. |
| Duplicate or dropped aggregated messages | Medium | Stable key `(range, category, base message)`; new tests cover universal, partial, and truncated cases. |
| Inconsistent cross-file fallback in `typecheck.rs` | High if not updated | Explicitly re-scope fallback to root chain in V1. |
| Mod-overlay / binary `.xs` edge cases | Low | Reuse existing `IncludeDiagnostic::Unreadable` and `tracing::warn!` paths; add repro coverage. |

## Rollback Plan

Revert the implementation commit. The diagnostic pipeline returns to single-file merged-view behavior; the RED repro reappears but no other functionality is broken.

## Dependencies

- Existing `tools/xs-language-server` workspace only.

## Success Criteria

- All existing tests still pass (no regression of the 336-test baseline).
- `test_indirect_include_does_not_emit_unresolved_when_used_as_callee` turns GREEN.
- New repro tests added and passing: orphan file, single-root, multi-root partial coverage, multi-root universal (>100 roots), multi-root truncated (>100 roots), forward-declaration across chain, dead-code orphan.
- `didOpen` latency stays <100 ms for files reachable from <5 roots; 147-root worst case stays <500 ms with caching and lazy short-circuit.

## Out-of-Scope Guardrails

V1 will NOT implement incremental invalidation, a symbol pre-resolution map, `@root` annotations, a curated root list, or any LSP client UI changes. These remain documented for V2.
