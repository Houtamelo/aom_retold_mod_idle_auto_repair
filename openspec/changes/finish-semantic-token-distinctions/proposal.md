# Proposal: Finish deferred semantic-token distinctions

## Intent

Closes the remaining Bucket C gaps from `expand-color-scheme-semantic-tokens` (`0.4.1 → 0.6.0`): no `Class` symbol kind, constants/rules/extern misclassified in semantic tokens, and the plugin converter unwired from the platform.

## Scope

### In Scope
- Add `SymbolKind::Class` and extract the class name from `class_specifier`.
- Extend the LSP legend with `constant`, `rule`, and `extern`.
- Emit `constant`, `rule`, and origin+`extern` modifier combos.
- Add plugin keys/descriptors for Constant, Rule, and extern variables; switch class-key fallback to `CLASS_NAME`.
- Implement `XsSemanticTokensSupport`, override `LspServerDescriptor.lspCustomization`, and delegate to `XsSemanticTokensConverter`.
- Add Rust/Kotlin tests and bump `pluginVersion` to `0.7.0`.

### Out of Scope
- Class member extraction, cross-file forward-decl ordering, recoloring existing categories, engine `extern` classification.

## Capabilities

### New Capabilities
- `lsp-class-extraction`: Add `Class` to the LSP symbol table.
- `lsp-semantic-token-distinctions`: Extend semantic-token legend, emission, and IntelliJ color wiring.

### Modified Capabilities
- None.

## Spec Deltas

- `specs/spec-lsp-class-extraction.md` (new)
- `specs/spec-lsp-semantic-token-distinctions.md` (new)

## Approach

Server side: add `Class` to `SymbolKind`, walk `class_specifier`, and extend `semantic_tokens.rs` with new token types, modifiers, and classifier branches. Emit origin and `extern` together so the plugin can map four distinct extern-variable colors.

Plugin side: implement `XsSemanticTokensSupport` delegating `(tokenType, modifiers)` to `XsSemanticTokensConverter`, register it via `lspCustomization`, and add missing keys/descriptors. Platform properties are confirmed; the mapping-method signature and minimum platform version are verified in a pre-spec spike.

## Affected Areas

- `tools/xs-language-server/src/symbols.rs` — `Class` extraction.
- `tools/xs-language-server/src/semantic_tokens.rs` — legend + emission.
- `tools/intellij-xs-plugin/.../lsp/` — `XsSemanticTokensSupport.kt` + `lspCustomization`.
- `tools/intellij-xs-plugin/.../highlight/` — new keys/descriptors.
- `tools/intellij-xs-plugin/gradle.properties` — version bump.

## Risks

- **Platform API**: mapping-method signature could differ. *Mitigation*: stub subclass spike.
- **Unwired converter**: missing `lspCustomization` would leave colors dead. *Mitigation*: test for non-default customizer.
- **Cache schema**: warm symbol caches may break. *Mitigation*: integration test + cache clear.
- **Rule value**: tokens highlight rule definitions only (string registrations unchanged).

## pluginVersion Impact

**MINOR: `0.6.0 → 0.7.0`.** New token types, color categories, and platform wiring are user-visible features.

## Slice Plan

**One slice / single PR.** The legend and plugin converter are tightly coupled; delta is ~270–350 LOC.

## Rollback Plan

Revert the PR. If symbol-cache issues appear, clear `~/.local/state/aomr_lsp/v2/` and reinstall the previous `0.6.0` zip from `dist/`.

## Dependencies

- Bundled IntelliJ Platform must expose `LspCustomization` and `LspSemanticTokensSupport`. The cached `2024.2` build lacks them; target `2024.2.2+`.

## Open Questions

1. Exact `LspSemanticTokensSupport` mapping-method signature.
2. Minimum platform version/build exposing the required classes.

## Acceptance Criteria

- [ ] `cargo test` passes new LSP tests for class origin, constant, rule, and extern.
- [ ] `./gradlew test` passes plugin converter and `lspCustomization` tests.
- [ ] XS Color Scheme page shows **Constant**, **Rule**, and **Extern Variable** categories and `pluginVersion` is `0.7.0`.
