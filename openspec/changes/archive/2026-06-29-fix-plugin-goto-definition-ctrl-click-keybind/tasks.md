# Tasks: fix-plugin-goto-definition-ctrl-click-keybind

## Review Workload Forecast

| Field | Value |
|---|---|
| Estimated changed lines | ~230 + `.zip` artifact |
| 400-line budget risk | Low |
| Chained PRs recommended | No |
| Suggested split | Single PR |
| Delivery strategy | single-pr |
| Chain strategy | pending |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: pending
400-line budget risk: Low

## Phase 0: Setup

- [x] 0.1 Confirm branch state (5m/L/—/git log --oneline -5)
- [x] 0.2 Confirm `:compileKotlin` works (10m/L/—/`.gradlew :compileKotlin` clean)
- [x] 0.3 Confirm `:test` passes (10m/L/0.2/`.gradlew :test` green) — baseline 62 tests, 1 pre-existing failure in `XsStartupActivityTest.autoDetectedModsAreSavedAndNotified`
- [x] 0.4 Note `pluginVersion` (2m/L/—/+`0.2.2`)

## Phase 1: RED — Failing tests

- [x] 1.1 `test_single_target_returns_one_navigable` (10m/L/—/fails in `XsGotoDeclarationHandlerTest`)
- [x] 1.2 `test_multi_target_returns_multiple_navigables` (10m/L/1.1/fails)
- [x] 1.3 Right-click regression check (10m/L/1.1/test or `@Ignore` if not unit-testable) — regression paths unchanged; not unit-tested
- [x] 1.4 Hover regression check (10m/L/1.1/test or `@Ignore` if not unit-testable) — hover unchanged; not unit-tested
- [x] 1.5 `test_non_xs_file_returns_null` (10m/L/1.1/fails)
- [x] 1.6 `test_lsp_timeout_returns_empty` (10m/L/1.1/fails)
- [x] 1.7 `test_external_file_navigates` (10m/L/1.1/fails)
- [x] 1.8 Confirm strict-TDD test compiles (5m/L/1.1/test file compiles and fails)
- [x] 1.9 Confirm full RED suite (10m/M/all/`.gradlew :test --tests XsGotoDeclarationHandlerTest` fails)
- [x] 1.10 Capture RED evidence (5m/L/1.9/in `verify-report.md`)

## Phase 2: GREEN — Implementation

- [x] 2.1 Create `XsDefinitionResolver.kt` (15m/L/1.9/interface + `XsLspDefinitionResolver` + stub)
- [x] 2.2 Create `XsGotoDeclarationHandler.kt` returning null (15m/L/2.1/implements `GotoDeclarationHandler`)
- [x] 2.3 Register handler in `plugin.xml` with `order="last"` (5m/L/2.2/extension present)
- [x] 2.4 Confirm still RED after stub (10m/L/2.3/tests fail)
- [x] 2.5 Implement LSP call in resolver (30m/M/2.1/`sendRequestSync(5000)` or fallback)
- [x] 2.6 Convert `Location`/`LocationLink` to `PsiElement` (20m/M/2.5/URI→VirtualFile→PsiFile→leaf)
- [x] 2.7 Wire handler to resolver (10m/L/2.6/delegates and catches exceptions)
- [x] 2.8 Confirm tests PASS (10m/M/2.7/`XsGotoDeclarationHandlerTest` green)

## Phase 3: GREEN — Build verification

- [x] 3.1 `:compileKotlin` (10m/L/2.8/no compile errors)
- [x] 3.2 `:test` (10m/M/3.1/all tests pass) — 67 tests, 1 pre-existing `XsStartupActivityTest` failure
- [x] 3.3 `:buildPlugin` (5m/L/3.2/`.zip` produced)
- [x] 3.4 Inspect `.zip` (5m/L/3.3/`plugin.xml` contains `gotoDeclarationHandler`)

## Phase 4: REFACTOR

- [x] 4.1 KDoc `XsGotoDeclarationHandler` (5m/L/3.4/documents fallback, XS filter, timeout, errors)
- [x] 4.2 KDoc `XsDefinitionResolver` (5m/L/3.4/documents seam, LSP call, conversion)
- [x] 4.3 XML comment on `plugin.xml` registration (5m/L/3.4/explains `order="last"`)

## Phase 5: VERIFY

- [ ] 5.1 Final `:test` (10m/M/4.3/all green)
- [ ] 5.2 Final `:buildPlugin` (5m/L/5.1/`.zip` regenerated)
- [ ] 5.3 Note warnings (5m/L/5.2/record in `verify-report.md`)
- [ ] 5.4 Write `verify-report.md` (10m/L/5.3/created in change dir)

## Phase 6: DOCUMENT

- [ ] 6.1 Mark Issue 1 resolved (5m/L/5.4/`docs/issues/2026-06-29-runtime-issues.md` updated)
- [ ] 6.2 Update `AGENTS.md` if convention emerges (5m/L/6.1/N/A or note)

## Phase 7: COMMIT

- [ ] 7.1 Bump `pluginVersion` (2m/L/5.4/version `0.2.3`)
- [ ] 7.2 Stage intended files (5m/M/7.1/source, test, `.zip`, docs, SDD artifacts staged)
- [ ] 7.3 Prepare commit message (2m/L/7.2/`fix(xs-plugin): wire GotoDeclarationHandler ...`)

## Review Workload Forecast

- Files changed: 3 Kotlin files, plugin.xml, gradle.properties, `.zip`, issue doc
- New test files: `XsGotoDeclarationHandlerTest.kt`
- Total changed lines: ~230 + binary artifact
- Chained PRs recommended: No
- 400-line budget risk: Low
- Decision needed before apply: No
