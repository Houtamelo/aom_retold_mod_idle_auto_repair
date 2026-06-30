# Tasks: Finish deferred semantic-token distinctions

## 1. Overview

This change closes the remaining Bucket C gaps from `expand-color-scheme-semantic-tokens` by adding `Class` to the LSP symbol table, extending the semantic-token legend with `constant`, `rule`, and `extern`, and wiring the IntelliJ plugin's converter into the platform so the new categories actually render in the editor. Work is server-first: the Rust LSP gains symbol-kind and token-emission changes, then strict-TDD tests lock the behavior, then the Kotlin plugin receives new color keys/descriptors and the `LspCustomization`/`LspSemanticTokensSupport` plumbing.

The change is intentionally one slice / one PR. The LSP legend and plugin converter are tightly coupled, and the expected delta is ~350–400 LOC across ~25 files. `pluginVersion` advances by MINOR to `0.7.0`, and the platform target moves to `2024.2.2` (`242.22855.74`) because the required `LspCustomization`/`LspSemanticTokensSupport` APIs are not present in the cached `2024.2` build.

## 2. Implementation tasks

### Phase A: LSP symbol table changes (Rust)

- [x] **Task A.1**: Add `SymbolKind::Class` variant to `tools/xs-language-server/src/symbols.rs` and update `label()` to return `"class"`.
  - Verification: `cargo check --manifest-path tools/xs-language-server/Cargo.toml` passes; existing 230 tests still pass.

- [x] **Task A.2**: Add `is_extern: bool` field to the `Symbol` struct in `tools/xs-language-server/src/symbols.rs` and default it to `false` at every `Symbol { ... }` construction site.
  - **Note**: field already present and defaulted; verified with `cargo check`.
  - Verification: `cargo check` passes; no behavioral change yet.

- [x] **Task A.3**: Add a `class_specifier` walker to `tools/xs-language-server/src/symbols.rs::build_full_symbol_table` that captures only the class name, full range, and selection range; do not walk `field_declaration_list` members.
  - Verification: `cargo check` passes; `cargo test` shows the new A.7/B.1 tests passing.

- [x] **Task A.4**: Add an `extern` walker (or extend existing declaration extraction) in `tools/xs-language-server/src/symbols.rs::build_full_symbol_table` to set `is_extern: true` on symbols declared `extern`.
  - **Note**: `extract_modifiers` already sets `is_extern` for variables/functions; verified with `cargo check`.
  - Verification: `cargo check` passes.

- [x] **Task A.5**: Update the per-file parse-cache schema in `tools/xs-language-server/src/cache.rs`: move from `game_parse/v1/` to `game_parse/v2/` and update the directory-layout comment.
  - Verification: `cargo check` passes; v1 caches are invalidated and rebuilt on first run.

- [x] **Task A.6**: Verify that `SemanticTokenType::CONSTANT` exists in `tower_lsp::lsp_types`; update imports in `tools/xs-language-server/src/semantic_tokens.rs` as needed.
  - **Note**: `SemanticTokenType::CONSTANT` already available from existing import; no change required.
  - Verification: `cargo check` passes.

- [x] **Task A.7**: Update `tools/xs-language-server/src/semantic_tokens.rs`:
  - Append `SemanticTokenType::CONSTANT` and `SemanticTokenType::new("rule")` to `TOKEN_TYPES`.
  - Append `SemanticTokenModifier::new("extern")` to `TOKEN_MODIFIERS`.
  - In `classify_identifier`, emit `CONSTANT` for `SymbolKind::Constant` and the custom `rule` type for `SymbolKind::Rule`.
  - In `classify_modifiers`, emit the `extern` modifier when `symbol.kind == SymbolKind::Variable && symbol.is_extern`.
  - Confirm `classify_type_identifier` resolves class references through the merged/own table without further changes.
  - Verification: `cargo build --manifest-path tools/xs-language-server/Cargo.toml` passes.

### Phase B: LSP strict-TDD tests

- [x] **Task B.1**: Create `tools/xs-language-server/tests/class_extraction_repro.rs` with 5 strict-TDD tests:
  1. Vanilla class reference emits `TYPE` with `unmodded` modifier.
  2. Modded class reference emits `TYPE` with `modded` modifier.
  3. Unknown class reference falls back to `engine` modifier.
  4. A `class` declaration does not crash the symbol-table walker.
  5. A file with 50+ class declarations builds in less than 100 ms.
  - Verification: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test class_extraction_repro --no-fail-fast` passes.

- [x] **Task B.2**: Create `tools/xs-language-server/tests/semantic_token_distinctions_repro.rs` with 4 strict-TDD tests:
  1. Constant reference emits the `constant` token type.
  2. Rule reference emits the `rule` token type (instead of `None`).
  3. Extern variable emits `variable` plus the `extern` and origin modifiers.
  4. New token types/modifiers compose correctly with existing modifiers.
  - Verification: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test semantic_token_distinctions_repro --no-fail-fast` passes.

- [x] **Task B.3**: Run the full LSP test suite.
  - Verification: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast` passes with 239 tests (230 baseline + 9 new).

### Phase C: Plugin text attributes + descriptors (Kotlin)

- [x] **Task C.1**: Add four new `TextAttributesKey` entries to `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt`:
  - `CONSTANT` → fallback `DefaultLanguageHighlighterColors.CONSTANT`
  - `RULE` → fallback `DefaultLanguageHighlighterColors.FUNCTION_DECLARATION`
  - `VARIABLE_EXTERN_UNMODDED` → fallback `DefaultLanguageHighlighterColors.GLOBAL_VARIABLE`
  - `VARIABLE_EXTERN_MODDED` → fallback `DefaultLanguageHighlighterColors.GLOBAL_VARIABLE`
  - Verification: `./gradlew :compileKotlin` passes.

- [x] **Task C.2**: Add four new `AttributesDescriptor` entries to `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt`:
  - `Identifier//Variable//Constant`
  - `Identifier//Function//Rule`
  - `Identifier//Variable//Extern UnModded`
  - `Identifier//Variable//Extern Modded`
  - Verification: `./gradlew :compileKotlin` passes.

### Phase D: Plugin semantic-tokens wiring (Kotlin)

- [x] **Task D.1**: Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt`:
  - Extend `LspSemanticTokensSupport()`.
  - Override `getTextAttributesKey(tokenType: String, modifiers: List<String>, file: PsiFile): TextAttributesKey?` and delegate to `XsSemanticTokensConverter.convert(tokenType, modifiers.toSet())`.
  - Override `getTokenTypes(): List<SemanticTokenType>` to return the standard LSP types plus `SemanticTokenType("rule")`.
  - Verification: `./gradlew :compileKotlin` passes after verifying exact `SemanticTokenType` constructor against the bundled 2024.2.2 sources.

- [x] **Task D.2**: Update `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt`:
  - Override `lspCustomization` to return an `LspCustomization` instance whose `semanticTokensCustomizer` is `XsSemanticTokensSupport()`.
  - Verification: `./gradlew :compileKotlin` passes.

- [x] **Task D.3**: Update `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSemanticTokensConverter.kt`:
  - Add `constant` → `CONSTANT` and `rule` → `RULE` mappings.
  - Add `variable` + `extern` + `unmodded` → `VARIABLE_EXTERN_UNMODDED` and `variable` + `extern` + `modded` → `VARIABLE_EXTERN_MODDED`.
  - Keep the existing `Set<String>` signature.
  - Verification: `./gradlew :compileKotlin` passes.

### Phase E: Plugin tests

- [x] **Task E.1**: Add 4 new tests to `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt` covering `constant`, `rule`, `variable+extern+unmodded`, and `variable+extern+modded`.
  - Verification: `./gradlew :test --tests "com.aomr.xs.lsp.XsSemanticTokensConverterTest"` passes.

- [x] **Task E.2**: Create `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsLspServerDescriptorTest.kt` asserting that `lspCustomization` returns a non-null customization with an `XsSemanticTokensSupport` instance as `semanticTokensCustomizer`.
  - **Note**: 2024.2.2 exposes `LspServerDescriptor.lspSemanticTokensSupport` directly; the test asserts that property is an `XsSemanticTokensSupport` instance instead of the indirection through `LspCustomization`.
  - Verification: `./gradlew :test --tests "com.aomr.xs.lsp.XsLspServerDescriptorTest"` passes.

- [x] **Task E.3**: Run the full plugin test suite.
  - Verification: `cd tools/intellij-xs-plugin && ./gradlew :test --no-daemon --offline` passes with 96 tests (91 baseline + 5 new).

### Phase F: Build artifacts + version bumps

- [x] **Task F.1**: Update `tools/intellij-xs-plugin/gradle.properties`:
  - `pluginVersion = 0.7.0`
  - `platformVersion = 2024.2.2`
  - `pluginSinceBuild = 242.22855.74`
  - Verify `pluginUntilBuild` remains `263.*` unless the platform bump requires a change.

- [x] **Task F.2**: If `tools/intellij-xs-plugin/build.gradle.kts` references `platformVersion` as a literal, update it; otherwise no change is needed.
  - **Note**: `build.gradle.kts` reads `platformVersion` from `gradle.properties`; no literal change required.
  - Verification: `./gradlew :compileKotlin` passes.

- [x] **Task F.3**: Build the plugin artifact.
  - Verification: `cd tools/intellij-xs-plugin && ./gradlew buildPlugin --no-daemon --offline` produces `build/distributions/intellij-xs-plugin-0.7.0.zip` and the file is larger than the existing 0.6.0 zip (4,132,718 bytes).

- [x] **Task F.4**: Build the LSP server release binary.
  - Verification: `cargo build --manifest-path tools/xs-language-server/Cargo.toml --release` passes cleanly.

### Phase G: Documentation + verification

- [x] **Task G.1**: Update `AGENTS.md` test counts:
  - LSP: 230 → 239
  - Plugin: 91 → 96
  - Update the trailing line in the "For `tools/xs-language-server/`" section.

- [x] **Task G.2**: Update `docs/issues/2026-06-29-runtime-issues.md` if it still references the deferred Bucket C items; mark them RESOLVED by this change.

- [x] **Task G.3**: Verify `README.md` for any Bucket C status line and update it if present.
  - **Note**: root `README.md` contains no Bucket C status line; no change required.

- [x] **Task G.4**: Add a CHANGELOG-style entry in `openspec/changes/finish-semantic-token-distinctions/CHANGELOG.md` (create the file if the convention requires it; use `openspec/changes/archive/2026-06-29-add-plugin-semantic-tasks.md` as a template).
  - **Note**: the referenced archive template does not exist; a CHANGELOG was created from the proposal/spec content.

## 3. Verification checklist (before commit)

- [ ] `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast` passes (239 tests).
- [ ] `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml -- -D warnings` passes, or any warnings are documented as pre-existing.
- [ ] `cd tools/intellij-xs-plugin && ./gradlew :test --no-daemon --offline` passes (96 tests).
- [ ] `cd tools/intellij-xs-plugin && ./gradlew buildPlugin --no-daemon --offline` produces `build/distributions/intellij-xs-plugin-0.7.0.zip`.
- [ ] Working tree noise (rustfmt churn, untracked research files) is not included in the commit.
- [ ] `git diff --stat` shows only the expected files modified (~20–25 files, ~400 LOC).
- [ ] `AGENTS.md` test counts are updated.
- [ ] Commit message prepared: `feat(xs-lsp, xs-plugin): finish Bucket C semantic-token distinctions (class extraction + constant/rule/extern + platform wiring)`.

## 4. Review workload forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~350–400 |
| New files | 4 (`class_extraction_repro.rs`, `semantic_token_distinctions_repro.rs`, `XsSemanticTokensSupport.kt`, `XsLspServerDescriptorTest.kt`) |
| Files changed | ~20–25 across LSP, plugin, properties, and docs |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Decision needed before apply | No |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low
