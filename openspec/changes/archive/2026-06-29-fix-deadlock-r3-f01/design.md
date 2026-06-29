# Design: `fix-deadlock-r3-f01` — Defensive lock-order refactor in LSP `did_close`

## Technical Approach

This change eliminates the only multi-mutex-hold pattern in `tools/xs-language-server/src/server.rs` and adds a regression test proving the new invariant. Strict TDD is the implementation strategy per the project override for tooling changes: write the failing invariant test, refactor, then add the doc-comment.

1. **`did_close` refactor**: wrap each `self.<field>.lock().await` in its own `{ }` block so every `MutexGuard` is dropped before the next `.await`. The current body holds `documents` across `symbol_tables` and into `clear_merged_view_cache`, which itself acquires `merged_views`. After the change:

   ```rust
   {
       let mut docs = self.documents.lock().await;
       docs.close(&uri);
   }
   {
       let mut tables = self.symbol_tables.lock().await;
       tables.remove(&uri);
   }
   self.clear_merged_view_cache(&uri).await;
   ```

2. **Contributor-facing doc-comment** on `pub struct XsLanguageServer` (`src/server.rs:42`) naming the scoped-block convention as the project's lock-acquisition discipline.

3. **Test extension** in `tests/r3_f01_deadlock_repro.rs`: add a focused invariant test that verifies the `did_close` sequence never holds 2+ mutexes at once. The three existing deadlock-reproduction tests stay unchanged as regression guards.

No shared XS mod files (`auto_scout.xs`, `auto_repair.xs`) are affected.

## Architecture Decisions

| Decision | Choice | Alternatives considered | Rationale |
|---|---:|---|---|
| Lock-acquisition pattern | Scoped-block release: each `let mut g = self.<mutex>.lock().await; …` pair lives in its own block; the guard drops before the next `.await`. | Single global lock-order document. | A convention document only records intent; the scoped pattern mechanically removes the hazard by dropping the guard. It also matches the existing code in `did_open`, `did_change`, and `get_or_build_merged_view`. |
| Doc-comment placement | On `pub struct XsLanguageServer` (`src/server.rs:42`). | Top-of-file comment. | IDE tooltips and rustdoc surface struct-level comments where contributors look first while adding new helper methods. |
| Test location | Extend `tests/r3_f01_deadlock_repro.rs`. | Create a new test file. | Preserves the existing R3-F-01 context and keeps the auditor's original regression tests alongside the new invariant test. |
| Invariant assertion mechanism | Test-only `InstrumentedMutex` wrapper around `tokio::sync::Mutex` with an `Arc<AtomicUsize>` counter and a RAII guard that increments on construction and decrements on `Drop`. | `try_lock_owned()` sampling between awaits. | The counter gives a deterministic, synchronous assertion at each step of the handler sequence and does not depend on scheduling luck. It exercises standard Rust `Drop`, `Arc`, and `AtomicUsize`. |

## Data Flow

```text
Pre-refactor did_close:
  documents.lock() ──► symbol_tables.lock() ──► await clear_merged_view_cache()
  ^ guard still held    ^ guard still held        ^ merged_views acquired inside

Post-refactor did_close:
  { documents.lock(); close(uri); }
              │
              ▼  guard dropped
  { symbol_tables.lock(); remove(uri); }
              │
              ▼  guard dropped
  await clear_merged_view_cache()  // only merged_views held briefly inside
```

## File Changes

| File | Action | Description |
|---|---|---|
| `tools/xs-language-server/src/server.rs` | Modify | `did_close` (`server.rs:406-414`): scope each `lock().await` acquisition. Add a doc-comment on `pub struct XsLanguageServer` documenting the single-mutex/scoped-block discipline. |
| `tools/xs-language-server/tests/r3_f01_deadlock_repro.rs` | Modify | Add a focused test function asserting that the `did_close` lock sequence never holds 2+ mutexes simultaneously. Existing tests remain. |

## Interfaces / Contracts

No new public interfaces, types, or protocols. The change is internal to an existing LSP handler and its regression test.

## Testing Strategy

Strict TDD per the tooling-domain override:

| Phase | What | Expected result |
|---|---|---|
| RED | Add invariant test asserting `did_close` holds ≤1 mutex. | Fails against the current implementation because `documents` and `symbol_tables` are held across the `merged_views` await. |
| GREEN | Refactor `did_close` into scoped blocks. | Invariant test passes. |
| REFACTOR | Add doc-comment on `XsLanguageServer`. | Full suite still green. |

Full verification:

| Layer | What to test | Approach |
|---|---|---|
| Unit / regression | Existing R3-F-01 deadlock reproductions | Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml` |
| Invariant | `did_close` never holds 2+ mutexes simultaneously | New test in `tests/r3_f01_deadlock_repro.rs` using `InstrumentedMutex` |
| Lint | No clippy regressions | `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml` |
| Integration | LSP roundtrip | Existing `lsp_roundtrip_test` / `cargo test` integration flows |

## Migration / Rollout

No migration required. This is a pure internal Rust refactor with no protocol, schema, cache-version, or deployed-mod behavior change.

## Open Questions

None.
