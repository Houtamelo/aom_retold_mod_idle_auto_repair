# LSP Reverse Include Graph Specification

<!-- delta-spec for archived change 2026-07-05-lsp-include-graph-aware-diagnostics -->

> **Added/updated by change:** `lsp-include-graph-aware-diagnostics`
> **Archived:** 2026-07-05
> **Change verdict:** PASS

## Purpose

The LSP server SHALL maintain a project-wide reverse-include graph so it can answer `roots_that_include(file)` for any visible `.xs` file. This graph is the data foundation for root-aware diagnostics and per-root merged-view caching.

## Requirements

### R1 — Build graph from workspace visible files

The graph SHALL be built from every readable `.xs` file visible to the workspace, resolving include edges through the existing mod-overlay-aware include resolver.

- GIVEN a workspace with `.xs` files, WHEN the graph is built for the first time, THEN it SHALL contain a node for every visible `.xs` file and a forward edge for every successfully resolved `include "..."` directive.
- GIVEN a file includes another file that is overridden by a mod, WHEN the graph is built, THEN the edge SHALL use the mod-overlay path.
- GIVEN a binary or unreadable `.xs` file, WHEN the graph is built, THEN it SHALL be skipped with a warning and contribute no edges.

### R2 — `roots_that_include` query

The graph SHALL expose `roots_that_include(file) -> Vec<PathBuf>` returning the set of files with no dependents that can reach the given file.

- GIVEN file `F` is an orphan, WHEN `roots_that_include(F)` is called, THEN it SHALL return an empty vector (the caller treats `F` as its own root).
- GIVEN file `F` has exactly one root `R`, WHEN `roots_that_include(F)` is called, THEN it SHALL return `vec![R]`.
- GIVEN file `F` is reachable from multiple roots, WHEN `roots_that_include(F)` is called, THEN it SHALL return all roots sorted deterministically.
- GIVEN an include cycle contains `F`, WHEN `roots_that_include(F)` is called, THEN it SHALL return an empty vector so the caller falls back to the orphan rule.

### R3 — Direct-neighbour accessors

The graph SHALL expose direct include and dependent accessors for diagnostics, tests, and future incremental features.

- GIVEN file `F`, WHEN `included_by(F)` is called, THEN it SHALL return the files that `F` directly includes.
- GIVEN file `F`, WHEN `dependents(F)` is called, THEN it SHALL return the files that directly include `F`.
- GIVEN `F` has no direct includes or dependents, WHEN the corresponding accessor is called, THEN it SHALL return an empty slice.

### R4 — Update on workspace changes

The graph SHALL be rebuilt when the workspace file set changes.

- GIVEN the graph is warm, WHEN a workspace folder is added or removed, THEN the graph SHALL be rebuilt on the next `roots_that_include` query.
- GIVEN the graph is warm, WHEN a watched file is created or deleted, THEN the graph SHALL be marked dirty and rebuilt on the next query.

## Scenarios

### S1 — Build graph on first file encounter

- GIVEN file `F.xs` is encountered for the first time in a session
- WHEN the server needs `roots_that_include(F.xs)`
- THEN the server SHALL build or update the reverse-include graph for the whole workspace
- AND SHALL expose `roots_that_include(F.xs)` returning the correct roots

### S2 — Orphan returns empty roots

- GIVEN file `O.xs` has no includers
- WHEN `roots_that_include(O.xs)` runs
- THEN it SHALL return an empty vector
- AND the caller SHALL treat `O.xs` as its own root

### S3 — Single-root chain

- GIVEN chain `R.xs → M.xs → F.xs`
- WHEN `roots_that_include(F.xs)` runs
- THEN it SHALL return `vec![R.xs]`

### S4 — Diamond includes

- GIVEN graph `R → A → D`, `R → B → D`
- WHEN `roots_that_include(D.xs)` runs
- THEN it SHALL return `vec![R.xs]` (single root, not double-counted)

### S5 — Multiple independent roots

- GIVEN graph `R1 → F.xs` and `R2 → F.xs`
- WHEN `roots_that_include(F.xs)` runs
- THEN it SHALL return a vector containing both `R1` and `R2`

### S6 — Cycle falls back to orphan

- GIVEN `A.xs` includes `B.xs` and `B.xs` includes `A.xs`
- WHEN `roots_that_include(A.xs)` runs
- THEN it SHALL return an empty vector
- AND the caller SHALL fall back to treating `A.xs` as its own root

### S7 — Workspace changes refresh the graph

- GIVEN a warm reverse-include graph
- WHEN a new `.xs` file is added under a workspace folder
- THEN the next `roots_that_include` query SHALL rebuild the graph
- AND reflect the new include edges

## Out of scope

- Real-time polling of the filesystem.
- Partial incremental graph updates; V1 rebuilds the whole graph on invalidation.
- Tracking include edges inside binary `.xs` files.

## Verification approach

- Rust integration tests in `tools/xs-language-server/lsp/tests/include_graph_repro.rs` covering S2–S6.
- LSP server tests that exercise workspace-folder add/remove for S7.

## Acceptance criteria

1. The graph SHALL contain every visible readable `.xs` file as a node.
2. The graph SHALL record forward and reverse include edges for every resolved `include "..."` directive.
3. `roots_that_include(file)` SHALL return all files with no dependents that can reach `file`.
4. The returned vectors SHALL be sorted deterministically and deduplicated.
5. Include cycles SHALL result in an empty root set for affected files, triggering the orphan fallback.
6. Workspace-folder and watched-file changes SHALL mark the graph dirty so it rebuilds on the next query.
7. Mod-overlay paths SHALL be used in place of vanilla paths when an include target is overridden.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `lsp-include-graph-aware-diagnostics` | 2026-07-05 | PASS | New spec for `ReverseIncludeGraph` data structure and `roots_that_include` semantics. |
