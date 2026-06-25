# Tasks: `true-include-paste`

## Review Workload Forecast

**Estimated changed lines:** ~1,050 (human-written + generated)
- Tree-sitter generated parser: ~40 grammar.js + regenerated `src/parser.c`/`src/grammar.json`/`src/node-types.json`
- Rust implementation: ~760 new / ~360 modified
- Rust tests + fixtures: ~300 new

**Chained PRs recommended:** yes
**Reason:** Total surface is ~1,050 LOC and spans grammar regeneration, a new `merged_view.rs` module, and rewiring of every LSP handler. The four natural slices (grammar/symbols → merged-view core → handler plumbing → tests/threshold) are independently mergeable and keep each review under ~400 lines of human-written code.

**400-line budget risk:** low
**Reasoning:** Each PR is scoped to one coherent layer. The largest human-written slice is the new `merged_view.rs` (~260 LOC) in PR 2, which is still under the budget. Generated parser churn lives in PR 1 and is mechanical.

**Chain strategy:** stacked-to-main
**PRs:** 4 sequential PRs targeting `xs-language-server/true-include-paste` (or `main` if the target branch is the integration point), each rebased on the previous.

---

## 1. PR strategy

This change is split into **4 chained PRs** using the cached `stacked-to-main` strategy.

| PR | Title | Tasks | Rationale |
|---|---|---|---|
| **PR 1 — Grammar + symbols** | feat(xs-lsp): grammar and symbol extraction for function-pointer defaults | T1, T2 | Self-contained parser/symbol work with a large generated diff. Merging first unblocks every downstream consumer that needs `bo_*` symbols. |
| **PR 2 — Merged view plumbing** | feat(xs-lsp): textual-paste merged view for include scope | T3, T4 | Builds the core include-graph engine and visibility filter. PR 1 must land first so the merged view has parseable symbols to merge. |
| **PR 3 — Handler integration** | feat(xs-lsp): wire merged view into completion, semantic, and server state | T5, T6, T7, T8, T9, T10 | Consumes `MergedView` in every LSP handler and the diagnostic pipeline. Depends on PR 2. |
| **PR 4 — Tests + threshold** | test(xs-lsp): include-paste fixtures, roundtrips, and game-folder threshold | T11, T12, T13 | Adds scenario coverage and tightens the game-folder acceptance guard. Depends on PR 3. |

PR 1 and PR 2 could conceptually be independent, but PR 2's merged-view unit tests are more meaningful once `bo_*` functions parse, so the chain order above is preferred.

---

## 2. Tasks

- [ ] **T1 — Grammar: add `function_pointer_type` and `lambda_expression` rules**
  - **Files:** `tools/xs-language-server/tree-sitter-xs/grammar.js`; regenerated `tools/xs-language-server/tree-sitter-xs/src/parser.c`, `src/grammar.json`, `src/node-types.json`
  - **Acceptance:**
    - Grammar parses `void boVillager(int a = -1, void(int) afterQueue = [](int id = -1) {}) { }` as a `function_definition` named `boVillager` with two parameters (scenarios 1, 2, 3).
    - `void boConditionalWait(int planID = -1, bool() condition = []() -> bool { return(true); }) { }` parses as `function_definition`.
    - Existing 75 unit tests still pass with no regression.
  - **Spec coverage:** scenarios 1, 2, 3.
  - **LOC delta:** +40 grammar.js / generated parser churn
  - **Notes:** Run `tree-sitter generate` inside `tools/xs-language-server/tree-sitter-xs/` before committing. The generated `src/parser.c` diff is expected to be large and mechanical.
  - **Commit:** `feat(xs-lsp): extend tree-sitter grammar with function-pointer types and lambda expressions`

- [ ] **T2 — Symbols: extract function-pointer defaults and recover function definitions from ERROR nodes**
  - **Files:** `tools/xs-language-server/src/symbols.rs`
  - **Acceptance:**
    - `boVillager`, `boBuild`, `boUnit`, `boTransaction`, `boConditionalWait`, and the dominant `bo_*` framework in `bo_system.xs` appear as `SymbolKind::Function` symbols in the per-file symbol table (scenarios 1, 2).
    - Function-pointer parameter types render as `return_type(params)` in `Symbol.params[].ty` (e.g., `void(int)`).
    - Lambda defaults are captured as raw source text in `Symbol.params[].default`.
    - `extract_error_function_definition` recovers function symbols from `ERROR` nodes that contain a body.
    - Existing 75 unit tests still pass.
  - **Spec coverage:** scenarios 1, 2, 3.
  - **LOC delta:** +70 / ~30
  - **Notes:** No change to `symbols::Visibility`; provenance will be added later by `merged_view.rs`.
  - **Commit:** `feat(xs-lsp): extract function-pointer parameters and lambda defaults from XS definitions`

- [x] **T3 — Parser + workspace: expose include directives and overlay-relative helpers**
  - **Files:** `tools/xs-language-server/src/parser.rs`, `tools/xs-language-server/src/workspace.rs`
  - **Acceptance:**
    - `parser::extract_include_directives(tree, source)` returns a `Vec<(String, Range)>` of resolved target strings and directive ranges.
    - `workspace::game_relative_path(path)` and `workspace::relativize` are `pub(crate)` so `MergedView::build` can compute cache keys and use `cache::load_or_parse_symbols`.
    - `workspace::resolve_include` exposes enough metadata to construct an `IncludeEdge` (`from`, `to`, `root`, `include_line`).
    - Existing parser and workspace tests still pass (scenarios 4, 7, 8 partial).
  - **Spec coverage:** partial scenarios 4, 7, 8; `spec-virtual-project-overlay.md` include-root scenarios.
  - **LOC delta:** +70 / ~20
  - **Notes:** Keep this slice small and mechanical; the graph builder and cycle detection belong in T4.
  - **Commit:** `feat(xs-lsp): expose IncludeEdge and tree-sitter include walker from workspace` (PR 2 commit 1)

- [x] **T4 — Merged view: new `merged_view.rs` module**
  - **Files:** `tools/xs-language-server/src/merged_view.rs` (NEW), `tools/xs-language-server/src/lib.rs`
  - **Acceptance:**
    - `MergedView::build` resolves includes through `workspace::resolve_include`, recurses transitively, and terminates cleanly on cycles keyed by absolute path (scenarios 4, 5, 6).
    - Missing targets produce `IncludeDiagnostic` without aborting the remaining merge (scenario 7).
    - Mod overlay is preferred over vanilla when both exist (scenario 8; `spec-virtual-project-overlay.md` merged-view overlay scenario).
    - Visibility filtering hides `static`/file-local variables from included files and keeps `extern` variables plus all non-`static` functions (scenarios 9, 10, 11).
    - New unit tests cover direct include, transitive include, cycle, missing target, mod overlay, `AI`/`TRIGGER`/`RANDOM_MAP` roots, `static` hidden, `extern` visible.
    - Existing tests still pass.
  - **Spec coverage:** scenarios 4, 5, 6, 7, 8, 9, 10, 11.
  - **LOC delta:** +260 / 0
  - **Notes:** Register `pub mod merged_view;` in `lib.rs` after `pub mod workspace;`. Include `VisibilityProvenance`, `MergedSymbol`, `IncludeEdge`, `IncludeGraph`, `IncludeDiagnostic`, `MergeError`, `MergedViewCacheKey`.
  - **Commit:** `feat(xs-lsp): add merged_view module for textual-paste include resolution` (PR 2 commit 2)

- [x] **T5 — Completion: replace regex approximation with merged view**
  - **Files:** `tools/xs-language-server/src/completion.rs`
  - **Acceptance:**
    - `completion::included_files` regex and the `path.ends_with(target)` check are removed.
    - `completion::complete_merged` consumes `MergedView::matching`.
    - `textDocument/completion` offers symbols from the include closure, including included functions and `extern` variables (scenario 13).
    - File-local variables remain hidden outside their declaring file.
    - Existing completion tests still pass.
  - **Spec coverage:** scenario 13.
  - **LOC delta:** +20 / -60
  - **Notes:** Net deletion is expected because the regex scan and path-matching logic are removed.
  - **Commit:** `refactor(xs-lsp): drive completion from merged view instead of include-regex approximation`

- [x] **T6 — Semantic: route forward-decl and mutable checks through merged view**
  - **Files:** `tools/xs-language-server/src/semantic.rs`
  - **Acceptance:**
    - `check_forward_declarations_for_merged_view` and `forward_callable_merged` resolve callees against `MergedView` and respect include-directive line numbers.
    - `check_mutable_redefinitions_for_merged_view` operates on the include closure as the effective translation unit.
    - `check_extern_collisions` remains project-wide via `VirtualProject`.
    - A call before an `include` line still errors; a call after the `include` line resolves (scenario 11 + `spec-semantic-diagnostics.md` call-before-include scenario).
    - `static` variables from included files stay unresolved in the includer (scenario 9).
    - Existing semantic tests still pass.
  - **Spec coverage:** scenarios 9, 10, 11, 12; `spec-semantic-diagnostics.md` include scenarios.
  - **LOC delta:** +90 / ~80
  - **Notes:** Leave the existing project-wide forward-callable path as a fallback only for files with no registered project.
  - **Commit:** `feat(xs-lsp): resolve forward declarations and mutable redefinitions through merged view`

- [x] **T7 — Typecheck: route cross-file function lookup through merged view**
  - **Files:** `tools/xs-language-server/src/typecheck.rs`
  - **Acceptance:**
    - `resolve_workspace_function` looks up user functions in `MergedView` instead of the full `VirtualProject`.
    - Calls to functions defined in included files type-check correctly (scenarios 11, 12).
    - Existing typecheck tests still pass.
  - **Spec coverage:** scenarios 11, 12.
  - **LOC delta:** +20 / ~10
  - **Notes:** Keep `int` ↔ `float` widening behavior intact.
  - **Commit:** `feat(xs-lsp): resolve cross-file function types through merged view`

- [x] **T8 — Diagnostics: wire merged view into `collect_all`**
  - **Files:** `tools/xs-language-server/src/diagnostics.rs`
  - **Acceptance:**
    - `diagnostics::collect_all` accepts an optional `MergedView` and passes it to semantic + typecheck.
    - `extern` collision still receives the full project; forward-decl/mutable/type checks receive the merged view.
    - Existing diagnostic tests still pass.
  - **Spec coverage:** scenarios 4–12 (cross-cutting).
  - **LOC delta:** +15 / ~10
  - **Notes:** Make the `MergedView` parameter `Option<&MergedView>` so callers that lack one degrade gracefully.
  - **Commit:** `feat(xs-lsp): wire merged view into the diagnostic pipeline`

- [x] **T9 — Server: build, cache, and invalidate merged views per open file**
  - **Files:** `tools/xs-language-server/src/server.rs`, `tools/xs-language-server/src/cache.rs`
  - **Acceptance:**
    - `XsLanguageServer` stores `merged_views: HashMap<Url, (MergedViewCacheKey, MergedView)>`.
    - `get_or_build_merged_view` returns a cached view when the content-hash key matches.
    - `did_close` removes the entry for the URI.
    - `did_change_watched_files` invalidates every cached merged view whose closure contains the changed path or any of its dependents, then re-diagnoses affected open files (scenario 16).
    - A 20-file include closure builds within 200 ms with warm per-file parse caches (scenario 17).
    - Existing LSP roundtrip tests still pass.
  - **Spec coverage:** scenarios 16, 17.
  - **LOC delta:** +100 / ~70
  - **Notes:** `MergedViewCacheKey` hashes current file text and sorted `(include_path, include_text_hash)` tuples.
  - **Commit:** `feat(xs-lsp): cache merged views per open file with include-graph invalidation`

- [x] **T10 — Hover, definition, and references: cross-include resolution**
  - **Files:** `tools/xs-language-server/src/server.rs` (handlers), `tools/xs-language-server/src/references.rs`
  - **Acceptance:**
    - `textDocument/hover` on an included symbol shows its signature and a `file://` URI pointing to the defining file (scenario 14).
    - `textDocument/definition` on an included symbol returns a single `Location` whose URI is the include target (scenario 15).
    - `textDocument/references` walks every file in the merged closure and returns uses in includer and included files; duplicates from diamond includes are avoided by path-deduplication (scenario 15 cross-cutting; references out-of-scope for rename remains file-local).
    - `textDocument/rename` remains file-local.
    - Existing server/references tests still pass.
  - **Spec coverage:** scenarios 14, 15.
  - **LOC delta:** +85 / ~20
  - **Notes:** Reuse `references::find_identifier_uses` and add a cross-file loop in the server handler.
  - **Commit:** `feat(xs-lsp): resolve hover, definition, and references across include boundaries`

- [x] **T11 — Semantic fixtures for include scenarios**
  - **Files:** `tools/xs-language-server/src/semantic_fixtures/include_*.xs` (NEW), `tools/xs-language-server/src/semantic.rs` (tests)
  - **Acceptance:**
    - 6–8 fixture file pairs for scenarios 4–12: direct include, transitive include, cycle, missing target, mod overlay, `static` hidden, `extern` visible, public function visible, mutable redefinition.
    - New `#[test]` cases in `semantic.rs` load fixtures and assert expected diagnostics.
    - `cargo test semantic` is green.
  - **Spec coverage:** scenarios 4–12.
  - **LOC delta:** +130 / 0
  - **Notes:** Keep fixture files small and focused; one assertion per unit test.
  - **Commit:** `test(xs-lsp): add semantic fixtures for include-paste scenarios`

- [x] **T12 — LSP round-trip tests for cross-include features**
  - **Files:** `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs`, plus fixture files under `tools/xs-language-server/src/test_fixtures/` if needed
  - **Acceptance:**
    - New test sequences for `textDocument/completion` across include (scenario 13), `textDocument/hover` across include (scenario 14), `textDocument/definition` across include (scenario 15), and include-cycle termination (scenario 6).
    - Missing-include diagnostic arrives at the LSP level (scenario 7).
    - `cargo run --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test` is green.
  - **Spec coverage:** scenarios 6, 7, 13, 14, 15.
  - **LOC delta:** +120 / ~20
  - **Notes:** Extend the existing round-trip harness; do not rewrite it.
  - **Commit:** `test(xs-lsp): add LSP roundtrip tests for cross-include resolution`

- [x] **T13 — Integration test: tighten threshold and add per-callee guards**
  - **Files:** `tools/xs-language-server/tests/game_folder_parse.rs`
  - **Acceptance:**
    - `UNRESOLVED_SYMBOL_THRESHOLD` is tightened from 5,335 to below 1,000 (scenario 18).
    - New per-callee regression guard asserts each of the top-10 `bo_*` callees (`boBuild`, `boVillager`, `boUnit`, `boConditionalWait`, `boAdvance`, `boIncreaseTimeout`, `boTransaction`, `boEnd`, `boExecute`, `boTech`) drops by at least 80% from baseline.
    - `cargo test --test game_folder_parse` passes when `AOMR_GAME_PATH` is set.
  - **Spec coverage:** scenario 18.
  - **LOC delta:** +60 / -10
  - **Notes:** Skip cleanly when `AOMR_GAME_PATH` is unset, matching existing integration-test behavior.
  - **Commit:** `test(xs-lsp): tighten unresolved-symbol threshold and add per-callee regression guards`

---

## 3. Dependency graph

```
T1 (grammar)
  │
  ▼
T2 (symbols)
  │
  ▼
T3 (parser + workspace helpers) ──→ T4 (merged_view)
                                      │
          ┌───────────────────────────┼───────────────────────────┐
          ▼                           ▼                           ▼
         T5 (completion)             T6 (semantic)              T10 (hover/def/ref)
          │                           │  │                        │
          │                           ▼  ▼                        │
          │                          T7 (typecheck)               │
          │                           │                           │
          └──────────┐    ┌───────────┘                           │
                     ▼    ▼                                       │
                    T8 (diagnostics) ◄─────────────────────────────┘
                     │
                     ▼
                    T9 (server cache)
                     │
                     ▼
            T11 (semantic fixtures)
                     │
                     ▼
            T12 (LSP roundtrip tests)
                     │
                     ▼
            T13 (game-folder integration threshold)
```

Key sequencing:
- T1 and T2 must precede T4 because `MergedView::build` merges symbols that must first exist.
- T3 must precede T4 because `MergedView::build` depends on `extract_include_directives` and `game_relative_path`.
- T4 must precede T5–T10 because every handler consumes the merged view.
- T9 must precede T10 if the hover/definition/references handlers are to read cached merged views; they can also call `get_or_build_merged_view`, but the cache is added in T9.
- T5–T10 can be developed in parallel once T4 lands, but it is usually simpler to do them in the order listed.
- T11–T13 are test/verification tasks and must follow their respective implementation dependencies.

---

## 4. Risks and mitigations

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| 1 | A lambda variant (e.g., `[]() -> bool { ... }` in `bo_system.xs`) still parses as `ERROR` after the grammar change. | Medium | High | T2 includes ERROR-node recovery for function definitions with bodies; full `bo_system.xs` parse is verified as part of T13's integration run. |
| 2 | Transitive-include cache invalidation misses a dependent open file when a deep include changes. | Medium | High | T4 stores `IncludeGraph` in every `MergedView`; T9 walks the graph in `did_change_watched_files` to invalidate every entry whose closure contains the changed path or any dependent. |
| 3 | `textDocument/references` across included utility files produces duplicate results when the same file is included transitively through multiple paths. | Medium | Low | The graph is keyed by absolute path, so each file is visited at most once; T12 adds a round-trip test with a shared utility file. |
| 4 | Per-keystroke latency exceeds 200 ms when merging 20+ files. | Medium | Medium | T9 caches by content-hash and reuses warm per-file parse caches; T12/T13 profile `human_assist.xs` if possible. |
| 5 | Function-pointer type `void(int)` conflicts with `cast_expression` `(type) value` or `parenthesized_expression`. | Low | High | T1 adds the conflict `[function_pointer_type, type_specifier]`; T13 runs the full game-folder parse test to confirm no new parse errors. |

---

## 5. Verification commands

Run after the implementation is complete:

```bash
# Build the language server and tree-sitter parser
cargo build --manifest-path tools/xs-language-server/Cargo.toml

# Unit tests
cargo test --manifest-path tools/xs-language-server/Cargo.toml

# LSP roundtrip
cargo run --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test

# Game-folder integration (with the game installed)
AOMR_GAME_PATH=/path/to/Age\ of\ Mythology\ Retold \
  cargo test --manifest-path tools/xs-language-server/Cargo.toml \
    --test game_folder_parse -- --nocapture
```

Expected final state:
- All 75 existing LSP unit tests pass.
- All new unit tests for grammar, symbols, merged view, semantic, and typecheck pass.
- `lsp_roundtrip_test` passes.
- `game_folder_parse::semantic_pipeline_unresolved_count_within_threshold` reports < 1,000 unresolved symbols.
- Top-10 `bo_*` callees each drop ≥ 80% from baseline.
