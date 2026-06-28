# XS Engine Syscall Completion Specification

## Capability summary
The plugin SHALL offer auto-completion for engine syscall names and SHALL insert or propose default parameter values when the user types `(` or `,` inside a syscall call.

## Rationale
The AoM:R engine exposes 1,805 syscalls across `ai*`, `kb*`, `tr*`, `xs*`, and other prefixes. Authors cannot memorize every signature. Completion shortens typing and default-value proposals reduce arity mistakes.

## Scenarios

### Scenario: happy path — completing a syscall name

- GIVEN the user has typed `aiE` inside an `.xs` file
- WHEN auto-completion is invoked
- THEN the lookup contains `aiEcho`, `aiEchoCategory`, and `aiEchoWarning`
- AND each item shows the return type and parameter summary

### Scenario: edge case — syscall with no parameters

- GIVEN the user completes `xsDisableSelf(` in a call context
- WHEN the opening parenthesis is typed
- THEN no default-value item is inserted
- AND the editor cursor remains immediately after `(` without inserting invalid text

### Scenario: negative case — no completion inside a string literal

- GIVEN the user is typing inside a double-quoted string: `"aiE|"` where `|` is the caret
- WHEN auto-completion is invoked
- THEN engine syscall completions SHALL NOT be offered

## XS-engine constraints

- Completion data is a static snapshot loaded from the bundled `syscalls.json`. It is not queried from the running AoM:R engine, so it works offline.
- Parameter defaults are stored as literal XS text. If a default refers to a user-defined constant or to another `kb*` value, the plugin inserts it verbatim; it does not evaluate it.
- `kb*` and `ai*` names in the JSON correspond to runtime engine entry points, but completion does not verify that the current script has declared any required constants.

## Out of scope
This spec does NOT cover workspace-defined functions, rules, variables, class members, or XML-derived constants.

## Verification approach

- Automated: `XsSyscallCompletionTest` fixture asserts that `aiPlanCreate` appears for the prefix `aiPlan` and that typing `aiPlanCreate(` inserts the first default parameter.
- Manual: in `human_assist.xs`, invoke completion on `aiE` and inside `aiPlanAddUnitType(`.

## Acceptance criteria

- The plugin MUST load `syscalls.json` as a bundled resource once at plugin load time.
- The plugin MUST offer lookup items for any prefix that matches one or more syscall names.
- The plugin MUST insert a complete call, including parentheses, when a name item is selected.
- The plugin MUST propose the first parameter's default value when `(` follows a syscall name and that parameter has a default.
- The plugin MUST propose the next parameter's default value when `,` is typed and the active call is a known syscall.
- Lookup items MUST be filtered by the current identifier prefix.
- The plugin MUST NOT offer syscall completions inside comments or string literals.
