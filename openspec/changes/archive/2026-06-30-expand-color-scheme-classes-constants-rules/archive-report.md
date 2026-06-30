# Archive Report — `expand-color-scheme-classes-constants-rules`

**Status**: ARCHIVED ✅
**Implementation commits**: `9cc7344`, `e512f10`
**Verify commit**: `e512f10` (fix for blocker found by verifier)
**Branch**: `xs-lsp-roundtrip-followup`
**Archived on**: 2026-06-30

## Outcome
The Bucket C remainder (class member extraction) is now complete. Class fields and methods are extracted into the symbol table with `class_owner` references, member references (`obj.field`, `Class.method()`) are recognized and emit a `member` semantic-token modifier with origin classification, and 6 new color categories (`Method Engine/UnModded/Modded`, `Field Engine/UnModded/Modded`) are wired in the plugin.

## Specs promoted to baseline
- `openspec/specs/spec-lsp-class-member-extraction.md` — LSP symbol-table class member extraction.
- `openspec/specs/spec-lsp-class-member-semantic-tokens.md` — `member` semantic-token modifier + 6 plugin color categories.

## Specs updated
- `openspec/specs/spec-lsp-class-extraction.md` (already cross-references the new member-extraction spec, modified in the prior slice's apply commit).

## Verify → fix loop
The verifier flagged a blocker in `server.rs::build_document_symbol_tree`: it drained `pending_members` when seeing a `Class` symbol, but `extract_class` emits the class BEFORE its members, so the buffer was always empty. Members were silently dropped from `textDocument/documentSymbol`. Commit `e512f10` rewrote the function to use a two-pass HashMap-based approach and added a regression test (`class_members_nest_under_their_class_in_document_symbol_tree`).

## Deviations accepted
1. `DefaultLanguageHighlighterColors.STATIC_METHOD` / `INSTANCE_FIELD` were available on 2024.2.2 — no fallback needed (vs. the design's `LOCAL_VARIABLE` / `FUNCTION_DECLARATION` fallback plan).
2. `docs/issues/2026-06-29-runtime-issues.md` does not list this follow-up, so no update was required.
3. 45+ pre-existing clippy warnings unchanged by this change. The "5 new warnings" claim from the verifier was traced to pre-existing `deprecated:` field usages in `symbol_to_lsp_child` / `symbol_to_workspace_symbol`, not new code.

## Test counts
- LSP: 239 → 253 (+14)
- Plugin: 96 → 102 (+6)

## pluginVersion
0.7.0 → 0.8.0 (MINOR).

## Platform resolution
Unchanged from prior slice: `platformVersion = 2024.2.2`, `pluginSinceBuild = 242.22855.74`.

## Build artifact
`dist/intellij-xs-plugin-0.8.0.zip` (4,143,325 bytes).

## Open follow-ups (NOT in this change)
- `fix-lsp-cross-file-forward-decl-ordering` — class forward-decl in include chain.
- `clippy cleanup` — 45+ pre-existing warnings.
- Working-tree noise: rustfmt churn across 22 LSP files, user's `scripts/deploy-mods.sh`, untracked `mod/spire_ai/`, `mod/test_targeting/`, research files.
- 4 unarchived pre-existing folders at `openspec/changes/`:
  - `expand-color-scheme-semantic-tokens/` (Bucket C umbrella plan)
  - `fix-color-scheme-quick-wins/` (Slice 1 of Bucket C, commit `1bc73e4`)
  - `add-lsp-semantic-tokens/` (Slice 2, commit `874dae9`)
  - `add-plugin-semantic-tokens/` (Slice 3, commit `88310ef`)
