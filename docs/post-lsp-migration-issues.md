# Post-Migration LSP Issues

Documented 2026-06-27 after the `intellij-xs-plugin-platform-lsp-migration` SDD cycle landed. The migration fixed three known bugs that prevented the plugin from "doing anything" in the IDE. However, the smoke test (T16) in Rider 2026.2 surfaced additional issues in the LSP **server** side and plugin-side TextMate integration.

## What the Migration Fixed

- **Bug #1**: `XsLanguageClient.publishDiagnostics()` only logged diagnostics. Now: the platform's `Lsp4jClient` surfaces them in the editor and Problems window.
- **Bug #2**: No `CompletionContributor` was registered. Now: the platform wires one automatically.
- **Bug #3**: No diagnostic integration. Now: the platform wires it automatically via `Lsp4jClient`.

Free features gained: semantic highlighting, inlay hints, folding, breadcrumbs, signature help, call/type hierarchy, rename refactoring, range formatting, code lens.

Plugin 0.1.4 → 0.1.5. `.zip` at `dist/intellij-xs-plugin-0.1.5.zip` (~3.9 MB).

## Resolution (2026-06-28)

The follow-up SDD cycle `lsp-server-semantic-fixes` addressed Categories A–D and closed the test-coverage gap. After the fixes, running the game-folder integration test against the retail AoM:R install reports:

```
duplicate_extern=0, wrong_uri=0, unresolved_symbol=0,
wrong_arg_count=0, rule_call_unresolved=0, total=0
```

- **Category A.0 (duplicate extern)**: Fixed by scoping collision detection to the current logical link unit (current file + transitive includes) and allowing duplicate `extern` declarations that share the same include chain.
- **Category A.1 (wrong diagnostic location)**: Fixed by emitting duplicate-extern diagnostics under the declaring file's URI and range instead of the open file.
- **Category B (missing engine API symbols)**: Fixed by wiring `EngineApi::lookup` into `resolve_callee`, adding a static builtin-callees list for `vector`/vector helpers, and capturing rules registered through `xsEnableRule`/`trRuleAdd` family so rule calls resolve.
- **Category C (wrong argument counts)**: Fixed by making the argument-count check default-aware (engine-API params treated as optional, workspace params required only when no default), allowing int/float runtime coercion, and accepting function/rule names where a function-pointer type is expected.
- **Category D (forward declarations in `human_assist.xs`)**: Re-evaluated; the two reported callables are now resolvable after rule-registration support. The remaining forward-declaration rules in `AGENTS.md` still apply to user-defined functions that are not rules.
- **Test coverage gap**: `tests/game_folder_parse.rs` now asserts per-category counts (`duplicate_extern`, `wrong_uri`, `unresolved_symbol`, `wrong_arg_count`, `rule_call_unresolved`, `total`) are all zero.
- **Plugin version**: Bumped to **0.1.6** because the bundled LSP binary changed.

**Still open**: Category E (no syntax highlighting) was not in scope for this cycle.

## Categories of Remaining Issues

Smoke test surfaced ~200 diagnostics in `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`. Most are false positives in the LSP server. Categorized below.

### Category A.0: Duplicate extern false positives (~17 errors)

**Symptom**:
```
duplicate extern: 'gUtilitiesCategoryID' is declared extern in 
  /home/.../game/ai/core/utilities/debug.xs and 
  /home/.../game/ai/human_assist/human_assist_debug.xs
```

**Root cause**: `tools/xs-language-server/src/semantic.rs` extern-collision check is too strict. In XS, `extern int gFoo;` is a global variable declaration; multiple files can (and should) declare the same extern — that's how includes share state. The LSP should only flag duplicates that arise from a SINGLE source file's include graph producing the same extern twice (e.g., when `debug.xs` is included via two different paths and the extern declaration appears in both transitive paths), NOT when different files independently declare the same logical extern.

**Affected files**:
- `tools/xs-language-server/src/semantic.rs` (extern collision check)
- `tools/xs-language-server/src/merged_view.rs` (visibility tracking across the include graph)

**Test to add**: LSP regression test that processes `human_assist.xs` and asserts zero duplicate-extern diagnostics. The merge view for `human_assist.xs` pulls in both `debug.xs` (line 4 declares `gUtilitiesCategoryID`) and `human_assist_debug.xs` (line 4 also declares it); this is intentional and should NOT trigger a duplicate-extern diagnostic.

### Category A.1: Wrong diagnostic location for duplicate extern (~17 errors)

**Symptom**: The duplicate extern diagnostic is reported at line 7 of `human_assist.xs` (which is a comment), even though the variable is declared at line 4 of the included files.

**User confirmation** (2026-06-27): "`gUtilitiesCategoryID` is defined at line 4 of both `human_assist_debug.xs` and `debug.xs`, neither at line 7. In `human_assist.xs`, `human_assist_debug.xs` is included in line 11."

**Root cause**: The diagnostic's `range` field is set to the open file (where the LSP discovered the collision via include processing) instead of pointing to the actual declaration location in the included files.

**Affected files**:
- `tools/xs-language-server/src/semantic.rs` (range construction)
- `tools/xs-language-server/src/diagnostics.rs` (categorize + count)

**Test to add**: A regression test that asserts the diagnostic `range` points to the actual declaration line in the included file (not the open file).

### Category B: Missing engine API symbols (~140 errors)

**Symptom**:
```
Error 0310: invalid symbol lookup 'xsSetContextPlayer' at line 57
Error 0310: invalid symbol lookup 'kbUnitGetPlanID' at line 83
Error 0310: invalid symbol lookup 'aiTaskStopUnit' at line 86
Error 0310: invalid symbol lookup 'aiPlanDestroy' at line 87
... (~140 of these)
```

Functions missing from the engine API extracted from `doxygen_retail.7z`:
- `xsSetContextPlayer`, `xsGetTime`, `xsGetTimeMS`, `xsIntToFloat`, `xsFloatToInt`, `xsBoolToString`, `xsVectorDistanceXZSqr`, `xsDisableRule`, `xsEnableRule`, `xsSetRuleMinInterval`, `xsSetRuleMaxInterval`, `xsRuleIgnoreIntervalOnce`
- `kbUnitGetPlanID`, `kbUnitIsType`, `kbUnitCount`, `kbUnitGetKBResourceID`, `kbUnitGetProtoUnitID`, `kbUnitGetActionType`, `kbUnitGetIdleTime`, `kbUnitGetPosition`, `kbUnitGetTargetUnitID`, `kbUnitQueryCreate`, `kbUnitQuerySetPlayerID`, `kbUnitQuerySetUnitType`, `kbUnitQuerySetState`, `kbUnitQueryResetResults`, `kbUnitQueryExecute`, `kbUnitQueryGetResult`, `kbUnitGetResourceAmount`, `kbBaseGetNumber`, `kbBaseGetIDByIndex`, `kbBaseIsFlagSet`, `kbBaseGetLocation`, `kbBaseGetDistance`, `kbBaseSetDistance`, `kbBaseSetFlag`, `kbBaseGetMainID`, `kbCanPath`, `kbResourceGetIsIDValid`, `kbResourceSetFlag`, `kbGetAmountValidResourcesByPosition`, `kbSetResourceSelectorFactor`, `kbAreaGroupGetNumber`, `kbAreaGroupGetType`, `kbProtoUnitIsType`, `kbProtoUnitGetName`, `kbSharedFunctionUnitGetByIndex`
- `aiTaskStopUnit`, `aiPlanDestroy`, `aiPlanCreate`, `aiPlanAddUnitType`, `aiPlanAddUnit`, `aiPlanSetVariableBool`, `aiPlanSetVariableFloat`, `aiPlanSetVariableInt`, `aiPlanSetFlag`, `aiPlanGetIDsByType`, `aiPlanGetNumberUnits`, `aiPlanGetName`, `aiPlanGetNumberByTypeAndVariableIntValue`, `aiPlanGetVariableInt`, `aiPlanGetNumberByType`, `aiPlanGetIDByTypeIndex`, `aiPlanGetBaseID`, `aiPlanGetLocation`, `aiPlanGetUnits`, `aiPlanGetIDByTypeAndVariableIntValue`, `aiPlanGetNumberMaxUnits`, `aiPlanSetPriority`, `aiPlanRemoveUnit`
- `aiEcho`, `aiEchoWarning`, `aiGetResourcePercentage`, `aiSetResourcePercentage`, `aiNormalizeResourcePercentages`, `aiSetNextGathererDistributionTime`, `aiSetFullUnitAssignmentTime`, `aiSetUnassignedUnitAssignmentTime`, `aiSetDeleteUnitsForbiddenPlans`, `aiSetHandler`
- `aiSendNotificationFoundNotEnoughFoodGatheringSpots`, `aiSendNotificationFoundNotEnoughFoodFarmGatheringSpots`, `aiSendNotificationFoundNotEnoughGoldGatheringSpots`, `aiSendNotificationFoundNotEnoughWoodGatheringSpots`
- `round`, `min`, `max`, `useSimpleNatureUnitQuery`

**Root cause**: The doxygen extraction code is incomplete. Either:
- The scraper config is missing certain sections of the doxygen XML
- The parsing logic skips certain function kinds (e.g., methods vs free functions, or `kb*` vs `ai*` categorization)
- The extraction categorizes some functions into the wrong namespace

**Affected files** (to investigate):
- `tools/xs-language-server/src/engine_api.rs` (or equivalent)
- `tools/xs-language-server/src/main.rs` (extraction entry point)

**Test to add**: A regression test that asserts all functions called in `human_assist.xs` are resolvable through the engine API. This would surface any missing symbols.

### Category C: Wrong argument counts (~15 errors)

**Symptom**:
```
expected 4 argument(s) to `aiPlanCreate`, got 2
expected 7 argument(s) to `aiPlanAddUnitType`, got 5
expected 3 argument(s) to `aiPlanGetNumberUnits`, got 1
expected 4 argument(s) to `kbUnitCount`, got 3
expected 2 argument(s) to `kbUnitGetKBResourceID`, got 1
expected 3 argument(s) to `kbUnitQuerySetPlayerID`, got 2
expected 2 argument(s) to `aiPlanGetNumberMaxUnits`, got 1
expected 4 argument(s) to `aiPlanGetUnits`, got 1
expected 2 argument(s) to `aiPlanGetLocation`, got 1
expected 8 argument(s) to `kbGetAmountValidResourcesByPosition`, got 4
expected 4 argument(s) to `useSimpleNatureUnitQuery`, got 1
```

**Root cause**: Same as Category B (doxygen extraction). The scraper is parsing parameter lists incorrectly (e.g., counting default parameters but not overload parameters, or missing `optional` markers).

**Affected files**: Same as Category B.

**Test to add**: A regression test that verifies the argument count for each function listed above. Use the LSP's `completion` or `signatureHelp` capability to query each function's expected signature.

### Category D: Forward declarations (2 errors, REAL user code)

**Symptom**:
```
'updateBreakdown' used at line 394 before declaration; add forward declaration or mark 'mutable'
'applyDistribution' used at line 877 before declaration; add forward declaration or mark 'mutable'
```

**Status**: NOT a false positive. XS requires explicit forward declarations per `AGENTS.md` (lines about "Forward declarations are required"):

> XS does NOT support implicit forward declarations like C/C++. A function must be either defined before it is called, OR declared (signature only) with a trailing semicolon earlier in the file. A function marked `mutable` is the only exception — it can be redefined later and is forward-callable.

**Affected file**: `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`

**Fix**: Add forward declarations to `human_assist.xs` (the user code, not the LSP). Example:

```xs
void updateBreakdown(int unused = -1);  // before line 394
void applyDistribution(int playerID = -1);  // before line 877
```

The exact signatures need to be confirmed by reading the function definitions later in the file.

### Category E: No syntax highlighting

**Symptom**: Opening `.xs` files in Rider produces no syntax highlighting (text is plain).

**Root cause** (suspected): Stub PSI throws `UnsupportedOperationException` in `XsParserDefinition.createElement()`, which interferes with TextMate fallback. The PSI was stubbed for the original "P2" PSI implementation that never landed.

**Affected files**:
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsParserDefinition.kt`
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsASTFactory.kt`
- `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` (verify bundle loads)
- `tools/intellij-xs-plugin/src/main/resources/package.json` (verify manifest)
- `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` (verify TextMate bundle provider extension)

**Fix options**:
1. Make `XsParserDefinition.createElement()` return a no-op AST node (cheap fix)
2. Remove the `lang.parserDefinition` extension from plugin.xml (cleaner, but may break other IntelliJ features that expect a parser definition)
3. Investigate why TextMate isn't loading independently — maybe the bundle detection fails for a different reason

### Test Coverage Gap

**File**: `tools/xs-language-server/tests/game_folder_parse.rs`

**Current state**: The test only checks `unresolved_symbol_count == 0`. This is why the test suite reported 120 tests passing even when the LSP produces 17 + 17 + 140 + 15 = ~189 false positives.

**Missing checks**:
- `duplicate_extern_count`
- `argument_mismatch_count`
- Diagnostic location correctness (per category A.1)

**Fix**:
```rust
// In game_folder_parse.rs:
let diagnostics = collect_all_diagnostics(game_path);
let unresolved_symbols = diagnostics.iter().filter(|d| d.is_unresolved_symbol()).count();
let duplicate_externs = diagnostics.iter().filter(|d| d.is_duplicate_extern()).count();
let argument_mismatches = diagnostics.iter().filter(|d| d.is_argument_mismatch()).count();

assert_eq!(unresolved_symbols, 0, "missing engine API symbols");
assert_eq!(duplicate_externs, 0, "duplicate extern false positives");
assert_eq!(argument_mismatches, 0, "wrong argument counts");
```

**This should be the FIRST fix** — without it, future regressions in any of these categories will go undetected.

## Reproduction Steps

To reproduce all of the above:

1. Install plugin 0.1.6 from `dist/intellij-xs-plugin-0.1.6.zip` in Rider 2026.2
2. Configure the game folder path in Settings → Languages & Frameworks → XS Language Server (point at AoM:R install root)
3. Open `mod/intelligent_auto_scout/game/ai/human_assist/human_assist.xs`
4. Semantic diagnostics for Categories A–D should no longer appear; any remaining diagnostics are likely real user-code issues
5. Observe no syntax highlighting (text appears plain) — Category E remains open

## Suggested Next SDD Cycle

**Name**: `lsp-server-semantic-fixes`

**Scope**:
- Fix Category A.0 (duplicate extern false positive) in `semantic.rs`
- Fix Category A.1 (wrong diagnostic location) in `semantic.rs` + `diagnostics.rs`
- Investigate and fix Category B + C (engine API extraction coverage + accuracy)
- Fix Category E (syntax highlighting — likely stub PSI fix)
- **FIRST**: Expand test thresholds in `game_folder_parse.rs` to cover all diagnostic categories
- Add pinpoint regression tests for the specific files/functions that triggered each category

**Out of scope**:
- Category D (real user code — user must add forward declarations to `human_assist.xs`)

**Effort estimate**: 8-12 hours, 200+ lines of LSP server changes, plus test expansion.

## Existing Engram Observations

For additional context (use `mem_get_observation`):
- `#1332` — Platform API research (Prisma + platform API)
- `#1334` — 3 bugs + migration path
- `#1335-#1339` — Phase artifacts (explore, propose, spec, design, tasks)
- `#1341` — Apply progress (original + remediation)
- `#1344` — Strict TDD policy for LSP/plugin tasks
- `#1345` — Verify report (post-remediation)
- `#1351` — LSP migration smoke test results
- `#1355` — Category A.1 wrong diagnostic location
- `#1356` — Test coverage gap

## Related Files

- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/` — full SDD artifacts for the migration that landed
- `openspec/changes/archive/true-include-paste/` — the concurrent change that also has strays in the working tree (do not conflate with this issue set)
- `docs/xs-language-syntax.md` — XS language reference (for understanding forward declarations)
- `docs/MythRMConstants.txt`, `docs/MythTRConstants.txt` — full lists of XS global constants
- `tools/xs-language-server/` — LSP server source
- `tools/intellij-xs-plugin/` — IntelliJ plugin source