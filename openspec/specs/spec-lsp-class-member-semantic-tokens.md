# spec-lsp-class-member-semantic-tokens

> **Status**: Active — promoted from `openspec/changes/expand-color-scheme-classes-constants-rules/` (commits `9cc7344` + `e512f10`, 2026-06-30).

## Purpose

This specification extends the XS LSP semantic-token pipeline and the IntelliJ plugin's color wiring so that class member references (`obj.field`, `Class.method()`) are rendered as distinct categories with an explicit `member` modifier and the same origin distinction (`engine`, `unmodded`, `modded`) used for top-level symbols.

## Background

The prior slice promoted `spec-lsp-semantic-token-distinctions.md`, adding `constant`, `rule`, and `extern` to the LSP legend and wiring the plugin's `XsSemanticTokensConverter` into the platform. Class member references, however, continue to be treated as ordinary functions or variables because member symbols were not extracted. Now that class fields and methods are in the symbol table, the LSP can detect `field_expression` nodes and emit a `member` modifier that the plugin maps to dedicated colors.

## Requirements

### R1 — Add `member` modifier to legend

The LSP semantic-token legend SHALL include a `member` modifier, added to `TOKEN_MODIFIERS` in `semantic_tokens.rs`.

- GIVEN an LSP client initializes the XS server, WHEN the server advertises its semantic-token legend, THEN `member` SHALL be present in `tokenModifiers`.

### R2 — Detect member references in `field_expression` nodes

The LSP identifier classifier SHALL detect `field_expression` AST nodes and classify their `field_identifier` child as a member reference.

- GIVEN an expression `obj.health`, WHEN semantic tokens are computed, THEN the token for `health` SHALL be classified as a member reference.

### R3 — Static method calls

For a `field_expression` of the form `ClassName.method()`, the LSP SHALL emit the `function` token type with `[member, <origin>]` modifiers.

- GIVEN a method `init` defined in class `Foo`, WHEN a reference `Foo.init()` is parsed, THEN the LSP SHALL emit `function` with `[member, unmodded]` (or the origin of the owning class).

### R4 — Instance field access

For a `field_expression` of the form `obj.field`, the LSP SHALL emit the `variable` token type with `[member, <origin>]` modifiers.

- GIVEN a field `health` defined in class `Unit`, WHEN a reference `obj.health` is parsed, THEN the LSP SHALL emit `variable` with `[member, <origin>]` modifiers where `<origin>` is the origin of `Unit`.

### R5 — Member origin heuristic

The origin modifier for member references SHALL be determined by scanning all class member symbols in the workspace. If any modded class defines the member, the LSP SHALL emit `modded`; else if any unmodded (vanilla) class defines it, `unmodded`; else `engine`.

- GIVEN a method `takeDamage` defined ONLY in a vanilla class, WHEN a reference is parsed, THEN it SHALL emit `[member, unmodded]`.
- GIVEN the same method `takeDamage` ALSO defined in a modded class, WHEN a reference is parsed, THEN it SHALL emit `[member, modded]`.

### R6 — New member text-attribute keys

The plugin's `XsTextAttributes` SHALL add six `TextAttributesKey` entries:

- `METHOD_ENGINE` for tokens with type `function` and modifiers containing `member` and `engine`.
- `METHOD_UNMODDED` for `function` + `member` + `unmodded`.
- `METHOD_MODDED` for `function` + `member` + `modded`.
- `FIELD_ENGINE` for `variable` + `member` + `engine`.
- `FIELD_UNMODDED` for `variable` + `member` + `unmodded`.
- `FIELD_MODDED` for `variable` + `member` + `modded`.

- GIVEN the plugin is built, WHEN `XsTextAttributes` is inspected, THEN the six keys above SHALL exist.

### R7 — New color-settings descriptors

The plugin's `XsColorSettingsPage` SHALL add six `AttributesDescriptor` entries, grouped under:

- `Identifier//Function//Method Engine`
- `Identifier//Function//Method UnModded`
- `Identifier//Function//Method Modded`
- `Identifier//Variable//Field Engine`
- `Identifier//Variable//Field UnModded`
- `Identifier//Variable//Field Modded`

- GIVEN the user opens **Settings → Editor → Color Scheme → XS**, WHEN the page renders, THEN the six descriptors above SHALL be visible.

### R8 — Extend `XsSemanticTokensConverter`

The plugin's `XsSemanticTokensConverter` SHALL map each new `(tokenType, modifiers)` combination to the corresponding key from R6. Unknown combinations SHALL continue to fall back to `XS_DEFAULT`.

- GIVEN the converter is invoked with `tokenType == "function"`, modifier `member`, and origin `unmodded`, WHEN it maps, THEN it SHALL return `METHOD_UNMODDED`.
- GIVEN `tokenType == "variable"`, modifier `member`, and origin `modded`, WHEN it maps, THEN it SHALL return `FIELD_MODDED`.

### R9 — Advertise member-aware token categories

If the IntelliJ Platform requires new token types to be declared in order for the `member` modifier composition to be rendered, `XsSemanticTokensSupport.getTokenTypes()` SHALL include those token types.

- GIVEN a member-aware token type is introduced, WHEN `XsSemanticTokensSupport.getTokenTypes()` is inspected, THEN it SHALL include that type.

## Scenarios

### S1 — Vanilla method reference

- GIVEN a method `takeDamage` defined in vanilla class `Unit`,
- WHEN a reference `unit.takeDamage()` is parsed,
- THEN the LSP SHALL emit `function` with `[member, unmodded]` modifiers.

### S2 — Modded method reference

- GIVEN a method `takeDamage` defined in modded class `MyModdedUnit` (in `mod/spire_ai/`),
- WHEN a reference `unit.takeDamage()` is parsed in another mod file,
- THEN the LSP SHALL emit `function` with `[member, modded]` modifiers.

### S3 — Vanilla field reference

- GIVEN a field `health` defined in vanilla class `Unit`,
- WHEN a reference `unit.health` is parsed,
- THEN the LSP SHALL emit `variable` with `[member, unmodded]` modifiers.

### S4 — Ambiguous member name prefers modded origin

- GIVEN a method `init` defined in BOTH vanilla class `Foo` and modded class `MyFoo`,
- WHEN a reference `obj.init()` is parsed,
- THEN the LSP SHALL emit `function` with `[member, modded]`.

### S5 — Color scheme page shows new categories

- GIVEN the plugin is installed in Rider 2024.2.2+,
- WHEN the user opens **Settings → Editor → Color Scheme → XS**,
- THEN the page SHALL list the six new Method/Field Engine/UnModded/Modded categories.

### S6 — Vanilla member method colored correctly

- GIVEN the plugin is installed and a vanilla class method reference is rendered in an XS editor,
- THEN it SHALL be colored with the user's chosen "Method UnModded" color.

### S7 — Modded member method colored correctly

- GIVEN the plugin is installed and a modded class method reference is rendered in an XS editor,
- THEN it SHALL be colored with the user's chosen "Method Modded" color.

## Out of scope

- Type inference for the object expression in `obj.field` or `obj.method()`.
- Differentiating static versus instance members in coloring.
- Member modifiers like `static`, `final`, or `override` (if XS has them).
- Constructors and `new` expressions.
- Array built-in calls (`.size()`, `.add()`, `.clear()`).
- Cross-file member lookup via `include` paste.

## Verification approach

- Strict-TDD Rust tests in `tools/xs-language-server/tests/semantic_tokens_repro.rs` covering S1–S4.
- Plugin converter tests in `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt` covering the new mappings in R8.
- `XsColorSettingsPageTest` extended to assert the descriptors from R7 are present (S5).
- `./gradlew buildPlugin` confirms the artifact is produced.
- Manual Rider smoke test confirming member method and field references render with the user-selected colors in S6–S7.
