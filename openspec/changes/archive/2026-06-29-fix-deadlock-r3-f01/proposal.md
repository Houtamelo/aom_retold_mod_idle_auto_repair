# Proposal: `fix-deadlock-r3-f01` — Defensive lock-order refactor in LSP `did_close`

## Intent

Eliminate the only cyclic lock-order hazard in `tools/xs-language-server/src/server.rs`: `did_close` currently holds `documents` across `symbol_tables` and into `clear_merged_view_cache` (`merged_views`). Although the auditor confirmed this cycle is not currently reachable, scoping the guards removes the latent deadlock and aligns `did_close` with the rest of the handlers.

## Scope

### In Scope
- Scope lock guards in `did_close` (`tools/xs-language-server/src/server.rs:406-414`) so only one mutex is held at a time.
- Preserve and extend `tools/xs-language-server/tests/r3_f01_deadlock_repro.rs` to assert the new invariant.

### Out of Scope
- No LSP protocol, cache schema, or workspace-model changes.
- No changes to `did_change_watched_files`, `get_or_build_merged_view`, or other handlers (already scoped).
- No impact on deployed XS mod packages.

## Capabilities

### New Capabilities
None. This is a pure internal-defensive refactor with no new user-facing behavior.

### Modified Capabilities
None. Existing `spec-lsp-server-lifecycle.md` covers the affected domain but its requirements do not change.

## Approach

1. Wrap each lock acquisition in `did_close` in its own `{ }` block so guards are dropped before the next `.await`.
2. Add a contributor-facing doc-comment on `XsLanguageServer` documenting the scoped-lock convention.
3. **Strict TDD is active for this change** (override of global `strict_tdd: false`). The reproduction test will be updated to assert `did_close` never holds two mutexes across an `.await`, then the implementation will be adjusted. Test runner: `cargo test --manifest-path tools/xs-language-server/Cargo.toml`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `tools/xs-language-server/src/server.rs` | Modified | `did_close` lock scoping only. |
| `tools/xs-language-server/tests/r3_f01_deadlock_repro.rs` | Modified | New invariant assertion added. |
| `tools/xs-language-server/Cargo.lock` | None expected | No dependency changes. |
| `mod/*` | None | No impact on deployed XS mod packages. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Future helpers reintroduce multi-mutex hold | Low | Doc-comment on `XsLanguageServer` naming the scoped-block convention. |
| Refactor changes async timing, exposing latent bug | Very low | Full `cargo test` suite and LSP roundtrip test remain green. |

## Rollback Plan

`git revert <change-sha>` is fully safe. This is a pure Rust refactor with no data migration, protocol change, or schema change. Reverting restores the previous (latent but currently unreached) deadlock window.

## Dependencies

None.

## Success Criteria

- [ ] `cargo test --manifest-path tools/xs-language-server/Cargo.toml` passes.
- [ ] `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml` passes.
- [ ] The reproduction test is updated to assert `did_close` holds no more than one mutex across `.await` and passes.
- [ ] No regressions in existing LSP integration or roundtrip tests.
