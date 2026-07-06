# Phase 4 — Wire LSP to the typed AST (exploration map)

**Scope:** swap `tools/xs-language-server/lsp/src/` from `tree_sitter::Tree` / `Cursor` to the typed AST in `tools/xs-language-server/lelwel-xs/src/ast`.

**Skill resolution:** `paths-injected` (loaded `sdd-explore` from `file:///home/houtamelo/.config/opencode/skills/sdd-explore/SKILL.md`).

---

## Section 1 — current tree_sitter surface area

### Files that touch `tree_sitter` directly

| File | tree-sitter types / functions | Lines (direct API) |
|------|------------------------------|--------------------|
| `lsp/src/parser.rs` | `Language`, `Parser`, `Tree`, `TreeCursor`, `Node`, `root_node`, `walk`, `children`, `child_by_field_name`, `byte_range`, `start_position`, `end_position`, `named_children` | 10–187 |
| `lsp/src/symbols.rs` | `Tree`, `Node`, `TreeCursor`, `root_node`, `walk`, `children`, `named_children`, `child_by_field_name`, `byte_range`, `start_position`, `end_position`, `kind`, `is_named` | 137–803 |
| `lsp/src/semantic_tokens.rs` | `Tree`, `Node`, `TreeCursor`, `root_node`, `walk`, `children`, `goto_first_child`, `goto_next_sibling`, `goto_parent`, `byte_range`, `start_position`, `end_position`, `child_by_field_name`, `parent`, `kind` | 90–545 |
| `lsp/src/references.rs` | `Tree`, `Node`, `TreeCursor`, `Point`, `root_node`, `walk`, `children`, `descendant_for_point_range`, `byte_range`, `start_position`, `end_position`, `parent`, `kind` | 10–83 |
| `lsp/src/typecheck.rs` | `Tree`, `Node`, `TreeCursor`, `root_node`, `walk`, `named_children`, `byte_range`, `start_position`, `end_position`, `child_by_field_name`, `kind` | 35–455 |
| `lsp/src/definition_check.rs` | `Node`, `Tree`, `root_node`, `walk`, `children`, `named_children`, `child_by_field_name`, `byte_range`, `start_position`, `end_position`, `kind` | 19–241 |
| `lsp/src/diagnostics.rs` | `Node`, `Tree`, `root_node`, `walk`, `children`, `is_error`, `is_missing`, `byte_range`, `start_position`, `end_position`, `is_named`, `kind` | 11–228 |
| `lsp/src/semantic.rs` | `Tree`, `Node`, `TreeCursor`, `root_node`, `walk`, `children`, `byte_range`, `start_position`, `end_position`, `named_children`, `child_by_field_name`, `kind` | 840–875 |
| `lsp/src/workspace.rs` | one test helper only (`direct_include_resolves_via_tree_sitter`) | 686 |
| `lsp/src/bin/day1_probe.rs` | full tree-sitter walker / debug tool | 5–215 |
| `lsp/src/bin/dump_top_level.rs` | full tree-sitter walker / debug tool | 5–80 |
| `lsp/src/bin/inspect_tree.rs` | full tree-sitter walker / debug tool | 5–58 |
| `tree-sitter-xs` (sibling crate) | grammar + generated Rust bindings used via `tree-sitter-xs` dep | — |

### Counts

- **Direct tree-sitter API matched lines (production + debug bins): ~233.**
- **Most heavily Cursor-dependent modules (`goto_first_child` / `goto_next_sibling` / `goto_parent`):**
  - `symbols.rs` (~27 cursor walks)
  - `semantic_tokens.rs` walk_for_tokens (lines 117–183) and `collect_symbols` (lines 478–532)
  - `references.rs` recursive `walk` (lines 37–45)
  - `typecheck.rs` recursive `walk` (lines 59–75)
  - `diagnostics.rs` recursive `walk` (lines 30–37)
  - `definition_check.rs` declaration / parameter walks
- **LOC in modules that will be rewritten:** `symbols.rs` 1,005, `semantic_tokens.rs` 621, `typecheck.rs` 933, `definition_check.rs` 453, `diagnostics.rs` 444, `references.rs` 185, `parser.rs` 187. Total ≈ 3,800 LOC of tree-sitter-adjacent code.

### Code that must become AST walks

- `symbols::build_symbol_table` walks `tree.root_node().children(...)` over `"rule_definition" | "function_definition" | "class_specifier" | "declaration" | "ERROR"`.
- `semantic_tokens::walk_for_tokens` uses `cursor.goto_first_child / goto_next_sibling` to visit every identifier/field_identifier/type_identifier/primitive_type.
- `references::find_identifier_uses` recursively walks all nodes looking for `"identifier"`.
- `typecheck::check_calls_with_merged` recursively walks `named_children` looking for `"call_expression"`.
- `diagnostics::collect_diagnostics` recursively walks `is_error || is_missing` nodes.
- `definition_check::validate_definitions` walks top-level children for `"function_definition" | "declaration"`.

---

## Section 2 — extract_* / find_* functions in `symbols.rs`

| Function | Purpose | Typed-AST replacement | Consumers |
|----------|---------|----------------------|-----------|
| `build_symbol_table` (137) | Build `SymbolTable` from a parse tree | `TranslationUnit::from_cst(&cst, NodeRef::ROOT)` and iterate `items` | `server.rs`, `cache.rs`, `completion.rs`, `merged_view.rs`, `semantic.rs` |
| `build_full_symbol_table` (167) | Same + local variables inside bodies | `build_symbol_table` + walk `FunctionDefinition.body.inner` for `BlockItem::Declaration` | `server.rs` (`rebuild_symbol_table`) |
| `extract_local_declarations` (173) | Recurse into `compound_statement` for locals | Walk `CompoundStatement.items` / recurse into nested `FunctionDefinition` | internal only |
| `extract_local_declaration` (189) | Extract one local variable | `Declaration::from_cst` → `InitDeclarator` → `Declarator` | internal only |
| `extract_rule_registrations` (247) | Find `xsEnableRule("...")` etc. | Walk `TranslationUnit` for call expressions; `CallExpr` via `Expr` or `UnparsedExpr` | `semantic.rs`, `semantic.rs::VirtualProject::from_files` |
| `extract_class` (294) | Extract class + members | `ClassDefinition::from_cst` and its `members: Vec<ClassMember>` | internal |
| `extract_class_member` (330) | Extract field/method inside class | `ClassMember::FieldDeclaration`, `ClassMember::FunctionDefinition` | internal |
| `extract_rule` (388) | Extract `rule Name {}` | `RuleDefinition::from_cst` (note: current PoC grammar returns `None` for non-empty bodies) | internal |
| `extract_function` (413) | Extract function definition | `FunctionDefinition::from_cst` | internal |
| `extract_declaration` (456) | Extract top-level / block variable | `Declaration::from_cst` | internal |
| `extract_forward_declaration` (509) | Extract `void foo();` | `ForwardDeclaration::from_cst` with `DirectDeclarator::FunctionDeclarator` | internal |
| `extract_error_function_definition` (557) | **Hack:** recover function def from an `ERROR` node | **Delete** — typed AST parses function-pointers correctly as `FunctionPointerParam`, and forward decls parse as `ForwardDeclaration` | internal |
| `extract_error_forward_declaration` (606) | **Hack:** recover forward decl from an `ERROR` node | **Delete** — `ForwardDeclaration::from_cst` handles this directly | internal |
| `extract_modifiers` (660) | Parse `extern`/`static`/`mutable`/`const` | `DeclarationSpecifiers::from_cst` (`storage_class_specifiers`, `type_qualifiers`) | internal |
| `extract_params` (716) | Build `Vec<Param>` | `ParameterList::from_cst` → `ParameterDeclaration` → `RegularParam` / `FunctionPointerParam` | internal |
| `has_ref_qualifier` (747) | Detect `ref` qualifier | `DeclarationSpecifiers::is_ref()` | internal |
| `format_function_pointer_type` (762) | Render `void(int)` for params | Derive from `FunctionPointerParam.fn_type` + its `ParameterList` | internal |
| `extract_param_default` (776) | Extract `= expr` text | `RegularParam.default` / `FunctionPointerParam.default` (both carry `(Span, UnparsedExpr)`) | internal |
| `node_range` / `node_text` / `find_named_child` (791–803) | tree-sitter helpers | Replace with `span_to_range(source, span)` and `cst.match_token(...)` / `cst.span(node)` | many |

### Function-pointer hacks

- `extract_error_function_definition` exists because the **tree-sitter XS grammar** parses function-pointer parameters with lambda defaults as an `ERROR` node when the default contains an inline lambda.
- The typed AST already distinguishes:
  - `ParameterInner::FunctionPointerParam` for `void(int) cb`
  - `ParameterInner::RegularParam` for normal params
  - `Expr::LambdaExpr` via `expr()` helpers on `InitDeclarator`, `RegularParam`, `FunctionPointerParam`
- Therefore T17 is a simple deletion once `symbols.rs` is rewritten.

---

## Section 3 — handler-level dependencies

### `semantic_tokens.rs`

Calls / data from `symbols.rs`:

- `symbols::SymbolTable` (line 83, 122, 202, 255, 343)
- `symbols::Symbol` / `SymbolKind` / `Visibility` (line 21, 24)
- `own_table.find(name)` (line 217)
- `MemberIndex::build` uses `own_table.symbols` and `cache::load_or_parse_symbols` (which calls `symbols::build_symbol_table`)
- `extract_symbols(tree, source)` (line 471) — used only by its own test

LSP protocol: `textDocument/semanticTokens/full` (`server.rs:672`).
Symbol path: `semantic_tokens::compute_tokens` consumes `own_table` + `MemberIndex`.

### `completion.rs`

Calls from `symbols.rs`:

- `symbols::{Symbol, SymbolKind}` (line 13)
- `symbols::build_symbol_table` in test helper `build_merged_view` (line 176)
- `symbol_to_completion_item` uses `sym.kind` and `sym.detail`

LSP protocol: `textDocument/completion` (`server.rs:519`).
Symbol path: completions come from `MergedView` (built from symbol tables), not directly from `symbols.rs` per keystroke.

### `definition_check.rs`

Does **not** call `symbols.rs`; it walks tree-sitter nodes directly. It must be rewritten to walk the typed AST:

- top-level declarations → `TranslationUnit::items`
- function params → `FunctionDefinition.declarator` → `FunctionDeclarator.params`
- `ref` qualifier → `DeclarationSpecifiers::is_ref()`
- default presence → `RegularParam.default.is_some()` / `FunctionPointerParam.default.is_some()`
- const RHS constant check → `Declaration`/`InitDeclarator.expr()` + typed expression analysis

LSP protocol: diagnostics generation (`server.rs:1024` → `diagnostics::collect_all` → `definition_check::validate_definitions`).

### `references.rs`

Calls from `symbols.rs`:

- `crate::symbols::SymbolTable` (line 54, 99)
- `table.find(name)` (line 61)
- `filter_declaration` reads `Symbol.selection_range`

It also uses `tree_sitter::Tree` directly for `find_identifier_uses` and `identifier_range_at`.

LSP protocols: `textDocument/references` (`server.rs:759`), `textDocument/rename` (`server.rs:914`), `textDocument/prepareRename` (`server.rs:975`).
Symbol path: after finding raw identifier ranges, `references::filter_declaration` removes the declaration by matching against `SymbolTable`.

### Other important consumers not listed in the task

- `typecheck.rs` — uses tree-sitter directly, not `symbols.rs`, but resolves callees via `MergedView`/`SymbolTable`.
- `diagnostics.rs` — uses tree-sitter `ERROR`/`MISSING` and forwards to `definition_check` and `typecheck`.
- `semantic.rs` / `merged_view.rs` / `cache.rs` — indirect users via `build_symbol_table` and `extract_rule_registrations`.

---

## Section 4 — Cargo / workspace wiring

### Current `lsp/Cargo.toml`

```toml
[dependencies]
...
tree-sitter = "0.26.9"
tree-sitter-c = "0.24.2"
tree-sitter-language = "0.1"
tree-sitter-xs = { version = "0.1.0", path = "../tree-sitter-xs" }

# Lelwel deps already present
logos = "0.16"
codespan-reporting = "0.13"

[build-dependencies]
lelwel = "0.10"
```

### What to change

1. Add the typed-AST crate as a normal dependency:

   ```toml
   xs-parser = { version = "0.1.0", path = "../lelwel-xs" }
   ```

   Crate name is currently `xs-parser` (not `xs_language_server_lelwel_xs`).

2. Replace `tree-sitter`, `tree-sitter-c`, `tree-sitter-language`, and `tree-sitter-xs` with `xs-parser`.
3. `lelwel-xs` **does not need a `[lib]` section** — it already exposes `src/lib.rs` with `pub mod ast; pub mod lexer; pub mod parser;`. Cargo infers a default lib target named `xs-parser` from the package name.

### Dependency-chain sketch

```
lsp crate
  ├─ xs-parser (path=../lelwel-xs)
  │    ├─ logos
  │    ├─ codespan-reporting
  │    └─ [build] lelwel
  ├─ tower-lsp-server
  ├─ serde/serde_json
  └─ ... (no tree-sitter)
```

`xs-parser` re-exports (via `ast/mod.rs`):

- `ast::{TranslationUnit, TopLevelItem, FunctionDefinition, ForwardDeclaration, ClassDefinition, RuleDefinition, IncludeDirective, ...}`
- `ast::{Declaration, Declarator, ParameterList, RegularParam, FunctionPointerParam, ...}`
- `parser::{Cst, Node, NodeRef, Rule, Span}`
- `lexer::Token`

### `parser.rs` swap

Turn `parser::parse(source) -> Option<Tree>` into:

```rust
pub fn parse(source: &str) -> Option<(xs_parser::ast::Cst<'_>, Vec<xs_parser::parser::Diagnostic>)>
```

or, for compatibility, keep returning `Option<Cst>` and expose diagnostics separately. Either option forces every call site and test to change.

---

## Section 5 — affected tests

### Test inventory (current `cargo test` baseline)

`cargo test --manifest-path tools/xs-language-server/Cargo.toml` currently reports **187 tests**; many fail locally because they require `AOMR_GAME_PATH` / `doxygen_retail.7z`. The orchestrator's earlier note of **252 tests** likely includes `lelwel-xs`'s 159 tests and/or the `lsp_roundtrip_test` binary.

| Test file | #tests | Functions / areas covered | Rewrite? |
|-----------|--------|---------------------------|----------|
| `lsp/src/parser.rs` | 3 | `parse`, `named_child`, `first_child_by_kind`, function-pointer/lambda parsing | **Yes** — these assert tree-sitter node shapes |
| `lsp/src/symbols.rs` | 18 | `build_symbol_table`, `extract_*`, `extract_rule_registrations`, visibility, params | **Yes** — entire module changes; tests will call typed-AST builder |
| `lsp/src/references.rs` | 6 | `find_identifier_uses`, `filter_declaration`, `identifier_range_at` | **Yes** — need byte-span → Range conversion and AST traversal |
| `lsp/src/definition_check.rs` | 15 | `validate_definitions` | **Yes** — rewrite on typed AST |
| `lsp/src/diagnostics.rs` | 12 | `collect_diagnostics`, `categorize`, message formatting | **Partial** — `collect_diagnostics` moves to lelwel diagnostics; categorization tests stay |
| `lsp/src/typecheck.rs` | 24 | `check_calls_with_merged`, argument count/type checks | **Yes** — walk typed expressions |
| `lsp/src/semantic.rs` | 36 | cross-file resolution, extern collisions, forward declarations, mutable redefinitions | **Partial** — logic stays, but fixture-building (`VirtualProject::from_files`) calls `parser::parse` and `symbols::*` |
| `lsp/src/completion.rs` | 5 | `complete`, `complete_merged`, prefix handling | **No** — only test helper calls `build_symbol_table`; minor signature fix |
| `lsp/src/semantic_tokens.rs` | 4 | `encode`, `extract_symbols`, `classify_origin` | **Partial** — encoding/origin tests stay; walker tests rewrite |
| `lsp/src/merged_view.rs` | 13 | `MergedView::build`, include paste, visibility filtering | **Mostly no** — depends on `parser::parse` + `build_symbol_table` signature |
| `lsp/src/server.rs` | 1 | `build_document_symbol_tree` nesting | **No** — pure `SymbolTable` logic |
| `lsp/src/cache.rs` | 12 | cached symbol table load/save | **No** — signature-level changes only |
| `lsp/tests/game_folder_parse.rs` | 9 | parse every game file, unexpected-error count, diagnostic totals | **Yes** — directly counts `tree_sitter::ERROR` nodes |
| `lsp/tests/symbols_cleanup_repro.rs` | 6 | ERROR-node fixtures, `extract_error_*` helpers, test naming | **Yes** — these lock tree-sitter behavior; delete/replace |
| `lsp/tests/forward_decl_repro.rs` | 6 | forward-decl diagnostics across includes | **Partial** — likely still valid but fixtures built with new parser |
| `lsp/tests/include_goto_definition_repro.rs` | 5 | include-directive goto definition | **Partial** — `parser::detect_include_path_at_position` changes |
| `lsp/tests/semantic_tokens_repro.rs` | 6 | semantic-token modifiers | **Partial** — may pass after walker rewrite |
| `lsp/tests/class_*_repro.rs` | 18 | class / member extraction and semantic tokens | **Partial** — extraction path changes but assertions stay |
| `lsp/tests/r3_f01_deadlock_repro.rs` | 0 | placeholder | — |
| `lsp/tests/r5_test_honesty_repro.rs` | 5 | LSP round-trip completion/references/diagnostics | **No** — black-box message checks |

### Estimated tests to add

- **New symbol-builder tests** for typed-AST path: ~10–15 tests mirroring the existing `symbols.rs` suite (extern, const, function params, forward decls, class members, function-pointer params).
- **New span-to-range tests** because `Cst` uses byte spans, not line/column.
- **New parser-diagnostic tests** replacing `ERROR`/`MISSING` node counts with lelwel diagnostic assertions.
- **New expression-walker tests** for `find_identifier_uses` and rule-registration extraction over typed `Expr`.
- **New include-directive tests** for `parser::extract_include_directives` / `detect_include_path_at_position` using `IncludeDirective`.

Estimated total new tests: **20–30**.

---

## Section 6 — risks / gotchas

### `lelwel-xs/` is gitignored

- `.gitignore:33` ignores `tools/xs-language-server/lelwel-xs/`.
- It **is already a workspace member** (`tools/xs-language-server/Cargo.toml` line 3), so `cargo build` / `cargo test` work.
- For Phase 4 to ship, the crate must stop being scratch. Two options:
  1. **Unignore it** where it lives; rename crate/package from `xs-parser` to `xs-language-server-ast` if desired.
  2. **Move it to a tracked directory** (e.g., `tools/xs-language-server/xs-parser/`) and update the workspace path.
- Recommendation: **option 2**, because the current path name `lelwel-xs` signals "exploratory" and the crate is now the canonical AST.

### `tree-sitter-xs` still exists

- `tools/xs-language-server/tree-sitter-xs/` has the old grammar + generated Rust bindings.
- No `lsp/src/` generated bindings are checked in; the dependency is pulled in via `tree-sitter-xs` package.
- Dropping the `tree-sitter` dependency means you can also delete or archive `tree-sitter-xs/` unless other tooling still uses it.

### Parser-diagnostic model change

- tree-sitter produces `ERROR` / `MISSING` nodes you walk manually.
- lelwel returns a `Vec<Diagnostic>` from `Parser::new(...).parse(...)`.
- `diagnostics.rs::collect_diagnostics` and `game_folder_parse.rs` will switch from node-counting to diagnostic-counting. This is a semantic change: thresholds like "≤ 3000 unexpected errors" will need recalibration or replacement.

### Include directives

- Current `parser::extract_include_directives` returns `(String, Range)` where `String` is the unquoted path.
- Typed AST returns `IncludeDirective { path: Spanned<String> }` with the raw quoted string. Consumers need to `strip_prefix('"').strip_suffix('"')`.
- `detect_include_path_at_position` will be much simpler: find the `IncludeDirective` whose `path.span` contains the cursor byte offset.

### Rules are not fully modeled

- `RuleDefinition::from_cst` returns `None` for real rule bodies (non-empty) because the PoC grammar emits ERROR nodes for them.
- `symbols.rs` currently recovers rule names from tree-sitter ERROR nodes. The new AST must either:
  - add a rule-recovery helper that walks the CST when `RuleDefinition` fails, or
  - fix the grammar to model rules cleanly before Phase 4.
- Risk: if rules are left unmodeled, `SymbolKind::Rule` symbols disappear, breaking outline, hover, and semantic tokens for rules.

### Byte spans vs line/column

- `tree_sitter::Node` gives `(row, column)` directly.
- `lelwel` only gives `Span = Range<usize>` (byte offsets).
- Every module that maps a node to an LSP `Range` needs a new `span_to_range(source, span)` utility. This is a mechanical but wide-reaching change.

### Function-pointers and lambdas

- The typed AST stores lambda defaults as `UnparsedExpr`; `expr()` re-extracts a typed `Expr`.
- `symbols.rs` detail strings (e.g., `void(int) cb = [](int id = -1) {}`) can be rendered from `FunctionPointerParam` + a formatter over `Expr`.
- If detail formatting is required, reuse the `format.rs` formatter from `lelwel-xs` or write a small one for signatures.

---

## Section 7 — recommended scope slicing

Based on T16–T19:

| Task | Scope | Recommended slice |
|------|-------|-------------------|
| **T16** — rewrite `symbols.rs` to use `TranslationUnit::from_cst` | Rebuilds the core `SymbolTable` + `build_symbol_table` / `build_full_symbol_table` / `extract_rule_registrations`. Touches `cache.rs`, `merged_view.rs`, `semantic.rs`, `server.rs`, `completion.rs` only through signatures. | **Own PR (Slice 1)**. It is the foundation for every other handler. Keep tests green for symbol extraction first. |
| **T17** — delete `extract_error_function_definition` | Mechanical cleanup after T16, but also removes the tree-sitter ERROR-node dispatch for forward decls. | **Merge into Slice 1** or a tiny follow-up PR immediately after T16. Not a separate chain by itself. |
| **T18** — wire hover / completion / definition / rename handlers to typed AST | `semantic_tokens.rs`, `references.rs`, `definition_check.rs`, `typecheck.rs`, `diagnostics.rs` all need rewrites. | **Slice 2 (AST walks)** — convert the four tree-sitter walkers (`semantic_tokens`, `references`, `typecheck`, `definition_check`) in one focused PR. `diagnostics.rs` can ride along because it only routes to the others and handles parse diagnostics. |
| **T19** — drop `tree_sitter` dependency | Update `lsp/Cargo.toml`, delete/update `parser.rs`, and decide fate of `tree-sitter-xs/` and debug bins (`day1_probe.rs`, `dump_top_level.rs`, `inspect_tree.rs`). | **Final slice (Slice 3)**. Only after no source file imports `tree_sitter`. This is also the right place to unignore / move `lelwel-xs`. |

### Suggested ordering

1. **PR-A: `symbols.rs` reimplementation + T17 cleanup**
   - Core `SymbolTable` from typed AST.
   - Update all `build_symbol_table` call sites to pass a `Cst`.
   - Delete `extract_error_*` and forward-decl hacks.
   - Add new symbol-builder unit tests.

2. **PR-B: AST walker handlers**
   - `semantic_tokens.rs`, `references.rs`, `typecheck.rs`, `definition_check.rs`, `diagnostics.rs`.
   - Add `span_to_range` helper.
   - Update `game_folder_parse.rs` and `symbols_cleanup_repro.rs` expectations.

3. **PR-C: remove tree-sitter dependency and promote `lelwel-xs`**
   - Drop deps; rewrite `parser.rs` as thin wrapper around `xs_parser::Parser`.
   - Delete or rewrite debug binaries.
   - Decide `lelwel-xs` → `xs-parser` rename/move and unignore.

### PR-size rationale

- PR-A alone is ~1,000 lines of changed code (full `symbols.rs` + call site signature updates). That is already large but is one coherent rewrite.
- PR-B is the highest-risk piece because it touches LSP user-facing behavior. Keeping it separate lets reviewers focus on semantic-token / reference correctness.
- PR-C is cleanup-only; if any debug tool is still useful, port it to the typed AST in the same PR before removal.

---

## Next step for the orchestrator

This map is ready for proposal/spec writing. The open decisions to surface to the user are:

1. Do we rename `lelwel-xs` → `xs-parser` and move it to a tracked directory now?
2. Should rule-definition extraction wait for a grammar fix, or do we add a CST-level rule-recovery fallback in this phase?
3. What diagnostic thresholds replace the `ERROR`-node counts in `game_folder_parse.rs`?
