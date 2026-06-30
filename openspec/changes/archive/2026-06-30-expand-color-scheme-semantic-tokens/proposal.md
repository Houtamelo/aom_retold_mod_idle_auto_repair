# Proposal: expand-color-scheme-semantic-tokens

## Status

Draft

## Background

Issue #3 in `docs/issues/2026-06-29-runtime-issues.md` asked for a richer XS color scheme in Rider/IntelliJ. The original `expand-color-scheme-categories` change (Option α) delivered only the inherited Rider `General` / `Language Defaults` categories and the `xs` → `XS` label rename. It deliberately deferred Bucket C — the semantic-token-driven engine / modded / unmodded symbol distinctions — to a follow-up change. A subsequent Rider smoke test on the `0.4.0` plugin confirmed that Bucket C is still missing and also surfaced three Option α gaps: `include` is not highlighted as a keyword, braces/operators fall back to `Default`, and per-language identifier-under-caret does not work.

This proposal recommends Option β: fix the three quick-win regressions (3a/3b/3c) and add a minimal but real LSP semantic-token pipeline for functions, variables, and types with engine/modded/unmodded modifiers. The heavier work — full class extraction, `extern` variable distinction, constant classification, rule classification, and cross-file forward-declaration ordering — is deferred to keep the review cycle focused.

## Scope decision

**Option β — 3a + 3b + 3c + a minimal Bucket C semantic-token emitter.**

- Option α is insufficient: it leaves the user's main ask (engine/modded/unmodded semantic coloring) unaddressed and ships three visible regressions.
- Option γ is too large for one cycle: it requires class extraction, full local-variable scope analysis, static/extern classification, constant/rule classification, and a large plugin-side converter. The risk of compounding LSP + plugin + grammar regressions is high.
- Option β delivers the high-value, user-noticeable categories now while honestly deferring the exotic categories to a follow-up cycle.

## User-facing contract

After this change:

- `include` is highlighted as a keyword in the editor.
- Braces (`{}`), brackets (`[]`), parens (`()`), comma, dot, and operation signs are colored per the user's chosen Braces and Operators settings (not `Default`).
- Eight new semantic-token categories appear in **Settings → Editor → Color Scheme → XS** and are actually applied by the editor:
  - **Identifier / Function / Engine function**
  - **Identifier / Function / UnModded function**
  - **Identifier / Function / Modded function**
  - **Identifier / Variable / Local variable**
  - **Identifier / Variable / Static variable**
  - **Identifier / Type / Built-in type** (`bool`, `int`, `float`, `string`, `vector`)
  - **Identifier / Type / UnModded class**
  - **Identifier / Type / Modded class**
- The existing 12 inherited Rider categories (identifier under caret, matched/unmatched brace, unknown symbol, Braces and Operators subcategories) continue to work.
- If 3c cannot be fixed per-language, the color-settings page documents the Rider/IntelliJ Platform limitation.
- All 215+ LSP tests and 68+ plugin tests still pass.
- Manual Rider smoke test confirms 3a and 3b work; 3c is either fixed or documented.

## Scope

### In scope

- **3a — `include` keyword**: extend the TextMate grammar to scope `include` as a keyword; verify the native lexer keyword set if needed.
- **3b — Braces/operators coloring**: extend `XsSyntaxHighlighter.kt` to map each lexer token to the corresponding `XsTextAttributes` key (BRACES, BRACKETS, COMMA, DOT, OPERATION_SIGN, OVERLOADED_OPERATOR, PARENTHESES, SEMI_COLON).
- **3c — Identifier under caret**: investigate why the per-language override does not work. Fix it if the cause is a wrong fallback constant; document the limitation if it is a platform API limitation.
- **Bucket C (partial) — Semantic tokens**:
  - LSP: advertise `textDocument/semanticTokens/full`; define a token-type legend (`function`, `variable`, `type`) and modifier legend (`engine`, `modded`, `unmodded`, `mutable`, `static`, `extern`); emit tokens from the per-file symbol table.
  - LSP: extract local variables (function parameters, body declarations, `extern` declarations) into the symbol table.
  - LSP: classify each symbol as engine / modded / unmodded based on the resolved file path.
  - Plugin: implement or wire `LspSemanticTokensSupport`; convert LSP tokens to `TextAttributesKey`s using the new keys.
  - Plugin: declare new `TextAttributesKey`s in `XsTextAttributes.kt` and register them in `XsColorSettingsPage.kt`.
  - Tests: strict-TDD tests for grammar/lexer, highlighter mapping, semantic-token emitter, and semantic-token converter.
- **pluginVersion bump**: `0.4.0 → 0.5.0` (MINOR per `AGENTS.md`: new LSP feature + new settings UI + new grammar scope).
- **AGENTS.md update**: bump the plugin test count line to reflect new tests.

### Out of scope (deferred to follow-up cycle)

- Class extraction and full class symbol-table support.
- `extern` variable as a distinct category from `static`/`local` (the `extern` modifier is emitted but not yet surfaced with its own color key).
- Constant classification (engine / modded / unmodded constant).
- Rule classification (`UnModded Rule`, `Modded Rule`).
- Cross-file forward-declaration ordering.
- Full Bucket C if 3a/3b/3c require more work than expected.

### Audit overlap

- `R7-F-17`: two `.xs` file-type registrations (native + TextMate). The lexical fixes in `XsSyntaxHighlighter` will only be visible if the native highlighter is the primary color driver; if TextMate wins, additional scope remapping or a primary-highlighter decision is needed.
- `R6-F-01`: keywords are lower-cased before lookup in `XsHighlightingLexer`; adding `include` must stay consistent with existing case handling.
- `R6-F-02`: `WHITE_SPACE` maps to `XS_DEFAULT` in `XsSyntaxHighlighter`; unrelated but in the same file.
- `R6-F-03`: `XS_DEFAULT` fallback is `IDENTIFIER`; relevant when adding default-like keys.

## Impact

- **Files changed:** ~10–15
  - `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingTokenTypes.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt` (new)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupportTest.kt` (new)
  - `tools/xs-language-server/src/server.rs`
  - `tools/xs-language-server/src/semantic_tokens.rs` (new)
  - `tools/xs-language-server/src/symbols.rs`
  - `tools/xs-language-server/src/workspace.rs` / `merged_view.rs`
  - `tools/intellij-xs-plugin/gradle.properties`
  - `AGENTS.md`
- **New files:** 2–3.
- **LOC delta:** +~700–1000 / -~10–20.
- **Risk:** medium.
- **Effort:** ~1–2 weeks.
- **Compatibility risk:** low. The change is additive; existing color-scheme keys persist by external name.
- **Patch-maintenance impact:** none — this is tooling only; no mod overlay `.xs` scripts change.
- **Rollback plan:** revert the Rust/Kotlin/properties changes; the LSP server and plugin are independently versioned.
- **pluginVersion bump:** `0.4.0 → 0.5.0` (MINOR per `AGENTS.md`).

## Success criteria

- [ ] `include` is highlighted as a keyword.
- [ ] Braces, brackets, parens, comma, dot, semicolon, and operation signs use the user's chosen Braces and Operators colors (not `Default`).
- [ ] The 8 new semantic-token categories are visible in **Settings → Editor → Color Scheme → XS**.
- [ ] Engine / modded / unmodded functions and variables are colored differently in the editor.
- [ ] Local variables and built-in types are highlighted.
- [ ] 3c is either fixed or documented in the color-settings page.
- [ ] `cargo test` for the LSP still passes (≥ 215 tests).
- [ ] `./gradlew :test` for the plugin still passes (≥ 68 tests).
- [ ] `pluginVersion` is `0.5.0` in the built `.zip`.
- [ ] Manual Rider smoke test confirms all sub-tasks.

## Future work (γ — documented, deferred)

- Full class extraction in the LSP symbol table and plugin keys for `UnModded class` / `Modded class` applied to all class declarations.
- Dedicated `extern` variable category and color key.
- Engine / modded / unmodded constant classification.
- `UnModded Rule` / `Modded Rule` classification once rule detection is added to the symbol table.
- Cross-file forward-declaration ordering as a separate SDD change.

## Risks and unknowns

- **TextMate vs native highlighter dominance (R7-F-17).** If TextMate remains the primary color driver, `XsSyntaxHighlighter` fixes for braces/operators may not be visible without additional TextMate remapping or promoting the native highlighter.
- **Identifier-under-caret may be unfixable per-language.** The platform's `IdentifierHighlighterPass` hard-codes the global key; a real per-language override requires a custom highlighter pass.
- **LSP semantic-token API shape.** The exact `LspSemanticTokensSupport` customization methods must be verified against the bundled 2024.2 platform sources during design/apply.
- **Local-variable extraction may duplicate parser work.** Refactor or extend existing semantic/typecheck layers rather than re-implementing.
- **Performance.** Semantic tokens are requested on every edit; generation must be bounded per file and avoid re-reading the full include closure on each keystroke.
- **Classification overlap.** A symbol can be engine *and* modded if a mod overrides an engine name; final precedence rule must be spec'd (e.g., workspace symbol > engine API).

## Open questions

1. Does the native `XsSyntaxHighlighter` or the TextMate bundle win for `.xs` files in 2024.2.2? Do we need to remap TextMate scopes or register the native highlighter as primary?
2. What is the exact override API on `LspSemanticTokensSupport` for the bundled platform version?
3. Should built-in types (`bool`, `int`, `float`, `string`, `vector`) be emitted as LSP semantic tokens of type `type` with modifier `builtin`, or remain lexical `keyword` tokens?
4. What is the precedence rule when a modded symbol shadows an engine symbol of the same name?
5. Should the plugin converter map `static`/`extern` function modifiers in addition to the origin modifiers?

## Next step

The spec phase should formalize the exact LSP semantic-token legend, source-classification rules, `LspSemanticTokensSupport` color-mapping API, the native-vs-TextMate primary-highlighter decision, and strict-TDD test scenarios for grammar/lexer, highlighter mapping, and semantic-token converter before any apply work begins.
