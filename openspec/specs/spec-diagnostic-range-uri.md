# Diagnostic Range and URI Specification

> **Added/updated by change:** `lsp-server-semantic-fixes`

## Capability summary

When the LSP emits a duplicate-`extern` diagnostic, the diagnostic `range` and `uri` SHALL identify the actual declaration that participates in the collision. The current file's `include` directive or a comment line SHALL NOT be used as the diagnostic location.

## Rationale

Misattributed diagnostics mislead authors. In the baseline, duplicate-extern errors were published on the current open file at the line of the `include` directive (or an unrelated comment line), even though the conflicting declaration lived in the included file. Correct attribution is required for go-to-definition, quick fixes, and clear error messages.

## Scenarios

### Scenario: diagnostic points at the included declaration

- GIVEN `human_assist.xs` includes `human_assist_debug.xs` at line 11
- AND both files declare `extern int gFoo = -1;` at line 4
- WHEN the LSP lints `human_assist.xs`
- THEN any duplicate-extern diagnostic about `gFoo` SHALL have `uri` pointing to `human_assist_debug.xs`
- AND `range.start.line` SHALL be `3` (0-indexed line of the declaration)
- AND `range.start.line` SHALL NOT be `6` (the include directive)

### Scenario: diagnostic points at the local declaration when the collision is local

- GIVEN `a.xs` declares `extern int gLocal = -1;` at line 2
- AND `a.xs` also declares `extern int gLocal = -1;` at line 8
- WHEN the LSP lints `a.xs`
- THEN any diagnostic about `gLocal` SHALL have `uri` pointing to `a.xs`
- AND `range.start.line` SHALL be `7`

### Scenario: message names both files

- GIVEN `a.xs` includes `b.xs`
- AND both files declare `extern int gFoo = -1;`
- WHEN the LSP emits a duplicate-extern diagnostic
- THEN the diagnostic message SHALL contain paths to both `a.xs` and `b.xs`

## XS-engine constraints

- Diagnostics are file-bounded resources in the LSP; a diagnostic MUST be attached to the URI of the file in which the offending symbol is declared.
- Related information MAY be used to link from the current open file to the declaration file, but the primary diagnostic URI must remain the declaration file.

## Out of scope

- Changing the URI/range conventions for other diagnostic categories (e.g., `use-before-definition`, argument-count mismatch).
- Quick fixes or code actions for duplicate-extern collisions.

## Verification approach

- Automated unit test: build a collision from an included file and assert the diagnostic URI is the included file.
- Automated unit test: build a same-file collision and assert the diagnostic URI is the current file.
- Integration test `game_folder_parse.rs`: assert no diagnostic is published with a URI/range mismatch against its generating declaration.

## Acceptance criteria

1. The diagnostic `range` SHALL be the exact source range of the redeclaration that participates in the collision.
2. The diagnostic `uri` SHALL be the URI of the file where the conflict's symbol is declared.
3. The diagnostic message SHALL contain both file paths for context.
4. The diagnostic SHALL NOT be placed on the `include` directive, a comment line, or any other unrelated location.
