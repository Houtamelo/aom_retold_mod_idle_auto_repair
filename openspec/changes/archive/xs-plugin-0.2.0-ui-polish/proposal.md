# Proposal: XS IntelliJ Plugin 0.2.0 UI Polish

## Intent

Make `.xs` files actually look like code in Rider/IDEA by fixing syntax highlighting and adding a Color Scheme settings entry. The plugin already bundles a TextMate grammar, but it is not applied because the IntelliJ language ID (`"XS"`) and the TextMate bundle language (`"xs"`) differ in case; there is also no native highlighter fallback. This change standardizes the language ID to lowercase `xs` and adds the missing editor integrations.

## Scope

### In Scope
- Align `XsLanguage` ID and every `plugin.xml` language reference to lowercase `xs`.
- Fix TextMate grammar association (bundle already declares `"language": "xs"`).
- Add a native `XsSyntaxHighlighterFactory` + `XsSyntaxHighlighter` fallback that maps common XS/TextMate scopes to IntelliJ text attributes.
- Add a `ColorSettingsPage` so **Settings → Editor → Color Scheme → XS** exists.
- Bump plugin version `0.1.6` → `0.2.0`.

### Out of Scope
- Rust LSP server changes (stale diagnostics, parse-error wording, references, definitions) — deferred to `xs-lsp-0.1.7-diagnostics-ux`.
- Regenerating or editing `xs.tmLanguage.json`.
- Mod scripts or deploy logic.

## Capabilities

### New Capabilities
- `xs-plugin-textmate-highlighting`: Standardize the XS language ID and wire both TextMate and native syntax highlighting for `.xs` files.
- `xs-plugin-color-settings-page`: Register a color scheme settings page with attribute descriptors and preview for XS scopes.

### Modified Capabilities
- None (this is purely plugin editor UI; no existing spec requirement changes).

## Approach

1. **Language ID unification**
   - Change `XsLanguage.kt` from `Language("XS")` to `Language("xs")`.
   - Update every `language="XS"` reference in `plugin.xml` to `language="xs"`.
   - Keep the TextMate bundle `package.json` as-is; it already uses lowercase `xs`.

2. **Native highlighter fallback**
   - Implement `XsSyntaxHighlighterFactory` registered via `<lang.syntaxHighlighterFactory>`.
   - The highlighter maps TextMate-style scopes (`keyword.xs`, `string.xs`, `comment.xs`, etc.) to `TextAttributesKey` instances backed by `DefaultLanguageHighlighterColors`.
   - If TextMate takes precedence the native highlighter is only a fallback; if TextMate remains disabled, users still see token coloring.

3. **Color Scheme page**
   - Implement `XsColorSettingsPage` returning display name `XS`, descriptors for each `TextAttributesKey`, and a short preview snippet.
   - Register via `<colorSettingsPage>` in `plugin.xml`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/XsLanguage.kt` | Modified | Language ID changed to lowercase `xs`. |
| `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` | Modified | Update language IDs; add syntax-highlighter and color-settings extensions. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/` | New | `XsSyntaxHighlighter`, `XsSyntaxHighlighterFactory`, `XsColorSettingsPage`, `XsTextAttributesKeys`. |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/` | New | Unit tests for registration and token mapping. |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/` | Modified | Add assertions that bundle language and `XsLanguage.ID` match. |
| `tools/intellij-xs-plugin/gradle.properties` | Modified | Plugin version `0.1.6` → `0.2.0`. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Missing one `language="XS"` update breaks that extension silently. | Med | Add a test that scans `plugin.xml` and asserts no uppercase `XS` language attributes remain. |
| Native highlighter tokenization differs from TextMate, producing inconsistent colors. | Low | Map only broad, stable scopes to default colors; do not reimplement the grammar. |
| TextMate and native highlighter both registering for `xs` could conflict. | Low | Use standard IntelliJ extension points; TextMate generally takes precedence when the bundle loads. |
| Color settings page keys collide with platform defaults. | Low | Prefix keys with `XS_`/`xs.` and keep them scoped to the XS language. |

## Rollback Plan

- Revert `XsLanguage.kt` to `Language("XS")`.
- Revert all `language="xs"` entries in `plugin.xml` to `language="XS"`.
- Remove `<lang.syntaxHighlighterFactory>` and `<colorSettingsPage>` extensions.
- Delete the new `highlight/` source and test directories.
- Revert `gradle.properties` version to `0.1.6`.
- Re-build and reinstall the previous plugin artifact. No mod scripts or game files are touched.

## Dependencies

- None external; only IntelliJ Platform SDK APIs already available via the `lang` module dependency.

## Success Criteria

- [ ] `./gradlew :test` passes, including new highlighting and color-page tests.
- [ ] `XsLanguage.INSTANCE.id == "xs"` and the TextMate bundle declares `"language": "xs"`.
- [ ] Manual smoke test: install the plugin in Rider/IDEA, open an `.xs` file, and observe keywords/comments/strings colored.
- [ ] Manual smoke test: **Settings → Editor → Color Scheme** lists `XS`; preview panel renders and settings changes are applied.

## Version Bump

Per `AGENTS.md` policy, new plugin features require a minor bump.

- **From**: `0.1.6`
- **To**: `0.2.0`
- **Reason**: new user-facing features (syntax highlighting fix and Color Scheme page).
