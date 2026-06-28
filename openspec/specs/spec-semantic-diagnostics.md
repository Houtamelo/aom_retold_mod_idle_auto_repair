# Semantic Diagnostics Specification

> **Added/updated by change:** `xs-language-server` (workspace & engine-data redesign)  
> **Updated by change:** `true-include-paste` (added include visibility rules to scenario §8)
> **Archived:** 2026-06-25
> **Change verdict:** PASS WITH DEVIATIONS

## Capability summary

The LSP server SHALL diagnose XS engine-enforced semantics: `extern` collisions across files, use-before-definition without `mutable` or forward declaration, missing forward declarations (engine `Error 0310`), file-local variable visibility, `mutable` redefinition signature equality, cross-file symbol resolution with/without `include`, included-file visibility (`static` hidden, `extern`/public visible), and int/float compatibility for engine calls and arithmetic.

## Rationale

These are the errors that currently only surface inside AoM:R. Catching them at edit time eliminates the "launch the game to validate" loop and prevents `Error 0310` at runtime. Because `include` behaves like textual paste, functions and variables defined in an included file are visible at the point of the include directive; file-local symbols from the included file must not leak into the includer.

## Scenarios

### Scenario: happy path — mutable forward-call

- GIVEN a file contains `mutable void helper(int planID = -1) { }` on line 5 and a call to `helper(1)` on line 2
- WHEN diagnostics run
- THEN no use-before-definition error is reported

### Scenario: error — use before definition

- GIVEN a file contains a call to `foo()` on line 2 and `void foo() { }` on line 10, and `foo` is not `mutable`
- WHEN diagnostics run
- THEN an error is reported at the call site matching engine `Error 0310`

### Scenario: error — extern collision

- GIVEN file A declares `extern int gX = -1;` and file B declares `int gX = -1;` (with or without `extern`)
- WHEN diagnostics run for the same virtual project
- THEN an error is reported for the duplicated global symbol

### Scenario: happy path — file-local static collision is OK

- GIVEN file A declares `int localOnly = 1;` and file B declares `int localOnly = 2;`
- WHEN diagnostics run
- THEN no collision error is reported

### Scenario: error — call before include directive

- GIVEN file A calls `bar()` before the line `include "b.xs"` and `b.xs` defines `void bar() { }`
- WHEN diagnostics run for file A
- THEN a use-before-definition error is reported at the call site

### Scenario: happy path — function defined in included file is visible

- GIVEN file A contains `include "b.xs"` and then calls `bar()`, and `b.xs` defines `void bar() { }`
- WHEN diagnostics run for file A
- THEN no use-before-definition error is reported

### Scenario: edge case — mutable redefinition with mismatched defaults

- GIVEN a file declares `mutable void helper(int x = 1) { }` and later redeclares `void helper(int x = 2) { }`
- WHEN diagnostics run
- THEN an error is reported because default values differ

### Scenario: happy path — int used where float expected

- GIVEN an engine call expects a `float` parameter and the call passes an `int` literal
- WHEN type checking runs
- THEN no error is reported

### Scenario: static_variable_in_included_file_is_hidden

- GIVEN `b.xs` declares `static int gHidden = 0;` and `a.xs` includes `b.xs` and references `gHidden`
- WHEN diagnostics run for `a.xs`
- THEN an `unresolved_symbol` diagnostic is produced at the reference to `gHidden`

### Scenario: extern_variable_in_included_file_is_visible

- GIVEN `b.xs` declares `extern int gShared = 0;` and `a.xs` includes `b.xs` and references `gShared`
- WHEN diagnostics run for `a.xs`
- THEN `gShared` is visible and no `unresolved_symbol` diagnostic is produced

### Scenario: public_function_in_included_file_is_visible

- GIVEN `b.xs` defines `void helper() { }` and `a.xs` includes `b.xs` and calls `helper()`
- WHEN diagnostics run for `a.xs`
- THEN no `use before definition` error is reported

### Scenario: mutable_redefinition_in_included_file_follows_engine_rules

- GIVEN `b.xs` defines `mutable void helper(int x = -1) { }` and `a.xs` includes `b.xs` and calls `helper(5)`
- WHEN diagnostics run for `a.xs`
- THEN no `unresolved_symbol` diagnostic is emitted and `helper` resolves to the definition in `b.xs`

## XS-engine constraints

- `xs-language-syntax.md` confirms XS has no C-style forward declaration and that `mutable` makes a function forward-callable and redefinable.
- Variables need `extern` to be visible across source files; function visibility across files follows definition-before-use or `mutable`.
- Included files behave like textual paste: included symbols are visible at the location of the `include` directive.
- Within an included file, `static` variables are file-local and SHALL NOT be visible in the includer.
- `extern` variables and public functions defined in an included file SHALL be visible in the includer.
- Int and float are interchangeable in arithmetic, comparisons, and engine-call arguments per the operator tables.

## Out of scope

- Full type inference or constant folding.
- Semantic checks not listed above (e.g., class inheritance, array bounds, vector dimension checks).
- Proving equivalence with every edge case of the XS engine.
- The combined `mutable extern` modifier behavior for functions; engine verification is required before adding a rule.

## Verification approach

- Automated: parser/LSP integration tests with fixture files for each error and happy path; merged-view unit tests for include visibility filtering.
- Type compatibility: unit tests assert `int` is accepted where `float` is expected (widening) and `float` is rejected where `int` is expected (loss of precision); string/bool are never implicitly converted to numeric types.
- Manual: load representative mod files in IntelliJ and confirm diagnostics match known engine behavior, including that a `static` variable from an included file is not offered by completion.

## Acceptance criteria

1. The server SHALL report an error when `extern X` in one file collides with any declaration or definition of `X` in another file of the same virtual project.
2. The server SHALL report an error when a function is called before its definition unless it is marked `mutable` or defined in text pasted from an include that precedes the call.
3. The server SHALL emit a diagnostic equivalent to engine `Error 0310: invalid symbol lookup` for any symbol that is used but unresolved.
4. The server SHALL treat non-`extern` top-level variables as file-local; autocompletion SHALL hide them outside their file.
5. The server SHALL allow the same file-local identifier name in different files without collision.
6. The server SHALL treat a `mutable` function as forward-callable and redefinable; a redefinition is valid only if name, parameter types, and default values are identical.
7. The server SHALL resolve cross-file symbols without `include` only when they are `extern` (variables) or defined before use (functions).
8. The server SHALL resolve symbols in an included file as visible in the including file, hiding `static`/file-local variables while exposing `extern` variables and public functions.
9. The server SHALL allow implicit `int` ↔ `float` compatibility in engine-call arguments, arithmetic, and comparisons.
10. Diagnostics SHALL update within < 200 ms per keystroke for representative mod files.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `xs-language-server` | 2026-06-24 | PASS WITH DEVIATIONS | Initial spec. |
| `true-include-paste` | 2026-06-25 | PASS WITH DEVIATIONS | Replaced the incomplete include scenario (AC 8) with explicit include-visibility scenarios and split the call-before-include case into its own scenario. |
