# Definition-Time Validation Specification

> **Added by change:** `lsp-server-semantic-fixes` (follow-up commit `1eca402b` + corrections)

## Capability summary

The LSP server SHALL validate definition-time constraints that the XS compiler enforces at parse time:

1. **Non-`ref` parameter without a default value** → error
2. **`ref` parameter with a default value** → error
3. **`const` variable assigned a non-constant expression** → error

These checks mirror the compiler so the LSP can flag errors before the file is loaded by the engine. Diagnostics are categorized as `DiagnosticCategory::DefinitionError`.

## Rationale

The XS compiler enforces several rules at definition time that the LSP previously did not check. Without these checks, invalid declarations (such as `void f(int x) {}` or `ref int y = 0`) silently reached the engine, producing runtime errors or undefined behavior that took hours of debugging to trace.

## Scenarios

### Scenario: error — non-ref parameter without default value

- GIVEN a function declaration `void f(int x) {}` where `x` is non-`ref` and has no `= value` clause
- WHEN the LSP lints the file
- THEN a diagnostic SHALL be emitted: `non-ref parameter `x` must have a default value`

### Scenario: error — ref parameter with default value

- GIVEN a function declaration `void f(ref int x = 0) {}` where `x` is `ref` and has an `= value` clause
- WHEN the LSP lints the file
- THEN a diagnostic SHALL be emitted: `ref parameter `x` cannot have a default value`

### Scenario: error — constant assigned non-constant expression

- GIVEN a declaration `const int x = aiEcho("hi");` where the RHS is a function call
- WHEN the LSP lints the file
- THEN a diagnostic SHALL be emitted: `constant `x` must be assigned a constant expression`

### Scenario: happy path — constant assigned literal

- GIVEN `const int x = 42;`
- WHEN the LSP lints the file
- THEN no diagnostic SHALL be emitted

### Scenario: happy path — constant assigned other constant

- GIVEN `const int cOne = 1; const int cTwo = cOne;`
- WHEN the LSP lints the file
- THEN no diagnostic SHALL be emitted

### Scenario: happy path — constant assigned arithmetic over constants

- GIVEN `const int cOne = 1; const int cTwo = cOne + 1;`
- WHEN the LSP lints the file
- THEN no diagnostic SHALL be emitted

### Scenario: happy path — constant assigned vector constructor

- GIVEN `const vector v = vector(1.0, 2.0, 3.0);`
- WHEN the LSP lints the file
- THEN no diagnostic SHALL be emitted

`vector(...)` is a built-in type constructor. The XS engine treats it as a literal value, not a function call result. All other call expressions (e.g. `aiEcho(...)`, `xsVectorSet(...)`) remain rejected.

## XS-engine constraints

- **Constant-expression rule**: A constant RHS must be one of:
  - A literal (number, string, `true`, `false`)
  - An identifier (presumably a constant reference)
  - A unary/parenthesized expression over other constant expressions
  - A binary expression where both operands are constant expressions
  - A `vector(...)` constructor call
- **Ref/default rule**: A parameter is either non-`ref` (must have a default) or `ref` (cannot have a default). Mixed cases are compiler errors.

## Out of scope (intentional gaps)

- **Top-level uninitialized variable declarations** such as `AttackWave gFoo;` are NOT flagged. The XS compiler requires initialization only for **scalar** types; class/struct instances are allowed to be declared without an initializer because they are constructed lazily via method calls. The LSP does not have reliable type information to distinguish scalars from classes, so this check is deferred to the compiler itself. (See `definition_check::tests::flags_uninitialized_top_level_variable` — the test asserts that no diagnostic is emitted, documenting the intentional gap.)

## Verification approach

- Unit tests in `tools/xs-language-server/src/definition_check.rs`:
  - `flags_non_ref_param_without_default`
  - `flags_ref_param_with_default`
  - `flags_constant_assigned_non_constant`
  - `allows_constant_assigned_literal`
  - `allows_constant_assigned_other_constant`
  - `allows_constant_assigned_arithmetic_over_constants`
  - `allows_constant_assigned_vector_constructor`
  - `rejects_constant_assigned_other_call`
  - `flags_uninitialized_top_level_variable` (asserts no diagnostic — documents the gap)
- Integration test `game_folder_parse.rs::test_game_folder_total_diagnostic_count_is_zero`:
  - Reports 0 `definition_error` diagnostics across the full official `game/**/*.xs` tree

## Acceptance criteria

1. The validator SHALL emit a diagnostic for every non-`ref` parameter that lacks a default value.
2. The validator SHALL emit a diagnostic for every `ref` parameter that has a default value.
3. The validator SHALL emit a diagnostic for every `const` declaration whose RHS is not a constant expression per the rule above.
4. The validator SHALL NOT emit a diagnostic for `vector(...)` calls on the RHS of `const` declarations.
5. The validator SHALL NOT emit a diagnostic for binary expressions on the RHS of `const` declarations when both operands are constant expressions.
6. The validator SHALL NOT emit a diagnostic for uninitialized top-level variable declarations.