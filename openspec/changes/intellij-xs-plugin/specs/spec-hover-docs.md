# XS Engine Syscall Hover Documentation Specification

## Capability summary
The plugin SHALL display a documentation tooltip when the user hovers over an engine syscall name, showing its signature and help text from the bundled `syscalls.json`.

## Rationale
Authors frequently need to confirm a syscall's parameter order or behavior. Showing the signature and docstring inline avoids context switches to a browser or the `docs/doxygen_retail/` HTML files.

## Scenarios

### Scenario: happy path — hover over a documented syscall

- GIVEN the source contains the identifier `aiPlanCreate`
- WHEN the user hovers over it
- THEN the popup shows the signature `int aiPlanCreate(...)`
- AND the popup includes the help paragraph from `syscalls.json`

### Scenario: edge case — syscall with no help paragraph

- GIVEN the source contains a syscall whose JSON entry has an empty `help` field
- WHEN the user hovers over it
- THEN the popup shows the signature
- AND it displays a placeholder such as "No documentation" instead of a blank popup

### Scenario: negative case — hover over an unknown identifier

- GIVEN the source contains a token that is not present in `syscalls.json`
- WHEN the user hovers over it
- THEN no syscall documentation popup is shown

## XS-engine constraints

- Hover content reflects the bundled static API snapshot. It does not query the game engine, so runtime behavior of `kb*` functions or dynamic plan state is not represented.
- The plugin SHALL render parameter defaults as stored in JSON without interpreting them as executable XS.

## Out of scope
This spec does NOT cover hover for workspace-defined functions, class members, or XML-derived constants.

## Verification approach

- Automated: `XsHoverTest` asserts that hovering over `kbUnitCount` in a fixture file produces a documentation string containing the syscall signature.
- Manual: in `human_assist.xs`, hover over `aiPlanCreate` and confirm the help text appears.

## Acceptance criteria

- The plugin MUST detect when the mouse or keyboard caret is over a token that matches a `syscalls.json` entry.
- The plugin MUST render the syscall's return type, name, and parameter list in the hover popup.
- The plugin MUST render the `help` paragraph when it is present.
- The plugin MUST show a deterministic placeholder when `help` is absent or whitespace-only.
- Hover resolution MUST run on a background thread and MUST NOT freeze the UI.
- The plugin MAY display the source filename from the JSON entry as supplementary metadata.
