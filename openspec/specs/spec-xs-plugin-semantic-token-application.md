---
key: aom_retold_mod_idle_auto_repair/xs-plugin-semantic-token-application
title: XS Plugin Semantic-Token Application
status: archived
archived_at: 2026-06-30
added_by: 2026-06-30-fix-semantic-token-color-application
---

# XS Plugin Semantic-Token Application

> **Added by change:** `2026-06-30-fix-semantic-token-color-application`
> **Plugin version**: 0.8.2

## Capability summary

The XS IntelliJ plugin MUST apply semantic-token classifications to identifier and type usages in real `.xs` files so users can distinguish categories (functions, variables, types, etc.) and origins (engine, modded, unmodded) and storage class (local / static / extern / member). The semantic-tokens annotation SHALL be the single source of color for identifier positions; the in-place `XsSyntaxHighlighter` MUST NOT hard-wire `IDENTIFIER → XS_IDENTIFIER`.

> Change: `2026-06-30-fix-semantic-token-color-application`
> Branch: `xs-lsp-roundtrip-followup`
> Plugin version: 0.8.1 → 0.8.2 (PATCH)

## ADDED Requirements

### Requirement: Modifier-Legend Parity

The `XsSemanticTokensSupport.tokenModifiers` override SHALL return the same seven modifier names the LSP server advertises in `semantic_tokens::TOKEN_MODIFIERS`: `engine`, `modded`, `unmodded`, `local`, `static`, `extern`, `member` (in this order).

#### Scenario: Legend parity

- GIVEN the plugin is in an active project
- WHEN the platform reads `LspServerDescriptor.lspSemanticTokensSupport.tokenModifiers`
- THEN it returns `listOf("engine", "modded", "unmodded", "local", "static", "extern", "member")`
- AND the names match `semantic_tokens::TOKEN_MODIFIERS` in `tools/xs-language-server/src/semantic_tokens.rs`

#### Scenario: Modifier preservation in converter

- GIVEN the LSP server returns a token with tokenType=`function` and tokenModifiersBitset including `engine`
- WHEN the platform calls `XsSemanticTokensSupport.getTextAttributesKey("function", listOf("engine"))`
- THEN the converter returns the `FUNCTION_ENGINE` TextAttributesKey

### Requirement: Lexer-Level Identifier Default Removed

`XsSyntaxHighlighter.getTokenHighlights(XsTokenTypes.IDENTIFIER)` SHALL return an empty array so the platform's semantic-tokens annotation is the only source of identifier coloring.

#### Scenario: IDENTIFIER token type takes no default

- GIVEN any open `.xs` file
- WHEN `XsSyntaxHighlighter.getTokenHighlights(XsTokenTypes.IDENTIFIER)` is called
- THEN it returns an empty array
- AND it does NOT return `XsTextAttributesKeys.XS_IDENTIFIER`

#### Scenario: IDENTIFIER still resolvable as language default

- GIVEN a `.xs` file opened in the IDE before the LSP server is ready
- WHEN the platform resolves an identifier range against the language default
- THEN the platform default-color for the language (`HighlighterColors.TEXT`) is applied
- AND `XS_IDENTIFIER` appears only as a schema-level fallback for users editing the color scheme
- AND no per-token `XS_IDENTIFIER` is hardwired into the syntax-highlighter

### Requirement: Built-in Type Non-Keyword Inheritance

`XsTextAttributes.TYPE_BUILTIN` SHALL inherit from `DefaultLanguageHighlighterColors.IDENTIFIER`, NOT from `DefaultLanguageHighlighterColors.KEYWORD`, so semantic-tokens coloring yields a non-keyword appearance for `int`/`bool`/`float`/`string`/`vector` usages.

#### Scenario: TYPE_BUILTIN fallback

- GIVEN the platform resolves the fallback attribute for `XS_TYPE_BUILTIN`
- THEN it returns the value of `DefaultLanguageHighlighterColors.IDENTIFIER`
- AND it does NOT resolve to `DefaultLanguageHighlighterColors.KEYWORD`

### Requirement: TextMate Storage_Types Rule Removed

`xs.tmLanguage.json` SHALL NOT include the `storage_types` repository rule (or any rule matching `int|float|bool|void|string|vector|class`) that pre-empts semantic-token coloring by tagging these names with `storage.type.built-in.primitive.c`.

#### Scenario: No storage-types scope

- GIVEN the bundled TextMate grammar `xs.tmLanguage.json` is parsed
- WHEN the platform scans it for `storage.type.*` scope assignments
- THEN no pattern matches `int|float|bool|void|string|vector|class`

#### Scenario: Identifiers still match via c_function_call rule

- GIVEN the TextMate grammar is loaded
- WHEN an identifier-shaped name appears in a context the `c_function_call` rule already handles
- THEN it is matched as `entity.name.function.c` / `variable.other.c` per the existing grammar

## MODIFIED Requirements

None in this change. The fix only ADDS new behavior; no existing requirement is being re-worded.

## REMOVED Requirements

### Requirement: Storage_Types Pre-Emption of Primitive Type Coloring

(Reason: The TextMate `storage_types` rule tagged primitive-type keywords as `storage.type.built-in.primitive.c`, which the platform mapped to the keyword color and which pre-empted semantic-token-driven coloring. Its removal restores the semantic-token pipeline as the sole source of authority for those ranges.)

## End of delta
