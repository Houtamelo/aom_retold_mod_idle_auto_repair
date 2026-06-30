# Platform API Resolution

## Result

The required `com.intellij.platform.lsp.api.customization.LspSemanticTokensSupport` API is **not present** in the cached `2024.2` (`242.20224.300`) build and **is present** in `2024.2.2` (`242.22855.74`).

## `gradle.properties` changes

```properties
pluginVersion = 0.7.0
platformVersion = 2024.2.2
pluginSinceBuild = 242.22855.74
```

The `platformVersion_2 = 2025.2` property was removed.

## API signature deviations from `design.md`

The `LspSemanticTokensSupport` API in `2024.2.2` differs from the design snippet:

```kotlin
// Actual 2024.2.2 signatures (from product.jar javap)
public java.util.List<java.lang.String> getTokenTypes();
public com.intellij.openapi.editor.colors.TextAttributesKey getTextAttributesKey(java.lang.String, java.util.List<java.lang.String>);
```

- No nested `SemanticTokenType` class exists.
- `getTokenTypes()` returns `List<String>` (exposed to Kotlin as a `val tokenTypes: List<String>`), not `List<SemanticTokenType>`.
- `getTextAttributesKey` takes two parameters (`tokenType: String`, `modifiers: List<String>`); there is **no `file: PsiFile`** parameter.

These deviations will be accommodated in `XsSemanticTokensSupport`.
