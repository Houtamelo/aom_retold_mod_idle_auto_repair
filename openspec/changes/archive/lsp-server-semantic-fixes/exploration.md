# SDD Explore: `lsp-server-semantic-fixes`

## 1. Context

The `intellij-xs-plugin-platform-lsp-migration` SDD change landed plugin 0.1.5 and fixed three IDE-side integration bugs, but the T16 Rider smoke test revealed that the **LSP server** emits false-positive diagnostics on real AoM:R XS source. The user's goal for this follow-up change is:

> Make the LSP lint the official game's entire `*.xs` scripts without any errors. Those scripts compile cleanly under the engine's XS compiler; the LSP needs to do the same.

This document captures the root-cause investigation for every diagnostic category, the exact source locations that must change, and the test-coverage gaps that prevented the existing suite from catching the bugs.

### Baseline captured

```bash
AOMR_GAME_PATH=/home/houtamelo/.steam/steam/steamapps/common/Age\ of\ Mythology\ Retold \
  cargo test --manifest-path tools/xs-language-server/Cargo.toml \
    --test game_folder_parse -- --nocapture 2>&1 | tee /tmp/lsp-smoke-baseline.log
```

Result (4/4 tests pass):

- Workspace resolved **302 parseable `.xs` files** (20 binary `.xs` skipped under `random_maps/`).
- Parser produced **6,045 symbols** and **156 unexpected `ERROR` nodes** (within existing thresholds).
- Semantic pipeline reported **0 unresolved-symbol diagnostics** (after the integration test manually filters out engine-API names).

**Critical caveat:** the test only exercises `SemanticChecker::check_all`, which runs `semantic.rs`. It does **not** run `typecheck::check_calls_with_merged` or count duplicate-extern / argument-mismatch diagnostics, so the "0 unresolved" headline masks the bugs described below.

---

## 2. Bug Inventory

| Category | Count (smoke test) | Example message | Root cause | Status |
|----------|-------------------:|-----------------|------------|--------|
| **A.0** Duplicate extern false positives | ~17 | `duplicate extern: 'gUtilitiesCategoryID' is declared extern in ...` | `semantic::check_extern_collisions` flags duplicate `extern` declarations across *unrelated* files in the same virtual project. | False positive |
| **A.1** Wrong diagnostic location for duplicate extern | ~17 | Diagnostic shown on line 7 of `human_assist.xs` (a comment) | `check_extern_collisions` emits diagnostics on the current-file URI even though the symbol is declared in an included file. | False positive + wrong range |
| **B** Missing engine API symbols (Error 0310) | ~140 | `Error 0310: invalid symbol lookup 'xsSetContextPlayer'` | `semantic::check_forward_declarations*` resolves callees only against workspace symbols; it ignores engine-API syscalls. Most listed names **are** present in `doxygen_retail.7z` extraction. | False positive |
| **C** Wrong argument count | ~15 | `expected 4 argument(s) to aiPlanCreate, got 2` | `typecheck::check_one_call` uses `arg_count != params.len()` instead of checking against non-defaulted parameters. XS requires every parameter to have a default value, so trailing args can be omitted. | False positive |
| **D** Forward declarations for `updateBreakdown` / `applyDistribution` | 2 | `'updateBreakdown' used at line 394 before declaration` | These symbols are **XS rules**, not functions. The engine allows calling a rule like a function. `semantic.rs` only resolves `SymbolKind::Function`, so rules are unresolved/forward-decl failures. | **Reclassified as LSP bug** |
| **E** No syntax highlighting | — | Plain text in editor | Likely IDE-side stub PSI / TextMate bundle loading; out of scope for the LSP server change. | Out of scope |

### A.0 / A.1 evidence

In the vanilla game tree:

- `game/ai/human_assist/human_assist_debug.xs:4` declares `extern int gUtilitiesCategoryID = -1;`
- `game/ai/core/utilities/debug.xs:4` declares the same `extern int gUtilitiesCategoryID = -1;`

The mod's `human_assist.xs` directly includes only `human_assist/human_assist_debug.xs` (line 11), not `core/utilities/debug.xs`. Because `semantic::check_extern_collisions` iterates over **every file in the virtual project**, it sees the two independent declarations and flags a collision even though they are separate headers for different consumers.

### B evidence

The engine-API cache (keyed by SHA-256 of `docs/doxygen_retail.7z`) contains the symbols flagged as "missing":

| Symbol | In `doxygen_retail.7z` cache? |
|--------|------------------------------|
| `xsSetContextPlayer` | Yes |
| `xsGetTime` | Yes |
| `kbUnitGetPlanID` | Yes |
| `aiPlanDestroy` | Yes |
| `round` | Yes |
| `min` | Yes |
| `max` | Yes |
| `useSimpleNatureUnitQuery` | **No** — but it is a workspace function defined in `game/ai/human_assist/human_assist_unit_queries.xs`, so it should resolve through the include-paste scope. |

The ~140 "Error 0310" diagnostics are therefore almost all **semantic.rs false positives**, not doxygen-extraction gaps.

### C evidence

Engine-API cache entries (selected):

| Symbol | Params with defaults | Valid arg counts |
|--------|---------------------|------------------|
| `aiPlanCreate` | 4 (`planName=""`, `type=-1`, `parentPlanID=-1`, `outputCategoryID=0`) | 0–4 |
| `kbGetAmountValidResourcesByPosition` | 8 (all have defaults) | 0–8 |
| `useSimpleNatureUnitQuery` (workspace) | 4 (`unitTypeID=-1`, `state=cUnitStateAlive`, `position=cInvalidVector`, `distance=-1.0`) | 0–4 |

Real call sites in the game folder use fewer args and compile cleanly:

- `game/ai/human_assist/human_assist.xs:99` — `aiPlanCreate("Autoscout with unit: " + unitID, cPlanExplore)` — 2 args.
- `game/ai/human_assist/human_assist.xs:597` — `kbGetAmountValidResourcesByPosition(basePosition, cResourceFood, cAIResourceSubTypeEasy, range)` — 4 args.
- `game/ai/human_assist/human_assist.xs:746` — `useSimpleNatureUnitQuery(cUnitTypeFishResource)` — 1 arg.

### D evidence

In both the vanilla game and the mod tree, the callees are defined as **rules**, not functions:

```xs
game/ai/human_assist/human_assist.xs:236    rule updateBreakdown
game/ai/human_assist/human_assist.xs:380    rule applyDistribution
```

The calls:

```xs
mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:394    updateBreakdown();
mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs:877    applyDistribution();
```

Because the symbol table marks them as `SymbolKind::Rule` and `semantic.rs` only resolves `SymbolKind::Function`, the LSP treats them as unresolved / use-before-definition.

---

## 3. XS Default-Argument Semantics (Category C cause)

### Web-research findings

Liam Appelbe, *AoMR Random Map Scripting* (2024-09-27):

> "while C++ function arguments can have default values, in XS **every argument must have a default value**. That is, every argument to every function is optional, and can be omitted. If you forget to define a default value for an argument, you'll get the dreaded 'failed to load' error."

Mythic_Freak, *Master XS - XS Basics* (Age of Mythology Heaven Forums):

> "it's not obligatory to actually supply a function with all its parameters, because they have a default value. ... XS works differently. It assigns a default value to all parameters in the header, which allows the function caller to leave out some params. ... Calls like `myFunc(10,20)` work smoothly though."

### Local evidence

The engine-API cache stores `default` values for the vast majority of parameters (see table above). Real game source calls these functions with fewer arguments and the engine accepts them.

### Implication for the LSP

A type-check argument-count error should fire only when the call provides **fewer arguments than parameters without a default value**. Counting `params.len()` as the required count is wrong. Parameters with a `default` value are optional.

### Risks / gaps

- Doxygen extraction does not capture defaults for **all** parameters (some detailed-doc blocks lack `default value:` text). Where no default is recorded, the LSP must be conservative and treat the parameter as required.
- XS does **not** support skipped positional arguments (e.g., `myFunc(5,,5)` is buggy). Omitting *trailing* args is the supported pattern. The fix should not suggest that arbitrary positional omission is valid.

---

## 4. XS Rule Semantics (Category D reclassification)

### What is an XS rule?

From mythicfreak's code reference and the AOE3 AI Scripting Guide:

```
rule <my_rule_name>
   <active | inactive>
   [minInterval <int>]
   [maxInterval <int>]
   ...
{
    // rule body
}
```

> "A rule is basically a function without return type or parameters that is automatically called again after a preset time has elapsed."

> "Rules can be called like a function. This is useful when you want to execute a rule immediately at a specific point in the code and without any interval."

### Where are rules registered?

Rules are registered implicitly by their definition. Control functions include:

- `xsEnableRule(string ruleName)`
- `xsDisableRule(string ruleName)`
- `xsSetRuleMinInterval(string ruleName, int)` / `xsSetRuleMaxInterval(...)`
- `xsRuleIgnoreIntervalOnce(string ruleName)`
- `trDelayedRuleActivation(string ruleName)`

Real call sites are common throughout `game/ai/`.

### Why the LSP flags them

- `symbols::extract_rule` creates a `Symbol { kind: SymbolKind::Rule, ... }` (`symbols.rs:169-190`).
- `semantic::forward_callable` and `semantic::forward_callable_merged` filter with `s.kind == SymbolKind::Function`.
- `semantic::check_forward_declarations*` therefore never resolves a rule callee.

### Reclassification

**This is not user-code error.** Calling a rule like a function is valid XS. The LSP's symbol-resolution logic must treat `SymbolKind::Rule` as forward-callable (no parameters, no return type) when the callee is used in a call expression.

---

## 5. Code Targets

### Category A.0 / A.1: duplicate-extern logic

| File | Function | Lines | What to change |
|------|----------|-------|----------------|
| `tools/xs-language-server/src/semantic.rs` | `check_extern_collisions` | 194–273 | Stop flagging duplicate `extern` declarations in unrelated files. Scope the check to a single link unit (current file + include closure) or to files that are both included by the same top-level file. |
| `tools/xs-language-server/src/diagnostics.rs` | `collect_all` | 40–78 | Ensure duplicate-extern diagnostics, if emitted, are published on the URI of the file where the declaration actually lives, or converted to `related_information` on the current file. |

### Category B: engine-API awareness in semantic linking

| File | Function | Lines | What to change |
|------|----------|-------|----------------|
| `tools/xs-language-server/src/semantic.rs` | `check_forward_declarations` | 277–347 | Accept engine-API syscalls as resolved; do not emit `Error 0310` for names found in `EngineApi`. |
| `tools/xs-language-server/src/semantic.rs` | `check_forward_declarations_for_merged_view` | 406–476 | Same as above, applied to include-paste scope. |
| `tools/xs-language-server/src/semantic.rs` | `forward_callable` | 583–625 | Consider engine-API names forward-callable. |
| `tools/xs-language-server/src/semantic.rs` | `forward_callable_merged` | 528–546 | Same. |
| `tools/xs-language-server/src/diagnostics.rs` | `collect_all` | 40–78 | Pass `EngineApi` into semantic checks so they can query it. |

### Category C: default-aware argument count

| File | Function | Lines | What to change |
|------|----------|-------|----------------|
| `tools/xs-language-server/src/typecheck.rs` | `check_one_call` | 75–152 | Replace `arg_count != params.len()` with `arg_count < required_count` where `required_count` counts parameters that have no default value. |
| `tools/xs-language-server/src/typecheck.rs` | `Callee::params` / `Syscall` | 168–182 | The `Param` struct already carries `default: Option<String>`; use it to decide optionality. |

### Category D: rules are callable

| File | Function | Lines | What to change |
|------|----------|-------|----------------|
| `tools/xs-language-server/src/semantic.rs` | `forward_callable` | 583–625 | Also accept `SymbolKind::Rule` as a resolved callee. |
| `tools/xs-language-server/src/semantic.rs` | `forward_callable_merged` | 528–546 | Also accept `SymbolKind::Rule` as resolved. |
| `tools/xs-language-server/src/typecheck.rs` | `resolve_workspace_function` | 184–206 | Rules have no parameters and no return type; typecheck should not emit count/type errors for rule calls. |

### Test-gap targets

| File | Function / area | Lines | What to add |
|------|-----------------|-------|-------------|
| `tools/xs-language-server/tests/game_folder_parse.rs` | `analyze_top_level_unresolved` | 418–515 | Load `EngineApi` and run `diagnostics::collect_all` (parse + typecheck + semantic) instead of only `SemanticChecker::check_all`. |
| `tools/xs-language-server/tests/game_folder_parse.rs` | thresholds | 386–402 | Add `TOTAL_DIAGNOSTIC_THRESHOLD = 0`, `DUPLICATE_EXTERN_THRESHOLD = 0`, `ARGUMENT_MISMATCH_THRESHOLD = 0`. |
| `tools/xs-language-server/tests/game_folder_parse.rs` | assertions | 517–558 | Assert each category count is zero, with examples printed on failure. |
| `tools/xs-language-server/src/diagnostics.rs` | diagnostic categorization | 1–118 | Expose stable helpers to classify diagnostics by message/code so the test does not re-parse strings. |

---

## 6. Test Coverage Gaps

### What `game_folder_parse.rs` checks today

1. **Parse every file** — asserts no unexpected `ERROR` nodes above thresholds.
2. **Workspace resolution** — asserts every `.xs` file resolves to a unique relative path.
3. **Unresolved-symbol threshold** — runs `SemanticChecker::check_all` on top-level files, filters out engine-API names by string matching, and asserts `unresolved_symbol_count == 0`.
4. **Top `bo_*` callees** — asserts a hard-coded list of build-order helpers resolve.

### What it misses

| Missed category | Why it is missed |
|-----------------|------------------|
| Duplicate extern diagnostics | `check_extern_collisions` runs but the test never counts or asserts on duplicate-extern messages. |
| Argument-count / argument-type mismatches | `SemanticChecker::check_all` does **not** call `typecheck::check_calls_with_merged`; no typecheck diagnostics are produced. |
| Engine-API unresolved symbol false positives | The test manually skips known engine names, so it cannot detect that `semantic.rs` would emit `Error 0310` for them in normal LSP operation. |
| Rule-call forward-declaration errors | Rules are not separately asserted; current threshold would catch them as "unresolved" only if they are not filtered out. |
| Wrong diagnostic locations | The test only counts messages, never verifies `Diagnostic.range`. |

### Required new thresholds

After fixes, the integration test should assert:

```rust
assert_eq!(duplicate_extern_count, 0);
assert_eq!(argument_mismatch_count, 0);
assert_eq!(unresolved_symbol_count, 0);      // without engine-API filtering
assert_eq!(total_diagnostic_count, 0);       // over all top-level game-folder files
```

### Regression tests to add

1. **Default-args unit test** — `void f(int x = -1, int y = -1) {}` called as `f(1)` produces no diagnostics.
2. **Rule-call unit test** — `rule r {} active {}` called as `r();` from another rule/function produces no diagnostics.
3. **Duplicate-extern regression** — two unrelated files both declaring `extern int gFoo;` produce no collision; the same file including two files that declare it produces a collision pointing at the declarations.
4. **Engine-API unresolved regression** — a call to `xsSetContextPlayer(0)` produces no `Error 0310`.

---

## 7. Proposed Implementation Strategy

### A.0 — Remove cross-file duplicate-extern false positives

Scope `check_extern_collisions` to the current file's include-paste closure instead of the whole virtual project. Two independent `extern` declarations in unrelated files are intentional headers, not collisions. Only flag when the same top-level file (directly or transitively) includes two declarations of the same extern.

### A.1 — Correct diagnostic location

If duplicate-extern diagnostics remain, publish each one on the URI of the file containing the declaration, or attach the declaration file as `related_information`. Do not attach all duplicates to the current open file with ranges from other files.

### B — Make semantic linking engine-API aware

Teach `semantic::check_forward_declarations*` to consult `EngineApi` before emitting `Error 0310`. If a callee name matches a known syscall, treat it as resolved. This eliminates the bulk of the ~140 false positives. Genuinely missing symbols (e.g., a newly added engine function not in `doxygen_retail.7z`) can be addressed by supplemental extraction or a manual allow-list.

### C — Default-aware argument count

Change `typecheck::check_one_call` to compute `required_count` as the number of parameters whose `default` is `None`. Emit a count error only when `arg_count < required_count`. Keep the existing per-argument type check for provided arguments.

### D — Treat rules as callable

Update `semantic.rs` resolution so that a `SymbolKind::Rule` satisfies a call expression. Because rules have no parameters and no return type, `typecheck.rs` should also skip count/type checks for rule calls.

### Test expansion

Convert `game_folder_parse.rs` from a single filtered threshold to a full diagnostic scan using `diagnostics::collect_all`. Add per-category counters and assert zero. Add focused unit tests for default args, rule calls, and engine-API resolution.

---

## 8. Risks / Unknowns

| Risk | Impact | Mitigation |
|------|--------|------------|
| Doxygen archive does not include every engine function. | A few engine symbols may still be flagged as unresolved after B fix. | Maintain a small manual allow-list for symbols known to exist but missing from extraction; verify against game source. |
| Doxygen default-value extraction is incomplete. | Some parameters may be treated as required when they are actually optional, causing lingering C false positives. | Cross-check real call sites in `game/` against extracted defaults; relax only when default is documented or call-site evidence is overwhelming. |
| Cross-file `extern` collision semantics are not exhaustively documented. | A.0 fix might miss true collisions if the engine rejects duplicate externs within a single include closure. | Add a regression test that includes the same extern header twice and verify the engine behavior; err on the side of fewer diagnostics. |
| Rules with parameters do not exist in XS, but rule bodies can reference globals. | Treating rule calls as no-arg calls is correct; no type inference needed. | Add unit test confirming rule calls accept zero arguments. |
| The integration test currently sees 156 unexpected parse errors. | Fixing semantic false positives may expose parse-error diagnostics or be blocked by them. | The 156 errors are already within thresholds and mostly grammar limitations (classes, `#if`); they are separate from this change and documented in the test. |
| Test runtime. | Running full `diagnostics::collect_all` over 149 top-level files plus engine-API extraction may be slow. | Cache engine API like the server does; the existing cache helper is reusable. |

---

## 9. Next Steps for Spec Phase

The following delta specs should be produced before design/apply:

1. **spec-duplicate-extern-semantics.md** — define when duplicate `extern` declarations are an error (single include-closure only) and where diagnostics must be published.
2. **spec-engine-api-symbol-resolution.md** — require `semantic.rs` to consult `EngineApi` before emitting `Error 0310`; list any known genuinely missing symbols and the allow-list policy.
3. **spec-default-argument-typecheck.md** — define required vs. optional parameters; specify that argument-count errors fire only when fewer than required args are provided.
4. **spec-rule-call-resolution.md** — define that `SymbolKind::Rule` satisfies call expressions; rule calls have zero parameters and void return type.
5. **spec-game-folder-diagnostic-thresholds.md** — replace the single `UNRESOLVED_SYMBOL_THRESHOLD` with per-category thresholds and a strict total-diagnostic threshold of zero.
6. **spec-diagnostic-categorization.md** — stable classification API in `diagnostics.rs` so tests (and future clients) can query duplicate-extern / argument-mismatch / unresolved counts reliably.

---

## Key Learnings

- The existing "0 unresolved" integration test is **not sufficient** for the user's goal; it filters out engine-API names and does not exercise `typecheck.rs`.
- Most "missing engine API" diagnostics are caused by `semantic.rs` not consulting the engine API, not by incomplete doxygen extraction.
- XS requires **default values on all parameters**, making the current `arg_count != params.len()` check wrong.
- XS **rules are callable like functions**, so `SymbolKind::Rule` must participate in call resolution.
- `check_extern_collisions` is too broad: independent `extern` declarations across unrelated files are normal XS.
