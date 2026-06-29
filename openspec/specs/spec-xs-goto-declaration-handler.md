# Spec: XS goto-declaration handler

> **Added/updated by change:** `fix-plugin-goto-definition-ctrl-click-keybind`

## Capability summary

The XS plugin SHALL provide navigation to symbol definitions via Ctrl+Click and the go-to-definition keybind by forwarding each trigger to the active LSP server's `textDocument/definition` endpoint.

## Rationale

Issue #1: hover and right-click navigation work, but Ctrl+Click and the go-to-definition keybind do not. A `GotoDeclarationHandler` wires the active LSP server's `textDocument/definition` to those paths without altering hover, right-click, or Find Usages.

## Scenarios

### Scenario: R1 — Ctrl+Click navigates to a single definition

- GIVEN an open `.xs` symbol resolves to one `Location`
- WHEN the user Ctrl+Click it
- THEN the editor SHALL open the file at the returned line and column

### Scenario: R2 — Keybind navigates to a single definition

- GIVEN the same state as R1
- WHEN the user invokes the go-to-definition keybind (`Ctrl+B` / `⌘B`)
- THEN the editor SHALL open the file at the returned line and column

### Scenario: R3 — Multi-target returns show a chooser

- GIVEN an open `.xs` symbol resolves to multiple `Location`s or `LocationLink`s
- WHEN the user invokes any goto trigger
- THEN the platform SHALL display a chooser popup and navigate to the selection

### Scenario: R4 — Right-click menu remains functional

- GIVEN the right-click → Go to → Declarations or usages menu
- WHEN the user invokes it
- THEN behavior SHALL remain unchanged

### Scenario: R5 — Hover remains functional

- GIVEN hover over an `.xs` symbol
- WHEN the user hovers
- THEN behavior SHALL remain unchanged

### Scenario: R6 — Non-XS files do nothing

- GIVEN an open file is not `.xs`
- WHEN the user invokes any goto trigger
- THEN the XS handler SHALL return empty/null and leave the default handler in control

### Scenario: R7 — LSP timeout does not freeze the editor

- GIVEN the LSP server is slow or hung
- WHEN the user invokes any goto trigger
- THEN the handler SHALL cap the request at five seconds, leaving the editor responsive with no navigation on timeout

### Scenario: R8 — External vanilla game file navigation works

- GIVEN a symbol definition is in vanilla `game/` outside module PSI
- WHEN the user invokes any goto trigger
- THEN the editor SHALL open that external file as a buffer at the target line and column

### Scenario: R9 — Strict-TDD unit test added

- GIVEN a stubbable `XsDefinitionResolver` seam
- WHEN `XsGotoDeclarationHandlerTest` exercises single target, multi-target, and timeout cases
- THEN the test SHALL pass with correct target assertions

## Cross-references

- **User issue:** `docs/issues/2026-06-29-runtime-issues.md` Issue #1.
- **Approach/root cause:** `proposal.md` and `explore.md`.
- **Version:** PATCH bump `0.2.2 → 0.2.3` per `AGENTS.md`.
- **Scope:** Plugin-side only; LSP server already implements `textDocument/definition`.
- **Future auto-wire:** Spec is silent on deduplication/removal if platform wiring later works.

## Out of scope

- Issues 2–4 from the runtime-issues doc.
- LSP server changes.
- Include-statement go-to-definition; the design SHALL NOT block a later second handler.
- Hover, right-click navigation, and Find Usages.

## Verification approach

- Automated: `XsGotoDeclarationHandlerTest` covers single target, multi-target, and timeout.
- Automated: `./gradlew :test` and `./gradlew buildPlugin` SHALL succeed.
- Manual: in Rider, verify Ctrl+Click/`Ctrl+B`, multi-target chooser, and hover/right-click behavior.

## Acceptance criteria

| # | Criterion |
|---|-----------|
| 1 | `XsGotoDeclarationHandler` SHALL be registered in `META-INF/plugin.xml`. |
| 2 | Handler SHALL call `textDocument/definition` with a five-second timeout. |
| 3 | Single `Location`/`LocationLink` SHALL navigate directly. |
| 4 | Multiple targets SHALL show a chooser; selection SHALL navigate. |
| 5 | Non-XS files SHALL return no targets. |
| 6 | Timeout SHALL return no targets and no error balloon. |
| 7 | Vanilla `game/` targets outside module PSI SHALL open as external buffers. |
| 8 | Hover and right-click navigation SHALL remain unchanged. |
| 9 | `pluginVersion` SHALL bump from `0.2.2` to `0.2.3`. |
| 10 | `XsGotoDeclarationHandlerTest` SHALL be added and pass under strict TDD. |

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `fix-plugin-goto-definition-ctrl-click-keybind` | 2026-06-29 | Spec phase | New spec formalizing the plugin-side go-to-definition handler contract. |
