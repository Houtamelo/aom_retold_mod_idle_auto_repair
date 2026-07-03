# Tasks: Phase 4 LSP Typed-AST Wiring

**Change type:** `chained-PR` — PR-A → PR-B → PR-C

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~3,800 total (PR-A ~1,000, PR-B ~2,000, PR-C ~500) |
| 400-line budget risk | High |
| Chained PRs recommended | Yes |
| Suggested split | PR-A → PR-B → PR-C |
| Delivery strategy | ask-on-risk |
| Chain strategy | pending |

Decision needed before apply: Yes
Chained PRs recommended: Yes
Chain strategy: pending
400-line budget risk: High

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Promote scratch parser crate to tracked workspace member | Pre-work / PR-A commit 1 | `git mv`, `.gitignore`, workspace member update |
| 2 | Build `SymbolTable` from typed AST | PR-A | Foundation for all handlers |
| 3 | Convert handlers to typed-AST walks | PR-B | `semantic_tokens`, `references`, `typecheck`, `definition_check`, `diagnostics` |
| 4 | Drop tree-sitter dependency and finalize | PR-C | Cargo cleanup, debug-bin removal, final green build |

## Phase 1: Pre-work (rename)

- [x] 1.1 **T-RNW-01** `git mv tools/xs-language-server/lelwel-xs tools/xs-language-server/xs-parser`; remove `.gitignore:33` line; change workspace member `lelwel-xs` to `xs-parser` in `tools/xs-language-server/Cargo.toml`.
- [x] 1.2 **T-RNW-02** Add or update `[lib]` `name = "xs_parser"` in `tools/xs-language-server/xs-parser/Cargo.toml`; search and update any remaining `use lelwel_xs::...` paths.
- [x] 1.3 **T-RNW-03** Run `/usr/local/cargo/bin/cargo build --manifest-path tools/xs-language-server/Cargo.toml` and confirm a clean build after the rename.

## Phase 2: PR-A — symbols.rs rewrite

> Premise check: `RuleDefinition::from_cst` now succeeds for non-empty rule bodies (S-RBE-01..08 pass; `lelwel-xs` unit tests 176/176 green). The design's CST-level rule-name recovery helper is **omitted** as stale.

- [x] 2.1 **T-LSP-A-01** Re-implement `lsp/src/symbols.rs::build_symbol_table` to call `xs_parser::Parser::parse(source)`, build `TranslationUnit::from_cst(&cst)`, and dispatch on `TopLevelItem` variants; keep the public signature stable or document any tiny change.
- [x] 2.2 **T-LSP-A-02** Delete `extract_error_function_definition` and `extract_error_forward_declaration` from `lsp/src/symbols.rs`; verify every call site compiles without them.
- [x] 2.3 **T-LSP-A-03** Add or update a test using a retail sample such as `game/ai/core/chairon.xs`; assert the typed-AST `SymbolTable` count matches the old tree-sitter count minus deleted `extract_error_*` workarounds.

## Phase 3: PR-B — handler rewires

- [ ] 3.1 **T-LSP-B-01** Create `lsp/src/range.rs` with `pub fn span_to_range(source: &str, span: Span) -> lsp_types::Range`; add CRLF, multi-byte, and empty-span unit tests.
- [ ] 3.2 **T-LSP-B-02** Rewrite `lsp/src/semantic_tokens.rs` to walk the typed AST and emit tokens via `span_to_range`.
- [ ] 3.3 **T-LSP-B-03** Rewrite `lsp/src/references.rs::find_references` to walk the typed AST.
- [ ] 3.4 **T-LSP-B-04** Rewrite `lsp/src/definition_check.rs::definition_position` to walk the typed AST.
- [ ] 3.5 **T-LSP-B-05** Rewrite `lsp/src/diagnostics.rs` to count `xs_parser::Diagnostic` items by severity, removing `is_error`/`is_missing` checks and tree-sitter message formatters.
- [ ] 3.6 **T-LSP-B-06** Rewrite the rest of `lsp/src/definition_check.rs` (declaration/param validation walker) against the typed AST.
- [ ] 3.7 **T-LSP-B-07** Rewrite `lsp/src/typecheck.rs` to walk typed `Expr::Call` / `PostfixExpr::Call` and map `expr_type` to `Expr` variants.
- [ ] 3.8 **T-LSP-B-08** Update `lsp/tests/game_folder_parse.rs` thresholds per user choice G2: assert typed-AST `ERROR`-severity diagnostic count is no higher than the tree-sitter baseline and record the measured baseline in `apply-progress.md`.
- [ ] 3.9 **T-LSP-B-09** Replace `lsp/tests/symbols_cleanup_repro.rs` with typed-AST extraction tests on the same fixtures.

## Phase 4: PR-C — drop tree-sitter and finalize

- [ ] 4.1 **T-LSP-C-01** Remove `tree-sitter`, `tree-sitter-c`, `tree-sitter-language`, and `tree-sitter-xs` from `tools/xs-language-server/lsp/Cargo.toml`; add `xs-parser = { path = "../xs-parser" }`; verify clean build.
- [ ] 4.2 **T-LSP-C-02** Simplify `lsp/src/parser.rs` to a thin wrapper over `xs_parser::parser::Parser::new`.
- [ ] 4.3 **T-LSP-C-03** Delete `lsp/src/bin/day1_probe.rs`, `lsp/src/bin/dump_top_level.rs`, and `lsp/src/bin/inspect_tree.rs`.
- [ ] 4.4 **T-LSP-C-04** Run the full unit-test suite: `/usr/local/cargo/bin/cargo test --lib --tests --manifest-path tools/xs-language-server/Cargo.toml`; verify all 252+ baseline and new tests pass. `rustdoc` doc-test failures are expected in this sandbox and must be excluded from the gate.
- [ ] 4.5 **T-LSP-C-05** Manually exercise `xs-language-server --stdio` on a fixture for `initialize`, `shutdown`, and one of `textDocument/definition` or `textDocument/semanticTokens`.

## Dependency graph

```text
T-RNW-01 ──┐
T-RNW-02 ──┼──► PR-A ──► PR-B ──► PR-C
T-RNW-03 ──┘

Within PR-A:  T-LSP-A-01 ─► T-LSP-A-02 ─► T-LSP-A-03
Within PR-B:  T-LSP-B-01 ─► T-LSP-B-02..B-09 (any order after B-01)
Within PR-C:  T-LSP-C-01 ─► T-LSP-C-02 ─► T-LSP-C-03 ─► T-LSP-C-04 ─► T-LSP-C-05
```

## Test plan

| Scenario | Task | Test(s) to add / update | Acceptance |
|----------|------|------------------------|------------|
| S-LSP-A-01 | T-LSP-A-01 | `from_cst_correctly_extracts_rule_class_function` | `TranslationUnit` contains matching `RuleDefinition`, `ClassDefinition`, and `FunctionDefinition` |
| S-LSP-A-02 | T-LSP-A-01 | `build_symbol_table_yields_symbol_entry_per_item` | one `SymbolEntry` per top-level item with correct `SymbolKind` and byte-span `Range` |
| S-LSP-A-03 | T-LSP-A-02 | grep + compile check | `extract_error_function_definition` absent and no remaining calls compile |
| S-LSP-A-04 | T-LSP-A-02 | grep + compile check | `extract_error_forward_declaration` absent and no remaining calls compile |
| S-LSP-A-05 | T-LSP-A-03 | `symbols_retail_chairon_count` | typed-AST symbol count matches old tree-sitter count minus deleted workarounds |
| S-LSP-A-06 | T-RNW-01..03 | `cargo build` and `cargo test` | rename leaves workspace build green |
| S-LSP-A-07 | T-LSP-A-01 | keep existing 18 `symbols.rs` tests updated | existing symbol-table tests stay green on typed AST |
| S-LSP-B-01 | T-LSP-B-01, B-02 | `span_to_range_crlf_multibyte_empty`; `semantic_tokens_full_typed_ast` | tokens produced via `span_to_range`; `grep -r tree_sitter lsp/src/semantic_tokens.rs` returns 0 |
| S-LSP-B-02 | T-LSP-B-03 | `find_references_typed_ast_matches_tree_sitter` | reference list equals old path for sampled identifier |
| S-LSP-B-03 | T-LSP-B-04 | `definition_position_typed_ast_returns_range` | returns typed-AST-defined position for resolved identifier |
| S-LSP-B-04 | T-LSP-B-05 | `diagnostics_count_by_severity` | no `is_error`/`is_missing` usage; severity counts match lelwel output |
| S-LSP-B-05 | T-LSP-B-06 | `definition_check_params_and_declarations_typed_ast` | validation walks `FunctionDefinition.declarator` and `DeclarationSpecifiers::is_ref()` |
| S-LSP-B-06 | T-LSP-B-08 | `game_folder_parse_typed_ast_thresholds` | `ERROR`-severity diagnostic count ≤ recorded baseline; per-file cap holds |
| S-LSP-B-07 | T-LSP-B-09 | `symbols_cleanup_typed_ast_forward_declarations` | new tests assert typed-AST extraction on old fixtures |
| S-LSP-C-01 | T-LSP-C-01, C-04 | grep gate | `grep -r tree_sitter tools/xs-language-server/lsp/src` returns 0 |
| S-LSP-C-02 | T-LSP-C-01 | grep gate | `grep tree-sitter tools/xs-language-server/lsp/Cargo.toml` returns 0 |
| S-LSP-C-03 | T-LSP-C-02 | `parser_wrapper_returns_cst_and_diagnostics` | `parse(source, types)` wraps `xs_parser::parser::Parser::new`; no tree-sitter state |
| S-LSP-C-04 | T-LSP-C-03 | file-existence check | `day1_probe`, `dump_top_level`, `inspect_tree` source files are gone |
| S-LSP-C-05 | T-RNW-01..03 | workspace build check | `xs-parser/` is tracked, `.gitignore` line removed |
| S-LSP-C-06 | T-LSP-C-04 | `cargo test --lib --tests` | 252+ tests pass; no new failures beyond environmental rustdoc issues |
| S-LSP-C-07 | T-LSP-C-05 | manual stdio round-trip | server responds to `initialize`, `shutdown`, and one handler request |

## Work-unit commit boundaries

### Pre-work / PR-A

1. `git mv lelwel-xs xs-parser`; unignore; update workspace member; add `[lib] name = "xs_parser"`.
2. Rewrite `lsp/src/symbols.rs` against typed AST; update `build_symbol_table` call sites in `cache.rs`, `semantic.rs`, `server.rs`, `merged_view.rs`, `completion.rs` tests.
3. Delete `extract_error_function_definition` and `extract_error_forward_declaration`; fix fallout.
4. Add typed-AST symbol-builder tests and retail count test.

### PR-B

1. Add `lsp/src/range.rs` + unit tests.
2. Rewrite `semantic_tokens.rs`.
3. Rewrite `references.rs`.
4. Rewrite `definition_check.rs`.
5. Rewrite `diagnostics.rs`.
6. Rewrite `typecheck.rs`.
7. Update integration tests (`game_folder_parse.rs`, `symbols_cleanup_repro.rs`) and record diagnostic baseline.

### PR-C

1. Drop tree-sitter deps from `lsp/Cargo.toml`; add `xs-parser` path dep.
2. Thin `lsp/src/parser.rs` wrapper.
3. Delete debug binaries.
4. Final green `cargo test --lib --tests` and manual stdio round-trip.

## Risk register

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Behavioral regressions in symbol counts | Medium | Keep existing symbol-table tests; add typed-AST extraction tests; run retail sample count test |
| Span-mapping regressions (CRLF/UTF-8/empty spans) | Medium | Unit-test `span_to_range` with CRLF, multi-byte, and empty-span fixtures |
| Deleted `extract_error_*` surface AST misses | Low–Medium | Add targeted forward-decl and function-pointer-param tests before deleting helpers |
| Environmental test break (missing `rustdoc`) | High | Gate verification on `cargo test --lib --tests`; document doc-test failures as expected |
| Game-folder diagnostic thresholds too tight/loose | Medium | Use initial caps from design; record measured baseline in PR-B `apply-progress.md`; tighten after first run |
| Gitignore/target leakage after rename | Low | Verify `.gitignore:33` removed; ensure no `target/` artifacts are committed under new path |

## Delivery strategy

- **Mode:** chained PR (PR-A → PR-B → PR-C).
- **Default chain strategy:** stacked-to-main; user may override to feature-branch-chain before apply.
- **Review budget:** 800 changed lines per slice.
- **Each PR is a focused, independently reviewable unit.** PR-A is the foundation; PR-B is the highest-risk behavioral rewrite; PR-C is cleanup and final dependency removal.
