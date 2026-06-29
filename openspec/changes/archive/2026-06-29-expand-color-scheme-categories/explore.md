## Bug status
CONFIRMED

Issue #3 reproduces as described: Settings → Editor → Color Scheme → `xs` currently exposes only
Comment, Default, Identifier, Keyword, Number, String, and the menu label is lowercase `xs`.

## Current color-scheme architecture

Color settings are registered as a standard IntelliJ Platform `ColorSettingsPage`:

* `META-INF/plugin.xml` registers `com.aomr.xs.highlight.XsColorSettingsPage` at line 38.
* `XsColorSettingsPage` returns the display name `"xs"`, an `XsSyntaxHighlighter`, and a flat array
  of six `AttributesDescriptor`s backed by `XsTextAttributesKeys`.
* `XsTextAttributesKeys` defines six keys (`XS_KEYWORD`, `XS_STRING`, `XS_COMMENT`, `XS_NUMBER`,
  `XS_IDENTIFIER`, `XS_DEFAULT`) and gives each a fallback to a
  `DefaultLanguageHighlighterColors` constant.
* `XsSyntaxHighlighter` maps lexer token types to those keys. It relies on the generated JFlex
  lexer (`XsLexerAdapter`) plus `XsHighlightingLexer`, which folds some identifiers into the
  `KEYWORD` or `NUMBER` token types.
* A TextMate bundle is also registered (plugin.xml line 31, `XsTextMateBundleProvider`). It is
  deployed as a VSCode-style extension (`package.json` + `syntaxes/xs.tmLanguage.json`). The
  platform TextMate integration already maps standard scopes such as `keyword.control.c`,
  `string.quoted.double.c`, etc. to `DefaultLanguageHighlighterColors`, but those mappings do not
  appear as configurable categories under the XS color-scheme page.
* There is **no** LSP semantic-highlights converter today; the plugin receives no semantic tokens
  from the XS LSP server.

## Code-path map

* `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml:31` — TextMate bundle provider
  registration.
* `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml:38` — `colorSettingsPage`
  extension point.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt:12` —
  `getDisplayName()` returns `"xs"`.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt:27-42` —
  current `AttributesDescriptor` list and `<tag>` → `TextAttributesKey` demo mapping.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributesKeys.kt:7-14` —
  all existing keys and their fallback colors.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt:13-26` —
  lexer-token-to-key mapping.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt:8-33` —
  lexer-level keyword / number promotion.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsTokenType.kt:6-23` — raw lexer token
  types (identifiers, braces, brackets, parens, comma, semicolon, etc.).
* `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json:1-1546+` — TextMate
  scopes for comments, strings, numbers, keywords, types, operators, braces, etc.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt:26-109` —
  LSP server descriptor; currently has no `lspCustomization` and therefore uses the platform's
  default semantic-tokens mapping (which requires the server to advertise the capability first).
* `tools/xs-language-server/src/server.rs:301-354` — `ServerCapabilities`; no
  `semantic_tokens_provider` is declared.
* `tools/xs-language-server/src/symbols.rs:24-109` — `SymbolKind`, `Visibility`, and `Symbol`
  structure.
* `tools/xs-language-server/src/semantic.rs:158-166` — semantic resolution returns
  `Workspace`/`Engine`/`Unresolved`, but it does not tag a symbol as *modded* vs *unmodded*.
* `tools/xs-language-server/src/workspace.rs:32-52` — `VirtualProject.file_overrides` records
  which files are supplied by the mod overlay vs the vanilla game folder.

## LSP-side data sufficiency

The LSP server currently provides **no semantic tokens**. It could in principle distinguish the
requested categories, but several gaps exist:

| New category | LSP data today | Gap |
| --- | --- | --- |
| Engine function | `engine_api.lookup(name)` (syscalls) plus `BUILTIN_CALLEES` | Already resolvable; needs to be emitted as a semantic token. |
| UnModded function | `SymbolKind::Function` defined in a vanilla game file | Origin path must be compared to `Workspace.game_path`; not exposed today. |
| Modded function | `SymbolKind::Function` defined in a mod overlay file | Same origin-path logic as above. |
| UnModded Rule / Modded Rule | `SymbolKind::Rule` plus origin path | Same as functions. |
| Local variable | Not tracked | The symbol table only records top-level declarations. Need AST scope walk or a new locals pass. |
| Static variable | `storage_class_specifier` `"static"` is parsed in `extract_modifiers` (line 513-514), but `Symbol` does not expose an `is_static` flag; `Visibility::Local` is currently used for non-`extern`/non-`const` variables. | Need expose `static` explicitly. |
| `extern` unmodded/modded variable | `is_extern` + `Visibility::Extern` + origin path | Already resolvable; needs origin flag. |
| Engine constant | `engine_api.aiplans` list (193 constants) + a small set of builtins | Already resolvable; needs semantic token. |
| UnModded / Modded constant | `SymbolKind::Constant` + origin path | Already resolvable; needs origin flag. |
| Built-in type | Lexically `bool`, `int`, `float`, `string`, `vector` | Already highlighted as keyword; can be moved to a dedicated category without LSP changes. |
| UnModded / Modded class | Not tracked | Tree-sitter grammar has `class_specifier`, but `symbols.rs` does not extract it and `SymbolKind` lacks a `Class` variant. |

Conclusion: **LSP changes are required**. They are probably small for functions/constants/rules
(add a semantic-token endpoint and a `source` origin tag), but larger for local variables and
classes.

## Rider General inheritance approach

The recommended IntelliJ Platform mechanism is `TextAttributesKey.createTextAttributesKey(
"XS_...", baseKey)` so the new key inherits from a standard key at runtime and from the active
scheme.

For the categories the user explicitly wants inherited from General / Language Defaults:

* `Code / Identifier under caret` → fallback to
  `EditorColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES`.
* `Code / Matched brace` → fallback to `CodeInsightColors.MATCHED_BRACE_ATTRIBUTES`.
* `Code / Unmatched brace` → fallback to `CodeInsightColors.UNMATCHED_BRACE_ATTRIBUTES`.
* `Errors and Warnings / Unknown symbol` → fallback to
  `CodeInsightColors.WRONG_REFERENCES_ATTRIBUTES`.
* `Braces and Operators` items → fallback to `DefaultLanguageHighlighterColors.BRACES`,
  `BRACKETS`, `COMMA`, `DOT`, `OPERATION_SIGN`, `PARENTHESES`, `SEMICOLON`.

Source references:

* JetBrains `EditorColors.java` shows `IDENTIFIER_UNDER_CARET_ATTRIBUTES`
  (https://github.com/JetBrains/intellij-community/blob/master/platform/editor-ui-api/src/com/intellij/openapi/editor/colors/EditorColors.java).
* JetBrains `CodeInsightColors.java` shows `MATCHED_BRACE_ATTRIBUTES`,
  `UNMATCHED_BRACE_ATTRIBUTES`, and `WRONG_REFERENCES_ATTRIBUTES`
  (https://github.com/JetBrains/intellij-community/blob/master/platform/core-api/src/com/intellij/openapi/editor/colors/CodeInsightColors.java).
* Default-language key inheritance is documented at
  https://plugins.jetbrains.com/docs/intellij/color-scheme-management.html.

`additionalTextAttributes` in `plugin.xml` with a bundled XML file is the alternative for forcing
fixed default colors, but the docs strongly discourage fixed defaults because they break scheme
compatibility. The key-dependency approach is therefore preferred.

## Affected files (potential)

Plugin-side:

* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributesKeys.kt` —
  declare the new keys and their fallbacks.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt` — update
  `getDisplayName()` to `"XS"` and expand `DESCRIPTORS` / `TAG_MAP` / `DEMO_TEXT`.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt` — map
  braces/operators and built-in types to the new inherited keys.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt` — possibly
  keep built-in types distinct from keywords.
* `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` — if TextMate scopes
  should also drive the new categories, new scopes or scope-to-key wiring may be required.
* `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt` — return a
  custom `LspCustomization` with an `LspSemanticTokensSupport` mapper that turns LSP token
  types/modifiers into the new `XsTextAttributesKeys`.
* `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt` —
  update assertions and add coverage for new descriptors / display name.
* `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterFactoryTest.kt`
  — add assertions that braces/operators map to the inherited keys.
* `tools/intellij-xs-plugin/gradle.properties` — bump `pluginVersion` to `0.3.0`.

LSP-side:

* `tools/xs-language-server/src/server.rs` — advertise `semantic_tokens_provider` in
  `ServerCapabilities`.
* New file `tools/xs-language-server/src/semantic_tokens.rs` (suggested) — implement
  `textDocument/semanticTokens/full` and/or `/range`; classify symbols by kind + origin
  (modded/unmodded/engine).
* `tools/xs-language-server/src/symbols.rs` — add `Class` variant and expose `is_static` if needed.
* `tools/xs-language-server/src/semantic.rs` / `merged_view.rs` — reuse existing resolution and
  provenance logic.
* `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` — extend with semantic-token fixtures
  once the endpoint exists.

## Test infrastructure

Existing plugin tests already exercise the color page and highlighter factory:

* `XsColorSettingsPageTest` verifies page registration, display name, and descriptor presence.
* `XsSyntaxHighlighterFactoryTest` verifies keyword/string/comment/number mapping.
* `XsTextMateHighlightingTest` verifies the TextMate bundle format and a few scope names.

There are no tests for LSP semantic tokens because the plugin/server do not implement them.
Proposed additions:

* Update `XsColorSettingsPageTest` for `"XS"` display name and the expanded descriptor list.
* Update `XsSyntaxHighlighterFactoryTest` for braces/operators/built-in-type keys.
* Add a unit test for the LSP semantic-token mapper (plugin side) once a converter exists.
* Extend the Rust roundtrip test with a fixture that asserts the server returns expected token
  kinds for engine, unmodded, and modded symbols.

## pluginVersion

Current: `0.2.3` (`tools/intellij-xs-plugin/gradle.properties:12`).

Planned bump: `0.3.0` (MINOR) per AGENTS.md plugin version bump policy for "new settings UI".

## Audit cross-reference

Issue #3 is not listed in `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md`. It is new UX
feedback, not an audit finding.

## Scope recommendation for proposal phase

* **Files to change:**
  * Plugin: `XsTextAttributesKeys.kt`, `XsColorSettingsPage.kt`, `XsSyntaxHighlighter.kt`,
    `XsHighlightingLexer.kt`, `XsLspServerDescriptor.kt`, `plugin.xml`,
    `XsColorSettingsPageTest.kt`, `XsSyntaxHighlighterFactoryTest.kt`, `gradle.properties`.
  * LSP: `server.rs`, `symbols.rs`, plus a new `semantic_tokens.rs` (or equivalent module).
* **New files:** `tools/xs-language-server/src/semantic_tokens.rs` (suggested). A bundled color
  scheme XML is **not** recommended unless fixed defaults are explicitly desired.
* **LOC delta:** plugin +~120–200 lines; LSP +~300–500 lines; tests +~100–200 lines. Total
  +~500–900 lines.
* **Risk:** medium. The LSP semantic-token endpoint must perform well on a workspace containing the
  full vanilla `game/` tree plus mod overlays. Plugin-side mapping depends on the platform LSP
  semantic-tokens API, which is relatively new and version-sensitive.
* **Effort:** approximately 8–16 hours, depending on how many symbol categories the proposal
  commits to versus defers.
* **LSP changes needed:** yes. The new categories cannot be populated from existing data without a
  server-side semantic-token provider (or equivalent symbol-kind endpoint).
* **Audit overlap:** none.

## Risks / unknowns

1. The exact `LspSemanticTokensSupport` API signatures in IntelliJ Platform 2024.2 are not fully
   documented in the public docs explored here. The source/classes should be inspected in the
   plugin's resolved dependency before final design.
2. It is unclear whether TextMate highlighting or the custom `SyntaxHighlighter` wins for `.xs`
   files at runtime. If TextMate remains dominant, brace/operator color changes may not be visible
   until the TextMate bundle is also updated (or its scopes are remapped).
3. Local-variable classification in the LSP server has no existing implementation; adding it is
   the largest open work item.
4. Class symbols are not extracted today. Adding `SymbolKind::Class` is straightforward, but class
   *usage* highlighting (not just declarations) requires additional AST analysis.
5. The `xs` → `XS` rename is safe for stored color-scheme data because schemes persist by
   `TextAttributesKey` external name, not by page display name; however, any screenshots/docs that
   reference the old label will need updating.

## Next step

The proposal phase should decide whether to bundle the LSP semantic-tokens work with the plugin
settings-UI work or split it, and then specify the exact set of LSP token types/modifiers and the
plugin-side `TextAttributesKey` mapping that satisfies each requested category.
