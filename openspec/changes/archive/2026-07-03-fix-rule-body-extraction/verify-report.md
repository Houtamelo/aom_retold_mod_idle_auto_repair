# Verification Report: 2026-07-03-fix-rule-body-extraction

**Change**: 2026-07-03-fix-rule-body-extraction  
**Version**: N/A  
**Mode**: Standard  
**Verifier**: sdd-verify executor  
**Date**: 2026-07-03

---

## Executive Summary

**PASS**.

The rule-body extraction fix is verified in its scope. `lelwel-xs` builds cleanly and all 176 unit tests pass, covering all nine spec scenarios. The two documented design deviations are present in code and tests as described. Public API additions match the spec. Parent-workspace test failures are environmental (missing `tools/docs/doxygen_retail.7z` path and a missing `rustdoc` binary), not regressions introduced by this change.

---

## Completeness

| Metric | Value |
|--------|-------|
| Tasks total | 9 |
| Tasks complete | 9 |
| Tasks incomplete | 0 |

All tasks from `tasks.md` show completed status in `apply-progress.md`.

---

## Build & Tests Execution

### Build

**Result**: ✅ Passed (with generated-code warnings)

Command:
```bash
export PATH=/usr/local/cargo/bin:$PATH
cd /home/houtamelo/Documents/projects/aom_retold_mod/tools/xs-language-server/lelwel-xs
cargo build 2>&1 | tail -10
```

Output:
```text
warning: variable `node_kind` is assigned to, but never used
    --> /home/houtamelo/Documents/projects/aom_retold_mod/tools/xs-language-server/target/debug/build/xs-parser-833d042651f3451c/out/generated.rs:4759:13
     |
4759 |         let mut node_kind = Rule::PrimaryExpr;
     |             ^^^^^^^^^^^^^
     |
     = note: consider using `_node_kind` instead

warning: `xs-parser` (lib) generated 8 warnings (run `cargo fix --lib -p xs-parser` to apply 3 suggestions)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.02s
```

Notes:
- No compiler errors.
- Warnings originate in lelwel-generated `parser.rs` and are cosmetic.

### Unit tests in `lelwel-xs`

**Result**: ✅ 176 passed; 0 failed; 0 ignored

Command:
```bash
cargo test 2>&1 | tail -30
```

Output:
```text
test ast::type_table::tests::default_seeds_primitives ... ok
test ast::type_table::tests::empty_table_has_no_types ... ok
test ast::type_table::tests::is_type_false_for_unknown_identifier ... ok
test parser::tests::predicate_known_class_is_declaration_start ... ok
test parser::tests::predicate_primitive_int_is_declaration_start ... ok
test ast::type_table::tests::insert_class_adds_user_type ... ok
test parser::tests::predicate_qualifier_is_declaration_start ... ok
test parser::tests::predicate_unknown_identifier_is_not_declaration_start ... ok
test ast::type_table::tests::with_primitives_seeds_builtin_types ... ok
test parser::tests::predicate_empty_table_primitive_is_not_declaration_start ... ok

test result: ok. 176 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests xs_parser
error: doctest failed, to rerun pass `--doc`

Caused by:
  could not execute process `rustdoc --edition=2024 ...` (never executed)

Caused by:
  No such file or directory (os error 2)
```

Notes:
- Unit-test count matches apply-progress baseline: **176 passed; 0 failed**.
- Baseline was 159 tests (from spec/tasks). Delta: **+17 tests** covering `TypeTable` (5), predicate (5), and rule-body extraction S-RBE-01..08 (8).
- Doc-test runner fails because the `rustdoc` executable is not installed in this sandbox. This is environmental and does not affect unit-test compliance.

### Parent workspace tests

**Result**: 152 passed; 35 failed (environmental)

Command:
```bash
cargo test --manifest-path tools/xs-language-server/Cargo.toml 2>&1 | tail -80
```

Failure pattern:
```text
load engine data from Doxygen archive: hashing archive "/home/houtamelo/Documents/projects/aom_retold_mod/tools/docs/doxygen_retail.7z"

Caused by:
    0: reading file for SHA-256: "/home/houtamelo/Documents/projects/aom_retold_mod/tools/docs/doxygen_retail.7z"
    1: No such file or directory (os error 2)
```

Notes:
- The actual archive is at `/home/houtamelo/Documents/projects/aom_retold_mod/docs/doxygen_retail.7z`.
- Test code uses a cwd-relative `tools/docs/doxygen_retail.7z` path, so it only passes when the working directory is the repository root and an extra `tools/` segment exists. This is a pre-existing path issue, not introduced by this change.
- The 35 failing tests are confined to `doxygen`, `engine_api`, `semantic`, and `typecheck` modules — none of which touch `block_item^` or `TypeTable`.

---

## Spec Compliance Matrix

| Requirement | Scenario | Test (module path) | Result |
|-------------|----------|--------------------|--------|
| R-RBE-01 | S-RBE-01 — empty body | `ast::top_level::tests::rule_definition_extracts_empty_body_s_rbe_01` | ✅ COMPLIANT |
| R-RBE-02 | S-RBE-02 — single statement `x;` | `ast::top_level::tests::rule_definition_extracts_statement_body_s_rbe_02` | ✅ COMPLIANT |
| R-RBE-03 | S-RBE-03 — single declaration `int x = 1;` | `ast::top_level::tests::rule_definition_extracts_declaration_body_s_rbe_03` | ✅ COMPLIANT |
| R-RBE-04 | S-RBE-04 — class-typed declaration with `insert_class` | `ast::top_level::tests::rule_definition_extracts_class_typed_declaration_body_s_rbe_04` | ✅ COMPLIANT |
| R-RBE-05 | S-RBE-05 — mixed body | `ast::top_level::tests::rule_definition_extracts_mixed_body_s_rbe_05` | ✅ COMPLIANT |
| R-RBE-06 | S-RBE-06 — retail snippet, no rule-level ERROR | `ast::top_level::tests::rule_definition_extracts_retail_snippet_s_rbe_06` | ✅ COMPLIANT |
| R-RBE-07 | S-RBE-07 — empty `TypeTable` rejects `int x;` | `ast::top_level::tests::rule_definition_empty_table_int_is_not_declaration_s_rbe_07` | ✅ COMPLIANT (adjusted) |
| R-RBE-08 | S-RBE-08 — unknown class falls through | `ast::top_level::tests::rule_definition_unknown_class_is_not_declaration_s_rbe_08` | ✅ COMPLIANT (adjusted) |
| R-RBE-09 | S-RBE-09 — existing 159 tests remain green | `cargo test` in `lelwel-xs`: 176 passed; 0 failed | ✅ COMPLIANT |

**Compliance summary**: 9/9 scenarios compliant.

---

## Correctness (Static Evidence)

| Requirement | Status | Notes |
|-------------|--------|-------|
| `TypeTable` created with primitive seeding | ✅ Implemented | `src/ast/type_table.rs` seeds `void`, `int`, `bool`, `float`, `string`, `vector` via `with_primitives()` |
| `TypeTable` wired into parser context | ✅ Implemented | `parser.rs` sets `type Context = TypeTable` |
| `predicate_block_item_1` implemented | ✅ Implemented | `parser.rs` maps primitive/qualifier tokens and looks up identifiers in `context.is_type()` |
| `block_item^` grammar updated | ✅ Implemented | `xs.llw` uses `?1 block_declaration_or_definition | statement` plus helper rule for func-def vs declaration split |
| `TypeTable` re-exported from `ast` | ✅ Implemented | `ast/mod.rs` has `pub use crate::ast::type_table::TypeTable;` |
| `parse_with_types` helper added | ✅ Implemented | Test helper in `ast/top_level.rs` uses `Parser::new_with_context` |
| Rule-body unit tests added | ✅ Implemented | S-RBE-01..08 covered in `ast/top_level.rs` |
| Existing tests unchanged / green | ✅ Implemented | 176 passed; 0 failed |

---

## Coherence (Design)

| Decision | Followed? | Notes |
|----------|-----------|-------|
| Introduce `TypeTable` parser context | ✅ Yes | `ParserCallbacks::Context = TypeTable` |
| Use semantic predicate `?1` on declaration branches | ✅ Yes | Grammar uses `?1`; callback implemented as `predicate_block_item_1` |
| Keep `Parser::new` backward compatible | ✅ Yes | `Parser::new` uses `TypeTable::default()` which delegates to `with_primitives()` |
| Re-export `TypeTable` from `xs_parser::ast` | ✅ Yes | `ast/mod.rs` re-export present |
| Limit scope to `block_item^`; leave `top_level_item^` unchanged | ✅ Yes | `xs.llw` `top_level_item^` still uses original `/` and `?t` semantics |
| Deviation 1: helper rule instead of direct `/` in `block_item^` | ✅ Yes | `block_declaration_or_definition` helper rule present due to lelwel 0.10.4 nested-ordered-choice/predicate restrictions |
| Deviation 2: S-RBE-07/08 assert predicate refusal | ✅ Yes | Tests verify no `Rule::Declaration` descendant rather than clean statement extraction |

---

## Issues Found

### CRITICAL
- None.

### WARNING
- **Environmental doctest runner failure**: `rustdoc` binary is not available in the sandbox, causing `cargo test` to report `Doc-tests xs_parser` as failed even though all 176 unit tests pass. This does not block the change but should be noted for CI setup.
- **Parent workspace test failures (environmental)**: 35 tests in `doxygen`, `engine_api`, `semantic`, and `typecheck` fail because they resolve `tools/docs/doxygen_retail.7z` relative to cwd. The real archive lives at `docs/doxygen_retail.7z`. This path mismatch is pre-existing and unrelated to rule-body extraction.

### SUGGESTION
- Investigate making parent-workspace tests robust to cwd by resolving `doxygen_retail.7z` relative to `CARGO_MANIFEST_DIR` or an env var (e.g., `AOMR_GAME_PATH`).
- Optionally silence generated-code warnings in `generated.rs` via `#[allow(unused_assignments)]` on the generated module or fix the lelwel template.

---

## Public API Verification

| Surface | Public? | Location | Notes |
|---------|---------|----------|-------|
| `TypeTable` | ✅ Yes | `src/ast/type_table.rs` | `pub struct TypeTable` |
| `TypeTable::with_primitives` | ✅ Yes | `src/ast/type_table.rs` | `pub fn with_primitives() -> Self` |
| `TypeTable::insert_class` | ✅ Yes | `src/ast/type_table.rs` | `pub fn insert_class(&mut self, name: &str)` |
| `TypeTable::is_type` | ✅ Yes | `src/ast/type_table.rs` | `pub fn is_type(&self, name: &str) -> bool` |
| `impl Default for TypeTable` | ✅ Yes | `src/ast/type_table.rs` | Delegates to `with_primitives()` |
| Re-export from `ast` | ✅ Yes | `src/ast/mod.rs` | `pub use crate::ast::type_table::TypeTable;` |

`Parser::new` still works without API changes because it constructs a default `TypeTable` internally.

---

## Risks / Blockers

| Risk | Level | Status |
|------|-------|--------|
| Missing class names parse as statements | Medium | Accepted / by design until Phase 4 populates `TypeTable.classes` |
| Qualifier keywords hard-coded as declaration starts | Low | Correct for XS; no counter-examples found |
| `for (int i...)` unsupported in rule bodies | Low | Accepted; sample stops before loop per design |
| Environmental test failures in parent workspace | Low | 35 failures are cwd/path related, not caused by this change |
| Missing `rustdoc` prevents doc-test execution | Low | Environmental; unit tests pass |

No blockers for archive.

---

## Recommendation for Archive

**OK to archive.**

The implementation matches the spec and design intent within the documented deviations. All targeted tests pass, the public API is complete and re-exported correctly, and identified failures are environmental. The change is isolated to `lelwel-xs` and safe to proceed to the archive phase.

---

## Artifacts Produced

- `openspec/changes/2026-07-03-fix-rule-body-extraction/verify-report.md` (this file)
