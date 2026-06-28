# Proposal — `true-include-paste`

**Project:** `aom_retold_mod_idle_auto_repair`  
**Change name:** `true-include-paste`  
**Base branch:** `xs-language-server/redesign-phase-5` (`ba856a3`)  
**Target branch:** `xs-language-server/true-include-paste`

---

## 1. Summary

Build real textual-paste semantics for Age of Mythology: Retold’s `include "..."` directive in the redesigned Rust XS Language Server, and fix the tree-sitter grammar / symbol extraction so that the dominant `bo_*` framework (`boVillager`, `boBuild`, `boUnit`, `boTransaction`, etc.) is recognized as `Function` symbols. The result is a per-file merged scope that feeds completion, hover, go-to-definition, references, semantic diagnostics, and type checking, replacing the current line-regex approximation and the “defined anywhere in the project” fallback. The realistic target is to drop the game-folder unresolved-symbol baseline from **5,335 to under ~500**, with most of the residual coming from genuinely unsupported grammar features such as `class` member declarations.

---

## 2. Problem

The LSP still treats `include "..."` as a heuristic rather than the engine’s textual paste. `completion.rs:131-149` scans `source.lines()` for lines starting with `include`, extracts the first quoted substring, and `completion.rs:111` matches targets with `path.ends_with(target)`. This is wrong in five ways: it misses multi-line includes, comments, and string literals; it ignores `workspace.rs:189-198` `resolve_include`, which understands the `AI`/`TRIGGER`/`RANDOM_MAP` include-root context (`workspace.rs:55-74`) and mod overlay; it does not follow transitive includes; it exposes every included-file symbol regardless of `static`/`extern` (`completion.rs:113` tests `is_current || is_included || sym.visibility == Visibility::Extern`); and the merged view is never passed to semantic analysis, so `semantic.rs:187-257` `check_forward_declarations` only sees the current file or the blanket `forward_callable` fallback (`semantic.rs:346-388`) instead of the actual include closure.

A second, blocking gap is in the parser. Functions in `game/ai/core/bo_system/bo_system.xs` take function-pointer default arguments such as `void(int) afterQueue = [](int id = -1) {}` (`bo_system.xs:12`). The current tree-sitter XS grammar has no `lambda_expression` rule and no function-pointer type in `parameter_declaration` (`grammar.js:345-356`), so these definitions surface as `declaration` nodes containing `ERROR` rather than as `function_definition`. Consequently `boBuild`, `boVillager`, `boUnit`, and `boTransaction` — the dominant callees — are not extracted into the `SymbolTable` at all. Merging symbol tables cannot expose symbols that were never extracted, which is why the 5,335 unresolved-symbol baseline will not move without grammar/symbol work.

---

## 3. Goal

- Calling a function defined in an included file no longer produces `Error 0310: invalid symbol lookup` in the includer, provided the include resolves.
- `boVillager`, `boBuild`, `boUnit`, `boTransaction`, `boConditionalWait`, `boAdvance`, `boTech`, and the rest of the dominant `bo_*` framework parse as `Function` symbols.
- `textDocument/completion`, `textDocument/hover`, `textDocument/definition`, and `textDocument/references` resolve symbols across include boundaries.
- Transitive includes are followed; a function defined in `a.xs`, included by `b.xs`, included by `c.xs`, is visible in `c.xs`.
- Cyclic includes terminate cleanly and expose symbols up to the first re-entry.
- `static`/file-local symbols from an included file are hidden in the includer; `extern` variables and public functions are visible.
- A missing include target produces a clear diagnostic rather than a silent empty merge.
- Mod overlay applies to include targets automatically (`workspace.rs:443-461`).

---

## 4. Non-goals

- Resolving the engine’s `class` member declaration grammar (deferred to a separate change).
- Resolving preprocessor `#if`/`#define` directives (already a known limitation; deferred).
- Cross-include `textDocument/rename`; this change enables `references` across includes but intentionally leaves rename file-local.
- Cross-mod includes (a file in one mod including an overlay file that only exists in another mod).
- Reaching zero unresolved symbols; the remaining ~500 residual is expected from unsupported grammar.

---

## 5. Approach

### 5.1 Grammar extension for function-pointer defaults

Extend `tree-sitter-xs/grammar.js` so a parameter can declare a function-pointer type and a default lambda:

- Add a `lambda_expression` rule matching `[]`, an optional parameter list, and a compound statement (and, for XS, the trailing-return-type form `[]() -> bool { ... }` seen in `bo_system.xs:164`).
- Add `lambda_expression` to the `expression` choices (`grammar.js:483-505`) so it can appear as the `default` value in `parameter_declaration` (`grammar.js:351-356`).
- Accept a function-pointer parameter type of the form `void(int)` (a type specifier followed by a parenthesized parameter-type list) in `parameter_declaration` without consuming the parameter name as part of the type.

After editing `grammar.js`, regenerate the parser with `tree-sitter generate` inside `tools/xs-language-server/tree-sitter-xs/`. The generated `src/` files are checked in. A unit test in `tree-sitter-xs/` (or in `symbols.rs` tests) will assert that `bo_system.xs:12` parses as a `function_definition` whose name is `boVillager`.

### 5.2 Symbol extraction update

Update `tools/xs-language-server/src/symbols.rs` to harvest the newly parseable `bo_*` functions:

- In `extract_params`, recognize a `lambda_expression` child as a default value and store its raw source text (the existing `extract_param_default` already captures any post-`=` child, so it should work once the grammar produces a single `lambda_expression` node).
- Extend the ERROR-node recovery path. Today `extract_error_forward_declaration` (`symbols.rs:333-378`) bails when it sees a `compound_statement` child, because it is designed for header-only signatures. Add a companion `extract_error_function_definition` that recovers a function symbol from an `ERROR` node that contains a primitive type, identifier, parameter list, and a body. This catches the current `bo_*` parse failures immediately while the grammar change is being validated.
- Ensure modifiers still work: `is_extern` / `is_mutable` flags (`symbols.rs:97-100`) and `function_visibility` (`symbols.rs:410-418`) must handle any new storage-class positions introduced by the grammar.

### 5.3 Merged-view include-paste plumbing

Introduce a new `MergedView` (exact module name left to the spec, likely a new `src/merged_view.rs` next to `workspace.rs`) that represents the translated scope of one file at analysis time.

For an open file `F`:

1. Parse `F` and collect `include_directive` nodes (`grammar.js:112-116`).
2. For each include, strip quotes from the `path` field and call `workspace::resolve_include` (`workspace.rs:189-198`), which already applies `IncludeRoot` context (`workspace.rs:55-74`) and mod overlay.
3. Recursively resolve included files, using `cache::load_or_parse_symbols` for each file and a `HashSet<PathBuf>` visited set to break cycles.
4. Merge symbol tables in scope order: `F` first, then direct includes in file order, then transitive includes. During the merge, keep symbol provenance (`OwnFile`, `DirectInclude { depth }`) and filter visibility:
   - Current-file symbols: all.
   - Included-file symbols: hide `static`/file-local variables; expose `extern` variables and functions (`Extern`/`Public`).
5. Return the merged table plus a list of missing-include diagnostics.

Wire `MergedView` into consumers:

- `completion.rs`: delete `included_files` (`completion.rs:131-149`) and the `path.ends_with(target)` check (`completion.rs:111`); call the merged view.
- `semantic.rs`: `check_forward_declarations` and `forward_callable` resolve callees against the merged view of the current file instead of the global `VirtualProject`.
- `typecheck.rs`: route cross-file function lookups through the merged view.
- `diagnostics.rs`: build the merged view in `collect_all` and pass it to semantic / typecheck.
- `server.rs`: build the merged view in `publish_diagnostics` and keep it in server state for non-mutating handlers (`completion`, `hover`, `definition`, `references`).
- `cache.rs`: reuse the existing per-file `SymbolTable` cache; add an optional memoized merged-view cache keyed by `(sha256(current_file_content), [(resolved_include_path, sha256(include_content))])` for per-keystroke latency.

---

## 6. Open questions for the spec phase

1. **Should grammar/symbol work be in this change or split out?**
   - User decided: include in this change.
   - *Justification:* Without it, the 5,335 baseline cannot drop meaningfully, because the dominant `bo_*` callees are not currently extracted as symbols.

2. **Should `extern` collision and `mutable` redefinition rules operate on the full project or on the per-file include closure?**
   - Recommendation: `extern` collision stays project-wide; `mutable` redefinition operates on the merged include closure (the effective translation unit).
   - *Justification:* `extern` enforces single-definition linking across files that the engine loads together, while `mutable` redefinition is a within-source-unit rule; included files are pasted, so the include closure is the correct unit.

3. **Where does the merged view live?**
   - Recommendation: a new `src/merged_view.rs` module that wraps `workspace::resolve_include` and the per-file caches.
   - *Justification:* It isolates include-graph construction, cycle detection, and merge arithmetic from `server.rs` and avoids bloating `workspace.rs`, which is already responsible for overlay resolution.

4. **What is the cache invalidation key and lifetime?**
   - Recommendation: memoize merged views in server state with a key of `(sha256(current_file_text), sorted [(abs_path, sha256(include_text))])`, invalidated whenever `didChange`/`didChangeWatchedFiles` touches any file in the closure.
   - *Justification:* A merged view depends only on the current file and the resolved include closure; reusing the existing per-file parse cache gives cheap hashes without new disk files.

5. **Should `references`/`rename` cross include boundaries?**
   - Recommendation: enable `references` across the include closure; keep `rename` file-local for now.
   - *Justification:* Finding uses in pasted included files matches engine semantics, but rename would edit shared include targets used by many includers and needs a separate design.

6. **How should `static`/`extern` visibility be represented during a merge?**
   - Recommendation: keep the existing `Visibility` enum unchanged; wrap merged symbols with provenance metadata (`origin: OwnFile | Included { depth, path }`) and filter during merge.
   - *Justification:* This avoids a new `Visibility::ExportedViaInclude` variant propagating through every consumer and makes the visibility rule explicit at merge time.

7. **What is the engine behavior for `mutable extern` functions?**
   - Recommendation: treat `mutable` and `extern` as orthogonal — `extern` controls cross-file visibility, `mutable` controls forward-callability and redefinability.
   - *Justification:* `docs/xs-language-syntax.md:152-154` documents `extern` for variables and `mutable` for functions independently; no spec or sample shows a combined modifier, so the spec should add an engine-verification fixture before final acceptance.

---

## 7. Risks

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| 1 | The grammar fix does not cover every lambda variant in `bo_system.xs` (e.g., `[]() -> bool { ... }` at `bo_system.xs:164`). | Medium | High | Add ERROR-node recovery for function definitions with bodies as a safety net; test the full `bo_system.xs` after regeneration. |
| 2 | Transitive-include cache invalidation misses a dependent open file when a deep include changes. | Medium | High | Build the include graph explicitly in `MergedView` and use it in `server.rs::did_change_watched_files` to re-diagnose all includers. |
| 3 | Per-keystroke latency exceeds the 200 ms budget while merging many included files. | Medium | Medium | Memoize merged views by content hash; profile on `human_assist.xs` before final acceptance. |
| 4 | Cycle across mod overlay + vanilla files is not detected because the visited set uses absolute paths. | Low | Medium | Unit-test cycles that span overlay and vanilla; ensure visited set keys on resolved absolute `PathBuf`. |
| 5 | `references` across includes surfaces unexpected duplicate results for symbols defined in widely-included utility files. | Medium | Low | Gate the feature behind the include closure and add a round-trip test with a shared utility file. |

**Rollback plan:** The change is additive on top of the existing approximation. If regressions appear, revert the branch commits on `xs-language-server/true-include-paste`; the previous per-file regex + global fallback behavior is preserved in `redesign-phase-5`.

---

## 8. Success criteria

- [ ] All existing 75 LSP unit tests pass (`cargo test` in `tools/xs-language-server/`).
- [ ] All 3 game-folder integration tests pass when `AOMR_GAME_PATH` is set.
- [ ] `semantic_pipeline_unresolved_count_within_threshold` reports an unresolved count below 1,000 (down from 5,335), trending toward the stretch target of ~500.
- [ ] Each of the top-10 unresolved callees by raw call-site count (`boBuild`, `boVillager`, `boUnit`, `boConditionalWait`, `boAdvance`, `boIncreaseTimeout`, `boTransaction`, `boEnd`, `boExecute`, `boTech`) drops by at least 80%.
- [ ] Grammar unit test: `void boVillager(int a = -1, void(int) afterQueue = [](int id = -1) {}) { }` parses as a `function_definition` named `boVillager`.
- [ ] New unit tests cover: single direct include, transitive include, include cycle, missing include target, mod overlay on include target, `AI`/`TRIGGER`/`RANDOM_MAP` include roots, `static` hidden, `extern`/public visible.
- [ ] LSP round-trip tests cover: completion across include, hover across include, definition across include, forward-decl diagnostic suppressed for include-defined function, missing include diagnostic emitted.
- [ ] The line-based `included_files` regex in `completion.rs:131-149` and the `path.ends_with(target)` check in `completion.rs:111` are removed.
- [ ] Manual smoke test in IntelliJ/Rider on `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` confirms cross-include completion, hover, and go-to-definition.

---

## 9. Out-of-scope follow-ups

- `class` member declarations and class-scoped symbol extraction.
- Preprocessor `#if`/`#define`/`#ifdef` resolution.
- Cross-mod includes and multi-mod symbol sharing.
- Workspace-wide `textDocument/rename` across include boundaries.
- Driving the unresolved-symbol count to exactly zero before the grammar gaps above are fixed.

---

## 10. Estimated effort

| Area | New / modified LOC | Notes |
|---|---|---|
| `tree-sitter-xs/grammar.js` + regeneration | ~40 / generated | Add `lambda_expression`, function-pointer parameter type, regenerate parser. |
| `src/symbols.rs` | ~80 / ~40 | Lambda default handling, ERROR recovery for function definitions with bodies. |
| `src/merged_view.rs` (new) | ~220 / — | Include graph, cycle detection, merge rules, provenance. |
| `src/workspace.rs` | ~30 / ~20 | Expose helpers for include-root / resolve_include reuse. |
| `src/completion.rs` | ~20 / ~60 | Remove regex + `path.ends_with`; consume merged view. |
| `src/semantic.rs` | ~60 / ~80 | `forward_callable` and `check_forward_declarations` use merged view. |
| `src/typecheck.rs` | ~30 / ~30 | Cross-file function lookup via merged view. |
| `src/diagnostics.rs` | ~20 / ~30 | Build and pass merged view. |
| `src/server.rs` | ~80 / ~60 | Keep merged views in state, invalidate on watched-file changes. |
| `src/cache.rs` | ~40 / ~10 | Merged-view memoization key logic. |
| `src/bin/lsp_roundtrip_test.rs` + fixtures | ~100 / ~20 | Include scenarios. |
| `tests/game_folder_parse.rs` | ~40 / ~10 | Threshold tightening and per-callee regression guard. |
| **Total** | **~760 new / ~360 modified** | **~1,100 LOC overall.** |

The change is likely a single PR of ~1,100 lines, but if the grammar work grows (e.g., full `class` spillover), it should be chain-split: (1) grammar + symbol extraction, (2) merged-view plumbing and tests. The tasks phase will decide.
