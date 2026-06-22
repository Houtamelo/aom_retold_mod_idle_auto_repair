# Apply Progress — `intellij-xs-plugin` P0 Scaffold

## Batch summary

**Apply batch:** P0 scaffold (tasks 0.1–0.4)  
**Branch:** `intellij-xs-plugin/p0-scaffold`  
**Target:** `master` (repo default branch; the SDD artifact originally said `main`, but the repository uses `master`)  
**Completed at:** 2026-06-22

## What was completed

- [x] **0.1 — Gradle project skeleton**: pinned Kotlin 2.0.21, IntelliJ Platform Gradle Plugin 2.2.1, JVM target 21, Gradle 8.10.2 wrapper, root `.gitignore` carve-out for `tools/intellij-xs-plugin/`, plugin `.gitignore`, and README.
- [x] **0.2 — Plugin manifest + language/file type registration**: `plugin.xml` with id `com.aomr.xs`, name `XS Language Support`, dependencies on `com.intellij.modules.platform`, `org.jetbrains.plugins.textmate`, and optional `com.intellij.modules.rider`, plus `XsLanguage`, `XsFileType`, `XsFileTypeFactory`, and `icons/xs.svg`.
- [x] **0.3 — TextMate bundle vendoring and provider**: vendored `syntaxes/xs.tmLanguage.json` from `/tmp/opencode/xs_ext/extension/syntaxes/xs.json` with `fileTypes` removed; implemented `XsTextMateBundleProvider` and `XsTextMateHighlightingTest`; copied `human_assist.xs` as a test fixture.
- [x] **0.4 — Build validation + bundled-resource guard + CI**: added `validateBundledResources` Gradle task; created `.github/workflows/intellij-xs-plugin.yml` at the repository root running `./gradlew buildPlugin` and `./gradlew test` against the `master` branch.

## What was NOT completed

None — P0 is a clean, complete batch.

## Commits

- `cb80ddf`: `ci(repo): move intellij-xs-plugin workflow to root .github/workflows`
- `5a943a3`: `ci(tools/intellij-xs-plugin): target master branch in workflow`
- `cd5dd9d`: `chore(openspec): mark intellij-xs-plugin P0 tasks complete and record progress`
- `0895c7b`: `ci(tools/intellij-xs-plugin): build plugin + run tests on PR`
- `8099520`: `feat(tools/intellij-xs-plugin): vendor textmate xs grammar`
- `a004bfb`: `feat(tools/intellij-xs-plugin): register xs file type and language`
- `ff98d5e`: `chore(tools/intellij-xs-plugin): gradle skeleton + build plugin manifest`

## Verification status

| Check | Status | Notes |
|---|---|---|
| `./gradlew buildPlugin` | pass | Produces `build/distributions/intellij-xs-plugin-0.1.0.zip`; packaged `plugin.xml` has `since-build="242" until-build="252.*"` |
| `./gradlew test` | pass | `XsTextMateHighlightingTest.bundleProviderExposesGrammar` and `grammarScopeMapping` pass |
| `./gradlew validateBundledResources` | pass | Verified; deliberately removing `syscalls.json` fails with a clear message |
| `./gradlew runIde` | documented-not-run | Not executed in the headless sandbox; listed as the manual smoke test in README and PR description |

## Caveats

- `FileTypeFactory` is used because the task explicitly requested it, and it is deprecated in modern IntelliJ Platform. It is suppressed with `@file:Suppress("DEPRECATION")`; a future cleanup can migrate to the `<fileType>` extension point.
- `syscalls.json` and `aiplans.json` are intentionally empty JSON arrays as P0 placeholders so the bundled-resource guard passes. They will be replaced with vendored data in P1 and P4.

## PR status

Branch `intellij-xs-plugin/p0-scaffold` has been pushed to `origin`. A GitHub PR was **not created from the sandbox** because `gh` is not installed and no `GITHUB_TOKEN`/`GH_TOKEN` is present. Open the PR manually at:

`https://github.com/Houtamelo/aom_retold_mod_idle_auto_repair/pull/new/intellij-xs-plugin/p0-scaffold`

Target branch: `master`.

## Next apply batch

**P1 — Engine API surface** (tasks 1.1, then 1.2+1.3, 1.4, 1.5+1.6 as chained PRs).
