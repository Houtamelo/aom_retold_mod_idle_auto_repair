# Archive Report: lsp-signature-help-and-document-link

**Date**: 2026-07-06
**Status**: ARCHIVED

## Summary

The `lsp-signature-help-and-document-link` change adds two new LSP features to the XS language server: `textDocument/signatureHelp` for engine and workspace call signatures, and `textDocument/documentLink` for clickable `include "..."` paths. Both capabilities are advertised in `ServerCapabilities` during initialize. The IntelliJ plugin version was bumped from 0.10.0 to 0.11.0.

## Commits

- `32322e5` feat(xs-lsp): add textDocument/signatureHelp
- `f236dfc` feat(xs-lsp): add textDocument/documentLink
- `87830fb` test(xs-lsp): extend roundtrip coverage for signatureHelp + documentLink
- `c0d5e2b` chore(xs-plugin): bump pluginVersion to 0.11.0
- `1586749` chore(openspec): record SDD artifacts for lsp-signature-help-and-document-link
- `3e24758` test(xs-lsp): add coverage for signatureHelp defaults + active-param cap + 3-include documentLink; fix roundtrip harness race
- `ca5cf24` chore(openspec): update apply-progress for fix-cycle

## Specs Synced

- `lsp-signature-help` → `openspec/specs/lsp-signature-help/spec.md` (new)
- `lsp-document-link` → `openspec/specs/lsp-document-link/spec.md` (new)
- `lsp-server-capabilities` → `openspec/specs/lsp-server-capabilities/spec.md` (newly established)

## Verification

- Workspace tests: 387 / 0 / 0
- Live roundtrip: PASS
- Plugin build smoke: PASS
- All 18 spec scenarios: COMPLIANT

## Deviations (carryover)

| Deviation | Status |
|-----------|--------|
| Capability flags (`signatureHelpProvider`, `documentLinkProvider`) advertised in PR-1/PR-2 instead of planned PR-3 slot | Documented / accepted — net `ServerCapabilities` matches design |
| `tests/game_folder_parse.rs` per-change "known-good-delta" note treated as N/A | Documented / accepted — existing threshold assertions cover the change |
| `./gradlew :test` deliberately skipped during apply | Documented / moot — Gate 5 re-run passed |
| First apply delegation returned empty; orchestrator completed inline | Documented / historical — not a compliance issue |
| `self_referential_artifact_hash` placeholder in `apply-progress.md` | Documented / accepted — actual artifact commit is `ca5cf24`; source/test truth unaffected |

## pluginVersion

- Before: 0.10.0
- After: 0.11.0 (MINOR bump per AGENTS.md; new LSP feature)

## LSP Capabilities Added

- `textDocument/signatureHelp` (engine + workspace call signatures, active-parameter highlight)
- `textDocument/documentLink` (clickable include `"..."` path tokens)

## Rollback Plan (for reference)

Revert the 7 implementation commits in reverse order; the LSP server will fall back to the previous capability set (no signatureHelp, no documentLink).
