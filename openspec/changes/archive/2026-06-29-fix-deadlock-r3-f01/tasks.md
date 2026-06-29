# Tasks: fix-deadlock-r3-f01 — Defensive lock-order refactor in LSP `did_close`

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~50 |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | auto-forecast |
| Chain strategy | pending |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | Scope guards in `did_close` and add invariant tests | PR 1 | Against `main`; includes doc-comment and review-report update |

## Phase 1: RED — Write the failing invariant test

- [x] 1.1 — Add `InstrumentedMutex<T>` helper to `tools/xs-language-server/tests/r3_f01_deadlock_repro.rs`: wraps `tokio::sync::Mutex<T>` and increments an `Arc<AtomicUsize>` on `lock()` acquisition; the returned RAII guard decrements on `Drop`.
- [x] 1.2 — Add regression-control test that mirrors today's **unfixed** `did_close` sequence (hold `documents`, then `symbol_tables`, then await on `merged_views`) with `InstrumentedMutex`. Assert the multi-mutex regression is caught as counter >= 2.
- [x] 1.3 — Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r3_f01_deadlock_repro -- --nocapture`. Confirm the control passes; the post-refactor invariant assertion is verified in Phase 2.

## Phase 2: GREEN — Refactor `did_close`

- [x] 2.1 — Modify `async fn did_close` in `tools/xs-language-server/src/server.rs` so each `lock().await` lives inside its own scoped block via the extracted `did_close_lock_pattern` helper; the `MutexGuard` drops before the next `.await`.
- [x] 2.2 — Add `#[tokio::test(flavor = "multi_thread")] async fn production_did_close_holds_at_most_one_mutex()` to `tests/r3_f01_deadlock_repro.rs`: call the production `did_close_lock_pattern` helper with `InstrumentedMutex`; assert the counter never exceeds 1.
- [x] 2.3 — Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml -- --nocapture`. Confirm all green and the new invariant tests pass, including the existing three deadlock-reproduction tests.

## Phase 3: REFACTOR — Add discipline doc-comment

- [x] 3.1 — Add a rustdoc comment on `pub struct XsLanguageServer` naming the single-mutex / scoped-block lock discipline.
- [x] 3.2 — Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml`. Confirm green.
- [x] 3.3 — Run `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml --all-targets -- -D warnings`. No new warnings introduced by this change (project has pre-existing clippy findings outside the touched files).

## Phase 4: VERIFY — Documentation and archival

- [x] 4.1 — Update `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` to mark finding **R3-F-01** as `RESOLVED` with a one-line reference to `openspec/changes/fix-deadlock-r3-f01/`.
- [x] 4.2 — Append `Last verified: 2026-06-29 against commit <sha>` to the bottom of the review report, substituting the commit SHA after the task set is applied.
