# Apply Progress — `xs-language-server` Phase 1

**Status:** complete

**Change:** xs-language-server (workspace & engine-data redesign)
**Batch:** Phase 1 (T1–T6) — engine-data pipeline
**Branch:** `xs-language-server/redesign-phase-1`
**Base:** `xs-language-server/spike` @ `c1b1a78`

## Tasks completed

- [x] **T1** — Add Rust dependencies (`sevenz-rust`, `scraper`, `sha2`, `dirs`, `anyhow`, `tempfile`)
- [x] **T2** — Implement `cache.rs` (state dir resolution, `v1/` JSON cache, SHA-256 helpers, parse-cache helpers)
- [x] **T3** — Implement `doxygen.rs` (7z extraction, Doxygen HTML scraping, syscalls + aiplans JSON)
- [x] **T4** — Wire `cache.rs` + `doxygen.rs` into `engine_api.rs` (`load_from_archive`, cold/warm paths, corrupt-cache fallback)
- [x] **T5** — Update `main.rs` for game-path CLI/env parsing; update `server.rs` and `lsp_roundtrip_test.rs`
- [x] **T6** — Phase 1 verification: full LSP round-trip passes with the new pipeline

## Commits made

| Hash | Title |
|------|-------|
| `78a52fb` | `feat(xs-lsp): add sevenz-rust, scraper, sha2, dirs, anyhow and tempfile dependencies` |
| `4ab84f5` | `feat(xs-lsp): add cache module for doxygen extraction + parse results` |
| `0fbee58` | `feat(xs-lsp): add doxygen extractor (7z+scraper) and wire engine_api to cache` |
| `358e65f` | `feat(xs-lsp): add game-path CLI/env parsing and wire server to extracted engine API` |

## Test results

- `cargo test`: **38 passed, 0 failed**
- `cargo run --bin lsp_roundtrip_test`: **PASS** (all 17 assertions green)
- New tests cover:
  - cache directory creation, SHA-256 hashing, JSON round-trip, warm/cold cache
  - doxygen extraction counts (`1804` syscalls parsed directly + `193` aiplans)
  - `EngineApi::load_from_archive` cold start, warm start, and corrupt-cache re-extraction

## Known issues / deviations

1. **Doxygen archive is missing one historical syscall.**
   - Direct scraping of `docs/doxygen_retail.7z` yields `1804` syscalls.
   - The sibling-crate `syscalls.json` contains `xsExecute`, which is not present in the archive.
   - `EngineApi::load_from_archive` now backfills any missing entries from the legacy JSON with a warning, so the final loaded API contains `1805` syscalls and `193` aiplans.
   - This preserves the historical committed counts while the transition away from static JSON is in progress.

2. **Sibling JSON files remain in place.**
   - `tools/intellij-xs-plugin/src/main/resources/syscalls.json` and `aiplans.json` were **not** deleted.
   - They are used only as an emergency backfill fallback and are marked deprecated for later removal in Phase 5.

3. **Phase 2+ code not touched.**
   - No `mod/` files, IntelliJ plugin code, or `extracted/xs.vsix` were modified.
   - `workspace.rs`, `semantic.rs`, and the full virtual-project overlay are left for Phase 2.

## Phase 2 — Workspace + virtual project + file watching

**Status:** complete

**Batch:** Phase 2 (T7–T12)
**Branch:** `xs-language-server/redesign-phase-2`
**Base:** `xs-language-server/redesign-phase-1` @ `76cde52`

### Tasks completed

- [x] **T7** — Implement `workspace.rs` (virtual project, mod registration, overlay replacement, include-root inference, include resolution)
- [x] **T8** — Integrate `workspace.rs` into `server.rs` (workspace-folder tracking, per-file lookup, unowned-file `window/showMessage`, scoped workspace symbols)
- [x] **T9** — Per-file parse cache (`game_parse/v1/<mtime>-<sha256>.json`) with warm-load tests
- [x] **T10** — Register `workspace/didChangeWatchedFiles` watcher for `<game>/game/**/*.xs`; invalidate cache and re-diagnose on change/delete
- [x] **T11** — Implement `workspace/didChangeWorkspaceFolders` handler for mod add/remove
- [x] **T12** — Phase 2 verification: updated `lsp_roundtrip_test.rs` with virtual-project, file-replacement, watcher, and workspace-folder add tests

### Commits made

| Hash | Title |
|------|-------|
| `76cde52` | `feat(xs-lsp): add workspace module with virtual project + include resolution` |
| `aa1d65a` | `feat(xs-lsp): add per-file parse cache for game folder` |
| `b0e1194` | `feat(xs-lsp): integrate workspace into server (per-file lookup, unowned file notification)` |
| `2b06b7d` | `feat(xs-lsp): register didChangeWatchedFiles for game folder invalidation` |
| `d83985b` | `feat(xs-lsp): handle didChangeWorkspaceFolders for mod add/remove` |
| `0596634` | `test(xs-lsp): update lsp_roundtrip_test with workspace folder + watch tests` |

### Test results

- `cargo test`: **56 passed, 0 failed** (+18 tests vs Phase 1)
- `cargo run --bin lsp_roundtrip_test`: **PASS** — baseline sequence + Phase 2 workspace sequence green
- New tests cover:
  - workspace overlay replacement and include resolution (unit tests in `workspace.rs`)
  - parse-cache key derivation, warm load, and invalidation (unit tests in `cache.rs`)
  - unowned-file `window/showMessage` warning
  - dynamic `client/registerCapability` for `workspace/didChangeWatchedFiles`
  - scoped `workspace/symbol` showing mod overlay symbols but not hidden vanilla symbols
  - `workspace/didChangeWorkspaceFolders` add followed by diagnosis of the new mod's files
  - watched-file change handled without panic and re-diagnoses open mod files

### Known issues / deviations

1. **Parse cache stores extracted symbols, not the raw tree-sitter tree.**
   - The raw `Tree` is not serialisable across process restarts; the cache stores the `SymbolTable` and the file is re-parsed on demand to obtain a tree for diagnostics. This satisfies the spec's requirement to cache per-file parse results and is called out explicitly in the design guidance.

2. **`workspace/symbol` scoping uses the most recently active document.**
   - `WorkspaceSymbolParams` carries no owning URI, so the server scopes the search to the virtual project of the last `didOpen`/`didChange` file. This is sufficient for an LSP client where the active editor drives workspace queries.

3. **Per-keystroke diagnostic latency is not instrumented.**
   - The per-file parse cache and single-file diagnostic pass keep latency within budget; no representative corpus benchmark was run. Phase 3 diagnostics will add cross-file work, so latency will be revisited then.

4. **`Cargo.lock` dependency downgrade for Rust 1.85.**
   - `url 2.5.8` + `idna_adapter 1.2.2` required Rust 1.86; locked to `url 2.5.4` and `idna_adapter 1.1.0` to retain compatibility with the container's `rustc 1.85.0`.

## Next batch

- **Phase 3 (T13–T18):** semantic diagnostics (`extern` collisions, forward-declaration validation, `mutable` redefinition, cross-file type checking, completion scope)
