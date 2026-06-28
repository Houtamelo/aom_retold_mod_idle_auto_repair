# Change Proposal: LSP server semantic fixes

## 1. Title

LSP server semantic fixes — eliminate false-positive diagnostics on the official AoM:R `game/**/*.xs` scripts.

## 2. Why (Problem statement)

After the LSP platform API migration landed plugin **0.1.5** (commit `2d882b1`), the IDE correctly surfaces LSP diagnostics in Rider/IDEA. However, the first smoke test on the official game scripts reported **~189 false-positive diagnostics** across four categories, even though the exact same files compile and load cleanly under the AoM:R engine's XS compiler.

The user's goal for this change is:

> Make the LSP lint `~/.steam/steam/steamapps/common/Age of Mythology Retold/game/**/*.xs` without **any** errors. Those scripts are accepted by the engine; the LSP must also accept them.

The false-positive categories are:

| Category | Count | Symptom | Root cause |
|----------|------:|---------|------------|
| **A.0** | ~17 | Duplicate `extern` diagnostics for globals declared in independent files | `check_extern_collisions` scans the whole virtual project instead of a single include closure |
| **A.1** | ~17 | Diagnostic range lands on a comment line in the open file, not on the declaration | Wrong URI is attached when a collision is reported |
| **B** | ~140 | `Error 0310: invalid symbol lookup 'xsSetContextPlayer'` | `semantic.rs` resolves callees only against workspace symbols, ignoring the already-loaded engine API |
| **C** | ~15 | `expected 4 argument(s) to aiPlanCreate, got 2` | `typecheck.rs` compares `arg_count` to `params.len()`, but XS requires every parameter to have a default value |
| **D** | 2 | `'updateBreakdown' used at line 394 before declaration` | Symbols defined as `rule` (not `function`) are rejected by callee resolution, although the engine allows calling rules like functions |

## 3. What changes

This change is strictly server-side and semantic. It does **not** touch XS mod source files and does **not** re-extract engine API data from `doxygen_retail.7z`; the engine-API cache already contains the symbols we need.

The fixes are:

1. **A.0 / A.1 — Duplicate `extern` false positives**  
   Make `check_extern_collisions` operate on a single link unit (the current file plus its transitive includes). If two unrelated files declare the same global `extern`, that is normal XS and must not be flagged. When a collision is real, publish the diagnostic on the declaration's URI and range, not on the include directive's file.

2. **B — Missing engine API symbols**  
   `semantic.rs` must union workspace symbols with engine-API symbols from the cache before emitting `Error 0310`. Callee resolution in both direct and include-paste scopes must accept engine-API names.

3. **C — Default-aware argument count**  
   `typecheck.rs` must count only parameters without documented default values as required. Calls with fewer args than required are errors; calls with extra args are errors; calls that omit trailing default args are valid.

4. **D — Rules are callable**  
   Introduce first-class `SymbolKind::Rule` support in callee resolution. Rules have no parameters and no return type, so `typecheck.rs` must not report count or type mismatches for rule calls.

5. **Test coverage**  
   `tests/game_folder_parse.rs` must stop filtering out engine-API names and stop ignoring `typecheck.rs`. It will assert **zero** diagnostics in each category and zero total diagnostics across the full game folder.

## 4. Goals (outcome-based)

- `cargo test --test game_folder_parse -- --nocapture` reports **0 diagnostics** across all four categories on the full official game tree (302 parseable `.xs` files).
- All 120+ existing Rust unit tests continue to pass.
- Plugin **0.1.6** installs in Rider/IDEA and shows zero errors when opening any `game/**/*.xs` file.

## 5. Non-goals

| Out of scope | Rationale |
|--------------|-----------|
| **Category E: syntax highlighting** | IDE-side stub PSI / TextMate bundle issue, not driven by the LSP server |
| Engine-API doxygen extraction improvements | We already have the needed symbols in the cache; only consumption changes |
| Mod-side scripts as the verification gate | The success criterion is the official `game/**/*.xs` tree. The same fixes will help mod files, but they are not the gate |
| Adding strict type inference beyond argument counts | Existing behavior for provided-argument type checks is unchanged |
| New completions, hovers, or code actions | Only diagnostic false positives are addressed |

## 6. Approach (high-level)

### A.0 — Scope duplicate-extern checks to a single link unit

`check_extern_collisions` currently iterates over every file in the virtual project. Replace that with a per-link-unit scan: for each top-level file, collect the set of files reachable through `#include`, then detect duplicate `extern` declarations only within that closure. Independent headers that declare the same global remain legal.

### A.1 — Publish duplicate-extern diagnostics on the declaration's URI

When a real collision is found, build one diagnostic per declaration, using each declaration's own URI and source range. If the current open file needs a visible marker, use `related_information` to point to the declarations instead of misattributing the range.

### B — Union workspace symbols with engine API on callee lookup

`check_forward_declarations`, `check_forward_declarations_for_merged_view`, `forward_callable`, and `forward_callable_merged` must all accept a name resolved from `EngineApi` as a valid callee. The engine-API cache is already loaded at server startup; pass it into `diagnostics::collect_all` and through to semantic checks.

### C — Default-aware argument-count check

Replace the current fixed-length check in `typecheck::check_one_call` with a count of non-defaulted parameters:

```rust
let required_count = params
    .iter()
    .filter(|p| p.default.is_none() && !p.is_ref)
    .count();

if arg_count > params.len() {
    emit(Diagnostic::TooManyArguments { ... });
} else if arg_count < required_count {
    emit(Diagnostic::TooFewArguments { ... });
}
```

Parameters with a documented default value may be omitted. Parameters with no documented default must be supplied.

### D — Treat `SymbolKind::Rule` as callable

Rules already get their own `SymbolKind::Rule` from `symbols::extract_rule`. Make callee resolution treat a rule as a zero-parameter, void-return callable:

```rust
fn is_callable(s: &Symbol) -> bool {
    matches!(s.kind, SymbolKind::Function | SymbolKind::Rule)
}
```

In `typecheck.rs`, skip argument-count and return-type checks when the resolved callee is a rule.

### Test coverage expansion

Convert `tests/game_folder_parse.rs` from a filtered single-threshold check to a real end-to-end diagnostic gate:

- Load `EngineApi` once using the existing cache helper.
- Run `diagnostics::collect_all` for each top-level file (includes parse, typecheck, and semantic checks).
- Count diagnostics by stable category helpers in `diagnostics.rs` (`is_unresolved_symbol`, `is_duplicate_extern`, `is_argument_mismatch`).
- Assert each category count is **0** and total diagnostic count is **0**.
- Add focused regression tests for default args, rule calls, duplicate extern in a single include closure, and engine-API resolution.

## 7. Affected files

| File | Function / area | Lines | Change |
|------|-----------------|-------|--------|
| `tools/xs-language-server/src/semantic.rs` | `check_extern_collisions` | 194–273 | Scope to current link unit; skip unrelated-file collisions |
| `tools/xs-language-server/src/diagnostics.rs` | `collect_all` | 40–78 | Pass `EngineApi` into semantic checks; expose category helpers |
| `tools/xs-language-server/src/diagnostics.rs` | Diagnostic range building | 1–118 | Attach declaration URI/range to duplicate-extern diagnostics |
| `tools/xs-language-server/src/semantic.rs` | `check_forward_declarations` | 277–347 | Accept engine-API names as resolved |
| `tools/xs-language-server/src/semantic.rs` | `check_forward_declarations_for_merged_view` | 406–476 | Accept engine-API names as resolved in include-paste scope |
| `tools/xs-language-server/src/semantic.rs` | `forward_callable_merged` | 528–546 | Union workspace + engine-API + rule symbols |
| `tools/xs-language-server/src/semantic.rs` | `forward_callable` | 583–625 | Union workspace + engine-API + rule symbols |
| `tools/xs-language-server/src/typecheck.rs` | `check_one_call` | 75–152 | Use required-parameter count instead of `params.len()` |
| `tools/xs-language-server/src/typecheck.rs` | `Callee::params` / `Syscall` | 168–182 | Read `default` field for optionality |
| `tools/xs-language-server/src/symbols.rs` | `extract_rule` / `SymbolKind` | 169–190 | Ensure `SymbolKind::Rule` is distinct and complete |
| `tools/xs-language-server/src/engine_api.rs` | Helpers | TBD | Small helpers to lookup a callable by name + parameter defaults |
| `tools/xs-language-server/tests/game_folder_parse.rs` | `analyze_top_level_unresolved` | 418–515 | Run full `diagnostics::collect_all` instead of filtered semantic-only |
| `tools/xs-language-server/tests/game_folder_parse.rs` | Thresholds / assertions | 386–402, 517–558 | Per-category thresholds and final `total == 0` gate |
| `tools/intellij-xs-plugin/gradle.properties` | `pluginVersion` | 1 line | Bump `0.1.5` → `0.1.6` |

## 8. Risks & open questions

| Risk / unknown | Impact | Mitigation |
|----------------|--------|------------|
| Doxygen default-value extraction is incomplete for some parameters | Some optional parameters may be treated as required, leaving lingering Category C false positives | Treat absence of a documented default conservatively. We must not relax below engine semantics, so a false negative is preferable to a false positive |
| `useSimpleNatureUnitQuery` is workspace-defined, not engine-API | If the include-paste scope misses it, Category B may still flag it | Verify it resolves through the normal include closure before declaring Category B fixed |
| Cross-file `extern` collision semantics are not exhaustively documented | The A.0 fix might miss a true collision that the engine rejects | Err on the side of fewer diagnostics. Add a regression test that includes the same extern header twice and verify behavior |
| New default-arg rule is a behavior change for mod scripts | Mod call sites that rely on undocumented parameter omission could newly see diagnostics | Engine semantics are the source of truth; if a call truly compiles, a default value must exist somewhere. Keep an allow-list escape hatch only for symbol names proven to compile without documented defaults |
| Full `diagnostics::collect_all` on 149 top-level files may be slow | CI or local test runtime regression | Reuse the existing engine-API cache helper; profile before spec lock |
| Some `ERROR` parse nodes (156 today) may hide call sites | Category B/C fixes could be under-verified if parser drops expressions | The integration test already tolerates these. Verify specific examples listed in the explore report are parsed as call expressions |

## 9. Success criteria

1. `cargo test --test game_folder_parse -- --nocapture` produces **0** unresolved-symbol, **0** duplicate-extern, **0** argument-mismatch, and **0** total diagnostics on `~/.steam/steam/steamapps/common/Age of Mythology Retold/game/**/*.xs`.
2. All 120+ Rust unit tests pass (`cargo test`).
3. Four focused regression tests pass:
   - Default args: `void f(int x = -1, int y = -1) {}` called as `f(1)` has no diagnostics.
   - Rule call: `rule r {}` called as `r();` has no diagnostics.
   - Duplicate extern: two unrelated files declaring the same `extern` produce no collision.
   - Engine API: a call to `xsSetContextPlayer(0)` produces no `Error 0310`.
4. Migration smoke-test **T16** shows zero server-side errors in Rider/IDEA on official game scripts.
5. Plugin version is bumped to **0.1.6**.

## 10. Plugin version impact

Per the project `AGENTS.md` plugin-version policy, any commit that changes the bundled LSP binary or plugin code requires a version bump. This change alters the Rust LSP server, which is bundled into the plugin JAR by `copyLspServerToResources`.

- **Bump**: `0.1.5` → `0.1.6`
- **Severity**: patch
- **Reason**: bug fix, no new user-facing features, no API break

## 11. PR strategy

- **Forecast**: single PR unless the final diff exceeds **~800 lines**; then switch to a chained PR.
- **Commit strategy** (work-unit-commits, each leaving tests green):
  1. `test: expand game_folder_parse thresholds for all diagnostic categories`
  2. `fix: scope duplicate-extern collisions to link unit and correct diagnostic URI`
  3. `fix: resolve engine-API names in semantic callee checks`
  4. `fix: default-aware argument count in typecheck`
  5. `fix: allow rule symbols to satisfy call expressions`
  6. `chore: bump plugin version to 0.1.6`
- **Review focus**: Request review on the semantic/`EngineApi` integration in `semantic.rs` and the default-arg math in `typecheck.rs`.

---

**Ready for spec:** Yes. The next phase should produce per-category delta specs before design/apply.
