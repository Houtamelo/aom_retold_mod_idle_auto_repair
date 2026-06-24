# Engine Data Pipeline Specification

> **Added/updated by change:** `xs-language-server` (workspace & engine-data redesign)  
> **Archived:** 2026-06-24  
> **Change verdict:** PASS WITH DEVIATIONS  
> **Deviation note:** Implementation bumped the engine-data cache schema from `v1/` to `v2/` after removing the legacy `xsExecute` JSON backfill; see the archived change's `verify-report.md` for full deviation details.

## Capability summary
The LSP server SHALL extract the Age of Mythology: Retold engine API from `doxygen_retail.7z` at startup, cache the extracted data by the archive's SHA-256 hash under `~/.local/state/aomr_lsp/v1/<hash>.json`, and load it on subsequent starts without re-extracting.

## Rationale
Engine syscalls, AI-plan constants, and help text are the baseline knowledge the server needs to validate XS code. Shipping static JSON is fragile; extracting from the upstream Doxygen archive guarantees the server always matches the installed game.

## Scenarios

### Scenario: happy path — cache hit on warm start
- GIVEN the server was started previously with the same `--game-path`
- WHEN it restarts
- THEN it computes the SHA-256 of `doxygen_retail.7z`
- AND it loads the matching cached JSON in < 1 s

### Scenario: edge case — cache miss on cold start
- GIVEN the archive hash has no matching cache file
- WHEN the server starts
- THEN it decompresses the 7z, scrapes the relevant Doxygen HTML pages, serializes the engine API JSON, and writes it to `v1/<hash>.json`
- AND cold start completes in < 10 s on a representative machine

### Scenario: negative case — missing game path
- GIVEN no `--game-path` argument, positional argument, or `AOMR_GAME_PATH` environment variable is set
- WHEN the server starts
- THEN it exits immediately with a descriptive error message

### Scenario: negative case — missing or corrupt 7z
- GIVEN a game path that does not contain `doxygen_retail.7z`
- WHEN the server starts
- THEN it exits immediately with a descriptive error

### Scenario: negative case — corrupted cached JSON
- GIVEN a cache file that fails to deserialize
- WHEN the server loads the cache
- THEN it treats the cache as a miss and re-extracts the archive

## XS-engine constraints
- The engine API is read-only from the LSP perspective. The server never writes to the game folder or modifies `doxygen_retail.7z`.
- Doxygen HTML is produced by the game developers; scraper warnings are acceptable for minor format drift, but the server MUST NOT silently produce an empty engine API on a supported archive.

## Out of scope
- Game XML-derived constants (`MythRMConstants.txt`, `MythTRConstants.txt`) are not extracted from the 7z.
- Cache garbage collection of old schema-version directories; they are left in place and ignored.
- Network fetching or auto-updating the 7z; the user must provide the archive.

## Verification approach
- Automated: unit tests for hash computation, cache hit/miss logic, and corrupt-cache fallback.
- Manual: start the server with a valid game path, measure warm/cold start times, and confirm completion items contain engine syscalls.

## Acceptance criteria
1. The server SHALL accept the game path via `--game-path`, a positional argument, or the `AOMR_GAME_PATH` environment variable, in that precedence order.
2. The server SHALL reject startup when the game path is missing.
3. The server SHALL reject startup when `doxygen_retail.7z` is not found beneath the game path.
4. The server SHALL compute the SHA-256 of `doxygen_retail.7z` and use it as the cache key.
5. The server SHALL write extracted engine API JSON to `~/.local/state/aomr_lsp/v1/<hash>.json` on cache miss.
6. The server SHALL load the cached JSON on cache hit and skip extraction.
7. The server SHALL fall back to re-extraction if the cached JSON is corrupted or has an incompatible schema.
8. The server SHALL expose extracted syscalls, parameters, default values, return types, and help text.
9. The cache schema version `v1/` prefix SHALL enable future incompatible schema bumps without overwriting existing cache files.
10. Warm start (cache hit) SHALL complete in < 1 s; cold start (cache miss) SHOULD complete in < 10 s.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `xs-language-server` | 2026-06-24 | PASS WITH DEVIATIONS | Initial spec; implementation deviated by bumping engine-data cache schema to `v2/`. |
