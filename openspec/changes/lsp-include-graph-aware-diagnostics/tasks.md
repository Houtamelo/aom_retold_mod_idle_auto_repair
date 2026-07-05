# Tasks: lsp-include-graph-aware-diagnostics

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | 1,300 – 1,600 |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR-1 → PR-2 → PR-3 → PR-4 → PR-5 → PR-6 |
| Delivery strategy | auto-forecast (C4 — chain) |
| Chain strategy | feature-branch-chain with tracker `lsp-include-graph-aware-diagnostics` |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: feature-branch-chain
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | PR | Branch | Base | Tests |
|------|------|----|--------|------|-------|
| 1 | Reverse-include graph + query API | PR-1 | `lsp-include-graph-pr-1` | `lsp-include-graph-aware-diagnostics` | `include_graph_repro.rs` |
| 2 | Per-root merged-view cache | PR-2 | `lsp-include-graph-pr-2` | PR-1 | `per_root_cache_repro.rs` |
| 3 | Root-based merged view + rebase | PR-3 | `lsp-include-graph-pr-3` | PR-2 | `merged_view_root_repro.rs` |
| 4 | Diagnostic context + root-scoped semantic/typecheck | PR-4 | `lsp-include-graph-pr-4` | PR-3 | `indirect_include_symbol_repro.rs` anchor |
| 5 | Aggregation + publish_diagnostics orchestration | PR-5 | `lsp-include-graph-pr-5` | PR-4 | `root_aware_diagnostics_repro.rs` |
| 6 | Edge-case coverage (dead code, forward-decl across chain, overlays) | PR-6 | `lsp-include-graph-pr-6` | PR-5 | extended `root_aware_diagnostics_repro.rs` |

---

## PR-1: Reverse-include graph (data structure + repro)

**Goal**: Stand up the project-wide reverse-include graph and the `roots_that_include(file)` query API.

**Branch**: `lsp-include-graph-pr-1`

**Status**: ✅ Complete (commit `9ef9378`)

**Files**:
- NEW: `tools/xs-language-server/lsp/src/include_graph.rs` — `ReverseIncludeGraph` struct; `build(forward: &IncludeGraph)`; `dependents` map; `roots_that_include(file) -> Vec<PathBuf>`; `direct_includers(file)`; `len()`; cycle handling via visited guard.
- MOD: `tools/xs-language-server/lsp/src/lib.rs` — add `pub mod include_graph;`.
- MOD: `tools/xs-language-server/lsp/Cargo.toml` + `Cargo.lock` — add `smallvec`.
- NEW: `tools/xs-language-server/lsp/tests/include_graph_repro.rs` — TDD repro tests.

**Tests (RED before, GREEN after)**:
- [x] `test_roots_that_include_empty_for_orphan_file`
- [x] `test_roots_that_include_finds_single_includer`
- [x] `test_roots_that_include_finds_transitive_chain`
- [x] `test_roots_that_include_handles_diamond`
- [x] `test_orphan_file_is_own_root_via_empty_return`
- [x] `test_direct_includers_returns_only_first_hop`
- [x] `test_len_counts_distinct_files`

**Dependency**: none (greenfield module)

**Estimated diff**: ~220 lines

**Rollback**: delete `include_graph.rs` and `tests/include_graph_repro.rs`; remove the module declaration from `lib.rs`.

**Cross-cutting concerns**:
- **Risk**: Cycles in the include graph could cause infinite BFS or incorrectly discard roots; missing a mod-overlay include edge would hide valid roots.
- **Verification command**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test include_graph_repro`
- **Documentation update**: none
- **Memory observation**: Graph rebuild must be triggered when workspace folders or watched files change; record the exact trigger points.

---

## PR-2: Per-root merged-view cache

**Goal**: Add a `PerRootMergedViewCache` keyed by `(root_path, closure_content_hash)` with up-front invalidation-by-path hooks.

**Branch**: `lsp-include-graph-pr-2` (base: PR-1)

**Files**:
- MOD: `tools/xs-language-server/lsp/src/cache.rs` — add `PerRootCacheKey`, `CachedRootView`, `PerRootMergedViewCache::new`, `get_or_build`, `invalidate_for_paths`, `closure_content_hash` helper.
- MOD: `tools/xs-language-server/lsp/src/merged_view.rs` — expose `closure_files()` iterator so the cache can compute closure hashes and invalidation sets (small helper, ~20 lines).
- NEW: `tools/xs-language-server/lsp/tests/per_root_cache_repro.rs` — TDD repro tests.

**Tests (RED before, GREEN after)**:
- `test_cache_returns_same_arc_on_identical_closure_hash`
- `test_cache_rebuilds_after_closure_change`
- `test_cache_invalidates_entry_containing_changed_path`
- `test_unchanged_root_view_reused_while_changed_root_rebuilds`
- `test_cache_stores_closure_file_list`

**Dependency**: PR-1 (graph module gives the root concept, but cache mechanics are independent; ordered second by design)

**Estimated diff**: ~200 lines

**Rollback**: revert all additions in `cache.rs`, remove `closure_files()` from `merged_view.rs`, delete `tests/per_root_cache_repro.rs`.

**Cross-cutting concerns**:
- **Risk**: Closure hash collisions or stale closure-file lists could return an out-of-date view after an edit.
- **Verification command**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test per_root_cache_repro`
- **Documentation update**: none
- **Memory observation**: The cache is intentionally trivial at first; invalidation hooks must be present even if the cache only holds a handful of root views.

---

## PR-3: Build merged view from arbitrary root + file rebase

**Goal**: Allow the server to build a `MergedView` from any root and rebase it to the currently diagnosed file.

**Branch**: `lsp-include-graph-pr-3` (base: PR-2)

**Files**:
- MOD: `tools/xs-language-server/lsp/src/merged_view.rs` — add `MergedView::build_from_root(root, workspace, project, cache_dir)`; `MergedView::rebase_to_file(&self, file, source, own_table) -> MergedView`; strengthen `closure_files()` return lifetime if needed.
- NEW: `tools/xs-language-server/lsp/tests/merged_view_root_repro.rs` — TDD repro tests.

**Tests (RED before, GREEN after)**:
- `test_build_from_root_loads_root_chain`
- `test_rebase_to_file_preserves_root_chain`
- `test_rebased_view_exposes_sibling_symbol`
- `test_rebased_view_keeps_own_table_for_current_file`

**Dependency**: PR-1, PR-2

**Estimated diff**: ~170 lines

**Rollback**: revert additions in `merged_view.rs`; delete `tests/merged_view_root_repro.rs`.

**Cross-cutting concerns**:
- **Risk**: `rebase_to_file` could accidentally drop include diagnostics or own-table symbols from the original file; lifetime changes could break existing callers.
- **Verification command**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test merged_view_root_repro`
- **Documentation update**: none
- **Memory observation**: `build_from_root` should reuse the per-root cache from PR-2 immediately, so the two pieces must share the `PerRootMergedViewCache` handle.

---

## PR-4: DiagnosticContext + root-chain semantic/typecheck

**Goal**: Refactor the diagnostic pass to accept both a file view and a root view, scope semantic/typecheck fallback to the root chain, and turn the existing anchor RED test GREEN.

**Branch**: `lsp-include-graph-pr-4` (base: PR-3)

**Files**:
- MOD: `tools/xs-language-server/lsp/src/diagnostics.rs` — introduce `DiagnosticContext`; refactor `collect_all` to take `&DiagnosticContext`; add `run_pass(ctx)` wrapper; keep current single-root behavior as the default path.
- MOD: `tools/xs-language-server/lsp/src/semantic.rs` — update `forward_callable_merged`, `check_forward_declarations_for_merged_view`, `check_mutable_redefinitions_for_merged_view`, `check_extern_collisions` to accept `root_view` and scope lookups to the root chain.
- MOD: `tools/xs-language-server/lsp/src/typecheck.rs` — update `resolve_workspace_function` fallback to `root_view.find(name)`; thread `root_view` through `check_calls_with_merged`.
- MOD: `tools/xs-language-server/lsp/src/server.rs` — update call sites to build a `DiagnosticContext` and pass `root_view`.
- MOD: `tools/xs-language-server/lsp/tests/indirect_include_symbol_repro.rs` — ensure the existing fixture exercises root-chain resolution.

**Tests (RED before, GREEN after)**:
- `indirect_include_symbol_repro::test_indirect_include_does_not_emit_unresolved_when_used_as_callee` (anchor)
- `test_forward_decl_across_chain_no_error`

**Dependency**: PR-1, PR-2, PR-3

**Estimated diff**: ~320 lines

**Rollback**: revert signature changes in `diagnostics.rs`, `semantic.rs`, `typecheck.rs`, and `server.rs`; restore the original anchor test fixture if it was modified.

**Cross-cutting concerns**:
- **Risk**: Signature churn can break existing `collect_all` callers or unit tests; the 336-test baseline must stay green.
- **Verification command**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test indirect_include_symbol_repro`
- **Documentation update**: none
- **Memory observation**: This PR is the TDD anchor for the whole change; the RED test must be failing for the expected reason before implementation starts.

---

## PR-5: Aggregation utility + publish_diagnostics orchestration

**Goal**: Wire the server to run a local pass, optionally run per-root passes, aggregate equivalent diagnostics, and publish universal/partial/truncated messages.

**Branch**: `lsp-include-graph-pr-5` (base: PR-4)

**Files**:
- MOD: `tools/xs-language-server/lsp/src/diagnostics.rs` — add `DiagnosticCategory`/`categorize`, `PerRootDiagnostic`, `AggregatedDiagnostic`, `DiagnosticKey`, `aggregate_results`, `format_root_suffix`, `strip_root_suffix`.
- MOD: `tools/xs-language-server/lsp/src/server.rs` — add `reverse_include_graph` and `per_root_merged_views` fields; implement `get_or_build_reverse_graph`, `get_or_build_root_merged_view`; rewrite `publish_diagnostics` with local short-circuit, per-root loop, and aggregation; extend `invalidate_merged_views_for` to also drop affected per-root cache entries.
- NEW: `tools/xs-language-server/lsp/tests/root_aware_diagnostics_repro.rs` — TDD repro tests covering orphan, single-root, multi-root partial/universal/truncated, and local short-circuit.

**Tests (RED before, GREEN after)**:
- `test_orphan_file_gets_full_diagnostics`
- `test_single_root_reachable_file_emits_as_seen_from`
- `test_multi_root_partial_coverage_lists_trees`
- `test_multi_root_universal_coverage_omits_tree_names`
- `test_multi_root_truncated_coverage_shows_first_three_and_more`
- `test_local_short_circuit_skips_multi_root_when_no_cross_file_issues`

**Dependency**: PR-4

**Estimated diff**: ~360 lines

**Rollback**: revert `diagnostics.rs` aggregation additions; revert `server.rs` orchestration changes to single-pass `publish_diagnostics`; delete `tests/root_aware_diagnostics_repro.rs`.

**Cross-cutting concerns**:
- **Risk**: Aggregation key instability can duplicate or drop messages; the 147-root worst-case must stay under budget because of the per-root cache and lazy short-circuit.
- **Verification command**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test root_aware_diagnostics_repro`
- **Documentation update**: If a new diagnostic-provenance suffix format is user-visible, add a one-line note in AGENTS.md under the LSP develop & verify loop.
- **Memory observation**: The universal/partial/truncated suffix format is locked; any deviation must be re-approved because it surfaces in editor UI.

---

## PR-6: Remaining spec coverage and edge cases

**Goal**: Close the spec coverage gap for dead-code orphans, forward-declaration-across-chain, mod-overlay paths, binary `.xs` exclusions, and include cycles.

**Branch**: `lsp-include-graph-pr-6` (base: PR-5)

**Files**:
- MOD: `tools/xs-language-server/lsp/tests/root_aware_diagnostics_repro.rs` — extend with edge-case fixtures.
- MOD: `tools/xs-language-server/lsp/src/diagnostics.rs` / `semantic.rs` — any small fixes discovered by edge-case tests (expected to be minimal).

**Tests (RED before, GREEN after)**:
- `test_dead_code_orphan_documents_over_classification`
- `test_forward_declaration_across_chain_no_unresolved`
- `test_mod_overlay_root_chain_respects_override`
- `test_binary_xs_excluded_from_include_graph`
- `test_include_cycle_falls_back_to_orphan`
- `test_unreadable_include_produces_include_diagnostic_not_graph_edge`

**Dependency**: PR-5

**Estimated diff**: ~200 lines

**Rollback**: revert test-file additions and any small code fixes; the baseline from PR-5 remains valid.

**Cross-cutting concerns**:
- **Risk**: Dead-code over-classification is an accepted V1 behavior; tests must assert the exact current behavior, not an ideal future behavior, to avoid false failures.
- **Verification command**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test root_aware_diagnostics_repro`
- **Documentation update**: Update `docs/xs-language-syntax.md` or `AGENTS.md` if the forward-declaration-acceptance behavior changes observable diagnostics.
- **Memory observation**: Record the exact orphan/cycle fallback semantics; V2 may revisit dead-code classification.

---

## Final integration checklist

- [ ] PR-1 GREEN: `cargo test --test include_graph_repro`
- [ ] PR-2 GREEN: `cargo test --test per_root_cache_repro`
- [ ] PR-3 GREEN: `cargo test --test merged_view_root_repro`
- [ ] PR-4 GREEN: `cargo test --test indirect_include_symbol_repro`
- [ ] PR-5 GREEN: `cargo test --test root_aware_diagnostics_repro`
- [ ] PR-6 GREEN: extended `cargo test --test root_aware_diagnostics_repro`
- [ ] Full workspace baseline green: `cargo test --manifest-path tools/xs-language-server/Cargo.toml`
- [ ] Linter / formatting clean: `cargo fmt --manifest-path tools/xs-language-server/Cargo.toml --check`
