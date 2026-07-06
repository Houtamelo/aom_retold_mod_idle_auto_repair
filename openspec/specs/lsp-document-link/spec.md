# Spec: LSP Document Link

## Purpose

Make every `include "..."` path in an `.xs` file clickable so users can navigate the include graph without manually locating files. This implements the Tier B item noted in `explore.md` Section 4 and Change 1 of `proposal.md`.

## Requirements

### REQ-DLK-01: Include directive links

The server **SHALL** return a `DocumentLink` for each `include "..."` directive in the requested file when the path resolves through the workspace.

#### Scenario: Single include

- **Given** a file contains `include "ai/core/core.xs"`
- **When** the client sends `textDocument/documentLink` for that file
- **Then** the response contains a link whose `target` URI points to the resolved file

### REQ-DLK-02: Unresolved includes omitted

The server **SHALL NOT** emit a `DocumentLink` for an include path that fails to resolve.

#### Scenario: Missing target

- **Given** a file contains `include "does/not/exist.xs"`
- **When** the client sends `textDocument/documentLink`
- **Then** the response **SHALL NOT** contain a link for that path

### REQ-DLK-03: Range covers the path literal only

The `DocumentLink.range` for an include directive **SHALL** span exactly the characters between the surrounding quotes, excluding the quotes and any surrounding whitespace.

#### Scenario: Quoted path range

- **Given** a line contains `include "ai/core/core.xs";`
- **When** the server generates the link
- **Then** `range` starts at the first character after the opening quote and ends at the last character before the closing quote

### REQ-DLK-04: Multiple include directives

The server **SHALL** return one `DocumentLink` per include directive in the file.

#### Scenario: Three includes

- **Given** a file contains three include directives
- **When** the client sends `textDocument/documentLink`
- **Then** the response contains exactly three links, each mapping to the corresponding target and range

#### Scenario: File with no includes

- **Given** a file contains no include directives
- **When** the client sends `textDocument/documentLink`
- **Then** the server **SHALL** return `null` or an empty array

### REQ-DLK-05: Mod-overlay resolution

Include resolution for document links **SHALL** follow the same mod-overlay-first, vanilla-`game/` fallback rules used for semantic resolution.

#### Scenario: Mod overlay copy

- **Given** a mod overlays the file at `game/ai/core/core.xs`
- **When** the server generates a link for `include "ai/core/core.xs"` in that mod
- **Then** the link target **SHALL** point to the mod overlay copy, not the vanilla file
