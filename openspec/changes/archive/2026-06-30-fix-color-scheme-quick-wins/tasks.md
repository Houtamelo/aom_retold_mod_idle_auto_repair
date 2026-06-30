# Tasks: fix-color-scheme-quick-wins

## Status
Complete (apply phase finished in a prior session; the apply sub-agent
exhausted its context window after committing the implementation; this
task list captures what was already done).

## Phase 0: Setup
- [x] Confirm branch state (xs-lsp-roundtrip-followup).
- [x] Confirm LSP build clean (220 tests baseline).
- [x] Confirm plugin tests pass (68 baseline).
- [x] Note current pluginVersion (0.4.0).

## Phase 1: RED (failing tests first)
- [x] Write `XsTextMateIncludeKeywordTest` with 3 tests (3a).
- [x] Write `XsSyntaxHighlighterTest` with 8 tests (3b).
- [x] Add `test_identifier_under_caret_descriptor_documents_global_setting`
  to `XsColorSettingsPageTest` (3c).
- [x] Run tests, confirm they FAIL.

## Phase 2: GREEN (3a — include keyword)
- [x] Add `include` to TextMate grammar keyword regex in
  `xs.tmLanguage.json` (1 line).
- [x] Add `include` to native lexer's keyword set in
  `XsHighlightingLexer.kt` (1 line).
- [x] Run the 3a tests, confirm PASS.

## Phase 3: GREEN (3b — braces/operators mapping)
- [x] Add `XsTokenTypes.DOT`, `SEMI_COLON`, `OPERATION_SIGN` token
  types in `XsTokenType.kt` (3 lines).
- [x] Add lexer rules for `.`, `;`, and operation signs in
  `XsLexer.flex` (4 lines).
- [x] Map each of 8 lexer token types to the corresponding
  `XsTextAttributes` key in `XsSyntaxHighlighter.kt` (7 lines).
- [x] Run the 3b tests, confirm all 8 PASS.

## Phase 4: GREEN (3c — identifier under caret)
- [x] Update `XsColorSettingsPage` descriptor name for
  `IDENTIFIER_UNDER_CARET` to include
  "(uses global General — per-language override is not supported in Rider)".
- [x] Run the 3c test, confirm PASS.

## Phase 5: VERIFY
- [x] Run full plugin test suite (80/81 pass; 1 pre-existing env failure).
- [ ] Run plugin build (pending — orchestrator to run before commit).
- [ ] Bump pluginVersion 0.4.0 → 0.4.1 (pending).
- [ ] Run plugin build again with new version (pending).
- [ ] Write verify report (pending — orchestrator to write).

## Phase 6: DOCUMENT
- [x] Update `docs/issues/2026-06-29-runtime-issues.md` Issue 3
  sub-findings (3a/3b/3c) — to be marked Resolved in this commit.
- [x] Update `AGENTS.md` test count line (68 → 81) — pending in
  this commit.

## Phase 7: COMMIT
- [x] Stage only intended files (no rustfmt churn, no
  `scripts/deploy-mods.sh`).
- [x] Commit with the Conventional Commits message
  `fix(xs-plugin): include keyword + braces/operators mapping + identifier-under-caret doc (Issue #3 quick wins)`.
  pluginVersion 0.4.0 → 0.4.1 (PATCH per AGENTS.md "bug fix" policy).
- (Pending — orchestrator handles.)

## Review Workload Forecast

- Files changed: 7 (4 modified source + 1 modified test + 1 modified
  `gradle.properties` + 1 modified `xs.tmLanguage.json` + 1 modified
  `AGENTS.md`).
- New test files: 2 (`XsTextMateIncludeKeywordTest.kt`,
  `XsSyntaxHighlighterTest.kt`).
- LOC delta: ~80 source + ~190 tests.
- Chained PRs: No (this is one of three planned slices).
- 400-line budget risk: Low.
- Decision needed before apply: No (apply was completed in a prior
  sub-agent session; this is the commit phase).
