# Spec: LSP Signature Help

## Purpose

Provide call-signature tooltips for engine syscalls and workspace callables when the user is inside an argument list. This is the highest-ROI UX item identified in `explore.md` Section 8 and the first feature of Change 1 in `proposal.md`.

## Requirements

### REQ-SIG-01: Engine-syscall signatures

The server **SHALL** return a `SignatureHelp` response for `textDocument/signatureHelp` when the cursor is inside the argument list of a call whose callee name exists in the loaded engine API.

#### Scenario: Engine syscall with multiple parameters

- **Given** an `.xs` file contains `kbUnitCreate(` and the cursor is inside the parentheses
- **When** the client sends `textDocument/signatureHelp`
- **Then** the response contains one `SignatureInformation` whose `label` is the syscall signature
- **AND** `activeParameter` is `0`

#### Scenario: Engine syscall with defaults

- **Given** a syscall has default values documented in the engine API
- **When** signature help is requested on that call
- **Then** each `ParameterInformation` item **SHALL** expose the parameter type, name, and documented default value

### REQ-SIG-02: Unknown identifier returns no result

The server **SHALL** return `null` (no result) when the callee under the cursor is neither an engine syscall nor a workspace-resolvable callable.

#### Scenario: Call to undefined function

- **Given** an `.xs` file contains `unknownFn(`
- **When** the client sends `textDocument/signatureHelp`
- **Then** the server **SHALL** return `null`

### REQ-SIG-03: Workspace-callable signatures

The server **SHALL** build a `SignatureHelp` from the merged-view or current-file symbol table when the callee is a user-defined function or a registered rule.

#### Scenario: User-defined function

- **Given** a file defines or includes `void repairUnit(int id)`
- **When** signature help is requested on `repairUnit(`
- **Then** the response **SHALL** contain a signature with the function's return type, name, and parameters

#### Scenario: Registered rule

- **Given** a rule named `cleanupLingering` is defined or registered at runtime
- **When** signature help is requested on a call to that rule name
- **Then** the response **SHALL** contain a signature with the rule name and no parameters

### REQ-SIG-04: Trigger characters

The server **SHALL** advertise and respond to `(` and `,` as trigger characters for `textDocument/signatureHelp`.

#### Scenario: Open parenthesis

- **Given** the user types `(` after a callable name
- **When** the client sends `textDocument/signatureHelp`
- **Then** the server returns the signature for that callable

#### Scenario: Comma inside argument list

- **Given** the user is inside an argument list and types `,`
- **When** the client sends `textDocument/signatureHelp`
- **Then** the server returns the same signature with `activeParameter` advanced to the next parameter

### REQ-SIG-05: Active-parameter computation

The server **SHALL** compute `activeParameter` as `min(commas_before_cursor, parameter_count - 1)` within the current set of parentheses.

#### Scenario: Second argument

- **Given** an argument list contains one complete argument and the cursor is after the first comma
- **When** signature help is requested
- **Then** `activeParameter` **SHALL** be `1`

#### Scenario: Cursor beyond parameter count

- **Given** a callable has `N` parameters and the cursor is after `N` or more commas
- **When** signature help is requested
- **Then** `activeParameter` **SHALL** be capped at `N - 1`
