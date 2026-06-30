# Verification Report

**Change**: `expand-color-scheme-classes-constants-rules` (Bucket C remainder — class member extraction + member semantic coloring)
**Commit**: `9cc73449b2c6015b98dbe72c3fda007fea934313`
**Branch**: `xs-lsp-roundtrip-followup`
**Version**: `pluginVersion 0.7.0 → 0.8.0`
**Mode**: Strict TDD (assumed active from `_repro.rs` strict-TDD files and spec language)

---

## Completeness

| Metric | Value |
|--------|-------|
| Tasks total | 8 phases (A.1–A.7, B.1–B.4, C.1–C.3, D.1–D.2, E.1–E.2, F.1–F.2, G.1–G.3, H.1–H.3) |
| Tasks complete | 28 / 28 checkboxes marked complete in `tasks.md` |
| Tasks incomplete | 0 |

---

## Build & Tests Execution

**Build**: ✅ Passed
```text
cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast
```

**Tests**: ✅ 252 LSP passed, 0 failed; ✅ 102 plugin tests executed, 0 failed
```text
# LSP test total
cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast 2>&1 \
  | grep -E "^test result" | awk '{sum += $4} END {print "Total passed: " sum}'
Total passed: 252

# LSP class member extraction tests
cargo test --manifest-path tools/xs-language-server/Cargo.toml \
  --test class_member_extraction_repro -- --nocapture 2>&1 | tail -10
running 7 tests
test class_field_is_extracted ... ok
...
test result: ok. 7 passed; 0 failed; 0 ignored

# LSP class member semantic tokens tests
cargo test --manifest-path tools/xs-language-server/Cargo.toml \
  --test class_member_semantic_tokens_repro -- --nocapture 2>&1 | tail -10
running 6 tests
...
test result: ok. 6 passed; 0 failed; 0 ignored

# Plugin tests
cd tools/intellij-xs-plugin && ./gradlew :test --rerun-tasks --no-daemon --offline 2>&1 | tail -3
> Task :test
BUILD SUCCESSFUL in 40s
20 actionable tasks: 20 executed
```

**Plugin test count confirmation**:
- `@Test` annotations counted: 96
- `XsGotoDeclarationHandlerTest` naming-convention tests (`fun test...`): 6
- Total expected plugin tests: **102** ✅

**Coverage**: ➖ Not available (no coverage tool detected for Rust/Kotlin in this repo).

---

## Spec Compliance Matrix

### `spec-lsp-class-member-extraction.md`

| Requirement / Scenario | Test | Result |
|------------------------|------|--------|
| R1 — `SymbolKind::ClassField`/`ClassMethod` variants | `symbols.rs` enum + `label()`; `class_member_extraction_repro.rs` | ✅ COMPLIANT |
| R2 — `Symbol::class_owner` field | `symbols.rs:97`; extraction tests assert `Some("Foo")` | ✅ COMPLIANT |
| R3 — Members stay `Visibility::Local` | `symbols.rs:405`; `merged_view.rs:628` excludes members | ✅ COMPLIANT |
| R4 — Field extraction from `field_declaration` | `symbols.rs:343-410` | ✅ COMPLIANT |
| R5 — Method extraction from class body | `symbols.rs:365-372`; `class_method_is_extracted` | ✅ COMPLIANT |
| R6 — `SymbolKind::Class` extraction preserved | `class_extraction_repro.rs::class_declaration_does_not_crash_walker` | ✅ COMPLIANT |
| R7 — Cache bump `game_parse/v2/` → `v3/` | `cache.rs:49-51` | ✅ COMPLIANT |
| R8 — Class members in **workspace/symbol** | `server.rs:1387-1398` sets `container_name` to `class_owner` | ✅ COMPLIANT |
| R8 — Class members in **DocumentSymbol** outline | Implementation exists but logic is reversed; members never attach to class | ❌ FAIL |
| S1 — Vanilla field + method extraction | `class_field_is_extracted`, `class_method_is_extracted` | ✅ COMPLIANT |
| S2 — Cross-file `obj.field` / `Class.method()` resolution | `cross_file_field_origin_is_unmodded_when_vanilla`, `cross_file_method_origin_is_unmodded_when_vanilla` | ✅ COMPLIANT |
| S3 — Ambiguous member name prefers modded | `ambiguous_member_prefers_modded_origin` | ✅ COMPLIANT |
| S4 — Cache compatibility on upgrade | No automated warm-v2-cache integration test found | ⚠️ UNTESTED |
| S5 — Large file performance (< 200 ms) | `many_classes_with_members_build_quickly` (60 classes) | ✅ COMPLIANT |
| S6 — Mixed class body | Implicitly covered by extraction tests; no dedicated case | ⚠️ PARTIAL |
| S7 — Malformed class body resilience | `malformed_class_body_does_not_panic` | ✅ COMPLIANT |

### `spec-lsp-class-member-semantic-tokens.md`

| Requirement / Scenario | Test | Result |
|------------------------|------|--------|
| R1 — `member` modifier in legend | `semantic_tokens.rs:36` | ✅ COMPLIANT |
| R2 — `field_expression` / `field_identifier` detection | `semantic_tokens.rs:133-136`, `classify_member_identifier` | ✅ COMPLIANT |
| R3 — `Class.method()` → `function` + `[member, origin]` | `static_method_call_emits_function_member_unmodded` | ✅ COMPLIANT |
| R4 — `obj.field` → `variable` + `[member, origin]` | `instance_field_reference_emits_variable_member_unmodded` | ✅ COMPLIANT |
| R5 — Member origin heuristic `modded > unmodded > engine` | `ambiguous_member_name_prefers_modded_origin`, `engine_class_member_reference_emits_variable_member_engine` | ✅ COMPLIANT |
| R6 — 6 new `TextAttributesKey` entries | `XsTextAttributes.kt:100-115` | ✅ COMPLIANT |
| R7 — 6 new `AttributesDescriptor` entries | `XsColorSettingsPage.kt:81-86` | ✅ COMPLIANT |
| R8 — Converter branches for 6 member combos | `XsSemanticTokensConverterTest.kt` 6 new assertions | ✅ COMPLIANT |
| R9 — `getTokenTypes()` unchanged | `XsSemanticTokensSupport.kt:20-27` still lists 5 base types | ✅ COMPLIANT |
| S1 — Vanilla method reference | `static_method_call_emits_function_member_unmodded` | ✅ COMPLIANT |
| S2 — Modded method reference | (no dedicated method-in-mod-class test) | ⚠️ PARTIAL |
| S3 — Vanilla field reference | `instance_field_reference_emits_variable_member_unmodded` | ✅ COMPLIANT |
| S4 — Ambiguous member name prefers modded | `ambiguous_member_name_prefers_modded_origin` | ✅ COMPLIANT |
| S5 — Color scheme page shows 6 new categories | `XsColorSettingsPageTest` not extended to assert member descriptors | ⚠️ PARTIAL |
| S6/S7 — Real editor rendering | Manual smoke test only | ⚠️ PARTIAL |

### Modified baseline `spec-lsp-class-extraction.md`

| Requirement | Result | Evidence |
|-------------|--------|----------|
| R3 now defers member extraction to `spec-lsp-class-member-extraction.md` | ✅ COMPLIANT | `openspec/specs/spec-lsp-class-extraction.md:29-34` references the new spec |

---

## TDD Compliance

| Check | Result | Details |
|-------|--------|---------|
| TDD Evidence reported | ❌ | No `apply-progress` / TDD Cycle Evidence table found in `openspec/changes/expand-color-scheme-classes-constants-rules/` |
| All tasks have tests | ✅ | Extraction, semantic-token, and converter tests exist |
| RED confirmed (tests exist) | ✅ | `class_member_extraction_repro.rs` (7), `class_member_semantic_tokens_repro.rs` (6), `XsSemanticTokensConverterTest.kt` (+6) |
| GREEN confirmed (tests pass) | ✅ | All 13 new LSP + 6 new plugin assertions pass |
| Triangulation adequate | ⚠️ | S4 (cache compatibility) and S6/S7 (real editor / outline) have no automated coverage |
| Safety Net for modified files | ➖ | Not verifiable from commit metadata; full baseline suite passes (252 + 102) |

---

## Correctness (Static Evidence)

| Requirement | Status | Notes |
|-------------|--------|-------|
| `SymbolKind` extended with `ClassField`/`ClassMethod` | ✅ Implemented | `tools/xs-language-server/src/symbols.rs:24-47` |
| `Symbol::class_owner` added with serde default | ✅ Implemented | `tools/xs-language-server/src/symbols.rs:95-97`; construction sites default to `None` |
| Class-body walker emits fields/methods | ✅ Implemented | `tools/xs-language-server/src/symbols.rs:315-410` |
| `MemberIndex` origin heuristic | ✅ Implemented | `tools/xs-language-server/src/semantic_tokens.rs:339-414` |
| `member` modifier emitted on declarations and references | ✅ Implemented | `semantic_tokens.rs:316-320`, `classify_member_identifier` |
| Plugin converter maps `(function/variable, member, origin)` | ✅ Implemented | `XsSemanticTokensConverter.kt:60-77` |
| Plugin color descriptors registered | ✅ Implemented | `XsColorSettingsPage.kt:81-86` |
| `DocumentSymbol` nesting under class | ❌ Broken | `server.rs:1339-1370` drains pending members before they are seen because the class symbol is emitted before its members |

---

## Coherence (Design)

| Decision | Followed? | Notes |
|----------|-----------|-------|
| `MemberIndex` placed in `semantic_tokens.rs` | ✅ Yes | Per `platform-resolution.md`; avoids circular `symbols` → `workspace` dependency |
| `class_owner: None` for `Class` symbol itself | ✅ Yes | `symbols.rs:329` |
| `getTokenTypes()` stays unchanged | ✅ Yes | `XsSemanticTokensSupport.kt` unchanged; `member` is a modifier |
| Cache schema bump to `game_parse/v3/` | ✅ Yes | `cache.rs:14-15`, `49-51`; v2 caches are ignored and rebuilt |
| `Visibility::Local` for members | ✅ Yes | `symbols.rs:405`; excluded from merged view |
| Document outline nested under class | ❌ No | Implementation order is class-then-members, but `build_document_symbol_tree` expects members-then-class |

---

## Issues Found

### CRITICAL
1. **`DocumentSymbol` members are not nested under their class** (`tools/xs-language-server/src/server.rs:1339-1370`)
   - The class symbol is emitted **before** its members in `symbols.rs::extract_class`, but `build_document_symbol_tree` drains `pending_members` when it encounters a `Class` symbol. At that point `pending_members` is always empty, so members are never attached and are dropped from the outline response entirely.
   - This directly violates `spec-lsp-class-member-extraction.md` R8 / S7.
   - Fix: gather the members that follow each class (e.g., two-pass, or store the last class index and append members after the loop).

### WARNING
2. **New clippy warnings introduced by this change**
   - `semantic_tokens.rs:70` `compute_tokens` now has 8 arguments (limit 7).
   - `semantic_tokens.rs:105` `walk_for_tokens` now has 11 arguments (limit 7).
   - `server.rs:1362`, `1378`, `1392` use the deprecated `DocumentSymbol::deprecated` / `SymbolInformation::deprecated` fields in newly added code.
   - These are in addition to the ~45 pre-existing warnings and should be cleaned up before claiming “no new warnings.”
3. **`XsColorSettingsPageTest` not extended for the 6 member descriptors**
   - The spec verification approach calls for an automated assertion that the new Method/Field Engine/UnModded/Modded descriptors are present. The converter is unit-tested, but the color-page descriptor test remains only for the prior 12 inherited + 4 0.7.0 categories.
4. **S4 cache-compatibility scenario is not exercised by an automated test**
   - The schema bump is implemented correctly, but there is no integration test that starts the server with a warm `game_parse/v2/` cache.

### SUGGESTION
5. Consider adding a focused Rust unit test for `build_document_symbol_tree` (or an LSP round-trip assertion) so R8 cannot regress.
6. Consider `#[allow(deprecated)]` on the new `DocumentSymbol`/`SymbolInformation` helpers, or migrate to the modern `tags` API in a follow-up.

---

## Deviations Review

| Deviation | Assessment |
|-----------|------------|
| D1 — `DefaultLanguageHighlighterColors.STATIC_METHOD` / `INSTANCE_FIELD` available | ✅ No concern; used directly in `XsTextAttributes.kt:100-115` |
| D2 — `docs/issues/2026-06-29-runtime-issues.md` does not list this follow-up | ✅ Confirmed; the file tracks the earlier four issues and does not list the current change as open |
| D3 — 45+ pre-existing clippy warnings unchanged | ⚠️ Partially true, **but at least 5 new warnings appear in changed files** (too-many-arguments ×2, deprecated-field ×3) |
| D4 — Fix: cross-file tests passed `current_file: Some(file)`, causing `MemberIndex` to skip the workspace file; fixed by passing `None` | ✅ Reasonable; server path still passes `Some` for the actually-open file so buffer members are indexed via `own_table` |

---

## Working Tree State

```text
$ git status --short
 M .idea/vcs.xml
 M openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/tasks.md
 M scripts/deploy-mods.sh
 M tools/xs-language-server/src/bin/day1_probe.rs
 M tools/xs-language-server/src/bin/dump_top_level.rs
 M tools/xs-language-server/src/bin/inspect_tree.rs
 M tools/xs-language-server/src/definition_check.rs
 M tools/xs-language-server/src/diagnostics.rs
 M tools/xs-language-server/src/doxygen.rs
 M tools/xs-language-server/src/engine_api.rs
 M tools/xs-language-server/src/main.rs
 M tools/xs-language-server/src/typecheck.rs
 M tools/xs-language-server/src/word.rs
 M tools/xs-language-server/tests/game_folder_parse.rs
?? docs/code-reviews/2026-06-29-lsp-and-plugin-findings.csv
?? docs/research/map_awareness_research.md
?? mod/spire_ai/
?? mod/test_targeting/
?? openspec/changes/expand-color-scheme-semantic-tokens/
?? scripts/encode_tactic_xmb.py
?? scripts/find_units_with_tag.py
```

The commit itself (`git show --stat 9cc7344`) contains only the expected ~25 files. The working-tree noise above is **not** part of the reviewed commit.

---

## Build Artifact

```text
$ ls -la dist/intellij-xs-plugin-0.8.0.zip
-rw-rw-r--+ 1 claude root 4143325 Jun 30 18:40 dist/intellij-xs-plugin-0.8.0.zip
```

Size `4,143,325` bytes is greater than the 0.7.0 baseline (`4,137,228` bytes). ✅

---

## Verdict

**FAILED**

The implementation correctly extracts class fields/methods, propagates `class_owner`, bumps the cache to `v3`, advertises the `member` modifier, colors `field_expression` references using the `modded > unmodded > engine` heuristic, and wires the 6 new plugin member colors. All 252 LSP tests and 102 plugin tests pass.

However, `spec-lsp-class-member-extraction.md` **R8** is not fully satisfied: `textDocument/documentSymbol` does **not** nest class members under their parent class because `build_document_symbol_tree` drains `pending_members` before the members have been collected. This is a functional regression/blocker in the documented scope.

**Recommendation**: **HOLD** merge until `server.rs::build_document_symbol_tree` is fixed and a covering test is added. Address the 5 new clippy warnings and extend `XsColorSettingsPageTest` to assert the 6 new member descriptors.
