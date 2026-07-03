# Design: Phase 4 LSP Typed-AST Wiring

## 1. Architecture overview

The LSP stops walking `tree_sitter::Tree` nodes and instead parses into the lossless lelwel `Cst`, builds the typed `TranslationUnit`, and dispatches every handler as a typed-AST walk.

```text
.xs source
    │
    ▼
lsp/src/parser.rs::parse(source, types)
    │
    ├──► xs_parser::Parser::new_with_context(source, types)
    │
    ▼
Cst + Vec<Diagnostic>
    │
    ├──► TranslationUnit::from_cst(&cst, NodeRef::ROOT)
    │         │
    │         ├─► symbols::build_symbol_table(&tu)        ──► SymbolTable
    │         ├─► semantic_tokens::walk(&tu, source)       ──► Vec<Token>
    │         ├─► references::find_identifier_uses(&tu)    ──► Vec<Range>
    │         ├─► typecheck::check_calls_with_merged(&tu)   ──► Vec<Diagnostic>
    │         ├─► definition_check::validate_definitions(&tu) ──► Vec<Diagnostic>
    │
    └──► diagnostics::from_lewel(diags)                    ──► Vec<Diagnostic>
    │
    ▼
LSP handlers (hover / completion / definition / references / semanticTokens / diagnostics)
```

`Cst` and `TranslationUnit` both borrow from the source string; the server already re-parses from the in-memory document on each request, so no lifetime change is required. A new `lsp/src/range.rs` provides the single source-to-LSP-range helper used by every module.

## 2. Pre-work (rename) — execute before any PR-A work

Mechanical, no-semantic-change commit that makes the parser crate a real workspace member:

1. `git mv tools/xs-language-server/lelwel-xs tools/xs-language-server/xs-parser`
2. Delete `.gitignore:33` (`tools/xs-language-server/lelwel-xs/`).
3. Update `tools/xs-language-server/Cargo.toml` workspace members from `"lelwel-xs"` to `"xs-parser"`.
4. The package name is already `xs-parser`, so Cargo infers the lib name `xs_parser`. If a `[lib]` section exists, set `name = "xs_parser"`.
5. There are no `use lelwel_xs::...` callers; update any if found after the move.
6. Run `cargo build --manifest-path tools/xs-language-server/Cargo.toml` and confirm clean.

## 3. PR-A: `symbols.rs` rewrite

Replace the tree-sitter extraction surface with typed-AST constructors.

| Old helper | Replacement |
|---|---|
| `build_symbol_table(tree, source)` | `build_symbol_table(tu: &TranslationUnit) -> SymbolTable` (spec API); internally calls `TranslationUnit::from_cst` if callers pass a `Cst`) |
| `build_full_symbol_table` | Same + walk `FunctionDefinition.body.inner` for `BlockItem::Declaration` |
| `extract_local_declarations` | Walk typed `BlockItem` list; dispatch `Declaration::from_cst` |
| `extract_rule_registrations` | Recursive typed `Expr`/`PostfixExpr` walker looking for `CallExpr` whose callee is one of `xsEnableRule`, `trRuleAdd`, etc. |
| `extract_class` | `ClassDefinition::from_cst` |
| `extract_class_member` | Match `ClassMember` variants (`FunctionDefinition`, `ForwardDeclaration`, `FieldDeclaration`) |
| `extract_rule` | `RuleDefinition::from_cst` |
| `extract_function` | `FunctionDefinition::from_cst` |
| `extract_declaration` | `Declaration::from_cst` |
| `extract_forward_declaration` | `ForwardDeclaration::from_cst` + `DirectDeclarator::FunctionDeclarator` |
| `extract_error_function_definition` / `extract_error_forward_declaration` | **Delete** (lines 557 and 606). They recovered function-pointer and forward-declaration shapes from tree-sitter `ERROR` nodes; the typed AST now produces `FunctionPointerParam` and `ForwardDeclaration` directly. |
| `extract_modifiers` | `DeclarationSpecifiers::from_cst` |
| `extract_params` | `ParameterList::from_cst` via the function declarator |
| `has_ref_qualifier` | `DeclarationSpecifiers::is_ref()` |
| `format_function_pointer_type` | Derive from `FunctionPointerParam.fn_type` and its `ParameterList`; fall back to `&source[span]` text when needed |
| `extract_param_default` | `RegularParam.default` or `FunctionPointerParam.default` (`Option<(Span, _)>`) |

**Call-site impact:** `cache::load_or_parse_symbols`, `semantic.rs::VirtualProject::from_files`, `server.rs`, `merged_view.rs`, `completion.rs` (test helper), and `semantic_tokens.rs` all change from passing `(&tree, source)` to passing `(&tu)` or `(&cst)`. Update symbols-only call sites in PR-A; do not rewrite the other handlers yet.

**Rule-body parity risk:** `RuleDefinition::from_cst` still returns `None` for non-empty rule bodies. To keep `SymbolKind::Rule` symbols from disappearing, add a small CST-level rule-name recovery helper in PR-A: scan the raw `Cst` children of `NodeRef::ROOT` for `Rule` token + `Identifier`, and synthesize `SymbolKind::Rule` entries. This preserves parity with the old tree-sitter path without expanding this PR's grammar scope.

**Tests:** Add `from_cst_correctly_extracts_*` tests for `FunctionDefinition`, `ForwardDeclaration`, `ClassDefinition`, `RuleDefinition`, `Declaration`, `FunctionPointerParam`, `RegularParam`, and `DeclarationSpecifiers` using the fixture source in `xs-parser/examples/test_parse.rs`. Keep the existing 18 symbols module tests, updated to use the typed-AST builder.

## 4. PR-B: handler rewires

Add `lsp/src/range.rs` first; then convert each handler.

### `range.rs`

```rust
pub fn span_to_range(source: &str, span: Span) -> lsp_types::Range;
```

Build a line-offset table once (scan `source` for `\n`/`\r\n`) and translate a byte span to `(line, character)`. Character is computed by UTF-16 code units (LSP convention) by scanning the line; add CRLF, multi-byte, and empty-span unit tests.

### Handler conversions

| Module | Change |
|---|---|
| `semantic_tokens.rs` | Replace `walk_for_tokens`/`collect_symbols` cursor walks with recursive typed-AST traversal over `Expr`, `PostfixExpr`, `TypeSpecifier`, and `Statement`. Emit tokens from node spans via `span_to_range`. `classify_member_identifier` matches `PostfixExpr::Field`. |
| `references.rs` | `find_identifier_uses` walks `Expr::Identifier` (and typed-AST field `name` spans when appropriate). `identifier_range_at(line, col)` converts the cursor position to a byte offset and finds the smallest typed-node span that contains it. `filter_declaration` stays unchanged. |
| `typecheck.rs` | Find call sites via typed `Expr::Call`. `extract_callee_name` becomes a match on `PostfixExpr`. `expr_type` becomes a match on `Expr` variants (`IntLiteral`, `StringLiteral`, etc.). |
| `definition_check.rs` | Walk `TranslationUnit::items`; validate `FunctionDefinition.declarator` params using typed `ParameterList` and `DeclarationSpecifiers::is_ref()`. Const-RHS validation uses `InitDeclarator.expr()` + typed expression constant analysis. |
| `diagnostics.rs` | `collect_diagnostics` consumes `Vec<Diagnostic>` from lelwel (via `parser::parse`) and maps severity. Remove `is_error`/`is_missing` helpers and the `friendly_kind`/`unexpected_token_message` tree-sitter message formatters. The `DiagnosticCategory` classifier and `collect_all` integration stay. |

### Test updates

- `tests/game_folder_parse.rs`: remove `count_unexpected_errors` and `is_recoverable_forward_decl`. Replace with `diagnostics::from_lewel(parse_diags)` severity counts. Initial caps:
  - total `ERROR`-severity parse diagnostics ≤ current tree-sitter baseline (1,571) + 10% ≈ **1,730**;
  - per-file `ERROR` diagnostics ≤ **100**.
  After the first measured run, tighten these to the actual lelwel baseline with a small headroom.
- `tests/symbols_cleanup_repro.rs`: delete the R1-F-02 ERROR-node refutation test; replace it with typed-AST assertions that forward declarations yield `TopLevelItem::ForwardDeclaration`. Keep the R1-F-01 audit test and the renamed R1-F-03 test, updated to use the typed extractor path.

## 5. PR-C: drop tree-sitter and finalize

1. `lsp/Cargo.toml`: remove `tree-sitter`, `tree-sitter-c`, `tree-sitter-language`, `tree-sitter-xs`. Add `xs-parser = { path = "../xs-parser" }`.
2. Workspace `Cargo.toml`: no change after pre-work.
3. `lsp/src/parser.rs`: thin wrapper:
   ```rust
   pub use xs_parser::parser::Parser;
   pub use xs_parser::ast::TypeTable;
   pub struct ParseResult<'a> { pub cst: Cst<'a>, pub diagnostics: Vec<Diagnostic> }
   pub fn parse(source: &str, types: &TypeTable) -> ParseResult<'_>;
   pub fn extract_include_directives(tu: &TranslationUnit) -> Vec<(String, Span)>;
   pub fn detect_include_path_at_position(tu: &TranslationUnit, source: &str, line: u32, col: u32) -> Option<String>;
   ```
4. Delete `lsp/src/bin/day1_probe.rs`, `dump_top_level.rs`, and `inspect_tree.rs` (cleaner than rewrite). `lsp_roundtrip_test.rs` is unaffected.
5. Final grep gates:
   - `grep -r tree_sitter tools/xs-language-server/lsp/src` → 0 hits.
   - `grep tree-sitter tools/xs-language-server/lsp/Cargo.toml` → 0 hits.
6. `cargo build --release` and `cargo test --manifest-path tools/xs-language-server/Cargo.toml` green.

## 6. Module / file changes table

| File | Action | Description |
|---|---|---|
| `tools/xs-language-server/lelwel-xs/` | Rename | Move to `xs-parser/`; unignore in `.gitignore` |
| `tools/xs-language-server/Cargo.toml` | Modify | Workspace member `lelwel-xs` → `xs-parser` |
| `tools/xs-language-server/xs-parser/Cargo.toml` | Modify | Ensure lib name `xs_parser` (or rely on package name) |
| `tools/xs-language-server/lsp/Cargo.toml` | Modify | Drop `tree-sitter*` deps; add `xs-parser` path dep |
| `tools/xs-language-server/lsp/src/parser.rs` | Modify | Thin wrapper over `xs_parser::Parser` |
| `tools/xs-language-server/lsp/src/range.rs` | Create | `span_to_range` source-to-LSP-range helper |
| `tools/xs-language-server/lsp/src/symbols.rs` | Rewrite | Typed-AST `SymbolTable` builder |
| `tools/xs-language-server/lsp/src/semantic_tokens.rs` | Rewrite | Typed AST token walker |
| `tools/xs-language-server/lsp/src/references.rs` | Rewrite | Typed AST identifier uses |
| `tools/xs-language-server/lsp/src/typecheck.rs` | Rewrite | Typed AST call/type check |
| `tools/xs-language-server/lsp/src/definition_check.rs` | Rewrite | Typed AST declaration/param validation |
| `tools/xs-language-server/lsp/src/diagnostics.rs` | Modify | Consume lelwel diagnostics, drop ERROR/MISSING walks |
| `tools/xs-language-server/lsp/src/cache.rs` | Modify | Update `load_or_parse_symbols` signature; keep caching only `SymbolTable` |
| `tools/xs-language-server/lsp/src/semantic.rs` | Modify | Update `parser::parse` calls and rule-registration collection |
| `tools/xs-language-server/lsp/src/server.rs` | Modify | Update `parser::parse` / `build_symbol_table` call sites |
| `tools/xs-language-server/lsp/src/merged_view.rs` | Modify | Update include-directive extraction and parser calls |
| `tools/xs-language-server/lsp/src/bin/day1_probe.rs` | Delete | Tree-sitter debug tool |
| `tools/xs-language-server/lsp/src/bin/dump_top_level.rs` | Delete | Tree-sitter debug tool |
| `tools/xs-language-server/lsp/src/bin/inspect_tree.rs` | Delete | Tree-sitter debug tool |
| `tools/xs-language-server/lsp/tests/game_folder_parse.rs` | Modify | Typed-AST diagnostic thresholds |
| `tools/xs-language-server/lsp/tests/symbols_cleanup_repro.rs` | Modify | Replace ERROR-node tests with typed-AST assertions |

## 7. Compatibility / regression strategy

- Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml` after each PR. Baseline expectation: 252+ unit tests pass and any new typed-AST tests are green. The ~35 pre-existing environmental failures (cwd-relative `doxygen_retail.7z` lookup in some unit tests) must not regress in number or cause.
- Run `tests/game_folder_parse.rs` with `AOMR_GAME_PATH` set after PR-B and PR-C to confirm total/per-file diagnostic caps hold.
- Manually exercise `xs-language-server --stdio` for `initialize`, `textDocument/definition`, `textDocument/references`, and `textDocument/semanticTokens/full`.
- Doc tests will fail in the sandbox because `rustdoc` is missing; document this as an expected environmental failure and exclude from the gate.

## 8. Migration / rollout

| Milestone | State |
|---|---|
| Pre-work merged | `xs-parser/` is tracked; `cargo build` clean. |
| PR-A merged | `SymbolTable` is built from typed AST; remaining handlers still use tree-sitter internally but the server is logically shippable if Phase 4 stops. |
| PR-B merged | All handlers walk the typed AST; tree-sitter dependency remains in `Cargo.toml` but is unused. |
| PR-C merged | Tree-sitter dependencies and debug binaries removed; final green build/test. |

## 9. Open questions / risks

1. **Rule body extraction gap.** If `RuleDefinition::from_cst` still returns `None` for non-empty rule bodies, PR-A must add CST-level rule-name recovery to preserve `SymbolKind::Rule` symbols; otherwise outline and semantic tokens for rules regress.
2. **Environmental test failures.** ~35 tests fail because they look for `doxygen_retail.7z` from the cwd. Confirm they do not break the LSP crate's test run when this change is applied.
3. **No `rustdoc` in sandbox.** Doc-test execution will fail; the verification gate must use `cargo test --lib --tests` or ignore doc-test failures.
4. **Span-to-range edge cases.** CRLF line endings and multi-byte UTF-8 characters must be unit-tested; a regression here corrupts every LSP range.
5. **Gitignore leakage.** The `lelwel-xs/` directory has been ignored; after `git mv`, verify no generated `target/` or `OUT_DIR` artifacts leak under the new `xs-parser/` path.
