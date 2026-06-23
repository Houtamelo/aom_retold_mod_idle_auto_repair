# XS Workspace Navigation Specification

## Capability summary
The plugin SHALL make engine syscalls navigable via in-memory documentation stubs and SHALL make workspace-local functions, rules, variables, classes, and members navigable via PSI references.

## Rationale
Authors switch repeatedly between calling engine APIs and reading their own code. Jump-to-definition removes the need to search manually and gives engine syscalls a readable, if synthetic, target.

## Scenarios

### Scenario: happy path — navigate to an engine syscall stub

- GIVEN the source contains the call `aiPlanCreate("myPlan", ...)`
- WHEN the user presses `Ctrl+B` on `aiPlanCreate`
- THEN an in-memory stub file opens showing the signature and help from `syscalls.json`
- AND the file title makes clear it is a generated documentation view

### Scenario: edge case — workspace function used before its definition

- GIVEN a file calls `disableAutoScouting()` on line 10 and the function is defined on line 90
- WHEN the user presses `Ctrl+B` on `disableAutoScouting`
- THEN the editor jumps to the function declaration on line 90

### Scenario: negative case — unknown identifier does not navigate

- GIVEN the source contains a token `nonExistentSymbol` that is neither a syscall nor a workspace symbol
- WHEN the user presses `Ctrl+B`
- THEN no navigation target is opened
- AND no error dialog is displayed

## XS-engine constraints

- Engine syscalls have no source file inside the workspace; navigation targets are generated as in-memory stubs and are not written to disk.
- Workspace reference resolution is static. It reflects the declarations the parser can see and does not account for runtime `kb*` evaluation or dynamic script loading.

## Out of scope
This spec does NOT cover find-usages, rename refactoring, or cross-language navigation into C++ engine sources.

## Verification approach

- Automated: `XsWorkspaceGoToDefinitionTest` uses test fixtures to assert that `Ctrl+B` on a workspace function jumps to its declaration and that `Ctrl+B` on a syscall opens a non-empty generated stub.
- Manual: in `human_assist.xs`, test `Ctrl+B` on `disableAutoScouting` and `aiPlanCreate`.

## Acceptance criteria

- The plugin MUST provide a navigation target for every name that exists in `syscalls.json`.
- The generated target MUST be an in-memory file containing the syscall signature, parameters, and help text.
- The plugin MUST resolve `Ctrl+B` on a workspace function identifier to the declaration PSI.
- The plugin MUST resolve `Ctrl+B` on a workspace rule, variable, class, or class member to the appropriate declaration when the PSI records it.
- The plugin MUST NOT attempt navigation for identifiers that match neither a syscall nor a workspace symbol.
- The plugin SHOULD support cross-file resolution of top-level workspace functions and rules.
