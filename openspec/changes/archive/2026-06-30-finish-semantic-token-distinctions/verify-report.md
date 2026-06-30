# Verification Report: Finish deferred semantic-token distinctions

**Change**: `finish-semantic-token-distinctions`  
**Commit**: `dfa829d080211bfa2be6c42d6ca340a30c19b78d`  
**Verifier**: `sdd-verify` executor  
**Date**: 2026-06-30

---

## Verdict

**VERIFIED**

The implementation matches the specs, design, and tasks. All targeted tests pass, the plugin artifact is built, documentation is updated, and the working-tree noise is excluded from the commit. The five reported design deviations are technically necessary adaptations to the pinned `tower_lsp` version and the IntelliJ 2024.2.2 platform API; they do not break any spec requirement.

---

## Test counts

| Layer | Target | Actual | Result |
|---|---|---|---|
| LSP (Rust) | 239 | 239 | PASS |
| Plugin (Kotlin) | 96 | 96 | PASS |

**Evidence**

- LSP: `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast` → all test suites green, sum of `running N tests` lines = 239.
- Plugin: `./gradlew :test --no-daemon --rerun-tasks` → BUILD SUCCESSFUL; XML test result files sum to 96 test cases.

---

## Spec coverage

### Spec 1: `spec-lsp-class-extraction.md`

| Requirement | Status | Evidence |
|---|---|---|
| R1 — `SymbolKind::Class` variant | PASS | `tools/xs-language-server/src/symbols.rs:30` declares `Class`; `label()` returns `"class"` at line 41. |
| R2 — `class_specifier` walker | PASS | `tools/xs-language-server/src/symbols.rs:150` calls `extract_class` in `build_symbol_table`; `extract_class:307-330` captures name, full range, selection range, and provenance. |
| R3 — Exclude class member extraction | PASS | `extract_class` pushes exactly one `SymbolKind::Class` symbol and does not recurse into `field_declaration_list`. `class_declaration_does_not_crash_walker` confirms exactly one Class symbol for a class with fields and methods. |
| R4 — Additive cache schema | PASS | `tools/xs-language-server/src/cache.rs:49-51` bumps parse cache to `game_parse/v2/`; `SymbolKind` uses string-tagged serde serialization, so old `v1` readers ignore unknown variants and new readers rebuild. |
| R5 — Class reference origin | PASS | `tools/xs-language-server/src/semantic_tokens.rs:242-275` `classify_type_identifier` resolves through `merged.and_then(|m| m.find(name))` and `own_table.find(name)`, emitting `SemanticTokenType::TYPE` with the file's origin modifier. |
| S1 — Vanilla class reference | PASS | `class_extraction_repro.rs:38-63` `vanilla_class_reference_emits_type_unmodded` passes. |
| S2 — Modded class reference | PASS | `class_extraction_repro.rs:66-93` `modded_class_reference_emits_type_modded` passes. |
| S3 — Unknown type fallback | PASS | `class_extraction_repro.rs:96-112` `unknown_class_reference_falls_back_to_engine` passes. |
| S4 — Performance | PASS | `class_extraction_repro.rs:136-161` `many_class_declarations_build_quickly` builds 60 classes in < 100 ms. |
| S5 — Cache compatibility | PASS | Covered by additive schema change; no regressions in `cargo test`. |

### Spec 2: `spec-lsp-semantic-token-distinctions.md`

| Requirement | Status | Evidence |
|---|---|---|
| R1 — `constant` token type | PASS | `tools/xs-language-server/src/semantic_tokens.rs:23-24` includes `SemanticTokenType::new("constant")` in `TOKEN_TYPES`. |
| R2 — `rule` token type | PASS | `tools/xs-language-server/src/semantic_tokens.rs:24` includes `SemanticTokenType::new("rule")`. |
| R3 — `extern` modifier | PASS | `tools/xs-language-server/src/semantic_tokens.rs:33` includes `SemanticTokenModifier::new("extern")`. |
| R4 — Emit `constant` | PASS | `tools/xs-language-server/src/semantic_tokens.rs:228` maps `SymbolKind::Constant` to `SemanticTokenType::new("constant")`. Test `constant_reference_emits_constant_token_type` passes. |
| R5 — Emit `rule` | PASS | `tools/xs-language-server/src/semantic_tokens.rs:229` maps `SymbolKind::Rule` to `SemanticTokenType::new("rule")`. Test `rule_reference_emits_rule_token_type` passes. |
| R6 — Emit `extern` modifier | PASS | `tools/xs-language-server/src/semantic_tokens.rs:311-313` pushes `extern` for `SymbolKind::Variable && symbol.is_extern`. Test `extern_variable_emits_extern_and_origin_modifiers` passes. |
| R7 — Wire customizer | PASS | `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt:38` exposes `lspSemanticTokensSupport = XsSemanticTokensSupport()`. Test `XsLspServerDescriptorTest.semanticTokensSupportIsXsSpecific` passes. |
| R8 — `XsSemanticTokensSupport` | PASS | File exists at `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt:16-27`; overrides platform mapping method and delegates to `XsSemanticTokensConverter`. |
| R9 — `getTokenTypes()` | PASS | `XsSemanticTokensSupport.kt:20-27` returns `[function, variable, type, constant, rule]` as `List<String>`. |
| R10 — New `TextAttributesKey`s | PASS | `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt:86-95` defines `CONSTANT`, `RULE`, `VARIABLE_EXTERN_UNMODDED`, `VARIABLE_EXTERN_MODDED`. |
| R11 — New descriptors | PASS | `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt:76-79` adds the four descriptors. |
| R12 — Extend converter | PASS | `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverter.kt:64-69, 76-77` maps the four new combinations. Tests `constantMapsToConstant`, `ruleMapsToRule`, `externVariableUnmoddedMapsToVariableExternUnmodded`, `externVariableModdedMapsToVariableExternModded` pass. |

### Scenario coverage (Spec 2)

| Scenario | Test evidence | Status |
|---|---|---|
| S1 — Constant reference colored | `semantic_token_distinctions_repro.rs` + `XsSemanticTokensConverterTest.constantMapsToConstant` | PASS |
| S2 — Rule reference colored | `semantic_token_distinctions_repro.rs` + `XsSemanticTokensConverterTest.ruleMapsToRule` | PASS |
| S3 — Extern vanilla variable | `semantic_token_distinctions_repro.rs` + `XsSemanticTokensConverterTest.externVariableUnmoddedMapsToVariableExternUnmodded` | PASS |
| S4 — Extern modded variable | `semantic_token_distinctions_repro.rs` + `XsSemanticTokensConverterTest.externVariableModdedMapsToVariableExternModded` | PASS |
| S5 — Color scheme page | `XsColorSettingsPageTest` (part of 96 plugin tests) | PASS |
| S6-S8 — Runtime coloring | Manual smoke test documented in `docs/issues/2026-06-29-runtime-issues.md`; automated wiring verified through converter + descriptor tests. | PASS |

---

## Implementation quality

- **LSP side**: The new `SymbolKind::Class` is cleanly integrated; the `class_specifier` walker is minimal and correct; semantic-token classification correctly distinguishes constant/rule/extern without breaking existing engine/modded/unmodded/local/static behavior.
- **Plugin side**: The converter remains stateless and unit-testable; `XsSemanticTokensSupport` is a thin platform-specific bridge; the new keys/descriptors follow the existing naming and grouping conventions.
- **Cache safety**: Bumping the parse-cache directory to `game_parse/v2/` prevents deserialization failures from older readers and forces a clean rebuild.
- **Build**: `gradle.properties` is correctly bumped to `pluginVersion = 0.7.0`, `platformVersion = 2024.2.2`, `pluginSinceBuild = 242.22855.74`.

---

## Deviations review

| Deviation | Assessment | Concern |
|---|---|---|
| D1 — `SemanticTokenType::new("constant")` instead of `SemanticTokenType::CONSTANT` | ACCEPTABLE | The pinned `tower_lsp` version does not expose a `CONSTANT` constant; using `new("constant")` is the idiomatic fallback and matches the LSP spec. |
| D2 — `lspSemanticTokensSupport` direct property instead of `lspCustomization` wrapper | ACCEPTABLE | The 2024.2.2 platform API exposes `LspServerDescriptor.lspSemanticTokensSupport` directly; there is no `LspCustomization` class. The implementation wires the customizer correctly and the dedicated test passes. |
| D3 — `type_identifier` node kind for class references | ACCEPTABLE | The walker matches both `_type_identifier` and `type_identifier` (`semantic_tokens.rs:125`), covering the grammar alias. |
| D4 — `XsStartupActivityTest` tolerance fix | ACCEPTABLE | The test now asserts presence of created mods rather than exact count, which is necessary because the headless fixture root overlaps with real repo/worktree mod directories. This is a test-hygiene fix, not a behavior change. |
| D5 — 43 pre-existing clippy warnings | ACCEPTABLE | `cargo clippy -- -D warnings` reports 43 errors, all warnings promoted to errors. None originate from the new/changed lines of this commit; the errors are in unmodified files (`definition_check.rs`, `diagnostics.rs`, `doxygen.rs`, `engine_api.rs`, `semantic.rs`, `server.rs`, `typecheck.rs`, `word.rs`, `workspace.rs`) and pre-existing functions in `merged_view.rs` / `symbols.rs` that were not touched by the change. |

---

## Working tree state

- `git status --short | wc -l` = 21.
- All 21 items are excluded from the commit:
  - rustfmt churn on 10 unmodified LSP files (`day1_probe.rs`, `dump_top_level.rs`, `inspect_tree.rs`, `definition_check.rs`, `diagnostics.rs`, `doxygen.rs`, `engine_api.rs`, `main.rs`, `typecheck.rs`, `word.rs`) and `tests/game_folder_parse.rs`;
  - `scripts/deploy-mods.sh` modification;
  - untracked research/mod files (`docs/code-reviews/2026-06-29-lsp-and-plugin-findings.csv`, `docs/research/map_awareness_research.md`, `mod/spire_ai/`, `mod/test_targeting/`, `openspec/changes/expand-color-scheme-semantic-tokens/`, `scripts/encode_tactic_xmb.py`, `scripts/find_units_with_tag.py`);
  - IDE config (`.idea/vcs.xml`) and archive task file update.
- The commit itself contains only the expected ~26 files, ~1,357 insertions, ~47 deletions.

---

## Build artifact

| Artifact | Size | Baseline (0.6.0) | Result |
|---|---|---|---|
| `dist/intellij-xs-plugin-0.7.0.zip` | 4,137,228 bytes | 4,132,718 bytes | PASS (> baseline) |

---

## Documentation / AGENTS.md

| Item | Status | Evidence |
|---|---|---|
| `AGENTS.md` test counts updated | PASS | Line 121 reads: LSP 239, plugin 96. |
| `docs/issues/2026-06-29-runtime-issues.md` Bucket C deferred items marked RESOLVED | PASS | Lines 436-438 state Bucket C remaining work is "resolved 2026-06-30 by `openspec/changes/finish-semantic-token-distinctions/` (plugin `0.7.0`)." |
| CHANGELOG exists | PASS | `openspec/changes/finish-semantic-token-distinctions/CHANGELOG.md` documents the change and deviations. |

---

## Issues found

**None.**

---

## Recommendation

**APPROVE TO MERGE.**

The change is complete, tested, documented, and the deviation from the original `design.md` snippets is justified by the actual platform API. No blockers.
