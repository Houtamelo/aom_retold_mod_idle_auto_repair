# Virtual Project Overlay Specification

> **Added/updated by change:** `xs-language-server` (workspace & engine-data redesign)  
> **Archived:** 2026-06-24  
> **Change verdict:** PASS WITH DEVIATIONS

## Capability summary
The LSP server SHALL present each registered workspace folder as an independent virtual project composed of the engine API, the vanilla `game/` folder, and the mod's `game/` overlay. A mod file at a relative path hides the vanilla file at the same path, and `include "..."` resolves first in the mod directory, then in the game folder, using include-root context inferred from the file's location.

## Rationale
Real AoM:R mods replace vanilla scripts at runtime. The server must mirror that overlay so diagnostics, completion, and navigation operate on the same source tree the engine will load.

## Scenarios

### Scenario: happy path — mod overlays a vanilla file
- GIVEN `mod/idle_auto_repair/game/ai/human_assist/human_assist.xs` exists
- AND the game folder contains `game/ai/human_assist/human_assist.xs`
- WHEN the server analyzes the mod file
- THEN the mod version is used
- AND the vanilla version is not visible to any consumer in that project

### Scenario: happy path — include resolution in AI context
- GIVEN a file at `mod/M/game/ai/human_assist/human_assist.xs` contains `include "core/core.xs"`
- WHEN the server resolves the include
- THEN it searches `mod/M/game/ai/core/core.xs` first
- AND then `game/ai/core/core.xs`

### Scenario: edge case — include in trigger context
- GIVEN a file at `mod/M/game/data/trigger/foo.xs` contains `include "bar/bar.xs"`
- WHEN the server resolves the include
- THEN it searches relative to `game/data/trigger/` first in the mod overlay, then in the game folder

### Scenario: negative case — file outside any registered mod
- GIVEN an `.xs` file that is not under any registered workspace folder
- WHEN it is opened
- THEN the server emits `window/showMessage`: "file not part of any registered mod; engine API only"
- AND only engine-API based features are available

### Scenario: edge case — mod-in-mod scanning
- GIVEN a workspace folder that itself contains another `game/` directory deeper inside it
- WHEN the server scans the workspace
- THEN recursion stops at the first `game/` boundary
- AND the nested `game/` is not registered as a separate mod root automatically

## XS-engine constraints
- The engine loads mod files over vanilla files by relative path. The server's overlay MUST hide the vanilla file when the relative path matches.
- `include` roots are runtime-specific: `game/ai`, `game/data/trigger`, `game/random_maps`. The server SHALL infer the root from the file's relative path under the game or mod `game/` folder.

## Out of scope
- Combined-mod shared sources (`mod/intelligent_auto_repair_and_scout/` reusing sibling feature files at deploy time). The LSP sees each registered mod independently; packaging and source sharing are handled by `scripts/deploy-mods.sh`, not by the server.
- Multi-mod symbol sharing; symbols are not shared across workspace folders.
- Server-side recursive file watching beyond the standard LSP watched-file notifications.

## Verification approach
- Automated: unit tests for relative-path overlay mapping, include-root inference for AI/TR/RM paths, and mod-in-mod boundary behavior.
- Manual: open a representative mod file and verify that go-to-definition on a vanilla `include` target lands on the mod overlay when present.

## Acceptance criteria
1. The server SHALL support `workspace/didChangeWorkspaceFolders` to add or remove mod workspace folders.
2. Each workspace folder SHALL map to a virtual project whose overlay directory is `<workspace-folder>/game/`.
3. The virtual project SHALL include the engine API, the vanilla `game/` folder, and the mod overlay.
4. A mod file at relative path `R` SHALL hide the vanilla file at `game/R` from all analysis consumers.
5. The server SHALL infer include-root context from the file's relative path under `game/ai/`, `game/data/trigger/`, or `game/random_maps/`.
6. `include "X"` SHALL resolve first in the mod's include-root directory, then in the vanilla game include-root directory.
7. Recursive workspace scanning for `game/` directories SHALL stop at `game/` boundaries and SHALL NOT treat nested `game/` directories as separate mods.
8. Files outside every registered workspace folder SHALL trigger the non-registered-mod warning and engine-API-only behavior.
9. Workspace symbol queries (`workspace/symbol`) SHALL be scoped to the owning virtual project.
10. The server SHALL treat each workspace folder as an independent project; symbols SHALL NOT leak between mods.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `xs-language-server` | 2026-06-24 | PASS WITH DEVIATIONS | Initial spec. |
