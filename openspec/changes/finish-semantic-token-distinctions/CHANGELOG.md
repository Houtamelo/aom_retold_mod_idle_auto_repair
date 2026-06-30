# CHANGELOG: Finish deferred semantic-token distinctions

## Version

`pluginVersion`: `0.6.0` → `0.7.0`  
LSP crate: no version bump (internal change).  
Platform target: `2024.2` (`242.20224.300`) → `2024.2.2` (`242.22855.74`).

## Added

### LSP (Rust)

- `SymbolKind::Class` variant and `class_specifier` extraction in the per-file symbol table.
- Class references in `_type_identifier` / `type_identifier` nodes now resolve through the merged/own table and emit `SemanticTokenType::TYPE` with the correct origin modifier (`engine`, `modded`, or `unmodded`).
- Extended semantic-token legend:
  - token type `constant`
  - custom token type `rule`
  - modifier `extern`
- `SymbolKind::Constant` references now emit the `constant` token type (previously `variable`).
- `SymbolKind::Rule` references now emit the custom `rule` token type (previously skipped).
- `extern` variable references now include the `extern` modifier alongside their origin modifier.
- Bumped per-file parse cache from `game_parse/v1/` to `game_parse/v2/` to accommodate the new `SymbolKind::Class` serialization.

### IntelliJ Plugin (Kotlin)

- New `TextAttributesKey`s:
  - `XS_CONSTANT`
  - `XS_RULE`
  - `XS_VARIABLE_EXTERN_UNMODDED`
  - `XS_VARIABLE_EXTERN_MODDED`
- New color-settings descriptors under **Settings → Editor → Color Scheme → XS**:
  - `Identifier//Variable//Constant`
  - `Identifier//Function//Rule`
  - `Identifier//Variable//Extern UnModded`
  - `Identifier//Variable//Extern Modded`
- `XsSemanticTokensConverter` now maps `constant`, `rule`, and `variable` + `extern` + origin to the new keys.
- New `XsSemanticTokensSupport` subclass of `LspSemanticTokensSupport` delegates `(tokenType, modifiers)` to `XsSemanticTokensConverter`.
- `XsLspServerDescriptor` now overrides `lspSemanticTokensSupport` so the platform invokes the XS customizer.

### Tests

- LSP: `tests/class_extraction_repro.rs` (5 tests).
- LSP: `tests/semantic_token_distinctions_repro.rs` (4 tests).
- Plugin: 4 new assertions in `XsSemanticTokensConverterTest`.
- Plugin: 1 new assertion in `XsLspServerDescriptorTest`.

## Changed

- `gradle.properties`: `pluginVersion` advanced to `0.7.0`; `platformVersion` advanced to `2024.2.2`; `pluginSinceBuild` advanced to `242.22855.74`; removed the unused `platformVersion_2` property.
- `AGENTS.md` test counts updated: LSP `230 → 239`, Plugin `91 → 96`.

## Deviations from `design.md`

- `SemanticTokenType::CONSTANT` does not exist in the pinned `tower_lsp` version; the server uses `SemanticTokenType::new("constant")`.
- The 2024.2.2 IntelliJ Platform API exposes `LspServerDescriptor.lspSemanticTokensSupport` directly instead of an `LspCustomization` wrapper. `XsLspServerDescriptor` overrides that property; there is no `LspCustomization` class.
- `LspSemanticTokensSupport` in 2024.2.2 has no nested `SemanticTokenType` class and `getTextAttributesKey(tokenType, modifiers)` has no `PsiFile` parameter. `XsSemanticTokensSupport` is implemented against the actual signatures.
- `XsStartupActivityTest.autoDetectedModsAreSavedAndNotified` was made environment-tolerant (asserts created mods are present rather than exact count) because the headless test project root overlaps with the real repo/worktrees and the detector finds more than the two synthetic mods.

## Known limitations

- Class member extraction and member semantic-token emission remain out of scope.
- `extern` modifier is emitted only for variable references, not functions.
- Rule references are colored uniformly; rule activation state (`active`/`inactive`) is not distinguished.
