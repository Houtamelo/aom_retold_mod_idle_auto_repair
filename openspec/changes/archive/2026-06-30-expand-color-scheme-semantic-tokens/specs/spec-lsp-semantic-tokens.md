# Delta Spec: LSP Semantic Tokens for XS Color Scheme

**Change:** `expand-color-scheme-semantic-tokens`

## Capability summary

The XS LSP server SHALL emit `textDocument/semanticTokens/full` tokens for functions, variables, and types, classifying each by origin (`engine`, `modded`, `unmodded`) and, for variables, by storage class (`local`, `static`). The IntelliJ plugin SHALL map those tokens to new color-scheme categories and fix `include`, brace/operator, and identifier-under-caret coloring.

## Out of scope (deferred)

- `extern` variable as its own color key.
- Engine/Modded/Unmodded constant classification.
- `UnModded Rule` / `Modded Rule` classification.
- Cross-file forward-declaration ordering (separate future SDD change).

## Cross-references

- `docs/issues/2026-06-29-runtime-issues.md` Issue 3 and the Rider smoke-test findings.
- `openspec/specs/spec-xs-color-scheme-categories.md` (Option α).
- `AGENTS.md`.
- This change implements a minimal Bucket C subset.

## ADDED Requirements

### R1 — `include` keyword highlighting

The TextMate grammar and native XS lexer SHALL treat `include` as a keyword.

- GIVEN `xs.tmLanguage.json` and `XsHighlightingLexer.kt`, WHEN inspected, THEN both keyword sets contain `include`.

### R2 — Brace / operator / punctuation coloring

`XsSyntaxHighlighter` SHALL map braces, brackets, parentheses, comma, dot, semicolon, and operation-sign tokens to the inherited `Braces and Operators` keys (`XS_BRACES`, `XS_BRACKETS`, `XS_PARENTHESES`, `XS_COMMA`, `XS_DOT`, `XS_SEMI_COLON`, `XS_OPERATION_SIGN`, `XS_OVERLOADED_OPERATOR`).

- GIVEN brace/operator/punctuation tokens in an `.xs` file, WHEN `XsSyntaxHighlighter` maps them, THEN each resolves to its declared key.

### R3 — Identifier under caret

The plugin SHALL make the per-language **Identifier under caret** setting work, or document the platform limitation in the XS color-scheme page.

- GIVEN the user changes **Color Scheme → XS → Identifier under caret**, WHEN the caret is in an identifier, THEN the color applies if supported; OTHERWISE the page states the global **General → Identifier under caret** setting must be used.

### R4 — Engine / Modded / Unmodded function distinction

The LSP server SHALL emit function tokens with origin modifier; the plugin SHALL map them to `XS_FUNCTION_ENGINE`, `XS_FUNCTION_UNMODDED`, and `XS_FUNCTION_MODDED`. `engine` = doxygen API; `modded` = mod `game/` overlay; `unmodded` = vanilla `game/` not overridden. Precedence: `modded` > `engine`.

- GIVEN engine/modded/unmodded function references, WHEN tokens are produced, THEN each emits the expected `(type, modifier)` and modded shadows engine.

### R5 — Local / Static variable distinction

The LSP server SHALL emit variable tokens with `local` or `static` modifier; the plugin SHALL map them to `XS_VARIABLE_LOCAL` and `XS_VARIABLE_STATIC`. `local` = inside a function/block, not `static`/`extern`; `static` = declared `static`.

- GIVEN local and `static` variable references, WHEN tokens are produced, THEN local emits `local` and static emits `static`.

### R6 — Built-in / UnModded / Modded type

The LSP server SHALL emit type tokens with `engine`, `unmodded`, or `modded` modifier; the plugin SHALL map them to `XS_TYPE_BUILTIN`, `XS_TYPE_UNMODDED_CLASS`, and `XS_TYPE_MODDED_CLASS`. `engine` = `bool`/`int`/`float`/`string`/`vector`; `unmodded` = vanilla class; `modded` = mod class.

- GIVEN built-ins plus unmodded/modded class references, WHEN tokens are produced, THEN built-ins emit `engine` and classes emit their origin.

### R7 — LSP semantic-token capability and legend

The LSP server SHALL advertise `textDocument/semanticTokens/full` with a legend containing at least the token types `function`, `variable`, and `type`, and the modifiers `engine`, `modded`, `unmodded`, `local`, and `static`.

- GIVEN an LSP client initializes the XS server, WHEN `ServerCapabilities` are returned, THEN `semanticTokensProvider` is present with the required legend.

### R8 — Plugin semantic-token color mapping

The plugin SHALL register semantic `TextAttributesKey`s for Engine/UnModded/Modded function, Local/Static variable, Built-in type, UnModded/Modded class and convert received tokens to those keys.

- GIVEN the user opens **Color Scheme → XS**, WHEN the page renders, THEN the semantic categories are listed and applied.

### R9 — pluginVersion bump

The committed `pluginVersion` SHALL be `0.5.0`.

- GIVEN `tools/intellij-xs-plugin/gradle.properties`, WHEN read, THEN `pluginVersion = 0.5.0`.

## MODIFIED Requirements

None.

## REMOVED Requirements

None.

## Verification approach

- `./gradlew :test` passes, including `XsColorSettingsPageTest` and a new semantic-token test.
- `cargo test --manifest-path tools/xs-language-server/Cargo.toml` passes and includes semantic-token emitter tests.
- `./gradlew buildPlugin` produces `.zip` with `pluginVersion = 0.5.0`.
- Manual Rider smoke test confirms `include`, braces/operators, and semantic function coloring.
