## Exploration: lsp-include-graph-aware-diagnostics

### Current State

1. **Diagnostics hot path (`server.rs:999`)**. On `textDocument/didOpen` the server builds a `semantic::VirtualProject` for the owning mod, then builds/caches a per-file `MergedView` for the opened file, and finally calls `diagnostics::collect_all` once. There is no concept of "which roots include this file"; the analysis is local to the opened file.

2. **Merged view is forward-only (`merged_view.rs:263`)**. `MergedView::build` walks only the current file's own `include "..."` directives transitively. It cannot see symbols that the engine would paste into the current file from *later* includes of a common root. For example, if root `C.xs` includes `B.xs` then `A.xs`, opening `A.xs` builds a merged view containing only `A.xs`, so a symbol defined in `B.xs` is invisible to `A.xs` even though the engine resolves it at link time.

3. **The exact false-positive gap is in `semantic.rs`**. The non-merged `forward_callable` helper has a fallback block (`semantic.rs:814-824`): a callee is callable if it is defined in *any other file of the project*. The merged variant, `forward_callable_merged` (`semantic.rs:728-746`), lacks that block. `check_forward_declarations_for_merged_view` uses only the merged variant, so it emits `Error 0310: invalid symbol lookup` for callees that are reachable from a sibling in the same include chain. This is why `indirect_include_symbol_repro.rs::test_indirect_include_does_not_emit_unresolved_when_used_as_callee` is RED.

4. **Other pipeline components already have broader fallback or broader scope**.
   - `typecheck::resolve_workspace_function` (`typecheck.rs:482-505`) falls back from the merged view to the full `VirtualProject`, so wrong-argument-count/type diagnostics can currently resolve symbols even when they are not in the file's own include chain.
   - `semantic::check_extern_collisions` (`semantic.rs:325-416`) gathers symbols from the whole project and then suppresses collisions for files on the same include chain. Its scope is broader than one root chain; V1 should likely narrow it.
   - `engine_api.rs` is independent: it resolves `kb*`, `tr*`, `ai*`, etc. against the Doxygen archive and is not affected by the include graph.

5. **Project model is root-unaware**. `semantic::VirtualProject::load_from_workspace` (`semantic.rs:85`) loads every visible file in the mod + game folder (`workspace::VirtualProject::visible_files`), not the forward closure from a root. There is no precomputed reverse-include graph or list of "files that include F".

### Affected Areas

- `tools/xs-language-server/lsp/src/server.rs`
  - `get_or_build_merged_view` / `publish_diagnostics`: need root classification, per-root analysis loop, and diagnostic aggregation.
  - Existing per-file merged-view cache (`cached_merged_view` / `invalidate_merged_views_for`) may need a parallel per-root cache.
- `tools/xs-language-server/lsp/src/diagnostics.rs`
  - `collect_all`: needs to run once per relevant root and aggregate results into the locked one-message-per-diagnostic format (universal, truncated >100 roots).
  - `DiagnosticCategory` / `categorize` will be used to group equivalent diagnostics across roots.
- `tools/xs-language-server/lsp/src/merged_view.rs`
  - `MergedView::build` / `IncludeGraph`: need an entry point that builds from an arbitrary root path, not only from the opened file. Reverse edges (`dependents`) are already present on `IncludeGraph` (`merged_view.rs:146`) but must be computed across the whole project.
- `tools/xs-language-server/lsp/src/semantic.rs`
  - `forward_callable_merged`, `check_forward_declarations_for_merged_view`, `check_mutable_redefinitions_for_merged_view`, and `check_extern_collisions`: all must be re-scoped to the root chain rather than the union of the merged view and the full `VirtualProject`.
- `tools/xs-language-server/lsp/src/typecheck.rs`
  - `resolve_workspace_function`: project-wide fallback should become root-chain fallback for V1 consistency (otherwise dead-code files can still silence symbol errors).
- `tools/xs-language-server/lsp/tests/indirect_include_symbol_repro.rs`
  - The RED `test_indirect_include_does_not_emit_unresolved_when_used_as_callee` is the TDD anchor. New repro tests will be needed for orphan files, single-root, multi-root partial coverage, universal >100 roots, truncated >100 roots, and the forward-declaration- across-the-chain case.

### Approaches

#### 1. Root-aware multi-pass diagnostics (mandatory baseline)

For the opened file F:
- If F is an orphan (has no includers in the workspace), treat F as its own root and run one diagnostic pass exactly as today.
- If F is included by roots R1…Rn, build `MergedView` from each Ri and run `collect_all` against each root chain.
- Aggregate equivalent diagnostics by `(range, category, base message)`. Emit one `Diagnostic` per distinct issue; attach root provenance following the locked format: no tree names when universal, first 3 tree names + "...and N more" when >100 roots.

- **Pros**: Implements the locked V1 design; fixes the RED repro; aligns with engine include-paste semantics.
- **Cons**: Worst-case cost is one full diagnostic pass per root. `ai/core/main.xs` has 147 includers, so a file in that chain could trigger up to 147 passes instead of 1 (~100x+ slowdown without mitigation).
- **Effort**: Medium.
- **Estimated speedup**: None by itself; it is the correctness baseline.
- **Interactions with locked design**: Directly implements the orphan heuristic and the locked aggregation format.

#### 2. Cache merged views per root

Key `MergedView` by `(root_path, content_hash_of_closure)` instead of by opened file. Once root Ri's closed view is built, reuse it for every file reachable from Ri. The existing `MergedViewCacheKey` already captures closure content hashes (`merged_view.rs:221`).

- **Pros**: Eliminates repeated closure walks. Files reachable from the same popular root share one build; memory cost is small because the data is already in `VirtualProject`.
- **Cons**: Requires cache invalidation when any file in a closure changes; needs a root-keyed store parallel to the existing per-URI cache.
- **Effort**: Low.
- **Estimated speedup**: Near O(1) per root after first open; turns worst-case 147 builds into one per distinct root.
- **Interactions**: None; pure performance win. Does not change message format or orphan heuristic.

#### 3. Lazy analysis with local-pass short-circuit

Always run the existing single-file pass first (the current behavior, which is also the orphan case). Only if that local pass produces unresolved-symbol / use-before-declaration diagnostics do we run the multi-root analysis for *those specific issues*. Files that already resolve locally skip the multi-root work entirely.

- **Pros**: Most files resolve locally; this eliminates multi-root work for the common case. The RED repro is exactly a case where the local pass fails and the multi-root pass is needed.
- **Cons**: Two-phase logic in `collect_all`; aggregation only applies to the subset of diagnostics that needed a cross-root check.
- **Effort**: Medium.
- **Estimated speedup**: Eliminates multi-root cost for ~80-90% of files in a typical mod; keeps `didOpen` <100ms in the common case.
- **Interactions**: Preserves universal/truncated aggregation format and the orphan heuristic (orphans are simply the local pass with F as root).

#### 4. Incremental invalidation

Maintain a global reverse-include graph. When a file changes, recompute only the roots whose reachable set changed. Use the reverse graph plus closure membership to invalidate affected cached root views.

- **Pros**: After initial warm-up, most edits affect only a handful of roots.
- **Cons**: Complexity in an async server; must keep reverse graph consistent with mod overlays and binary `.xs` skips; risk of stale diagnostics if invalidation is slightly wrong; hard to unit-test thoroughly.
- **Effort**: Medium-High.
- **Estimated speedup**: Makes repeated edits on the same file fast, but first `didOpen` of a file in a 147-root chain still pays the cost.
- **Interactions**: Pure caching layer; does not affect message format or orphan heuristic.

#### 5. Symbol pre-resolution map

Pre-compute a map `(file, root) → symbols provided to that root's chain`. Then symbol-existence questions become hash lookups. Type-checking and redefinition checking still need source context, so this is not a full replacement for per-root analysis.

- **Pros**: Fast O(1) lookup for "is this name defined somewhere reachable?"; could dramatically speed up the E0310 path that currently fails.
- **Cons**: Expensive to build and maintain; does not help arg-count, type, extern-collision, or mutable-redefinition diagnostics.
- **Effort**: High.
- **Estimated speedup**: Large for symbol-resolution-only checks; otherwise marginal.
- **Interactions**: Works with aggregation, but may require storing root sets per symbol and keeping them in sync with edits.

#### 6. Defer to a `defined_elsewhere`-style fallback for symbol-existence questions

When the only question is "is this name defined anywhere reachable from a root that includes this file?", answer it by scanning each root closure once (reusing the per-root merged view cache) rather than re-running the full `collect_all` per root. Full diagnostics would still run once per root, but the forward-declaration existence check could short-circuit earlier.

- **Pros**: Low-hanging speedup specifically for the unresolved-symbol false-positive that the RED test exercises.
- **Cons**: Adds a second resolution path; does not reduce work for full semantic/type diagnostics.
- **Effort**: Low-Medium.
- **Estimated speedup**: Cuts per-root cost for the E0310 check drastically.
- **Interactions**: Preserves aggregation format and orphan heuristic.

### Recommendation

For V1, combine **Approach 1 (root-aware multi-pass)** + **Approach 3 (lazy local-pass short-circuit)** + **Approach 2 (per-root merged-view cache)**.

- Approach 1 is required to satisfy the locked design and fix the RED repro.
- Approach 3 keeps the common `didOpen` path fast: most files resolve locally, so the multi-root work is only triggered for files with actual cross-file resolution problems.
- Approach 2 prevents the 147-root worst case from repeatedly re-walking the same closure across multiple open files.

Use **Approach 6** as an implementation shortcut inside the forward-declaration path only if profiling shows that symbol-existence scans dominate the per-root cost.

Defer **Approach 4 (incremental invalidation)** and **Approach 5 (symbol pre-resolution map)** to V2 unless V1 profiling shows we still exceed the 100ms `didOpen` budget.

### Risks

- **Performance cliff**: A 147-root file without lazy short-circuit or caching could produce a >100x `didOpen` slowdown. The mitigation stack must ship together with the correctness fix.
- **Aggregation correctness**: Grouping diagnostics from many roots while preserving range, message, and related information needs a stable equality key; subtle differences in per-root ranges (especially for transitive includes) could duplicate or drop messages.
- **Inconsistent cross-file fallback**: `typecheck.rs` currently resolves workspace callees against the whole `VirtualProject`. If V1 does not update this fallback to respect the root chain, the LSP will keep diagnosing dead-code workspace calls as if they were reachable.
- **Mod-overlay edge cases**: Building views from many roots will exercise more random-map/binary `.xs` paths and overlay overrides; existing `tracing::warn!` + `IncludeDiagnostic::Unreadable` handling should cover them, but test coverage must confirm it.

### Ready for Proposal

Yes. The root-cause file/gap is identified, the locked design is mapped to concrete code locations, and performance options are quantified. The next recommended phase is `sdd-propose` to lock V1 scope (baseline + lazy short-circuit + per-root cache) and V2 deferrals, followed by `sdd-spec` / `sdd-design`.
