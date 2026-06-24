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

## Next batch

- **Phase 2 (T7–T12):** workspace model + virtual project + per-file parse cache + file watching
- Continue on branch `xs-language-server/redesign-phase-2` once this PR is merged.
