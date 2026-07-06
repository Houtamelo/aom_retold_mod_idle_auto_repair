# Design: LSP Include-Graph-Aware Diagnostics

## 1. Architecture overview

```text
textDocument/didOpen(F)
        │
        ▼
publish_diagnostics(F)
        │
        ├─ graph = get_or_build_reverse_include_graph(workspace)
        ├─ roots = graph.roots_that_include(F)
        │        ├─ if roots.is_empty() → treat F as its own root
        │        └─ else → sorted list of root paths
        │
        ├─ file_view  = get_or_build_merged_view(F)       // existing forward-closure view
        ├─ local_diags = run_pass(F, file_view as root_view)
        ├─ if local_diags has no unresolved/use-before-decl issues
        │        emit local_diags; return
        │
        ├─ for each root R in roots:
        │        root_view = per_root_cache.get_or_build(R)
        │        pass_diags = run_pass(F, file_view, root_view)
        │        aggregate(pass_diags, R)
        │
        └─ client.publish_diagnostics(uri, aggregated_diagnostics)
```

## 2. Module changes

### `tools/xs-language-server/lsp/src/include_graph.rs` (NEW)

| Function | Summary |
|----------|---------|
| `ReverseIncludeGraph::build(ws, project) -> Self` | Walks every visible `.xs` file, parses `include` directives, resolves edges, stores forward (`included_by`) and reverse (`dependents`) edges. |
| `roots_that_include(&self, file) -> Vec<PathBuf>` | Reverse BFS through `dependents`, collecting files with no dependents. Cycles fall back to `[file]`. |
| `dependents(&self, file)` / `included_by(&self, file)` | Direct-neighbour accessors. |

### `tools/xs-language-server/lsp/src/cache.rs`

| Function | Summary |
|----------|---------|
| `PerRootMergedViewCache::new(cache_dir) -> Self` | Hash-map keyed by `PerRootCacheKey { root, closure_hash }`. |
| `get_or_build(&mut self, root, build_fn) -> Arc<MergedView>` | Hash lookup; on miss calls `build_fn`, stores `Arc<MergedView>` plus closure file list/hash. |
| `invalidate_for_paths(&mut self, paths)` | Drops any entry whose closure contains a changed path. |
| `closure_content_hash(root_source, files)` | SHA-256 of sorted `(path, content_hash)` tuples. |

### `tools/xs-language-server/lsp/src/merged_view.rs`

| Function | Summary |
|----------|---------|
| `MergedView::build_from_root(root, ws, project, cache_dir) -> MergedView` | Loads root source + symbols from disk/cache, calls existing `MergedView::build`. |
| `MergedView::rebase_to_file(&self, file, source, own_table) -> MergedView` | Returns a copy whose `current_file` and `own_table` are `file`, keeping the root-chain symbols. |
| `MergedView::closure_files(&self) -> impl Iterator<Item = &Path>` | Own file + all resolved includes. |

### `tools/xs-language-server/lsp/src/diagnostics.rs`

| Function | Summary |
|----------|---------|
| `collect_all(ctx: &DiagnosticContext) -> DiagnosticsByUri` | Replaces the current 6-argument form. Uses `ctx.file_view`, `ctx.root_view`, and `ctx.project`. |
| `run_pass(ctx) -> DiagnosticsByUri` | Thin wrapper used by the server loop. |
| `DiagnosticCategory` / `categorize` | Re-used as the aggregation bucket. |
| `aggregate_results(per_root: Vec<(PathBuf, DiagnosticsByUri)>, roots: &[PathBuf], current_uri: &Uri) -> Vec<Diagnostic>` | Groups by `(uri, range, category, base_message)`, appends root-provenance suffix. |
| `format_root_suffix(producing_roots, total_roots) -> String` | Implements universal / partial / truncated formatting. |

### `tools/xs-language-server/lsp/src/semantic.rs`

| Function | Change |
|----------|--------|
| `check_extern_collisions(project, current_file, file_view, root_view)` | Gathers symbols only from files in `root_view.files()`; still suppresses same-chain edges. |
| `check_forward_declarations_for_merged_view(project, engine, current_file, file_view, root_view)` | Uses `file_view` for own-file order; uses `root_view` for root-chain fallback. |
| `check_mutable_redefinitions_for_merged_view(file_view, root_view)` | Prefers `root_view` when present (redefinition scope is the full root chain). |
| `forward_callable_merged(file_view, project, callee, call_line, root_view)` | Returns `true` if the callee is callable in `file_view` **or** exists anywhere in `root_view`. |

### `tools/xs-language-server/lsp/src/typecheck.rs`

| Function | Change |
|----------|--------|
| `resolve_workspace_function(merged, root_view, project, name)` | Fallback order: `merged.find(name)` → `root_view.find(name)` → registered rules. Does **not** scan `project.files`. |
| `check_calls_with_merged(..., root_view)` | Thread `root_view` through to `resolve_workspace_function`. |

### `tools/xs-language-server/lsp/src/server.rs`

| Item | Change |
|------|--------|
| `XsLanguageServer.reverse_include_graph` | New `Arc<tokio::sync::RwLock<ReverseIncludeGraph>>`. |
| `XsLanguageServer.per_root_merged_views` | New `Arc<tokio::sync::RwLock<PerRootMergedViewCache>>`. |
| `get_or_build_reverse_graph().await` | Builds from workspace visible files on first access; marks dirty on watched-file add/remove. |
| `get_or_build_root_merged_view(root).await` | Loads root source and hits `PerRootMergedViewCache`. |
| `publish_diagnostics()` | Implements the orchestration diagram: local short-circuit, per-root loop, aggregation, publish. |
| `invalidate_merged_views_for()` | Also invalidates `per_root_merged_views` entries touched by changed paths. |

## 3. Data model

```rust
/// Project-wide include graph, rebuilt when the visible file set changes.
pub struct ReverseIncludeGraph {
    /// file -> files it directly includes
    included_by: HashMap<PathBuf, Vec<PathBuf>>,
    /// file -> files that directly include it
    dependents: HashMap<PathBuf, Vec<PathBuf>>,
}

impl ReverseIncludeGraph {
    pub fn build(workspace: &Workspace, project: &VirtualProject) -> Self;
    pub fn roots_that_include(&self, file: &Path) -> Vec<PathBuf>;
}

/// Key for caching a root-chain merged view.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PerRootCacheKey {
    pub root: PathBuf,
    pub closure_hash: String,
}

pub struct PerRootMergedViewCache {
    cache_dir: PathBuf,
    entries: HashMap<PerRootCacheKey, CachedRootView>,
}

struct CachedRootView {
    view: Arc<MergedView>,
    closure_files: Vec<PathBuf>,
    closure_hash: String,
}

/// One diagnostic pass scoped to a single (file, root) pair.
pub struct DiagnosticContext<'a> {
    pub file: &'a Path,
    pub source: &'a str,
    /// The forward-closure view for the file being diagnosed.
    pub file_view: &'a MergedView,
    /// The full include chain of the root that includesthe file.
    pub root_view: &'a MergedView,
    pub project: &'a semantic::VirtualProject,
    pub engine: &'a EngineApi,
}

/// Pre-aggregation diagnostic produced for one root.
pub struct PerRootDiagnostic {
    pub uri: Uri,
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub code: Option<NumberOrString>,
    pub category: DiagnosticCategory,
    /// Message with any previous aggregation suffix stripped.
    pub base_message: String,
}

/// Aggregated across roots, ready to publish.
pub struct AggregatedDiagnostic {
    pub uri: Uri,
    pub range: Range,
    pub severity: DiagnosticSeverity,
    pub code: Option<NumberOrString>,
    pub message: String,
    pub producing_roots: Vec<PathBuf>,
}
```

## 4. Algorithm pseudocode

### `roots_that_include(F)`

```rust
fn roots_that_include(&self, file: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back(file.to_path_buf());

    while let Some(cur) = queue.pop_front() {
        let parents = self.dependents.get(&cur).map(|v| v.as_slice()).unwrap_or(&[]);
        if parents.is_empty() && cur != file {
            roots.push(cur);
            continue;
        }
        for p in parents {
            if visited.insert(p.clone()) {
                queue.push_back(p.clone());
            }
        }
    }

    if roots.is_empty() {
        // Cycle or true orphan: treat F as its own root.
        return vec![file.to_path_buf()];
    }

    roots.sort_by(|a, b| a.as_os_str().cmp(b.as_os_str()));
    roots.dedup();
    roots
}
```

### `run_pass(F, file_view, root_view) -> DiagnosticsByUri`

```rust
fn run_pass(ctx: &DiagnosticContext) -> DiagnosticsByUri {
    collect_all(ctx)
}
```

`collect_all` is refactored to read every diagnostic source from `ctx`:

```rust
let mut diags: DiagnosticsByUri = HashMap::new();

// parse + definition checks are local to the source
parse_and_definition_checks(ctx, &mut diags);

// type-check uses root_view for workspace-callee fallback
typecheck::check_calls_with_merged(
    ctx.source, ctx.engine, ctx.file_view.own_table(),
    Some(ctx.file_view), Some(ctx.root_view), Some(ctx.project),
    &mut diags,
);

// semantic checks scoped to the root chain
semantic::check_extern_collisions(ctx.project, ctx.file, ctx.file_view, ctx.root_view);
semantic::check_forward_declarations_for_merged_view(
    ctx.project, ctx.engine, ctx.file, ctx.file_view, ctx.root_view,
);
semantic::check_mutable_redefinitions_for_merged_view(ctx.file_view, ctx.root_view);

// include diagnostics from the file's own forward closure
for inc in ctx.file_view.missing_includes() { ... }
```

### `aggregate_results`

```rust
fn aggregate_results(
    per_root: Vec<(PathBuf, DiagnosticsByUri)>,
    roots: &[PathBuf],
    current_uri: &Uri,
) -> DiagnosticsByUri {
    let mut groups: HashMap<DiagnosticKey, AggregatedDiagnostic> = HashMap::new();
    let total_roots = roots.len();

    for (root, by_uri) in per_root {
        for (uri, diags) in by_uri {
            for d in diags {
                let base = strip_root_suffix(&d.message);
                let key = DiagnosticKey {
                    uri: uri.clone(),
                    range: d.range,
                    category: categorize(&d),
                    message: base.clone(),
                };
                groups.entry(key)
                    .or_insert_with(|| AggregatedDiagnostic { /* copy d, producing_roots empty */ })
                    .producing_roots.push(root.clone());
            }
        }
    }

    let mut out: DiagnosticsByUri = HashMap::new();
    for agg in groups.into_values() {
        let suffix = if total_roots <= 1 || agg.producing_roots.len() == total_roots {
            String::new()
        } else {
            format_root_suffix(&agg.producing_roots, total_roots)
        };
        let mut d = Diagnostic {
            message: format!("{}{}", agg.base_message, suffix),
            ..agg.into_diagnostic()
        };
        out.entry(agg.uri).or_default().push(d);
    }
    out
}
```

### `format_root_suffix`

```rust
fn format_root_suffix(producing_roots: &[PathBuf], total_roots: usize) -> String {
    let names: Vec<String> = producing_roots.iter()
        .map(|p| root_name(p))
        .collect();

    if total_roots <= 100 {
        format!("\n  as seen from: {}", names.join(", "))
    } else if names.len() <= 3 {
        format!("\n  as seen from: {}", names.join(", "))
    } else {
        let shown: Vec<_> = names.iter().take(3).cloned().collect();
        let rest = names.len() - 3;
        format!("\n  as seen from: {}, ...and {} more",
                shown.join(", "), rest)
    }
}
```

## 5. Caching strategy

* **Reverse-include graph** is built once on first `roots_that_include` query and rebuilt when a workspace folder is added/removed or a watched file is created/deleted. Rebuilding walks ≤350 `.xs` files and is <200ms in the shipped game folder.
* **Per-root merged views** are cached by `(root_path, closure_content_hash)`. The closure hash is the SHA-256 of every file in the root's include closure, so any file change invalidates the entry.
* **Invalidation** is path-driven: when any file changes, drop cache entries whose stored closure-file list contains that path. This also covers text changes inside an open file because `did_change` re-reads the root source and rebuilds the view.
* **Memory cost**: worst case `O(roots × avg_closure_size)`. With 147 popular roots and ~5 files per closure, this is well under 5MiB of symbol tables plus source text. Binary `.xs` files never enter the graph, so they consume nothing.
* **Thread safety**: both caches live behind `Arc<tokio::sync::RwLock<_>>`. Reads (`get`) take read locks; builds/invalidations take write locks. No handler holds both caches at once.

## 6. Aggregation edge cases

| Situation | Behaviour |
|-----------|-----------|
| Same diagnostic at byte-exact range from two roots | One aggregated diagnostic; roots merged into `producing_roots`. |
| Same diagnostic at different ranges | Different `(range, ...)` keys → two diagnostics emitted. |
| Different identifiers/messages at same range | Different `message` → two diagnostics emitted. |
| One root reports 1 unresolved symbol, another reports 5 different ones | 6 distinct aggregated keys (or fewer if any coincide). |
| Diagnostic produced by all roots | Universal: no suffix. |
| Diagnostic produced by some roots, ≤100 total roots | Suffix lists every producing root name, in the **currently-open → mod-overlay → alphabetical** order (see below). |
| Diagnostic produced by some roots, >100 total roots | Suffix shows first 3 producing names (same priority order) + `...and N more`. |

### Producing-roots ordering (user-confirmed 2026-07-05)

The list of root names that appears after `as seen from:` must be ordered as follows (applied after
the `>100 → truncate to 3 + "and N more"` rule):

1. **Currently-open**: roots whose path corresponds to a file currently open in any LSP editor
   buffer (taken from the LSP state on `publish_diagnostics`). Shown first, in the order buffers
   were opened. The currently-diagnosed file is **excluded** from the open list (it is the subject
   of the diagnostic, not a root that produced it).
2. **Mod-overlay**: roots whose path falls under a registered mod root (any mod listed in
   `Workspace::registered_mods`). Shown next, alphabetically.
3. **Alphabetical remainder**: all other roots, alphabetically.

This ordering is consumed both by `format_root_suffix` (universal/partial/truncated rules) and by
the diagnostic itself when listing same-key producing roots.

The equality key is:

```rust
(uri, range.start, range.end, diagnostic_category, base_message)
```

Severity and code come from the first encountered root; by design all roots produce the same severity/code for a given issue.

## 7. Mod-overlay and binary `.xs` considerations

* `ReverseIncludeGraph::build` iterates `VirtualProject::visible_files`, which already excludes binary `.xs` files via `is_readable_xs_file`. Unreadable files are skipped with a `tracing::warn!` and contribute no edges.
* Include resolution uses `workspace.resolve_include_edge`, which applies mod-overlay precedence. A file overridden by a mod is treated as the override path in both `included_by` and `dependents`.
* A missing or unparseable include target produces an `IncludeDiagnostic::Missing`/`Unreadable` in the **file view** (forward closure), not in the reverse graph. The reverse graph only records successfully resolved edges.
* If `roots_that_include` encounters a cycle, it falls back to `[file]` (orphan rule), which keeps diagnostics visible instead of silently dropping the file.

## 8. Test design

All tests live under `tools/xs-language-server/lsp/tests/` and use small temporary `game/` trees.

| Test file | Fixture | Expected outcome | RED/GREEN |
|-----------|---------|------------------|-----------|
| `indirect_include_symbol_repro.rs` (existing) | `C → B; C → A; A calls B's symbol` | No unresolved diagnostic on A. | Currently RED; turns GREEN after `forward_callable_merged` root-chain fallback. |
| `include_graph_repro.rs` | Chain `R → M → F` plus sibling `R → S` | `roots_that_include(F) == [R]`. Orphan `O` returns `[O]`. | RED → GREEN in PR-1. |
| `per_root_cache_repro.rs` | `R → A → B`; edit `B`, re-diagnose F | Cache hit for unchanged `R`; rebuild after edit. | RED → GREEN in PR-2. |
| `root_aware_diagnostics_repro.rs` | Multiple sub-fixtures | Covers orphan, single-root, multi-root partial/universal/truncated, dead-code orphan, forward-declaration-across-chain, local-short-circuit. | RED → GREEN through PR-3..PR-6. |
| `merged_view_root_repro.rs` (optional small PR-3 test) | `R → A → B`, diagnose A | `rebase_to_file` exposes B's symbol to A's checks. | RED → GREEN in PR-3. |
| `didOpen_latency_budget.rs` (optional, `#[ignore]`) | Real or synthetic high-includer file | `didOpen` <100ms for <5 roots, <500ms for 147-root case. | Not gating CI. |

### Aggregation format assertions

* **Universal** (`>2` roots, all produce): message must NOT contain `as seen from`.
* **Partial** (`2..=100` roots, subset produce): message contains `as seen from: r1, r2, ...` for exactly the producing roots.
* **Truncated** (`>100` roots): first 3 producing names appear, followed by `...and N more`.

## 9. Implementation order (PR slices)

| PR | Scope | Turns GREEN | Est. lines |
|----|-------|-------------|------------|
| **PR-1** | `include_graph.rs` + `include_graph_repro.rs` | Reverse-graph query correctness. | ~180 |
| **PR-2** | `cache.rs` `PerRootMergedViewCache` + `per_root_cache_repro.rs` | Cache hit/miss/invalidation semantics. | ~160 |
| **PR-3** | `MergedView::build_from_root` / `rebase_to_file` + small repro | File can be diagnosed with a sibling root's symbols. | ~150 |
| **PR-4** | Refactor `collect_all` → `DiagnosticContext`; update `semantic.rs` helpers; update `typecheck.rs` fallback; turn existing `indirect_include_symbol_repro.rs` GREEN. | Anchor RED test passes. | ~300 |
| **PR-5** | Aggregation utility + `publish_diagnostics` orchestration + `root_aware_diagnostics_repro.rs` | Orphan, single-root, partial, universal, truncated, local short-circuit. | ~350 |
| **PR-6** | Remaining spec coverage: dead-code orphan, forward-declaration-across-chain, edge cases in mod-overlay/binary `.xs`. | Full scenario coverage. | ~200 |

Each slice is autonomous, compiles, and keeps the 336-test baseline green.

## 10. Risks

| Risk | Mitigation |
|------|------------|
| **Performance cliff on high-includer files** (147 roots) | Lazy local-pass short-circuit + per-root cache are mandatory in PR-5. Optional latency test in PR-5. |
| **Duplicate or dropped aggregated messages** | Stable equality key + explicit edge-case tests (PR-5). |
| **Stale cache after external file edits** | `did_change_watched_files` invalidates both per-URI and per-root caches by changed paths. **Strategy: eager** (user-confirmed 2026-07-05) — the server registers a `**/*.xs` watcher on `on_initialized` and revalidates each affected root on every change notification, instead of waiting for a lazy re-build on the next `didOpen`. |
| **Watcher lifecycle bugs** (file deleted between scans, pattern drift after `.claude-sandbox.toml` changes) | Use `tower-lsp::Client::register_capability` once in `on_initialized`; deregister only on server shutdown. Single glob `**/*.xs` scoped to workspace folders. |
| **Watched-file notifications flooding the server** when many files change at once (e.g. engine reload) | Per-URI coalescing window of 50ms via `tokio::time::sleep` + a `Mutex<HashSet<PathBuf>>`; one coalesced invalidation pass per window. |
| **Inconsistent cross-file fallback if `typecheck.rs` not updated** | PR-4 explicitly changes `resolve_workspace_function` to root-chain fallback. |
| **Mod-overlay / binary `.xs` regressions** | Reuse existing `is_readable_xs_file`, `tracing::warn!`, and `IncludeDiagnostic` paths; covered by existing tests and PR-6 repro. |
| **Cycle in include graph yields no root** | `roots_that_include` falls back to `[file]` (orphan rule), avoiding infinite loops. |

## 11. User-confirmed choice deviations from locked design

Recorded 2026-07-05 by user answer during SDD apply session:

1. **Aggregation producing-roots order = currently-open → mod-overlay → alphabetical** (instead of
   pure alphabetical). Documented in §6 above. Affects PR-5 only.
2. **Cache invalidation = eager via `did_change_watched_files`** (instead of purely lazy on next
   `didOpen`). Documented in §10 above. Affects PR-2 (cache invalidation hooks) and PR-5 (server
   orchestration + watcher registration).
