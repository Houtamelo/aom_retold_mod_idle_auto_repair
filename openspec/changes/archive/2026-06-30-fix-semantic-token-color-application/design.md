# Design: Fix semantic-token color application in editor

## Technical Approach

The fix is plugin-side only. Three coordinated changes make the LSP-driven semantic-tokens annotation the sole source of identifier and primitive-type coloring:

1. Override `XsSemanticTokensSupport.tokenModifiers` to mirror the LSP legend so modifier bitsets survive platform-side filtering.
2. Remove the `XsSyntaxHighlighter.IDENTIFIER → XS_IDENTIFIER` default so identifiers are not pre-colored by the syntax-highlighter pass.
3. Reparent `TYPE_BUILTIN` from `KEYWORD` to `IDENTIFIER` and strip the TextMate `storage_types` rule so primitive-type keywords are not pre-colored either.

## Architecture Decisions

### Decision: tokenModifiers legend parity

**Choice**: Override `XsSemanticTokensSupport.tokenModifiers` to return the same seven names that `semantic_tokens::TOKEN_MODIFIERS` advertises, in the same order.

**Alternatives**:

| # | Option | Rejected because |
|---|--------|------------------|
| 1 | SDK default 23-name list | Drops custom modifiers before `getTextAttributesKey` |
| 2 | Empty list | Disables modifier-aware support |
| 3 | Rely on LSP legend alone | SDK filters modifiers against the plugin-side list |

**Rationale**: The platform filters incoming modifiers against `support.tokenModifiers` before `getTextAttributesKey`. Without the override, custom XS modifiers are dropped; only `static` survives from the SDK default list.

### Decision: Lexer-level identifier default removed

**Choice**: `XsSyntaxHighlighter.getTokenHighlights(XsTokenTypes.IDENTIFIER)` returns an empty array instead of `[XS_IDENTIFIER]`.

**Alternatives**:

| # | Option | Rejected because |
|---|--------|------------------|
| 1 | Leave default in place | Pre-empts semantic-token classification |
| 2 | Global-highlighter override | Fragile, fights platform layering |
| 3 | Reset `XS_IDENTIFIER` fall-through only | Locals still render as identifiers and blend |

**Rationale**: A pre-applied `XS_IDENTIFIER` default still acts as the baseline color. Returning an empty array leaves semantic tokens as the only identifier color source.

### Decision: TYPE_BUILTIN no longer inherits from KEYWORD

**Choice**: `TYPE_BUILTIN` inherits from `DefaultLanguageHighlighterColors.IDENTIFIER`.

**Alternatives**:

| # | Option | Rejected because |
|---|--------|------------------|
| 1 | Keep KEYWORD | Always keyword-colored |
| 2 | Fresh key without inheritance | Disrupts scheme migration |
| 3 | Inherit from STATIC_FIELD | Semantically wrong |

**Rationale**: The TYPE_* keys should inherit the platform type-name default. On 2024.2 that is `IDENTIFIER`, matching `TYPE_UNMODDED_CLASS` and `TYPE_MODDED_CLASS`.

### Decision: TextMate `storage_types` rule removed

**Choice**: Delete the `storage_types` repository block in `xs.tmLanguage.json` and the `#storage_types` include references at the top level, in `function-call-innards`, and in `function-innards`.

**Alternatives**:

| # | Option | Rejected because |
|---|--------|------------------|
| 1 | Rename scope to `entity.name.type.xs` | No reliable IDE mapping |
| 2 | Add higher-priority rule | TextMate has no layer priority |
| 3 | Override SDK mapping via component | Too invasive |

**Rationale**: `storage.type.built-in.primitive.c` maps to the keyword color and wins over later semantic-token annotations. Removing the rule lets primitive types fall through to identifier-style scopes and be recolored by semantic tokens.

## Data Flow

```
editor buffers a .xs file
   │
   │ 1. Lexer pass emits IDENTIFIER tokens
   │    ↓
   │ 2. Highlighting lexer maps primitive names to KEYWORD
   │    ↓
   │ 3. SyntaxHighlighter:
   │      KEYWORD            → [XS_KEYWORD]
   │      IDENTIFIER         → []
   │      LBRACE/RBRACE      → [BRACES]
   │    ↓
   │ 4. TextMate pass; primitive types stay unscoped
   │    ↓
   │ 5. Semantic-tokens pass
   │      tokenModifiers matches LSP legend → modifiers survive
   │      Converter maps (type, mods) to TextAttributesKey
   │      Platform creates TEXT_ATTRIBUTES annotation per range
   │    ↓
   │ 6. Compositor paints by highest-priority annotation.
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt` | Modify | Override `tokenModifiers` with LSP legend. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt` | Modify | `IDENTIFIER` returns empty. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt` | Modify | `TYPE_BUILTIN` inherits `IDENTIFIER`. |
| `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` | Modify | Remove `storage_types` rule and includes. |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupportTest.kt` | Create | Test legend parity. |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterTest.kt` | Modify | Assert `IDENTIFIER` empty. |
| `tools/intellij-xs-plugin/gradle.properties` | Modify | Bump to `0.8.2`. |

## Interfaces / Contracts

```kotlin
// tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt
internal class XsSemanticTokensSupport : LspSemanticTokensSupport() {
    override val tokenTypes: List<String> = listOf(
        "function", "variable", "type", "constant", "rule",
    )
    override val tokenModifiers: List<String> = listOf(
        "engine", "modded", "unmodded", "local", "static", "extern", "member",
    )
    override fun getTextAttributesKey(tokenType: String, modifiers: List<String>): TextAttributesKey =
        XsSemanticTokensConverter.convert(tokenType, modifiers.toSet())
}
```

`XsSemanticTokensConverter.convert` is unchanged; it now receives the full modifier list.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|--------------|----------|
| Unit | `XsSemanticTokensSupport.tokenModifiers` parity | `assertEquals` |
| Unit | `XsSyntaxHighlighter.getTokenHighlights(IDENTIFIER)` empty | `assertEquals(0, keys.size)` |
| Unit | `XsTextAttributes.TYPE_BUILTIN` not `KEYWORD` | Walk fallback chain |
| Unit | `xs.tmLanguage.json` has no `storage_types` scope | Assert no matching scope |
| Existing | `XsColorSettingsPageTest` 7/7 green | Re-run assertions |
| E2E | Colors for `int`, `PlayerInfo`, `someFn`, `MAX_UNITS` | Manual Rider smoke test |

## Migration / Rollout

No migration required. Plugin-only, no data files, no protocol changes.

## Open Questions

None.
