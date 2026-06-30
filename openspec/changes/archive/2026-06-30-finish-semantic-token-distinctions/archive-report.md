# Archive Report — `finish-semantic-token-distinctions`

**Status**: ARCHIVED ✅
**Commit**: `dfa829d080211bfa2be6c42d6ca340a30c19b78d`
**Branch**: `xs-lsp-roundtrip-followup`
**Archived on**: 2026-06-30

## Outcome

The Bucket C semantic-token work that was deferred from slices `1bc73e4` (quick wins), `874dae9` (LSP tokens), and `88310ef` (plugin tokens) is now complete. Class extraction, constant/rule distinction, extern variable distinction, and the platform wiring (`XsSemanticTokensSupport` + `lspSemanticTokensSupport`) are all implemented and verified.

## Specs promoted to baseline

- `openspec/specs/spec-lsp-class-extraction.md` — LSP symbol-table class extraction
- `openspec/specs/spec-lsp-semantic-token-distinctions.md` — constant/rule/extern distinction + plugin wiring

## Specs updated

No existing `openspec/specs/spec-lsp-semantic-tokens.md` baseline file was present, so no existing spec was modified during this archive. The partial Bucket C semantic-token spec remains in its original slice folder (`add-lsp-semantic-tokens/` / `add-plugin-semantic-tokens/`) and will be archived separately when requested.

## Deviations accepted

1. `SemanticTokenType::new("constant")` instead of `CONSTANT` (pinned `tower_lsp` lacks CONSTANT).
2. `lspSemanticTokensSupport` direct property instead of `LspCustomization` wrapper (the 2024.2.2 platform API).
3. `type_identifier` node kind handled alongside `_type_identifier` (grammar alias).
4. `XsStartupActivityTest` tolerance fix (legitimate test-hygiene fix).
5. 43 pre-existing clippy warnings unchanged by this change.

## Test counts

- LSP: 230 → 239 (+9)
- Plugin: 91 → 96 (+5)

## pluginVersion

0.6.0 → 0.7.0 (MINOR)

## Platform resolution

`platformVersion = 2024.2.2`, `pluginSinceBuild = 242.22855.74`. `platformVersion_2 = 2025.2` was removed.

## Build artifact

`dist/intellij-xs-plugin-0.7.0.zip` (4,137,228 bytes).

## Open follow-ups (NOT in this change)

- `fix-lsp-cross-file-forward-decl-ordering` — class forward-decl in include chain.
- `expand-color-scheme-classes-constants-rules` — Bucket C remainder: class member extraction.
- `clippy cleanup` — 43 pre-existing warnings.
- Working-tree noise: rustfmt churn across 22 LSP files, user's `scripts/deploy-mods.sh`, untracked `mod/spire_ai/`, `mod/test_targeting/`, research files.
- 3 unarchived Bucket C slice folders (`fix-color-scheme-quick-wins`, `add-lsp-semantic-tokens`, `add-plugin-semantic-tokens`) and the umbrella plan (`expand-color-scheme-semantic-tokens`) at `openspec/changes/` — to be archived separately if/when the user wants.
