# Spec: LSP Server Capabilities

## Purpose

Define the contract for `ServerCapabilities` advertised during LSP `initialize`, so clients know which requests the XS LSP server supports. This domain is established by the `lsp-signature-help-and-document-link` change; no prior spec documented the server's capability advertisement set.

## Requirements

### REQ-CAP-01: Signature help provider

The server **SHALL** advertise `signatureHelpProvider` in `ServerCapabilities`.

#### Scenario: Initialize response

- **Given** the server receives an `initialize` request
- **When** it returns `InitializeResult.capabilities`
- **Then** `signatureHelpProvider` equals `SignatureHelpOptions { trigger_characters: ["(", ","], retrigger_characters: null, work_done_progress: null }`

### REQ-CAP-02: Document link provider

The server **SHALL** advertise `documentLinkProvider` in `ServerCapabilities`.

#### Scenario: Initialize response

- **Given** the server receives an `initialize` request
- **When** it returns `InitializeResult.capabilities`
- **Then** `documentLinkProvider` equals `DocumentLinkOptions { resolve_provider: false, work_done_progress: null }`

### REQ-CAP-03: Existing capabilities preserved

The server **SHALL NOT** regress any capability flags that were advertised before this change.

#### Scenario: Capability preservation

- **Given** the previous `ServerCapabilities` advertised `text_document_sync`, `completion_provider`, `hover_provider`, `definition_provider`, `document_symbol_provider`, `references_provider`, `rename_provider`, `workspace_symbol_provider`, `diagnostic_provider`, `semantic_tokens_provider`, and `workspace.workspace_folders`
- **When** this change is applied
- **Then** each of those fields **SHALL** remain identical in value and structure

## Origin note

This full spec is created because `openspec/specs/lsp-server-capabilities/spec.md` did not exist; the `lsp-signature-help-and-document-link` proposal implicitly establishes the server-capabilities domain.
