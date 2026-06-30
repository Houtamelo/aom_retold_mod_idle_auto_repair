# Archive Report: Fix semantic-token color application in editor

> **Change**: `2026-06-30-fix-semantic-token-color-application`
> **Branch**: `xs-lsp-roundtrip-followup` at HEAD `fe95928`
> **Archived**: 2026-06-30
> **Plugin version**: 0.8.2 (PATCH bump from 0.8.1)

## Outcome

The two user-reported rendering bugs are fixed:

1. Identifier sub-categories (variables, functions, rules, constants, methods, fields) now render with their dedicated `TextAttributesKey` in real `.xs` files, distinguishable per origin (engine / modded / unmodded) and storage class (local / static / extern / member).
2. Built-in type usages (`int`, `string`, `bool`, `float`, `vector`) now render with the dedicated **Type → Built-in Type** category (a non-keyword, type-looking color inherited from the platform `IDENTIFIER` default).

## Specs Synced

| Domain | Action | Details |
|--------|--------|---------|
| `xs-plugin-semantic-token-application` | Created | New flat spec at `openspec/specs/spec-xs-plugin-semantic-token-application.md`. 4 ADDED requirements + 1 REMOVED requirement (TextMate `storage_types` pre-emption of primitive type coloring). |

## Delta Summary

| Section | Count |
|---------|-------|
| ADDED Requirements | 4 |
| MODIFIED Requirements | 0 |
| REMOVED Requirements | 1 |
| Total Scenarios | 7 |

## Verdict

**PASS WITH WARNINGS** (per `verify-report.md`)

The 3 WARNINGs are test-coverage gaps, not functional defects:

1. Modifier preservation in converter — UNTESTED runtime path. Logic covered indirectly by `XsSemanticTokensConverterTest`.
2. IDENTIFIER language default fallback — UNTESTED runtime path.
3. Identifiers still match via `c_function_call` TextMate rule — optional scenario; existing TextMate tests pass.

## Files Changed (per `fe95928`)

| File | Action | ~LOC |
|------|--------|------|
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt` | Modify | +18/-15 |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt` | Modify | +1/-1 |
| `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt` | Modify | +6/-2 |
| `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` | Modify | -18 |
| `tools/intellij-xs-plugin/gradle.properties` | Modify | +1/-1 |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupportTest.kt` | Create | +25 |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterTest.kt` | Modify | +8 |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsColorSettingsPageTest.kt` | Modify | +13 |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/XsTextMateBundleFormatTest.kt` | Modify | +39 |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterFactoryTest.kt` | Modify | +11/-4 |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/XsTextMateHighlightingTest.kt` | Modify | +0/-1 |
| `AGENTS.md` | Modify | +1/-1 |

## Test Counts

| Layer | Before | After | Delta |
|-------|--------|-------|-------|
| Plugin | 102 | 107 | +5 |
| LSP | 253 | 253 | 0 |

## Parallel-work observation

User signaled parallel LSP work during this cycle. At archive time, `git status -s` showed no uncommitted LSP files. No LSP files were included in commit `fe95928`.

## Archive Contents

- `proposal.md` ✅
- `specs/xs-plugin-semantic-token-application/spec.md` ✅
- `design.md` ✅
- `tasks.md` ✅
- `verify-report.md` ✅
- `archive-report.md` ← this file

## Source of Truth Updated

The following flat spec is now the source of truth for the new behavior:
- `openspec/specs/spec-xs-plugin-semantic-token-application.md`

## SDD Cycle Complete

The change has been fully planned, implemented, verified, and archived. Ready for the next change.
