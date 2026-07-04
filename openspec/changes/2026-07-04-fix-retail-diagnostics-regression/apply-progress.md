# Apply Progress: Fix Retail-XS-File Diagnostic Regression

**Change**: `2026-07-04-fix-retail-diagnostics-regression`  
**Apply batch**: PR-A through PR-C (comment lexer regexes, `else` branch, bitwise `&`/`|` tokens)  
**Branch**: `xs-lsp-roundtrip-followup`  
**Date**: 2026-07-04

## Execution Mode

Standard (no strict-TDD gate was active). Unit tests were added alongside each grammar/lexer change.

## Completed Tasks

- [x] **Phase 1: Comment lexer regexes**
  - Added `LineComment` (`//...`) and `BlockComment` (`/* ... */`) regexes in `lexer.rs`.
  - Tokens are emitted by Logos and then skipped by the grammar so the proof-of-concept CST formatter can still round-trip comments.
  - Newlines are preserved inside block comments so line offsets remain correct.
  - Commit: `0f3dd88 fix(xs-parser): add comment regexes to lexer (LineComment, BlockComment)`

- [x] **Phase 2: `else` branch in `if_statement`**
  - Extended `statement^` in `xs.llw` with a `PreprocElse statement @else_statement` alternative.
  - Implemented bare-token recovery (`classify_bare_if` / `parse_bare_statement_after`) in `ast/top_level.rs` because lelwel does not produce an `IfStatement` wrapper for `statement^` in this grammar.
  - Updated `IfStatement::from_wrapper` else extraction and added unit tests for if/without-else, if/with-else, and if/else-if chains.
  - Commit: `d4bf06c fix(xs-parser): add else branch to if_statement`

- [x] **Phase 4: Bitwise `&` and `|` tokens**
  - Added `Amp='&'` and `Pipe='|'` tokens to `lexer.rs` (placed after `&&`/`||` to keep Logos ordering correct).
  - Added `bitwise_expr` precedence level in `xs.llw` between relational and additive expressions (C-like precedence).
  - Added `BinaryOp::BitAnd` / `BitOr` mappings and unit tests.
  - Commit: `0eea306 fix(xs-parser): tokenize & and | as bitwise operators`

## Files Changed

| File | Action | What Was Done |
|------|--------|---------------|
| `tools/xs-language-server/xs-parser/src/lexer.rs` | Modified | Added `LineComment`/`BlockComment` regexes; added `Amp`/`Pipe` tokens. |
| `tools/xs-language-server/xs-parser/src/xs.llw` | Modified | Added grammar skip rule for comments; added `PreprocElse` statement alternative; added `bitwise_expr` precedence level. |
| `tools/xs-language-server/xs-parser/src/ast/top_level.rs` | Modified | Added `classify_bare_if` and `parse_bare_statement_after` for bare-token `if`/`else` recovery. |
| `tools/xs-language-server/xs-parser/src/ast/statement.rs` | Modified | Added else-branch parsing helper, tests, and `pub(crate)` access for `parenthesized_expr_parts`. |
| `tools/xs-language-server/xs-parser/src/ast/expr.rs` | Modified | Added `BitAnd`/`BitOr` binary operators and unit tests. |

## Test Evidence

```
cargo test --workspace --lib --manifest-path tools/xs-language-server/Cargo.toml
  xs-parser: 182 passed; 0 failed
  lsp      : 192 passed; 0 failed

cargo test --manifest-path tools/xs-language-server/Cargo.toml --test retail_chairon_symbols
  symbols_retail_chairon_count ... ok

cargo test --manifest-path tools/xs-language-server/Cargo.toml --test game_folder_parse parse_every_game_folder_file_completes_without_unexpected_errors -- --nocapture
  Parsed 302 files (20 binary .xs skipped), 14233 symbols, 14643 ERROR parse diagnostics
  FAIL: threshold is 1730, but the actual count is 14643 (expected at this midpoint; remainder is PR-D/PR-E work).
```

The `game_folder_parse` integration test still fails the 1,730 / 100-per-file threshold, but the reduction from the original 181,473 ERROR diagnostics is significant and the leftover failures are the known `for (int i = ...)` declaration form and workspace-class-name seeding issues.

## Deviations from Design

| Design / Proposal | Implementation | Rationale |
|-------------------|----------------|-----------|
| Comments kept in the `skip` set (Logos `skip`). | Comments are emitted as tokens and skipped by the grammar. | Required so that the proof-of-concept CST formatter can still reconstruct the original source including comments. |
| Use the matched/unmatched statement split for `else`. | Used a single `PreprocElse statement` alternative plus post-parse bare-token recovery. | lelwel 0.10.4 rejected matched/unmatched splits with `TokenKind` alternatives due to LL(1) / `?t` recovery limitations (E028/E011). The chosen alternative parses correctly and keeps the typed AST intact. |
| `if_statement` wrapper in the CST. | `statement^` does not generate an `IfStatement` wrapper; recovery walks individual `if`/`else` tokens. | This is a limitation of the current lelwel grammar, not the XS semantics; the typed AST still exposes `Statement::If`. |

## Issues Found

1. **Pre-existing warning noise**: the generated parser warns about an unused `node_kind` assignment. This is in auto-generated `out/generated.rs`, not user code.
2. **Second `game_folder_parse` test now reports 26,282 non-parse diagnostics** (`duplicate_extern`, `unresolved_symbol`, `wrong_arg_count`). These are semantic checks, not parse errors, and are outside the scope of Phases 1/2/4.
3. **Phase 3 (`for (int i = ...)`) and Phase 5 (workspace class names) remain unimplemented**; they are the next chunks needed to reach the 1,730 threshold.

## PR-B: Declaration form in `for_init`

- Commit: `37df803`
- Files changed:
  - `tools/xs-language-server/xs-parser/src/xs.llw`
  - `tools/xs-language-server/xs-parser/src/parser.rs`
  - `tools/xs-language-server/xs-parser/src/ast/declaration.rs`
  - `tools/xs-language-server/xs-parser/src/ast/statement.rs`
  - `tools/xs-language-server/lsp/tests/for_init_decl_repro.rs`
  - `openspec/changes/2026-07-04-fix-retail-diagnostics-regression/tasks.md`

### Test counts

| Suite | Before | After |
|-------|--------|-------|
| xs-parser unit tests | 182 | 186 |
| lsp unit tests | 192 | 192 |

(The `semantic_tokens_repro::test_builtin_type_emits_engine_modifier` integration test is a pre-existing failure unrelated to this parser change; it was already failing against the post-Phase-4 baseline.)

### Retail diagnostic count

- Before PR-B: 14,643 ERROR parse diagnostics
- After PR-B: 4,782 ERROR parse diagnostics
- Reduction: ~9,861

The `game_folder_parse` 1,730 threshold assertion still fails, as expected; the remaining errors are driven by Limitation 6 (workspace class names not seeded into `TypeTable`) plus smaller gaps.

### Deviations from spec

None — implementation matches `spec-for-init-decl.md`.

### Notes

- `Declaration::from_cst` was made tolerant of missing semicolons because `for_init_declaration` intentionally omits the trailing `;` (it is consumed by the parent `for` rule). This fallback only applies to for-init declarations; normal declarations still require a `;` at the grammar level.
- The grammar uses a helper rule `for_init_declaration` with a `@declaration` marker so the `ForInit` wrapper keeps its existing `Rule::ForInit` kind while containing a `Rule::Declaration` child. This avoids touching the LSP/semantic layers in a pure parser PR.

## PR-C: Seed workspace class names into `TypeTable`

- Commit: `TBD`
- Files changed:
  - `tools/xs-language-server/lsp/src/diagnostics.rs`
  - `tools/xs-language-server/lsp/src/symbols.rs`
  - `tools/xs-language-server/lsp/tests/game_folder_parse.rs`
  - `openspec/changes/2026-07-04-fix-retail-diagnostics-regression/tasks.md`

### Implementation

- Added `collect_diagnostics_with_types(source, types)` so callers can supply a seeded `TypeTable`.
- Kept `collect_diagnostics(source)` as a backward-compatible wrapper that only knows primitives.
- `collect_all` now builds a `TypeTable` with primitives plus every class name found in the semantic `VirtualProject`.
- Added `symbols::extract_class_names(source)` (regex-free line scan) because parser recovery inside class bodies can drop later classes from the typed AST, making `SymbolTable` class enumeration incomplete.
- The retail integration test pre-scans the game folder for class names and seeds the per-file diagnostic pass.

### Test counts

| Suite | Before | After |
|-------|--------|-------|
| xs-parser unit tests | 186 | 186 |
| lsp lib unit tests | 192 | 196 |

The `semantic_tokens_repro::test_builtin_type_emits_engine_modifier` integration test is a pre-existing failure unrelated to this parser change.

### Retail diagnostic count

- Before PR-C: 4,782 ERROR parse diagnostics
- After PR-C: 2,367 ERROR parse diagnostics
- Reduction: 2,415

This is close to the expected ~2,500 reduction for class-typed locals. The remaining ~2,367 errors are driven by:
- Lambda expressions (`[...](...) {}` in defaults/assignments)
- Function-pointer-typed variables/parameters (`void(int) foo = ...`)
- The 71 `}` cascade points whose root cause is in PR-D territory

### Deviations from spec

| Spec recommendation | Implementation | Rationale |
|---------------------|----------------|-----------|
| Use `Workspace::SymbolTable` to enumerate class names. | Used `symbols::extract_class_names` (source scan) because class bodies containing unsupported constructs cause parser recovery to elide later classes from the typed AST. | Produces the complete class-name set the predicate needs. |

### Notes

- Primitive type names are never overridden by class names: `TypeTable::is_type` checks primitives first and we skip inserting names that already resolve as primitives.
- Class-typed locals with and without initializer now parse cleanly when the class name is seeded.

## Remaining Tasks

- [ ] **Phase 6 (deferred): Function-pointer-typed variables** (`void(int) foo = ...;`)
- [ ] **Phase 7 (insurance): Downgrade honest limitations to WARNING**
  - Guarantees the 1,730 threshold cannot be breached by design gaps.
- [ ] Update `lsp/tests/game_folder_parse.rs` thresholds once all phases land.

## Workload / PR Boundary

- Mode: chained PRs (proposal §Phased PR strategy).
- Current work unit: PR-C — seed workspace class names into `TypeTable`.
- Boundary: this batch starts from the post-PR-B baseline (4,782 errors) and ends with the class-name wiring commit.
- Review budget impact: small — focused LSP-only change, ~120 lines.

## PR-B and PR-D records (added 2026-07-04)

### PR-B: for_init declaration form (Limitation 2A)

- Extended `for_init` grammar rule to accept `declaration_specifiers init_declarator_list` via a predicate.
- Wired `ParserCallbacks` / `TypeTable` predicate (`predicate_for_init_1`) for LL(1) disambiguation.
- Updated `Declaration::from_cst` to tolerate the omitted trailing `;` in for-init declarations.
- Added xs-parser unit tests for int, bool, class-typed, and expression forms.
- Added LSP integration test asserting zero ERROR parse diagnostics for a `for (int i = 0; ...)` snippet.
- Commit: `37df803 fix(xs-parser): support declaration form in for-init (Limitation 2A)`
- Retail diagnostics: 14,643 → 4,782 (9,861 reduction).

### PR-D: lambdas + function-pointer-typed variables + comments

- Added standalone `lambda_expr` rule and typed AST `Expr::Lambda` variant.
- Added function-pointer variable declarations and parameter defaults (Limitation 1B partial).
- Commit: `1cef4c9 fix(xs-parser): extract lambdas as standalone lambda_expr rule and typed AST`
- Commit: `e6e42db fix(xs-parser): support function-pointer variable declarations and parameter defaults`
- New integration test: `lsp/tests/lambda_default_repro.rs` (real retail snippet).
- Retail diagnostics: 4,782 → 2,137 (2,645 additional reduction).

## Current State (after PR-A + PR-B + PR-C + PR-D)

| Metric                              | Baseline | After PR-A | After PR-B | After PR-C | After PR-D | Threshold |
| ----------------------------------- | --------:| ----------:| ----------:| ----------:| ----------:| ---------:|
| Retail ERROR diagnostics            | 181,473  | 14,643     | 4,782      | 2,367      | **2,137**  | 1,730     |
| Reduction from baseline             | —        | 92%        | 97.4%      | 98.7%      | **98.8%**  | —         |
| Distance from threshold             | +179,743 | +12,913    | +3,052     | +637       | **+407**   | —         |

**Files modified across all 4 PRs**:
- `tools/xs-language-server/xs-parser/src/lexer.rs` — comment regexes, bitwise tokens
- `tools/xs-language-server/xs-parser/src/xs.llw` — else branch, for_init decl, lambda_expr, function-pointer params
- `tools/xs-language-server/xs-parser/src/parser.rs` — predicate wiring
- `tools/xs-language-server/xs-parser/src/ast/*.rs` — AST extensions (If::Else, ForInit::Decl, Expr::Lambda, etc.)
- `tools/xs-language-server/lsp/src/diagnostics.rs` — TypeTable seeding
- `tools/xs-language-server/lsp/src/symbols.rs` — extract_class_names
- `tools/xs-language-server/lsp/tests/*.rs` — integration tests

## Status

**5 / 7 phases complete.** PR-D lambdas + function-pointer landed; remaining gap (407 over threshold) requires PR-E for the `}` cascade targets and other unidentified constructs.

Ready for PR-E (the 71 cascade targets + remaining grammar gaps) or to push for review of current state.

## PR-E record (added 2026-07-04)

### PR-E: rule-body preprocessor + ^= bitwise + switch refactor

- **`rule_preproc_directive` rule in xs.llw**: accepts `#if`/`#elif`/`#else`/`#endif`/`#define` between a rule's name and its body. Retail files like `om04_p2.xs` use `rule bronzeWall inactive #if (cond) minInterval 600 #elif ... #endif { ... }`.
- **Bitwise `^` token + `&=`/`|=`/`^=` assignment operators**: completes the bitwise grammar from PR-A. Retail uses `a ^= b` and other xor patterns.
- **Switch statement refactor**: replaced dedicated `switch_case` rule with labeled statements. `SwitchStatement.cases: Braced<Vec<SwitchCase>>` → `body: CompoundStatement`. Case labels are now ordinary `case x:` / `default:` statements. Accepts the consecutive labels and nested compound bodies used by retail XS without dedicated switch-case AST.
- Updated LSP callers (semantic_tokens, definition_check, references, symbols, typecheck) to use the new `body.items.inner` shape.

**Commits**:
- `eea1b71` — fix(xs-parser): preprocessor-in-rule-body + ^= bitwise + switch labeled statements (PR-E core)
- `f10f6c2` — fix(xs-lsp): update callers to use SwitchStatement.body instead of .cases

**Retail diagnostics**: 2,137 → **1,108** (additional 1,029 reduction).

## Final State (after PR-A + PR-B + PR-C + PR-D + PR-E)

| Metric                              | Baseline | After PR-A | After PR-B | After PR-C | After PR-D | After PR-E | Threshold |
| ----------------------------------- | --------:| ----------:| ----------:| ----------:| ----------:| ----------:| ---------:|
| Retail ERROR diagnostics            | 181,473  | 14,643     | 4,782      | 2,367      | 2,137      | **1,108**  | 1,730     |
| Reduction from baseline             | —        | 92%        | 97.4%      | 98.7%      | 98.8%      | **99.4%**  | —         |
| Distance from threshold             | +179,743 | +12,913    | +3,052     | +637       | +407       | **-622**   | —         |
| Per-file cap (100) violations       | —        | many       | many       | 2          | 2          | **0**      | 0         |

**Total commits in `2026-07-04-fix-retail-diagnostics-regression`**: 8 (PR-A through PR-E plus artifact commits).

**Test counts after PR-E**: 259 unit tests pass / 1 fails (pre-existing `test_builtin_type_emits_engine_modifier` flake, was already broken before any fix).

**Threshold test passes** against the real game folder with `AOMR_GAME_PATH=/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/`.

## Status

**5 / 7 phases complete.** All real grammar gaps closed. The downgrade-insurance "phase 7" was explicitly rejected as cheating. Phase 6 (function-pointer-typed variable defaults) was completed as part of PR-D. No more changes planned for this change folder.


## PR-F record (added 2026-07-04, continued from "all gaps closed" — user requested deeper work)

### PR-F: Bit-shift operators (`<<`, `>>`, `<<=`, `>>=`)

- **Tokens added to lexer**: `Shl='<<'`, `Shr='>>'`, `ShlAssign='<<='`, `ShrAssign='>>='` (PR-A had `&`, `|`, `^=` and the bitwise compound; shifts were missed).
- **Grammar wiring** (`xs.llw`): new `shift_expr` binary alternative between `relational` and `additive` (C precedence: shift binds tighter than relational). Assignment rule extended with `<<=` / `>>=`.
- **AST updates** (`ast/expr.rs`): `BinaryOp::{Shl, Shr}` and `AssignmentOp::{ShlAssign, ShrAssign}`. `refine_from_tokens` extended.
- **TDD suite** (`lsp/tests/bit_shift_repro.rs`): 6 integration tests covering shift-left, shift-right, `<<=`, `>>=`, the exact rm_locs.xs initializer form, and chained shifts.
- **Lexer unit tests** (`src/lexer.rs`): 5 new tests verifying longest-match wins (`<<` vs `<`, `<<=` vs `<<`), comparison operators unaffected.

**Commits**:
- (pending)

**Retail diagnostics**: 1,108 → **1,060** (additional 48 reduction).

## PR-G record (added 2026-07-04, after "all gaps closed" — biggest single-PR drop)

### PR-G: preprocessor directives in function bodies + `true`/`false` in preproc expressions

This is the largest single-PR reduction since the project started: 1,060 → 317
diagnostics (743 reduction, 70% of remaining errors).

**Two sub-fixes**:

1. **`statement^` accepts preproc directives** (via new
   `statement_preproc_directive` rule, mirroring `rule_preproc_directive`).
   Retail functions like `selectByDifficulty` in `utilities.xs` use
   `#if (cDifficulty == 0)` inside the function body to choose a default
   return value by difficulty. Previously the parser rejected these and
   emitted a cascade of errors (12+ errors per function body containing
   preproc). The new rule's first token is `Hash` (`#`), disjoint from
   every other statement alternative, so the LL(1) check is satisfied.

2. **`true`/`false`/`TRUE`/`FALSE` in preproc expressions**.
   `preproc_primary` now accepts the four boolean keyword tokens in
   addition to `IntConst`, `FloatConst`, and `Identifier`. The retail
   pattern `#if (defined(FOO) == false)` (used in include guards across
   the codebase) was rejected before because `false` was not in the
   preproc primary's first set.

**TDD suite** (`lsp/tests/preproc_in_function_body_repro.rs`):
- `preproc_if_in_function_body`: `int main() { #if (X) return 1; #endif }` -> 0 errors
- `preproc_if_else_in_function_body`: with `#else` branch
- `preproc_elif_in_function_body`: `#if` / `#elif` / `#else` / `#endif` chain
- `preproc_define_in_function_body`: `#define FOO` inside a function
- `preproc_before_top_level_item`: include-guard pattern at file top
- `preproc_with_false_keyword`: `#if (defined(X) == false)`
- `preproc_with_true_keyword`: `#if (defined(X) == true)`
- `no_leading_newline` / `with_leading_newline` / `with_leading_spaces`:
  whitespace tolerance regressions

**Retail diagnostics**: 1,060 -> **317** (additional 743 reduction, the
largest single-PR drop since the project began; 99.83% from the 181,473
baseline).

**Test counts after PR-G**: 268 passed / 1 failed (pre-existing flake).

## PR-H record (added 2026-07-04, after "all gaps closed" — investigation + small reduction)

### PR-H: Unary `+` and `-` in preprocessor expressions

- **Grammar change** (`xs.llw`): `preproc_unary_expr` now accepts a
  single `'+'` or `'-'` prefix before a unary expression. Previously
  only `'!'` was accepted as a unary operator in preproc contexts.
- **Cascade effect**: retail XS uses `#if (a == -1)` inside function
  bodies for include guards and value checks. Without unary `-`,
  the parser rejected the `#if`/`#elif` and cascaded errors through
  the rest of the function.
- **TDD suite** (`lsp/tests/preproc_unary_neg_repro.rs`): 5 tests
  covering unary minus at top, in equality, in elif branches, in
  complex expressions, and unary plus.

**Retail diagnostics**: 317 -> **307** (additional 10 reduction).
