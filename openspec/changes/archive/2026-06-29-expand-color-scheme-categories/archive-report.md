# Archive Report: expand-color-scheme-categories

## Status
Archived

## Date
2026-06-29

## Artifacts
- Spec (NEW): `openspec/specs/spec-xs-color-scheme-categories.md`
- Spec (synced from prior): N/A
- Design: `openspec/changes/archive/2026-06-29-expand-color-scheme-categories/design.md`
- Tasks: `openspec/changes/archive/2026-06-29-expand-color-scheme-categories/tasks.md`
- Verify report: `openspec/changes/archive/2026-06-29-expand-color-scheme-categories/verify-report.md`
- Proposal: `openspec/changes/archive/2026-06-29-expand-color-scheme-categories/proposal.md`
- Exploration: `openspec/changes/archive/2026-06-29-expand-color-scheme-categories/explore.md`

## Outcome
This change resolves Issue #3 by expanding the IntelliJ XS color scheme settings page. The page display name is renamed from `"xs"` to `"XS"`, and twelve inherited Rider/IntelliJ `General` / `Language Defaults` categories are exposed under **Settings → Editor → Color Scheme → XS**: `Code / Identifier under caret`, `Code / Matched brace`, `Code / Unmatched brace`, `Errors and Warnings / Unknown symbol`, and the eight `Braces and Operators` entries (Braces, Brackets, Comma, Dot, Operation sign, Overloaded operator, Parentheses, Semi-colon). The implementation follows Option α from the proposal: new `TextAttributesKey`s are declared in `XsTextAttributes.kt`, registered via `AttributesDescriptor` entries in `XsColorSettingsPage.kt`, preview tags are extended, and the existing `XsSyntaxHighlighter` is left unchanged so there is zero regression risk for current highlighting.

Bucket C — semantic-token-driven categories such as engine/modded/unmodded functions, variables, constants, types/classes, and built-in type as a dedicated semantic category — is intentionally deferred to a future change. That future work depends on LSP `textDocument/semanticTokens` and a plugin-side token-to-`TextAttributesKey` mapper, which are out of scope here. The warnings in the verify report (preview does not demonstrate every new category, actual editor token wiring is unchanged) are acceptable under Option α and should be addressed when Bucket C is implemented.

## Test count trajectory
- Before: 67 (after the previous archived change `fix-plugin-goto-definition-ctrl-click-keybind`)
- After: 68 (`XsColorSettingsPageTest` now contains 5 passing tests, including 3 new assertions for the uppercase display name and the 12 new descriptor keys)

## Plugin version
Bumped `0.2.3 → 0.3.0` (MINOR, per `AGENTS.md` "new settings UI / new LSP feature / new grammar scope" policy).

## Cross-references
- User issue: `docs/issues/2026-06-29-runtime-issues.md` Issue 3 (now marked Resolved with archive path and deferred-scope note)
- Related archived changes:
  - `openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/` (Issue #1)
  - `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/` (Issue #2)
- Version policy: `AGENTS.md` plugin version bump policy
- Parent spec context: `openspec/specs/spec-color-settings-page.md` (existing main spec for the XS color settings page)

## Commit guidance
The orchestrator will commit the change with the following Conventional Commits message:
```
feat(xs-plugin): expand color scheme categories with renamed 'XS' label + 12 inherited Rider categories (Issue #3 partial)

Adds com.aomr.xs.highlight.XsTextAttributes with 12 inherited
TextAttributesKey categories, registers them on XsColorSettingsPage
under Code / Errors and Warnings / Braces and Operators, renames the
page display name to "XS", and extends preview tags/DEMO_TEXT.
XsColorSettingsPageTest asserts uppercase name and the 12 new keys.

Bumps pluginVersion 0.2.3 → 0.3.0.
Partially resolves Issue #3 from docs/issues/2026-06-29-runtime-issues.md.
```

### Files to stage
Stage only the intended files:
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt` (new)
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt` (modified)
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt` (updated)
- `tools/intellij-xs-plugin/gradle.properties` (version bump)
- `docs/issues/2026-06-29-runtime-issues.md` (resolved note + archive path)
- `openspec/specs/spec-xs-color-scheme-categories.md` (promoted)
- `openspec/changes/archive/2026-06-29-expand-color-scheme-categories/` (archived SDD artifacts)

Do **not** stage the gitignored `dist/intellij-xs-plugin-0.3.0.zip`, unrelated `scripts/deploy-mods.sh` changes, rustfmt churn in `tools/xs-language-server/`, or untracked `mod/spire_ai/` / `mod/test_targeting/` directories.
