# Callable Rules Specification

> **Added/updated by change:** `lsp-server-semantic-fixes`

## Capability summary

The LSP server SHALL treat `rule` symbols as callable like functions. A call expression naming a rule resolves to the rule symbol and produces no `unresolved symbol` or `use before declaration` diagnostic. Rule calls SHALL NOT be subject to argument-count or return-type checks because the engine handles their signatures internally.

## Rationale

Symbols such as `updateBreakdown` and `applyDistribution` are declared with the `rule` keyword, not as functions. The AoM:R engine allows calling a rule like a function to execute it immediately outside its normal interval. The current LSP only resolves `SymbolKind::Function`, so these valid calls produce false `Error 0310` diagnostics.

## Scenarios

### Scenario: happy path — rule defined in the workspace is callable

- GIVEN `game/ai/human_assist/human_assist.xs` defines `rule updateBreakdown` at line 236
- AND `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs` calls `updateBreakdown();` at line 394
- WHEN the LSP lints the mod file
- THEN no "unresolved symbol: updateBreakdown" diagnostic SHALL be emitted

### Scenario: happy path — rule call requires no argument checks

- GIVEN a rule `rule r active { }`
- WHEN the LSP lints a call `r(1, 2);`
- THEN no argument-count diagnostic SHALL be emitted

### Scenario: rule registration patterns are recognized

- GIVEN `game/ai/core/perfrule.xs` calls `xsEnableRule("updateBreakdown");`
- WHEN the LSP analyzes the workspace
- THEN `updateBreakdown` SHALL be registered as a `SymbolKind::Rule` if no `rule updateBreakdown` definition is present

### Scenario: error — unknown rule is still flagged

- GIVEN no `rule updateBreakdown` definition exists in the workspace
- AND no `xsEnableRule("updateBreakdown")` registration is found
- WHEN the LSP lints a call to `updateBreakdown();`
- THEN an `unresolved symbol` diagnostic SHALL be emitted

## XS-engine constraints

- Rules have no return type and no declared parameters, but the engine tolerates calls that pass arguments.
- Rules are registered implicitly by definition or explicitly via `xsEnableRule`, `trRuleAdd`, `trRuleAddActive`, and related control functions.
- A rule call is resolved by name in the same logical scope as a function call.

## Out of scope

- Verifying that a rule is currently enabled or disabled at the call site.
- Checking the `minInterval` / `maxInterval` attributes of rule definitions.
- Treating rule calls as available for completion outside the current file.

## Verification approach

- Automated unit test: `rule r {} active {}` called as `r();` produces no diagnostics.
- Automated unit test: a call to an unknown rule still emits `Error 0310`.
- Integration test `game_folder_parse.rs`: no unresolved-symbol diagnostics are produced for rule calls in the official game folder.

## Acceptance criteria

1. The LSP SHALL introduce or preserve a distinct `SymbolKind::Rule` representation for rule symbols.
2. During workspace analysis, the LSP SHALL recognize `xsEnableRule("name")`, `trRuleAdd("name")`, `trRuleAddActive("name")`, and similar rule-control calls as rule registrations.
3. The LSP SHALL allow rule symbols to satisfy call expressions in callee resolution.
4. When a call site names a rule, the LSP SHALL resolve it to the `SymbolKind::Rule` entry and SHALL NOT emit an `unresolved symbol` diagnostic.
5. Rule symbols SHALL NOT be subject to parameter-count checks or return-type checks.
