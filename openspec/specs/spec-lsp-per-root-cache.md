# LSP Per-Root Merged-View Cache Specification

<!-- delta-spec for archived change 2026-07-05-lsp-include-graph-aware-diagnostics -->

> **Added/updated by change:** `lsp-include-graph-aware-diagnostics`
> **Archived:** 2026-07-05
> **Change verdict:** PASS

## Purpose

The LSP server SHALL cache `MergedView` instances by root path and closure content hash so that files reachable from the same root share one built view. This prevents repeated closure walks when diagnosing files inside popular root chains (for example, files reachable from `ai/core/main.xs`).

## Requirements

### R1 — Cache key

Each cached root view SHALL be keyed by a `(root_path, closure_content_hash)` tuple.

- GIVEN a root `R`, WHEN a root view is cached, THEN the key SHALL contain the absolute path of `R` and a SHA-256 hash computed from the sorted `(path, content_hash)` tuples of every file in `R`'s include closure.

### R2 — Cache hit returns the same view

A request for the same root with an unchanged closure SHALL return the cached `Arc<MergedView>`.

- GIVEN `MergedView(R)` has been built and cached, WHEN another request for `MergedView(R)` arrives with the same closure hash, THEN it SHALL return the same `Arc` and SHALL NOT rebuild the view.

### R3 — Cache miss builds and stores

A request for a root that is not cached or whose closure changed SHALL build the view and store it.

- GIVEN no cached entry exists for root `R`, WHEN `MergedView(R)` is requested, THEN the server SHALL build the view and insert it into the cache.

### R4 — Closure change invalidates the entry

Changing any file in a root's closure SHALL invalidate that root's cached entry.

- GIVEN a cached entry for root `R`, WHEN any file `F` in `R`'s closure changes, THEN the entry for `R` SHALL be invalidated.
- GIVEN roots `R1` and `R2` both reach file `F`, WHEN `F` changes, THEN entries for both `R1` and `R2` SHALL be invalidated.

### R5 — Unchanged roots are reused while changed roots rebuild

A diagnosed file reachable from multiple roots SHALL reuse cached views for unchanged roots and rebuild only changed roots.

- GIVEN file `X` is reachable from roots `R1` and `R2`, AND `R1`'s closure is unchanged, AND `R2`'s closure has changed, WHEN diagnostics are published for `X`, THEN the server SHALL reuse `R1`'s cached view AND rebuild `R2`'s view.

## Scenarios

### S1 — Root view cached on first build

- GIVEN the server computes `MergedView(root = R.xs)` for the first time
- WHEN the result is produced
- THEN the server SHALL cache it keyed by `(R_path, closure_content_hash)`
- AND a later request for `MergedView(R.xs)` with the same hash SHALL return the cached view

### S2 — Closure change invalidates root cache

- GIVEN a cached `MergedView(R.xs)` exists
- AND any file `F` in `R.xs`'s closure changes
- WHEN the next access to `MergedView(R.xs)` occurs
- THEN the cache entry SHALL be invalidated
- AND the view SHALL be recomputed

### S3 — Unchanged root reused while changed root rebuilds

- GIVEN file `X.xs` is reachable from roots `R1.xs` and `R2.xs`
- AND `R1.xs`'s closure has not changed
- AND `R2.xs`'s closure has changed
- WHEN diagnostics are published for `X.xs`
- THEN the server SHALL reuse `R1.xs`'s cached view
- AND SHALL recompute `R2.xs`'s view

### S4 — Shared closure file invalidates multiple roots

- GIVEN roots `R1.xs` and `R2.xs` both include `shared.xs`
- AND both `R1.xs` and `R2.xs` have cached views
- WHEN `shared.xs` changes
- THEN both cache entries SHALL be invalidated

## Out of scope

- Disk persistence of root views across server restarts.
- Cross-session cache sharing.
- Compression or eviction policy for large root-view caches.

## Verification approach

- Rust integration tests in `tools/xs-language-server/lsp/tests/per_root_cache_repro.rs` covering S1–S4.
- Server integration tests that watch a file change and assert cache invalidation.

## Acceptance criteria

1. The cache key SHALL be `(root_path, closure_content_hash)`.
2. A cache hit for the same key SHALL return the existing `Arc<MergedView>` without rebuilding.
3. A cache miss SHALL build the view and store it.
4. Any change to a file in the root's closure SHALL invalidate that root's cache entry.
5. A file shared by multiple roots SHALL, on change, invalidate every root cache whose closure contains it.
6. A diagnosed file reachable from multiple roots SHALL reuse unchanged root views and rebuild only changed ones.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `lsp-include-graph-aware-diagnostics` | 2026-07-05 | PASS | New spec for `PerRootMergedViewCache` keyed by `(root_path, closure_content_hash)`. |
