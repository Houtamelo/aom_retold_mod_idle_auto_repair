# Definition-Time Validation Specification

> **Added by change:** `lsp-server-semantic-fixes` (follow-up commits `1eca402b`, `74a9033`, and the type-aware correction in this spec's sibling commit)

## Capability summary

The LSP server SHALL validate definition-time constraints that the XS compiler enforces at parse time:

1. **Non-`ref` parameter without a default value** → error
2. **`ref` parameter with a default value** → error
3. **Top-level variable of scalar type without an initializer** → error
4. **`const` variable assigned a non-constant expression** → error

These checks mirror the compiler so the LSP can flag errors before the file is loaded by the engine. Diagnostics are categorized as `DiagnosticCategory::DefinitionError`.

## Rationale

The XS compiler enforces several rules at definition time that the LSP previously did not check. Without these checks, invalid declarations silently reached the engine, producing runtime errors or undefined behavior that took hours of debugging to trace.

## XS-engine constraints (uniform rule)

- **Ref/default rule**: A parameter is either non-`ref` (the compiler enforces a default value at definition time — even when source omits `= value`) or `ref` (cannot have a default — the compiler rejects). Mixed cases are compiler errors.
- **Scalar vs class rule** for uninitialized top-level declarations: the compiler requires initialization **only for scalar types** (`bool`, `int`, `float`, `string`, `vector`). Class/struct instances are accepted without an initializer because the compiler-defaulted field values are sufficient. (Example: `AttackWave gFoo;` is valid; `int x;` is not.)
- **Constant-expression rule**: A constant RHS must be one of:
  - A literal (number, string, `true`, `false`)
  - An identifier (presumably a constant reference)
  - A unary/parenthesized expression over other constant expressions
  - A binary expression where both operands are constant expressions
  - A `vector(...)` constructor call (the engine treats `vector(...)` as a literal, not a function call)

## Scenarios

### Scenario: error — non-ref parameter without default value

- GIVEN a function declaration `void f(int x) {}` where `x` is non-`ref` and has no `= value` clause
- WHEN the LSP lints the file
- THEN a diagnostic SHALL be emitted: `non-ref parameter `x` must have a default value`

### Scenario: error — ref parameter with default value

- GIVEN a function declaration `void f(ref int x = 0) {}` where `x` is `ref` and has an `= value` clause
- WHEN the LSP lints the file
- THEN a diagnostic SHALL be emitted: `ref parameter `x` cannot have a default value`

### Scenario: error — uninitialized scalar top-level variable

- GIVEN a declaration `int x;` (no initializer) at file scope
- WHEN the LSP lints the file
- THEN a diagnostic SHALL be emitted: `variable `x` of scalar type must be initialized`

The same rule applies to `float`, `bool`, `string`, and `vector`. Arrays of primitive element types (`int[]`, `float[]`, etc.) are also scalar for this purpose.

### Scenario: happy path — uninitialized class top-level variable

- GIVEN a declaration `AttackWave gFoo;` (no initializer) at file scope
- WHEN the LSP lints the file
- THEN no diagnostic SHALL be emitted

The class/struct is accepted with default field values; subsequent setter calls (`gFoo.setName(...)`) configure it.

### Scenario: happy path — `extern` variable without initializer

- GIVEN a declaration `extern int gFoo;` at file scope
- WHEN the LSP lints the file
- THEN no diagnostic SHALL be emitted

`extern` declarations name a definition elsewhere and are exempted regardless of type.

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

## Out of scope (deliberate)

- Type checking of `ref` parameters separately from non-`ref` parameters at the call site (e.g. detecting that an rvalue was passed where a `ref` is expected). The compiler enforces this; the LSP currently only enforces the **count** of ref args, not their lvalueness.
- Detecting class types beyond the grammar's primitive set. The scalar set (`bool | int | float | string | vector`) is closed; anything else is treated as a class.

## Verification approach

- Unit tests in `tools/xs-language-server/src/definition_check.rs`:
  - `flags_non_ref_param_without_default`
  - `flags_ref_param_with_default`
  - `flags_uninitialized_top_level_variable` (`int x;` → error)
  - `flags_uninitialized_float_variable` / `_bool_variable` / `_string_variable` / `_vector_variable` (each scalar type → error)
  - `allows_uninitialized_class_variable` (`AttackWave gFoo;` → OK)
  - `allows_uninitialized_extern_variable` (`extern int gFoo;` → OK)
  - `flags_constant_assigned_non_constant`
  - `allows_constant_assigned_literal`
  - `allows_constant_assigned_other_constant`
  - `allows_constant_assigned_arithmetic_over_constants`
  - `allows_constant_assigned_vector_constructor`
  - `rejects_constant_assigned_other_call`
- Unit tests in `tools/xs-language-server/src/typecheck.rs` (call-site ref rule):
  - `ref_param_missing_at_call_site_is_an_error`
  - `ref_param_provided_at_call_site_is_ok`
  - `no_ref_params_means_any_call_count_above_ref_required_is_ok`
  - `ref_param_must_appear_before_non_ref_params_with_defaults`
- Integration test `game_folder_parse.rs::test_game_folder_total_diagnostic_count_is_zero`:
  - Reports 0 `definition_error` diagnostics across the full official `game/**/*.xs` tree (verifies the class exemption for the 68 `AttackWave`/`MigrationStrategyData` patterns in campaign files)

## Acceptance criteria

1. The validator SHALL emit a diagnostic for every non-`ref` parameter that lacks a default value.
2. The validator SHALL emit a diagnostic for every `ref` parameter that has a default value.
3. The validator SHALL emit a diagnostic for every uninitialized top-level variable whose type is in the scalar set (`bool`, `int`, `float`, `string`, `vector`, or array of those).
4. The validator SHALL NOT emit a diagnostic for uninitialized top-level variables of any other (class) type.
5. The validator SHALL NOT emit a diagnostic for `extern` declarations without an initializer.
6. The validator SHALL emit a diagnostic for every `const` declaration whose RHS is not a constant expression per the rule above.
7. The validator SHALL NOT emit a diagnostic for `vector(...)` calls on the RHS of `const` declarations.
8. The validator SHALL NOT emit a diagnostic for binary expressions on the RHS of `const` declarations when both operands are constant expressions.
9. The call-site arg-count check SHALL require `ref` parameters and treat all other params as optional, uniformly for workspace and engine-API callees.