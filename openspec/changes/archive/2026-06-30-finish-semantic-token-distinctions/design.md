# Design: Finish deferred semantic-token distinctions

## 1. Resolved open questions

| # | Question | Resolution | Rationale |
|---|----------|------------|-----------|
| 1 | Cache migration for `SymbolKind::Class` | **(a) Bump the per-file parse-cache schema version.** Change `cache.rs` to write per-file symbol caches under `game_parse/v2/` instead of `game_parse/v1/`. | `SymbolKind` is string-tagged JSON, so a new variant is forward-compatible for the new reader but not for the old reader on downgrade. A directory bump mirrors the existing `v2/` engine-cache pattern and forces a clean rebuild. |
| 2 | Platform build bump | **(a) Bump `pluginSinceBuild` to `242.22855.74` (IntelliJ 2024.2.2) and advance `platformVersion` to `2024.2.2`.** | `LspCustomization` / `LspSemanticTokensSupport` are documented from 2024.2.2+. The locally cached `2024.2` build (`242.20224.300`) does not contain those classes, so the build dependency must also be advanced. |
| 3 | Converter signature mismatch | **(b) Adapt in `XsSemanticTokensSupport`.** Keep `XsSemanticTokensConverter.convert(tokenType, Set<String>)` and call `modifiers.toSet()` in the support class. | This avoids changing the existing converter and its 10 unit tests. The list-to-set conversion is trivial for the token volume of an XS file. |
| 4 | Class key fallback | **(a) Keep `DefaultLanguageHighlighterColors.IDENTIFIER` for `TYPE_UNMODDED_CLASS` / `TYPE_MODDED_CLASS`.** New keys get their own defaults: `CONSTANT` -> `CONSTANT`, `RULE` -> `FUNCTION_DECLARATION`, `VARIABLE_EXTERN_*` -> `GLOBAL_VARIABLE`. | The class-extraction spec explicitly marks recoloring the existing class keys as out of scope. The new keys pick semantically closest platform defaults. |
| 5 | Extern functions | **(a) Emit `extern` only for `SymbolKind::Variable` references.** | The spec (R6) requires `extern` only for variables. `Symbol::is_extern` is already set on functions as well, but emitting it now would require new `FUNCTION_EXTERN_*` keys that are out of scope. |
| 6 | Origin modifiers for constants/rules | **(a) Keep origin modifiers in LSP emission; the plugin ignores them for `constant` and `rule`.** | `classify_identifier` already attaches the origin modifier uniformly. The converter maps any `constant`/`rule` token to a single key, so origin is harmless and leaves room for future origin-specific colors. |

## 2. LSP changes (Rust)

### 2.1 Symbol table (`tools/xs-language-server/src/symbols.rs`)

- Add `Class` to `SymbolKind` and update `label()` to return `"class"`.
- In `build_symbol_table`, match `"class_specifier"` alongside `rule_definition`, `function_definition`, and `declaration`.
- Add `extract_class`:

```rust
fn extract_class(node: Node, source: &str, out: &mut Vec<Symbol>) {
    let name_node = node.child_by_field_name("name").unwrap();
    let name = node_text(name_node, source).to_string();
    out.push(Symbol {
        name,
        kind: SymbolKind::Class,
        ty: String::new(),
        params: Vec::new(),
        is_extern: false,
        is_mutable: false,
        is_static: false,
        is_forward: false,
        visibility: Visibility::Public,
        full_range: node_range(node),
        selection_range: node_range(name_node),
        detail: format!("class {}", name),
    });
}
```

- Members inside `field_declaration_list` are **not** walked; only the class name is captured.
- XS classes are only valid at top level, so nested class bodies are ignored by this walker.

### 2.2 Semantic tokens (`tools/xs-language-server/src/semantic_tokens.rs`)

- Extend `TOKEN_TYPES` with `SemanticTokenType::CONSTANT` and `SemanticTokenType::new("rule")`.
- Extend `TOKEN_MODIFIERS` with `SemanticTokenModifier::new("extern")`.
- In `classify_identifier`:
  - `SymbolKind::Constant` -> `SemanticTokenType::CONSTANT`
  - `SymbolKind::Rule` -> `SemanticTokenType::new("rule")` (replace `return None`)
  - `SymbolKind::Variable`/`Function` stay as before.
- In `classify_modifiers`, push `extern` when `symbol.kind == SymbolKind::Variable && symbol.is_extern`.
- `classify_type_identifier` needs no change: class references now resolve through the merged/own table and emit `TYPE` with the correct origin.

### 2.3 Extern detection

`extern` is already parsed by `extract_modifiers` and stored in `Symbol::is_extern` for variables and functions. No new walker is required.

### 2.4 Cache compatibility (`tools/xs-language-server/src/cache.rs`)

Bump the per-file parse-cache directory:

```rust
pub fn parse_cache_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("game_parse").join("v2")
}
```

Update the directory-layout comment at the top of `cache.rs`.

### 2.5 Tests

- `tools/xs-language-server/tests/class_extraction_repro.rs` (5 tests):
  1. Vanilla class reference -> `unmodded`
  2. Modded class reference -> `modded`
  3. Unknown class reference -> `engine`
  4. Class declaration does not crash the walker
  5. 50+ class declarations build in < 100 ms
- `tools/xs-language-server/tests/semantic_token_distinctions_repro.rs` (4 tests):
  1. Constant reference emits `constant`
  2. Rule reference emits `rule`
  3. Extern variable emits `variable` + `[extern, ...]`
  4. Constant + rule + extern compose with existing modifiers

## 3. Plugin changes (Kotlin)

### 3.1 New `XsSemanticTokensSupport.kt`

```kotlin
internal class XsSemanticTokensSupport : LspSemanticTokensSupport() {
    override fun getTextAttributesKey(
        tokenType: String,
        modifiers: List<String>,
        file: PsiFile,
    ): TextAttributesKey? =
        XsSemanticTokensConverter.convert(tokenType, modifiers.toSet())

    override fun getTokenTypes(): List<SemanticTokenType> =
        listOf(
            SemanticTokenType.FUNCTION,
            SemanticTokenType.VARIABLE,
            SemanticTokenType.TYPE,
            SemanticTokenType.CONSTANT,
            SemanticTokenType("rule")
        )
}
```

(Exact `SemanticTokenType` constructor/import to be verified against the bundled 2024.2.2 sources during apply.)

### 3.2 `XsLspServerDescriptor` update

Override `lspCustomization`:

```kotlin
override val lspCustomization: LspCustomization?
    get() = object : LspCustomization() {
        override val semanticTokensCustomizer = XsSemanticTokensSupport()
    }
```

### 3.3 `XsTextAttributes.kt` additions

Add four keys:

```kotlin
val CONSTANT = createTextAttributesKey("XS_CONSTANT", DefaultLanguageHighlighterColors.CONSTANT)
val RULE = createTextAttributesKey("XS_RULE", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)
val VARIABLE_EXTERN_UNMODDED = createTextAttributesKey("XS_VARIABLE_EXTERN_UNMODDED", DefaultLanguageHighlighterColors.GLOBAL_VARIABLE)
val VARIABLE_EXTERN_MODDED = createTextAttributesKey("XS_VARIABLE_EXTERN_MODDED", DefaultLanguageHighlighterColors.GLOBAL_VARIABLE)
```

Existing `TYPE_UNMODDED_CLASS` / `TYPE_MODDED_CLASS` remain as `IDENTIFIER`.

### 3.4 `XsColorSettingsPage.kt` additions

Add descriptors:

```kotlin
AttributesDescriptor("Identifier//Variable//Constant", XsTextAttributes.CONSTANT),
AttributesDescriptor("Identifier//Function//Rule", XsTextAttributes.RULE),
AttributesDescriptor("Identifier//Variable//Extern UnModded", XsTextAttributes.VARIABLE_EXTERN_UNMODDED),
AttributesDescriptor("Identifier//Variable//Extern Modded", XsTextAttributes.VARIABLE_EXTERN_MODDED)
```

### 3.5 `XsSemanticTokensConverter.kt` additions

Add token-type constants `TOKEN_TYPE_CONSTANT = "constant"` and `TOKEN_TYPE_RULE = "rule"`. Extend the `when`:

```kotlin
TOKEN_TYPE_CONSTANT -> XsTextAttributes.CONSTANT
TOKEN_TYPE_RULE -> XsTextAttributes.RULE
TOKEN_TYPE_VARIABLE -> when {
    modifiers.contains(MODIFIER_EXTERN) && modifiers.contains(MODIFIER_UNMODDED) ->
        XsTextAttributes.VARIABLE_EXTERN_UNMODDED
    modifiers.contains(MODIFIER_EXTERN) && modifiers.contains(MODIFIER_MODDED) ->
        XsTextAttributes.VARIABLE_EXTERN_MODDED
    isStatic -> XsTextAttributes.VARIABLE_STATIC
    else -> XsTextAttributes.VARIABLE_LOCAL
}
```

Keep the existing `Set<String>` signature.

### 3.6 `plugin.xml` update

`<depends>com.intellij.modules.lsp</depends>` is already present. No new extension point is needed because the customizer is wired through `lspCustomization`.

### 3.7 Tests

- Update `XsSemanticTokensConverterTest.kt` with 4 new assertions: `constant`, `rule`, `variable+extern+unmodded`, `variable+extern+modded`.
- Add `XsLspServerDescriptorTest.kt` asserting `lspCustomization?.semanticTokensCustomizer` is an `XsSemanticTokensSupport` instance.

## 4. `plugin.xml` update

`patchPluginXml` sets `since-build` from `pluginSinceBuild`. Bump the property to `242.22855.74`. No new `<extensions>` entries.

## 5. `gradle.properties` update

```properties
pluginVersion = 0.7.0
platformVersion = 2024.2.2
pluginSinceBuild = 242.22855.74
```

(Minor version bump per `AGENTS.md`; platform version advanced so the required classes are present at compile time.)

## 6. Risks & mitigations

1. **Parse-cache downgrade.** Old servers cannot read `game_parse/v2/` entries written by the new server. *Mitigation:* the schema bump isolates old and new caches; users downgrading can delete `~/.local/state/aomr_lsp/` if needed.
2. **Missing platform API on build.** If `2024.2.2` is not resolvable by the IntelliJ Platform Gradle Plugin, compilation fails. *Mitigation:* verify `./gradlew buildPlugin` after bump; the build number `242.22855.74` was confirmed from JetBrains release data.
3. **`getTokenTypes` return type/custom constructor.** The exact `SemanticTokenType` API may differ from the design snippet. *Mitigation:* apply phase starts with a stub compile to fix imports/constructor before writing tests.
4. **Class members not extracted.** Class references resolve, but members are still treated as local variables/functions via existing walkers. *Mitigation:* out of scope per spec; existing behavior unchanged.
5. **Extern functions not colored.** `is_extern` on functions is ignored for tokens. *Mitigation:* matches spec and avoids expanding the change.
6. **Token-type order.** Adding `constant`/`rule` at the end of `TOKEN_TYPES` changes indices; the plugin maps by name, so order does not affect rendering.
7. **`SemanticTokenType::new("rule")` must be stable.** The LSP spec allows custom token type strings; the plugin and server use the same literal.
8. **Color-page test fragility.** New descriptors are asserted by display name; keep strings in sync between `XsTextAttributes` and `XsColorSettingsPage`.

## 7. Test plan

| Layer | File | Tests |
|-------|------|-------|
| LSP | `tests/class_extraction_repro.rs` | 5 new |
| LSP | `tests/semantic_token_distinctions_repro.rs` | 4 new |
| Plugin | `XsSemanticTokensConverterTest.kt` | +4 updated |
| Plugin | `XsLspServerDescriptorTest.kt` | 1 new |
| **Total new/modified** | | **14** |

Existing baseline: 230 LSP + 91 plugin. Expected after change: 239 LSP + 96 plugin.

## 8. Slice plan

**One slice / single PR.** The LSP legend and plugin converter are tightly coupled; the expected delta is ~350–400 LOC, safely under the 400-line review budget and single-agent context window.
