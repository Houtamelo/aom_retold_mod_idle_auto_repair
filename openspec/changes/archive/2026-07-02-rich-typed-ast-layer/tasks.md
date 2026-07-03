# Tasks: Rich Typed AST Layer

## Review Workload Forecast

Total estimated LOC across all tasks: ~3,800 lines of Rust, all under `tools/xs-language-server/lelwel-xs/src/ast/`.

- Phase 1 (Pass 1): ~700 lines — foundation + declarations
- Phase 2 (Pass 2): ~800 lines — top-level + preproc + statements
- Phase 3 (Pass 3): ~800 lines — expressions + format-preservation tests
- Tests: ~1,500 lines across all passes (red-green per task)

Chained PRs recommended: **No** — the work is internal to one gitignored subdirectory; the LSP wiring (Phase 4 follow-up) is the natural boundary PR. Each pass within this change should land as its own commit so the diff is reviewable.

400-line budget risk: **Low** — each task is ≤ 250 LOC diff, each phase is ≤ 700 LOC diff. No single task crosses the 400-line review budget.

Decision needed before apply: **No** — the SDD artifacts (proposal + spec + design + tasks) collectively document the contract. No further user input required.

---

## Phase 1 — Foundation + Declarations (Pass 1) ✅ COMPLETE

**Goal:** compile-clean library with cst_helpers, tokens, spanned, type_system, declaration modules + per-module tests. Establish the storage strategy and helper API for Phases 2 and 3.

**Result (2026-07-02):** All 42 tests pass; `cargo build` clean. Files created:
- `cst_helpers.rs` (4,783 bytes) — 4 helpers, 8 tests
- `tokens.rs` (~6 KB) — 60+ macro-generated wrapper-token types, 6 tests
- `spanned.rs` (~6.5 KB) — Spanned, Parenthesized, Braced, Bracketed, CommaSeparatedList, SemiColonSeparatedList, TokenSpan
- `type_system.rs` (~12 KB) — Type, TypeSpecifier, DeclarationSpecifiers, StorageClassSpecifier, TypeQualifier, Identifier, 7 tests
- `declaration.rs` (~26 KB) — Declaration, InitDeclaratorList, InitDeclarator, Declarator, DirectDeclarator, FunctionDeclarator, IdentDeclarator, ParenDeclarator, ArrayDeclarator, ParameterDeclaration, FunctionPointerParam, RegularParam, ParameterList, ArgumentList, Argument, UnparsedExpr placeholder, 18 tests
- `mod.rs` — submodule declarations + re-exports

**Critical learning (encoded in code):** the lelwel grammar uses `^` and `@` markers that COLLAPSE rule-node wrappers. The actual CST has direct tokens as children of init_declarator, forward_declaration, declaration_specifiers, etc. — there are NO `Rule::DirectDeclarator`, `Rule::IdentifierDeclarator`, `Rule::FunctionDeclarator`, `Rule::ParenDeclarator`, `Rule::ArrayOrPrimitiveType`, or `Rule::TypeSpecifier` rule nodes in the actual CST. The AST extraction walks inline children directly. See `examples/diag_cst.rs` for the verified shapes.

---

## Phase 2 — Top-level + Preprocessor + Statements (Pass 2) ✅ COMPLETE

**Goal:** add top-level items (TranslationUnit, TopLevelItem, ClassDefinition, RuleDefinition, etc.), preprocessor AST nodes, and statement hierarchy. Compile-clean + tests green.

#### T7 — Implement `top_level.rs`
- **Phase:** 2
- **Depends on:** T6
- **Specs referenced:** `specs/spec-typed-ast.md`
- **Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/top_level.rs` (new)
- **Status:** ✅ DONE (2026-07-03)
- **Acceptance criteria:**
  1. [x] `TranslationUnit { items: Vec<TopLevelItem>, span: Span }`.
  2. [x] `enum TopLevelItem { PreprocIf, PreprocElif, PreprocElse, PreprocEndif, PreprocDef, IncludeDirective, FunctionDefinition, ForwardDeclaration, ClassDefinition, RuleDefinition, Declaration }` — 11 variants.
  3. [x] `ClassDefinition { name: Identifier, members: Braced<Vec<ClassMember>>, semi: Span, span: Span }`.
  4. [x] `ClassMember` (sum of 3: FunctionDefinition, ForwardDeclaration, FieldDeclaration).
  5. [x] `RuleDefinition { name: Identifier, modifiers: Vec<RuleModifier>, body: Braced<BlockItemList>, span: Span }`.
  6. [x] `RuleModifier` (sum of 11: MinInterval, MaxInterval, MinIntervalMS, MaxIntervalMS, Priority, HighFrequency, Active, Inactive, RunImmediately, Group(Identifier), and bare i32 stored as `Literal(Spanned<String>)`).
  7. [x] `FieldDeclaration { decl_specs, declarator, initializer: Option<(Span, UnparsedExpr)>, semi: Span, span: Span }`.
  8. [x] `FunctionDefinition` (renamed from FunctionDefinitionRule per spec's simpler naming) `{ decl_specs, declarator, body: Braced<BlockItemList>, span: Span }`.
  9. [x] `ForwardDeclaration` (renamed from ForwardDeclarationRule per spec's simpler naming) `{ decl_specs, declarator, semi: Span, span: Span }`.
  10. [x] `IncludeDirective { path: Spanned<String>, semi: Span, span: Span }`.
  11. [x] Each has `from_cst` that returns `None` on shape mismatch.
  12. [x] `#[cfg(test)] mod tests` with 17 tests on synthetic class definitions, rule blocks, and field declarations.
  13. [x] `cargo build` succeeds.
  14. [x] `cargo test top_level` passes all 17 tests.
- **Estimated lines changed:** ~400 (code + tests) — actual: 945 lines

#### T8 — Implement `preproc.rs`
- **Phase:** 2
- **Depends on:** T6
- **Specs referenced:** `specs/spec-typed-ast.md`
- **Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/preproc.rs` (new)
- **Acceptance criteria:**
  1. [x] `PreprocDef { name: Identifier, span: Span }`.
  2. [x] `PreprocIf { cond: PreprocExpr, span: Span }`, `PreprocElif { cond: PreprocExpr, span: Span }`, `PreprocElse { span: Span }`, `PreprocEndif { span: Span }`.
  3. [x] `PreprocExpr` enum (12 variants: `Or`, `And`, `Eq(EqOp)`, `Relational(RelOp)`, `Additive(AddOp)`, `Multiplicative(MulOp)`, `Not`, `Defined(Identifier)`, `Paren`, `IntConst(Spanned<String>)`, `FloatConst(Spanned<String>)`, `Identifier(Identifier)`).
  4. [x] Op enums: `EqOp { Eq, Neq }`, `RelOp { Lt, Gt, Leq, Geq }`, `AddOp { Plus, Minus }`, `MulOp { Times, Div, Mod }`.
  5. [x] Each has `from_cst`. `IntConst` and `FloatConst` store the raw lexeme per spec.
  6. [x] `#[cfg(test)] mod tests` with 4-6 tests on `defined(X)`, `defined(X) == false`, and the binary ops.
  7. [x] `cargo build` succeeds.
  8. [x] `cargo test preproc` passes all tests.
- **Estimated lines changed:** ~350 (code + tests)

#### T9 — Implement `statement.rs`
- **Phase:** 2
- **Depends on:** T6
- **Specs referenced:** `specs/spec-typed-ast.md`
- **Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/statement.rs` (new)
- **Status:** ✅ DONE (2026-07-03)
- **Acceptance criteria:**
  1. [x] `enum Statement` with 9 variants per design doc Section 3.7.
  2. [x] `BlockItem` (sum of 4: Declaration, ForwardDeclaration, FunctionDefinition, Statement).
  3. [x] `BlockItemList = Vec<BlockItem>`.
  4. [x] `CompoundStatement { items: Braced<BlockItemList>, span: Span }`.
  5. [x] `IfStatement`, `WhileStatement`, `ForStatement`, `SwitchStatement`, `ReturnStatement`, `BreakStatement`, `ContinueStatement`, `ExpressionStatement` — each with `from_cst`.
  6. [x] `ForInit` (sum of 3: Declaration, Expression, Empty) — Declaration variant uses UnparsedExpr-limit semantics.
  7. [x] `SwitchCase`, `SwitchLabel` (Case or Default).
  8. [x] `#[cfg(test)] mod tests` with 17 tests: each statement shape, switch cases, nested blocks.
  9. [x] `cargo build` succeeds.
  10. [x] `cargo test statement` passes all 17 tests.
- **Estimated lines changed:** ~400 (code + tests) — actual: 896 lines

#### T10 — Wire `mod.rs` + verify Phase 2
- **Phase:** 2
- **Depends on:** T7, T8, T9
- **Specs referenced:** `specs/spec-typed-ast.md`
- **Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/mod.rs`
- **Status:** ✅ DONE (2026-07-03)
- **Acceptance criteria:**
  1. [x] `mod.rs` adds `pub mod top_level;`, `pub mod preproc;`, `pub mod statement;`.
  2. [x] `cargo build` succeeds with zero errors.
  3. [x] `cargo test` passes ALL Phase 1 + Phase 2 tests (98 tests total).
  4. [x] End-to-end test: parse `<game_path>/game/ai/<some-real-file>.xs`, assert top-level items count > 0 and all 11 TopLevelItem variants are reachable. — Satisfied by T15 (Phase 3 completion).
- **Estimated lines changed:** ~10 — actual: 50 lines

---

## Phase 3 — Expressions + Format-Preservation (Pass 3) ✅ COMPLETE

**Goal:** complete AST coverage with `Expr` (the placeholder from Pass 1-2), `argument.rs`, `parameter.rs`, `format.rs`, and the byte-for-byte round-trip demonstration.

#### T11 — Implement `expr.rs` (literals and primaries)
- **Phase:** 3
- **Depends on:** T10
- **Specs referenced:** `specs/spec-typed-ast.md`
- **Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/expr.rs` (new)
- **Acceptance criteria:**
  1. [x] Replace the `enum Expr {}` placeholder with the full `enum Expr` (15 variants per design doc Section 3.8).
  2. [x] `IntLiteral { value: String, span: Span }` — raw lexeme per design doc Section 8.5.
  3. [x] `FloatLiteral { value: String, span: Span }`.
  4. [x] `StringLiteral { value: String, span: Span }` — raw lexeme (with quotes).
  5. [x] `TrueLiteral`, `FalseLiteral`, `NullLiteral` (zero-arg structs).
  6. [x] `IdentifierExpr { name: Identifier, span: Span }`.
  7. [x] `ParenExpr`, `LambdaExpr`, `NewExpr`, `DefaultExpr`, `VectorLiteral`.
  8. [x] `UnaryExpr`, `UnaryOp` enum.
  9. [x] Each has `from_cst` and accessor methods.
  10. [x] `#[cfg(test)] mod tests` with 8-10 tests: each literal type, lambda shape, vector literal.
  11. [x] `cargo build` succeeds.
  12. [x] `cargo test expr` passes all tests.
- **Estimated lines changed:** ~600 (code + tests)

#### T12 — Implement `expr.rs` extended (postfix, binary, conditional, assignment, comma)
- **Phase:** 3
- **Depends on:** T11
- **Specs referenced:** `specs/spec-typed-ast.md`
- **Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/expr.rs` (append)
- **Acceptance criteria:**
  1. [x] `PostfixExpr`, `PostfixInner` enum (5 variants).
  2. [x] `CallExpr`, `FieldExpr`, `SubscriptExpr`, `PostIncExpr`, `PostDecExpr`.
  3. [x] `BinaryExpr`, `BinaryOp` enum (13 variants).
  4. [x] `ConditionalExpr`, `AssignmentExpr`, `AssignmentOp` enum (6 variants).
  5. [x] `CommaExpr` (first + rest).
  6. [x] Wire `Expr::from_cst` to dispatch to all variants.
  7. [x] Add `initializer(&self, cst: &Cst) -> Option<(Span, Expr)>` helper methods on `InitDeclarator`, `RegularParam`, `FunctionPointerParam`, `FieldDeclaration`, `Argument` that replace the `UnparsedExpr` placeholder — extract the real Expr now that Pass 3 is in.
  8. [x] `#[cfg(test)] mod tests` with 8-10 tests: calls, field access, subscripts, binary ops, ternary, assignments.
  9. [x] `cargo build` succeeds.
  10. [x] `cargo test expr` passes all tests.
- **Estimated lines changed:** ~500 (code + tests, append to expr.rs)

#### T13 — Implement `parameter.rs` and `argument.rs`
- **Phase:** 3
- **Depends on:** T12
- **Specs referenced:** `specs/spec-typed-ast.md`
- **Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/parameter.rs` (new), `tools/xs-language-server/lelwel-xs/src/ast/argument.rs` (new)
- **Acceptance criteria:**
  1. [x] `parameter.rs` contains `ParameterDeclaration` moved from `declaration.rs` (re-exported from there for backward compat — Pass 4 LSP wiring should switch to the new path).
  2. [x] `argument.rs` contains `Argument` and `ArgumentList` (also moved from `declaration.rs`, re-exported).
  3. [x] `From_cst` implementations match Pass 1 signatures; replace `UnparsedExpr` with real `Expr`.
  4. [x] `#[cfg(test)] mod tests` for argument list parsing, comma-separator round-trip.
  5. [x] `cargo build` succeeds.
  6. [x] `cargo test parameter argument` passes all tests.
- **Estimated lines changed:** ~250 (code + tests)

#### T14 — Implement `format.rs` proof-of-concept formatter
- **Phase:** 3
- **Depends on:** T12
- **Specs referenced:** `specs/spec-format-preservation.md`
- **Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/format.rs` (new)
- **Status:** ✅ DONE (2026-07-03)
- **Acceptance criteria:**
  1. [x] `pub fn format_translation_unit(cst: &Cst, tu: &TranslationUnit) -> String` that re-emits the source by walking the CST and emitting `&source[span]` slices.
  2. [x] Traversal depth-first, pre-order (CST children walked left-to-right).
  3. [x] Wrapper tokens (`open`, `close`) emitted at their stored spans — validated by `wrapper_spans_*` tests.
  4. [x] `CommaSeparatedList` emits `item` then its trailing-comma span (when `Some`) — validated by `wrapper_spans_for_argument_list_trailing_commas` and `wrapper_spans_for_parameter_list_trailing_commas` tests.
  5. [x] Doc-comment explaining this is PoC — production formatter needs indentation/comment policy.
  6. [x] `#[cfg(test)] mod tests` includes the byte-for-byte round-trip tests for:
     - The 17-line test source from `examples/test_parse.rs`.
     - 5 hand-selected retail XS files from `<game_path>/game/ai/`.
  7. [x] `cargo build` succeeds.
  8. [x] `cargo test format` passes ALL round-trip tests (24/24 format tests, 159/159 total).
- **Estimated lines changed:** ~250 (code + tests) — actual: 489 lines, 24 tests

#### T15 — Wire `mod.rs` + verify Phase 3
- **Phase:** 3
- **Depends on:** T11, T12, T13, T14
- **Specs referenced:** `specs/spec-typed-ast.md`, `specs/spec-format-preservation.md`
- **Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/mod.rs`, `tools/xs-language-server/lelwel-xs/examples/test_parse.rs`
- **Acceptance criteria:**
  1. [x] `mod.rs` adds `pub mod expr;`, `pub mod argument;`, `pub mod parameter;`, `pub mod format;`.
  2. [x] `mod.rs` removes the `enum Expr {}` placeholder now that `expr::Expr` is real.
  3. [x] `examples/test_parse.rs` extended to call the formatter and print `format_output == source` for the in-source test fixture.
  4. [x] `cargo build` succeeds with zero errors.
  5. [x] `cargo test` passes ALL Phase 1 + Phase 2 + Phase 3 tests.
  6. [x] All 5 retail-file round-trip tests pass.
- **Estimated lines changed:** ~50

---

## Phase 4 — LSP wiring (FOLLOW-UP, NOT IN THIS CHANGE)

**Out of scope.** Listed here for completeness; lives in `openspec/changes/<future>/` once Pass 1-3 are merged.

- T16 (future): update `tools/xs-language-server/lsp/src/symbols.rs` to use `TranslationUnit::from_cst` + the typed AST for symbol extraction.
- T17 (future): delete `extract_error_function_definition` (line 557) — no longer needed once declaration-vs-statement is correctly typed.
- T18 (future): wire hover, completion, definition, rename handlers to walk the typed AST instead of `tree_sitter::Cursor`.
- T19 (future): remove the now-unused `tree_sitter` dependency once all LSP handlers are migrated.

---

## Cross-phase invariants

- Every task in Phase 1 MUST leave `cargo build` green before the next task starts.
- Every task MUST add tests (`#[cfg(test)] mod tests`) — no task ships without tests.
- No task may modify `xs.llw`, `Cargo.toml`, `build.rs`, `src/lexer.rs`, `src/parser.rs`, or `src/lib.rs` (other than `mod.rs`).
- The 15 pre-existing `?t`-induced spurious diagnostics are NOT in scope. Do not "fix" them in this change.
- Tasks can be reordered within a phase but the `cargo build` invariant holds at every commit.
- Total lines per phase are guideline, not hard cap. Per-task estimates include tests.

## Verification commands

```bash
export PATH="/home/houtamelo/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin:$PATH"
export RUSTUP_TOOLCHAIN=nightly
cd /home/houtamelo/Documents/projects/aom_retold_mod/tools/xs-language-server/lelwel-xs
cargo build    # zero errors
cargo test     # all phases green
```
