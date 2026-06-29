# Proposal: expand-color-scheme-categories

## Status

Draft

## Background

Issue #3 in `docs/issues/2026-06-29-runtime-issues.md` asks for a richer XS color scheme in Rider/IntelliJ: the settings menu label should be `XS` instead of `xs`, and the color-settings page should expose many more categories than the current six (Comment, Default, Identifier, Keyword, Number, String). The requested categories fall into three groups: semantic-token-driven symbol distinctions (engine / modded / unmodded functions, variables, constants, types), built-in type highlighting, and inherited Rider `General` / `Language Defaults` categories (identifier-under-caret, matched/unmatched brace, unknown symbol, braces/operators).

The explore report confirmed that the plugin is currently a thin LSP client with only a lexical `SyntaxHighlighter` and TextMate bundle. The XS LSP server does **not** advertise `textDocument/semanticTokens`, and there is no plugin-side semantic-token-to-`TextAttributesKey` converter. Several of the requested categories therefore cannot be populated today without substantial LSP work (local-variable scope extraction, class extraction, origin-path tagging for modded/unmodded). Keeping this change scoped prevents pulling in an unplanned LSP feature.

## Scope decision

**Choose Option α — Bucket A + Bucket B only.**

- Bucket A: declare inherited Rider `General` / `Language Defaults` categories as plugin `TextAttributesKey`s.
- Bucket B: rename the color-settings page display name from `"xs"` to `"XS"`.
- Bucket C (semantic-token-driven categories) is deferred to a follow-up change.

Justification:

- The full user ask requires LSP changes that are separate from the plugin color-settings UI work.
- Option α delivers visible, user-customizable categories in approximately two hours with near-zero regression risk.
- Option α does not block Options β or γ; the new `TextAttributesKey`s declared here can be reused by the semantic-token mapper later.
- The explore report explicitly recommends splitting the LSP semantic-token work from the plugin UI work.

## User-facing contract (under α)

- **Settings → Editor → Color Scheme → XS** is shown with an uppercase label (renamed from `xs`).
- The existing six categories remain available and functional:
  - Comment, Default, Identifier, Keyword, Number, String.
- New categories appear under the XS color-settings page, inherited from standard Rider/IntelliJ keys:
  - **Code / Identifier under caret**
  - **Code / Matched brace**
  - **Code / Unmatched brace**
  - **Errors and Warnings / Unknown symbol**
  - **Braces and Operators / Braces**
  - **Braces and Operators / Brackets**
  - **Braces and Operators / Comma**
  - **Braces and Operators / Dot**
  - **Braces and Operators / Operation sign**
  - **Braces and Operators / Overloaded operator**
  - **Braces and Operators / Parentheses**
  - **Braces and Operators / Semi-colon**
- Each new category inherits its default colors from the active IDE scheme, so it is immediately customizable without bundling fixed colors.
- Braces, brackets, parens, commas, semicolons, dots, and built-in operators that are already lexed by the plugin are mapped to the new operator/bracket keys where the existing highlighter currently maps them to `XS_DEFAULT` or keyword.

## Scope

### In scope (α)

- Rename `XsColorSettingsPage.getDisplayName()` from `"xs"` to `"XS"`.
- Declare ~13 new `TextAttributesKey`s in `XsTextAttributesKeys.kt`, each falling back to a standard IntelliJ/Rider key:
  - `XS_IDENTIFIER_UNDER_CARET`
  - `XS_MATCHED_BRACE`
  - `XS_UNMATCHED_BRACE`
  - `XS_UNKNOWN_SYMBOL`
  - `XS_BRACES`
  - `XS_BRACKETS`
  - `XS_COMMA`
  - `XS_DOT`
  - `XS_OPERATION_SIGN`
  - `XS_OVERLOADED_OPERATOR`
  - `XS_PARENTHESES`
  - `XS_SEMI_COLON`
- Add corresponding `AttributesDescriptor` entries in `XsColorSettingsPage` and update `DEMO_TEXT` / `TAG_MAP` to demonstrate the new categories.
- Wire the existing lexer tokens for braces, brackets, parens, comma, semicolon, dot, and operators to the new keys in `XsSyntaxHighlighter` (and `XsHighlightingLexer` if built-in types/operators need to be distinguished from keywords).
- Add or update plugin tests to assert:
  - display name is `"XS"`;
  - the new descriptors / keys are registered;
  - braces/operators map to the inherited keys.
- Bump `pluginVersion` from `0.2.3` to `0.3.0` per `AGENTS.md` MINOR policy for a new settings UI.

### Out of scope (deferred to follow-up changes)

- LSP server-side `textDocument/semanticTokens` capability.
- Origin-path classification (engine / modded / unmodded) for any symbol.
- Symbol-kind categories: Engine/UnModded/Modded function, rule, constant, variable, class.
- Scope-based categories: Local variable, Static variable, extern variable.
- Built-in type as a dedicated semantic category (under α it is a lexical category only; no user-visible distinction from keyword is required here).
- Plugin-side semantic-token-to-`TextAttributesKey` converter.
- Issue #1, Issue #2, and Issue #4 from `docs/issues/2026-06-29-runtime-issues.md`.

### Audit overlap

None. Issue #3 is not listed in `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md`. It is new UX feedback, not an audit finding.

## Impact (under α)

- **Files changed:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributesKeys.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt` (if built-in type/operator token split is needed)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterFactoryTest.kt`
  - `tools/intellij-xs-plugin/gradle.properties`
- **New files:** 0.
- **LOC delta:** +~50–100 / -~5–10.
- **Risk:** low.
- **Effort:** ~2 hours.
- **Compatibility risk:** low. Adding `TextAttributesKey`s and descriptors is additive; existing schemes persist keys by external name, and the display-name change does not affect persisted color data.
- **Rollback plan:** revert the changed Kotlin/properties files.
- **Patch-maintenance impact:** none — this is plugin tooling only; no mod overlay script changes.
- **pluginVersion bump:** `0.2.3 → 0.3.0` (per `AGENTS.md` MINOR policy: new settings UI / new LSP feature / new grammar scope).

## Success criteria

- Settings → Editor → Color Scheme shows **XS** (not `xs`).
- The 12–13 new `TextAttributesKey`s are registered and visible in the color-settings page under XS.
- The existing six categories (Comment, Default, Identifier, Keyword, Number, String) still appear and retain their colors.
- Braces/brackets/parens/operators/commas/semicolons/dots in `.xs` files are highlighted using the new inherited categories when no overriding scheme is set.
- `./gradlew buildPlugin` produces a valid plugin `.zip`.
- Updated or new IntelliJ Platform tests pass.
- Color settings persist across IDE restarts (schemes are persisted by key external name, which is unchanged for existing keys).

## Future work (β and γ — documented, deferred)

### Option β — Bucket A + B + simplified C

- Add an LSP `textDocument/semanticTokens` endpoint that classifies only functions, constants, and types by origin (engine / modded / unmodded).
- Add plugin-side semantic-token mapping for those categories only.
- Skip local/static/extern variable categories and class categories (they require parser/scope work).
- Estimated LOC delta: +~400–600 (LSP + plugin); risk: medium; effort: ~2–3 days.

### Option γ — Full Bucket A + B + C

- Implement the complete requested taxonomy, including local-variable scope extraction, `static`/`extern` flag exposure, class extraction, and origin-path tagging for every symbol kind.
- Add full LSP semantic tokens and plugin-side converter.
- Estimated LOC delta: +~1000–1500; risk: medium-high; effort: ~1–2 weeks.

## Risks and unknowns

- The exact `EditorColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES`, `CodeInsightColors.MATCHED_BRACE_ATTRIBUTES`, etc. constants are public API, but the names should be verified against the bundled IntelliJ Platform 2024.2 dependency at design/apply time.
- It is unclear whether the custom `XsSyntaxHighlighter` or the TextMate bundle wins at runtime for `.xs` files. If TextMate remains dominant for braces/operators, the user may not see the new colors until TextMate scopes are also remapped or TextMate is disabled for the language.
- The `xs` → `XS` rename is safe for persisted color-scheme data but may require updating any screenshots or documentation that reference the old label.
- Some categories (e.g., `XS_UNKNOWN_SYMBOL`) will not be populated by the plugin today; they still appear in settings and fall back to the platform default.

## Open questions

1. Should `XsColorSettingsPage.DEMO_TEXT` demonstrate every new category, or only a representative subset to keep the preview readable?
2. Should the existing built-in types (`bool`, `int`, `float`, `string`, `vector`) remain under `XS_KEYWORD` for α, or should they move to a new `XS_BUILTIN_TYPE` key? (The user’s full list includes `Built-in type`, but under α this is a lexical decision and not tied to LSP semantic tokens.)
3. Do we want to expose a bundled default color-scheme XML for fixed defaults, or rely entirely on key inheritance? The explore report recommends inheritance to avoid scheme-compatibility breakage.
4. Are there existing TextMate scope-to-key mappings that already cover braces/operators, and should the plugin reuse those rather than duplicate in `XsSyntaxHighlighter`?

## Next step

The spec phase should formalize the exact set of new `TextAttributesKey`s, their fallback keys, the `AttributesDescriptor` labels and grouping, and the lexer token-to-key mapping, then write strict-TDD test scenarios for display-name, descriptor registration, and operator/bracket highlighting.
