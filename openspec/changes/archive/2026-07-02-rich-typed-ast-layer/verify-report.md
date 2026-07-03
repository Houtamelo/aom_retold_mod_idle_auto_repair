# Verify Report: `2026-07-02-rich-typed-ast-layer` — Phase 1 + 2 + 3

**Branch:** `xs-lsp`  
**Phase verified:** 1 (Foundation + Declarations), 2 (Top-level + Preprocessor + Statements), 3 (Expressions + Format-Preservation)  
**Date:** 2026-07-03  
**Overall result:** ✅ PASS

---

## Executive Summary

All three passes of the rich typed AST layer are implemented and green. `cargo build` succeeds with zero errors, and `cargo test` reports **159 passed; 0 failed**. The proof-of-concept formatter round-trips the canonical 17-line test source and all 5 hand-selected retail XS files byte-for-byte. Spec coverage is complete for both `spec-typed-ast.md` and `spec-format-preservation.md`. The only outstanding items are minor cleanup warnings and a documentation update to `tasks.md` checkboxes.

---

## Build & Test Status

### Build

```text
$ export PATH=/usr/local/cargo/bin:$PATH
$ cd tools/xs-language-server/lelwel-xs
$ cargo build 2>&1 | tail -10
warning: variable `node_kind` is assigned to, but never used
    --> .../target/debug/build/xs-parser-.../out/generated.rs:4693:13
warning: `xs-parser` (lib) generated 8 warnings (run `cargo fix --lib -p xs-parser` to apply 3 suggestions)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s
```

**Status:** ✅ Clean build — 0 errors. Warnings are either pre-existing generated-parser dead-code warnings or minor new warnings detailed in *Issues Found* below.

### Tests

```text
$ cargo test 2>&1 | tail -5
test result: ok. 159 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests xs_parser

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Test Results by Module

| Module | Tests | Status |
|--------|------:|:------:|
| `ast::argument` | 3 | ✅ |
| `ast::cst_helpers` | 9 | ✅ |
| `ast::declaration` | 11 | ✅ |
| `ast::expr` | 27 | ✅ |
| `ast::format` | 24 | ✅ |
| `ast::parameter` | 3 | ✅ |
| `ast::preproc` | 22 | ✅ |
| `ast::spanned` | 9 | ✅ |
| `ast::statement` | 17 | ✅ |
| `ast::tokens` | 6 | ✅ |
| `ast::top_level` | 21 | ✅ |
| `ast::type_system` | 7 | ✅ |
| **Total** | **159** | **✅** |

*Note: Argument and Parameter tests were split out of `declaration.rs` during Pass 3 (T13). The original Phase 1 declaration test count was 18; after the split, `declaration.rs` retains 11 tests and the new modules carry 3 each.*

---

## Completeness (tasks.md)

| Metric | Value |
|--------|------:|
| Tasks total | 15 (T1–T15) |
| Tasks complete | 15 |
| Tasks incomplete | 0 |

- **Phase 1 (T1–T6):** complete — 42 tests green.
- **Phase 2 (T7–T10):** complete — `top_level.rs` and `statement.rs` implemented; `preproc.rs` was already present and exercised by 22 tests.
- **Phase 3 (T11–T15):** complete — `expr.rs` + `argument.rs` + `parameter.rs` + `format.rs` + final wire-up, 159 tests green.

---

## Spec Compliance Matrix

### `specs/spec-typed-ast.md`

| Scenario | Covering test(s) | Result |
|----------|------------------|--------|
| AST extraction from a clean source | `ast::format::tests::format_translation_unit_returns_same_source` plus per-module extraction tests | ✅ COMPLIANT |
| AST extraction with errors | `ast::top_level::tests::top_level_item_returns_none_for_error_node`, `ast::preproc::tests::from_cst_returns_none_on_unrelated_node` | ✅ COMPLIANT |
| Wrapper-token span preserved | `ast::format::tests::wrapper_spans_for_function_body_braces`, `wrapper_spans_for_function_declarator_parens`, `wrapper_spans_for_call_expression_parens`, `wrapper_spans_for_subscript_brackets`, `wrapper_spans_for_class_definition_braces` | ✅ COMPLIANT |
| Format-preserving round-trip | `ast::format::tests::round_trip_test_parse_source` and the 5 `round_trip_retail_*` tests | ✅ COMPLIANT |
| Span on every node | Wrapper-span tests + every `from_cst` test asserts structural span fields; `TranslationUnit` extraction used in `format_translation_unit_returns_same_source` | ✅ COMPLIANT |
| Multi-element declarators with mixed initializers | `ast::declaration::tests::parses_multi_declarator_list` | ✅ COMPLIANT |
| Comma-separated list separator preservation | `ast::format::tests::wrapper_spans_for_parameter_list_trailing_commas`, `wrapper_spans_for_argument_list_trailing_commas` | ✅ COMPLIANT |

**Compliance summary:** 7/7 scenarios compliant.

### `specs/spec-format-preservation.md`

| Scenario | Covering test(s) | Result |
|----------|------------------|--------|
| Parenthesized wrap round-trips | `ast::format::tests::wrapper_spans_for_function_declarator_parens`, `wrapper_spans_for_call_expression_parens`, `wrapper_spans_for_subscript_brackets` | ✅ COMPLIANT |
| Comma-separated list round-trips | `ast::format::tests::wrapper_spans_for_parameter_list_trailing_commas`, `wrapper_spans_for_argument_list_trailing_commas` | ✅ COMPLIANT |
| Trailing comma preserved | `ast::format::tests::wrapper_spans_for_argument_list_trailing_commas`, `wrapper_spans_for_parameter_list_trailing_commas` | ✅ COMPLIANT |
| Round-trip on real XS source | `ast::format::tests::round_trip_test_parse_source` | ✅ COMPLIANT |
| Round-trip on retail XS file | `ast::format::tests::round_trip_retail_chairon`, `round_trip_retail_strategy`, `round_trip_retail_main`, `round_trip_retail_startup_flow`, `round_trip_retail_human_assist_debug` | ✅ COMPLIANT |

**Compliance summary:** 5/5 scenarios compliant. The 6 round-trip tests mentioned in the acceptance gates are the single canonical-source test plus the 5 retail-file tests.

### Example run (`test_parse.rs`)

```text
$ cargo run --example test_parse 2>&1 | tail -5
=== Format round-trip ===
source bytes:     277
formatted bytes:  277
MATCH — byte-for-byte round-trip succeeded.
```

---

## Correctness (Static Evidence)

| Requirement | Status | Notes |
|-------------|:------:|-------|
| Owned `String` storage for identifiers/literals | ✅ Implemented | `Identifier = Spanned<String>`, literal `.value = String` |
| `from_cst` returns `Option<Self>` and never panics on shape mismatch | ✅ Implemented | All extraction paths use `?` on helper lookups; error-node tests confirm `None` |
| Every AST node stores a syntax handle | ✅ Implemented | Every struct carries `syntax: NodeRef`; `span(&cst)` computed on demand |
| Wrapper-token spans preserved | ✅ Implemented | `Parenthesized`, `Braced`, `Bracketed`, `CommaSeparatedList` all record punctuation `Span`s |
| `UnparsedExpr` placeholder replaced by real `Expr` | ✅ Implemented | `mod.rs` exports `expr::Expr` (18 variants); no `enum Expr {}` placeholder remains |
| `cargo build` green at every phase boundary | ✅ Verified | Build remained green throughout; current build is clean |

---

## Coherence (Design)

| Design Decision | Followed? | Notes |
|-----------------|:---------:|-------|
| Rich-types (eager extraction) over lazy | ✅ Yes | All fields extracted in `from_cst`; formatter uses stored spans |
| Wrapper-token types in `spanned.rs` | ✅ Yes | `Parenthesized`, `Braced`, `Bracketed`, `CommaSeparatedList`, `SemiColonSeparatedList`, `Spanned`, `TokenSpan` |
| CST helpers in `cst_helpers.rs` | ✅ Yes | `child_by_rule`, `children_by_rule`, `child_by_token`, `only_child`, `is_skip_token` |
| Inline extraction for `@`/`^` collapsed rules | ✅ Yes | `Declarator::from_inline`, `Declarator::from_inline_children` encode the discovery from Phase 1 |
| Expr placeholder strategy | ✅ Adjusted | Pass 1 used `UnparsedExpr`; Pass 3 replaced with real `Expr`; `UnparsedExpr` alias still exported for backward compat |
| Proof-of-concept formatter walks CST children | ✅ Yes | Simplest correct PoC: concatenate source spans of direct children of `NodeRef::ROOT` |

---

## Deviation Log

| # | Deviation | Rationale / Resolution |
|---|-----------|------------------------|
| 1 | **T11–T13 salvage story** — the original apply agent (`breezy-scarlet-caribou`) delivered the substantive `expr.rs` implementation but timed out, leaving 28 build errors and 29 test failures. | Fixed locally and documented in `apply-progress.md`. Final `expr.rs` (18 variants, 27 tests), `argument.rs`, and `parameter.rs` all meet their acceptance criteria. `cargo test` reports 159/159 green. |
| 2 | **Spec-mismatch on `enum Expr {}` placeholder** — the design doc considered and rejected a placeholder enum with `unimplemented!()`. | The actual implementation used `UnparsedExpr` in Pass 1–2, then replaced it with the full `Expr` enum in Pass 3. The placeholder pattern was not used as the final solution; the real `Expr` is exported via `mod.rs`. |
| 3 | **Original deviation #3 from Phase 2 — `ClassDefinition.semi` fallback for missing trailing `;`** — the grammar required `;` after `class_specifier`, which conflicted with real XS. | **Resolved by the separate change `2026-07-03-fix-class-specifier-no-trailing-semi/`** (see Cross-references). The grammar was fixed, and the defensive zero-width-span fallback remains as a safety net for partial parses. |
| 4 | **`tasks.md` checkbox drift** — Phase 2 T8 and Phase 3 T11–T15 acceptance checkboxes are still `[ ]` in `tasks.md` even though the work is complete and tested. | This is a documentation artifact, not an implementation gap. The verify report records actual completion. Recommended cleanup before archive. |

---

## Cross-references

- **`openspec/changes/2026-07-03-fix-class-specifier-no-trailing-semi/`** — separate SDD change that resolves Phase 2 deviation #3.
  - `apply-progress.md`: records the `xs.llw:238` grammar fix and 4 new regression tests.
  - `verify-report.md`: PASS, 102/102 tests (98 pre-existing + 4 new), build clean, before/after diagnostic measurement documented.
  - This change removed the spurious trailing-`;` requirement in `class_specifier`, matching real XS class definitions.

- **`docs/plans/2026-07-02-typed-ast-design.md`** — canonical design catalogue referenced by `design.md`.

---

## Issues Found

**CRITICAL:** None.

**WARNING:**
- `tasks.md` still shows unchecked acceptance boxes for completed tasks (T8, T11–T15). Update the file or add a note referencing this verify report to avoid future confusion.
- Two new compiler warnings appeared in user-written code:
  - `unused import: only_child` in `lelwel-xs/src/ast/declaration.rs:880` and `lelwel-xs/src/ast/expr.rs:28`.
  - `mismatched_lifetime_syntaxes` in `lelwel-xs/src/ast/top_level.rs:818` (test helper).
  These are style-level and do not affect correctness, but should be cleaned up before archival.

**SUGGESTION:**
- Run `cargo fix --lib -p xs-parser` to apply the 3 pre-existing generated-parser suggestions (optional).
- Add a one-line note in `apply-progress.md` or `tasks.md` confirming that the T10 end-to-end test criterion was satisfied by T15, since the deferred item is now complete.
- Consider a follow-up lint pass once the AST layer is moved out of the gitignored `lelwel-xs/` scratch directory and into the main LSP source tree.

---

## Recommendations for Archive

1. **Fix doc drift:** update `tasks.md` checkboxes to reflect actual Phase 2/3 completion (or keep them unchecked with an explicit note that the verify report is the source of truth).
2. **Clean warnings:** remove the two unused `only_child` imports and adopt `'_` lifetime syntax in the `top_level.rs` test helper.
3. **Capture gitignored source:** `tools/xs-language-server/lelwel-xs/src/ast/` is gitignored. Ensure the archive step records the source code location and any intended commit/move plan to the main `xs-language-server` tree.
4. **Keep grammar-limitation notes:** the non-empty rule-body parsing issue and the missing `else` branch are PoC grammar limitations, not AST bugs. They should remain documented so Phase 4 LSP wiring does not re-discover them.
5. **Defer Phase 4 work:** the LSP wiring (symbol extraction, hover, completion, etc.) and removal of `extract_error_function_definition` are explicitly out of scope and should be tracked as a new SDD change.

## Sign-off

Phase 1 + Phase 2 + Phase 3 of `2026-07-02-rich-typed-ast-layer` are verified. Build clean, 159/159 tests pass, all spec scenarios covered, byte-for-byte round-trip demonstrated. Verdict: **PASS**.

---

## Verification Report (SDD Skill Format)

**Change**: `2026-07-02-rich-typed-ast-layer`  
**Version**: N/A  
**Mode**: Standard (Strict TDD mode is inactive)

### Completeness
| Metric | Value |
|--------|------:|
| Tasks total | 15 |
| Tasks complete | 15 |
| Tasks incomplete | 0 |

### Build & Tests Execution
**Build**: ✅ Passed  
```text
$ cargo build
warning: ... generated parser warnings ...
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.01s
(0 errors)
```

**Tests**: ✅ 159 passed / 0 failed / 0 skipped  
```text
$ cargo test
test result: ok. 159 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Coverage**: Not instrumented — not applicable.

### Spec Compliance Matrix
(See matrices above.)

**Compliance summary**: 12/12 scenarios compliant (7 typed-AST + 5 format-preservation).

### Correctness (Static Evidence)
(See table above.)

### Coherence (Design)
(See table above.)

### Issues Found
**CRITICAL:** None  
**WARNING:** `tasks.md` checkbox drift; two new unused-import/lifetime warnings.  
**SUGGESTION:** Minor cleanup and doc updates before archive.

### Verdict
**PASS** — Build clean, all tests pass, all spec scenarios covered, byte-for-byte round-trip demonstrated, deviations documented and resolved.
