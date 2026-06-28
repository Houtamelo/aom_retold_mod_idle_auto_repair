# Tasks: xs-lsp-0.1.7-diagnostics-ux

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Est. changed lines | ~240 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR; work-unit commit per phase |
| Delivery strategy | single-pr |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Files | Est. lines |
|------|------|-------|------------|
| 1 | Issue #5: Engine definition returns null | `server.rs`, `lsp_roundtrip_test.rs` | ~30 |
| 2 | Issue #1: Stale-diagnostic clearing | `diagnostics.rs`, `server.rs` | ~35 |
| 3 | Issue #6: Actionable parse-error messages | `diagnostics.rs` | ~100 |
| 4 | Issue #4: Engine-symbol references | `server.rs`, `lsp_roundtrip_test.rs` | ~70 |
| 5 | Version bump & final verification | `gradle.properties` | ~5 |
| **Total** |  |  | **~240** |

## Phase 1: Issue #5 — Engine-symbol definition returns null

- [ ] 1.1 [RED] In `lsp_roundtrip_test.rs`, rename engine-definition test to expect `null` for `aiEcho`; run to failure.
- [ ] 1.2 [GREEN] In `server.rs::goto_definition`, return `Ok(None)` when ident resolves only via engine API (`find_syscall`/`find_aiplan`) and has no workspace definition.
- [ ] 1.3 [VERIFY] Run `cargo run --bin lsp_roundtrip_test`; confirm engine test passes and workspace definition returns a real `Location`.

## Phase 2: Issue #1 — Stale-diagnostic clearing

- [ ] 2.1 [RED] In `lsp_roundtrip_test.rs`, add `didChange` fixing last parse error and assert empty `publishDiagnostics`; run to failure.
- [ ] 2.2 [GREEN] In `diagnostics.rs::collect_all`, insert empty map entry for `current_uri` (`diags.entry(uri.clone()).or_default();`).
- [ ] 2.3 [REFACTOR] Update stale capability comments at `server.rs:239-250`.

## Phase 3: Issue #6 — Actionable parse-error messages

- [ ] 3.1 [RED] In `diagnostics.rs` unit tests, assert missing `;` message is `Missing ';'` and blacklist `MISSING`/`ERROR`/`node`/`column(s)`; run to failure.
- [ ] 3.2 [RED] Add tests for missing `}` and unexpected identifier `foo`.
- [ ] 3.3 [GREEN] Rewrite `diagnostics.rs::to_diagnostic` using `LookaheadNamesIterator` for `Missing '<token>'`/`Unexpected <kind> '<text>'`; fallback to line/column.
- [ ] 3.4 [VERIFY] Run unit tests and `AOMR_GAME_PATH=... cargo test --test game_folder_parse`.

## Phase 4: Issue #4 — Engine-symbol references

- [ ] 4.1 [RED] In `lsp_roundtrip_test.rs`, assert `aiEcho` references returns ≥2 workspace `Location`s; run to failure.
- [ ] 4.2 [GREEN] In `server.rs::references`, for engine-only idents walk workspace `.xs` files, collect uses via `find_identifier_uses`, dedupe with `seen`, honor `include_declaration`.
- [ ] 4.3 [GREEN] Enforce ≤500 ms total and ~50 ms per-file budgets; warn/truncate on overflow.
- [ ] 4.4 [VERIFY] Assert <500 ms and run `cargo run --bin lsp_roundtrip_test`.

## Phase 5: Version bump and final verification

- [ ] 5.1 [GREEN] Bump `pluginVersion` in `tools/intellij-xs-plugin/gradle.properties` to `0.1.7`.
- [ ] 5.2 [VERIFY] Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml` and `cargo run --bin lsp_roundtrip_test`; expect PASS.
- [ ] 5.3 [MANUAL] IDE smoke test: install plugin, open mod `.xs`, confirm `aiEcho` definition null, references show uses, clean file clears diagnostics.
