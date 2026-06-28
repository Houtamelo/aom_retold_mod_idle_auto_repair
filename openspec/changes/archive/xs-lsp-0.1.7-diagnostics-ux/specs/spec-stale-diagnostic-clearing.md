# Stale Diagnostic Clearing Specification

> **Added/updated by change:** `xs-lsp-0.1.7-diagnostics-ux`

## Capability summary

The LSP server MUST publish an empty diagnostic array for an open file URI when all parse, typecheck, semantic, and include-related diagnostics have been resolved. This ensures clients remove stale error markers after the user fixes the last problem.

## Affected files

- `tools/xs-language-server/src/diagnostics.rs` — `collect_all`
- `tools/xs-language-server/src/server.rs` — `publish_diagnostics`

## Requirements

### Requirement: Publish empty diagnostics for clean files

The server MUST insert an entry mapping the open file URI to an empty diagnostic vector when every diagnostic category reports zero issues. The server MUST then send a `textDocument/publishDiagnostics` notification whose `diagnostics` array is empty and whose `version` matches the current document version.

#### Scenario: last parse error is fixed

- GIVEN an open `.xs` file has a parse error caused by a missing `;`
- WHEN the user inserts the missing `;` and the file parses successfully
- THEN the server SHALL publish `textDocument/publishDiagnostics` with an empty `diagnostics` array for that URI
- AND the notification `version` SHALL equal the current document version

#### Scenario: partial diagnostic fix

- GIVEN an open `.xs` file has one parse error and one typecheck error
- WHEN the user fixes only the parse error
- THEN the server SHALL publish a diagnostic array containing only the typecheck error
- AND the parse error SHALL NOT appear in the array

#### Scenario: clean file after previous diagnostics

- GIVEN an open `.xs` file previously published parse and semantic diagnostics
- WHEN a subsequent `didChange` removes every remaining issue
- THEN the server SHALL publish an empty diagnostic array for that URI
