# Proposal: fix-color-scheme-quick-wins

## Status
Draft

## Background

After the `expand-color-scheme-categories` cycle (commit `a370e78`,
plugin 0.3.0) shipped Option α — renaming the color-scheme page from
"xs" to "XS" and declaring 12 inherited Rider General / Language
Defaults `TextAttributesKey`s — the user installed the plugin in
Rider and reported three remaining gaps documented in
`docs/issues/2026-06-29-runtime-issues.md` "Rider smoke test
findings (2026-06-30)":

- **3a** — `include` is not being rendered as a keyword.
- **3b** — Braces and operators use the `Default` color, not the
  user-customized Braces/Brackets/Comma/Dot/etc. colors.
- **3c** — Identifier under caret is not working; per-language
  override is not supported in Rider (the customization lives in
  the global General color scheme).

The full Bucket C (engine/modded/unmodded function/variable/constant/
type, local/static, builtin, class) is deferred to a separate cycle
(`add-lsp-semantic-tokens` + `add-plugin-semantic-tokens`).

## Approach

Three small bug fixes that the user already validated the approach for
in the Rider smoke test:

1. **3a** — Add `include` to:
   - The TextMate grammar keyword regex
     (`tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`).
   - The native highlighting lexer's keyword set
     (`tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt`).
2. **3b** — Bind the 8 inherited `TextAttributesKey`s
   (`XsTextAttributes.BRACES`, `BRACKETS`, `COMMA`, `DOT`,
   `OPERATION_SIGN`, `OVERLOADED_OPERATOR`, `PARENTHESES`, `SEMI_COLON`)
   to their respective lexer tokens in
   `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt`.
   Add the missing `XsTokenTypes.DOT`, `SEMI_COLON`, `OPERATION_SIGN`
   token types in `XsTokenType.kt` and their lexer rules in
   `XsLexer.flex`.
3. **3c** — Document the per-language identifier-under-caret
   limitation by extending the descriptor name in
   `XsColorSettingsPage.kt` to include
   "(uses global General — per-language override is not supported in
   Rider)". The test in `XsColorSettingsPageTest.kt` asserts this
   labelling.

## User-facing contract

After this change:

- `include` is highlighted using the Keyword color in `.xs` files.
- Braces `{}`, brackets `[]`, parens `()`, comma, dot, semicolon, and
  operation signs use the user-customized colors from
  **Settings → Editor → Color Scheme → XS > Braces and Operators**.
  They no longer fall back to `Default`.
- The Identifier-under-caret category's display name in the color
  settings page explicitly notes that the per-language override is
  not supported in Rider; the user is directed to the global
  **Color Scheme → General** setting.

## Scope

### In scope

- 3a: TextMate grammar + native lexer (`XsHighlightingLexer`).
- 3b: `XsSyntaxHighlighter` mappings, new `XsTokenTypes`, lexer rules
  in `XsLexer.flex`.
- 3c: `XsColorSettingsPage` descriptor name update + test assertion.
- Strict-TDD tests:
  - `XsTextMateIncludeKeywordTest` (3 tests for 3a)
  - `XsSyntaxHighlighterTest` (8 tests for 3b)
  - `XsColorSettingsPageTest::test_identifier_under_caret_descriptor_documents_global_setting` (3c)
- `pluginVersion` bump: `0.4.0 → 0.4.1` (PATCH per AGENTS.md "bug fix"
  policy — these are fixes to existing functionality, not new
  features).

### Out of scope

- Bucket C (engine/modded/unmodded function/variable/constant/type,
  local/static, builtin, class) — see `add-lsp-semantic-tokens` and
  `add-plugin-semantic-tokens`.
- Cross-file forward-declaration ordering (separate future spec).
- `Identifier under caret` global workaround: the user is expected
  to customize it under **Color Scheme → General**, not under XS.

## Impact

- Files changed:
  - `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`
    (1 line added: `include` to keyword regex)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt`
    (1 line added: `include` to keyword set)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt`
    (7 lines added: 8 token-type to TextAttributesKey mappings)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsTokenType.kt`
    (3 lines added: DOT, SEMI_COLON, OPERATION_SIGN tokens)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsLexer.flex`
    (4 lines added: lexer rules for the new tokens)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt`
    (1 line modified: descriptor name updated to include the global-limitation note)
  - `tools/intellij-xs-plugin/gradle.properties`
    (1 line modified: `pluginVersion 0.4.0 → 0.4.1`)
  - `AGENTS.md` (test count line bumped: 68 → 81)
  - `docs/issues/2026-06-29-runtime-issues.md` (Issue 3 sub-findings
    marked resolved)
- New test files:
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/XsTextMateIncludeKeywordTest.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterTest.kt`
- New test assertion:
  - `XsColorSettingsPageTest::test_identifier_under_caret_descriptor_documents_global_setting`
- LOC delta: ~80 lines across tracked files + ~190 lines in 2 new
  test files.
- Risk: low.
- Effort: 0.5-1 day.
- Compatibility: additive; existing color settings persist.

## Success criteria

- 3a: `include` is highlighted as a keyword in `.xs` files (verified
  by `XsTextMateIncludeKeywordTest::test_include_keyword_in_textmate_grammar`
  + the two `test_native_lexer_recognizes_include_*` tests).
- 3b: All 8 `XsSyntaxHighlighterTest` tests pass. Braces, brackets,
  parens, comma, dot, semicolon, operation signs use the user-
  customized colors.
- 3c: `XsColorSettingsPageTest::test_identifier_under_caret_descriptor_documents_global_setting`
  passes; the descriptor name includes "uses global General".
- Plugin builds: `./gradlew :buildPlugin` produces
  `dist/intellij-xs-plugin-0.4.1.zip` with `pluginVersion=0.4.1`.
- Plugin test count: 80 passing + 1 environmental failure (the
  pre-existing `XsStartupActivityTest.autoDetectedModsAreSavedAndNotified`
  due to untracked mod directories).
- `cargo test --no-fail-fast` still reports 220/220 passing (no LSP
  changes in this slice).

## Risks and unknowns

- **3c may not be a fixable bug**: if the per-language identifier-
  under-caret override truly is a Rider limitation (not a plugin
  bug), the only mitigation is the descriptor-label documentation.
  This is acceptable per the user's directive ("Document this as a
  limitation that we will tackle in a future spec") and the
  explore-phase investigation.
- **TextMate vs native highlighter precedence**: if Rider's TextMate
  scope overrides the native highlighter, the `include` keyword
  colorization might be driven by the TextMate scope (which we did
  fix) rather than the native lexer's `KEYWORD` token (which we also
  fixed). The fix covers both paths; one will win, both are correct.
- **Lexical edge cases**: the new `OPERATION_SIGN` token covers
  `=`, `+`, `-`, `*`, `/`, `%`, `<`, `>`, `!`, `~`, `&`, `|`, `^`.
  Other operators (e.g. `<<=`, `>>=`, `&&`, `||`, `==`, `!=`, `>=`,
  `<=`) are not in the list and may still be miscategorized. The
  verify-phase Rider smoke test should confirm.

## Next step

Run `sdd-verify` with fresh-context adversarial review, then
`sdd-archive` to move the change folder, then commit with the
prescribed Conventional Commits message.
