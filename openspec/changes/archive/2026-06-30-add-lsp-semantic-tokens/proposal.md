# Proposal: add-lsp-semantic-tokens

## Status
Draft

## Background

After the Issue #3 Option α cycle (`a370e78`, plugin 0.3.0) shipped
the inherited Rider categories and the `expand-color-scheme-categories`
quick-wins slice (commit `1bc73e4`, plugin 0.4.1), the deferred
Bucket C work remains: emit LSP `textDocument/semanticTokens/full`
classifications for engine / modded / unmodded functions and
variables, plus local-variable and built-in-type extraction.

The umbrella `expand-color-scheme-semantic-tokens` change folder
(`openspec/changes/expand-color-scheme-semantic-tokens/`) holds the
full plan (explore, proposal, spec, design, tasks). This slice
implements the **LSP server** portion of that plan; the
`add-plugin-semantic-tokens` slice implements the plugin side.

## Approach

Add `textDocument/semanticTokens/full` capability to the LSP server
in `tools/xs-language-server/src/server.rs`. Define a small semantic-
token legend (3 token types — function, variable, type — and 5
modifiers — engine, modded, unmodded, static, extern). Walk the
symbol table for each open file and emit delta-encoded tokens.

The legend intentionally omits the heavier token types / modifiers
that would require more extraction work (class detection, rule
detection, full constant classification, full extern distinction).
Those are deferred to a follow-up cycle.

Origin classification (engine / modded / unmodded) is determined
per-file by the workspace resolver:
- A symbol in a file under the mod's `game/` overlay → `modded`.
- A symbol in a file under the vanilla `<AOMR>/game/` folder → `unmodded`.
- A symbol defined in the engine API (doxygen extract) → `engine`.

Local-variable extraction walks function bodies to find local-
variable declarations and includes them in the symbol table via a
new `build_full_symbol_table` helper.

## User-facing contract (server side)

After this change:

- The LSP server advertises `semanticTokensProvider` with a legend
  containing 3 token types and 5 modifiers.
- The LSP server responds to `textDocument/semanticTokens/full`
  with the correct token-type + modifier combinations for each
  function / variable / type reference in the open file.
- The plugin side (delivered in the next slice, `add-plugin-semantic-tokens`)
  wires the LSP response to its color-scheme categories.

## Scope

### In scope

- `tools/xs-language-server/src/semantic_tokens.rs` — new module
  defining the legend, the handler, and the delta-encoded output.
- `tools/xs-language-server/src/server.rs` — register the
  `semantic_tokens_provider` capability; add the
  `semantic_tokens_full` handler.
- `tools/xs-language-server/src/lib.rs` — register the new module.
- `tools/xs-language-server/src/symbols.rs` — add `is_static: bool`
  field to `Symbol`; add `build_full_symbol_table` that includes
  local-variable extraction; add `extract_local_declarations` walker.
- `tools/xs-language-server/src/references.rs` — add `is_static: false`
  to the test fixture (matches the new Symbol field).
- `tools/xs-language-server/src/semantic.rs` — pass `is_static` from
  the modifier detection through.
- `tools/xs-language-server/tests/semantic_tokens_repro.rs` — new
  file with 6 strict-TDD integration tests.
- `tools/intellij-xs-plugin/gradle.properties` —
  `pluginVersion` bumped `0.4.1 → 0.5.0` (MINOR per AGENTS.md
  "new LSP feature" policy).
- `AGENTS.md` — test-count line bumped `220 → 230`.

### Out of scope

- Plugin-side semantic-token converter, color-scheme category
  registration, plugin.xml wiring — see `add-plugin-semantic-tokens`.
- Class extraction (full class symbols in the symbol table).
- Constant / rule / full-extern classification.
- Cross-file forward-declaration ordering (separate future spec).

## Impact

- Files changed:
  - 1 new Rust file: `tools/xs-language-server/src/semantic_tokens.rs` (~250 LOC).
  - 5 modified Rust files in `tools/xs-language-server/src/`.
  - 1 new Rust test file: `tools/xs-language-server/tests/semantic_tokens_repro.rs` (~300 LOC).
  - 1 modified gradle.properties.
  - 2 modified docs (`AGENTS.md`, `docs/issues/2026-06-29-runtime-issues.md`).
- LOC delta: ~600 across tracked files.
- Risk: low (the implementation is isolated; tests prove correctness).
- Effort: ~1 day.
- Compatibility: the `Symbol::is_static` field is `#[serde(default)]`,
  so pre-existing serialized data (if any) still parses.

## Success criteria

- All 6 strict-TDD integration tests in
  `tests/semantic_tokens_repro.rs` pass:
  - `test_semantic_token_legend_advertised` — capability + legend
    registered.
  - `test_engine_function_emits_engine_modifier` — engine API
    function calls emit the `engine` modifier.
  - `test_modded_function_emits_modded_modifier` — function calls
    in a mod-overlaid file emit `modded`.
  - `test_unmodded_function_emits_unmodded_modifier` — function
    calls in a vanilla-game-only file emit `unmodded`.
  - `test_local_variable_emits_local_modifier` — local variable
    references emit `local`.
  - `test_builtin_type_emits_engine_modifier` — `bool` / `int` /
    `float` / `string` / `vector` references emit `engine`.
- All 220 prior LSP tests still pass.
- `cargo clippy --all-targets` reports no new warnings.
- Plugin builds: `dist/intellij-xs-plugin-0.5.0.zip` contains the
  new LSP binary; bundled plugin.xml reads `<version>0.5.0</version>`.

## Risks and unknowns

- The origin classification depends on the mod-vs-vanilla path
  resolution; if the LSP can't reliably distinguish modded from
  unmodded files, the modifier will fall back to a sensible
  default (e.g. `unmodded` if not under a known mod root).
- Performance: semantic-token requests can fire on every edit;
  the implementation should be efficient. The current design
  rebuilds the merged view per request (acceptable for files
  up to a few thousand lines; may need caching for larger files).
- Token legend stability: changing the legend after the plugin
  consumes it could break the plugin side. The legend here is
  intentionally small and additive; future categories extend
  rather than replace.

## Next step

Run `sdd-verify` with fresh-context adversarial review, then
`sdd-archive` to move the change folder, then commit with the
prescribed Conventional Commits message.