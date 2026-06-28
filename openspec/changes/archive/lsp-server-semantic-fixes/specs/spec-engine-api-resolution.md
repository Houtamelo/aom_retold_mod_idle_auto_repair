# Engine-API Symbol Resolution Specification

> **Added/updated by change:** `lsp-server-semantic-fixes`

## Capability summary

The LSP server SHALL resolve callee symbols against the loaded engine-API cache before emitting an `Error 0310: invalid symbol lookup` diagnostic. The engine API is already extracted from `doxygen_retail.7z` at startup and stored in the workspace; semantic linking only needs to consume it.

## Rationale

Baseline semantic checks resolve callees only against workspace symbols. Because engine syscalls (`kb*`, `ai*`, `tr*`, `xs*`, `rm*`) are defined in the compiled engine rather than in user XS source, every call to one of them produced a false `Error 0310`. The cache already contains these names; consuming it eliminates ~140 false positives.

## Scenarios

### Scenario: happy path — engine syscall resolves

- GIVEN the engine-API cache contains `kbUnitGetPosition`
- WHEN the LSP lints a `.xs` file that calls `kbUnitGetPosition(...)`
- THEN no `unresolved symbol` diagnostic SHALL be emitted

### Scenario: error — truly unknown symbol is still flagged

- GIVEN the engine-API cache contains `kbUnitGetPosition`
- WHEN the LSP lints a `.xs` file that calls `foobarBaz(...)`
- THEN an `unresolved symbol: foobarBaz` diagnostic SHALL be emitted

### Scenario: workspace function shadows engine API

- GIVEN the engine-API cache contains `kbUnitGetPosition`
- AND the workspace defines `void kbUnitGetPosition() {}`
- WHEN the LSP lints a call to `kbUnitGetPosition()` in that workspace
- THEN the workspace definition SHALL win and no diagnostic SHALL be emitted

### Scenario: include-paste scope uses engine API too

- GIVEN the engine-API cache contains `xsSetContextPlayer`
- AND a call to `xsSetContextPlayer(0)` appears inside a file that uses `include` paste
- WHEN the LSP lints that file
- THEN no `Error 0310` diagnostic SHALL be emitted

## XS-engine constraints

- Engine syscalls are resolved by the runtime engine, not by XS source files.
- The engine-API cache is keyed by the SHA-256 of `doxygen_retail.7z`; stale game data requires cache invalidation which is handled separately.
- Workspace symbols in the same logical scope take precedence over engine-API symbols.

## Out of scope

- Adding engine-API names to completion, hover, or signature help (these features already exist).
- Re-extracting or correcting `doxygen_retail.7z` data.
- A manual allow-list for symbols missing from the archive; that is reserved for a future change if empirically required.

## Verification approach

- Automated unit test: a call to `xsSetContextPlayer(0)` produces no diagnostic when the engine API is loaded.
- Automated unit test: a call to a fabricated name produces an `Error 0310`.
- Integration test `game_folder_parse.rs`: unresolved-symbol count across the official game folder is zero with engine-API filtering disabled.

## Acceptance criteria

1. When resolving a callee symbol, if the symbol is not found in workspace symbols, the LSP SHALL consult the loaded engine-API cache.
2. If the callee is found in the engine-API cache, the LSP SHALL treat it as resolved and SHALL attach the engine-API signature for future hover/signature-help use.
3. If the callee is not found in either workspace symbols or the engine-API cache, the LSP SHALL emit an `unresolved symbol` diagnostic.
4. Engine-API resolution SHALL apply in both direct-file and include-paste (`merged_view`) semantic checks.
