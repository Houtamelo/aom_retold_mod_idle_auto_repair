# XS Syscall Regeneration Pipeline Specification

## Capability summary
The plugin SHALL include a committed Gradle task that regenerates `syscalls.json` from the `docs/doxygen_retail/` HTML files so the bundled engine API data can be refreshed when the game changes.

## Rationale
`syscalls.json` is a snapshot of the engine API. Patches add, remove, or modify syscalls. A reproducible generator reduces manual extraction work and helps reviewers see exactly what changed.

## Scenarios

### Scenario: happy path — regeneration produces a usable JSON file

- GIVEN the plugin project is checked out and `docs/doxygen_retail/` is present
- WHEN the user runs `./gradlew regenerateSyscalls`
- THEN a new `syscalls.json` is written to the bundled resources path
- AND the resulting JSON can be loaded by the completion and hover providers
- AND a diff against the committed file is small and expected

### Scenario: edge case — missing default-value table

- GIVEN a doxygen page describes a function but its parameter table has no default-value row
- WHEN the generator processes that page
- THEN it emits an empty default value for that parameter
- AND it continues processing remaining functions

### Scenario: negative case — invalid source directory

- GIVEN `docs/doxygen_retail/` is missing or the configured HTML files are absent
- WHEN the task runs
- THEN the task fails with a clear error message
- AND it does not overwrite the existing `syscalls.json` with partial output

## XS-engine constraints

- The generator maps each doxygen page to one source filename (for example, `aifuncs_8cpp.html` → `aifuncs.cpp`). That source name is preserved in the JSON so plugin hover can indicate origin.
- Array types such as `float[]` and `void` returns are preserved exactly as Doxygen emits them.
- If Doxygen lists multiple overloads, the generator MUST emit one entry per unique syscall name.
- The pipeline does not execute or test against the engine; it only extracts static documentation.

## Out of scope
This spec does NOT cover automatic commits, CI scheduling, Marketplace versioning, or regenerating `aiplans.json`.

## Verification approach

- Automated: a unit/CLI test runs the generator against the committed `docs/doxygen_retail/` and asserts counts by source file that match the expected distribution in `explore.md`.
- Manual: run `./gradlew regenerateSyscalls`, inspect the git diff, and confirm completion/hover still work.

## Acceptance criteria

- The plugin MUST provide a Gradle task named `regenerateSyscalls` that is runnable from `tools/intellij-xs-plugin/`.
- The task MUST read the relevant doxygen HTML files for all syscall source files.
- The task MUST produce a JSON file matching the schema used by the plugin (`syscalls[].{name,help,return_type,params,filename}`).
- Missing default-value tables MUST result in an empty default string, not a skipped parameter or a crash.
- Multiple entries with the same name MUST be deduplicated to one entry.
- Return types, array brackets, and `void` MUST be preserved without normalization.
- The task MUST fail with a clear message when the source directory is missing.
- The task SHOULD write the output to the same path the plugin reads so the diff is reviewable before commit.
