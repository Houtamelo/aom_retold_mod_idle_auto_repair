# Exploration: Finish deferred Bucket C semantic-token distinctions

## Executive summary

The umbrella `expand-color-scheme-semantic-tokens` change (Slices 1–3, pluginVersion `0.4.0 → 0.6.0`) intentionally deferred four semantic-token distinctions: full class extraction in the LSP symbol table, constant tokens, rule tokens, and `extern` variable tokens. After reading the committed code, the deferred items are all root-cause归类 in `tools/xs-language-server/src/symbols.rs` and `tools/xs-language-server/src/semantic_tokens.rs`; the plugin side already has the scaffolding for the original 8 categories but lacks both the new keys/descriptors and, critically, the platform `LspSemanticTokensSupport` wiring that would make any custom converter execute in the editor. The work is bounded, logically one slice, and should be a MINOR plugin version bump (`0.6.0 → 0.7.0`).

---

## Current state: LSP symbol table

`tools/xs-language-server/src/symbols.rs`:

- `SymbolKind` (lines 24–30) has only four variants: `Rule`, `Function`, `Variable`, `Constant`. **There is no `Class` variant.**
- `Symbol` (lines 82–112) already carries `is_extern`, `is_mutable`, `is_static`, and `visibility`, so storage-class / provenance metadata is present for most symbols.
- `build_symbol_table` (lines 137–162) walks `translation_unit` children and handles:
  - `rule_definition`
  - `function_definition`
  - `declaration`
  - `ERROR`
  - **It does NOT handle `class_specifier`.**
- `build_full_symbol_table` (lines 166–170) calls `build_symbol_table` then `extract_local_declarations`.
- `extract_local_declarations` / `extract_local_declaration` (lines 172–232) only produces `SymbolKind::Variable`; body `const` declarations would currently become variables, not constants.
- `extract_declaration` (lines 370–424) already distinguishes `SymbolKind::Constant` when `modifiers.is_const` is true, and sets `Visibility::Extern` / `Const` / `Local` correctly.
- `extract_function` (lines 328–368) captures `is_extern`, `is_static`, `is_mutable`, and `visibility`, including `Visibility::Extern`.

So classes are parsed by the grammar but never enter any symbol table, which is why a class reference like `MapInfo gMapInfo;` cannot be resolved to a workspace symbol.

---

## Current state: LSP semantic-token emission

`tools/xs-language-server/src/semantic_tokens.rs`:

- Legend (lines 19–31):
  - Token types: `function`, `variable`, `type`.
  - Modifiers: `engine`, `modded`, `unmodded`, `local`, `static`.
  - **No `constant`, `rule`, or `extern` entries.**
- `server_capabilities()` (lines 44–54) advertises the legend above.
- `walk_for_tokens` (lines 95–167) emits tokens for:
  - `identifier` → `classify_identifier`
  - `_type_identifier` → `classify_type_identifier`
  - `primitive_type` → hard-coded `type` + `engine`
- `classify_identifier` (lines 182–235):
  - `Function` → `SemanticTokenType::FUNCTION`
  - `Variable | Constant` → `SemanticTokenType::VARIABLE`  ← **constants look like variables**
  - `Rule` → `return None`  ← **rules are skipped**
- `classify_type_identifier` (lines 237–270):
  - Resolves `_type_identifier` through `merged.find()` → `ms.provenance.origin()`
  - Then `own_table.find()` → `current_file`
  - Otherwise falls back to `Origin::Engine`
  - Because classes are **not** in the symbol table, every user-defined class reference currently falls back to `engine` and the plugin paints it as `TYPE_BUILTIN`.
- `classify_modifiers` (lines 297–306):
  - Always emits the origin modifier.
  - For `Variable` / `Constant`, adds `static` if `is_static`, else `local` if `visibility == Visibility::Local`.
  - **No `extern` modifier is emitted**, even though `Symbol.is_extern` is faithfully set by the extractor.
- `encode` (lines 309–346) encodes using the module-local `TOKEN_TYPES` / `TOKEN_MODIFIERS` indices; adding new token types or modifiers is safe as long as legend order is mirrored in the plugin.

`tools/xs-language-server/src/server.rs`:

- The semantic-tokens provider is advertised at line 353:
  `semantic_tokens_provider: Some(semantic_tokens::server_capabilities())`.
- The handler is at lines 748–778.

---

## Tree-sitter grammar findings

`tools/xs-language-server/tree-sitter-xs/grammar.js`:

- `class_specifier` (lines 213–217):
  ```js
  class_specifier: $ => prec.right(seq(
    'class',
    field('name', $._type_identifier),
    field('body', $.field_declaration_list),
  )),
  ```
- `_type_identifier` (lines 712–715) is an alias of `$.identifier` exposed as `type_identifier`, and semantic-token walker matches node kind `_type_identifier`.
- `class_specifier` is included in `_top_level_item` (line 91) and participates in conflicts `[class_specifier, type_specifier]` and `[_top_level_item, class_specifier]` (lines 51–52).
- Class body (lines 326–347):
  - `field_declaration_list` contains `field_declaration` nodes.
  - `field_declaration` is a declaration specifiers + optional `_field_declarator` + optional body/init.
  - `_field_declarator` is `function_declarator | identifier`.
  - **There is no separate AST node kind for methods vs fields**; a method is a `field_declaration` whose declarator is a `function_declarator` and whose optional body is a `compound_statement`.

Live vanilla examples (read-only under `~/.steam/steam/steamapps/common/Age of Mythology Retold/game`) confirm the pattern, e.g. `game/ai/core/startup/map_analysis.xs`:

```xs
class MapInfo
{
   bool mIsNomadMap = false;
   ...
   void displayMapInfo()
   {
      ...
   }
};
extern MapInfo gMapInfo;
```

This validates that class extraction only needs to capture the class name; member extraction is a separate, larger concern.

---

## Current state: plugin TextAttributes + converter

`tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt` (lines 16–82):

- 12 inherited platform categories (`IDENTIFIER_UNDER_CARET`, `MATCHED_BRACE`, `UNMATCHED_BRACE`, `UNKNOWN_SYMBOL`, `BRACES`, `BRACKETS`, `COMMA`, `DOT`, `OPERATION_SIGN`, `OVERLOADED_OPERATOR`, `PARENTHESES`, `SEMI_COLON`).
- 8 semantic-token categories from Slice 3 (lines 57–81):
  - `FUNCTION_ENGINE`, `FUNCTION_UNMODDED`, `FUNCTION_MODDED`
  - `VARIABLE_LOCAL`, `VARIABLE_STATIC`
  - `TYPE_BUILTIN`, `TYPE_UNMODDED_CLASS`, `TYPE_MODDED_CLASS`
- **Missing:** constant keys, rule keys, and extern-variable keys.
- Note: `TYPE_UNMODDED_CLASS` and `TYPE_MODDED_CLASS` currently fall back to `DefaultLanguageHighlighterColors.IDENTIFIER` rather than `CLASS_NAME`.

`tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt` (lines 45–75):

- 21 descriptors: 6 lexical + 12 inherited + 8 semantic.
- The 8 semantic descriptors are at lines 67–74.
- **Missing descriptors for Constant, Rule, and Extern Variable.**

`tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverter.kt` (lines 25–73):

- Current mapping table:
  - `function` + (`engine`/`modded`/`unmodded`) → `FUNCTION_ENGINE`/`FUNCTION_MODDED`/`FUNCTION_UNMODDED`
  - `variable` + `static` → `VARIABLE_STATIC`
  - `variable` without `static` → `VARIABLE_LOCAL`  ← **ignores origin and `extern`**
  - `type` + (`engine`/`modded`/`unmodded`) → `TYPE_BUILTIN`/`TYPE_MODDED_CLASS`/`TYPE_UNMODDED_CLASS`
- It declares a `MODIFIER_EXTERN` constant (line 37), but **no branch consumes it**.
- It also has no branches for `constant` or `rule` token types.

`tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt` (lines 26–109):

- **Does not override `lspCustomization`.**
- **`XsSemanticTokensSupport` does not exist anywhere in the plugin source.**
- Therefore, the `XsSemanticTokensConverter` is effectively dead code from the platform's point of view. Even the existing `TYPE_UNMODDED_CLASS` / `TYPE_MODDED_CLASS` categories cannot fire for real class references until an `LspSemanticTokensSupport` subclass is created and wired through `lspCustomization`.

`tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` (lines 19–49) has no semantic-token extension point.

---

## Existing test coverage summary

`tools/xs-language-server/tests/semantic_tokens_repro.rs`:

- 6 tests (currently included in the 230 LSP tests):
  1. legend advertised
  2. engine function modifier
  3. modded function modifier
  4. unmodded function modifier
  5. local variable modifier
  6. built-in type (`int`) engine modifier
- Pattern: build a temp `Workspace` / `VirtualProject`, call `compute_for`, then grep tokens by text and assert `(token_type, modifiers)`.

`tools/xs-language-server/src/symbols.rs` unit tests:

- Cover `extern` variable, `const` as `Constant`, functions, rules, forward declarations, local extraction, and rule-registration helpers.
- **No test currently covers `class_specifier` extraction.**

`tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt`:

- 10 tests mapping `(tokenType, modifiers)` → `TextAttributesKey`.
- Covers function origins, variable static/local, type origins, unknown fallback, and function-without-origin default.
- **No tests for `constant`, `rule`, or `extern` combos.**

The strict-TDD pattern used in the prior slice is to write a compile-time contract (references to not-yet-created APIs) plus a runtime contract that greps generated tokens/keys. The same pattern applies here.

---

## Scope

1. **Add `SymbolKind::Class` and extract `class_specifier` in the LSP symbol table**
   - Add `Class` variant to `SymbolKind` and update `label()`.
   - Add a `class_specifier` branch in `build_symbol_table` and an `extract_class` helper that captures the class name (`_type_identifier`), ranges, and current-file origin.
   - Add a unit test in `symbols.rs`.
   - Estimated: **+30 LOC**.
   - // In: `tools/xs-language-server/src/symbols.rs:24-30`, `137-162`.

2. **Extend the LSP semantic-token legend**
   - Add custom token types `constant` and `rule` to `TOKEN_TYPES`.
   - Add `extern` modifier to `TOKEN_MODIFIERS`.
   - Estimated: **+10 LOC**.
   - // In: `tools/xs-language-server/src/semantic_tokens.rs:19-31`.

3. **Emit `constant` tokens for `SymbolKind::Constant`**
   - In `classify_identifier`, map `SymbolKind::Constant` to the new `constant` token type.
   - Estimated: **+5 LOC**.
   - // In: `tools/xs-language-server/src/semantic_tokens.rs:222-226`.

4. **Emit `rule` tokens for `SymbolKind::Rule`**
   - In `classify_identifier`, map `SymbolKind::Rule` to the new `rule` token type instead of returning `None`.
   - Estimated: **+5 LOC**.
   - // In: `tools/xs-language-server/src/semantic_tokens.rs:222-226`.

5. **Emit the `extern` modifier**
   - In `classify_modifiers`, push `extern` when `symbol.is_extern`.
   - For extern variables this produces modifiers `{origin, extern}`; for extern functions `{origin, extern}`.
   - Estimated: **+5 LOC**.
   - // In: `tools/xs-language-server/src/semantic_tokens.rs:297-306`.

6. **Wire the plugin converter to the platform**
   - Create `XsSemanticTokensSupport` extending `LspSemanticTokensSupport()` and override the platform mapping method to delegate to `XsSemanticTokensConverter.convert(tokenType, modifiers)`.
   - Override `lspCustomization` in `XsLspServerDescriptor` to set `semanticTokensCustomizer = XsSemanticTokensSupport()`.
   - Estimated: **+40 LOC** (+1 new file).
   - // In: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt:26-34`, new `XsSemanticTokensSupport.kt`.

7. **Add plugin `TextAttributesKey`s for the new categories**
   - Constant: `CONSTANT_ENGINE`, `CONSTANT_UNMODDED`, `CONSTANT_MODDED`.
   - Rule: `RULE_UNMODDED`, `RULE_MODDED` (engine rules do not exist).
   - Extern variable: `VARIABLE_EXTERN_UNMODDED`, `VARIABLE_EXTERN_MODDED`.
   - Consider switching `TYPE_UNMODDED_CLASS` / `TYPE_MODDED_CLASS` fallback from `IDENTIFIER` to `CLASS_NAME`.
   - Estimated: **+30 LOC**.
   - // In: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt:57-81`.

8. **Register the new descriptors in the color settings page**
   - Under `Identifier//Constant`, `Identifier//Rule`, and `Identifier//Variable//Extern`.
   - Estimated: **+10 LOC**.
   - // In: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt:66-75`.

9. **Extend `XsSemanticTokensConverter`**
   - Handle `constant` + origin (`engine`/`unmodded`/`modded`).
   - Handle `rule` + origin (`unmodded`/`modded`), with a conservative fallback.
   - Handle `variable` + `extern` + origin → `VARIABLE_EXTERN_UNMODDED` / `VARIABLE_EXTERN_MODDED`.
   - Update KDoc to list the full legend.
   - Estimated: **+30 LOC**.
   - // In: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverter.kt:48-73`.

10. **Strict-TDD tests**
    - LSP: add tests to `semantic_tokens_repro.rs` for class origin, constant origin, rule origin, and extern-variable modifier.
    - LSP: add a class-extraction test to `symbols.rs`.
    - Plugin: add converter tests to `XsSemanticTokensConverterTest.kt` for constant, rule, extern-variable combos.
    - Plugin: extend `XsColorSettingsPageTest` to assert the new descriptors exist.
    - Plugin: add a lightweight test that `XsLspServerDescriptor.lspCustomization` returns a non-default semantic-token customizer.
    - Estimated: **+100 LOC**.
    - // In: `tools/xs-language-server/tests/semantic_tokens_repro.rs`, `tools/xs-language-server/src/symbols.rs` tests, `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt`, `XsColorSettingsPageTest.kt`.

11. **Version bump and documentation**
    - Bump `pluginVersion` from `0.6.0` to `0.7.0`.
    - Update `AGENTS.md` LSP/plugin test-count lines.
    - Estimated: **+5 LOC**.
    - // In: `tools/intellij-xs-plugin/gradle.properties:12`, `AGENTS.md`.

**Total estimated LOC delta: +270–350 (well under the 400-line review budget).**

---

## Out of scope

- **Class member extraction** — fields and methods inside `field_declaration_list` are not extracted as separate symbols. Only the class name is needed to give class references the correct `modded`/`unmodded` origin.
- **Cross-file forward-declaration ordering** — remains a separate future SDD change.
- **TextMate / native highlighter work** — already handled by the umbrella change.
- **Changing the fallback color of existing semantic categories** unless the spec explicitly asks (e.g., `TYPE_UNMODDED_CLASS` defaulting to `IDENTIFIER` is noted but optional).
- **Engine `extern` classification** — engine code has no `extern` variables, so only `extern unmodded` and `extern modded` categories are proposed.

---

## Recommended approach

**Minimal-change path.**

- Add `Class` to `SymbolKind` and extract only the class name from `class_specifier`. This immediately makes `_type_identifier` references resolve to workspace symbols and lets the existing `type` + `modded`/`unmodded` mapping in the plugin fire.
- Add `constant` and `rule` as new custom LSP token types, and `extern` as a new modifier. This keeps encoding/decoding simple and avoids overloading existing token types with too many modifiers.
- Add the missing `LspSemanticTokensSupport` wiring so the existing and new plugin keys are actually used by the platform.

Justification: the umbrella change built the pipeline and the first 8 categories; this change only fills the deferred gaps, respects the existing architecture, and avoids the much larger work of class member extraction.

---

## Slice recommendation

**1 slice / single PR.**

- All changes are tightly coupled by the same LSP legend and the same plugin converter.
- The estimated delta (~270–350 LOC) is comfortably below the 400-line review budget.
- A single PR makes it easier to verify end-to-end that class/constant/rule/extern tokens flow from the LSP through the converter to the editor.

If the platform `LspSemanticTokensSupport` wiring turns out to require a larger-than-expected platform workaround, consider a follow-up slice just for that wiring; but that is not the forecast.

---

## Risks

1. **Platform API uncertainty** — the exact signature of `LspSemanticTokensSupport.getTextAttributesKey(...)` and the property name in `LspCustomization` must be verified against the bundled 2024.2 platform sources before spec/design. A wrong override will fail plugin compilation or silently disable semantic tokens.
2. **Plugin converter is currently unwired** — even the existing 8 categories rely on the missing `XsSemanticTokensSupport` + `lspCustomization` wiring. If this wiring is forgotten again, all new work will be test-only.
3. **Cache compatibility** — `SymbolKind` is serialized in the per-file symbol cache under `~/.local/state/aomr_lsp/v2/`. Adding a `Class` variant is a schema change; while JSON-serialized unit enums are string-tagged and forward-compatible for new readers, the proposal/verify phase should confirm by running with a warm cache.
4. **Class extraction performance** — adding a top-level `class_specifier` walk is cheap, but the proposal should forbid member extraction to prevent `O(methods × fields)` overhead in large vanilla files such as `strategy_internal.xs`.
5. **Rule token usefulness** — rules are primarily referenced via string literals in `xsEnableRule("...")`, `trRuleAdd("...")`, etc. Emitting tokens only on `rule_definition` identifiers covers the definition, not every runtime registration string, so user-visible value is limited.

---

## pluginVersion impact

**MINOR: `0.6.0 → 0.7.0`.**

Per `AGENTS.md` bump policy, a new LSP feature (custom `constant`/`rule` token types and `extern` modifier), new settings-page categories, and a new platform wiring class all trigger a minor bump. Bug-fix-only changes would be a patch, but this change materially extends the advertised semantic-token legend and user-facing color categories.

---

## Open questions for the proposal phase

1. **Exact platform API** — What is the precise override signature of `LspSemanticTokensSupport` and the corresponding property in `LspCustomization` for IntelliJ Platform 2024.2/2025.2? Verify against bundled sources.
2. **Constant/rule representation** — Should `const` and `rule` be new custom token types (recommended), or modifiers on `variable`/`function`? Confirm in the spec.
3. **Extern variable categories** — Should `extern` always be combined with `modded`/`unmodded` (giving `VARIABLE_EXTERN_UNMODDED` / `VARIABLE_EXTERN_MODDED`), or should `extern` be a standalone modifier independent of origin?
4. **Class key fallback** — Should `TYPE_UNMODDED_CLASS` / `TYPE_MODDED_CLASS` fall back to `DefaultLanguageHighlighterColors.CLASS_NAME` instead of `IDENTIFIER` for a more useful default?
5. **Class member extraction** — Confirm that extracting only the class name (no fields/methods) is acceptable for this cycle.
