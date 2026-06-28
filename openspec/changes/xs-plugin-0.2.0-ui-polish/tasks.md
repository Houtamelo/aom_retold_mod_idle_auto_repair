# Tasks: XS IntelliJ Plugin 0.2.0 UI Polish

## Review Workload Forecast

| Field | Value |
|---|---|
| Estimated changed lines | ~280 (130 production Kotlin, 110 tests, 30 plugin.xml/gradle, 10 docs) |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | single PR |
| Delivery strategy | single-pr |
| Chain strategy | size-exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

## Phase 1: Language ID unification

- [x] 1.1 [RED] Add `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsLanguageIdTest.kt`: assert `XsLanguage.INSTANCE.id == "xs"` and scan `META-INF/plugin.xml` for any remaining `language="XS"`.
- [x] 1.2 [GREEN] Change `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/XsLanguage.kt` constructor to `Language("xs")`.
- [x] 1.3 [GREEN] Update every `language="XS"` to `language="xs"` in `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`.

## Phase 2: Native syntax highlighter fallback

- [x] 2.1 [RED] Add `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterFactoryTest.kt`: assert a `SyntaxHighlighterFactory` is registered for `"xs"` and tokenizes keywords, strings, comments, and numbers.
- [x] 2.2 [GREEN] Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributesKeys.kt` with keys for keyword/string/comment/number/identifier/default.
- [x] 2.3 [GREEN] Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt` wrapping the existing `XsLexerAdapter` and classifying keywords, comments, string/char literals, numbers, identifiers, and default text.
- [x] 2.4 [GREEN] Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt` mapping lexer token types to `TextAttributesKey` instances.
- [x] 2.5 [GREEN] Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterFactory.kt` and register it in `plugin.xml` via `<lang.syntaxHighlighterFactory language="xs" implementationClass="..."/>`.

## Phase 3: Color Scheme settings page

- [x] 3.1 [RED] Add `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt`: assert `XsColorSettingsPage` is registered, display name is `"xs"`, and descriptors cover keyword/string/comment/number/identifier/default.
- [x] 3.2 [GREEN] Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt` with `AttributesDescriptor`s, demo text, and a tag-to-descriptor map.
- [x] 3.3 [GREEN] Register `<colorSettingsPage implementation="com.aomr.xs.highlight.XsColorSettingsPage"/>` in `plugin.xml`.

## Phase 4: Verification and archive

- [x] 4.1 Run `./gradlew :test`; if permission errors occur, run `sudo chown -R $(id -un):$(id -gn) tools/intellij-xs-plugin/build`.
- [x] 4.2 Update `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/XsTextMateBundleFormatTest.kt` to assert the grammar `language == XsLanguage.ID`.
- [x] 4.3 Run existing TextMate tests and confirm they still pass.
- [x] 4.4 Bump `pluginVersion` from `0.1.6` to `0.2.0` in `tools/intellij-xs-plugin/gradle.properties`.
- [x] 4.5 Build plugin: `./gradlew buildPlugin -x buildSearchableOptions`.
- [x] 4.6 Copy the produced plugin zip to `dist/` if the project keeps release artifacts there.
- [x] 4.7 Commit per work-unit: language ID change, highlighter/factory, color page, version/build artifacts.

## Phase 5: Manual smoke test

- [ ] 5.1 Install the plugin zip in Rider/IDEA.
- [ ] 5.2 Open a mod `.xs` file; verify tokens are colored.
- [ ] 5.3 Open **Settings → Editor → Color Scheme → xs**; verify the page and preview render.
- [ ] 5.4 Capture screenshots or notes and attach to the change archive/PR.
