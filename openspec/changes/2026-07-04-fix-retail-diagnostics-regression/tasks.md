# Tasks: Fix Retail-XS-File Diagnostic Regression

## Phase 1: Comment lexer regexes
- [x] Add `LineComment` regex for `//...` in `lexer.rs`.
- [x] Add `BlockComment` regex for `/* ... */` in `lexer.rs`.
- [x] Keep tokens in the grammar `skip` rule so CST formatter can round-trip comments.
- [x] Preserve newlines inside block comments for correct line offsets.

## Phase 2: `else` branch in `if_statement`
- [x] Extend `statement^` to accept `PreprocElse statement @else_statement`.
- [x] Recover typed `Statement::If` (with optional `else_`) from bare-token CST shape.
- [x] Update `IfStatement::from_wrapper` else extraction.
- [x] Add unit tests for if/without-else, if/with-else, and if/else-if chains.

## Phase 3: `for (int i = 0; ...)` declaration form
- [x] Extend `for_init` at `xs.llw:550` to accept `declaration_specifiers init_declarator_list`.
- [x] Wire `ParserCallbacks` / `TypeTable` predicate for LL(1) disambiguation.
- [x] Add unit tests for declaration-form `for` loops.
- [x] Add LSP integration test asserting zero ERROR parse diagnostics for `for (int i = 0; ...)`.

## Phase 4: Bitwise `&` and `|` tokens
- [x] Add `Amp='&'` and `Pipe='|'` tokens to `xs.llw` and `lexer.rs`.
- [x] Add `bitwise_expr` precedence level in `binary_expr` grammar.
- [x] Map bitwise operators in `BinaryOp` and add unit tests.

## Phase 5: Seed workspace class names into `TypeTable`
- [ ] Update `lsp/src/diagnostics.rs` to pass engine/class names to `TypeTable::with_primitives()`.
- [ ] Verify `BOSystem myBO = ...;` style declarations resolve.

## Phase 6: Function-pointer-typed variables
- [ ] Add grammar support for `void(int) foo = ...;` declarations.
- [ ] Add targeted unit tests for the 12 retail occurrences.

## Phase 7: Downgrade honest limitations to WARNING
- [ ] Change remaining unsupported-construct diagnostics from `ERROR` to `WARNING`.
- [ ] Update `game_folder_parse` thresholds and assertions.
