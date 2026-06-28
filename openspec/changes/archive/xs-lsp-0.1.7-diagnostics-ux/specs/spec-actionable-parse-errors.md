# Actionable Parse-Error Messages Specification

> **Added/updated by change:** `xs-lsp-0.1.7-diagnostics-ux`

## Capability summary

Parse-error diagnostics MUST describe problems in terms the author can act on. The message MUST NOT expose tree-sitter internals such as `MISSING` or `ERROR` node kinds, raw column counts, or generic node labels.

## Affected files

- `tools/xs-language-server/src/diagnostics.rs` — `to_diagnostic`

## Requirements

### Requirement: Human-readable parse-error diagnostics

The server MUST translate MISSING tree-sitter nodes into a message identifying the expected token by name (e.g., `Missing ';'`). The server MUST translate ERROR nodes into a message identifying the unexpected token text (e.g., `Unexpected identifier 'foo'`). The diagnostic message MUST NOT contain the substrings `MISSING`, `ERROR`, `node`, or `column(s)`.

#### Scenario: missing semicolon

- GIVEN an `.xs` file with a statement missing a terminating `;`
- WHEN the server emits a parse diagnostic
- THEN the diagnostic message SHALL identify the missing token as `';'` (e.g., `Missing ';'`)
- AND the message SHALL NOT contain `MISSING`, `ERROR`, `node`, or `column(s)`

#### Scenario: unexpected identifier

- GIVEN an `.xs` file with an identifier in an invalid expression context
- WHEN the server emits a parse diagnostic
- THEN the diagnostic message SHALL read `Unexpected identifier '<name>'` using the actual identifier text
- AND the message SHALL NOT contain `MISSING`, `ERROR`, `node`, or `column(s)`

#### Scenario: missing closing brace

- GIVEN an `.xs` file with a block missing its closing `}`
- WHEN the server emits a parse diagnostic
- THEN the diagnostic message SHALL identify the missing `}` token (e.g., `Missing '}'`)
- AND the message SHALL NOT contain raw tree-sitter node names or column counts
