## Verification Report

**Change**: `2026-06-30-fix-semantic-token-color-application`  
**Version**: 0.8.1 → 0.8.2 (PATCH)  
**Branch**: `xs-lsp-roundtrip-followup`  
**HEAD**: `fe95928b9d717c0927507ed80203d6230be69672`  
**Mode**: Standard

### Completeness

| Metric | Value |
|--------|-------|
| Tasks total | 16 (4 RED + 4 GREEN + 3 build/test + 1 version bump + 3 commit + 1 handoff) |
| Tasks complete | 16 |
| Tasks incomplete | 0 |

All tasks in `tasks.md` are checked complete. The Engram apply-progress memory (#1520) confirms Phase 1–Phase 6 finished with no deviations from design.

### Build & Tests Execution

**Build**: ✅ Passed  
**Tests**: ✅ 107 passed / ❌ 0 failed / ⚠️ 0 skipped

```text
$ ./gradlew :test --offline --no-daemon --rerun-tasks
To honour the JVM settings for this build a single-use Daemon process will be forked. For more on this, please refer to https://docs.gradle.org/8.10.2/userguide/gradle_daemon.html#sec:disabling_the_daemon in the Gradle documentation.
Daemon will be stopped at the end of the build
> Task :checkKotlinGradlePluginConfigurationErrors SKIPPED

> Task :generateLexer
Reading "/home/houtamelo/Documents/projects/aom_retold_mod/tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsLexer.flex"
Constructing NFA : 160 states in NFA
Converting NFA to DFA : 
.................................................56 states before minimization, 36 states in minimized DFA
Writing code to "/home/houtamelo/Documents/projects/aom_retold_mod/tools/intellij-xs-plugin/src/main/gen/com/aomr/xs/psi/XsLexer.java"

> Task :initializeIntellijPlatformPlugin

> Task :patchPluginXml
[org.jetbrains.intellij.platform] Patching plugin.xml: attribute 'since-build=[242]' of 'idea-version' tag will be set to '242.22855.74'
[org.jetbrains.intellij.platform] Patching plugin.xml: attribute 'until-build=[252.*]' of 'idea-version' tag will be set to '263.*'

> Task :verifyPluginProjectConfiguration

> Task :buildLspServer

> Task :stageLspServer
> Task :processResources
> Task :generateManifest
> Task :processTestResources
> Task :validateBundledResources
    Finished `release` profile [optimized] target(s) in 0.04s
> Task :compileKotlin
> Task :compileJava
> Task :classes
> Task :instrumentCode
> Task :jar
> Task :instrumentedJar
> Task :composedJar
> Task :prepareTestSandbox
> Task :prepareTest
> Task :compileTestKotlin
> Task :compileTestJava NO-SOURCE
> Task :testClasses
> Task :instrumentTestCode
> Task :test

BUILD SUCCESSFUL in 36s
20 actionable tasks: 20 executed
```

Test result XML aggregation (`dist/.build/test-results/test/TEST-*.xml`, 23 files):

```text
Files:   23
Tests:   107
Failures: 0
Errors:   0
Skipped:  0
```

**Coverage**: ➖ Not available (no coverage task configured for this plugin project).

### Spec Compliance Matrix

| Requirement | Scenario | Test | Result |
|-------------|----------|------|--------|
| Modifier-Legend Parity | Legend parity | `XsSemanticTokensSupportTest > legend_matches_lsp_tokenTypes` / `legend_matches_lsp_tokenModifiers` | ✅ COMPLIANT |
| Modifier-Legend Parity | Modifier preservation in converter | (none found at the `XsSemanticTokensSupport` bridge) | ⚠️ UNTESTED |
| Lexer-Level Identifier Default Removed | IDENTIFIER token type takes no default | `XsSyntaxHighlighterTest > test_identifier_returns_no_text_attributes` | ✅ COMPLIANT |
| Lexer-Level Identifier Default Removed | IDENTIFIER still resolvable as language default | (none found) | ⚠️ UNTESTED |
| Built-in Type Non-Keyword Inheritance | TYPE_BUILTIN fallback | `XsColorSettingsPageTest > type_builtin_not_inherits_from_keyword` | ✅ COMPLIANT |
| TextMate Storage_Types Rule Removed | No storage-types scope | `XsTextMateBundleFormatTest > grammarHasNoStorageTypesRuleForPrimitiveTypes` | ✅ COMPLIANT |
| TextMate Storage_Types Rule Removed | Identifiers still match via c_function_call rule | (none found) | ⚠️ UNTESTED |

**Compliance summary**: 4/7 scenarios compliant

Notes on UNTESTED scenarios:

- **Modifier preservation in converter**: The support class advertises the correct `tokenModifiers` legend, and `XsSemanticTokensConverterTest` exhaustively checks every (type, modifier) combination (e.g. `engineFunctionMapsToFunctionEngine`, `engineMemberMethodMapsToMethodEngine`, etc.). However, there is no test that calls `XsSemanticTokensSupport.getTextAttributesKey("function", listOf("engine"))` to prove the bridge forwards modifiers intact. This is a coverage gap, not a runtime failure.
- **IDENTIFIER still resolvable as language default**: No fixture or unit test exercises the platform’s language-default fallback path for an identifier before the LSP is ready. The scenario is a schema-level / platform-behavior claim rather than a code path under direct test.
- **Identifiers still match via c_function_call rule**: Optional scenario. No test asserts that an identifier-shaped token resolves to `entity.name.function.c` / `variable.other.c`. Existing TextMate tests continue to pass, indicating the grammar remains functional after the removal.

### Correctness (Static Evidence)

| Requirement | Status | Notes |
|-------------|--------|-------|
| `tokenModifiers` legend parity | ✅ Implemented | `XsSemanticTokensSupport.kt:31-33` returns `engine, modded, unmodded, local, static, extern, member` in that order. |
| Lexer-level identifier default removed | ✅ Implemented | `XsSyntaxHighlighter.kt:19` maps `XsTokenTypes.IDENTIFIER → EMPTY_KEYS`. |
| `TYPE_BUILTIN` non-keyword inheritance | ✅ Implemented | `XsTextAttributes.kt:81` creates `XS_TYPE_BUILTIN` with `DefaultLanguageHighlighterColors.IDENTIFIER` fallback. |
| `storage_types` rule removed | ✅ Implemented | `xs.tmLanguage.json` no longer contains the `storage_types` repository block or `#storage_types` includes; `grep -n "storage_types"` returns zero matches. |
| Plugin version bump | ✅ Implemented | `gradle.properties` updated to `pluginVersion = 0.8.2`. |
| AGENTS.md test count | ✅ Updated | AGENTS.md line 121 now reports 107 plugin tests. |

### Coherence (Design)

| Decision | Followed? | Notes |
|----------|-----------|-------|
| tokenModifiers legend parity | ✅ Yes | Exact 7-name override matches `semantic_tokens::TOKEN_MODIFIERS` order and content. |
| Lexer-level identifier default removed | ✅ Yes | `IDENTIFIER` returns `EMPTY_KEYS`; no `[XS_IDENTIFIER]` hard-wiring. |
| TYPE_BUILTIN no longer inherits from KEYWORD | ✅ Yes | `TYPE_BUILTIN` fallback is `DefaultLanguageHighlighterColors.IDENTIFIER`, consistent with `TYPE_UNMODDED_CLASS` / `TYPE_MODDED_CLASS`. |
| TextMate `storage_types` rule removed | ✅ Yes | Repository block and all three `#storage_types` includes (top level, `function-call-innards`, `function-innards`) removed. |

### Issues Found

**CRITICAL**: None

**WARNING**:

1. **Spec scenario UNTESTED: Modifier preservation in converter** — There is no runtime test proving that `XsSemanticTokensSupport.getTextAttributesKey("function", listOf("engine"))` returns `FUNCTION_ENGINE`. The converter logic is covered, and the legend is verified, but the bridge method itself is not exercised with modifiers.
2. **Spec scenario UNTESTED: IDENTIFIER still resolvable as language default** — No test exercises the platform language-default fallback for an identifier before semantic tokens are available.
3. **Spec scenario UNTESTED: Identifiers still match via c_function_call rule** — Optional scenario; no direct test covers it, though existing TextMate tests pass.

**SUGGESTION**:

1. Add a focused unit test for `XsSemanticTokensSupport.getTextAttributesKey("function", listOf("engine")) == XsTextAttributes.FUNCTION_ENGINE` to close the modifier-preservation gap.
2. If the IDE startup fallback behavior becomes observable in fixtures, add a test that confirms unmatched identifiers resolve to the platform default text attribute before LSP semantic tokens arrive.
3. Consider adding a TextMate fixture assertion that an identifier like `myFunction` still resolves to `entity.name.function.c` or `variable.other.c` after the `storage_types` removal.

### Verdict

**PASS WITH WARNINGS**

All production changes from `design.md` are present at HEAD, the build and all 107 plugin tests pass, and no CRITICAL issues were found. The change is safe to merge, but three spec scenarios are not covered by tests: the modifier-preservation bridge path, the identifier language-default fallback, and the optional `c_function_call` rule coverage.
