# Delta for Engine-API Symbol References

> **Added/updated by change:** `xs-lsp-0.1.7-diagnostics-ux`

## Delta from existing spec

This delta extends `openspec/specs/spec-engine-api-resolution.md`. That specification defines engine-API resolution for semantic diagnostics; this document adds `textDocument/references` behavior for engine symbols.

## Affected files

- `tools/xs-language-server/src/server.rs` — `textDocument/references` handler
- `tools/xs-language-server/src/references.rs` — `find_identifier_uses`

## ADDED Requirements

### Requirement: Return workspace use sites for engine-API references

For an identifier that resolves exclusively to an engine-API symbol and has no workspace definition, the server MUST return every use site of that identifier across all workspace files (mod sources and game folder). When the request specifies `includeDeclaration: false`, the result MUST NOT include a declaration location, because engine symbols have no source declaration. The result SHOULD be returned within 500 ms for a typical mod workspace of up to 10 files.

#### Scenario: find usages of `aiEcho`

- GIVEN a mod file calls the engine API function `aiEcho`
- AND other mod files in the workspace also call `aiEcho`
- AND the workspace defines no `aiEcho`
- WHEN the user invokes `textDocument/references` on an `aiEcho` identifier
- THEN the response SHALL contain a `Location` for every use site of `aiEcho` in the workspace
- AND the response SHALL NOT be an empty array

#### Scenario: exclude declaration when requested

- GIVEN `textDocument/references` is invoked on an `aiEcho` identifier with `includeDeclaration: false`
- WHEN the server resolves the symbol to the engine API
- THEN every returned `Location` SHALL be a use site
- AND no returned `Location` SHALL claim to be the symbol declaration

#### Scenario: performance budget for a typical mod

- GIVEN a mod workspace of 10 or fewer `.xs` files
- WHEN the user invokes `textDocument/references` on an engine API symbol used in multiple files
- THEN the response SHALL complete within 500 ms
