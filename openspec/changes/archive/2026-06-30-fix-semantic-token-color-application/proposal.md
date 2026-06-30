---
key: aom_retold_mod_idle_auto_repair/2026-06-30-fix-semantic-token-color-application
summary: Fix two IntelliJ plugin rendering bugs so identifier sub-categories and built-in type usages render with their dedicated TextAttributesKeys in real .xs files.
project: aom_retold_mod_idle_auto_repair
change: 2026-06-30-fix-semantic-token-color-application
status: Ready for spec
type: proposal
artifact_store: openspec
plugin_version_bump: 0.8.1 -> 0.8.2
branch: xs-lsp-roundtrip-followup
---

# Proposal: Fix semantic-token color application in editor

## Why

Two rendering bugs reported in Rider after the Bucket C semantic-token work only appear in real `.xs` files — the **Settings → Editor → Color Scheme → XS** synthetic preview renders correctly.

1. **Identifier sub-categories render as the generic `XS_IDENTIFIER` color.** Variables, functions, rules, constants, methods, and fields all share the same identifier color in edited `.xs` files.
2. **Built-in type usages (`int`, `string`, `bool`, `float`, `vector`) render as `XS_KEYWORD`**. They should use the dedicated **Type → Built-in Type** category.

The root causes are on the plugin side: the `XsSemanticTokensSupport` modifier legend is incomplete, the lexer-level identifier mapping pre-empts semantic tokens, and the TextMate `storage_types` rule forces primitive types into the keyword color before semantic tokens run.

## What Changes

- Identifier sub-categories render with their dedicated `TextAttributesKey` in edited `.xs` files (engine, modded, unmodded, local, static, extern, and member modifiers).
- Built-in type usages (`int`, `string`, `bool`, `float`, `vector`) render with the dedicated **Type → Built-in Type** category — a non-keyword, type-looking color inherited from the platform `IDENTIFIER` default.
- `XS_IDENTIFIER` moves from a per-token lexer fallback to a true schema-default for the language; unmatched identifiers fall back to it only when semantic tokens are absent.
- The `storage_types` TextMate rule that pre-empts semantic tokens for primitive types is removed.

## Impact

| Area | Impact |
|------|--------|
| `tools/intellij-xs-plugin/` | Modified |
| `tools/xs-language-server/` | Unchanged |
| Plugin tests | +3 to +5 new tests |
| Plugin version | PATCH: `0.8.1 → 0.8.2` |

## Risk

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Removing `XsSyntaxHighlighter.IDENTIFIER → XS_IDENTIFIER` leaves identifiers unstyled for ~250 ms while the LSP starts. | Low | Keep `XS_IDENTIFIER` as the converter fallback for tokens received without semantic metadata. |
| Removing `storage_types` from `xs.tmLanguage.json` could break another include. | Low | Grep confirms no `#storage_types` reference depends on the primitive-types match. |

## Out of Scope

- Shared-brace coloring issues.
- Comment-block bracket matching.
- Class member extraction beyond the modifiers already emitted by the LSP.
- Cross-file forward-declaration ordering.
- Recoloring existing categories whose keys already render correctly.

## Open Questions

None. All decisions are answered by the accompanying `explore.md`.
