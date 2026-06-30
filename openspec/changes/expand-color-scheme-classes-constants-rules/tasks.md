# Tasks: Expand class member extraction for semantic coloring

## 1. Overview

This change extracts class fields and methods from XS `class_specifier` bodies so member references such as `obj.field` and `Class.method()` can be resolved, receive a new `member` semantic-token modifier plus an origin modifier (`engine`/`unmodded`/`modded`), and appear nested in the IDE outline. Work proceeds server-first: the Rust LSP adds `SymbolKind::ClassField` and `ClassMethod`, a `class_owner` field, a class-body walker, a member-origin index, and a `member` modifier; then strict-TDD Rust tests lock the behavior; finally the Kotlin plugin receives six new member-specific text attributes, color descriptors, and converter branches.

This is intentionally one slice / one PR. Member extraction, the `member` modifier, the `game_parse/v2` → `v3` cache bump, and plugin color wiring are mutually dependent. The expected delta is ~600–750 changed lines across ~25–30 files. `pluginVersion` advances by MINOR to `0.8.0`; platform target remains `2024.2.2` (`242.22855.74`) from the prior slice.

## 2. Implementation tasks

### Phase A: LSP symbol table changes (Rust)

- [x] **A.1** Add `SymbolKind::ClassField` and `SymbolKind::ClassMethod` variants to `tools/xs-language-server/src/symbols.rs` and update `label()`.
  - Verification: `cargo check --manifest-path tools/xs-language-server/Cargo.toml` passes; existing 239 tests still pass.

- [x] **A.2** Add `#[serde(default)] pub class_owner: Option<String>` to `Symbol` in `tools/xs-language-server/src/symbols.rs`.
  - Update all `Symbol { ... }` construction sites to default `class_owner: None`.
  - Verification: `cargo check` passes; existing tests still pass.

- [x] **A.3** Add a `field_declaration` walker inside `class_specifier` extraction in `tools/xs-language-server/src/symbols.rs::build_full_symbol_table`.
  - Emit `Symbol { kind: ClassField, name: <field>, class_owner: Some(<class>), visibility: Local, ... }`.
  - Skip malformed/`ERROR` declarator children.
  - Verification: `cargo check` passes.

- [x] **A.4** Add a `function_declarator` walker inside `class_specifier` extraction in `tools/xs-language-server/src/symbols.rs::build_full_symbol_table`.
  - Emit `Symbol { kind: ClassMethod, name: <method>, class_owner: Some(<class>), visibility: Local, ... }`.
  - Verification: `cargo check` passes.

- [x] **A.5** Build a `MemberIndex` (member name → strongest origin) in `tools/xs-language-server/src/symbols.rs` or a new `member_index.rs`.
  - Scan own table and visible workspace symbol tables for `ClassField`/`ClassMethod`.
  - Heuristic: modded > unmodded > engine.
  - Verification: `cargo check` passes.

- [x] **A.6** Update the per-file parse-cache schema in `tools/xs-language-server/src/cache.rs`: move from `game_parse/v2/` to `game_parse/v3/`.
  - Update directory-layout comment and `parse_cache_dir` doc string.
  - Existing v2 caches are invalidated and rebuilt on first run.
  - Verification: `cargo check` passes.

- [x] **A.7** Update other exhaustive `SymbolKind` consumers.
  - `tools/xs-language-server/src/completion.rs`: map `ClassField` → `FIELD`, `ClassMethod` → `METHOD`.
  - `tools/xs-language-server/src/semantic_tokens.rs`: map `ClassField` → `VARIABLE`, `ClassMethod` → `FUNCTION`; append `member` modifier.
  - `tools/xs-language-server/src/server.rs`: map new kinds in document-symbol / workspace-symbol responses (`ClassField` → `FIELD`, `ClassMethod` → `METHOD`, `container_name` = class owner).
  - Verification: `cargo check` passes.

### Phase B: LSP semantic-tokens changes (Rust)

- [x] **B.1** Extend `TOKEN_MODIFIERS` in `tools/xs-language-server/src/semantic_tokens.rs` with `SemanticTokenModifier::new("member")`.
  - Verification: `cargo check` passes.

- [x] **B.2** Add `classify_member_identifier` in `tools/xs-language-server/src/semantic_tokens.rs`.
  - Detect `field_expression` / `field_identifier` AST nodes.
  - `ClassName.method()`/call-position → `function` + `[member, <origin>]`.
  - `obj.field` → `variable` + `[member, <origin>]`.
  - No member match → no token emitted (array builtins stay uncolored).
  - Verification: `cargo check` passes.

- [x] **B.3** Add `classify_member_origin(member_name, workspace, project) -> Origin` in `tools/xs-language-server/src/semantic_tokens.rs`.
  - Delegate to the `MemberIndex` from A.5.
  - Verification: `cargo check` passes.

- [x] **B.4** Update `classify_identifier` and `walk_for_tokens` in `tools/xs-language-server/src/semantic_tokens.rs`.
  - Route `field_identifier` to `classify_member_identifier`.
  - Append `member` modifier when symbol kind is `ClassField`/`ClassMethod`.
  - Update `compute_tokens` signature to accept `member_index: &MemberIndex`.
  - Verification: `cargo build` passes.

### Phase C: LSP strict-TDD tests

- [x] **C.1** Create `tools/xs-language-server/tests/class_member_extraction_repro.rs` with 7 strict-TDD tests:
  1. Vanilla class with field emits `ClassField` symbol.
  2. Vanilla class with method emits `ClassMethod` symbol.
  3. Cross-file `obj.field` reference resolves to the field's origin.
  4. Cross-file `Class.method()` reference resolves to the method's origin.
  5. Ambiguous member name (vanilla + modded) → modded wins.
  6. 50+ classes with members build in < 200 ms.
  7. Malformed class body does not panic.
  - Verification: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test class_member_extraction_repro --no-fail-fast` passes.

- [x] **C.2** Create `tools/xs-language-server/tests/class_member_semantic_tokens_repro.rs` with 6 strict-TDD tests:
  1. `obj.field` in vanilla class emits `variable` + `[member, unmodded]`.
  2. `Class.method()` in vanilla class emits `function` + `[member, unmodded]`.
  3. `obj.field` in modded class → `[member, modded]`.
  4. Ambiguous member name → `[member, modded]`.
  5. Member of engine class → `[member, engine]`.
  6. Member declaration inside class emits `member` modifier and correct token type.
  - Verification: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test class_member_semantic_tokens_repro --no-fail-fast` passes.

- [x] **C.3** Run the full LSP test suite.
  - Verification: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast` passes with **252 tests** (239 baseline + 13 new).

### Phase D: Plugin text attributes + descriptors (Kotlin)

- [x] **D.1** Add six new `TextAttributesKey` entries to `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt`:
  - `METHOD_ENGINE` → fallback `DefaultLanguageHighlighterColors.STATIC_METHOD` (or `FUNCTION_DECLARATION` if unavailable).
  - `METHOD_UNMODDED`
  - `METHOD_MODDED`
  - `FIELD_ENGINE` → fallback `DefaultLanguageHighlighterColors.INSTANCE_FIELD` (or `LOCAL_VARIABLE` if unavailable).
  - `FIELD_UNMODDED`
  - `FIELD_MODDED`
  - Verification: `./gradlew :compileKotlin` passes.

- [x] **D.2** Add six new `AttributesDescriptor` entries to `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt`:
  - `Identifier//Function//Method Engine` → `XsTextAttributes.METHOD_ENGINE`
  - `Identifier//Function//Method UnModded` → `XsTextAttributes.METHOD_UNMODDED`
  - `Identifier//Function//Method Modded` → `XsTextAttributes.METHOD_MODDED`
  - `Identifier//Variable//Field Engine` → `XsTextAttributes.FIELD_ENGINE`
  - `Identifier//Variable//Field UnModded` → `XsTextAttributes.FIELD_UNMODDED`
  - `Identifier//Variable//Field Modded` → `XsTextAttributes.FIELD_MODDED`
  - Verification: `./gradlew :compileKotlin` passes.

### Phase E: Plugin semantic-tokens wiring (Kotlin)

- [x] **E.1** Update `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSemanticTokensConverter.kt`.
  - Add `MODIFIER_MEMBER = "member"`.
  - Add 6 new branches: `function` + `member` + origin → `METHOD_*`, and `variable` + `member` + origin → `FIELD_*`.
  - Keep existing non-member branches unchanged.
  - Verification: `./gradlew :compileKotlin` passes.

- [x] **E.2** Verify `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt` needs no changes.
  - Per design Q2: `member` is a modifier on existing `function`/`variable` types; no new token type needed.
  - Justification note in commit or code comment if helpful.

### Phase F: Plugin tests

- [x] **F.1** Update `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt` with 6 new tests:
  - `function` + `member` + `engine` → `METHOD_ENGINE`
  - `function` + `member` + `unmodded` → `METHOD_UNMODDED`
  - `function` + `member` + `modded` → `METHOD_MODDED`
  - `variable` + `member` + `engine` → `FIELD_ENGINE`
  - `variable` + `member` + `unmodded` → `FIELD_UNMODDED`
  - `variable` + `member` + `modded` → `FIELD_MODDED`
  - Verification: `./gradlew :test --tests "com.aomr.xs.lsp.XsSemanticTokensConverterTest"` passes.

- [x] **F.2** Run the full plugin test suite.
  - Verification: `cd tools/intellij-xs-plugin && ./gradlew :test --no-daemon` passes with **102 tests** (96 baseline + 6 new).

### Phase G: Build artifacts + version bumps

- [x] **G.1** Update `tools/intellij-xs-plugin/gradle.properties`:
  - `pluginVersion = 0.8.0`
  - `platformVersion` and `pluginSinceBuild` remain unchanged (`2024.2.2` / `242.22855.74`).

- [x] **G.2** Build the plugin artifact.
  - Verification: `cd tools/intellij-xs-plugin && ./gradlew buildPlugin --no-daemon` produces `build/distributions/intellij-xs-plugin-0.8.0.zip` and the file is larger than the existing 0.7.0 zip (4,137,228 bytes).

- [x] **G.3** Build the LSP server release binary.
  - Verification: `cargo build --manifest-path tools/xs-language-server/Cargo.toml --release` passes cleanly.

### Phase H: Documentation + verification

- [x] **H.1** Update `AGENTS.md` test counts.
  - LSP: 239 → **252**.
  - Plugin: 96 → **102**.

- [x] **H.2** Verify all test suites pass.
  - LSP: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast` passes with 252 tests.
  - Plugin: `cd tools/intellij-xs-plugin && ./gradlew :test --no-daemon` passes with 102 tests.

- [x] **H.3** Update `docs/issues/2026-06-29-runtime-issues.md` if it lists `expand-color-scheme-classes-constants-rules` as an open follow-up.
  - Mark it RESOLVED by this change.

## 3. Verification checklist (before commit)

- [x] `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast` passes (252 tests).
- [x] `cd tools/intellij-xs-plugin && ./gradlew :test --no-daemon` passes (102 tests).
- [x] `cd tools/intellij-xs-plugin && ./gradlew buildPlugin --no-daemon` produces `build/distributions/intellij-xs-plugin-0.8.0.zip`.
- [x] `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml -- -D warnings` passes, or any warnings are documented as pre-existing.
- [x] Working tree noise (rustfmt churn, untracked research files) is NOT included in the commit.
- [x] `git diff --stat` shows only the expected files modified (~25–30 files, ~600–750 LOC).
- [x] `AGENTS.md` test counts are updated.
- [x] Commit message prepared: `feat(xs-lsp, xs-plugin): extract class members (fields + methods) for outline + member coloring`.

## 4. Review workload forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~600–750 |
| New files | 2 LSP test files + possible `member_index.rs` |
| Files changed | ~25–30 across LSP, plugin, properties, and docs |
| 400-line budget risk | N/A |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Decision needed before apply | No |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: N/A (no budget cap)
