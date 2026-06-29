# Design: `fix-lsp-false-positive-forward-decl`

## Technical Approach

Precompute the line at which an included symbol becomes visible in the file being analysed (`effective_line`) and store it in `VisibilityProvenance` at merged-view build time. The three line-order checks in `semantic.rs` switch from `include_line()` (raw line in the immediate includer) to `effective_line()` (line in the analysed file where the include chain begins). This fixes Issue #2 without rewinding the include graph on every diagnostic pass.

## Data Structure Changes

Add `effective_line: u32` to both include variants of `VisibilityProvenance`:

```rust
pub enum VisibilityProvenance {
    OwnFile,
    DirectInclude {
        origin: PathBuf,
        introduced_at: PathBuf,
        include_line: u32,     // line in `introduced_at` (unchanged)
        effective_line: u32,   // NEW: line in the analysed file
    },
    TransitiveInclude {
        origin: PathBuf,
        introduced_at: PathBuf,
        include_line: u32,     // line in `introduced_at` (unchanged)
        depth: usize,
        effective_line: u32,   // NEW: line in the analysed file
    },
}
```

- Type is `u32` to match `include_line`, `selection_range.start.line`, and LSP positions.
- `include_line` is retained unchanged for diagnostics/debugging that need the immediate includer's edge.
- Add `pub fn effective_line(&self) -> u32` on `VisibilityProvenance` that returns the new field for include variants and `0` for `OwnFile`.
- Add `pub fn effective_line(&self) -> u32` on `MergedSymbol`:
  - `OwnFile` → `self.symbol.selection_range.start.line`
  - include variants → `self.provenance.effective_line()`

This keeps the OwnFile-vs-include decision in one place; `semantic.rs` can call `ms.effective_line()` directly.

## Propagation Algorithm

The existing merged-view builder is a depth-first walk. Use DFS to minimize change and because source-order traversal of `include` directives naturally yields the earliest reaching edge (the first `include` in the analysed file whose closure reaches the symbol). Cycles are already terminated by the `visited: HashSet<PathBuf>` tracked from the analysed file root.

Signature changes:

```rust
fn walk_includes(
    file: &Path,
    source: &str,
    depth: usize,
    effective_line: u32,   // NEW
    workspace: &Workspace,
    project: &VirtualProject,
    cache_dir: &Path,
    visited: &mut HashSet<PathBuf>,
    view: &mut MergedView,
);

fn add_included_symbols(
    table: &SymbolTable,
    origin: &Path,
    introduced_at: &Path,
    effective_line: u32,   // NEW
    include_line: u32,
    depth: usize,
    view: &mut MergedView,
);
```

Propagation rules:

1. **Current-file symbol**: `effective_line` is irrelevant; `MergedSymbol::effective_line()` falls back to `selection_range.start.line`.
2. **Direct include** in `MergedView::build` at analysed-file line `L`: pass `effective_line = L` into `add_included_symbols` and `walk_includes`.
3. **Transitive include** in `walk_includes` at intermediate-file line `M`: keep the inherited `effective_line`; `include_line` records `M` for the edge.
4. **Cycle**: `visited` prevents re-entry; symbols already added keep the first `effective_line` encountered.
5. **Multiple root includes**: the first source-order traversal that reaches a file wins. Because `parser::extract_include_directives` returns directives in source order, this is the minimum current-file line that reaches the symbol.

## Call-site Updates in `semantic.rs`

All three sites replace raw `include_line()` ordering with `ms.effective_line()` (provided by the new `MergedSymbol` method).

| Function | Current lines | Change |
|---|---|---|
| `resolve_callee` | `semantic.rs:200-243` | Replace `ms.provenance.include_line() < call_line` with `ms.effective_line() < call_line`, folding the separate `OwnFile` clause into `effective_line()`. |
| `effective_line` | `semantic.rs:367-372` | Replace the implementation with `ms.effective_line()`. Keep the helper so callers stay readable. |
| `forward_callable_merged` | `semantic.rs:820-846` | Replace the `def_line` match with `let def_line = ms.effective_line();`. |

Also update `MergedView::visibility_line` (`merged_view.rs:434-436`) to return `ms.effective_line()` instead of `ms.provenance.include_line()`.

## Test Fixture Layout

Create `tools/xs-language-server/tests/forward_decl_repro.rs`. Fixtures are inline `const &str` strings written to a temporary `game/ai/` tree; this matches the integration-test style in `game_folder_parse.rs` and avoids a new public fixture path.

Each test uses a small helper that mirrors `semantic.rs`'s `merged_fixture` but uses the public crate API:

```rust
fn merged_fixture(files: &[(&str, &str)], current_rel: &str) -> (TempDir, SemProject, MergedView, PathBuf) { ... }
```

### Reproducers

1. **`transitive_include_no_false_positive`**: `game/ai/core/main.xs` includes `core/core.xs` at line 0 and calls `setupDebugCategories` at line 2; `core/core.xs` includes `utilities/debug.xs` at a late line (e.g. line 100) where `setupDebugCategories` is defined. Assert zero "before declaration" diagnostics.
2. **`same_file_missing_forward_decl_is_still_error`**: single file with `void foo() { bar(); }` before `void bar() {}`. Assert one "before declaration" diagnostic.
3. **`mutable_call_before_redefinition_is_clean`**: analysed file includes a file containing `mutable void helper() {}` followed by `void helper() {}`; analysed file calls `helper()` after the include. Assert zero diagnostics.
4. **`cyclic_include_terminates`**: `a.xs` includes `b.xs`, `b.xs` includes `a.xs`. Build merged view for `a.xs` and assert `graph().is_cyclic()` without hanging.
5. **`multiple_root_include_takes_first_line`**: analysed file includes the same target at lines 5 and 10 (0-indexed). Assert the symbol's `effective_line` is 5.

## Strict-TDD Task List

1. Add `transitive_include_no_false_positive` test; run and confirm RED.
2. Add `effective_line: u32` field to `VisibilityProvenance::DirectInclude` / `TransitiveInclude` (GREEN compiler, no behaviour change).
3. Update `MergedView::build`, `walk_includes`, and `add_included_symbols` to propagate `effective_line`.
4. Add `VisibilityProvenance::effective_line()` and `MergedSymbol::effective_line()`.
5. Update `resolve_callee`, `effective_line`, and `forward_callable_merged` to use `effective_line()`.
6. Update `MergedView::visibility_line` to return `effective_line()`.
7. Run `cargo test`; confirm Step 1's test is GREEN.
8. Add tests (2)–(5); run and confirm GREEN.
9. Run `AOMR_GAME_PATH=... cargo test --test game_folder_parse -- --nocapture`; confirm all R6 counts remain zero.
10. Update the `effective_line` doc-comment in `semantic.rs` to state that ordering uses the current-file include line.
11. Bump `tools/intellij-xs-plugin/gradle.properties` `pluginVersion` from `0.2.2` to `0.2.3` per AGENTS.md (bug fix in LSP sources changes the bundled artifact).

## Risk Mitigation

- **Mutable ordering**: covered by test (3) above; any regression turns a clean diagnostic into a false positive or misses a real signature error.
- **Cycles**: test (4) completes in bounded time; a regressed walker would infinite-loop and be caught by the test harness timeout.
- **Multiple-root edge ordering**: test (5) asserts the first source-order edge's line is stored. If DFS ordering changes, this test fails.
- **Existing test regressions**: no existing test asserts numeric `include_line()` values. If one is added between design and apply, reinterpret it to assert `effective_line()` for ordering and `include_line()` only for edge diagnostics.
- **game_folder_parse regression**: the Issue #2 reproduction path is exactly the vanilla `main.xs` layout; the integration test's top-level count assertions should remain at zero.

## Out-of-scope Reminders

- Issues 1, 3, and 4 from `docs/issues/2026-06-29-runtime-issues.md`.
- Duplicate-`extern` diagnostics surfaced while reproducing Issue #2.
- R3-F-04 engine-API reference scope.
- Plugin-side Kotlin changes.
- Grammar/parser/tree-sitter changes.

## ADRs

### ADR-1: Precompute vs on-demand effective line

- **Context**: `semantic.rs` currently needs the current-file include line for ordering, but only `VisibilityProvenance::include_line()` (the intermediate-file line) is stored.
- **Decision**: Precompute `effective_line` during merged-view construction.
- **Consequences**: Line-order checks become O(1); cycles and multi-root edges are resolved once at build time. The merged view is already cached, so no per-keystroke graph walk is added.
- **Alternatives**: Compute the line on demand by walking `IncludeGraph`. Rejected because `collect_all` runs diagnostics on every keystroke and the same graph walk would repeat for every call site.

### ADR-2: Minimum vs maximum for multiple-root includes

- **Context**: The same symbol may be reachable from the analysed file through two different root `include` directives at different lines.
- **Decision**: Use the line of the first reaching edge encountered during source-order DFS.
- **Consequences**: The earliest include in the file wins, matching the textual-paste model and giving the most permissive (earliest visibility) line.
- **Alternatives**: Use the maximum/latest edge. Rejected because it would flag calls as "before declaration" even though an earlier include already made the symbol visible.

### ADR-3: DFS vs BFS for cycle handling

- **Context**: The merged view already walks includes recursively.
- **Decision**: Keep DFS and the existing `HashSet<PathBuf>` visited guard.
- **Consequences**: Implementation change is small; source order is preserved; cycles terminate cleanly because a file is added to `visited` before its children are walked.
- **Alternatives**: Switch to BFS. Rejected: no benefit for the "first reaching edge" rule and would require larger restructuring.

### ADR-4: Own-file effective line

- **Context**: `VisibilityProvenance::OwnFile` has no include line.
- **Decision**: `MergedSymbol::effective_line()` returns `symbol.selection_range.start.line` for `OwnFile`; include variants store the precomputed current-file line.
- **Consequences**: Callers have one accessor for all line-order comparisons.
- **Alternatives**: Make `effective_line` an `Option<u32>` on the enum. Rejected because it forces callers to unwrap and duplicates the already-known symbol position.

### ADR-5: Test fixture layout

- **Context**: New regression tests need small multi-file XS layouts.
- **Decision**: Inline fixture strings in `tests/forward_decl_repro.rs`, written to a temporary `game/ai/` directory at test time.
- **Consequences**: Tests are self-contained; no new crate public API or `include_str!` directory needed.
- **Alternatives**: Add `.xs` files under `tools/xs-language-server/tests/fixtures/`. Rejected: existing tooling tests prefer inline strings or `semantic_fixtures` under `src/`; a new `tests/fixtures/` convention is unnecessary for five small reproducers.

## File Changes

| File | Action | Description |
|---|---|---|
| `tools/xs-language-server/src/merged_view.rs` | Modify | Add `effective_line` field; propagate it in `build`, `walk_includes`, `add_included_symbols`; add `VisibilityProvenance::effective_line()` and `MergedSymbol::effective_line()`; update `visibility_line()`. |
| `tools/xs-language-server/src/semantic.rs` | Modify | Update `resolve_callee`, `effective_line` helper, and `forward_callable_merged` to use `ms.effective_line()`; update the helper doc-comment. |
| `tools/xs-language-server/tests/forward_decl_repro.rs` | Create | Strict-TDD regression tests for Issue #2: transitive includes, same-file error, mutable ordering, cycles, and multiple-root includes. |
| `tools/intellij-xs-plugin/gradle.properties` | Modify | Bump `pluginVersion` `0.2.2` → `0.2.3`. |

## Testing Strategy

| Layer | What | Approach |
|---|---|---|
| Unit | Provenance field propagation | `merged_view.rs` existing tests plus new assertions on `ms.effective_line()` in `forward_decl_repro.rs`. |
| Integration | Issue #2 reproducer | `tests/forward_decl_repro.rs` runs the full semantic pipeline on synthetic fixtures. |
| Integration | Real game folder | `AOMR_GAME_PATH=... cargo test --test game_folder_parse` asserts all tracked counts remain zero. |

## Migration / Rollout

No schema, cache, or deployed-mod migration. A clean revert of `merged_view.rs`, `semantic.rs`, the new test file, and `gradle.properties` restores the previous behaviour.

## Open Questions

None anticipated.
