# Delta for Engine-API Symbol Definition

> **Added/updated by change:** `xs-lsp-0.1.7-diagnostics-ux`

## Delta from existing spec

This delta extends `openspec/specs/spec-engine-api-resolution.md`. That specification defines engine-API resolution for semantic diagnostics; this document adds `textDocument/definition` behavior for engine symbols and removes the legacy non-navigable `xs-stub://engine/<name>` response.

## Affected files

- `tools/xs-language-server/src/server.rs` — `textDocument/definition` handler
- `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` — regression assertions

## ADDED Requirements

### Requirement: Engine symbols return no definition

For an identifier that resolves exclusively to an engine-API symbol and has no workspace definition, `textDocument/definition` MUST return `null` (no result). The server MUST NOT return a virtual URI of the form `xs-stub://engine/<name>` or any other synthetic `Location` for engine symbols. Workspace-defined symbols SHALL continue to resolve to their declarations as before.

#### Scenario: go to definition on `aiEcho`

- GIVEN a mod file references the engine API function `aiEcho`
- AND the workspace defines no `aiEcho`
- WHEN the user invokes `textDocument/definition` on `aiEcho`
- THEN the response SHALL be `null`
- AND the response SHALL NOT contain a `Location`

#### Scenario: regression — workspace symbol still resolves

- GIVEN a mod file calls a function `helper` defined in the workspace
- WHEN the user invokes `textDocument/definition` on `helper`
- THEN the response SHALL be a single `Location` pointing to the declaration file URI and declaration range

#### Scenario: regression — no stub URI scheme

- GIVEN any `textDocument/definition` response from the server
- WHEN the symbol is engine API or workspace-defined
- THEN no returned `Location.uri` SHALL use the `xs-stub://` scheme
