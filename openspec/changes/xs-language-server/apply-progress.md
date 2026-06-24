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

## Phase 3 — Semantic diagnostics

**Status:** complete

**Batch:** Phase 3 (T13–T18)
**Branch:** `xs-language-server/redesign-phase-3`
**Base:** `xs-language-server/redesign-phase-2` @ `e35d4e1`

### Tasks completed

- [x] **T13** — Extend `symbols.rs` with `Visibility` enum (`Local`/`Const`/`Extern`/`Public`), `is_mutable`, `is_forward`, and parameter default-value extraction. Unit tests for extern, mutable, and forward-declaration recovery.
- [x] **T14** — Implement `semantic.rs`: `extern` collision detection, forward-declaration validation, `mutable` redefinition equality, unresolved-symbol `Error 0310`. Unit tests with fixtures.
- [x] **T15** — Wire semantic diagnostics into `diagnostics.rs`/`server.rs`: on `didOpen`/`didChange`, build a semantic virtual project and merge semantic diagnostics with parse/typecheck diagnostics.
- [x] **T16** — Extend `typecheck.rs`: implicit `int`→`float` widening, cross-file user-function argument checking via the virtual project.
- [x] **T17** — Update `completion.rs`: combine engine API with exported project symbols; show all symbols in the current file and only `extern` symbols from other files; hide file-local variables outside their file.
- [x] **T18** — Phase 3 verification: semantic fixture files under `tools/xs-language-server/src/semantic_fixtures/`, LSP round-trip tests for forward decl, extern collision, mutable redefinition, int/float widening/loss.

### Commits made

| Hash | Title |
|------|-------|
| `e35d4e1` | `feat(xs-lsp): track extern/mutable/visibility in symbol table` |
| `2c967a9` | `feat(xs-lsp): add semantic module for extern collision + forward-decl + mutable redefinition` |
| `cd92cc9` | `feat(xs-lsp): add int↔float widening + cross-file typecheck` |
| `1881b92` | `feat(xs-lsp): scope completion to exported symbols + current file` |
| `ecd2050` | `feat(xs-lsp): wire semantic diagnostics into didOpen/didChange` |
| `405244c` | `test(xs-lsp): add semantic fixtures and roundtrip tests` |

### Test results

- `cargo test`: **75 passed, 0 failed** (+19 tests vs Phase 2)
- `cargo build --bin xs-language-server && cargo run --bin lsp_roundtrip_test`: **PASS** (baseline + workspace + semantic sequences green)
- New coverage:
  - `semantic.rs`: extern collisions, forward declarations, mutable redefinition, unresolved symbols
  - `typecheck.rs`: int→float widening, float→int loss, cross-file user-function type checks
  - `completion.rs`: current-file symbols, extern-only cross-file symbols, hidden file-local variables
  - `lsp_roundtrip_test.rs`: seven end-to-end semantic fixture scenarios

### Known issues / deviations

1. **Forward declarations parse as `ERROR` nodes.**
   - The current tree-sitter XS grammar does not have a dedicated rule for function headers without bodies, so `void bar(int x = -1);` surfaces as an `ERROR` node.
   - `symbols::build_symbol_table` recovers forward declarations from these `ERROR` nodes when they contain a type, identifier, and parameter list and no body.
   - This is documented in `symbols.rs` and matches the engine-enforced rule without requiring a grammar change in this phase.

2. **Completion cross-file visibility is `extern`-only (not `Public` functions from other files).**
   - This follows the explicit scoping guidance in the implementation spec.
   - `Public` functions remain visible within the current file and via `include` textual paste.

3. **`include "..."` textual paste is approximated.**
   - `completion.rs` includes the full symbol set for files whose relative path matches an `include` target in the current file.
   - Forward-declaration and type-check cross-file resolution currently treat all project function definitions as visible; included files are not reordered.

4. **Per-keystroke diagnostic latency not instrumented.**
   - The semantic project is rebuilt on every `didOpen`/`didChange`. Real mods may require incremental update in a later phase to consistently stay under the 200 ms budget.

## Next batch

- **Phase 4 (T19–T26):** IntelliJ client integration
