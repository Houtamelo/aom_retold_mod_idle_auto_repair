# Tasks: expand-color-scheme-categories

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~150–200 + `.zip` artifact |
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
- [x] 0.2 Confirm plugin compiles (10m/L/—/`./gradlew :compileKotlin`)
- [x] 0.3 Confirm plugin tests pass (10m/M/—/`./gradlew :test` green except pre-existing `XsStartupActivityTest.autoDetectedModsAreSavedAndNotified` env failure)
- [x] 0.4 Note current `pluginVersion` is `0.2.3` (2m/L/—)

## Phase 1: RED — Failing tests

- [x] 1.1 Update `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt` (10m/M/—/R1,R2,R4):
  - Replace/rename the lowercase display-name test with `test_display_name_is_XS_uppercase` asserting `"XS"`.
  - Add `test_attribute_descriptors_contain_12_new_keys` asserting each of: `IDENTIFIER_UNDER_CARET`, `MATCHED_BRACE`, `UNMATCHED_BRACE`, `UNKNOWN_SYMBOL`, `BRACES`, `BRACKETS`, `COMMA`, `DOT`, `OPERATION_SIGN`, `OVERLOADED_OPERATOR`, `PARENTHESES`, `SEMI_COLON`.
  - Keep `test_existing_six_categories_still_present` for Keyword, String, Comment, Number, Identifier, Default.
- [x] 1.2 Run targeted tests; confirm FAIL (10m/M/1.1/capture RED evidence)
- [x] 1.3 Record RED evidence in `verify-report.md` (5m/L/1.2)

## Phase 2: GREEN — Minimal fix

- [x] 2.1 Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt` with the 12 inherited `TextAttributesKey` declarations and their fallback keys per design (10m/L/1.3)
- [x] 2.2 Update `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt` (15m/M/2.1):
  - Rename `getDisplayName()` from `"xs"` to `"XS"`.
  - Append the 12 new `AttributesDescriptor`s grouped under `Code//`, `Errors and Warnings//`, and `Braces and Operators//`.
  - Extend `TAG_MAP` and `DEMO_TEXT` with representative tags (`<brace>`, `<bracket>`, `<paren>`, `<comma>`, `<semicolon>`, `<dot>`, `<op>`) for the preview.
- [x] 2.3 Run targeted `XsColorSettingsPageTest`; confirm PASS (10m/M/2.2)
- [x] 2.4 Run `./gradlew :compileKotlin` to verify no compile errors (5m/L/2.3)

## Phase 3: GREEN — Build verification

- [x] 3.1 Run `./gradlew :test` — all green except pre-existing env failure (10m/M/2.4)
- [x] 3.2 Run `./gradlew :buildPlugin` — `.zip` produced (10m/L/3.1)
- [x] 3.3 Bump `pluginVersion` to `0.3.0` in `tools/intellij-xs-plugin/gradle.properties` per AGENTS.md minor policy for new settings UI (2m/L/3.2)
- [x] 3.4 Re-run `./gradlew :buildPlugin` — artifact name and `META-INF/plugin.xml` reflect `0.3.0` (10m/L/3.3)
- [x] 3.5 Inspect `.zip` `META-INF/plugin.xml`; confirm `<colorSettingsPage>` and `0.3.0` version are present (5m/L/3.4)

## Phase 4: REFACTOR

- [x] 4.1 Add KDoc to `XsTextAttributes.kt` explaining each category and its inherited fallback key (5m/L/3.5)
- [x] 4.2 Add KDoc to `XsColorSettingsPage.kt` explaining the grouping structure and the use of `//` for nested subcategories (5m/L/3.5)

## Phase 5: VERIFY

- [x] 5.1 Run final `./gradlew :test` (10m/M/4.2)
- [x] 5.2 Run final `./gradlew :buildPlugin` (5m/L/5.1)
- [x] 5.3 Note any new warnings from 5.1/5.2 (5m/L/5.2)
- [x] 5.4 Write `openspec/changes/expand-color-scheme-categories/verify-report.md` (10m/L/5.3)

## Phase 6: DOCUMENT

- [x] 6.1 Update `docs/issues/2026-06-29-runtime-issues.md` Issue 3: mark **Resolved 2026-06-29** by this change and add a scope-deferred note that semantic-token-driven categories remain future work (5m/L/5.4)

## Phase 7: COMMIT

- [ ] 7.1 Stage intended files (5m/M/6.1):
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt` (new)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt` (modified)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt` (updated)
  - `tools/intellij-xs-plugin/gradle.properties` (version bump)
  - `docs/issues/2026-06-29-runtime-issues.md` (resolved note)
  - SDD artifacts
  - Note: `dist/intellij-xs-plugin-0.3.0.zip` is gitignored and should not be staged.
- [ ] 7.2 Prepare commit message: `feat(xs-plugin): expand color scheme categories with renamed 'XS' label + 12 inherited Rider categories (Issue #3 partial)` (2m/L/7.1)

## Review Workload Forecast

- Files changed: ~4 Kotlin/properties/docs files + binary artifact
- New test files: 0 (updated 1)
- Total changed lines: ~150–200 + `.zip`
- Chained PRs recommended: No
- 400-line budget risk: Low
- Decision needed before apply: No
