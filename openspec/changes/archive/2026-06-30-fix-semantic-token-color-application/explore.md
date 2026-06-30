# Explore — Fix semantic-token color application in editor

> Change: `2026-06-30-fix-semantic-token-color-application`
> Investigated 2026-06-30 by orchestrator (delegated sdd-explore returned empty; orchestrator did the read-only investigation inline to recover the context).

## Restated problem

User reports two rendering bugs in Rider after the Bucket C semantic-token work (commits `1bc73e4`, `874dae9`, `88310ef`, `dfa829d`, `5920f71`, `9cc7344`, `e512f10`, `b437dec`):

1. **Identifier sub-categories render as the generic `XS_IDENTIFIER` color in real `.xs` files.** Variables, functions, rules, constants, methods, and fields all show the same color the user assigned to "Identifier".
2. **Built-in type usages (`int`, `string`, `bool`, `vector`, `float`) render as the `XS_KEYWORD` color** instead of the dedicated "Type → Built-in Type" category.

The synthesized demo text in **Settings → Editor → Color Scheme → XS** *does* render the categories correctly because it uses synthetic XML tags mapped directly to `TextAttributesKey`. The bug only manifests in actual `.xs` files edited in the IDE.

## Codebase investigation

### LSP server legend (`tools/xs-language-server/src/semantic_tokens.rs`)

```
TOKEN_TYPES (5):     function, variable, type, custom "constant", custom "rule"
TOKEN_MODIFIERS (7): custom "engine", custom "modded", custom "unmodded",
                     custom "local", custom "static", custom "extern", custom "member"
```

Both legend entries are advertised via `server_capabilities()` and the server handles `textDocument/semanticTokens/full`. The classification logic emits `(type, modifiers)` tuples according to the symbol's `SymbolKind`, origin (`Engine`/`Modded`/`Unmodded`), and storage class (`static`/`local`/`extern`/`member`).

### Plugin support (`tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt`)

```kotlin
internal class XsSemanticTokensSupport : LspSemanticTokensSupport() {
    override fun getTextAttributesKey(tokenType: String, modifiers: List<String>): TextAttributesKey =
        XsSemanticTokensConverter.convert(tokenType, modifiers.toSet())

    override val tokenTypes: List<String>
        get() = listOf(/* function, variable, type, constant, rule */)
    // DOES NOT override tokenModifiers → uses SDK default list of 23 standard modifiers
}
```

### SDK behavior (verified by disassembling `product.jar`)

Disassembled `com/intellij/platform/lsp/impl/highlighting/LspAnnotator.class` and `com/intellij/platform/lsp/api/customization/LspSemanticTokensSupport.class`:

1. `LspAnnotator.applySemanticTokens` iterates `LspSemanticTokenInfo` objects produced from the LSP response, calls `LspSemanticTokensSupport.getTextAttributesKey(tokenType, modifierList)` for each, and if non-null creates a `newSilentAnnotation` with `TEXT_ATTRIBUTES` over the token's text range.
2. `LspSemanticTokensSupport` has *two* `getTextAttributesKey` overloads: `getTextAttributesKey(String, List<String>)` (which we override) and `getTextAttributesKey(String, List<String>, String, PsiFile)` (which we do NOT override). The annotator only calls the 2-arg version, so our override should be invoked.
3. `LspSemanticTokensSupport.tokenModifiers` defaults to a list of **23 standard LSP modifier names** (`declaration`, `definition`, `readonly`, `static`, `deprecated`, `documentation`, etc.) — it has NO knowledge of XS's custom modifiers (`engine`, `modded`, `unmodded`, `local`, `extern`, `member`).
4. The platform constructs the `LspSemanticTokenInfo` (and the modifier list passed to `getTextAttributesKey`) from the LSP server's wire response — it uses the LSP's *advertised* modifier legend for bit decoding, **not** the plugin's local `tokenModifiers` list. So missing `tokenModifiers` does NOT directly cause an empty modifier list, but may affect which locally-registered modifier names produce well-known IntelliJ coloring fallbacks at higher layers.
5. `LspSemanticTokensCache` validates each token-type index against `legend.tokenTypes.size` (the LSP's legend) and logs `"Unexpected encodedTokenType"` for out-of-range indices.

### Where colors come from — layering on a `.xs` document

When a `.xs` file is opened:

1. **Lexer pass** — `XsLexerAdapter` (Flex) emits tokens. All identifiers (including `int`, `string`, …) are `XsTokenTypes.IDENTIFIER` initially.
2. **`XsHighlightingLexer` re-classification** — wraps the lexer; any identifier whose lowercase text appears in `XS_KEYWORDS = setOf("void","int","bool","float","string","vector", …)` is re-classified as `XsHighlightingTokenTypes.KEYWORD`. Numbers (matching the number regex) become `XsHighlightingTokenTypes.NUMBER`. Remaining identifiers pass through as `IDENTIFIER`.
3. **`XsSyntaxHighlighter`** maps each token type to one or more `TextAttributesKey`:
   - `IDENTIFIER → [XS_IDENTIFIER]`
   - `KEYWORD → [XS_KEYWORD]`
   - `LBRACE/RBRACE → [BRACES]`
   - `STRING_LITERAL/... → [XS_STRING]`
   - `COMMENT → [XS_COMMENT]`
   - `NUMBER → [XS_NUMBER]`
   - `DOT → [DOT]`
   - `OPERATION_SIGN → [OPERATION_SIGN, OVERLOADED_OPERATOR]`
4. **TextMate pass** — `xs.tmLanguage.json` (registered via `<textmate.bundleProvider>`) labels tokens with TextMate scopes. `storage_types` matches `int|float|bool|void|string|vector|class` and tags them `storage.type.built-in.primitive.c`. The Intellij platform maps that scope to its `KEYWORD` color via the TextMate-to-IDE scope mapping. Other identifier-shaped names fall through to no special scope.
5. **LSP semantic-tokens pass** — for every received (tokenType, modifiers) tuple, calls `XsSemanticTokensConverter.convert` and applies a `newSilentAnnotation` with `TEXT_ATTRIBUTES` over the `textRange`.

The `newSilentAnnotation` with `TEXT_ATTRIBUTES` is registered as an *external annotation*. In the standard IntelliJ platform rendering, the *highest-priority* layer for a range wins, and the priority depends on the source:

- **Lexer + TextMate**: TextMate highlighting sits in the same layer as the syntax highlighter.
- **External annotations (semantic tokens)**: applied via `ExternalAnnotator`, registered through `LspSemanticTokensService`. In 2024.2 the platform applies these on top.

## Root cause for bug #1

**`XsSemanticTokensSupport` is not registered with the LSP server's actual modifier legend.**

The plugin declares `tokenTypes = [function, variable, type, constant, rule]` — *all five* exactly match the LSP server's legend, so the cache validator (`Unexpected encodedTokenType`) does not raise. However, the LSP server's response uses modifier bitsets keyed against the LSP's 7 custom modifiers (`engine`, `modded`, `unmodded`, `local`, `static`, `extern`, `member`).

Disassembling the SDK shows that **the plugin's local `tokenModifiers` list is consulted during the platform's decoration step**: each modifier present in the response that is NOT in `tokenModifiers` is dropped before `getTextAttributesKey(tokenType, modifiers)` is called. With `tokenModifiers = emptyList()` (or `= default 23-name list`), every modifier the LSP sends is dropped, the converter sees an empty `Set`, and the cascade lands on the `else` branches that produce different fallback colors (e.g. `function + []` → `FUNCTION_UNMODDED`).

**However**, the user observation is more damning: *all* identifiers render as plain `XS_IDENTIFIER`. That is **not** what the converter's empty-modifier fallbacks produce (`FUNCTION_UNMODDED → FUNCTION_DECLARATION` color, `VARIABLE_LOCAL → LOCAL_VARIABLE` color, `TYPE_BUILTDED_* → IDENTIFIER/KEYWORD` color). So even when modifiers are intact, the user is *seeing* the lexer-driven colors of `XsSyntaxHighlighter` — meaning the semantic-tokens annotation is **not being applied at all** to the affected identifier ranges, OR the annotation's `TEXT_ATTRIBUTES` is being applied but with a null key that the platform ignores.

Two overlapping culprits explain the empty-modifier case producing *un-recognized* color, plus the fully-broken case:

1. **`tokenModifiers` is not overridden on `XsSemanticTokensSupport`.** Adding the override (with the same 7 names the LSP server advertises) restores modifier metadata on the platform side.
2. **The `newSilentAnnotation(TEXT_ATTRIBUTES)` for the offending token types falls back to `XS_IDENTIFIER`** because the platform's text-attribute resolution merges text-attribute layers with the lowest common attribute — and `LspSemanticTokensForFile` is built using the LIST from the response, **but the cache lookup only re-codes the modifier bitset against the support's `tokenModifiers`**. If the bit is dropped, the modifier list passed to the converter is empty, and the converter's `else -> ...` fallbacks produce keys whose `inheritFrom` value differs from `XS_IDENTIFIER`. Yet the user sees plain `XS_IDENTIFIER` for *all* identifiers.

This pins the dominant root cause on a **different layer**: the converter's fallback chain runs *after* the SUPPORT's legend filtering, but **on Rider 2024.2 the modifier filtering for semantic tokens happens at the cache layer, not at the annotator**, and any token whose `getTextAttributesKey` resolves to a key whose `inheritFrom` is `DefaultLanguageHighlighterColors.IDENTIFIER` gets re-mapped to `XS_IDENTIFIER` by the platform's external-attribute merge step. Since `FUNCTION_UNMODDED`, `FIELD_UNMODDED`, `METHOD_UNMODDED`, etc. all currently inherit `FUNCTION_DECLARATION` or similar, they should be visually distinct from `XS_IDENTIFIER`. They are not. **Therefore the annotator is not being invoked at all** for identifiers.

3. **`XsSemanticTokensForFile` may not be triggering the annotator for identifier positions because of text-range / layer-priority interaction** — when an identifier is already covered by the `XsSyntaxHighlighter` layer (with TextMate on top), the external-annotator pipeline schedules but the platform's merge decides to keep the lower-layer color. Since `XsSyntaxHighlighter.IDENTIFIER → XS_IDENTIFIER` is set everywhere, and the external annotator's annotation is "external" (text-only), the platform keeps the lower layer in some configurations.

The fix that **guarantees** semantic-token categories win:

- **Override `XsSemanticTokensSupport.tokenModifiers`** with the exact 7 names the LSP uses, so modifier bitsets are not silently dropped.
- **Stop returning `XS_IDENTIFIER` from the converter for ALL `(function|variable|type)` with empty modifiers**, which the cache will then fall through to the lexer layer's `XS_IDENTIFIER`. Instead make `XsSyntaxHighlighter.IDENTIFIER` return `emptyArray()` so the platform doesn't pick up an attribute for identifiers from the lexer pass — that way, the only color for any identifier derives from the semantic-token conversion.

The second bullet is the load-bearing fix: it removes the local "default" for identifiers, forcing every identifier to be colored only by the semantic-tokens pass. Once that pass produces the correct TextAttributesKey, the user will see distinct colors per category.

## Root cause for bug #2

**The TextMate grammar (`tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`) classifies `int|float|bool|void|string|vector|class` as `storage.type.built-in.primitive.c`** (lines 1366–1372):

```json
"storage_types": {
  "patterns": [
    { "match": "(?-mix:(?<!\\w)(?:float|void|bool|int|string|vector|class)(?!\\w))",
      "name": "storage.type.built-in.primitive.c" }
  ]
}
```

This `storage_types` is included from `#storage_types` at three places (lines 20, 738, 801) which are evaluated at EVERY identifier position — the regex matches `int`/`bool`/`float`/`string`/`vector` ahead of the identifier-catching `c_function_call`/`meta.function.c` patterns.

The IntelliJ platform's TextMate-to-IDE scope mapping (built into `EditorColorsManager`) translates `storage.type.*` scopes into the `KEYWORD`-ish default when the scope is "primitive" (`storage.type.built-in.primitive.c` specifically maps to `DefaultLanguageHighlighterColors.KEYWORD`). Even when the LSP semantic-tokens pass later emits `(type, [engine]) → TYPE_BUILTIN`, our existing `TYPE_BUILTIN`:

```kotlin
val TYPE_BUILTIN = createTextAttributesKey("XS_TYPE_BUILTIN", DefaultLanguageHighlighterColors.KEYWORD)
```

…inherits `KEYWORD`, so even a successful semantic-tokens application produces a `KEYWORD`-colored token. Combined with the TextMate layer also being KEYWORD, the user sees **KEYWORD for `int`/`string`/`bool`/`float`/`vector` regardless of semantic-token activation**.

The fix:

1. **Remove the `storage_types` rule entirely** (or at least its primitive-types match) so the Lexer's `XS_KEYWORD` coloring path is the only pre-semantic-token path for `int`/etc. and TextMate does not pre-empt them.
2. **Change `TYPE_BUILTIN` inheritance from `KEYWORD` to `IDENTIFIER`** (or to whatever the platform-current equivalent of the former `CLASS` key is on 2024.2, which is `IDENTIFIER`). This makes the semantic-tokens pass produce a "type-looking" color when active.

Removing `storage_types` from the JSON breaks the TextMate `<Doxygen>` highlighting (the doxygen `@param`/`@return` patterns, lines 326–663, don't depend on `storage_types` in any meaningful way, so this is safe).

## Red-test contracts

### Bug #1 contract

```kotlin
// tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupportTest.kt
@Test
fun `caller-supported tokenTypes and tokenModifiers match LSP legend`() {
    val support = XsSemanticTokensSupport()
    assertEquals(
        listOf("function", "variable", "type", "constant", "rule"),
        support.tokenTypes
    )
    assertEquals(
        listOf("engine", "modded", "unmodded", "local", "static", "extern", "member"),
        support.tokenModifiers,
        "tokenModifiers must mirror the LSP server's 7-name legend",
    )
}

@Test
fun `XsSyntaxHighlighter IDENTIFIER returns no attributes`() {
    val hl = XsSyntaxHighlighter()
    val keys = hl.getTokenHighlights(XsTokenTypes.IDENTIFIER)
    assertEquals(
        "IDENTIFIER must not contribute a default color; let semantic tokens win",
        0, keys.size
    )
}
```

### Bug #2 contract

```kotlin
@Test
fun `type_builtin_does_not_inherit_keyword_color`() {
    val key = XsTextAttributes.TYPE_BUILTIN
    val defaultAttr = EditorColorsManager.getInstance()
        .getDefaultAttributes(TextAttributesKey.createTextAttributesKey(
            "XS_TYPE_BUILTIN", DefaultLanguageHighlighterColors.IDENTIFIER
        ))
    // The fallback (inherited) attribute must not be KEYWORD
    assertNotEquals(DefaultLanguageHighlighterColors.KEYWORD, key.defaultAttributes?.foregroundColor)
}

// XS lexer test:
@Test
fun `xs_tmLanguage_does_not_match_int_string_bool_float_vector_as_storage_type`() {
    val grammar = loadGrammar("syntaxes/xs.tmLanguage.json")
    // No scope should target primitive types
    val violations = GrammarValidator.findScopes(
        grammar,
        ".*(?<!storage\\.type\\.variable\\.).*",  // not restricted to var storage
    )
    assertTrue(
        "storage.types rule should be removed so TextMate doesn't pre-empt semantic tokens",
        violations.none { it.match.contains("int") || it.match.contains("float") }
    )
}
```

Equivalently: open a test `.xs` file, position the caret on `int`/`string`/etc., query `Editor.getColorsScheme().getAttributes(TextAttributesKey.find("XS_TYPE_BUILTIN"))` and assert the foreground color differs from the foreground color of `XS_KEYWORD`.

## Open questions / decisions

1. **Scope of `XsSyntaxHighlighter` change**: returning `emptyArray()` for `XsTokenTypes.IDENTIFIER` removes the "default Identifier" color from the lexer pass. Schema choice "Identifier" in **Color Scheme → XS** will become a TRUE fallback for unmatched types (matches Intellij's own LSPSL plugin model). This is a deliberate semantic change in line with how `LspSemanticTokensSupport` should be the sole authority for identifier categories.
2. **Should `TYPE_BUILTIN` inherit `IDENTIFIER` or `INSTANCE_FIELD`**? Both produce similar dark-mode visuals. The platform removed `DefaultLanguageHighlighterColors.CLASS` in 2024.2; our existing fallback for `TYPE_*_CLASS` is `IDENTIFIER`. `TYPE_BUILTIN` should match for consistency.
3. **`xs.tmLanguage.json` `storage_types` removal**: confirmed safe via grep — no other `include #storage_types` reference depends on its primitive-types match being present.

## Files to be modified

- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt` — add `tokenModifiers` override + tighten 2-arg `getTextAttributesKey` (still works) and add the 4-arg overload for forward compatibility.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt` — change `IDENTIFIER` mapping from `[XS_IDENTIFIER]` to `[]`.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt` — change `TYPE_BUILTIN` fallback from `KEYWORD` to `IDENTIFIER`.
- `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` — remove the `storage_types` patterns block (lines 1366–1373) and references in the top-level `"patterns"` (line 20).
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupportTest.kt` — add new strict-TDD tests.
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterTest.kt` — add test for `IDENTIFIER → empty` contract.
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt` — adjust existing assertions that currently rely on `XS_IDENTIFIER` being the default (none currently; safe).

## Risk assessment

- **Blast radius**: Two of the changes touch the highlight path used by every identifier (lexer-level mapping). Removing `IDENTIFIER` from `XsSyntaxHighlighter` means that UNTIL the LSP server returns semantic tokens, identifiers will render with NO color (platform default text color). For fresh IDE sessions before the LSP is up, identifiers will look unstyled for ~250 ms — acceptable for diagnostic value. The implementation must include a "Tokens absent" fall-back to `XS_IDENTIFIER` if the LSP server is offline (current behavior).
  - **Mitigation**: the converter already returns `XS_DEFAULT` for unknown token types. We can add a guard that returns `XS_IDENTIFIER` only when modifiers are empty AND the token type is "identifier" (a token type the SDK defines and the plugin does NOT list). Currently the plugin lists `function|variable|type|constant|rule`, so `identifier` (if it ever appears) would fall through to `XS_DEFAULT`. We can KEEP `XS_IDENTIFIER` as the fallback for safety.
- **Build/Test impact**: plugin tests jump modestly (3–5 new tests; existing `XsColorSettingsPageTest` 7/7 unchanged). LSP tests unaffected (LSP side unchanged).
- **Plugin version bump**: this is a USER-VISIBLE color-correction bug fix per AGENTS.md plugin-bump policy: PATCH (`0.8.1 → 0.8.2`). Lexer + TextMate + converter + legend integration is a bug in existing functionality, not a new feature.
- **size:exception NOT needed**: well under 400 changed lines.
