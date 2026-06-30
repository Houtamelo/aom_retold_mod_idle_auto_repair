# Design: `expand-color-scheme-semantic-tokens`

## Technical Approach

Fix the three Option α regressions (3a/3b/3c) and add a minimal, real LSP semantic-token pipeline for Bucket C.

- **3a**: add `include` to the TextMate keyword list and to the native `XsHighlightingLexer` keyword set.
- **3b**: map every native lexer token (braces/brackets/parens/comma/dot/semicolon/operators) to the 12 inherited `XsTextAttributes` keys.
- **3c**: the per-language **Identifier under caret** key cannot override the platform’s global pass; document the limitation in the color-scheme page.
- **Bucket C**: advertise `textDocument/semanticTokens/full`; emit tokens for functions, variables, and type references classified by origin (`engine`/`modded`/`unmodded`) and, for variables, by `local`/`static`.

Out-of-scope: full class symbol-table support, `extern` variable color key, constant/rule classification, cross-file forward-declaration ordering.

## Architecture Decisions

### ADR-1 — Scope decision
**Choice**: Option β — 3a + 3b + 3c + a minimal Bucket C emitter.
**Alternatives**: Option α (insufficient, leaves semantic coloring unaddressed); Option γ (too large for one review cycle).
**Rationale**: Delivers the user-noticeable engine/modded/unmodded distinction now while honestly deferring exotic categories.

### ADR-2 — LSP semantic-token legend
**Choice**: 3 token types (`function`, `variable`, `type`) and 5 modifiers (`engine`, `modded`, `unmodded`, `local`, `static`).
**Alternatives**: The larger Bucket C legend (`class`, `constant`, `parameter`, `mutable`, `extern`, `declaration`).
**Rationale**: Matches the 8 new color categories in the spec; keeps the emitter small and avoids duplicating deferred work.

### ADR-3 — Origin classification
**Choice**: Use the symbol’s defining file path.
**Rule**:
1. Workspace symbol (merged view or per-file table) beats engine API.
2. If the definition file is in a mod overlay (`VirtualProject.file_overrides`) → `modded`.
3. Else if under `Workspace.game_path/game` → `unmodded`.
4. Else → `engine`.

### ADR-4 — Identifier under caret
**Choice**: Outcome (b) — the platform limitation.
**Rationale**: `XsTextAttributes.IDENTIFIER_UNDER_CARET` already falls back to `EditorColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES`. IntelliJ’s `IdentifierHighlighterPass` hard-codes the global keys, so the per-language override is ignored. Document the limitation on the color-scheme page.

### ADR-5 — TextMate vs native highlighter precedence
**Choice**: Use the native `XsSyntaxHighlighter` as the primary driver.
**Mitigation**: If the smoke test shows TextMate still overrides braces/operators, remap TextMate scopes or temporarily de-register the TextMate bundle for `.xs` in the IDE.

### ADR-6 — pluginVersion bump
**Choice**: `0.4.0 → 0.5.0` (MINOR).
**Rationale**: Per `AGENTS.md`, a new LSP feature + new settings UI + new grammar scope triggers a minor bump.

## Data Flow

```
                                  ┌──────────────────────────────────────┐
                                  │  tools/xs-language-server/src/...    │
                                  │  server.rs initializes + handler     │
                                  └──────────────┬───────────────────────┘
                                                 │ textDocument/semanticTokens/full
                                                 │
                       ┌─────────────────────────┴──────────────────────────────┐
                       │         semantic_tokens.rs                                │
                       │  • walk tree (declarations + identifier occurrences)    │
                       │  • resolve symbol → merged view → engine API            │
                       │  • classify origin (modded/unmodded/engine)             │
                       │  • classify variable storage (local/static)             │
                       │  • classify type nodes (primitive/class)                │
                       └──────────────┬────────────────────────────────────────┘
                                      │ encoded token array
                                      ▼
                        tools/intellij-xs-plugin/...
                        XsLspServerDescriptor.lspCustomization
                        └─> XsSemanticTokensSupport
                            └─> XsSemanticTokensConverter
                                └─> XsTextAttributes.<semantic key>
```

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` | Modify | Add `include` to the keyword list. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt` | Modify | Add `"include"` to `XS_KEYWORDS`. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsTokenTypes.kt` | Modify | Add `DOT`, `SEMI_COLON`, `OPERATION_SIGN` token types. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsLexer.flex` | Modify | Return `DOT`, `SEMI_COLON`, `OPERATION_SIGN` for the relevant chars. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt` | Modify | Map all braces/operators to `XsTextAttributes` keys. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt` | Modify | Add 8 semantic keys. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt` | Modify | Register the 8 new descriptors; update identifier-under-caret label. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt` | Create | Platform `LspSemanticTokensSupport` subclass / wrapper. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverter.kt` | Create | Convert `(type, modifiers)` to `TextAttributesKey`. |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt` | Modify | Wire `lspCustomization.semanticTokensCustomizer`. |
| `tools/xs-language-server/src/server.rs` | Modify | Advertise `semanticTokensProvider`; register handler; import new module. |
| `tools/xs-language-server/src/semantic_tokens.rs` | Create | Legend, token extraction, encoding, origin classification. |
| `tools/xs-language-server/src/symbols.rs` | Modify | Add `is_static` flag to `Symbol`; add helpers to build a per-file table that includes local variables. |
| `tools/xs-language-server/src/workspace.rs` | No change | Reuse existing path helpers. |
| `tools/intellij-xs-plugin/gradle.properties` | Modify | `pluginVersion = 0.5.0`. |
| `AGENTS.md` | Modify | Bump plugin/LSP test counts. |
| `docs/issues/2026-06-29-runtime-issues.md` | Modify | Update the Bucket C/3a/3b/3c status lines. |

## Interfaces / Contracts

### TextMate grammar fix (3a)

`tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json:16`

```diff
-      "match": "\\b(break|continue|else|for|if|return|while|do)\\b",
+      "match": "\\b(include|break|continue|else|for|if|return|while|do)\\b",
       "name": "keyword.control.c"
```

Also add `"include"` to `XS_KEYWORDS` in `XsHighlightingLexer.kt` so the native lexer highlights it as a keyword.

### Native lexer / highlighter fix (3b)

`XsTokenTypes.kt` adds three token types:

```kotlin
@JvmField val DOT = IElementType("XS_DOT", XsLanguage.INSTANCE)
@JvmField val SEMI_COLON = IElementType("XS_SEMI_COLON", XsLanguage.INSTANCE)
@JvmField val OPERATION_SIGN = IElementType("XS_OPERATION_SIGN", XsLanguage.INSTANCE)
```

`XsLexer.flex` returns them:

```flex
"."               { return XsTokenTypes.DOT; }
";"               { return XsTokenTypes.SEMI_COLON; }
"=" | "+" | "-" | "*" | "/" | "%" | "<" | ">" | "!" | "~" | "&" | "|" | "^" { return XsTokenTypes.OPERATION_SIGN; }
```

`XsSyntaxHighlighter.kt` maps the 8 categories:

```kotlin
override fun getTokenHighlights(tokenType: IElementType?): Array<TextAttributesKey> {
    if (tokenType == null) return EMPTY_KEYS
    return when (tokenType) {
        XsTokenTypes.LBRACE, XsTokenTypes.RBRACE -> arrayOf(XsTextAttributes.BRACES)
        XsTokenTypes.LBRACKET, XsTokenTypes.RBRACKET -> arrayOf(XsTextAttributes.BRACKETS)
        XsTokenTypes.LPAREN, XsTokenTypes.RPAREN -> arrayOf(XsTextAttributes.PARENTHESES)
        XsTokenTypes.COMMA -> arrayOf(XsTextAttributes.COMMA)
        XsTokenTypes.DOT -> arrayOf(XsTextAttributes.DOT)
        XsTokenTypes.SEMI_COLON -> arrayOf(XsTextAttributes.SEMI_COLON)
        XsTokenTypes.OPERATION_SIGN -> arrayOf(XsTextAttributes.OPERATION_SIGN)
        // existing mappings ...
        else -> arrayOf(XsTextAttributesKeys.XS_DEFAULT)
    }
}
```

### Identifier under caret (3c)

No code fix possible without a custom `IdentifierHighlighterPass`. Update the descriptor label and a comment:

```kotlin
AttributesDescriptor(
    "Code//Identifier under caret (uses global General → Identifier under caret)",
    XsTextAttributes.IDENTIFIER_UNDER_CARET
)
```

### LSP legend and capability

`tools/xs-language-server/src/semantic_tokens.rs`:

```rust
use tower_lsp::lsp_types::{
    SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
    SemanticTokensServerCapabilities, SemanticTokenType, SemanticTokenModifier,
};

const TOKEN_TYPES: &[SemanticTokenType] = &[
    SemanticTokenType::FUNCTION,
    SemanticTokenType::VARIABLE,
    SemanticTokenType::TYPE,
];

const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
    SemanticTokenModifier::new("engine"),
    SemanticTokenModifier::new("modded"),
    SemanticTokenModifier::new("unmodded"),
    SemanticTokenModifier::new("local"),
    SemanticTokenModifier::new("static"),
];

pub fn server_capabilities() -> SemanticTokensServerCapabilities {
    SemanticTokensServerCapabilities::SemanticTokensOptions(SemanticTokensOptions {
        legend: SemanticTokensLegend {
            token_types: TOKEN_TYPES.into(),
            token_modifiers: TOKEN_MODIFIERS.into(),
        },
        full: Some(tower_lsp::lsp_types::SemanticTokensFullOptions::Bool(true)),
        range: Some(false),
        work_done_progress_options: Default::default(),
    })
}
```

Register in `server.rs:301-356`:

```rust
semantic_tokens_provider: Some(server_capabilities_from_semantic_tokens()),
```

### Semantic-token handler

```rust
async fn semantic_tokens_full(
    &self,
    params: SemanticTokensParams,
) -> Result<Option<SemanticTokens>> {
    let uri = params.text_document.uri;
    let text = { self.documents.lock().await.get(&uri).unwrap_or("").to_string() };
    let current_file = uri.to_file_path().ok();
    let merged = match current_file {
        Some(_) => self.get_or_build_merged_view(&uri, &text).await,
        None => None,
    };

    let own_table = {
        let tables = self.symbol_tables.lock().await;
        tables.get(&uri).cloned().unwrap_or_default()
    };

    let (project, ws) = {
        let ws = self.workspace.lock().await;
        let project = ws.lookup_mod(&uri)
            .map(|e| ws.build_virtual_project(e))
            .unwrap_or_default();
        (project, ws.clone())
    };

    let tokens = semantic_tokens::compute_tokens(
        &text,
        current_file.as_deref(),
        &own_table,
        merged.as_ref(),
        &self.engine,
        &ws,
        &project,
    );

    Ok(Some(SemanticTokens {
        result_id: None,
        data: semantic_tokens::encode(&tokens),
    }))
}
```

### Token extraction

`semantic_tokens::compute_tokens`:

1. Parse the file.
2. Build a full per-file table including locals via a new `symbols::build_full_symbol_table(tree, source)` helper.
3. Walk the tree twice:
   - **Declarations**: emit a token over each symbol’s `selection_range`.
   - **Identifier occurrences**: for every `identifier` node that is not a declaration, resolve the symbol using the same precedence as hover: merged view first, then the per-file table, then engine API.
4. For function/variable occurrences, map to `function`/`variable` and apply origin/storage modifiers.
5. For `primitive_type` nodes → `type` + `engine`.
6. For `_type_identifier` nodes → resolve class definition; if found in a mod overlay → `type` + `modded`, else in vanilla game → `type` + `unmodded`.

### Local variable extraction

In `symbols.rs`, add `build_full_symbol_table` that first calls `build_symbol_table` and then walks every `compound_statement` body looking for `declaration` children whose `init_declarator` introduces an identifier. These become `Symbol { kind: Variable, visibility: Local, is_static: ... }`. Use the same `extract_modifiers` logic already present in the module.

```rust
pub fn build_full_symbol_table(tree: &Tree, source: &str) -> SymbolTable {
    let mut table = build_symbol_table(tree, source);
    extract_local_declarations(tree.root_node(), source, &mut table.symbols);
    table
}
```

### Origin classification

```rust
pub fn classify_origin(
    file: &std::path::Path,
    workspace: &Workspace,
    project: &VirtualProject,
) -> Origin {
    if project.file_overrides.values().any(|p| p == file) {
        Origin::Modded
    } else if workspace.game_relative_path(file).is_some() {
        Origin::Unmodded
    } else {
        Origin::Engine
    }
}
```

### Plugin converter

`XsSemanticTokensConverter.kt`:

```kotlin
object XsSemanticTokensConverter {
    fun convert(tokenType: String, modifiers: List<String>): TextAttributesKey = when {
        tokenType == "function" && "modded" in modifiers -> XsTextAttributes.FUNCTION_MODDED
        tokenType == "function" && "unmodded" in modifiers -> XsTextAttributes.FUNCTION_UNMODDED
        tokenType == "function" && "engine" in modifiers -> XsTextAttributes.FUNCTION_ENGINE
        tokenType == "variable" && "static" in modifiers -> XsTextAttributes.VARIABLE_STATIC
        tokenType == "variable" && "local" in modifiers -> XsTextAttributes.VARIABLE_LOCAL
        tokenType == "type" && "engine" in modifiers -> XsTextAttributes.TYPE_BUILTIN
        tokenType == "type" && "modded" in modifiers -> XsTextAttributes.TYPE_MODDED_CLASS
        tokenType == "type" && "unmodded" in modifiers -> XsTextAttributes.TYPE_UNMODDED_CLASS
        else -> XsTextAttributesKeys.XS_DEFAULT
    }
}
```

New keys in `XsTextAttributes.kt`:

```kotlin
val FUNCTION_ENGINE = createTextAttributesKey("XS_FUNCTION_ENGINE", DefaultLanguageHighlighterColors.FUNCTION_CALL)
val FUNCTION_UNMODDED = createTextAttributesKey("XS_FUNCTION_UNMODDED", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)
val FUNCTION_MODDED = createTextAttributesKey("XS_FUNCTION_MODDED", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)
val VARIABLE_LOCAL = createTextAttributesKey("XS_VARIABLE_LOCAL", DefaultLanguageHighlighterColors.LOCAL_VARIABLE)
val VARIABLE_STATIC = createTextAttributesKey("XS_VARIABLE_STATIC", DefaultLanguageHighlighterColors.STATIC_FIELD)
val TYPE_BUILTIN = createTextAttributesKey("XS_TYPE_BUILTIN", DefaultLanguageHighlighterColors.KEYWORD)
val TYPE_UNMODDED_CLASS = createTextAttributesKey("XS_TYPE_UNMODDED_CLASS", DefaultLanguageHighlighterColors.CLASS_NAME)
val TYPE_MODDED_CLASS = createTextAttributesKey("XS_TYPE_MODDED_CLASS", DefaultLanguageHighlighterColors.CLASS_NAME)
```

### Wiring the converter

In `XsLspServerDescriptor.kt`, override `lspCustomization`:

```kotlin
override val lspCustomization = object : LspCustomization() {
    override val semanticTokensCustomizer = XsSemanticTokensSupport()
}
```

`XsSemanticTokensSupport` wraps the platform’s abstract class/object. Its exact override method signature must be verified against the bundled 2024.2 platform sources; the class delegates token/modifier names to `XsSemanticTokensConverter`.

No new `plugin.xml` extension point is expected for this path; if the platform API requires one, add it during apply.

## Testing Strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit (LSP) | Legend advertised | Test `SemanticTokensServerCapabilities` contains `function`, `variable`, `type` and the 5 modifiers. |
| Unit (LSP) | Origin classification | Test `classify_origin` for an overlay file, a vanilla game file, and an engine path. |
| Unit (LSP) | Local/static extraction | Test `build_full_symbol_table` finds a variable inside a function body and a `static` top-level variable. |
| Unit (LSP) | Token emission | Test `compute_tokens` emits expected `(type, modifiers)` for engine/modded/unmodded functions and built-in types. |
| Unit (Plugin) | `include` keyword in TextMate grammar | Parse `xs.tmLanguage.json` and assert the keyword regex contains `include`. |
| Unit (Plugin) | Braces/operators mapping | Test `XsSyntaxHighlighter` returns the correct `TextAttributesKey` for `LBRACE`, `DOT`, `OPERATION_SIGN`, etc. |
| Unit (Plugin) | New descriptors | Extend `XsColorSettingsPageTest` to assert the 8 semantic keys are present. |
| Unit (Plugin) | Converter mapping | Test `XsSemanticTokensConverter.convert` yields the expected keys for every `(type, modifiers)` pair. |
| Integration | Plugin build | `./gradlew :test` green; `./gradlew :buildPlugin` produces `intellij-xs-plugin-0.5.0.zip`. |
| Integration | LSP tests | `cargo test --manifest-path tools/xs-language-server/Cargo.toml` green. |
| Manual | Rider smoke test | Verify `include`, braces/operators, and semantic function coloring. |

## Strict-TDD Task List

1. Write failing LSP test for semantic-token legend.
2. Write failing LSP test for engine/modded/unmodded function tokens.
3. Write failing LSP test for local/static variable tokens.
4. Write failing LSP test for built-in type tokens.
5. Write failing plugin test for `include` keyword in TextMate grammar.
6. Write failing plugin test for braces/operators mapping in `XsSyntaxHighlighter`.
7. Write failing plugin test for the 8 new `TextAttributesKey`s.
8. Write failing plugin test for the semantic-token converter.
9. Run all tests, confirm FAIL.
10. Implement LSP semantic-tokens capability.
11. Implement LSP origin classification.
12. Implement LSP local-variable extraction.
13. Implement LSP token emission.
14. Run LSP tests, confirm PASS.
15. Add `include` to TextMate grammar.
16. Add `"include"` to `XsHighlightingLexer.XS_KEYWORDS`.
17. Extend `XsLexer.flex` with `DOT`, `SEMI_COLON`, `OPERATION_SIGN`.
18. Add new token types to `XsTokenTypes.kt`.
19. Extend `XsSyntaxHighlighter` with 8 mappings.
20. Add 8 new semantic `TextAttributesKey`s.
21. Register 8 new descriptors in `XsColorSettingsPage`.
22. Implement semantic-token converter.
23. Wire converter through `XsLspServerDescriptor.lspCustomization`.
24. Investigate identifier-under-caret (3c) and document limitation.
25. Run plugin tests, confirm PASS.
26. Bump `pluginVersion` to `0.5.0`.
27. Run `./gradlew :buildPlugin`. Confirm `.zip` produced.
28. Update `AGENTS.md` and `docs/issues/2026-06-29-runtime-issues.md`.
29. Commit (orchestrator handles).

## Migration / Rollout

No migration. Existing color-scheme keys persist by external name. The change is additive; reverting only requires reverting the Rust/Kotlin/properties changes.

## Out-of-Scope Reminders

- Class extraction as a first-class `SymbolKind` is deferred; only a targeted class-origin lookup is used for type-token classification.
- Constant / rule classification is deferred.
- `extern` variable as a distinct color key is deferred.
- Cross-file forward-declaration ordering is a separate future spec.

## Open Questions

1. What is the exact override signature of `LspSemanticTokensSupport` in the bundled 2024.2 platform? Verify during apply.
2. Does TextMate override the native `XsSyntaxHighlighter` for braces/operators in Rider? Validate in the smoke test; apply ADR-5 fallback if needed.
3. Should the class-origin helper cache per virtual project or per file? Decide after measuring token-generation latency.
