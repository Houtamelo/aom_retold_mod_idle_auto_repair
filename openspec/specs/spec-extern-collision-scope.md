# Extern Collision Scope Specification

> **Added/updated by change:** `lsp-server-semantic-fixes`

## Capability summary

The LSP server SHALL scope duplicate-`extern` collision detection to a single logical link unit: the currently analyzed file plus every file transitively reachable through `include "..."` directives. Independent `extern` declarations in unrelated files SHALL NOT be flagged as collisions, because XS treats each includer as a separate translation unit and header-style re-declaration of the same global is intentional.

## Rationale

The existing `semantic::check_extern_collisions` scans every file in the virtual project. AoM:R source commonly declares the same global `extern` in multiple helper headers consumed by different includers (e.g., `game/ai/human_assist/human_assist_debug.xs` and `game/ai/core/utilities/debug.xs`). Flagging those as errors produces false positives and contradicts engine behavior.

## Scenarios

### Scenario: happy path — duplicate extern in unrelated top-level files is allowed

- GIVEN `game/ai/a.xs` declares `extern int gFoo = -1;`
- AND `game/ai/b.xs` declares `extern int gFoo = -1;`
- AND neither file includes the other
- WHEN the LSP lints `a.xs`
- THEN no duplicate-extern diagnostic SHALL be emitted

### Scenario: happy path — duplicate extern inside a single include closure is allowed

- GIVEN `human_assist.xs` declares `extern int gFoo = -1;`
- AND `human_assist_debug.xs` declares `extern int gFoo = -1;`
- AND `human_assist.xs` includes `human_assist_debug.xs`
- WHEN the LSP lints `human_assist.xs`
- THEN no duplicate-extern diagnostic SHALL be emitted (both declarations are in the same merged scope)

### Scenario: error — same extern included twice in one link unit is still a collision

- GIVEN `human_assist.xs` includes both `debug_a.xs` and `debug_b.xs`
- AND both `debug_a.xs` and `debug_b.xs` declare `extern int gBar = -1;`
- WHEN the LSP lints `human_assist.xs`
- THEN a duplicate-extern diagnostic for `gBar` SHALL be emitted

### Scenario: error — extern in one file collides with a definition in the same link unit

- GIVEN `a.xs` declares `extern int gBaz = -1;`
- AND `b.xs` declares `int gBaz = -1;` (without `extern`)
- AND `a.xs` includes `b.xs`
- WHEN the LSP lints `a.xs`
- THEN a duplicate-extern diagnostic for `gBaz` SHALL be emitted

## XS-engine constraints

- XS has no C-style forward declarations; `extern` globals are the cross-file visibility mechanism.
- `include "..."` behaves like textual paste, so symbols from included files share the includer's logical scope.
- Files that are not in the same include closure are independent translation units even when they reside in the same virtual project.

## Out of scope

- Collision detection for file-local (`static` / non-`extern`) top-level variables in different files.
- Detection of `extern` collisions across separate mod roots or game-install roots; each root owns its own globals.

## Verification approach

- Automated unit test in `semantic.rs`: two unrelated files with the same `extern` produce zero diagnostics.
- Automated unit test: a file that includes two headers with the same `extern` produces a diagnostic.
- Integration test `game_folder_parse.rs`: duplicate-extern count across the official `game/**/*.xs` tree is zero.

## Acceptance criteria

1. The LSP SHALL only flag a duplicate `extern` when both declarations occur in the same logical unit (same file, or a file `include`-pasted into the current file through `merged_view`).
2. The LSP SHALL NOT flag an extern collision between a top-level file and an unrelated mod or game file.
3. The LSP SHALL treat `extern` declarations in different mod folders as independent; each virtual project owns its own globals.
4. A real collision inside a single include closure SHALL still be reported with correct source location.
