# Design: Expand class member extraction for semantic coloring

## 1. Resolved open questions

| # | Question | Resolution | Rationale |
|---|----------|------------|-----------|
| 1 | Plugin display names for the 6 member descriptors | Use `Identifier//Function//Method Engine`, `Identifier//Function//Method UnModded`, `Identifier//Function//Method Modded`, `Identifier//Variable//Field Engine`, `Identifier//Variable//Field UnModded`, `Identifier//Variable//Field Modded` | Matches the existing `UnModded function` / `UnModded class` capitalization in `XsColorSettingsPage.kt`. The group prefix (`Function`/`Variable`) stays singular, and the leaf uses title case. |
| 2 | Must `getTokenTypes()` advertise new member-aware categories? | No. `member` is a modifier on the existing `function` / `variable` token types; no new token type is added. `XsSemanticTokensSupport.tokenTypes` stays unchanged. | The platform distinguishes token types from modifiers. A modifier that composes with existing types does not require a new type entry; if it did, prior modifiers like `extern` and `static` would also have forced new types. |
| 3 | `class_owner` for the `Class` symbol itself | `None` | A class does not own itself. Only `ClassField` and `ClassMethod` set `class_owner: Some(<class name>)`. The `Class` symbol remains a top-level declaration. |
| 4 | Performance target for member extraction | Keep 200 ms for 50+ class declarations with members on the reference machine. | The existing `class_specifier` walker only records the class name; adding a child walk over `field_declaration_list` is linear in body size. 200 ms is a reasonable stretch target; if benchmarks miss it, the spec can be relaxed to 300 ms before apply. |

## 2. LSP changes (Rust)

### 2.1 Symbol table (`tools/xs-language-server/src/symbols.rs`)

Add `ClassField` and `ClassMethod` to `SymbolKind` and update `label()`.

Add `#[serde(default)] pub class_owner: Option<String>` to `Symbol`. All existing `Symbol { ... }` construction sites default `class_owner: None`.

Extend `extract_class` to walk the `field_declaration_list` body. Pseudocode:

```rust
fn extract_class(node: Node, source: &str, out: &mut Vec<Symbol>) {
    let name_node = node.child_by_field_name("name").unwrap();
    let class_name = node_text(name_node, source).to_string();
    // emit Class symbol (unchanged except class_owner: None)

    let Some(body) = node.child_by_field_name("body") else { return };
    let mut cursor = body.walk();
    for child in body.children(&mut cursor) {
        if child.kind() == "field_declaration" {
            extract_class_member(child, source, &class_name, out);
        }
    }
}

fn extract_class_member(node: Node, source: &str, class_name: &str, out: &mut Vec<Symbol>) {
    let Some(declarator) = node.child_by_field_name("declarator") else { return };
    // declarator is _field_declarator: function_declarator or identifier
    let (name, kind, params) = if declarator.kind() == "function_declarator" {
        let name_node = method_name_from_declarator(declarator, source);
        let params = declarator.child_by_field_name("parameters")
            .map(|p| extract_params(p, source)).unwrap_or_default();
        (name_node, SymbolKind::ClassMethod, params)
    } else if declarator.kind() == "identifier" {
        (node_text(declarator, source).to_string(), SymbolKind::ClassField, Vec::new())
    } else {
        return;
    };

    let ty = type_from_declarator_or_specifiers(node, source);
    out.push(Symbol {
        name,
        kind,
        ty,
        params,
        class_owner: Some(class_name.to_string()),
        visibility: Visibility::Local,
        // ... remaining fields mirror existing defaults
    });
}
```

Members receive `Visibility::Local` so `merged_view::include_visible` keeps them file-scoped.

### 2.2 Semantic tokens (`tools/xs-language-server/src/semantic_tokens.rs`)

Add `SemanticTokenModifier::new("member")` to `TOKEN_MODIFIERS`.

Add a `MemberIndex` built once per token computation:

```rust
pub struct MemberIndex {
    origins: HashMap<String, Origin>, // member name -> strongest origin
}

impl MemberIndex {
    pub fn build(
        own_table: &SymbolTable,
        workspace: &Workspace,
        project: &VirtualProject,
        cache_dir: &Path,
        current_file: Option<&Path>,
    ) -> Self;
}
```

`build` scans `own_table` and every visible workspace file (cached via `cache::load_or_parse_symbols`) for `ClassField`/`ClassMethod` symbols, computes each member's file origin, and stores the strongest origin per name (`Modded > Unmodded > Engine`).

Update `walk_for_tokens` to handle `"field_expression"` / `"field_identifier"`:

- When a `field_identifier` is reached, call `classify_member_identifier`.
- Determine token type from AST context: `FUNCTION` if the `field_expression` is the `function` child of a `call_expression`, otherwise `VARIABLE`.
- Emit the token with modifiers `[member, <origin>]`.
- If the name has no workspace member match, emit nothing (array builtins like `.size()` stay uncolored).

Update `classify_identifier`:

- Map `ClassField` → `SemanticTokenType::VARIABLE`, `ClassMethod` → `SemanticTokenType::FUNCTION`.
- Append `member` modifier when the resolved symbol is a class member.

Update `classify_modifiers` to push `member` for `ClassField`/`ClassMethod`.

Change `compute_tokens` signature to accept `member_index: &MemberIndex`. The server builds the index once; tests build it with a temp cache dir.

### 2.3 Cache compatibility (`tools/xs-language-server/src/cache.rs`)

Bump the per-file parse cache from `game_parse/v2/` to `game_parse/v3/`:

```rust
pub fn parse_cache_dir(cache_dir: &Path) -> PathBuf {
    cache_dir.join("game_parse").join("v3")
}
```

Update the directory-layout comment at the top of the file and the doc string for `parse_cache_dir`. Engine-API cache remains at `v2/`. Warm `v2` parse caches are ignored and rebuilt as `v3` on first access.

### 2.4 Document outline / workspace symbol

`document_symbol` currently returns a flat list. Replace the simple `table.symbols.iter().map(symbol_to_lsp)` with a grouping step:

1. Partition symbols by `class_owner`.
2. Emit top-level symbols (`class_owner: None`) in source order.
3. For each `Class` symbol, attach its `ClassField`/`ClassMethod` children in source order.
4. Keep non-class top-level symbols childless.

Add `ClassField` and `ClassMethod` mappings to `symbol_to_lsp` and `symbol_to_workspace_symbol`:

- `ClassField` → `SymbolKind::FIELD`
- `ClassMethod` → `SymbolKind::METHOD`
- For workspace symbols, set `container_name` to `class_owner` when present.

### 2.5 Other Rust consumers

Update exhaustive `SymbolKind` matches:

- `completion.rs`: `ClassField` → `CompletionItemKind::FIELD`, `ClassMethod` → `CompletionItemKind::METHOD`.
- `merged_view.rs`: no change needed; members are `Local` and therefore excluded from cross-file paste.
- `semantic_tokens.rs`: add `ClassField`/`ClassMethod` branches in `classify_identifier`.
- `typecheck.rs` / `references.rs`: existing filters on `Function | Rule` remain valid; no action.

### 2.6 Tests

- `tools/xs-language-server/tests/class_member_extraction_repro.rs` (7 tests):
  1. Vanilla class field extraction.
  2. Vanilla class method extraction.
  3. Cross-file `obj.field` reference resolves to the field's origin.
  4. Cross-file `Class.method()` reference resolves to the method's origin.
  5. Ambiguous member name (vanilla + modded) → `modded` wins.
  6. 50+ classes with members build in < 200 ms.
  7. Malformed class body does not panic.

- `tools/xs-language-server/tests/class_member_semantic_tokens_repro.rs` (6 tests):
  1. `obj.field` emits `variable` + `[member, unmodded]`.
  2. `Class.method()` emits `function` + `[member, unmodded]`.
  3. `obj.field` in modded class → `[member, modded]`.
  4. Ambiguous member name → `modded`.
  5. Member reference composes with `engine` modifier (member of engine class — fallback path).
  6. Member declaration inside class emits `member` modifier and correct token type.

## 3. Plugin changes (Kotlin)

### 3.1 `XsTextAttributes.kt`

Add 6 keys:

```kotlin
val METHOD_ENGINE = createTextAttributesKey("XS_METHOD_ENGINE", DefaultLanguageHighlighterColors.STATIC_METHOD)
val METHOD_UNMODDED = createTextAttributesKey("XS_METHOD_UNMODDED", DefaultLanguageHighlighterColors.STATIC_METHOD)
val METHOD_MODDED = createTextAttributesKey("XS_METHOD_MODDED", DefaultLanguageHighlighterColors.STATIC_METHOD)
val FIELD_ENGINE = createTextAttributesKey("XS_FIELD_ENGINE", DefaultLanguageHighlighterColors.INSTANCE_FIELD)
val FIELD_UNMODDED = createTextAttributesKey("XS_FIELD_UNMODDED", DefaultLanguageHighlighterColors.INSTANCE_FIELD)
val FIELD_MODDED = createTextAttributesKey("XS_FIELD_MODDED", DefaultLanguageHighlighterColors.INSTANCE_FIELD)
```

If `INSTANCE_FIELD` is unavailable in the bundled platform, fall back to `LOCAL_VARIABLE` during apply and update this design.

### 3.2 `XsColorSettingsPage.kt`

Add 6 descriptors per the Q1 resolution:

```kotlin
AttributesDescriptor("Identifier//Function//Method Engine", XsTextAttributes.METHOD_ENGINE),
AttributesDescriptor("Identifier//Function//Method UnModded", XsTextAttributes.METHOD_UNMODDED),
AttributesDescriptor("Identifier//Function//Method Modded", XsTextAttributes.METHOD_MODDED),
AttributesDescriptor("Identifier//Variable//Field Engine", XsTextAttributes.FIELD_ENGINE),
AttributesDescriptor("Identifier//Variable//Field UnModded", XsTextAttributes.FIELD_UNMODDED),
AttributesDescriptor("Identifier//Variable//Field Modded", XsTextAttributes.FIELD_MODDED)
```

### 3.3 `XsSemanticTokensConverter.kt`

Add `MODIFIER_MEMBER = "member"`. Extend the converter branches:

```kotlin
TOKEN_TYPE_FUNCTION -> when {
    modifiers.contains(MODIFIER_MEMBER) && engine -> XsTextAttributes.METHOD_ENGINE
    modifiers.contains(MODIFIER_MEMBER) && modded -> XsTextAttributes.METHOD_MODDED
    modifiers.contains(MODIFIER_MEMBER) && unmodded -> XsTextAttributes.METHOD_UNMODDED
    engine -> XsTextAttributes.FUNCTION_ENGINE
    modded -> XsTextAttributes.FUNCTION_MODDED
    unmodded -> XsTextAttributes.FUNCTION_UNMODDED
    else -> XsTextAttributes.FUNCTION_UNMODDED
}
TOKEN_TYPE_VARIABLE -> when {
    modifiers.contains(MODIFIER_MEMBER) && engine -> XsTextAttributes.FIELD_ENGINE
    modifiers.contains(MODIFIER_MEMBER) && modded -> XsTextAttributes.FIELD_MODDED
    modifiers.contains(MODIFIER_MEMBER) && unmodded -> XsTextAttributes.FIELD_UNMODDED
    isExtern && unmodded -> XsTextAttributes.VARIABLE_EXTERN_UNMODDED
    isExtern && modded -> XsTextAttributes.VARIABLE_EXTERN_MODDED
    isStatic -> XsTextAttributes.VARIABLE_STATIC
    else -> XsTextAttributes.VARIABLE_LOCAL
}
```

### 3.4 `XsSemanticTokensSupport.kt`

No change needed. `tokenTypes` already advertises `function` and `variable`; `member` is a modifier and is not listed here.

### 3.5 Tests

Extend `XsSemanticTokensConverterTest.kt` with 6 new assertions covering the `(function, member+engine/unmodded/modded)` and `(variable, member+engine/unmodded/modded)` mappings.

## 4. `gradle.properties` update

```properties
pluginVersion = 0.8.0
platformVersion = 2024.2.2
pluginSinceBuild = 242.22855.74
```

Minor bump per project policy: new LSP feature + new color-scheme categories.

## 5. Risks & mitigations

| Risk | Mitigation |
|------|------------|
| Member-name ambiguity mis-colors references | Document the name-based heuristic; a future type-inference slice can refine. |
| Array builtins (`.size()`, `.add()`, `.clear()`) colored as members | Only emit a member token when the name exists in the `MemberIndex`; otherwise fall back to no token. |
| Cache schema bump forces one-time re-parse | Bump `game_parse/v2/` → `v3/`; verify warm-cache rebuild cost in integration test. |
| Malformed class bodies crash extraction | Guard `field_declaration` traversal with node-kind checks; skip `ERROR` and unknown declarator shapes. |
| Plugin display-name drift | Keep a dedicated `XsColorSettingsPageTest` assertion for each new descriptor string. |
| `MemberIndex` scan per keystroke adds latency | Bounded by visible file count and cache hits; future slice can incrementally cache the index in the server. |
| `INSTANCE_FIELD` / `STATIC_METHOD` missing on 2024.2.2 | Apply phase starts with a stub compile; fall back to `LOCAL_VARIABLE` / `FUNCTION_DECLARATION` if needed. |

## 6. Test plan

| Layer | File | New tests |
|-------|------|-----------|
| LSP | `tests/class_member_extraction_repro.rs` | 7 |
| LSP | `tests/class_member_semantic_tokens_repro.rs` | 6 |
| Plugin | `XsSemanticTokensConverterTest.kt` | +6 |
| **Total new** | | **19** |

Existing baseline: 239 LSP + 96 plugin. Expected after this change: 258 LSP + 102 plugin.

## 7. Slice plan

**One slice / single PR.** Member extraction, the `member` modifier, cache bump, and plugin color wiring are mutually dependent: the plugin cannot color members until the LSP emits them, and the LSP semantic-token tests cannot verify end-to-end coloring without the converter. The estimated delta is 600–750 changed lines (Rust + Kotlin + tests), within the user's preflight `review_budget_lines=D-unbounded`.
