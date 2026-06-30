# spec-lsp-semantic-token-distinctions

> **Status**: Active — promoted from `openspec/changes/finish-semantic-token-distinctions/` (commit `dfa829d`, 2026-06-30).

## Purpose

This specification extends the XS LSP semantic-token pipeline and the IntelliJ plugin color wiring so that constants, rules, and `extern` variables are rendered as distinct categories, and so that the plugin's existing semantic-token converter is actually invoked by the platform.

## Background

The current LSP legend advertises only `function`, `variable`, and `type` token types, plus `engine`, `modded`, `unmodded`, `local`, and `static` modifiers. Consequences:

- `SymbolKind::Constant` references are classified as `SemanticTokenType::VARIABLE`, making them visually identical to mutable variables.
- `SymbolKind::Rule` references are skipped entirely (`return None`) because the legend contains no matching token type.
- `extern` variables carry `Symbol::is_extern == true` and `Visibility::Extern`, but the classifier emits no `extern` modifier, so they render like ordinary top-level variables.
- On the plugin side, `XsSemanticTokensConverter` exists but is never called. `XsLspServerDescriptor` does not override platform semantic-token customization, and no `XsSemanticTokensSupport` subclass exists, so every semantic-token category — existing and new — is dead code from the editor's point of view.

## Requirements

### R1 — `constant` token type

The LSP semantic-token legend SHALL be extended to include the standard token type `SemanticTokenType::CONSTANT`.

- GIVEN an LSP client initializes the XS server, WHEN the server advertises its semantic-token legend, THEN the legend SHALL contain `constant`.

### R2 — `rule` token type

The LSP semantic-token legend SHALL be extended to include a custom `rule` token type (for example, `SemanticTokenType::new("rule")`).

- GIVEN an LSP client initializes the XS server, WHEN the server advertises its semantic-token legend, THEN the legend SHALL contain `rule`.

### R3 — `extern` modifier

The LSP semantic-token legend SHALL be extended to include an `extern` modifier.

- GIVEN an LSP client initializes the XS server, WHEN the server advertises its semantic-token legend, THEN the legend SHALL contain `extern`.

### R4 — Emit `constant` tokens

The LSP `classify_identifier` function SHALL emit `SemanticTokenType::CONSTANT` for references that resolve to a `SymbolKind::Constant` symbol, replacing the current `SemanticTokenType::VARIABLE` emission. The origin modifier of the defining file (`engine`, `modded`, or `unmodded`) SHALL still be emitted.

- GIVEN a constant `const int MAX = 100;` declared in `mod/spire_ai/game/foo.xs`, WHEN a reference `MAX` is parsed in another file, THEN the LSP SHALL emit `SemanticTokenType::CONSTANT` with modifier `modded`.

### R5 — Emit `rule` tokens

The LSP `classify_identifier` function SHALL emit the new `rule` token type for references that resolve to a `SymbolKind::Rule` symbol, replacing the current `return None` behavior. The origin modifier of the defining file SHALL still be emitted.

- GIVEN a rule `rule init() {}` declared in `<AOMR>/game/foo.xs`, WHEN a reference `init()` is parsed, THEN the LSP SHALL emit the `rule` token type with modifier `unmodded`.

### R6 — Emit `extern` modifier for variables

The LSP `classify_modifiers` function SHALL emit the `extern` modifier for variable references whose defining symbol is declared `extern`. The existing origin modifier SHALL remain present, producing modifier sets such as `[extern, unmodded]` or `[extern, modded]`.

- GIVEN an `extern int gFoo;` declared in `<AOMR>/game/foo.xs`, WHEN a reference `gFoo` is parsed, THEN the LSP SHALL emit `SemanticTokenType::VARIABLE` with modifiers `[extern, unmodded]`.
- GIVEN an `extern int gMyModdedFoo;` declared in `mod/spire_ai/game/foo.xs`, WHEN a reference is parsed, THEN the LSP SHALL emit `SemanticTokenType::VARIABLE` with modifiers `[extern, modded]`.

### R7 — Wire platform semantic-token customizer

The plugin's `XsLspServerDescriptor` SHALL expose an XS-specific semantic-token customizer so the platform invokes the plugin converter.

- GIVEN an `XsLspServerDescriptor` instance, WHEN its semantic-token support property is queried, THEN it SHALL return a non-null customizer that is an instance of `XsSemanticTokensSupport`.

### R8 — Implement `XsSemanticTokensSupport`

The plugin SHALL provide a new `XsSemanticTokensSupport` class that extends the platform's semantic-token support class and overrides the mapping method that converts `(tokenType, modifiers)` into a `TextAttributesKey`. The override SHALL delegate the pair to `XsSemanticTokensConverter`.

- GIVEN a semantic-token customizer is installed, WHEN the editor receives a token with type `constant`, THEN the customizer SHALL return the `CONSTANT` `TextAttributesKey`.

### R9 — Advertise custom token types to the platform

The `XsSemanticTokensSupport.getTokenTypes()` override SHALL return the union of the standard LSP token-type identifiers and the custom `rule` type, so that the platform's Color Scheme UI recognizes the new categories.

- GIVEN `XsSemanticTokensSupport.getTokenTypes()` is called, WHEN the result is inspected, THEN it SHALL contain at least `function`, `variable`, `type`, `constant`, and `rule`.

### R10 — New `TextAttributesKey`s

The plugin's `XsTextAttributes` SHALL add the following keys:

- `CONSTANT` for the `constant` token type.
- `RULE` for the `rule` token type.
- `VARIABLE_EXTERN_UNMODDED` for `variable` + `extern` + `unmodded`.
- `VARIABLE_EXTERN_MODDED` for `variable` + `extern` + `modded`.

- GIVEN the plugin is built, WHEN `XsTextAttributes` is inspected, THEN the four keys above SHALL exist.

### R11 — New color-settings descriptors

The plugin's `XsColorSettingsPage` SHALL add four new `AttributesDescriptor` entries, grouped under:

- `Identifier//Variable//Constant`
- `Identifier//Function//Rule`
- `Identifier//Variable//Extern UnModded`
- `Identifier//Variable//Extern Modded`

- GIVEN the user opens **Settings → Editor → Color Scheme → XS**, WHEN the page renders, THEN the four descriptors above SHALL be visible.

### R12 — Extend `XsSemanticTokensConverter`

The plugin's `XsSemanticTokensConverter` SHALL add mappings for the four new `(tokenType, modifiers)` combinations to the keys defined in R10. Unknown combinations SHALL continue to fall back to `XS_DEFAULT`.

- GIVEN the converter is invoked with `tokenType == "constant"` and any origin modifier, WHEN it maps to a key, THEN it SHALL return `CONSTANT`.
- GIVEN the converter is invoked with `tokenType == "rule"` and any origin modifier, WHEN it maps to a key, THEN it SHALL return `RULE`.
- GIVEN the converter is invoked with `tokenType == "variable"`, modifier `extern`, and origin `unmodded`, WHEN it maps to a key, THEN it SHALL return `VARIABLE_EXTERN_UNMODDED`.
- GIVEN the converter is invoked with `tokenType == "variable"`, modifier `extern`, and origin `modded`, WHEN it maps to a key, THEN it SHALL return `VARIABLE_EXTERN_MODDED`.

## Scenarios

### S1 — Constant reference in a mod file

- GIVEN a constant `const int MAX = 100;` declared in `mod/spire_ai/game/foo.xs`,
- WHEN a reference `MAX` is parsed in another file,
- THEN the LSP SHALL emit `SemanticTokenType::CONSTANT` with modifier `modded`, AND the plugin SHALL render it with the user's chosen Constant color.

### S2 — Rule reference in vanilla

- GIVEN a rule `rule init() {}` declared in `<AOMR>/game/foo.xs`,
- WHEN a reference `init()` is parsed,
- THEN the LSP SHALL emit the `rule` token type with modifier `unmodded`, AND the plugin SHALL render it with the user's chosen Rule color.

### S3 — Extern variable reference in vanilla

- GIVEN an `extern int gFoo;` declared in `<AOMR>/game/foo.xs`,
- WHEN a reference `gFoo` is parsed,
- THEN the LSP SHALL emit `SemanticTokenType::VARIABLE` with modifiers `[extern, unmodded]`, AND the plugin SHALL map it to `VARIABLE_EXTERN_UNMODDED`.

### S4 — Extern variable reference in modded code

- GIVEN an `extern int gMyModdedFoo;` declared in `mod/spire_ai/game/foo.xs`,
- WHEN a reference is parsed,
- THEN the LSP SHALL emit `SemanticTokenType::VARIABLE` with modifiers `[extern, modded]`, AND the plugin SHALL map it to `VARIABLE_EXTERN_MODDED`.

### S5 — Color-scheme page shows new categories

- GIVEN the plugin is installed in Rider 2024.2.2+,
- WHEN the user opens an XS file and navigates to **Settings → Editor → Color Scheme → XS**,
- THEN the page SHALL list the four new categories: Constant, Rule, Extern UnModded, and Extern Modded.

### S6 — Constant reference colored correctly

- GIVEN the plugin is installed and a constant reference is rendered in an XS editor,
- THEN it SHALL be colored with the user's chosen Constant color.

### S7 — Rule reference colored correctly

- GIVEN the plugin is installed and a rule reference is rendered in an XS editor,
- THEN it SHALL be colored with the user's chosen Rule color.

### S8 — Extern variable color depends on origin

- GIVEN the plugin is installed and an `extern` variable reference is rendered,
- THEN the color SHALL depend on whether the variable is in vanilla or modded code (`VARIABLE_EXTERN_UNMODDED` vs. `VARIABLE_EXTERN_MODDED`).

## Out of scope

- Distinguishing different `extern` storage types (for example, `extern static`).
- Distinguishing rule modifiers (for example, `rule active` vs. `rule inactive`).
- Class member extraction or member semantic-token emission.
- Cross-file forward-declaration ordering.
- Recoloring existing categories other than the four new distinctions.

## Verification approach

- Strict-TDD Rust tests in `tools/xs-language-server/tests/semantic_token_distinctions_repro.rs` covering S1–S4.
- Plugin converter tests in `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt` covering the new mappings in R12.
- A plugin test asserting that `XsLspServerDescriptor` exposes an XS-specific semantic-token customizer (R7).
- `XsColorSettingsPageTest` extended to assert the descriptors from R11 are present (S5).
- `./gradlew buildPlugin` confirms the artifact is produced.
- Manual Rider smoke test confirming that constant, rule, and extern-variable references are colored as specified in S6–S8.
