## Verification Report

**Change**: fix-deadlock-r3-f01
**Mode**: Strict TDD (user override)

### Completeness
| Task | Status | Evidence |
|------|--------|----------|
| 1.1 — `InstrumentedMutex` helper added | [x] | `tests/r3_f01_deadlock_repro.rs:34-87` |
| 1.2 — Regression-control (unfixed pattern) | [x] | `tests/r3_f01_deadlock_repro.rs:307-329` |
| 1.3 — Control test passes | [x] | `cargo test --test r3_f01_deadlock_repro` green |
| 2.1 — `did_close` refactored into scoped blocks | [x] | `tools/xs-language-server/src/server.rs:481-491` delegates to helper |
| 2.2 — Production invariant test added | [x] | `tests/r3_f01_deadlock_repro.rs:334-371` |
| 2.3 — Full suite green | [x] | `cargo test` 198/198 passed |
| 3.1 — Lock-discipline doc-comment | [x] | `tools/xs-language-server/src/server.rs:111-116` |
| 3.2 — Tests green after doc | [x] | `cargo test` passed |
| 3.3 — Clippy introduces no new warnings | [x] | No warnings emitted in changed code region |
| 4.1 — R3-F-01 marked RESOLVED in review doc | [x] | `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md:25,93` |
| 4.2 — Last-verified line appended | [x] | Same doc, line 907 |

### Build / Tests / Coverage
| Command | Exit | Notes |
|---------|------|-------|
| `cargo test --manifest-path tools/xs-language-server/Cargo.toml` | 0 | 198 tests passing (182 unit + 9 game-folder integration + 7 deadlock-repro integration) |
| `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test r3_f01_deadlock_repro` | 0 | 7/7 passed |
| `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml --all-targets -- -D warnings` | 101 | Fails because unrelated pre-existing warnings are promoted to errors. The change itself adds no new warnings in the touched regions. |

### Spec Compliance Matrix
| Spec scenario | Covering test | Result |
|---------------|---------------|--------|
| Scenario 1 — handler releases each guard before acquiring the next | `production_did_close_holds_at_most_one_mutex` | PASS |
| Scenario 1 (triangulation) | `did_close_pattern_triangulates_with_populated_stores` | PASS |
| Scenario 2 — regression test rejects multi-mutex holds | `unfixed_did_close_holds_multiple_mutexes` | PASS |
| Scenario 2 (wrapper correctness) | `instrumented_mutex_counts_simultaneous_holds` | PASS |

### Correctness Table
| Concern from design | Implementation evidence | Result |
|---------------------|-------------------------|--------|
| Scoped-block pattern in `did_close` | `server.rs:93-101` (`did_close_lock_pattern`), `server.rs:481-491` (production delegate) | CONFORMS |
| Doc-comment on `XsLanguageServer` | `server.rs:111-116` | CONFORMS |
| `InstrumentedMutex` wrapper used in tests | `tests/r3_f01_deadlock_repro.rs:34-87` and 4 new tests | CONFORMS |
| Review doc updated with R3-F-01 RESOLVED | `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md:25,93,907` | CONFORMS |

### Design Coherence Table
| Decision | In implementation? |
|----------|---------------------|
| Lock-acquisition pattern (scoped blocks, one mutex at a time) | yes |
| Doc-comment placement (on `pub struct XsLanguageServer`) | yes |
| Test location (extend `r3_f01_deadlock_repro.rs`) | yes |
| Invariant assertion mechanism (`InstrumentedMutex` counter) | yes |

### Deviations
- Documented deviation: the helper items were exported as `#[doc(hidden)] pub` instead of `pub(crate)`. Integration tests are external crates and cannot call `pub(crate)` code; `#[doc(hidden)]` keeps the items out of rustdoc and signals they are not stable API. Acceptable per apply-progress.

### Issues
CRITICAL: None
WARNING: `cargo clippy --all-targets -- -D warnings` exits non-zero because the project already contains many unrelated clippy findings that are promoted to errors by `-D warnings`. The change introduces no new warnings in the modified code regions, but any CI job using `-D warnings` will fail until the legacy findings are addressed.
SUGGESTION: None

### Strict TDD Compliance

#### TDD Compliance
| Check | Result | Details |
|-------|--------|---------|
| TDD Evidence reported | ✅ | Found in apply-progress (`sdd/fix-deadlock-r3-f01/apply-progress`) |
| All tasks have tests | ✅ | 6/6 applicable tasks have test files; 5 doc/lint tasks are N/A by design |
| RED confirmed (tests exist) | ✅ | All RED-tagged test files exist in the codebase |
| GREEN confirmed (tests pass) | ✅ | All reported tests pass on execution (198/198) |
| Triangulation adequate | ✅ | Tasks 2.1/2.2 triangulated with populated stores; single-case tasks match their single scenario |
| Safety Net for modified files | ✅ | Original 3 deadlock-repro tests still pass; full suite green before/after change per apply-progress |

**TDD Compliance**: 6/6 applicable checks passed

---

#### Test Layer Distribution
| Layer | Tests | Files | Tools |
|-------|-------|-------|-------|
| Unit | 182 | `src/*.rs` | `cargo test` |
| Integration | 16 | `tests/game_folder_parse.rs`, `tests/r3_f01_deadlock_repro.rs` | `cargo test` |
| E2E | 0 | — | — |
| **Total** | **198** | **2 integration files + lib sources** | |

---

#### Changed File Coverage
Coverage analysis skipped — no coverage tool detected for the Rust LSP crate.

---

#### Assertion Quality
**Assertion quality**: ✅ All assertions verify real behavior

The new tests assert on:
- the simultaneous-guard counter of `InstrumentedMutex`,
- the explicit multi-mutex count in the unfixed regression control,
- the maximum guard count observed during the production `did_close_lock_pattern`, and
- the actual side effects of `close`/`remove`/`clear_cache` in the triangulation case.
No tautologies, empty-collection checks, type-only assertions, or smoke-only assertions were found.

---

#### Quality Metrics
**Linter**: ⚠️ Pre-existing clippy findings are promoted to errors under `-D warnings`; no new findings in changed code regions.
**Type Checker**: ✅ No errors — the crate compiles and all tests pass.

### Final Verdict
PASS WITH WARNINGS
