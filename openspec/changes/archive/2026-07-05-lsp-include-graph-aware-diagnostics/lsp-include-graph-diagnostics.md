# LSP Include-Graph-Aware Diagnostics

> **Capability:** `lsp-include-graph-aware-diagnostics`  
> **Modified capability:** `semantic-diagnostics`  
> **Change:** `lsp-include-graph-aware-diagnostics`

## Purpose

When an `.xs` file is opened, the LSP server SHALL discover every workspace root that includes that file, run diagnostics against each root's include chain, and aggregate equivalent results into one LSP `Diagnostic` per distinct issue. This aligns edit-time diagnostics with the engine's link-time include-paste semantics.

## Requirement 1: Root classification

### Scenario: Orphan file treated as its own root

- GIVEN file `F` has zero includers in the workspace
- WHEN `textDocument/didOpen` arrives for `F`
- THEN the LSP SHALL treat `F` as its own root
- AND SHALL emit diagnostics from exactly one pass against `F` as root
- AND SHALL NOT consult any other root for this file

### Scenario: Single-root file uses its includer as root

- GIVEN file `F` has exactly one includer `R`
- WHEN `textDocument/didOpen` arrives for `F`
- THEN the LSP SHALL treat `R` as the only root for `F`
- AND SHALL emit diagnostics from `R`'s include closure
- AND the diagnostic message SHALL mention `as seen from: R` unless `R` is the only root

### Scenario: Multi-root universal coverage omits tree names

- GIVEN file `F` has `n ≥ 2` includers `R1..Rn`
- AND diagnostic `D` appears in every root closure
- WHEN diagnostics are published for `F`
- THEN the LSP SHALL emit `D` exactly once
- AND SHALL NOT list any tree names in the message

### Scenario: Multi-root partial coverage lists producing trees

- GIVEN file `F` has `n` includers `R1..Rn` where `2 ≤ n ≤ 100`
- AND diagnostic `D` appears in only some root closures
- WHEN diagnostics are published for `F`
- THEN the LSP SHALL emit `D` exactly once
- AND SHALL list the tree names that produced `D`

### Scenario: Multi-root truncated coverage for more than 100 includers

- GIVEN file `F` has `n > 100` includers `R1..Rn`
- AND diagnostic `D` appears in only some root closures
- WHEN diagnostics are published for `F`
- THEN the LSP SHALL emit `D` exactly once
- AND SHALL truncate the tree-name list to the first three followed by `...and N more`

## Requirement 2: Multi-pass analysis

### Scenario: Compute roots and run one pass per root

- GIVEN `textDocument/didOpen` arrives for file `F`
- WHEN the server begins diagnostic analysis
- THEN it SHALL compute `roots = roots_that_include(F)`
- AND if `roots` is empty it SHALL treat `F` as the sole root
- AND for each root `R` in `roots` it SHALL run diagnostics against `R`'s include chain
- AND it SHALL aggregate equivalent diagnostics by `(range, category, base message)` into one `Diagnostic` per distinct issue

### Scenario: Same diagnostic at different ranges is distinct

- GIVEN two roots produce the same diagnostic at different ranges
- WHEN diagnostics are aggregated
- THEN the LSP SHALL treat them as distinct diagnostics because range is part of the equality key
- AND SHALL emit both

### Scenario: Different identifiers at the same range are distinct

- GIVEN two roots produce diagnostics for different identifiers at the same range
- WHEN diagnostics are aggregated
- THEN the LSP SHALL treat them as distinct diagnostics because message is part of the equality key
- AND SHALL emit both

## Requirement 3: Lazy local-pass short-circuit

### Scenario: Local pass with no cross-file issues skips multi-root work

- GIVEN `textDocument/didOpen` arrives for file `F`
- AND the local diagnostic pass with `F` as root produces no cross-file symbol issues
- WHEN the server publishes diagnostics
- THEN the LSP SHALL skip the multi-root loop entirely
- AND SHALL emit the local-pass diagnostics directly

### Scenario: Reachable unresolved symbol is suppressed locally and aggregated across roots

- GIVEN the local pass for `F` reports an unresolved symbol `X`
- AND `X` is reachable from at least one root chain that includes `F`
- WHEN the server runs the multi-root pass
- THEN the multi-root pass SHALL suppress the local diagnostic for `X`
- AND SHALL emit at most one diagnostic per distinct issue with aggregation

## Requirement 4: Per-root merged-view cache

### Scenario: Root view cached on first build

- GIVEN the LSP computes `MergedView(root=R)` for the first time
- WHEN the result is produced
- THEN the LSP SHALL cache it keyed by `(R_path, content_hash_of_closure)`
- AND a later request for `MergedView(R)` with the same hash SHALL return the cached view

### Scenario: Closure change invalidates root cache

- GIVEN a cached `MergedView(R)` entry exists
- AND any file `F` in `R`'s closure changes
- WHEN the next access to `MergedView(R)` occurs
- THEN the cache entry for `R` SHALL be invalidated
- AND the view SHALL be recomputed

### Scenario: Unchanged root views are reused while changed ones recompute

- GIVEN opened file `F` is reachable from roots `R1` and `R2`
- AND `R1`'s closure has not changed
- AND `R2`'s closure has changed
- WHEN diagnostics are published for `F`
- THEN the LSP SHALL reuse `R1`'s cached view
- AND SHALL recompute `R2`'s view

## Requirement 5: Forward declarations across the include chain

### Scenario: Cross-chain call without forward declaration is accepted

- GIVEN file `A.xs` is reachable from root `R` as `R → A → ... → B`
- AND `A.xs` calls a function defined in `B.xs`
- AND `B.xs` appears after `A.xs` in `R`'s include chain
- AND `A.xs` does not forward-declare that function
- WHEN the LSP analyzes `A.xs`
- THEN it SHALL NOT emit an `undefined symbol` diagnostic for that function
- AND it SHALL emit `used at line N before declaration; add forward declaration or mark 'mutable'` only if the function is not reachable from any root that includes `A.xs`

## Requirement 6: Workspace-wide fallback rescoping

### Scenario: Callee fallback is limited to reachable root chains

- GIVEN `typecheck::resolve_workspace_function` looks up a callee
- WHEN the callee is not found in the file's merged view
- THEN it SHALL fall back to symbols defined in any file reachable from a root that includes the current file
- AND SHALL NOT use the whole `VirtualProject` as fallback

## Requirement 7: Reverse-include graph

### Scenario: Build reverse graph on first file encounter

- GIVEN file `F` is encountered for the first time in a workspace session
- WHEN the server needs `roots_that_include(F)`
- THEN it SHALL build or update the reverse-include graph for the whole workspace
- AND SHALL expose `roots_that_include(F) -> Vec<RootFile>`

### Scenario: Workspace changes update the graph

- GIVEN a reverse-include graph exists
- WHEN a file is added or removed from the workspace
- THEN the graph SHALL be updated accordingly

## Requirement 8: Test coverage

### Scenario: Anchor RED test turns GREEN

- GIVEN the existing test `tools/xs-language-server/lsp/tests/indirect_include_symbol_repro.rs::test_indirect_include_does_not_emit_unresolved_when_used_as_callee`
- WHEN the implementation is complete
- THEN the test SHALL pass

### Scenario: New root-aware diagnostics test covers required cases

- GIVEN a new test file `tools/xs-language-server/lsp/tests/root_aware_diagnostics_repro.rs`
- WHEN it runs against the implementation
- THEN it SHALL cover:
  - an orphan file gets full diagnostics
  - a single-root reachable file emits `as seen from: R` or omits the name when `R` is the only root
  - multi-root partial coverage lists the producing trees
  - multi-root universal coverage (`>2` roots) emits no tree names
  - multi-root truncated coverage (`>100` roots) emits `...and N more`
  - a dead-code orphan file documents over-classification behavior
  - forward declaration across the chain turns from RED to GREEN
  - the local-pass short-circuit preserves diagnostics for files without cross-file issues
