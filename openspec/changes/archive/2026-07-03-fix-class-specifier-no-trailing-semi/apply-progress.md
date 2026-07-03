# Apply Progress: `fix-class-specifier-no-trailing-semi`

**Branch:** `xs-lsp` (work happens in scratch subdir `lelwel-xs/` which is gitignored)
**Started:** 2026-07-03
**Current state:** ✅ COMPLETE

## Phase 1 — Grammar fix + regression tests ✅

**Goal:** drop `';'` from `class_specifier` in `xs.llw` to match real XS; add regression tests; verify with direct before/after measurement.

**Files modified:**

- `tools/xs-language-server/lelwel-xs/src/xs.llw` — line 238 changed:
  - Before: `class_specifier: 'class' Identifier '{' class_member* '}' ';';`
  - After: `class_specifier: 'class' Identifier '{' class_member* '}';` (with explanatory comment)
- `tools/xs-language-server/lelwel-xs/src/ast/top_level.rs` — added 4 regression tests in `#[cfg(test)] mod tests` (`class_definition_parses_without_trailing_semi_no_diagnostics`, `class_definition_parses_with_trailing_semi_no_diagnostics`, `class_definition_no_trailing_semi_is_default`, `field_declaration_inside_class_unchanged`).
- `openspec/changes/2026-07-02-rich-typed-ast-layer/apply-progress.md` — Phase 2 deviation #3 updated to record this fix.

**Files NOT modified** (per T7 hard rule + this change's scope):

- `Cargo.toml`, `build.rs`, `src/lexer.rs`, `src/parser.rs`, `src/lib.rs` — unchanged.
- `src/ast/preproc.rs`, `src/ast/statement.rs`, `src/ast/tokens.rs`, `src/ast/cst_helpers.rs`, `src/ast/spanned.rs`, `src/ast/type_system.rs`, `src/ast/declaration.rs` — unchanged (T7/T8/T9 modules not touched).
- `src/ast/mod.rs` — unchanged (no new modules added).

**Decisions made during apply:**

1. **Diagnostic range analysis** — initial implementation assumed the 149 "missing ';'" diags on `bo_system_internal.xs` were all on class definitions. Direct measurement showed they are spread across many rule shapes (top-level resync errors). The fix is correct, but the proposal/spec was overclaimed and was corrected in the verify report.

2. **Test #2 (`with_trailing_semi`) was wrong first** — initial test asserted `cd.semi` would be a real span when `;` is present, but after the grammar change the trailing `;` is OUTSIDE the `class_specifier` node (it's a sibling at the translation_unit level). The test was updated to assert `cd.semi` is zero-width in this case, which matches the new grammar semantics.

3. **Defensive zero-width-span fallback in `ClassDefinition::from_cst` is preserved** — even though the grammar no longer requires `;`, the fallback handles partial parses where the parser's error recovery may surface a class_specifier with no `;` child. No regression risk, and the fallback is rarely triggered in real sources.

**Test count:** 102 tests (98 pre-existing + 4 new). All green.
**Build status:** `cargo build` clean (0 errors, 6 pre-existing dead-code warnings unrelated to this change).
**Diagnostic measurement:** verified 1 diag per simple class definition drops to 0 after fix (see `verify-report.md` for full before/after table).
