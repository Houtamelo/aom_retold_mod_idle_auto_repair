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

## Remaining Tasks

- [ ] **Phase 5: Seed workspace class names into `TypeTable`**
  - Update `lsp/src/diagnostics.rs` to pass class names to `TypeTable::with_primitives()`.
- [ ] **Phase 6 (deferred): Function-pointer-typed variables** (`void(int) foo = ...;`)
- [ ] **Phase 7 (insurance): Downgrade honest limitations to WARNING**
  - Guarantees the 1,730 threshold cannot be breached by design gaps.
- [ ] Update `lsp/tests/game_folder_parse.rs` thresholds once all phases land.

## Workload / PR Boundary

- Mode: chained PRs (proposal §Phased PR strategy).
- Current work unit: PR-A through PR-B — comment regexes, `else`, bitwise tokens, and `for_init` declaration form.
- Boundary: this batch starts from the post-Phase-4 baseline and ends with the four parser commits above.
- Review budget impact: small — each commit is focused, and the combined user-facing diff is under 300 lines.

## Status

4 / 7 phases complete.  
Ready for verify on the PR-A/PR-B slice, or continue with Phase 5 in the next apply batch.
