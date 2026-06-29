# Design: `expand-color-scheme-categories`

## Technical Approach

Declare 12 plugin-local `TextAttributesKey`s that inherit from standard Rider/IntelliJ defaults, register them on the existing `XsColorSettingsPage`, and rename the page from `"xs"` to `"XS"`. No TextMate grammar, lexer, or LSP changes are made under Option α.

## Architecture Decisions

| ADR | Choice | Rationale |
|---|---|---|
| ADR-1 — Minimal wiring | Only declare keys + register descriptors; leave `XsSyntaxHighlighter` and `xs.tmLanguage.json` unchanged. | Option α is intentionally small. The spec’s fallback clause for unmapped tokens is satisfied, and a larger grammar/highlighter refactor is avoided. |
| ADR-2 — Display name | `"XS"`. | Matches the user request and keeps the label short. |
| ADR-3 — Grouping | `//` nested groups: `Code`, `Errors and Warnings`, `Braces and Operators`. | Standard IntelliJ `AttributesDescriptor` convention for color scheme sub-categories. |
| ADR-4 — Unknown symbol fallback | `CodeInsightColors.WRONG_REFERENCES_ATTRIBUTES`. | Issue #3 describes "Unknown symbol" as the red indicator for unresolved references, which is exactly this platform key. |
| ADR-5 — Overloaded operator fallback | `DefaultLanguageHighlighterColors.OPERATION_SIGN`. | No dedicated `OVERLOADED_OPERATOR` key exists; the new XS category still appears separately while inheriting operation-sign defaults. |

## Data Flow

```
META-INF/plugin.xml (existing <colorSettingsPage>)
         │
         ▼
XsColorSettingsPage.getAttributeDescriptors()
         │
         ▼
XsTextAttributes.<KEY> ──fallback──▶ platform EditorColors / CodeInsightColors / DefaultLanguageHighlighterColors
         │
         ▼
IDE renders categories under Settings → Editor → Color Scheme → XS
```

## File Changes

| File | Action | Description |
|---|---|---|
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt` | Create | Object with the 12 inherited `TextAttributesKey`s. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt` | Modify | Rename display name to `"XS"`; add 12 descriptors; extend `TAG_MAP`/`DEMO_TEXT`. |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt` | Modify | Assert uppercase name and the 12 new descriptors. |
| `tools/intellij-xs-plugin/gradle.properties` | Modify | `pluginVersion` `0.2.3 → 0.3.0` per AGENTS.md minor policy. |
| `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` | No change | `<colorSettingsPage>` already registered. |

## Interfaces / Contracts

### `XsTextAttributes.kt`

```kotlin
package com.aomr.xs.highlight

import com.intellij.openapi.editor.DefaultLanguageHighlighterColors
import com.intellij.openapi.editor.colors.CodeInsightColors
import com.intellij.openapi.editor.colors.EditorColors
import com.intellij.openapi.editor.colors.TextAttributesKey
import com.intellij.openapi.editor.colors.TextAttributesKey.createTextAttributesKey

object XsTextAttributes {
    val IDENTIFIER_UNDER_CARET = createTextAttributesKey("XS_IDENTIFIER_UNDER_CARET", EditorColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES)
    val MATCHED_BRACE = createTextAttributesKey("XS_MATCHED_BRACE", CodeInsightColors.MATCHED_BRACE_ATTRIBUTES)
    val UNMATCHED_BRACE = createTextAttributesKey("XS_UNMATCHED_BRACE", CodeInsightColors.UNMATCHED_BRACE_ATTRIBUTES)
    val UNKNOWN_SYMBOL = createTextAttributesKey("XS_UNKNOWN_SYMBOL", CodeInsightColors.WRONG_REFERENCES_ATTRIBUTES)
    val BRACES = createTextAttributesKey("XS_BRACES", DefaultLanguageHighlighterColors.BRACES)
    val BRACKETS = createTextAttributesKey("XS_BRACKETS", DefaultLanguageHighlighterColors.BRACKETS)
    val COMMA = createTextAttributesKey("XS_COMMA", DefaultLanguageHighlighterColors.COMMA)
    val DOT = createTextAttributesKey("XS_DOT", DefaultLanguageHighlighterColors.DOT)
    val OPERATION_SIGN = createTextAttributesKey("XS_OPERATION_SIGN", DefaultLanguageHighlighterColors.OPERATION_SIGN)
    val OVERLOADED_OPERATOR = createTextAttributesKey("XS_OVERLOADED_OPERATOR", DefaultLanguageHighlighterColors.OPERATION_SIGN)
    val PARENTHESES = createTextAttributesKey("XS_PARENTHESES", DefaultLanguageHighlighterColors.PARENTHESES)
    val SEMI_COLON = createTextAttributesKey("XS_SEMI_COLON", DefaultLanguageHighlighterColors.SEMICOLON)
}
```

### `XsColorSettingsPage.kt` updates

```kotlin
override fun getDisplayName(): String = "XS"

private val DESCRIPTORS = arrayOf(
    // existing six
    AttributesDescriptor("Keyword", XsTextAttributesKeys.XS_KEYWORD),
    AttributesDescriptor("String", XsTextAttributesKeys.XS_STRING),
    AttributesDescriptor("Comment", XsTextAttributesKeys.XS_COMMENT),
    AttributesDescriptor("Number", XsTextAttributesKeys.XS_NUMBER),
    AttributesDescriptor("Identifier", XsTextAttributesKeys.XS_IDENTIFIER),
    AttributesDescriptor("Default", XsTextAttributesKeys.XS_DEFAULT),
    // new inherited categories
    AttributesDescriptor("Code//Identifier under caret", XsTextAttributes.IDENTIFIER_UNDER_CARET),
    AttributesDescriptor("Code//Matched brace", XsTextAttributes.MATCHED_BRACE),
    AttributesDescriptor("Code//Unmatched brace", XsTextAttributes.UNMATCHED_BRACE),
    AttributesDescriptor("Errors and Warnings//Unknown symbol", XsTextAttributes.UNKNOWN_SYMBOL),
    AttributesDescriptor("Braces and Operators//Braces", XsTextAttributes.BRACES),
    AttributesDescriptor("Braces and Operators//Brackets", XsTextAttributes.BRACKETS),
    AttributesDescriptor("Braces and Operators//Comma", XsTextAttributes.COMMA),
    AttributesDescriptor("Braces and Operators//Dot", XsTextAttributes.DOT),
    AttributesDescriptor("Braces and Operators//Operation sign", XsTextAttributes.OPERATION_SIGN),
    AttributesDescriptor("Braces and Operators//Overloaded operator", XsTextAttributes.OVERLOADED_OPERATOR),
    AttributesDescriptor("Braces and Operators//Parentheses", XsTextAttributes.PARENTHESES),
    AttributesDescriptor("Braces and Operators//Semi-colon", XsTextAttributes.SEMI_COLON)
)
```

`TAG_MAP` and `DEMO_TEXT` are extended to demonstrate the new categories with tags such as `<brace>`, `<bracket>`, `<paren>`, `<comma>`, `<semicolon>`, `<dot>`, and `<op>`.

## Testing Strategy

| Layer | What | Approach |
|---|---|---|
| Unit | Color page display name and descriptors | `XsColorSettingsPageTest` asserts `displayName == "XS"` and the 12 new keys are in `attributeDescriptors`. |
| Integration | Full plugin suite | `./gradlew :test` green. |
| Build | Plugin artifact | `./gradlew :buildPlugin` produces `.../intellij-xs-plugin-0.3.0.zip`. |
| Manual | Settings UI | Verify uppercase label, nested groups, and preview. |

## Strict-TDD Task List

1. Add `displayNameIsUppercaseXS` to `XsColorSettingsPageTest`; run → FAIL.
2. Add `descriptorsIncludeInheritedCategories` asserting the 12 keys; run → FAIL.
3. Run targeted tests; confirm FAIL.
4. Create `XsTextAttributes.kt` with the 12 keys.
5. Update `XsColorSettingsPage.kt`: rename display name, add 12 descriptors, extend `TAG_MAP`/`DEMO_TEXT`.
6. Run targeted tests; confirm PASS.
7. Run `./gradlew :test`; confirm green.
8. Bump `pluginVersion` to `0.3.0`.
9. Run `./gradlew :buildPlugin`; confirm `.zip` produced.
10. Inspect `.zip`: `META-INF/plugin.xml` contains `<colorSettingsPage>` and version `0.3.0`.

## Migration / Rollout

No migration. Colors persist by `TextAttributesKey` external name; existing XS keys are untouched.

## Out-of-Scope Reminders

- Issues 1, 2, and 4 from `docs/issues/2026-06-29-runtime-issues.md` are not addressed.
- No changes to `tools/xs-language-server/`.
- Semantic-token-driven categories are deferred.
- TextMate grammar (`xs.tmLanguage.json`) is unchanged under Option α.

## Open Questions

None.
