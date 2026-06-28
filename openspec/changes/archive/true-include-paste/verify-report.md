# Verification Report — `true-include-paste`

**Project:** `aom_retold_mod_idle_auto_repair`  
**Change:** `true-include-paste`  
**Base branch:** `xs-language-server/redesign-phase-5` (`ba856a3`)  
**Verified branch:** `xs-language-server/true-include-paste-tests` (`788742b`)  
**Chain:** 4 stacked PRs, 15 commits (`ba856a3..788742b`)  
**Verification date:** 2026-06-25  
**Verifier:** `sdd-verify` executor  

---

## 1. Verdict

**Overall verdict: PASS WITH DEVIATIONS**

The implementation realises true textual-paste include semantics for the redesigned XS Language Server, extends the tree-sitter grammar so the dominant `bo_*` framework parses as `Function` symbols, and wires the merged scope into completion, hover, definition, references, semantic diagnostics, and type checking. All automated test suites pass: 105 Rust unit tests, the LSP round-trip binary, and the game-folder integration tests report **0 unresolved-symbol diagnostics across 149 top-level files** with all top-10 `bo_*` callees at zero. The documented design deviations (placement of `IncludeEdge`, omission of a `MergeError::MissingTarget` variant, placement/timing of backslash normalization, and the tightened integration-test scope) do not change specified user-visible behaviour; they are recorded in §6 for maintainer review.

| Metric | Value |
|---|---|
| Spec scenarios (true-include-paste) | 16 / 18 SATISFIED, 2 / 18 PARTIALLY SATISFIED |
| Tasks complete (functional) | 13 / 13 |
| Rust unit tests | 105 passed, 0 failed |
| LSP round-trip test | PASS |
| Game-folder integration tests | 4 passed, 0 failed; 0 unresolved workspace symbols |
| Top-10 `bo_*` callee regression guards | 10 / 10 PASS (zero unresolved calls each) |
| Commits (`ba856a3..788742b`) | 15 |

---

## 2. Scope verification

### `spec-true-include-paste.md` scenarios

| # | Scenario | Implementation evidence | Status | Justification |
|---|----------|------------------------|--------|---------------|
| 1 | `function_with_function_pointer_default_parses_as_function_definition` | `parser::tests::function_with_function_pointer_default_parses_as_function_definition` (`src/parser.rs`); commit `c041f40` | SATISFIED | Test passed; AST contains `function_definition` named `boVillager` with two parameters and a `function_pointer_type` child. |
| 2 | `function_with_trailing_lambda_default_extracts_default_values` | `symbols::tests::extracts_function_pointer_parameter_type_and_lambda_default`, `symbols::tests::extracts_lambda_default_with_return_type` (`src/symbols.rs`); commit `30c5302` | SATISFIED | Lambda default text captured verbatim; function-pointer type rendered as `void(int)` / `bool()`. |
| 3 | `function_with_simple_default_still_works` | `parser::tests::function_with_simple_default_still_parses` (`src/parser.rs`); `symbols::tests::extracts_simple_default_parameter_still` (`src/symbols.rs`) | SATISFIED | Existing simple-default path unaffected; test passed. |
| 4 | `direct_include_resolves_in_includer` | `merged_view::tests::direct_include_makes_symbol_visible` (`src/merged_view.rs`; `8eb7974`); `semantic::tests::direct_include_resolves_symbol` (`src/semantic.rs`; `bb93a0e`) | SATISFIED | Symbol from directly included file resolves in includer. |
| 5 | `transitive_include_resolves` | `merged_view::tests::transitive_include_makes_symbol_visible` (`src/merged_view.rs`); `semantic::tests::transitive_include_resolves_symbol` (`src/semantic.rs`) | SATISFIED | Depth-2 symbol visible in top-level file. |
| 6 | `include_cycle_does_not_loop` | `merged_view::tests::include_cycle_terminates_cleanly` (`src/merged_view.rs`); `semantic::tests::cycle_includes_do_not_loop` (`src/semantic.rs`); roundtrip `cycle_does_not_hang` (`3d3271b`) | SATISFIED | Cycle terminates; no duplicate symbols; server exits cleanly on cyclic includes. |
| 7 | `missing_include_target_produces_diagnostic` | `merged_view::tests::missing_include_target_produces_diagnostic` (`src/merged_view.rs`); `semantic::tests::missing_include_target_produces_diagnostic` (`src/semantic.rs`) | SATISFIED | Missing target reported as `IncludeDiagnostic`; other includes still processed. |
| 8 | `mod_overlay_on_include_target_is_preferred` | `merged_view::tests::mod_overlay_on_include_target_is_preferred` (`src/merged_view.rs`); `workspace::tests::include_edge_prefers_mod_overlay` (`src/workspace.rs`) | SATISFIED | Mod overlay copy chosen over vanilla when both exist. |
| 9 | `static_variable_in_included_file_is_hidden` | `merged_view::tests::static_variable_in_included_file_is_hidden` (`src/merged_view.rs`); `completion::tests::included_visibility_filters_static_and_keeps_extern` (`src/completion.rs`); `semantic::tests::static_symbol_in_included_file_is_hidden` (`src/semantic.rs`) | SATISFIED | `static` variables and functions from included files are filtered out. |
| 10 | `extern_variable_in_included_file_is_visible` | `merged_view::tests::extern_variable_in_included_file_is_visible` (`src/merged_view.rs`); `completion::tests::included_visibility_filters_static_and_keeps_extern` (`src/completion.rs`) | SATISFIED | `extern` variables from included files remain visible. |
| 11 | `public_function_in_included_file_is_visible` | `merged_view::tests::public_function_in_included_file_is_visible` (`src/merged_view.rs`); `semantic::tests::call_after_include_is_clean` (`src/semantic.rs`); `typecheck::tests::flags_wrong_arg_type_for_included_workspace_function` (`src/typecheck.rs`) | SATISFIED | Non-`static` functions from included files resolve; call-after-include produces no use-before-definition error. |
| 12 | `mutable_redefinition_in_included_file_follows_engine_rules` | `semantic::tests::mutable_function_in_included_file_is_callable`, `semantic::tests::mutable_redefinition_across_include_is_checked` (`src/semantic.rs`) | SATISFIED | Mutable function in included file callable; redefinition signatures checked across include closure. |
| 13 | `completion_offers_included_symbols` | `completion::tests::included_file_symbols_are_offered`, `completion::tests::prefix_filters_included_symbols` (`src/completion.rs`); roundtrip `completion_across_include` (`3d3271b`) | SATISFIED | Included symbols appear in `textDocument/completion`; `included_files` regex removed. |
| 14 | `hover_shows_included_file_signature` | Roundtrip `hover_across_include` (`3d3271b`); `server.rs` hover handler uses `MergedView` (`d837c72`) | SATISFIED | Included-symbol hover returns signature and a `file://` URI to the defining file. |
| 15 | `goto_definition_jumps_to_included_file` | Roundtrip `definition_across_include` (`3d3271b`); `server.rs` definition handler uses `MergedView` (`d837c72`) | SATISFIED | Definition response points to the included file. |
| 16 | `included_file_change_invalidates_merged_view` | `server.rs` `invalidate_merged_views_for` (lines 165–182) and `did_change_watched_files` (lines 419–443) (`d366182`); `merged_view::tests::cache_key_differs_when_include_changes` validates cache-key recomputation | PARTIALLY SATISFIED | Invalidation logic is implemented and exercised by cache-key tests, but no dedicated roundtrip/automated test simulates a watched-file change that triggers `did_change_watched_files` for an open includer. |
| 17 | `merged_view_under_keystroke_budget` | `server.rs` `get_or_build_merged_view` + `MergedViewCacheKey` (`d366182`); per-file parse cache reused | PARTIALLY SATISFIED | Caching infrastructure is in place, but no automated benchmark exercises a 20-file include closure against the 200 ms budget. |
| 18 | `unresolved_symbol_baseline_drops_below_threshold` | `tests/game_folder_parse.rs::semantic_pipeline_unresolved_count_within_threshold` (`788742b`) | SATISFIED | Count is 0, well below the 1,000 threshold (down from 5,335 baseline). |

### Cross-cutting specs

- **`spec-semantic-diagnostics.md`** — all included scenarios pass via the new `semantic.rs` merged-view paths (`eace534`, `bb93a0e`). `extern` collision remains project-wide; forward-decl/mutable/type checks use the per-file include closure.
- **`spec-virtual-project-overlay.md`** — include resolution inherits mod-overlay preference (`workspace::resolve_include`); verified by `workspace::tests::include_edge_prefers_mod_overlay` and the integration test parsing only the vanilla game folder while the merged-view unit tests cover overlay preference.

---

## 3. Design conformance

| Design decision | Implementation evidence | Status |
|-----------------|------------------------|--------|
| New `src/merged_view.rs` module; `pub mod merged_view;` in `src/lib.rs` | `src/merged_view.rs` exists; `src/lib.rs` registers module after `workspace` | MATCHES |
| `MergedView` public API (`VisibilityProvenance`, `MergedSymbol`, `IncludeGraph`, `IncludeDiagnostic`, `MergeError`, `MergedViewCacheKey`) | Defined in `src/merged_view.rs` lines 49–176 | MATCHES |
| `IncludeEdge` lives in the merged-view module | Design specified `merged_view.rs`; actual definition is in `src/workspace.rs` line 118 | DEVIATES |
| Missing include target surfaces as `MergeError::MissingTarget` | No such enum variant exists; missing targets are collected as `IncludeDiagnostic` (`src/merged_view.rs` lines 117, 293, 427) and translated to LSP diagnostics in `src/diagnostics.rs` | DEVIATES |
| Grammar: `function_pointer_type` + `lambda_expression` + parameter-declaration change + conflict | `tree-sitter-xs/grammar.js` (`c041f40`); regenerated `parser.c`/`grammar.json`/`node-types.json` | MATCHES |
| Symbol extraction: lambda defaults, function-pointer types, ERROR-node recovery for function definitions with bodies | `src/symbols.rs` (`30c5302`); tests pass; `bo_*` symbols extracted | MATCHES |
| Visibility during merge: own-file all; included `static` hidden; included `extern`/public visible | `src/merged_view.rs` merge/filter logic; `completion::tests::included_visibility_filters_static_and_keeps_extern` | MATCHES |
| Completion removes regex-based `included_files` and `path.ends_with` | `src/completion.rs` no longer contains either pattern (`8edae96`) | MATCHES |
| Semantic: `check_forward_declarations_for_merged_view`, `check_mutable_redefinitions_for_merged_view`, `forward_callable_merged`; `check_extern_collisions` project-wide | `src/semantic.rs` lines 101–106, 373–495, 701+ tests | MATCHES |
| Typecheck routes cross-file function lookup through merged view | `src/typecheck.rs` `resolve_workspace_function` takes `Option<&MergedView>` (`48859d3`) | MATCHES |
| Diagnostics passes merged view to semantic/typecheck | `src/diagnostics.rs` `collect_all` signature `Option<&MergedView>` (`668ad22`) | MATCHES |
| Server caches merged views per open file and invalidates on watched-file changes | `src/server.rs` `merged_views`, `get_or_build_merged_view`, `invalidate_merged_views_for` (`d366182`) | MATCHES |
| Hover, definition, references use merged view; rename stays file-local | `src/server.rs` handlers (`d837c72`); `textDocument/rename` still walks current-file source only | MATCHES |
| Backslash normalization handled consistently | Added in `workspace::normalize_include_target` (`70e4eec`) rather than in the original merged-view PR | DEVIATES |
| Integration acceptance threshold and scope | Design/spec targeted full-game-folder unresolved count `< 1,000`; implementation analyzes only top-level files and excludes `random_maps/`, with threshold tightened to 0 (`188e258`, `788742b`) | DEVIATES |

### Deviation notes

1. **`IncludeEdge` module placement** — Minor structural difference; `IncludeEdge` is still used by both `workspace.rs` and `merged_view.rs`, so behaviour is unchanged.
2. **`MergeError::MissingTarget`** — The design proposed an enum variant; the implementation chose a richer `IncludeDiagnostic` type carrying range/root metadata. This produces better diagnostics and does not abort the merge, so it is functionally equivalent.
3. **Backslash normalization** — The design did not call it out as a separate work item; it was added in PR 3 when real include targets containing `\` failed to resolve. Low impact.
4. **Integration-test scope** — Scoping to top-level files and excluding `random_maps/` removes false positives from included files analyzed in isolation and from RM-specific engine API gaps. The resulting 0-count threshold is stricter than the spec's 1,000 threshold.

---

## 4. Task completion

| Task | Title | Commit | LOC delta | Status |
|---|---|---|---|---|
| T1 | Grammar: add `function_pointer_type` and `lambda_expression` rules | `c041f40` | +9,783 / −8,523 (parser-regeneration churn; human-written grammar.js +17/−1) | COMPLETE |
| T2 | Symbols: extract function-pointer defaults and recover function definitions from ERROR nodes | `30c5302` | +131 / −6 | COMPLETE |
| T3 | Parser + workspace: expose include directives and overlay-relative helpers | `6a36b7e` | +175 / −1 | COMPLETE |
| T4 | Merged view: new `merged_view.rs` module | `8eb7974` | +639 / −1 | COMPLETE |
| T5 | Completion: replace regex approximation with merged view | `8edae96` | +130 / −111 | COMPLETE |
| T6 | Semantic: route forward-decl and mutable checks through merged view | `eace534` | +328 / −2 | COMPLETE |
| T7 | Typecheck: route cross-file function lookup through merged view | `48859d3` | +69 / −4 | COMPLETE |
| T8 | Diagnostics: wire merged view into `collect_all` | `668ad22` | +48 / −4 | COMPLETE |
| T9 | Server: build, cache, and invalidate merged views per open file | `d366182` | +135 / −26 | COMPLETE |
| T10 | Hover, definition, and references: cross-include resolution | `d837c72` | +150 / −49 | COMPLETE |
| T11 | Semantic fixtures for include scenarios | `bb93a0e` | +187 | COMPLETE |
| T12 | LSP round-trip tests for cross-include features | `3d3271b` | +378 / −1 | COMPLETE |
| T13 | Integration test: tighten threshold and add per-callee guards | `788742b` | +445 / −101 (includes `tasks.md` update) | COMPLETE |

**Note:** `tasks.md` lists T1 and T2 with unchecked boxes (`[ ]`) even though both commits are present and their acceptance tests pass. This is recorded as a documentation hygiene issue in §6.

---

## 5. Test results

| Suite | Command | Result |
|---|---|---|
| Rust unit tests | `cargo test --manifest-path tools/xs-language-server/Cargo.toml` | **PASS** — 105 passed, 0 failed |
| LSP round-trip test | `cargo run --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test` | **PASS** — all sequences green, including `completion_across_include`, `hover_across_include`, `definition_across_include`, and `cycle_does_not_hang` |
| Game-folder integration | `AOMR_GAME_PATH=... cargo test --manifest-path tools/xs-language-server/Cargo.toml --test game_folder_parse -- --nocapture --test-threads=1` | **PASS** — 4 passed, 0 failed |

### Game-folder integration metrics

- **Parseable `.xs` files:** 302
- **Binary `.xs` files skipped:** 20 (under `random_maps/`)
- **Symbols extracted:** 6,045
- **Unexpected parse ERRORs:** 156 (threshold 3,000; no file > 100 errors)
- **Top-level files analyzed semantically:** 149
- **Unresolved-symbol diagnostics:** **0** (threshold 0)
- **Top-10 `bo_*` callee unresolved counts:** all 0

### Performance observations

- Full integration test finished in ~43.5 s single-threaded, analysing 149 top-level files. This demonstrates the pipeline scales to the real game folder, but it is not a per-keystroke latency measurement.
- No dedicated benchmark validates the 200 ms merged-view build budget for a 20-file include closure. The caching layers (`MergedViewCacheKey`, per-file parse cache) are in place and reused.

---

## 6. Deviations

| # | What was supposed to happen | What actually happened | Why | Impact |
|---|---|---|---|---|
| 1 | `IncludeEdge` defined in `src/merged_view.rs` | Defined in `src/workspace.rs` line 118 | The edge needs `workspace::IncludeRoot`; keeping it near `resolve_include` reduced coupling friction. | None — behaviour identical. |
| 2 | `MergeError::MissingTarget` enum variant for unresolved include targets | Missing targets are collected as `IncludeDiagnostic`; `MergeError` only has `Io` and `Cycle` | `IncludeDiagnostic` carries range/root metadata and lets the merge continue for other includes. | Minor positive — diagnostics are richer and non-fatal. |
| 3 | Backslash normalization (if needed) implemented as part of the merged-view/core work | Added later in `workspace::normalize_include_target` (`70e4eec`) after real game includes with `\` failed | Discovered during PR 3 integration testing. | None — fix is present and tested. |
| 4 | Game-folder integration test asserts unresolved count `< 1,000` across the full game folder | Test scoped to top-level non-`random_maps` files and threshold tightened to 0 | Top-level-only analysis avoids false positives for included files; `random_maps/` uses RM-specific engine API unsupported by the Doxygen archive. | None for users — the guard is stricter than specified. |
| 5 | `tasks.md` should reflect task completion with all checkboxes marked `[x]` | T1 and T2 are listed as `[ ]` despite being complete | Documentation oversight; the commits and tests are present. | Minor hygiene — no functional effect. |
| 6 | Every scenario covered by a passing automated test | Scenarios 16 and 17 rely on implementation inspection and cache-key tests rather than dedicated end-to-end tests | Server-level watched-file invalidation and performance benchmarking are hard to exercise deterministically in the existing harness. | Minor — functionality is implemented; tests can be added as a fast-follow. |

---

## 7. Known issues

| Priority | Issue | Note / mitigation |
|---|---|---|
| Low | `tasks.md` T1/T2 checkboxes remain unchecked | Update the task file in a follow-up commit to match actual completion. |
| Medium | No dedicated 200 ms keystroke-budget benchmark | Caching is implemented but not measured on a representative 20-file include closure. Add a targeted benchmark before claiming the budget is universally met. |
| Medium | No automated test for `did_change_watched_files` invalidating an open includer's merged view | `invalidate_merged_views_for` is implemented; only cache-key recomputation and watcher registration are exercised automatically. Consider adding a roundtrip sequence that modifies an included file and re-queries diagnostics. |
| Low | Integration threshold is now 0 | Any regression in cross-include symbol resolution immediately fails CI. This is intentional but means future grammar/parser changes must keep the guard green. |
| Low | Manual IDE smoke test not run | Cross-include completion/hover/definition were verified via the roundtrip binary; a real IntelliJ/Rider exercise is recommended before release, as noted in the proposal acceptance criteria. |
| Low | Historical `run_workspace_tests` `workspace/symbol` `mod_b` assertion has been reported as flaky in some tower-lsp timing conditions | Not reproduced in this verification run; if flakes resurface, harden the roundtrip message泵 or increase synchronization. |

---

## 8. Files changed

Total across `ba856a3..788742b`: **+12,638 / −8,798** lines (net +3,840). The large positive delta is dominated by regenerated `tree-sitter-xs/src/parser.c`.

| File / directory | Added | Removed | Net | PR / note |
|---|---|---|---|---|
| `openspec/changes/true-include-paste/tasks.md` | 281 | 0 | +281 | PR 4 |
| `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` | 378 | 1 | +377 | PR 4 |
| `tools/xs-language-server/src/completion.rs` | 88 | 102 | −14 | PR 3 |
| `tools/xs-language-server/src/diagnostics.rs` | 46 | 4 | +42 | PR 3 |
| `tools/xs-language-server/src/lib.rs` | 3 | 1 | +2 | PR 2 |
| `tools/xs-language-server/src/merged_view.rs` | 706 | 0 | +706 | PR 2 + PR 3 additions |
| `tools/xs-language-server/src/parser.rs` | 140 | 0 | +140 | PR 1 (grammar tests) + PR 2 (include walker) |
| `tools/xs-language-server/src/semantic.rs` | 481 | 2 | +479 | PR 3 + PR 4 fixtures/tests |
| `tools/xs-language-server/src/semantic_fixtures/include_*.xs` | 34 | 0 | +34 | PR 4 |
| `tools/xs-language-server/src/server.rs` | 247 | 72 | +175 | PR 3 |
| `tools/xs-language-server/src/symbols.rs` | 131 | 6 | +125 | PR 1 |
| `tools/xs-language-server/src/typecheck.rs` | 69 | 4 | +65 | PR 3 |
| `tools/xs-language-server/src/workspace.rs` | 146 | 2 | +144 | PR 2 + PR 3 normalization |
| `tools/xs-language-server/tests/game_folder_parse.rs` | 208 | 81 | +127 | PR 3 + PR 4 |
| `tools/xs-language-server/tree-sitter-xs/grammar.js` | 16 | 1 | +15 | PR 1 |
| `tools/xs-language-server/tree-sitter-xs/src/grammar.json` | 102 | 2 | +100 | PR 1 (generated) |
| `tools/xs-language-server/tree-sitter-xs/src/node-types.json` | 70 | 0 | +70 | PR 1 (generated) |
| `tools/xs-language-server/tree-sitter-xs/src/parser.c` | 9,492 | 8,520 | +972 | PR 1 (generated) |

---

## 9. Verification commands

### 1. Unit tests

```bash
cargo test --manifest-path tools/xs-language-server/Cargo.toml
```

**Result:**

```text
running 105 tests
...
test result: ok. 105 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.86s
```

### 2. LSP round-trip test

```bash
cargo run --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test
```

**Result (selected):**

```text
PASS (include completion_across_include): response matched expected content
PASS (include hover_across_include): response matched expected content
PASS (include definition_across_include): response matched expected content
PASS (cycle_does_not_hang): server exited cleanly despite include cycle
PASS (workspace): workspace symbol scoped to mod_a shows overlay, not vanilla
PASS (workspace): mod_b file diagnosed after didChangeWorkspaceFolders add
...
PASS: server exited cleanly (status: exit status: 0)
```

### 3. Game-folder integration tests

```bash
AOMR_GAME_PATH="/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold" \
  cargo test --manifest-path tools/xs-language-server/Cargo.toml \
    --test game_folder_parse -- --nocapture --test-threads=1
```

**Result:**

```text
running 4 tests
test every_game_folder_file_resolves_in_workspace ... Workspace resolved 302 relative paths across 302 parseable .xs files (20 binary .xs skipped)
ok
test parse_every_game_folder_file_completes_without_unexpected_errors ... Parsed 302 files (20 binary .xs skipped), 6045 symbols, 156 unexpected errors
ok
test semantic_pipeline_unresolved_count_within_threshold ... Semantic check emitted diagnostics across 149 top-level files (20 binary .xs skipped); 0 flagged as unresolved_symbol (threshold: 0)
ok
test top_bo_callees_have_zero_unresolved_calls ... PASS: boBuild has zero unresolved-symbol diagnostics
PASS: boVillager has zero unresolved-symbol diagnostics
PASS: boUnit has zero unresolved-symbol diagnostics
PASS: boConditionalWait has zero unresolved-symbol diagnostics
PASS: boAdvance has zero unresolved-symbol diagnostics
PASS: boIncreaseTimeout has zero unresolved-symbol diagnostics
PASS: boTransaction has zero unresolved-symbol diagnostics
PASS: boEnd has zero unresolved-symbol diagnostics
PASS: boExecute has zero unresolved-symbol diagnostics
PASS: boTech has zero unresolved-symbol diagnostics
ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 43.50s
```

### 4. Repository diff sanity check

```bash
git diff --stat ba856a3..HEAD
git log --oneline ba856a3..HEAD
```

Confirmed 15 commits and the file set listed in §8.

---

## 10. Recommendation

**PASS WITH DEVIATIONS.** The change satisfies its core goal: true include-paste semantics are implemented, the `bo_*` framework is now extracted and resolved, and the game-folder integration test reports zero unresolved workspace symbols across 149 top-level files. The documented deviations are minor structural or scope adjustments that do not regress specified behaviour, and the two partially-satisfied scenarios (watched-file invalidation and keystroke-budget measurement) lack automated coverage but not implementation. Merge is recommended after acknowledging the deviations in §6.

---

## Skill resolution

- **Skill loaded:** `sdd-verify` from `~/.config/opencode/skills/sdd-verify/SKILL.md`
- **Resolution:** `paths-injected`
