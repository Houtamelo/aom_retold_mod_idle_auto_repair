# Semantic Diagnostics Specification

> **Added/updated by change:** `xs-language-server` (workspace & engine-data redesign)  
> **Archived:** 2026-06-24  
> **Change verdict:** PASS WITH DEVIATIONS

## Capability summary
The LSP server SHALL diagnose XS engine-enforced semantics: `extern` collisions across files, use-before-definition without `mutable` or forward declaration, missing forward declarations (engine `Error 0310`), file-local variable visibility, `mutable` redefinition signature equality, cross-file symbol resolution with/without `include`, and int/float compatibility for engine calls and arithmetic.

## Rationale
These are the errors that currently only surface inside AoM:R. Catching them at edit time eliminates the "launch the game to validate" loop and prevents `Error 0310` at runtime.

## Scenarios

### Scenario: happy path — mutable forward-call
- GIVEN a file contains `mutable void helper(int planID = -1) { }` on line 5 and a call to `helper(1)` on line 2
- WHEN diagnostics run
- THEN no use-before-definition error is reported

### Scenario: error — use before definition
- GIVEN a file contains a call to `foo()` on line 2 and `void foo() { }` on line 10
- AND `foo` is not `mutable`
- WHEN diagnostics run
- THEN an error is reported at the call site matching engine `Error 0310`

### Scenario: error — extern collision
- GIVEN file A declares `extern int gX = -1;`
- AND file B declares `int gX = -1;` (with or without `extern`)
- WHEN diagnostics run for the same virtual project
- THEN an error is reported for the duplicated global symbol

### Scenario: happy path — file-local static collision is OK
- GIVEN file A declares `int localOnly = 1;`
- AND file B declares `int localOnly = 2;`
- WHEN diagnostics run
- THEN no collision error is reported

### Scenario: error — missing included function definition
- GIVEN file A includes file B and calls `bar()` before `bar` is defined
- AND `bar` is not `mutable`
- WHEN diagnostics run
- THEN a use-before-definition error is reported at the call site

### Scenario: edge case — mutable redefinition with mismatched defaults
- GIVEN a file declares `mutable void helper(int x = 1) { }` and later redeclares `void helper(int x = 2) { }`
- WHEN diagnostics run
- THEN an error is reported because default values differ

### Scenario: happy path — int used where float expected
- GIVEN an engine call expects a `float` parameter
- AND the call passes an `int` literal
- WHEN type checking runs
- THEN no error is reported

## XS-engine constraints
- `xs-language-syntax.md` confirms XS has no C-style forward declaration and that `mutable` makes a function forward-callable and redefinable.
- Variables need `extern` to be visible across source files; function visibility across files follows definition-before-use or `mutable`.
- Included files behave like textual paste: included symbols are visible in the including file.
- Int and float are interchangeable in arithmetic, comparisons, and engine-call arguments per the operator tables.

## Out of scope
- Full type inference or constant folding.
- Semantic checks not listed above (e.g., class inheritance, array bounds, vector dimension checks).
- Proving equivalence with every edge case of the XS engine.

## Verification approach
- Automated: parser/LSP integration tests with fixture files for each error and happy path.
- Type compatibility: unit tests assert `int` is accepted where `float` is expected (widening) and `float` is rejected where `int` is expected (loss of precision); string/bool are never implicitly converted to numeric types.
- Manual: load representative mod files in IntelliJ and confirm diagnostics match known engine behavior.

## Acceptance criteria
1. The server SHALL report an error when `extern X` in one file collides with any declaration or definition of `X` in another file of the same virtual project.
2. The server SHALL report an error when a function is called before its definition unless it is marked `mutable`. XS has no C-style forward declarations.
3. The server SHALL emit a diagnostic equivalent to engine `Error 0310: invalid symbol lookup` for any symbol that is used but unresolved.
4. The server SHALL treat non-`extern` top-level variables as file-local; autocompletion SHALL hide them outside their file.
5. The server SHALL allow the same file-local identifier name in different files without collision.
6. The server SHALL treat a `mutable` function as forward-callable and redefinable; a redefinition is valid only if name, parameter types, and default values are identical.
7. The server SHALL resolve cross-file symbols without `include` only when they are `extern` (variables) or defined before use (functions).
8. The server SHALL resolve symbols in an included file as visible in the including file.
9. The server SHALL allow implicit `int` ↔ `float` compatibility in engine-call arguments, arithmetic, and comparisons.
10. Diagnostics SHALL update within < 200 ms per keystroke for representative mod files.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `xs-language-server` | 2026-06-24 | PASS WITH DEVIATIONS | Initial spec; per-keystroke latency budget was not instrumented on a representative corpus. |
