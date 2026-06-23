# XS AI Plan Constants Completion Specification

## Capability summary
The plugin SHALL offer completion for the bundled `cPlan*` constants used by `aiPlanGetVariable*` / `aiPlanSetVariable*` syscalls.

## Rationale
AI plan scripts reference over one hundred plan-variable constants that name numeric slots. Showing these constants as completion items reduces the risk of using a mistyped or obsolete identifier.

## Scenarios

### Scenario: happy path — plan constants complete by prefix

- GIVEN the user has typed `cPlanAdd` in an `.xs` file
- WHEN auto-completion is invoked
- THEN the lookup contains `cPlanAddResourcePriority` and other matching `cPlan*` constants from `aiplans.json`

### Scenario: edge case — entry with string variable value

- GIVEN an `aiplans.json` entry has `variable_type` `"int"` but `variable_value` stored as the string `"-1"`
- WHEN constant entries are loaded
- THEN the item still appears in completion
- AND the detail text chooses one canonical representation

### Scenario: negative case — unrelated prefixes do not include plan constants

- GIVEN the user has typed `cUnitType`
- WHEN auto-completion is invoked
- THEN `cPlan*` items SHALL NOT appear in the lookup list

## XS-engine constraints

- `cPlan*` constants are a static mapping loaded from `aiplans.json`. The engine evaluates them at runtime inside `aiPlanGetVariableInt` and similar syscalls; the plugin does not validate that a chosen constant is appropriate for a given plan type.

## Out of scope
This spec does NOT cover runtime evaluation of plan variables, XML-derived constants, or computing numeric values not present in the JSON.

## Verification approach

- Automated: `XsAiplanCompletionTest` loads a synthetic `aiplans.json` and asserts that typing `cPlan` proposes known entries.
- Manual: in a file containing plan code, type `cPlan` and confirm completion.

## Acceptance criteria

- The plugin MUST bundle `aiplans.json` as a read-only resource.
- The plugin MUST load the constants once at plugin load time.
- The plugin MUST contribute lookup items for `cPlan*` prefixes filtered by the current identifier.
- The plugin MUST display `variable_type` and `variable_value` as item detail or description text when available.
- The plugin SHALL NOT mix `cPlan*` completions with XML-derived prefix families.
- The plugin MUST update its index if the bundled `aiplans.json` is replaced.
