## Verification Report

**Change**: `fix-lsp-symbols-cleanup`
**Mode**: Strict TDD (tooling-domain override)
**Verifier commands run**: 2026-06-29

### Completeness

| Task / gate | Status | Evidence |
|-------------|--------|----------|
| `is_false` helper removed and serde predicate inlined | ✅ | `tools/xs-language-server/src/symbols.rs:76` — `#[serde(default, skip_serializing_if = "std::ops::Not::not")]` |
| `Param::render` method and empty `impl Param` block removed | ✅ | No `fn render` or `Param::render` call sites in `src/symbols.rs`; `grep` returns only comment references in the audit test file |
| Misleading test renamed | ✅ | `tools/xs-language-server/src/symbols.rs:792` — `fn function_definition_with_inner_error_in_default_value_still_extracts` |
| Audit tests inverted to strict-TDD post-fix contracts | ✅ | `tools/xs-language-server/tests/symbols_cleanup_repro.rs:13` (`param_render_no_callers_in_production_source`) and `:54` (`function_definition_with_inner_error_in_default_value_still_extracts_test_renamed`) |
| R1-F-02 / R1-F-03 regression tests preserved | ✅ | `tests/symbols_cleanup_repro.rs:89,138,180,212` all pass |
| Full LSP test suite green | ✅ | `cargo test` — 209 passed, 0 failed |
| Build with tests clean | ✅ | `cargo build --tests` succeeds (one pre-existing dead-code warning in `bin/lsp_roundtrip_test.rs`) |
| No new clippy warnings | ✅ | Warning set identical to apply-phase baseline (`/tmp/clippy_batch_b.txt`) |

### Build / Tests / Coverage

| Command | Exit | Notes |
|---------|------|-------|
| `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast` | 0 | **209 tests passed, 0 failed**: 182 unit + 9 game-folder + 7 deadlock-repro + 5 R5 honesty + 6 symbols-cleanup |
| `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test symbols_cleanup_repro` | 0 | 6/6 passed |
| `cargo build --manifest-path tools/xs-language-server/Cargo.toml --tests` | 0 | Clean build; pre-existing warning `constant DID_CHANGE is never used` in unrelated binary |
| `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml --tests` | 0 | Same warning kinds/counts as apply baseline; no new warnings in changed regions |

### Spec Compliance Matrix

| Spec scenario | Covering test | Result |
|---------------|---------------|--------|
| **Scenario 1** — dead public method removed on discovery; helpers deleted/inlined; audit tests inverted | `param_render_no_callers_in_production_source` | PASS |
| **Scenario 1** (source-level absence) | `grep` shows no `fn render` / `.render(` / `Param::render` in production source | PASS |
| **Scenario 2** — misleading test label renamed on discovery | `function_definition_with_inner_error_in_default_value_still_extracts_test_renamed` | PASS |
| **Scenario 2** (actual code path still exercised) | `fn function_definition_with_inner_error_in_default_value_still_extracts` in `src/symbols.rs` | PASS |

### Correctness Table

| Concern from design | Implementation evidence | Result |
|---------------------|-------------------------|--------|
| `is_false` inlined via `std::ops::Not::not` | `src/symbols.rs:74-77` | CONFORMS |
| `Param::render` no longer exists | No matches in `src/symbols.rs`; `grep` for `.render(` and `Param::render` in `src/` returns nothing | CONFORMS |
| Test renamed and comment updated | `src/symbols.rs:791-805` new name + explanatory comment | CONFORMS |
| JSON shape of `Param::is_ref` unchanged | `serde` annotation still skips `false`; serialization behavior asserted indirectly by existing parser/symbol round-trips passing | CONFORMS |

### Design Coherence Table

| Decision | In implementation? |
|----------|---------------------|
| ADR-1 — inline `is_false` via `std::ops::Not::not` | yes |
| ADR-2 — rename misleading test instead of deleting it | yes |
| ADR-3 — invert audit tests to strict-TDD contracts | yes (`param_render_no_callers_in_production_source`, rename contract, R1-F-02/R1-F-03 tests preserved) |
| ADR-4 — no public API change | yes (`Param` struct fields unchanged; JSON shape unchanged) |

### Acceptance Criteria

From `specs/spec-lsp-symbol-honesty.md` § Acceptance criteria:

1. **`Param::render` no longer exists in `tools/xs-language-server/src/symbols.rs`.**
   - ✅ Verified. `grep -n "fn render\|Param::render" src/symbols.rs` returns no matches. `param_render_no_callers_in_production_source` passes.

2. **The `is_false` helper is deleted or inlined; `Param::is_ref`'s `#[serde(skip_serializing_if = ...)]` annotation compiles to equivalent behavior.**
   - ✅ Verified. `grep -n "fn is_false\|is_false" src/symbols.rs` returns no matches. Annotation is `skip_serializing_if = "std::ops::Not::not"` at `src/symbols.rs:76`. Full test suite (209 tests) passes, confirming serialization round-trips are intact.

3. **The misleading test `recovers_function_definition_from_error_node_with_body` is renamed to reflect actual behavior OR removed (with the regular-path coverage already existing in the test file).**
   - ✅ Verified. Old name absent from `src/symbols.rs`; new name `function_definition_with_inner_error_in_default_value_still_extracts` present at line 792. Rename contract passes.

### Spec Scenarios

- **Scenario: dead public method is removed on discovery**
  - GIVEN a dead/contradictory `pub fn` with zero callers.
  - WHEN surfaced by audit.
  - THEN the method and its helpers are deleted/inlined and audit tests are inverted.
  - **Coverage**: `Param::render` and `is_false` are gone; the strict-TDD test `param_render_no_callers_in_production_source` asserts the post-fix state. **PASS**.

- **Scenario: misleading test label is renamed or removed on discovery**
  - GIVEN a test whose name describes a different code path than it exercises.
  - WHEN surfaced by audit.
  - THEN the test is renamed to match its actual path or removed if already covered.
  - **Coverage**: `recovers_function_definition_from_error_node_with_body` → `function_definition_with_inner_error_in_default_value_still_extracts`; `function_definition_with_inner_error_in_default_value_still_extracts_test_renamed` asserts both the new name presence and old name absence. **PASS**.

### Deviations

- None from the spec or design.
- Apply phase documented a substitution: strict-TDD **compile-fail** contracts for `Param::render` absence were replaced with **runtime source-grep** contracts. This is a mechanical necessity (a compile-fail test for a deleted method would break compilation for the entire test crate once the method is gone). The verification result is equivalent: dead method absence is asserted and runtime callers are confirmed to be zero.

### Issues

- **CRITICAL**: None.
- **WARNING**: The project retains pre-existing clippy warnings (same set as apply baseline). The change introduces no new warnings in the modified regions, but a CI job using `cargo clippy -- -D warnings` will fail until legacy findings are addressed.
- **SUGGESTION**: None.

### Risks / Warnings

1. **Pre-existing clippy warnings remain unchanged.** No new warnings were introduced by this change, but the existing warning baseline still exists (e.g., unused constants, derivable impls, deprecated LSP fields).
2. **Runtime grep substitutes for compile-fail contract.** As noted in the apply progress, the absence of `Param::render` is enforced by `param_render_no_callers_in_production_source`, which greps source files. This catches accidental re-introduction of callers but is not a compiler-level guarantee.

### Final Verdict

**PASS WITH WARNINGS**

All acceptance criteria and spec scenarios are satisfied, the full test suite passes (209/209), and no new clippy warnings were introduced. The only reservation is the unchanged pre-existing clippy warning baseline, which is unrelated to this change.

### Next Recommended Phase

`sdd-archive`
