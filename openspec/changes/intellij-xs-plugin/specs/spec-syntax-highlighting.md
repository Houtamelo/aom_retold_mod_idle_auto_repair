# XS Syntax Highlighting Specification

## Capability summary
The plugin SHALL color XS source files by bundling the existing VS Code TextMate grammar and registering it for the `*.xs` file type.

## Rationale
XS is a C-like language with no first-class IDE support in JetBrains products. Syntax highlighting distinguishes keywords, types, comments, strings, numbers, rule names, and the `k`/`g`/`s` naming conventions, which reduces reading errors for mod authors.

## Scenarios

### Scenario: happy path — full grammar applies to an `.xs` file

- GIVEN the project contains `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`
- WHEN the file is opened in the IDE
- THEN keywords such as `rule`, `if`, `void`, and `int` receive distinct token scopes
- AND string literals, comments, numbers, and rule names each receive their own scope
- AND the rendering is visually equivalent to VS Code on the same file

### Scenario: edge case — rule definition on an indented line

- GIVEN a file containing the line `   rule MyRule` with leading whitespace
- WHEN the TextMate grammar tokenizes the line
- THEN `rule` is scoped as a keyword and `MyRule` is scoped as a rule name
- AND the leading spaces do not prevent the rule pattern from matching

### Scenario: negative case — non-`.xs` files are not colored as XS

- GIVEN a file named `README.txt`
- WHEN the file is opened
- THEN the XS TextMate grammar SHALL NOT be applied
- AND the file is treated with the IDE's default text handling

## XS-engine constraints

- The TextMate grammar mirrors common XS lexical conventions that the AoM:R engine accepts, e.g., the `rule <name>` declaration form where `<name>` is not followed by `(`.
- The `k_`, `g_`, and `s_` prefix scopes are naming conventions, not engine-enforced syntax; the plugin SHALL NOT treat them as syntax errors when they are absent.
- Syntax highlighting is purely lexical and does not validate that a token names a real engine syscall, `kb*` function, or valid type.

## Out of scope
This spec does NOT cover semantic highlighting based on symbol type, code inspections, or automatic formatting.

## Verification approach

- Automated: an `XsTextMateHighlightingTest` uses the IntelliJ test framework to assert that a fixture file receives non-default token scopes for keywords and strings.
- Manual: run `./gradlew runIde`, open `human_assist.xs`, and perform a side-by-side color parity check against VS Code.

## Acceptance criteria

- The plugin MUST declare `*.xs` as a file type owned by the XS language.
- The plugin MUST register the bundled `syntaxes/xs.json` as a TextMate bundle via the bundled `org.jetbrains.plugins.textmate` plugin.
- The grammar MUST assign scopes to keywords, primitive types, comments, strings, numbers, rule names, and `k`/`g`/`s` prefixed identifiers.
- The plugin SHOULD degrade to plain-text rendering if the TextMate bundle is missing, without crashing the IDE.
- The plugin MUST pass a manual visual parity test on `human_assist.xs` in both IntelliJ IDEA and Rider before this capability is considered complete.
