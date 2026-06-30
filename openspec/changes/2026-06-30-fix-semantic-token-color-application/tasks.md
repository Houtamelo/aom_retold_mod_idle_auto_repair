# Tasks: Fix semantic-token color application in editor

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~60–110 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | single PR |
| Delivery strategy | auto-forecast |
| Chain strategy | pending |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Identifiers + built-in types render with semantic-token categories | PR 1 | single mixed commit acceptable |

## Phase 1 — Test contracts (RED)

- [x] 1.1 Create `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupportTest.kt` and assert `tokenTypes`/`tokenModifiers` match the LSP legend.
- [x] 1.2 In `XsSyntaxHighlighterTest.kt` add `identifier_returns_no_text_attributes` asserting `XsSyntaxHighlighter().getTokenHighlights(XsTokenTypes.IDENTIFIER)` is empty.
- [x] 1.3 In `XsColorSettingsPageTest.kt` add `type_builtin_not_keyword` asserting `XsTextAttributes.TYPE_BUILTIN` fallback is not `DefaultLanguageHighlighterColors.KEYWORD`.
- [x] 1.4 In `XsTextMateBundleFormatTest.kt` add `grammar_has_no_storage_types_rule` asserting `xs.tmLanguage.json` has no `storage_types` repository block or primitive-type scope.

## Phase 2 — Implementation (GREEN)

- [x] 2.1 `XsSemanticTokensSupport.kt` — override `tokenModifiers` with `engine`, `modded`, `unmodded`, `local`, `static`, `extern`, `member` in that order.
- [x] 2.2 `XsSyntaxHighlighter.kt` — change the `XsTokenTypes.IDENTIFIER` branch to return `EMPTY_KEYS`.
- [x] 2.3 `XsTextAttributes.kt` — reparent `TYPE_BUILTIN` from `DefaultLanguageHighlighterColors.KEYWORD` to `IDENTIFIER`.
- [x] 2.4 `xs.tmLanguage.json` — remove the `storage_types` repository block and the three `#storage_types` includes.

## Phase 3 — Plugin build + tests

- [x] 3.1 Run `./gradlew :compileKotlin` (offline). Fix any compile errors.
- [x] 3.2 Run `./gradlew :test --offline`. All tests, including the four new ones, must pass.
- [x] 3.3 Confirm `XsColorSettingsPageTest` 7/7 and `XsTextMateBundleFormatTest` baseline assertions remain green.

## Phase 4 — Version bump

- [x] 4.1 Bump `tools/intellij-xs-plugin/gradle.properties` `pluginVersion` from `0.8.1` to `0.8.2`.

## Phase 5 — Commit

- [x] 5.1 Single commit: `fix(xs-plugin): apply semantic-token colors to identifiers + built-in types`.
- [x] 5.2 Commit body must note: (a) `tokenModifiers` legend parity, (b) `XsSyntaxHighlighter` ID default removed, (c) `TYPE_BUILTIN` reparented to `IDENTIFIER`, (d) `storage_types` TextMate rule removed, (e) pluginVersion 0.8.2.
- [x] 5.3 Before staging, run `git status` and `git diff` to ensure only plugin-side files are included; exclude unrelated LSP work.

## Phase 6 — Verification handoff

- [x] 6.1 Update `AGENTS.md` plugin test count if the four new tests shifted the baseline (expected 102 → 106).
