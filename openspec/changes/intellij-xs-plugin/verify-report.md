# Verification Report — `intellij-xs-plugin` P1 Engine API Surface

**Change:** `intellij-xs-plugin`  
**Phase verified:** P1 — Engine API surface (PRs P1.1, P1.2+1.3, P1.4, P1.5+1.6)  
**Verification date:** 2026-06-23  
**Verifier:** `sdd-verify` executor  
**Build/test execution:** Not executed in this phase per task instruction; relying on apply-progress test evidence plus source/test inspection.

---

## 1. Executive summary

P1 implementation satisfies the P1 specs and task acceptance criteria. All four batches documented in `apply-progress.md` report green `./gradlew test` runs (32 tests total), `buildPlugin` succeeds, and `validateBundledResources` passes. The engine syscalls data layer, name completion, default-value completion, hover documentation, parameter-info handler, and engine-stub navigation are all implemented and wired in `plugin.xml`. The only gaps are test-coverage gaps — a handful of spec transitions are implemented correctly but not exercised by a dedicated assertion. No implementation defects that would block merging were found. Overall verdict: **PASS with notes**.

---

## 2. Per-spec coverage

### spec-engine-completion.md

**Status:** PASS with notes

**Scenarios from spec:**

1. **Happy path — completing a syscall name**  
   GIVEN `aiE` typed inside an `.xs` file  
   WHEN auto-completion is invoked  
   THEN lookup contains `aiEcho`, `aiEchoCategory`, and `aiEchoWarning`  
   AND each item shows return type and parameter summary.

2. **Edge case — syscall with no parameters**  
   GIVEN `xsDisableSelf(` in a call context  
   WHEN opening parenthesis is typed  
   THEN no default-value item is inserted  
   AND caret remains immediately after `(`.

3. **Negative case — no completion inside a string literal**  
   GIVEN typing inside `"aiE|"`  
   WHEN auto-completion is invoked  
   THEN engine syscall completions SHALL NOT be offered.

**Verified by:**

- `XsSyscallCompletionTest.testEngineSyscallCompletionAndDefaults` Scenarios 1, 2, 3, 6, 7.
- `XsEngineApiTest.searchByPrefixFiltersCaseSensitively` confirms the data layer returns `aiEcho`, `aiEchoCategory`, `aiEchoWarning` for prefix `aiE`.
- Production code: `XsCompletionContributor.SyscallNameCompletionProvider`, `SyscallDefaultCompletionProvider`, `SyscallInsertHandler`, and `XsCallContextDetector`.

**Notes:**

- Scenario 1 is fully tested (prefix `aiE` and `kbUnit`). The return-type/parameter summary is rendered via `XsCompletionContributor.formatSignature` as the lookup tail text.
- Scenario 2 is tested (zero-parameter `xsDisableSelf` produces no default popup). The implementation returns early when `paramIndex < 0 || paramIndex >= syscall.params.size`.
- Scenario 3 is tested for string literals; the same `isInStringOrComment` path also covers line comments. Adding an analogous block-comment assertion is harmless and would close a trivial gap.
- The spec acceptance criterion "insert a complete call, including parentheses, when a name item is selected" is implemented in `SyscallInsertHandler` but is not directly asserted by the current fixture test. This is a test gap, not an implementation gap.

---

### spec-hover-docs.md

**Status:** PASS

**Scenarios from spec:**

1. **Happy path — hover over a documented syscall**  
   GIVEN source contains `aiPlanCreate`  
   WHEN user hovers  
   THEN popup shows signature `int aiPlanCreate(...)`  
   AND includes help paragraph from `syscalls.json`.

2. **Edge case — syscall with no help paragraph**  
   GIVEN source contains a syscall with empty `help` field  
   WHEN user hovers  
   THEN popup shows signature  
   AND displays a placeholder such as "No documentation" instead of blank.

3. **Negative case — hover over an unknown identifier**  
   GIVEN source contains a token not in `syscalls.json`  
   WHEN user hovers  
   THEN no syscall documentation popup is shown.

**Verified by:**

- `XsHoverTest.hoverOverAiEchoShowsSignatureHelpAndParameters`
- `XsHoverTest.hoverOverKbUnitCountShowsSignature`
- `XsHoverTest.hoverOverEmptyHelpSyscallShowsPlaceholder`
- `XsHoverTest.hoverOverUnknownIdentifierReturnsNull`
- `XsHoverTest.generateDocIsFast`
- Production code: `XsDocumentationProvider`.

**Notes:**

- Scenario 1 is covered, although the test uses `aiEcho` and `kbUnitCount` rather than `aiPlanCreate`. The rendering path is identical for any documented syscall, so coverage is adequate.
- Scenario 2 is covered with a synthetic syscall (`help = "   "`) because no vendored entry has blank help text.
- Scenario 3 is directly covered.
- The optional "source filename as supplementary metadata" acceptance criterion is implemented (`<small>Source: ...</small>`) and appears in `renderDocumentation`.
- Background-thread/UI-freeze requirement is exercised indirectly by the <100 ms performance test for 1,000 calls.

---

### spec-parameter-info.md

**Status:** PASS with notes

**Scenarios from spec:**

1. **Happy path — current parameter highlighted**  
   GIVEN `aiPlanSetVariableBool(planID, |)`  
   WHEN IDE shows parameter info  
   THEN popup displays full signature  
   AND second parameter is rendered in bold.

2. **Edge case — no opening parenthesis yet**  
   GIVEN caret is inside a syscall identifier but `(` has not been typed  
   WHEN IDE queries parameter info  
   THEN no popup is shown  
   AND editor remains responsive.

3. **Negative case — parameter info is limited to engine syscalls**  
   GIVEN workspace-defined function call `myHelper(a, |)`  
   WHEN IDE requests parameter info  
   THEN capability SHALL NOT be required to show a popup for that call.

**Verified by:**

- `XsParameterInfoTest.testParameterInfoScenarios` Tests 1, 2, 3, 4, 5, 6.
- Production code: `XsParameterInfoHandler` and `XsCallContextDetector`.

**Notes:**

- Scenario 1 is fully tested (caret on second argument → parameter index 1).
- Scenario 2 is **not directly tested**, but the implementation is correct: `XsCallContextDetector.findCallContext` searches backwards for an unmatched `(` and returns `null` if none exists, so `findOwner` returns `null` and no popup is shown. Recommend adding a dedicated assertion in P2 or as a fast-follow.
- Scenario 3 is behaviorally covered by Test 4 (`fictionalCall` produces no owner). No workspace PSI exists yet in P1, so a true workspace-function negative case cannot be constructed; this is acceptable for P1.
- Missing closing parenthesis is explicitly tested (Test 6) and handled by `findMatchingRParen` returning `-1`.

---

### spec-workspace-navigation.md

**Status:** PASS with notes (engine-stub portion only)

**Scenarios from spec (P1-only subset):**

1. **Happy path — navigate to an engine syscall stub**  
   GIVEN source contains call `aiPlanCreate("myPlan", ...)`  
   WHEN user presses `Ctrl+B` on `aiPlanCreate`  
   THEN an in-memory stub file opens showing signature and help from `syscalls.json`  
   AND file title makes clear it is a generated documentation view.

2. **Edge case — workspace function used before its definition**  
   *(P3 scope; not evaluated for P1.)*

3. **Negative case — unknown identifier does not navigate**  
   GIVEN source contains token `nonExistentSymbol` that is neither syscall nor workspace symbol  
   WHEN user presses `Ctrl+B`  
   THEN no navigation target is opened  
   AND no error dialog is displayed.

**Verified by:**

- `XsEngineNavigationTest.testEngineNavigationScenarios` Tests 1, 2, 3, 4, 5.
- Production code: `XsEngineReferenceContributor`, `XsEngineReference`, `XsEngineStubGenerator`, `XsIdentifier`, `XsASTFactory`.

**Notes:**

- Scenario 1 is covered, though the test uses `aiEcho` rather than `aiPlanCreate`. The resolved target is asserted to be a `LightVirtualFile`, the stub text is asserted to contain the signature and help text.
- The "file title makes clear it is a generated documentation view" acceptance criterion is implemented (`"${syscall.name} [XS Engine Stub].xs"`) but is not asserted by the test. Add a filename assertion as a fast-follow.
- Scenario 3 is directly tested (`fictionalCall` yields no reference).

---

## 3. Per-task coverage

### Task 1.1 — Vendored engine resources + API indexes

**Acceptance criteria from tasks.md:**

- `XsEngineApi.lookup("aiEcho")` returns the full syscall object (name, help, return_type, params, filename).
- All 1,805 syscalls and 193 AI-plan constants are loaded at plugin init.
- Lookup is O(1) by exact name and supports prefix iteration.

**Met?** YES

**Evidence:**

- `XsEngineApiTest.lookupKnownSyscall` asserts `aiEcho` lookup shape.
- `XsEngineApiTest.sizeMatchesVendoredSyscallCount` asserts 1,805 syscalls.
- `XsAiPlansTest.sizeMatchesVendoredPlanCount` asserts 193 AI plans.
- `XsEngineApiTest.searchByPrefixFiltersCaseSensitively` and `XsAiPlansTest.searchByPrefixReturnsMatchingPlans` exercise prefix iteration.
- `XsEngineApi.kt` uses `byName` map for O(1) exact lookup and `sortedByName` for prefix iteration.

**Notes:**

- The data is loaded lazily on first access (Kotlin object initialization) rather than strictly at plugin load time. This still satisfies the intent of loading exactly once and avoiding per-keystroke allocation, but the wording in tasks.md says "at plugin init." Consider eager initialization from a plugin lifecycle listener if strict init-time loading is required.

---

### Task 1.2 — Engine syscall name completion

**Acceptance criteria:**

- Typing `aiE` proposes `aiEcho`, `aiEchoCategory`, `aiEchoWarning`.
- Selecting a syscall inserts a complete call with parentheses.
- Completions are filtered by the current identifier prefix.
- No completions are offered inside comments or string literals.

**Met?** YES

**Evidence:**

- `XsSyscallCompletionTest` Scenario 1 covers the `aiE` prefix.
- `SyscallInsertHandler` in `XsCompletionContributor` inserts `()` and moves the caret between the parentheses.
- Prefix filtering is implemented via `XsEngineApi.searchByPrefix(prefix)` plus platform-side lookup filtering.
- `XsCallContextDetector.isInStringOrComment` blocks completions inside strings/comments; Scenarios 3 and 4 test this.

**Notes:**

- The insert-with-parens behavior is implemented but not directly asserted by a fixture test. A follow-up assertion accepting the `aiEcho` item and checking document text would close this gap.

---

### Task 1.3 — Default parameter-value completion on `(` / `,`

**Acceptance criteria:**

- Typing `aiPlanCreate(` inserts/proposes the first parameter's default value when one exists.
- Typing `,` inside a known syscall call proposes the next parameter's default value.
- Zero-parameter syscalls (`xsDisableSelf(`) insert nothing extra.

**Met?** YES

**Evidence:**

- `XsCompletionContributor.SyscallDefaultCompletionProvider` combined with `XsCallContextDetector.findCallContext` computes the active parameter index from preceding commas.
- `XsSyscallCompletionTest` Scenario 5 covers the `,` case (`aiPlanCreate(0, <caret>)` proposes `-1`).
- `XsSyscallCompletionTest` Scenario 6 covers zero-parameter `xsDisableSelf(`.

**Notes:**

- The open-parenthesis case is implemented but not directly tested in the fixture. The comma test plus code inspection confirms the same path handles `(`.

---

### Task 1.4 — Hover documentation provider

**Acceptance criteria:**

- Hovering over `kbUnitCount` renders `int kbUnitCount(...)` and help paragraph.
- Empty help shows a deterministic placeholder instead of a blank popup.
- Hover resolution runs on a background thread and does not block the UI.

**Met?** YES

**Evidence:**

- `XsHoverTest.hoverOverKbUnitCountShowsSignature` asserts signature contents.
- `XsHoverTest.hoverOverAiEchoShowsSignatureHelpAndParameters` asserts help text and parameter names.
- `XsHoverTest.hoverOverEmptyHelpSyscallShowsPlaceholder` asserts `(no help available)`.
- `XsHoverTest.generateDocIsFast` asserts 1,000 calls finish in <100 ms.
- `XsDocumentationProvider` is registered in `plugin.xml` under `lang.documentationProvider`.

---

### Task 1.5 — Parameter info handler

**Acceptance criteria:**

- Caret inside `aiPlanSetVariableBool(planID, |)` bolds the second parameter.
- Signature with types and names is shown for known syscalls.
- Missing closing parenthesis is handled gracefully.
- No popup for identifiers absent from `syscalls.json`.

**Met?** YES

**Evidence:**

- `XsParameterInfoTest` Tests 1, 2, 5, 6, and 4 cover all four acceptance criteria.
- Production code: `XsParameterInfoHandler`, `renderParameterInfo`, `highlightRange`, `findOwner`.
- Registered in `plugin.xml` as `lang.parameterInfoHandler`.

**Notes:**

- The "no opening parenthesis yet" scenario from the spec is not directly tested; see spec notes above.

---

### Task 1.6 — Engine syscall go-to-definition stubs

**Acceptance criteria:**

- `Ctrl+B` on any syscall name opens a generated in-memory `LightVirtualFile`.
- Stub shows the syscall signature, parameters, and help text.
- File tab title clearly indicates it is a generated documentation view.

**Met?** YES

**Evidence:**

- `XsEngineNavigationTest` Tests 1, 2, 3 cover `Ctrl+B` resolution and stub content.
- `XsEngineStubGenerator` creates `LightVirtualFile` stubs and caches them by syscall name.
- `XsEngineReferenceContributor` contributes references for all identifiers present in `XsEngineApi`.
- `XsIdentifier` and `XsASTFactory` ensure identifier leaves consult `ReferenceProvidersRegistry`.
- Stub filename is `"${syscall.name} [XS Engine Stub].xs"`.
- Cache reuse is asserted in `XsEngineNavigationTest` Test 5.

**Notes:**

- The tab-title/filename assertion is not present in the current test.

---

## 4. Test quality audit

**Strengths:**

- Data-layer tests assert exact vendor counts (1,805 syscalls, 193 AI plans) and exercise both exact and prefix lookups.
- `XsHoverTest` uses lightweight `LeafPsiElement` synthetic elements to avoid fixture lifecycle overhead, which is appropriate for a PSI-agnostic provider and gives fast, deterministic assertions.
- `XsParameterInfoTest` covers nesting-aware comma counting indirectly through caret positions.
- `XsEngineNavigationTest` exercises real `findReferenceAt` + `resolve()` behavior, proving the reference contributor wiring and `XsIdentifier` leaf are functional.

**Weaknesses / gaps:**

- `XsSyscallCompletionTest` groups seven scenarios into a single test method due to reported headless-sandbox fixture hangs. This reduces isolation and makes failures harder to diagnose.
- The insert handler that adds `()` after a syscall name completion is not directly asserted.
- The open-parenthesis default-completion transition (`aiPlanCreate(<caret>`) is not directly asserted; only the comma transition is.
- `XsParameterInfoTest` does not assert the "caret inside identifier before `(`" negative case.
- `XsEngineNavigationTest` does not assert the generated stub filename pattern.
- The parameter-info tests call internal helper methods (`findParameterIndex`, `renderParameterInfo`) rather than the platform `ParameterInfoHandler` API entry points. This is pragmatic for unit testing, but a future test should exercise `updateUI`/`findElementForParameterInfo` if fixture stability improves.

**Coverage adequacy:** Coverage is adequate for the capabilities implemented. The gaps above are all assertion gaps, not missing feature coverage.

---

## 5. Deviations from spec

| # | Spec / task wording | Implementation | Rationale |
|---|---|---|---|
| 1 | `syscalls.json` loaded "once at plugin init time" | Loaded lazily on first `XsEngineApi` access | Kotlin object singleton avoids startup cost; still loaded exactly once |
| 2 | `aiplans.json` expected to support `cPlan*` prefixes per exploratory spec | Vendored data uses grouped prefixes (`cAttackPlan*`, `cBuildPlan*`, etc.) | Real VS Code extension data groups constants by plan kind; `XsAiPlansTest` was adjusted to `cAttackPlan` |
| 3 | `LangDocumentationProvider` / `ParameterInfoHandler` platform interfaces | Tests call exposed internal helpers; platform entry points delegate to them | Headless fixture lifecycle issues documented in apply-progress |
| 4 | SurroundingPairsProvider extension (P0.5 deviation, carried forward) | `XsSurroundingPairsProvider` exists but is not registered because the extension point is absent in 2024.2 | Platform provides surrounding-pair behavior via brace matcher / quote handler integration; class is retained for future API |

No deviations break P1 behavior.

---

## 6. Risks & follow-ups

1. **PSI-less detection migration.** Completion, parameter info, and default-value completion rely on `XsCallContextDetector`, which lexes the entire file on every request. Once P2 introduces a real Grammar-Kit PSI, the plugin will have two context-detection implementations. Unless reconciled, fixes in one path can diverge from the other, leading to inconsistent behavior between completion and parameter info.

2. **Identifier leaf reference pattern.** `XsIdentifier` overrides `getReferences()` to consult `ReferenceProvidersRegistry`. When P2 adds composite PSI elements from the BNF grammar (e.g., a `REFERENCE` composite), the current contributor pattern may need to move from leaf-level to composite-level to avoid duplicate or missing references.

3. **Fixture test fragility.** Multiple `BasePlatformTestCase` methods hang in the headless sandbox, forcing scenario grouping and `forkEvery = 1`. As P2/P3 add more fixture-heavy tests, this strategy may become unsustainable. Root-causing the hang (likely a missing/disposed project fixture or IDE event queue issue) will pay dividends before expanding fixture coverage.

4. **LightVirtualFile cache lifetime.** `XsEngineStubGenerator` is currently instantiated per reference contributor companion. If other components later create their own generator instances, the cache will fragment. Consider making it a singleton service.

---

## 7. Recommended actions

**Before merging P1:**

- [ ] No blocking actions. The implementation is complete and tests pass.

**Non-blocking fast-follows (can be done before or during P2):**

- [ ] Add a fixture assertion that accepting `aiEcho` completion inserts `aiEcho()` with the caret between the parentheses.
- [ ] Add an assertion for `aiPlanCreate(<caret>)` proposing the first default value.
- [ ] Add a parameter-info assertion for caret inside the identifier before `(` returning no owner.
- [ ] Assert the generated stub filename contains `[XS Engine Stub]` in `XsEngineNavigationTest`.
- [ ] Add a block-comment negative completion assertion analogous to the string/line-comment ones.
- [ ] Decide whether to eagerly initialize `XsEngineApi`/`XsAiPlans` at plugin load time or accept lazy initialization and update task wording.

**Next phase:** After user merges the P1 PRs, proceed to `sdd-archive P1` and then `sdd-apply P2`.

---

## 8. Compliance summary

| Spec | Status | Directly tested scenarios | Implemented but not directly tested scenarios |
|---|---|---|---|
| spec-engine-completion.md | PASS with notes | 3/3 | Insert-with-parens, block-comment exclusion |
| spec-hover-docs.md | PASS | 3/3 | — |
| spec-parameter-info.md | PASS with notes | 2/3 | No opening parenthesis yet |
| spec-workspace-navigation.md (engine-stub only) | PASS with notes | 1/2 P1 scenarios | Stub tab-title assertion |

| Task | Status |
|---|---|
| 1.1 Vendored engine resources + API indexes | YES |
| 1.2 Engine syscall name completion | YES |
| 1.3 Default parameter-value completion | YES |
| 1.4 Hover documentation provider | YES |
| 1.5 Parameter info handler | YES |
| 1.6 Engine syscall go-to-definition stubs | YES |

**Overall verdict:** **PASS with notes**.
