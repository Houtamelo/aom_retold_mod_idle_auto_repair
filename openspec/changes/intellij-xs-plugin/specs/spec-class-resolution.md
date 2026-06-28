# XS Class and Type Resolution Specification

## Capability summary
The plugin SHALL parse and represent class definitions, methods, member variables, lambda types, `ref` parameters, and array types in the PSI so that member completion and navigation can resolve against the declared type.

## Rationale
The VS Code extension supports classes, lambdas, and arrays. Delivering parity in IntelliJ requires a structured PSI; without it, member lookups after `.` and type-aware navigation are impossible.

## Scenarios

### Scenario: happy path — member completion from class type

- GIVEN a class `UnitGroup` declares a method `removeExpired()` and a variable declared as `UnitGroup group`
- WHEN the user types `group.` and invokes completion
- THEN `removeExpired` appears in the completion list

### Scenario: edge case — `ref` array declaration parsed as one variable

- GIVEN a declaration `int[] counts` appears in the source
- WHEN the PSI is built
- THEN it is represented as a single array-typed variable declaration
- AND the identifier `counts` is exposed as a named PSI element

### Scenario: negative case — parser recovers from incomplete class

- GIVEN a file contains `class Broken { int x;` without a closing brace, followed by a valid function `void reset()`
- WHEN the PSI is built
- THEN the function `reset` is still exposed as a top-level declaration
- AND the plugin does not crash or discard the entire file

## XS-engine constraints

- XS classes are a subset of C++-like syntax; `ref`, arrays, and lambdas have runtime semantics that the plugin does not evaluate.
- Class member resolution is based on static declared types, not on runtime `kbUnit*` or `aiPlan*` state.
- The parser SHALL treat the `k`/`g`/`s` naming conventions as ordinary identifiers.

## Out of scope
This spec does NOT cover type inference, semantic type checking, overload resolution, or generic templates.

## Verification approach

- Automated: parser fixture tests assert that class, method, member, lambda, `ref`, and array declarations produce expected PSI elements.
- Automated: `XsWorkspaceGoToDefinitionTest` asserts that `Ctrl+B` on a member reference resolves to the declaration in the class.
- Manual: open a fixture class/lambda file and test member completion and navigation.

## Acceptance criteria

- The parser MUST recognize class declarations, method declarations, and member variable declarations.
- The parser MUST represent lambda type syntax as a callable PSI element.
- The parser MUST recognize `ref` and array type modifiers on parameters and variables.
- The PSI MUST expose named elements for functions, rules, variables, classes, methods, and members.
- The PSI MUST implement error recovery so that syntax errors in one declaration do not hide sibling declarations.
- The plugin MUST resolve member access `obj.member` or `obj.method(...)` to the declaration in the declared class of `obj`.
- The plugin SHOULD expose top-level function and rule declarations as named PSI elements.
