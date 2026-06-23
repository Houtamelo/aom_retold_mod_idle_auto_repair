# XS Syscall Parameter Info Specification

## Capability summary
The plugin SHALL display a parameter-info popup inside an engine-syscall call and highlight the parameter that corresponds to the current caret position.

## Rationale
Many syscalls accept a long parameter list, for example `aiPlanAddUnitType(planID, unitTypeID, count)`. Parameter info reminds the author of the expected type and name of the active argument.

## Scenarios

### Scenario: happy path — current parameter highlighted

- GIVEN the user is typing `aiPlanSetVariableBool(planID, |)` and `|` is the caret
- WHEN the IDE shows parameter info
- THEN the popup displays the full signature
- AND the second parameter is rendered in bold

### Scenario: edge case — no opening parenthesis yet

- GIVEN the caret is inside a syscall identifier but the `(` has not been typed
- WHEN the IDE queries parameter info
- THEN no popup is shown
- AND the editor remains responsive

### Scenario: negative case — parameter info is limited to engine syscalls

- GIVEN the user is typing a workspace-defined function call `myHelper(a, |)`
- WHEN the IDE requests parameter info
- THEN this capability SHALL NOT be required to show a popup for that call

## XS-engine constraints

- Parameter order and parameter defaults come from `syscalls.json`. Parameter names are descriptive only; the plugin does not enforce types or validate values against the game engine.
- `kb*` parameters may logically expect constants such as `cUnitType*`, but the popup SHALL show only the parameter name from JSON.

## Out of scope
This spec does NOT cover parameter info for workspace-defined functions, class methods, or lambda calls.

## Verification approach

- Automated: `XsParameterInfoTest` opens a fixture call, places the caret at successive comma positions, and asserts that the bold parameter index increases.
- Manual: in `human_assist.xs`, place the caret inside `aiPlanAddUnitType(...)` and confirm the popup tracks the active argument.

## Acceptance criteria

- The plugin MUST detect when the caret is inside a pair of parentheses that follows a known syscall name.
- The plugin MUST display the syscall signature with parameter names and types.
- The plugin MUST bold the parameter whose index matches the number of preceding commas in the active argument list.
- The plugin SHALL gracefully handle a missing closing parenthesis, for example at end-of-file.
- The plugin SHALL NOT trigger the popup for identifiers that are absent from `syscalls.json`.
