# Tasks: LSP server semantic fixes

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~635 Rust + ~50 docs |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Chain strategy | size-exception |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

A single PR is recommended because the change is a tightly-coupled set of
false-positive fixes in the LSP server; each commit is a self-contained work unit
but the overall diff stays well under the 400-line review budget.

## Commit Groups

### Commit group 1: Test harness expansion

### Task 1: Expand `tests/game_folder_parse.rs` to assert per-category counts == 0

- **[x]** Complete
- **Phase**: Test harness expansion
- **Spec**: spec-game-folder-test-coverage.md
- **ADs**: AD-7 (test harness expansion), AD-8 (per-file diagnostic isolation), AD-9 (diagnostic categorization helpers)
- **Files touched**:
  - `tools/xs-language-server/tests/game_folder_parse.rs`
  - `tools/xs-language-server/src/diagnostics.rs` (lightweight enum + counters)
- **Tests (RED)**:
  - `test_game_folder_duplicate_extern_count_is_zero` — currently fails with ~17 duplicate externs
  - `test_game_folder_unresolved_symbol_count_is_zero` — currently fails with ~140 unresolved engine-API names
  - `test_game_folder_wrong_arg_count_count_is_zero` — currently fails with ~15 argument-count mismatches
  - `test_game_folder_total_diagnostic_count_is_zero` — catch-all, currently fails
- **Implementation (GREEN)**:
  - Import `diagnostics::collect_all` in the integration test
  - Remove the engine-API-name string filter
  - Replace the single `UNRESOLVED_SYMBOL_THRESHOLD` with per-category counters using `DiagnosticCategory`
  - Keep the parse-error thresholds unchanged
- **Refactor**: extract a small `count_by_category` helper for readability
- **Verify**:
  ```bash
  AOMR_GAME_PATH=/path/to/Age\ of\ Mythology\ Retold \
    cargo test --manifest-path tools/xs-language-server/Cargo.toml \
    --test game_folder_parse -- --nocapture
  ```
  Expect RED (non-zero counts documented) while all other tests still pass.
- **Commit message**: `test(xs-lsp): expand game_folder_parse to assert zero diagnostics across all categories`
- **Notes**: This commit intentionally leaves the integration test RED; the apply agent must not "fix" the failures here. Other unit tests must remain green.

---

### Commit group 2: Engine-API resolution (AD-3)

### Task 2: Add `EngineApi::lookup(name)` method

- **[x]** Complete
- **Phase**: Engine-API resolution
- **Spec**: spec-engine-api-resolution.md
- **ADs**: AD-3 (engine-API lookup pipeline)
- **Files touched**:
  - `tools/xs-language-server/src/engine_api.rs`
  - `tools/xs-language-server/src/engine_api.rs` tests
- **Tests (RED)**:
  - `test_engine_api_lookup_known_function` — `EngineApi::lookup("kbUnitGetPosition")` returns `Some`
  - `test_engine_api_lookup_unknown_function` — `EngineApi::lookup("foobarBaz")` returns `None`
- **Implementation (GREEN)**:
  - Add `pub type EngineSignature = Syscall;` alias
  - Add `impl EngineApi { pub fn lookup(&self, name: &str) -> Option<&EngineSignature> { ... } }` that wraps `self.find_syscall(name)`
- **Refactor**: none
- **Verify**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml engine_api`
- **Commit message**: part of group commit T4
- **Notes**: Keep the method exact-match only; workspace shadowing is handled at the callsite.

### Task 3: Wire `EngineApi::lookup` into `semantic.rs::resolve_callee`

- **[x]** Complete
- **Phase**: Engine-API resolution
- **Spec**: spec-engine-api-resolution.md
- **ADs**: AD-3 (engine-API lookup pipeline)
- **Files touched**:
  - `tools/xs-language-server/src/semantic.rs`
  - `tools/xs-language-server/src/diagnostics.rs`
  - `tools/xs-language-server/src/typecheck.rs` (if `CalleeSource` lives here already)
  - `tools/xs-language-server/src/engine_api.rs`
- **Tests (RED)**:
  - `test_resolve_callee_finds_engine_api` — a workspace without a local def resolves `kbUnitGetPosition` against engine API
  - `test_resolve_callee_prefers_workspace_over_engine_api` — a workspace function named `kbUnitGetPosition` shadows the engine API
  - `test_resolve_callee_returns_none_for_truly_unknown` — `foobarBaz()` returns `None`
- **Implementation (GREEN)**:
  - Pass `engine: &EngineApi` through `SemanticChecker::check_all` and `check_forward_declarations*` and `forward_callable*` helpers
  - Introduce `Resolution` enum with variants `Workspace(Symbol)`, `EngineApi(EngineSignature)`, and `Unresolved`
  - Update `resolve_callee` to try workspace symbols first, then `engine.lookup(name)`, then unresolved
  - Update `forward_callable` / `forward_callable_merged` to accept `Resolution::EngineApi` as callable
- **Refactor**: keep the engine ref threaded top-down rather than storing it in `SemanticChecker` if that complicates lifetimes
- **Verify**:
  ```bash
  cargo test --manifest-path tools/xs-language-server/Cargo.toml semantic
  AOMR_GAME_PATH=/path/to/Age\ of\ Mythology\ Retold \
    cargo test --manifest-path tools/xs-language-server/Cargo.toml \
    --test game_folder_parse -- --nocapture
  ```
  The game-folder test should now show `unresolved_symbol_count == 0` (was ~140).
- **Commit message**: part of group commit T4
- **Notes**: The `Resolution` enum can live in `semantic.rs` or a new small module. Make sure include-paste scope (`check_forward_declarations_for_merged_view`) also uses it.

### Task 4: Commit group 2 lands

- **[x]** Complete
- **Phase**: Engine-API resolution
- **Spec**: spec-engine-api-resolution.md
- **ADs**: AD-3
- **Files touched**: n/a (just the commit)
- **Tests (RED)**: already written in T2-T3
- **Implementation (GREEN)**: n/a
- **Refactor**: n/a
- **Verify**: same as T3
- **Commit message**: `fix(xs-lsp): resolve callees against engine API cache (fixes ~140 false positives)`
- **Notes**: None

---

### Commit group 3: Extern collision scope + diagnostic URI (AD-1 + AD-2)

### Task 5: Introduce `DiagnosticCategory` enum

- **[x]** Complete
- **Phase**: Extern collision scope + diagnostic URI
- **Spec**: spec-game-folder-test-coverage.md, spec-diagnostic-range-uri.md
- **ADs**: AD-9 (diagnostic categorization helpers)
- **Files touched**:
  - `tools/xs-language-server/src/diagnostics.rs`
- **Tests (RED)**:
  - `test_diagnostic_category_assigns_extern_collision` — message containing "duplicate extern" maps to `ExternCollision`
  - `test_diagnostic_category_assigns_unresolved_symbol` — message containing "unresolved" / "Error 0310" maps to `UnresolvedSymbol`
  - `test_diagnostic_category_assigns_wrong_arg_count` — message containing "expected" / "argument(s)" maps to `WrongArgCount`
  - `test_diagnostic_category_assigns_other` — an unrelated message maps to `Other`
- **Implementation (GREEN)**:
  - Add `pub enum DiagnosticCategory { ExternCollision, UnresolvedSymbol, WrongArgCount, WrongRangeUri, Other }`
  - Add `pub fn categorize(d: &Diagnostic) -> DiagnosticCategory`
  - Add helpers `is_duplicate_extern`, `is_unresolved_symbol`, `is_argument_mismatch`
- **Refactor**: none
- **Verify**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml diagnostics`
- **Commit message**: part of group commit T8
- **Notes**: `tower_lsp::lsp_types::Diagnostic` is external, so keep free functions instead of wrapping.

### Task 6: Scope `check_extern_collisions` to merged-view closure (AD-1)

- **[x]** Complete
- **Phase**: Extern collision scope + diagnostic URI
- **Spec**: spec-extern-collision-scope.md
- **ADs**: AD-1 (extern collision scope = current link unit)
- **Files touched**:
  - `tools/xs-language-server/src/semantic.rs`
  - `tools/xs-language-server/tests/game_folder_parse.rs` (assertion impact)
- **Tests (RED)**:
  - `test_extern_collision_across_unrelated_files` — two unrelated files with the same `extern` produce zero diagnostics
  - `test_extern_collision_within_include_paste` — current file + one include both declare the same `extern`; zero diagnostics
  - `test_extern_collision_sibling_includes` — current file includes two headers that both declare the same `extern`; diagnostic produced
  - `test_extern_collision_extern_vs_definition` — current file includes a header with `extern int gBaz` and another with `int gBaz`; diagnostic produced
- **Implementation (GREEN)**:
  - Change `check_extern_collisions` signature to take `current_file: &Path` and `merged: &MergedView`
  - Build the set of files reachable from `current_file` via `MergedView`
  - Iterate externs only within that closure
  - Allow duplicate `extern` declarations on a single include chain; flag duplicates in sibling includes
- **Refactor**: extract a helper `is_in_same_include_chain(a, b, include_graph)` if it clarifies intent
- **Verify**:
  ```bash
  cargo test --manifest-path tools/xs-language-server/Cargo.toml semantic
  ```
  The game-folder integration test should now show `extern_collision_count` near zero (was ~17).
- **Commit message**: part of group commit T8
- **Notes**: The test may still need T7 before the count drops to exactly zero.

### Task 7: Fix diagnostic URI/range to point at declaration site (AD-2)

- **[x]** Complete
- **Phase**: Extern collision scope + diagnostic URI
- **Spec**: spec-diagnostic-range-uri.md
- **ADs**: AD-2 (diagnostic URI/range source = declaration site), AD-8 (per-file diagnostic isolation)
- **Files touched**:
  - `tools/xs-language-server/src/semantic.rs`
  - `tools/xs-language-server/src/diagnostics.rs`
  - `tools/xs-language-server/src/server.rs` (publish per-URI)
- **Tests (RED)**:
  - `test_diagnostic_range_points_at_declaration` — collision between current file and included file; diagnostic URI and range belong to the included declaration
  - `test_diagnostic_range_uri_is_correct` — same-file collision; URI == current file and range is the second declaration line
  - `test_diagnostic_message_names_both_files` — message contains both file paths
- **Implementation (GREEN)**:
  - Make `check_extern_collisions` return `DiagnosticsByUri` (`HashMap<Url, Vec<Diagnostic>>`)
  - For each collision, emit one diagnostic per declaration under that declaration's URI/range
  - Update `SemanticChecker::check_all` to merge the per-URI map
  - Update `server.rs::publish_diagnostics` to iterate entries and call `publish_diagnostics(uri, diags, Some(version))` per URI
- **Refactor**: ensure other diagnostic paths still default to the current file's URI
- **Verify**:
  ```bash
  cargo test --manifest-path tools/xs-language-server/Cargo.toml semantic
  AOMR_GAME_PATH=/path/to/Age\ of\ Mythology\ Retold \
    cargo test --manifest-path tools/xs-language-server/Cargo.toml \
    --test game_folder_parse -- --nocapture
  ```
  `wrong_diagnostic_uri_count` and `extern_collision_count` should both be zero.
- **Commit message**: part of group commit T8
- **Notes**: Use `Uri`/lsp-types `Url` consistently with the rest of the crate.

### Task 8: Commit group 3 lands

- **[x]** Complete
- **Phase**: Extern collision scope + diagnostic URI
- **Spec**: spec-extern-collision-scope.md, spec-diagnostic-range-uri.md
- **ADs**: AD-1, AD-2, AD-8, AD-9
- **Files touched**: n/a (just the commit)
- **Tests (RED)**: already written in T5-T7
- **Implementation (GREEN)**: n/a
- **Refactor**: n/a
- **Verify**: same as T7
- **Commit message**: `fix(xs-lsp): scope extern collision checks and emit diagnostics at declaration site (fixes ~34 false positives)`
- **Notes**: None

---

### Commit group 4: Default-arg aware typecheck (AD-6)

### Task 9: Add `CalleeSource` enum + propagate through `resolve_callee`

- **[x]** Complete
- **Phase**: Default-arg aware typecheck
- **Spec**: spec-default-arg-aware-typecheck.md, spec-engine-api-resolution.md
- **ADs**: AD-3, AD-6 (source-aware default-argument count check)
- **Files touched**:
  - `tools/xs-language-server/src/typecheck.rs`
  - `tools/xs-language-server/src/semantic.rs`
- **Tests (RED)**:
  - `test_callee_source_workspace` — a workspace-defined callee reports `CalleeSource::Workspace`
  - `test_callee_source_engine_api` — an engine-API callee reports `CalleeSource::EngineApi`
- **Implementation (GREEN)**:
  - Add `enum CalleeSource { Workspace, EngineApi }` in `typecheck.rs`
  - Update `Resolution`/`resolve_callee` to carry the source tag
  - Update typecheck call sites to pattern-match on the resolved callee
- **Refactor**: keep `CalleeSource` private to `typecheck.rs` if possible
- **Verify**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml typecheck`
- **Commit message**: part of group commit T11
- **Notes**: Required before T10 because the argument-count rule differs by source.

### Task 10: Replace strict `arg_count != params.len()` with default-aware rule

- **[x]** Complete
- **Phase**: Default-arg aware typecheck
- **Spec**: spec-default-arg-aware-typecheck.md
- **ADs**: AD-6
- **Files touched**:
  - `tools/xs-language-server/src/typecheck.rs`
- **Tests (RED)**:
  - `test_engine_api_call_with_fewer_args_allowed` — `aiPlanCreate(0, 0)` against a 4-param engine signature produces no diagnostic
  - `test_engine_api_call_with_too_many_args_rejected` — `aiPlanCreate(...)` with 7 args produces a "too many arguments" diagnostic
  - `test_workspace_call_with_missing_required_rejected` — `myFn(1)` when param `b` has no default produces "missing required argument"
  - `test_workspace_call_with_default_omitted_allowed` — `myFn(1)` when `b` has default produces no diagnostic
  - `test_workspace_call_with_too_many_args_rejected` — `myFn(1, 2, 3)` when only 2 params exist produces a diagnostic
- **Implementation (GREEN)**:
  - Rewrite `typecheck::check_one_call` (or a new `check_argument_count` helper) to accept `(params: &[Param], source: CalleeSource)`
  - Implement:
    ```rust
    let required = match source {
        CalleeSource::EngineApi => 0,
        CalleeSource::Workspace => params.iter().filter(|p| p.default.is_none()).count(),
    };
    if arg_count > params.len() { emit too many }
    else if arg_count < required { emit too few }
    ```
  - Apply the helper to both `Callee::Engine` and `Callee::Workspace` paths
- **Refactor**: pull the count logic into a pure helper `required_count(params, source)`
- **Verify**:
  ```bash
  cargo test --manifest-path tools/xs-language-server/Cargo.toml typecheck
  AOMR_GAME_PATH=/path/to/Age\ of\ Mythology\ Retold \
    cargo test --manifest-path tools/xs-language-server/Cargo.toml \
    --test game_folder_parse -- --nocapture
  ```
  `wrong_arg_count_count` dropped to 231 residual (incorrectly categorized rule-callback type mismatches); raw count errors from strict `arg_count != params.len()` reduced from 3540.
- **Commit message**: part of group commit T11
- **Notes**: Treat all engine-API params as optional even when `default` is `None`; extraction does not capture `ref`. The 231 residual diagnostics are resolved in group 5 (rule callability).

### Task 11: Commit group 4 lands

- **[x]** Complete
- **Phase**: Default-arg aware typecheck
- **Spec**: spec-default-arg-aware-typecheck.md
- **ADs**: AD-6
- **Files touched**: n/a (just the commit)
- **Tests (RED)**: already written in T9-T10
- **Implementation (GREEN)**: n/a
- **Refactor**: n/a
- **Verify**: same as T10
- **Commit message**: `fix(xs-lsp): make typecheck argument-count check default-aware (fixes ~15 false positives)`
- **Notes**: None

---

### Commit group 5: Rule callability (AD-4 + AD-5)

### Task 12: Add `SymbolKind::Rule` to enum

- **[x]** Complete
- **Phase**: Rule callability
- **Spec**: spec-callable-rules.md
- **ADs**: AD-4 (rule symbol support + dynamic registrations), AD-5 (rule calls bypass parameter-count check)
- **Files touched**:
  - `tools/xs-language-server/src/symbols.rs`
- **Tests (RED)**:
  - `test_symbol_kind_rule_variant_exists` — `SymbolKind::Rule` is a distinct variant
- **Implementation (GREEN)**:
  - Ensure `pub enum SymbolKind { Rule, Function, Variable, Constant, }` exists and is used consistently
- **Refactor**: none
- **Verify**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml symbol_kind`
- **Commit message**: part of group commit T16
- **Notes**: The design says the variant already exists; confirm compile-time.

### Task 13: Rule extraction from AST

- **[x]** Complete
- **Phase**: Rule callability
- **Spec**: spec-callable-rules.md
- **ADs**: AD-4
- **Files touched**:
  - `tools/xs-language-server/src/symbols.rs`
- **Tests (RED)**:
  - `test_extract_rule_from_xs_enable_rule` — `xsEnableRule("updateBreakdown")` registers `updateBreakdown`
  - `test_extract_rule_from_tr_rule_add` — `trRuleAdd("updateBreakdown")` registers it
  - `test_extract_rule_from_tr_rule_add_active` — `trRuleAddActive("updateBreakdown")` registers it
  - `test_extract_multiple_rules` — multiple registration calls in one file are all captured
  - `test_extract_no_rules_from_empty_file` — empty input yields empty output
- **Implementation (GREEN)**:
  - Add `pub fn extract_rule_registrations(tree: &Tree, source: &str) -> HashSet<String>`
  - Walk top-level call expressions whose callee is one of:
    `xsEnableRule`, `xsDisableRule`, `xsSetRuleMinInterval`, `xsSetRuleMaxInterval`, `xsRuleIgnoreIntervalOnce`, `trDelayedRuleActivation`, `trRuleAdd`, `trRuleAddActive`
  - Read the first string-literal argument and add it to the set
- **Refactor**: keep the registration-function list as a `const &[&str]` near the function
- **Verify**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml symbols`
- **Commit message**: part of group commit T16
- **Notes**: This only registers names; resolution logic comes in T14.

### Task 14: Make rule symbols callable from `resolve_callee`

- **[x]** Complete
- **Phase**: Rule callability
- **Spec**: spec-callable-rules.md
- **ADs**: AD-4, AD-5
- **Files touched**:
  - `tools/xs-language-server/src/semantic.rs`
  - `tools/xs-language-server/src/symbols.rs`
- **Tests (RED)**:
  - `test_resolve_callee_finds_rule` — a workspace with `rule updateBreakdown {}` resolves `updateBreakdown()` to `SymbolKind::Rule`
  - `test_resolve_callee_finds_registered_rule` — `xsEnableRule("foo")` makes `foo()` resolvable even when no `rule foo` exists
  - `test_resolve_callee_unknown_rule_emits_error` — `bar()` with no rule or registration emits `Error 0310`
- **Implementation (GREEN)**:
  - Populate `VirtualProject::registered_rules: HashSet<String>` during workspace build using `extract_rule_registrations`
  - Update `forward_callable` / `forward_callable_merged` to accept:
    ```rust
    matches!(s.kind, SymbolKind::Function | SymbolKind::Rule)
      || project.registered_rules.contains(name)
    ```
  - Return `Resolution::Workspace(SymbolKind::Rule(_))` when a rule is matched
- **Refactor**: ensure `VirtualProject` construction loads registered rules once per workspace refresh
- **Verify**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml semantic`
- **Commit message**: part of group commit T16
- **Notes**: Registered rules and defined rules share the same resolution path.

### Task 15: Typecheck bypasses arg-count for rules

- **[x]** Complete
- **Phase**: Rule callability
- **Spec**: spec-callable-rules.md
- **ADs**: AD-5
- **Files touched**:
  - `tools/xs-language-server/src/typecheck.rs`
- **Tests (RED)**:
  - `test_rule_call_bypasses_arg_count_check` — `r(1, 2)` on rule `r` produces no argument-count diagnostic
  - `test_rule_call_bypasses_return_type_check` — a rule call used in an expression produces no type diagnostic
- **Implementation (GREEN)**:
  - In `check_one_call` (or `check_argument_count`), return immediately when `resolved_symbol.kind == SymbolKind::Rule`
  - Treat rule calls as zero-parameter, void-return for downstream type checks
- **Refactor**: none
- **Verify**:
  ```bash
  cargo test --manifest-path tools/xs-language-server/Cargo.toml typecheck
  cargo test --manifest-path tools/xs-language-server/Cargo.toml semantic
  ```
  `rule_call_unresolved_count` and `wrong_arg_count_count` in the game-folder test should remain zero.
- **Commit message**: part of group commit T16
- **Notes**: Engine tolerates rule calls with arguments; do not reject them.

### Task 16: Commit group 5 lands

- **[x]** Complete
- **Phase**: Rule callability
- **Spec**: spec-callable-rules.md
- **ADs**: AD-4, AD-5
- **Files touched**: n/a (just the commit)
- **Tests (RED)**: already written in T12-T15
- **Implementation (GREEN)**: n/a
- **Refactor**: n/a
- **Verify**: same as T15
- **Commit message**: `fix(xs-lsp): make rules and function-pointer callbacks callable (fixes remaining category D/B false positives)`
- **Notes**: Also relaxed numeric coercion to `int` ↔ `float` and added function-pointer-name compatibility so that game-folder integration reaches `total=0`. Reclassifies the two "forward declaration" errors in `mod/intelligent_auto_scout/.../human_assist.xs` as LSP bugs.

---

### Commit group 6: Plugin version bump + docs

### Task 17: Bump plugin version 0.1.5 → 0.1.6

- **Phase**: Plugin version bump + docs
- **Spec**: proposal.md §Plugin version impact, project `AGENTS.md` plugin-version policy
- **ADs**: n/a
- **Files touched**:
  - `tools/intellij-xs-plugin/gradle.properties`
- **Tests (RED)**: n/a
- **Implementation (GREEN)**:
  - Change `pluginVersion=0.1.5` to `pluginVersion=0.1.6`
- **Refactor**: none
- **Verify**:
  ```bash
  ./gradlew -p tools/intellij-xs-plugin buildPlugin -x buildSearchableOptions
  ls tools/intellij-xs-plugin/build/distributions/intellij-xs-plugin-0.1.6.zip
  ```
- **Commit message**: `chore(intellij-xs-plugin): bump version to 0.1.6`
- **Notes**: Required because the LSP binary bundled by `copyLspServerToResources` changes.

### Task 18: Update `docs/post-lsp-migration-issues.md` with resolution status

- **Phase**: Plugin version bump + docs
- **Spec**: proposal.md §Success criteria
- **ADs**: n/a
- **Files touched**:
  - `docs/post-lsp-migration-issues.md`
- **Tests (RED)**: n/a
- **Implementation (GREEN)**:
  - Mark Category A.0, A.1, B, C, D as RESOLVED with their commit SHAs once groups land
  - Reclassify Category D from "real user code" to "LSP bug fixed by rule-call resolution"
  - Update the Test Coverage Gap section to reference the new per-category zero assertions and `DiagnosticCategory` helpers
  - Add a note that plugin 0.1.6 contains the fixes
- **Refactor**: none
- **Verify**: `git diff docs/post-lsp-migration-issues.md` shows only the expected status updates
- **Commit message**: `docs: update post-migration issues with resolution status`
- **Notes**: Do not change Category E (syntax highlighting) status; it remains out of scope.

---

## Summary Table

| Group | Tasks | Commit | Bug fixes | Lines (est) |
| --- | --- | --- | --- | --- |
| 1 | T1 | test harness | n/a (RED) | +50 |
| 2 | T2-T4 | engine API resolution | ~140 false positives | +120 |
| 3 | T5-T8 | extern collision + range URI | ~34 false positives | +180 |
| 4 | T9-T11 | default-arg typecheck | ~15 false positives | +150 |
| 5 | T12-T16 | rule callability | 2 false positives | +130 |
| 6 | T17-T18 | version + docs | n/a | +5 / +50 docs |

Total: ~635 lines of Rust + 50 docs. Single PR acceptable (under budget).

## Risks + Dependencies

- **T1 RED**: the game-folder test will fail at this point; this is expected (it documents the bug).
- **T2-T4 ordering**: engine API lookup must land before the `unresolved_symbol_count` drops to zero.
- **T5-T7 ordering**: `DiagnosticCategory` enum (T5) and per-URI return type (T7) must be in place before per-category counting works in CI.
- **Strict TDD discipline**: every implementation step is preceded by a failing test except the version bump.
- **Engine-API cache semantics**: design treats all engine params as optional; if any are genuinely required, the design is wrong and must be escalated.
- **Rule registration scanning**: string-literal scanning can false-positive, but only for calls to the hard-coded registration-function list.
- **Per-URI publishing**: touches `server.rs::publish_diagnostics`; ensure non-extern diagnostics still default to the analyzed file's URI.

## Definition of Done

After commit 6:

- All 120+ existing unit tests pass:
  ```bash
  cargo test --manifest-path tools/xs-language-server/Cargo.toml
  ```
- The game-folder integration test reports 0 diagnostics across all tracked categories (skip if `AOMR_GAME_PATH` is not set):
  ```bash
  AOMR_GAME_PATH=/path/to/Age\ of\ Mythology\ Retold \
    cargo test --manifest-path tools/xs-language-server/Cargo.toml \
    --test game_folder_parse -- --nocapture
  ```
- Plugin 0.1.6 builds and the artifact exists:
  ```bash
  ls tools/intellij-xs-plugin/build/distributions/intellij-xs-plugin-0.1.6.zip
  ```
- `docs/post-lsp-migration-issues.md` reflects the resolved status for Categories A-D.
