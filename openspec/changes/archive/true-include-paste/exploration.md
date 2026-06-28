# Exploration — `true-include-paste`

**Change:** `true-include-paste`  
**Branch target:** `xs-language-server/true-include-paste`  
**Exploration date:** 2026-06-24  
**Base:** `xs-language-server/redesign-phase-5` (`ba856a3`)  

---

## 1. Problem verification

### Restatement

The redesigned LSP still treats Age of Mythology: Retold’s `include "..."` directive as a heuristic rather than the textual paste the engine performs. `completion.rs` extracts include targets with a line-by-line regex (`included_files`, lines 131-149) and matches them against file paths with `path.ends_with(target)` (line 111). The semantic pass, meanwhile, analyses each file in isolation: `semantic.rs::check_forward_declarations` (lines 187-257) only consults the current file’s own symbol table and a blanket "defined anywhere in the virtual project" fallback (`forward_callable`, lines 346-388). This means symbols that should be visible only because an included file defines them are either exposed too broadly (all included-file symbols regardless of `static`/`extern`) or not resolved at all (transitive includes, exact include-path semantics, and per-include scoping).

### Confirming the 5 approximation issues

| # | Claim in blocker doc | Verdict | Evidence |
|---|---|---|---|
| 1 | Include extraction is a per-line regex that misses multi-line includes, comments, and string literals. | **Confirmed.** | `completion.rs:131-149` scans `source.lines()`, trims, checks `line.starts_with("include")`, then extracts the first quoted substring. It never uses the AST. |
| 2 | Included files are matched by `path.ends_with(target)` instead of `workspace.rs::resolve_include`. | **Confirmed.** | `completion.rs:111` tests `path.ends_with(target)`; the proper resolver is `workspace.rs:189-198`, which understands `IncludeRoot` (`Ai`, `Trigger`, `RandomMap`, lines 55-74) and mod overlay. |
| 3 | Transitive includes are not followed. | **Confirmed.** | `included_files` only looks at the current file’s text. There is no recursion into files included by an included file. |
| 4 | All included-file symbols are exposed, ignoring `extern`/`static`. | **Confirmed for completion.** | `completion.rs:113` includes a symbol if `is_current || is_included || sym.visibility == Visibility::Extern`, so file-local (`Local`) and `static` symbols from included files leak into the includer. |
| 5 | The merged view is not fed into semantic analysis. | **Confirmed.** | `semantic.rs::forward_callable` (lines 346-388) uses either same-file definitions or *any* function definition elsewhere in the project; it does not restrict lookups to the include closure. `diagnostics.rs:38-52` passes the whole `VirtualProject` to semantic/typecheck, not a per-file include closure. |

### Quantified impact

- **Baseline unresolved-symbol count:** 5,335 unresolved workspace symbols across 302 parseable game files. This is measured by `tests/game_folder_parse.rs::semantic_pipeline_unresolved_count_within_threshold` (threshold set at 5,870 in lines 337-341). The test printed:
  ```
  5335 flagged as unresolved_symbol, 37332 attributed to engine API calls (threshold: 5870)
  ```
- **Dominant callees (raw call-site counts across `game/**/*.xs`):**
  | Callee | Raw call sites |
  |---|---|
  | `boBuild` | 356 |
  | `boVillager` | 279 |
  | `boUnit` | 279 |
  | `boConditionalWait` | 141 |
  | `boAdvance` | 56 |
  | `boIncreaseTimeout` | 51 |
  | `boTransaction` | 50 |
  | `boEnd` | 42 |
  | `boExecute` | 30 |
  | `boTech` | 28 |

  These counts come from counting all occurrences of `name(` for functions defined in `game/ai/core/bo_system/bo_system.xs`.

- **Important caveat:** not every occurrence is unresolved. The current semantic pass already considers any function defined *anywhere* in the project as callable, so if `boVillager` were correctly extracted from `bo_system.xs` it would already be resolved in other files. The fact that 5,335 calls remain unresolved indicates that many of these dominant callees are **not currently being extracted as `Function` symbols**, most likely because the tree-sitter grammar cannot parse their `void(int) callback = [](...)` default arguments. This is surfaced below as a critical risk.

---

## 2. Scope of change

### Expected edits per file

| File | Change |
|---|---|
| `tools/xs-language-server/src/workspace.rs` | Add an include-graph / merged-view constructor that calls `resolve_include` (lines 189-198), uses the existing per-file `SymbolTable` cache, and walks transitive includes. |
| `tools/xs-language-server/src/completion.rs` | Replace `included_files` (lines 131-149) and the `path.ends_with` check (line 111) with a call into the merged view. |
| `tools/xs-language-server/src/semantic.rs` | Update `check_forward_declarations` (lines 187-257) and `forward_callable` (lines 346-388) to resolve calls against the merged view instead of the global project. |
| `tools/xs-language-server/src/typecheck.rs` | Route `resolve_workspace_function` (lines 168-179) through the merged view so included function signatures are available. |
| `tools/xs-language-server/src/diagnostics.rs` | Wire the merged view into `collect_all` (lines 38-52) so `semantic` and `typecheck` receive it. |
| `tools/xs-language-server/src/server.rs` | Build the merged view in `publish_diagnostics` (lines 728-777) and handlers (`completion` lines 375-396, `hover` lines 398-437, `goto_definition` lines 439-492, `references` lines 563-605). Invalidate/clear it on watched-file changes (lines 328-373). |
| `tools/xs-language-server/src/cache.rs` | Possibly extend caching: the merged view is currently not cached, and the per-file parse cache only stores `SymbolTable` (lines 95-106). A merged-view cache keyed by file path + included-file hashes may be needed for latency. |
| `tools/xs-language-server/src/symbols.rs` | Add `Visibility::ExportedViaInclude` (blocker doc proposal) or otherwise distinguish symbols that are file-local in the included file but visible in the includer; ensure `static`/`extern` semantics are preserved during merge. |
| `tools/xs-language-server/src/references.rs` | Optionally expand `find_identifier_uses` to search included file sources when resolving references/rename. |
| `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` | Add merged-view test sequences: include that resolves, include that does not exist, transitive include, mod overlay on include target, include cycle. |
| `tools/xs-language-server/tests/game_folder_parse.rs` | Tighten `UNRESOLVED_SYMBOL_THRESHOLD` (line 341) and add per-callee regression guard. |

### New modules vs. edits

- **New module likely needed:** `merged_view.rs` (or new types inside `workspace.rs`) containing the include graph builder, cycle detector, and the merged symbol table. Keeping it adjacent to `workspace.rs` is natural because `resolve_include` already lives there.
- **Edits only:** parser, grammar, word extraction, engine API, cache key logic.

### New public API surface

- `MergedView` — the merged scope for one file, preserving symbol provenance (own file vs. direct include vs. transitive include).
- `merge_includes_for_file(file: &Path, project: &VirtualProject) -> MergedView` — the API proposed in the blocker doc.
- `IncludeGraph` / `IncludeEdge` (provisional names) — represent resolved include relationships and support cycle detection + cache invalidation.
- Possible `Visibility::ExportedViaInclude` variant in `symbols.rs`.

### Estimated delta

The blocker doc estimates ~600–700 LOC total. A `git diff --stat` against the current branch is currently empty because no implementation exists yet, so the estimate remains the blocker doc’s 300–400 LOC new code + 150 LOC modified + 150 LOC tests.

---

## 3. Architecture sketch

For each file under analysis, the pipeline should be:

1. **Parse** the current file (`parser.rs:17-20`).
2. **Walk the AST** and collect `include_directive` nodes. The grammar already defines these at `tree-sitter-xs/grammar.js:112-116`:
   ```js
   include_directive: $ => seq('include', field('path', $.string_literal), ';'),
   ```
   The path child is a `string_literal`; stripping quotes yields the raw include target.
3. **Resolve** each target through `workspace::resolve_include` (`workspace.rs:189-198`), which already applies include-root context (`IncludeRoot::Ai/Trigger/RandomMap`, lines 55-74) and mod overlay (`VirtualProject.file_overrides`, line 35).
4. **Recurse** into the resolved files, using the existing `cache::load_or_parse_symbols` (`cache.rs:130-163`) to obtain each file’s `SymbolTable`. A visited set prevents infinite recursion on cyclic includes.
5. **Merge** symbol tables in scope order: current file first, then direct includes in file order, then transitive includes. Visibility filtering hides `static`/file-local variables from the includer while exposing `extern` and public functions.
6. **Feed the `MergedView`** into `completion`, `hover`, `definition`, `references`, `semantic::check_forward_declarations`, and `typecheck::resolve_workspace_function` instead of the raw `VirtualProject`.

The merged view should live **per open file** in the server state, analogous to the existing `symbol_tables` map (`server.rs:46`). It should be rebuilt in `publish_diagnostics` (`server.rs:728-777`) and kept fresh for non-mutating handlers (`completion`/`hover`/`definition`/`references`). Because a merge only depends on the current file and the content of included files, it can be memoized with a key derived from the file hash and the hashes of its resolved include closure; the existing per-file parse cache (`game_parse/v1/<mtime>-<sha256>.json`, `cache.rs:48-51`) already gives us cheap access to the inputs.

Interaction with mod overlay is straightforward: `resolve_include` already prefers mod files over vanilla files (`workspace.rs:171-181`). Any merged view therefore sees mod-shadowed include targets automatically.

---

## 4. Risks and unknowns

1. **Parser/grammar gap for the dominant `bo_*` functions.** Many functions in `game/ai/core/bo_system/bo_system.xs` — including `boBuild`/`boVillager`/`boUnit`/`boTransaction` — take function-pointer default arguments such as `void(int) afterQueue = [](int id = -1) {}`. Running `dump_top_level` on `bo_system.xs` shows that `boBuild` (line 28) is parsed as a `declaration` containing `ERROR` nodes rather than as a `function_definition`. Consequently these symbols are not extracted into the `SymbolTable`. **Merging symbol tables will not expose them.** This directly challenges the blocker doc’s assumption that true include-paste will drive the 5,335 threshold down. The proposal phase must decide whether to add grammar/symbol-extraction work to the change or to accept a much smaller drop in the unresolved count.

2. **Cycle detection algorithm.** No include graph exists today; `workspace.rs` only resolves one hop. A recursive DFS with a `visited: HashSet<PathBuf>` and a per-chain recursion stack is the obvious choice, but edge cases (cycle across three files, self-include, mod and vanilla forming a cycle after overlay) need unit coverage.

3. **Multi-pass vs. single-pass merge.** Semantic rules today run over the *entire* virtual project (`semantic.rs:96-102`). If we switch to a per-file merged view, rules such as `extern` collision (lines 106-183) and `mutable` redefinition (lines 261-309) may need to decide whether they apply to the whole project or only the include closure. The proposal needs to pin this down before specs can be written.

4. **Memory and latency.** Building a merged view for every keystroke could re-read and re-merge dozens of included files. The server already loads the full virtual project on each `publish_diagnostics` (`server.rs:779-796`), so the cost is bounded by the include closure, but a per-keystroke rebuild is still risky. A merged-view memoization cache keyed by `(current_file_hash, [(include_path, hash)])` is almost certainly required to stay under the 200 ms per-keystroke budget.

5. **Cache invalidation on included-file changes.** `server.rs::did_change_watched_files` currently invalidates only the changed file’s parse cache and re-diagnoses all open mod files (lines 328-373, with a `Phase 3` TODO at line 349). With merged views, the server needs an include graph to know which open files depend on a changed include target.

6. **Tree-sitter `include_directive` handling.** The grammar rule exists (`grammar.js:112-116`), but extraction of the `path` field and quote stripping has not been exercised in the current Rust code. The first implementation should pin this with a unit test.

7. **Mod overlay on included files.** While `resolve_include` supports overlay, the proposal must decide whether a mod file can include a file that only exists in the mod overlay (not in vanilla). `resolve_include_falls_back_to_vanilla` (`workspace.rs:443-461`) says yes; this should be a covered scenario.

8. **The `mut extern` corner case.** `symbols.rs` tracks both `is_extern` and `is_mutable` (lines 97-100), and `function_visibility` maps `extern` to `Visibility::Extern` (lines 410-418). The engine semantics of a symbol that is *both* `extern` and `mutable` are not documented in the existing specs; this needs verification before the visibility/merge rules are fixed.

9. **References/rename scope.** `references.rs` currently only walks the current file (`references.rs:36-54`). Expanding it to the merged view is marked “consider” in the blocker doc. The proposal should make an explicit yes/no decision; leaving it implicit will create inconsistent UX.

---

## 5. Test strategy

### Increment the integration threshold

`tests/game_folder_parse.rs` already defines `UNRESOLVED_SYMBOL_THRESHOLD: usize = 5870` (line 341) and explains that it should be tightened when include-paste is implemented. The proposal should pick an interim target after measuring the effect of the merged view. Because of the parser gap noted in §4, the first implementation may only drop the count partially; the test should assert the new measured value plus a small headroom, not jump straight to 0.

Proposed additions to `tests/game_folder_parse.rs`:
- A `#[test]` that lists the top 10 unresolved callees and asserts each stays below its pre-fix count.
- A happy-path assertion that a known included function (e.g. `boTech`, which parses correctly) is resolved when reached through `core.xs` includes.

### Unit tests for the merged view

A new `workspace::tests` suite or `merged_view::tests` should cover:
- Single direct include (symbol visible in includer).
- Transitive include chain (A includes B, B includes C; C’s symbol visible in A).
- Include cycle (A includes B, B includes A; no infinite loop, symbols visible up to first re-entry).
- Missing include target (graceful empty merge + diagnostic).
- Mod overlay on the include target.
- Include-root context: AI vs TRIGGER vs RANDOM_MAP.
- Visibility filtering: `static` variable in included file is hidden; `extern` variable is visible; public function is visible.

### LSP roundtrip tests

`src/bin/lsp_roundtrip_test.rs` should add sequences that open an includer and verify:
- `textDocument/completion` offers symbols from the included file.
- `textDocument/hover` on an included symbol shows the included file’s signature.
- `textDocument/definition` jumps to the included file (real `file://` URI, not an engine stub).
- `textDocument/references` can optionally find uses across the include boundary.
- The includer does **not** receive a forward-declaration diagnostic for a function defined only in the included file.
- A missing include produces a diagnostic.

### Semantic fixtures

Add under `src/semantic_fixtures/`:
- `include_forward_decl.xs` + `include_helper.xs` — positive case.
- `include_static_hidden.xs` + `include_static_helper.xs` — static variable must not leak.
- `include_missing.xs` — negative case, missing target.

### Manual smoke test

Before final acceptance, open `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` in IntelliJ/Rider and verify cross-include completion, hover, and go-to-definition, matching the blocker doc’s manual checklist.

---

## 6. Recommendation

### Readiness for planning

**Conditionally ready for `sdd-propose`.** The problem is well scoped, the existing primitives (`resolve_include`, per-file parse cache, `IncludeRoot`, mod overlay) are solid, and the target behavior is clear. However, the exploration uncovered a **critical assumption in the blocker doc that needs to be resolved before specs are written**: the tree-sitter grammar and symbol extractor currently do not represent the dominant `bo_*` functions as `Function` symbols because of function-pointer default arguments. If the goal is to drive the 5,335 unresolved-symbol count down, the change may need to include grammar/symbol-extraction work, not just include-paste plumbing. If that additional scope is accepted, the change is ready to plan; if it is rejected, the proposal must reset the success criteria.

### Open questions the proposal phase must answer

1. Does the fix include improving `symbols.rs` / the grammar so that functions with function-pointer defaults are extracted? If not, what is the realistic target for the unresolved-symbol threshold?
2. Should `semantic.rs` rules (`extern` collision, `mutable` redefinition) operate on the full project or on the per-file include closure?
3. Where does the merged view live: new `merged_view.rs`, inside `workspace.rs`, or inside `server.rs`?
4. What is the cache invalidation key and lifetime for merged views?
5. Should `references`/`rename` cross include boundaries, or only completion/hover/definition/semantic?
6. How should `static`/`extern` visibility be represented during a merge — new `Visibility::ExportedViaInclude`, a separate provenance map, or something else?
7. What is the engine behavior for `mutable extern` functions?

### Flagged blockers

- **Technical blocker:** The current parser does not extract the most common unresolved callees (`boVillager`, `boBuild`, `boUnit`, etc.) as symbols. Until this is addressed, the integration-test threshold will not drop to the implied near-zero level.
- **Spec blocker:** The existing `spec-semantic-diagnostics.md` acceptance criterion 8 says the server shall resolve symbols in an included file, but it does not define visibility rules for `static` vs. `extern` vs. public functions in the included file. This must be added.
