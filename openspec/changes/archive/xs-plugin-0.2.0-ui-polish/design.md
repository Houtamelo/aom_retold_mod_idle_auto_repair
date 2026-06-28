# Design: XS IntelliJ Plugin 0.2.0 UI Polish

## Technical Approach

Fix the plugin's editor UI by unifying the IntelliJ `Language` ID to lowercase `"xs"` (matching the bundled TextMate bundle), adding a small native syntax highlighter as a fallback, and registering a `ColorSettingsPage`. The work stays entirely within `tools/intellij-xs-plugin/`; no XS mod scripts or LSP server code change.

## Architecture Decisions

### Decision: Lowercase language ID for `XsLanguage` and `plugin.xml`

| Option | Trade-off | Decision |
|--------|-----------|----------|
| Rename `XsLanguage` to `"xs"` and update `plugin.xml` | Touches every language extension; avoids case-sensitive TextMate mismatch | **Chosen** |
| Keep `Language("XS")` and change TextMate `language` to `"XS""` | Smaller diff, violates IntelliJ/VSCode lowercase convention | Rejected |

**Implementation**: change `XsLanguage.kt` constructor argument from `"XS"` to `"xs"`, then update every `language="XS"` attribute in `plugin.xml` to `language="xs"`. A pre-flight test scans `plugin.xml` and fails if any uppercase language reference remains.

### Decision: Native syntax highlighter fallback

| Option | Trade-off | Decision |
|--------|-----------|----------|
| New `XsSyntaxHighlighterFactory` + small highlighting lexer | Adds safety net; only used when TextMate bundle fails | **Chosen** |
| Rely only on TextMate | Leaves highlighting broken if the bundle association fails again | Rejected |

**Implementation**: add `XsSyntaxHighlighterFactory`, `XsSyntaxHighlighter`, and `XsHighlightingLexer` under `com.aomr.xs.highlight`. The lexer tokenizes keywords, line/block comments, string/char literals, numeric literals, identifiers, and default text, returning `IElementType`s mapped to `TextAttributesKey`s. Keywords are the XS reserved words defined in `docs/xs-language-syntax.md` (`void`, `int`, `bool`, `float`, `string`, `if`, `else`, `while`, `for`, `return`, `mutable`, `extern`, etc.).

### Decision: `ColorSettingsPage` under Color Scheme settings

| Option | Trade-off | Decision |
|--------|-----------|----------|
| `XsColorSettingsPage` with `AttributesDescriptor`s | Standard JetBrains UX; live preview included | **Chosen** |
| `additionalTextAttributes` only | Lighter weight, but no Settings page entry | Rejected |

**Implementation**: add `XsColorSettingsPage` in the same `highlight` package. It returns display name `"xs"`, descriptors for `Keyword`, `String`, `Comment`, `Number`, `Identifier`, and `Default`, and a demo XS snippet with tags such as `<keyword>void</keyword>`. Tag-to-key mapping is provided by `getAdditionalHighlightingTagToDescriptorMap()`.

## Data Flow

```
IDE startup
  XsFileTypeFactory.createFileTypes() ──→ registers XsFileType for "xs"

Open foo.xs
  FileTypeManager ──→ XsFileType ──→ XsLanguage(id="xs")
  Editor asks SyntaxHighlighterFactory for language "xs"
    XsSyntaxHighlighterFactory ──→ XsSyntaxHighlighter
      getHighlightingLexer() ──→ XsHighlightingLexer
      tokens ──→ XsTextAttributesKeys ──→ editor colors

Settings → Editor → Color Scheme
  ColorSettingsPage extension for "xs" ──→ XsColorSettingsPage
    descriptors + demo text + tag map ──→ preview panel
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/XsLanguage.kt` | Modify | Change `Language("XS")` → `Language("xs")`. |
| `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` | Modify | Update language IDs; add `<lang.syntaxHighlighterFactory>` and `<colorSettingsPage>` extensions. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributesKeys.kt` | Create | `TextAttributesKey` constants backed by `DefaultLanguageHighlighterColors`. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt` | Create | Display-only lexer classifying keyword/string/comment/number/identifier/default tokens. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt` | Create | Maps lexer token types to the keys above. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterFactory.kt` | Create | Factory registered in `plugin.xml`. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt` | Create | Color Scheme settings page with descriptors and demo text. |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/` | Create | `XsLanguageIdTest`, `XsSyntaxHighlighterFactoryTest`, `XsColorSettingsPageTest`. |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/XsTextMateBundleFormatTest.kt` | Modify | Assert bundle grammar `language` matches `XsLanguage.ID`. |
| `tools/intellij-xs-plugin/gradle.properties` | Modify | Bump `pluginVersion` from `0.1.6` to `0.2.0`. |

## Interfaces / Contracts

```kotlin
// Text attributes shared by highlighter and color page
object XsTextAttributesKeys {
    val XS_KEYWORD: TextAttributesKey = createTextAttributesKey("XS_KEYWORD", KEYWORD)
    val XS_STRING:  TextAttributesKey = createTextAttributesKey("XS_STRING", STRING)
    val XS_COMMENT: TextAttributesKey = createTextAttributesKey("XS_COMMENT", LINE_COMMENT)
    val XS_NUMBER:  TextAttributesKey = createTextAttributesKey("XS_NUMBER", NUMBER)
    val XS_IDENTIFIER: TextAttributesKey = createTextAttributesKey("XS_IDENTIFIER", IDENTIFIER)
    val XS_DEFAULT: TextAttributesKey = createTextAttributesKey("XS_DEFAULT", DEFAULT)
}

class XsSyntaxHighlighterFactory : SyntaxHighlighterFactory() {
    override fun getSyntaxHighlighter(project: Project?, virtualFile: VirtualFile?): SyntaxHighlighter
}

class XsSyntaxHighlighter : SyntaxHighlighterBase() {
    override fun getHighlightingLexer(): Lexer
    override fun getTokenHighlights(tokenType: IElementType?): Array<TextAttributesKey>
}

class XsColorSettingsPage : ColorSettingsPage {
    override fun getDisplayName(): String
    override fun getAttributeDescriptors(): Array<AttributesDescriptor>
    override fun getDemoText(): String
    override fun getAdditionalHighlightingTagToDescriptorMap(): Map<String, TextAttributesKey>
}
```

## Testing Strategy

| Layer | What to Test | Approach |
|-------|--------------|----------|
| Unit | Language ID lowercase and no uppercase `plugin.xml` references | `XsLanguageIdTest` loads `XsLanguage.INSTANCE.id` and scans `META-INF/plugin.xml` as a resource |
| Unit | Native highlighter registration and token mapping | `XsSyntaxHighlighterFactoryTest` tokenizes a sample `.xs` string and asserts each token maps to the expected `TextAttributesKey` |
| Unit | Color settings page content | `XsColorSettingsPageTest` resolves the page and asserts descriptors cover keyword/string/comment/number/identifier/default and demo text is non-empty |
| Integration | TextMate bundle still registers correctly | Run existing `XsTextMateBundleFormatTest` and `XsTextMateHighlightingTest`; add an assertion that bundle `language == XsLanguage.ID` |
| Manual | Visual highlighting and settings page | Install the plugin in Rider/IDEA, open an `.xs` file, and open **Settings → Editor → Color Scheme → xs** |

## Migration / Rollout

No runtime migration is required. Rollback is a pure revert: restore `XsLanguage("XS")`, restore `language="XS"` entries, remove the two new `plugin.xml` extensions, delete the `highlight/` source and test directories, and restore `pluginVersion`. No mod files are affected.

## Implementation Order

Strict TDD: each decision below starts with a failing test.

1. **AD-1 Language ID unification** — failing test asserts lowercase language ID and no uppercase `plugin.xml` references; then apply code change.
2. **AD-2 Native syntax highlighter** — failing test asserts a `SyntaxHighlighterFactory` for `"xs"` produces expected token keys; then add lexer/highlighter/factory.
3. **AD-3 Color settings page** — failing test asserts the `"xs"` color page exposes the required descriptors and demo text; then add `XsColorSettingsPage` and register it.

## Risks & Open Questions

- **TextMate precedence**: `XsTextMateBundleFormatTest` and `XsTextMateHighlightingTest` must keep passing; the native highlighter is only a fallback. Verify manually that TextMate colors still win when the bundle loads.
- **Color settings UI availability**: `ColorSettingsPage` depends on the color-scheme settings UI present in Rider and IDEA Ultimate. Because `plugin.xml` already declares `com.intellij.modules.ultimate`, this is safe, but should be confirmed on the target build.
- **Foreign-uid Gradle build directories**: as noted in `openspec/config.yaml`, Gradle may leave `build/`/`dist/` owned by another uid; run `sudo chown -R $(id -un):$(id -gn) tools/intellij-xs-plugin/{build,dist}` if `./gradlew :test` fails with permission errors.
- **Display name capitalization**: spec allows the color page display name to be `"xs"` or `"XS"`; this design chooses `"xs"` for consistency with the language ID.
