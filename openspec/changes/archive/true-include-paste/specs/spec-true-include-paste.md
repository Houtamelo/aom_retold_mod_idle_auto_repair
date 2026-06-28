# True Include-Paste Specification

> **Added/updated by change:** `true-include-paste`

## Capability summary

The LSP server SHALL model Age of Mythology: Retold’s `include "..."` directive as a textual paste. For every analyzed file it SHALL build a merged scope from the file and its resolved include closure, with correct visibility: `static`/file-local symbols from included files are hidden, `extern` variables and public functions are visible. Transitive includes SHALL be followed, cyclic includes SHALL terminate cleanly, and missing include targets SHALL produce diagnostics without aborting the remaining merge. The tree-sitter grammar and symbol extractor SHALL also be extended so function definitions with function-pointer parameter types and lambda defaults are parsed and extracted as `Function` symbols.

## Rationale

AoM:R loads an includer as if the contents of each included file were pasted at the `include` directive. The current LSP uses a line-regex approximation (`completion.rs:131-149`) and a global project fallback (`semantic.rs:346-388`), so it exposes the wrong symbols, misses transitive includes, ignores `static`/`extern`, and cannot resolve the dominant `bo_*` callees because they do not parse as functions. True include-paste semantics align diagnostics, completion, hover, definition, references, and type checking with the engine.

## Scenarios

The scenarios below exercise the grammar change, the merged-view builder, visibility filtering, LSP cross-file features, and acceptance thresholds. The planned merged-view entry point is `MergedView::for_file(file: &Path, project: &VirtualProject)` in `src/merged_view.rs`; it consumes `workspace::resolve_include` (`workspace.rs:189-198`) and the per-file parse cache (`cache.rs:130-163`). Grammar changes target `tree-sitter-xs/grammar.js` (`parameter_declaration` lines 345-356; `expression` lines 483-505) and `src/symbols.rs` parameter/lambda extraction.

### Scenario: function_with_function_pointer_default_parses_as_function_definition

- GIVEN an `.xs` file contains the definition `void foo(int x = -1, void(int) cb = [](int id = -1) {}) { }`
- WHEN the file is parsed
- THEN the AST contains a `function_definition` node whose name is `foo` and whose parameter list has two entries

### Scenario: function_with_trailing_lambda_default_extracts_default_values

- GIVEN the same `foo` definition with a lambda default for `cb`
- WHEN symbol extraction runs
- THEN `foo` is stored as a `Function` symbol with two parameters, and the default value of `cb` is the raw source text of the lambda expression

### Scenario: function_with_simple_default_still_works

- GIVEN an `.xs` file contains the definition `void bar(int x = -1) { }`
- WHEN the file is parsed and symbol extraction runs
- THEN the node is a `function_definition` named `bar`, and the symbol table records it as a `Function` with one parameter

### Scenario: direct_include_resolves_in_includer

- GIVEN `a.xs` contains `include "b.xs"` and `b.xs` defines `void helper() { }`
- WHEN the merged view is built for `a.xs`
- THEN `helper` is resolvable from `a.xs`

### Scenario: transitive_include_resolves

- GIVEN `a.xs` includes `b.xs`, `b.xs` includes `c.xs`, and `c.xs` defines `void deep() { }`
- WHEN the merged view is built for `a.xs`
- THEN `deep` is visible in `a.xs`

### Scenario: include_cycle_does_not_loop

- GIVEN `a.xs` includes `b.xs` and `b.xs` includes `a.xs`
- WHEN the merged view is built for `a.xs`
- THEN the traversal terminates in bounded time and no symbol appears more than once in the merged scope

### Scenario: missing_include_target_produces_diagnostic

- GIVEN `a.xs` contains `include "nonexistent.xs"`
- WHEN the merged view is built for `a.xs`
- THEN a diagnostic is emitted whose message includes the target path `nonexistent.xs`, and resolution of any other includes in `a.xs` proceeds normally

### Scenario: mod_overlay_on_include_target_is_preferred

- GIVEN `mod/x/game/ai/foo.xs` contains `include "bar.xs"`, and both `mod/x/game/ai/bar.xs` and `<AOMR>/game/ai/bar.xs` exist
- WHEN the merged view is built for `foo.xs`
- THEN the symbol table for `bar.xs` is loaded from `mod/x/game/ai/bar.xs`, not from the vanilla game folder

### Scenario: static_variable_in_included_file_is_hidden

- GIVEN `b.xs` declares `static int gHidden = 0;` and `a.xs` includes `b.xs`
- WHEN the merged view is built for `a.xs`
- THEN `gHidden` is absent from completion and symbol resolution for `a.xs`

### Scenario: extern_variable_in_included_file_is_visible

- GIVEN `b.xs` declares `extern int gShared = 0;` and `a.xs` includes `b.xs` and references `gShared`
- WHEN diagnostics run for `a.xs`
- THEN `gShared` is visible and no `unresolved_symbol` diagnostic is produced

### Scenario: public_function_in_included_file_is_visible

- GIVEN `b.xs` defines `void helper() { }` and `a.xs` includes `b.xs` and calls `helper()`
- WHEN diagnostics run for `a.xs`
- THEN no `use before definition` error is reported and `textDocument/completion` at the call site offers `helper`

### Scenario: mutable_redefinition_in_included_file_follows_engine_rules

- GIVEN `b.xs` defines `mutable void helper(int x = -1) { }` and `a.xs` includes `b.xs` and calls `helper(5)`
- WHEN diagnostics run for `a.xs`
- THEN no `unresolved_symbol` diagnostic is emitted and `helper` resolves to the definition in `b.xs`

### Scenario: completion_offers_included_symbols

- GIVEN `a.xs` includes `b.xs`, `b.xs` defines `void included() { }`, and the cursor is inside `a.xs`
- WHEN `textDocument/completion` is invoked
- THEN `included` appears in the completion list

### Scenario: hover_shows_included_file_signature

- GIVEN `a.xs` includes `b.xs` and `b.xs` defines `void included() { }`
- WHEN `textDocument/hover` is invoked on the `included` token in `a.xs`
- THEN the hover content shows the signature `void included()` and a `file://` URI pointing to `b.xs`

### Scenario: goto_definition_jumps_to_included_file

- GIVEN `a.xs` includes `b.xs` and `b.xs` defines `void included() { }`
- WHEN `textDocument/definition` is invoked on the `included` token in `a.xs`
- THEN the response is a single `Location` whose URI points to `b.xs`

### Scenario: included_file_change_invalidates_merged_view

- GIVEN `a.xs` is open in the editor and its merged view has already been built
- WHEN `b.xs` is modified and the server receives `workspace/didChangeWatchedFiles`
- THEN the merged view for `a.xs` is invalidated and the next diagnostics pass rebuilds it

### Scenario: merged_view_under_keystroke_budget

- GIVEN an `.xs` file that directly or transitively includes 20 other files
- WHEN the merged view is built with warm per-file parse caches
- THEN the build completes within 200 ms

### Scenario: unresolved_symbol_baseline_drops_below_threshold

- GIVEN `tests/game_folder_parse.rs::semantic_pipeline_unresolved_count_within_threshold` is run after the implementation
- WHEN the test asserts the unresolved-symbol threshold
- THEN the reported count is below 1,000 (down from 5,335)

## Deferred to design phase

- The combined `mutable extern` modifier behavior for functions: no XS sample or spec currently demonstrates it, so the visibility/merge rules should be validated against engine output before a scenario is added.

## Acceptance criteria

1. The grammar SHALL accept a function-pointer parameter type and a lambda default value in a function definition.
2. The symbol extractor SHALL represent the dominant `bo_*` functions (e.g., `boVillager`, `boBuild`) as `Function` symbols.
3. Include resolution SHALL use `workspace::resolve_include`, honoring `AI`/`TRIGGER`/`RANDOM_MAP` include roots and mod overlay.
4. The merged view SHALL follow transitive includes and SHALL terminate cleanly on cyclic includes keyed by absolute path.
5. Symbols from included files SHALL carry provenance metadata and SHALL be filtered by visibility: `static`/file-local variables hidden, `extern` variables and public functions visible.
6. The merged view SHALL feed completion, hover, definition, references, semantic diagnostics, and type checking for each open file.
7. A missing include target SHALL produce a diagnostic and SHALL NOT abort resolution of the remaining includes.
8. A changed included file SHALL invalidate the merged views of every open file that includes it.
9. The existing 75 LSP unit tests SHALL continue to pass, the three game-folder integration tests SHALL pass when `AOMR_GAME_PATH` is set, and the unresolved-symbol baseline for the full game folder SHALL fall below 1,000.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `true-include-paste` | 2026-06-24 | — | New spec covering grammar fix, merged-view include-paste semantics, visibility, and cross-file LSP features. |
