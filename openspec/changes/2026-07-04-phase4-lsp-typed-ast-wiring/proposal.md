# Proposal: `2026-07-04-phase4-lsp-typed-ast-wiring`

## 1. Intent

Wire the Rust LSP server (`tools/xs-language-server/lsp/`) to the completed typed AST layer so it stops walking `tree_sitter::Cursor` nodes. This removes the last maintenance sink (`extract_error_function_definition` and its ERROR-node recovery), lets forward declarations be first-class `ForwardDeclaration` nodes, and drops the `tree-sitter` dependency entirely.

## 2. Scope

### In scope
- Rewrite `lsp/src/symbols.rs` to build `SymbolTable` from `TranslationUnit::from_cst`.
- Convert `semantic_tokens.rs`, `references.rs`, `typecheck.rs`, `definition_check.rs`, and `diagnostics.rs` to typed-AST walks.
- Remove every `tree-sitter*` dependency from `lsp/Cargo.toml`.
- Promote the scratch crate `lelwel-xs/` to tracked `xs-parser/`.

### Out of scope
- New LSP features (new semantic-token categories, new handlers).
- Grammar fixes beyond the already-archived rule-body extraction.
- Lifetime-based string interning in the AST.

### Capabilities
- **New capabilities:** None (pure refactor).
- **Modified capabilities:** None at spec level; existing LSP behaviors are re-implemented on typed AST.

## 3. Chained PR strategy

We deliver as **three stacked PRs** so no single review exceeds the 800-line budget and the foundation is verified before the higher-risk handler rewrites land.

| PR | Branch target | Scope | Commit shape | Roll-forward | Roll-back |
|---|---|---|---|---|---|
| **PR-A** | `main` | `symbols.rs` rewrite + call-site signature updates + T17 cleanup. | `git mv lelwel-xs xs-parser` + unignore; rewrite `symbols.rs`; update callers; add symbol-builder tests. | Merge PR-A, rebase PR-B onto it. | `git revert <PR-A-merge>`; restore `lelwel-xs/` if needed. |
| **PR-B** | PR-A branch | Handler rewrites + `span_to_range` + test updates. | One commit per handler; add `span_to_range`; update `game_folder_parse.rs` and `symbols_cleanup_repro.rs`. | Merge PR-B into PR-A branch; keep PR-A branch alive until PR-C ships. | `git revert <PR-B-merge>` on PR-A branch; rebase PR-C if necessary. |
| **PR-C** | PR-B branch | Drop tree-sitter + promote scratch + debug-binary cleanup. | Rename workspace member; remove `tree-sitter*` deps; rewrite/delete debug bins; final green `cargo test`. | Merge PR-C into PR-B branch; merge the stacked chain to `main`. | `git revert <PR-C-merge>` on PR-B branch; restore old `lsp/Cargo.toml` and `tree-sitter-xs/` from git history. |

After PR-A merges, PR-B targets the PR-A branch; after PR-B merges, PR-C targets the PR-B branch. If GitHub diffs show already-merged slices, re-target/rebase until each child diff is clean.

## 4. Pre-work needed before PR-A starts

The scratch crate must become a real workspace member before PR-A consumes it.

1. `git mv tools/xs-language-server/lelwel-xs tools/xs-language-server/xs-parser`
2. Delete `.gitignore:33` (`tools/xs-language-server/lelwel-xs/`).
3. Update `tools/xs-language-server/Cargo.toml` workspace members from `lelwel-xs` to `xs-parser`.
4. Verify `cargo test` still passes in the crate (already 176/176 green).
5. Record these steps as PR-A's first commit so the rename is preserved in history.

## 5. Approach per slice

### PR-A approach
- Replace `build_symbol_table(tree, source)` with `build_symbol_table(cst)` that does `TranslationUnit::from_cst(&cst, NodeRef::ROOT)` and dispatches on `TopLevelItem` variants.
- `build_full_symbol_table` does the same and then walks `FunctionDefinition.body.inner` for block-level declarations.
- Replace each `extract_*` helper with the matching typed-AST constructor/field: `extract_function` → `FunctionDefinition::from_cst`, `extract_declaration` → `Declaration::from_cst`, `extract_class` → `ClassDefinition::from_cst`, etc.
- Delete `extract_error_function_definition` (line 557) and `extract_error_forward_declaration` — the typed AST parses function pointers as `FunctionPointerParam` and forward decls as `ForwardDeclaration`.
- Update all call sites that passed `&tree_sitter::Tree` to pass `&Cst`.

### PR-B approach
- Add a shared `span_to_range(source, span)` helper that maps a lelwel byte span to an LSP `Range` by scanning line breaks.
- Convert each handler to typed-AST traversal methods:
  - `semantic_tokens`: walk `Expr` / `Statement` / `Declaration` nodes instead of cursor `goto_first_child`.
  - `references`: collect identifier uses from `Expr::Identifier` and `PostfixExpr::Field`.
  - `typecheck`: find calls via `Expr::Call` / `PostfixExpr::Call`.
  - `definition_check`: validate params via `FunctionDefinition.declarator` and `DeclarationSpecifiers::is_ref()`.
  - `diagnostics`: route lelwel `Diagnostic` output instead of walking `ERROR`/`MISSING` nodes.
- Update integration tests to expect typed-AST extractors and lelwel diagnostics.

### PR-C approach
- In `lsp/Cargo.toml`: remove `tree-sitter`, `tree-sitter-c`, `tree-sitter-language`, `tree-sitter-xs`; add `xs-parser = { path = "../xs-parser" }`.
- Rewrite `parser.rs` as a thin wrapper: `parse(source) -> Option<Cst>` plus diagnostics and include extraction.
- Delete or rewrite `bin/day1_probe.rs`, `bin/dump_top_level.rs`, `bin/inspect_tree.rs` against the typed AST.
- Drop all unused imports so `grep -r tree_sitter lsp/src lsp/tests` returns empty.

## 6. Affected code paths

### PR-A
| Function | Lines | Current behavior | Replacement |
|---|---|---|---|
| `build_symbol_table` | 137–163 | Walks tree-sitter root children by `kind()` | `TranslationUnit::from_cst` dispatch |
| `build_full_symbol_table` | 167–171 | Calls `extract_local_declarations` on tree nodes | Walk `FunctionDefinition.body.inner` |
| `extract_local_declarations` | 173–186 | Recurses into `compound_statement` | Walk typed `BlockItem` list |
| `extract_local_declaration` | 189–202 | Extracts one variable from `declaration` node | `Declaration::from_cst` |
| `extract_rule_registrations` | 247– | Walks for `xsEnableRule` calls | Walk `TranslationUnit` for `CallExpr` |
| `extract_class` | 294– | Extracts class + members | `ClassDefinition::from_cst` |
| `extract_class_member` | 330– | Walks class children | Match `ClassMember` variants |
| `extract_rule` | 388– | Extracts `rule Name {}` | `RuleDefinition::from_cst` |
| `extract_function` | 413– | Extracts function definition | `FunctionDefinition::from_cst` |
| `extract_declaration` | 456– | Extracts top-level declaration | `Declaration::from_cst` |
| `extract_forward_declaration` | 509– | Extracts `void foo();` | `ForwardDeclaration::from_cst` |
| `extract_error_function_definition` | 557– | **Hack:** recovers fn-ptr defaults from ERROR | **Delete** |
| `extract_error_forward_declaration` | 606– | **Hack:** recovers forward decl from ERROR | **Delete** |
| `extract_modifiers` | 660– | Parses `extern`/`static`/`mutable`/`const` | `DeclarationSpecifiers::from_cst` |
| `extract_params` | 716– | Builds `Vec<Param>` | `ParameterList::from_cst` |
| `has_ref_qualifier` | 747– | Detects `ref` | `DeclarationSpecifiers::is_ref()` |
| `format_function_pointer_type` | 762– | Renders signature text | Derive from `FunctionPointerParam` |
| `extract_param_default` | 776– | Extracts `= expr` text | `RegularParam.default` / `FunctionPointerParam.default` |
| `node_range` / `node_text` / `find_named_child` | 791–803 | tree-sitter helpers | `span_to_range` + `cst` helpers |

### PR-B
| File | What changes |
|---|---|
| `lsp/src/semantic_tokens.rs` | Replace `walk_for_tokens` and `collect_symbols` with typed-AST traversal. |
| `lsp/src/references.rs` | Replace `find_identifier_uses` and `identifier_range_at` with AST walks + span map. |
| `lsp/src/typecheck.rs` | Replace `check_calls_with_merged` and `expr_type` with typed `Expr` matching. |
| `lsp/src/definition_check.rs` | Replace `validate_definitions` walk with typed `TranslationUnit::items` matching. |
| `lsp/src/diagnostics.rs` | Consume lelwel `Diagnostic` vector instead of `is_error`/`is_missing` nodes. |
| `lsp/tests/game_folder_parse.rs` | Replace ERROR-node thresholds with lelwel diagnostic baselines. |
| `lsp/tests/symbols_cleanup_repro.rs` | Delete ERROR-recovery tests; add first-class forward-decl tests. |

### PR-C
| File/Path | Change |
|---|---|
| `tools/xs-language-server/lsp/Cargo.toml` | Drop `tree-sitter*`, add `xs-parser`. |
| `tools/xs-language-server/Cargo.toml` | Workspace member `lelwel-xs` → `xs-parser`. |
| `tools/xs-language-server/lsp/src/parser.rs` | Thin wrapper around `xs_parser::Parser`. |
| `tools/xs-language-server/lsp/src/bin/day1_probe.rs` | Rewrite or delete. |
| `tools/xs-language-server/lsp/src/bin/dump_top_level.rs` | Rewrite or delete. |
| `tools/xs-language-server/lsp/src/bin/inspect_tree.rs` | Rewrite or delete. |
| `.gitignore:33` | Remove `tools/xs-language-server/lelwel-xs/` line. |

## 7. Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| **Behavioral regressions** if the typed AST extracts fewer/more symbols than the tree-sitter path did. | Medium | Keep existing symbol-table unit tests as a regression harness; add typed-AST-specific tests for every `extract_*` replacement; run game-folder integration test before PR-C. |
| **Span-mapping regressions** if `span_to_range` mishandles multi-byte characters or line breaks. | Medium | Unit-test `span_to_range` with CRLF, multi-byte, and empty spans; mirror tree-sitter `Point` outputs for fixtures. |
| **Per-file cargo test breakage on doc tests** because `rustdoc` is not in the sandbox. | High (environmental) | Document as known/expected environmental failure; do not block the change on it. |
| **Deleted `extract_error_*` functions surface AST misses** for edge-case forward declarations or function-pointer params. | Low-Medium | Add targeted tests for forward decls, function-pointer params, and lambda defaults before deleting the helpers; audit against retail game folder. |

## 8. Acceptance

- [ ] All 252 existing LSP unit tests stay green (or improve).
- [ ] New unit tests added for typed-AST symbol extraction and `span_to_range`.
- [ ] `cargo test --manifest-path tools/xs-language-server/Cargo.toml` introduces no **new** failures (the existing ~35 environmental failures from cwd-relative `doxygen_retail.7z` lookup remain acceptable).
- [ ] After PR-C, `grep -r tree_sitter tools/xs-language-server/lsp/` returns 0 hits.
- [ ] After PR-C, `grep -r tree-sitter tools/xs-language-server/lsp/Cargo.toml` returns 0 hits.
- [ ] `cargo build --release` is green after PR-C.

## 9. Alternate approaches rejected

- **Keep tree-sitter as a fallback** — rejected. Maintaining two parser back-ends indefinitely contradicts T19 and the migration's goal of first-class forward declarations. The typed AST is the parser of record.
- **Big-bang single PR** — rejected. The rewrite touches ~3,800 lines of tree-sitter-adjacent code and would exceed the 800-line review budget. A single 1,500+ line diff would also hide rule-extraction edge cases in `symbols.rs` and make bisection impossible.

## 10. References

- Archive: `openspec/changes/archive/2026-07-02-rich-typed-ast-layer/`
- Archive: `openspec/changes/archive/2026-07-03-fix-rule-body-extraction/`
- Explore map: `openspec/explore/2026-07-03-phase4-lsp-wiring/explore.md`
- Migration plan: `docs/plans/2026-07-01-migrate-tree-sitter-to-lelwel.md`
- Design doc: `docs/plans/2026-07-02-typed-ast-design.md`
- Canonical specs: `openspec/specs/spec-typed-ast.md`, `openspec/specs/spec-format-preservation.md`, `openspec/specs/spec-rule-body-extraction.md`

## 11. Sign-off

Effective verification method per `openspec/config.yaml` strict-TDD override for `tools/xs-language-server/**`: `cargo test --manifest-path tools/xs-language-server/Cargo.toml`. The global `manual_deploy_and_ingame_log` method does not apply to this tooling-only change.
