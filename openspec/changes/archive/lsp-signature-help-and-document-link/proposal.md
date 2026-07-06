# Proposal: Signature help and document links for the XS LSP

## Intent

Implement Change 1: `textDocument/signatureHelp` and `textDocument/documentLink`. XS files are dense with engine calls (`kb*`, `ai*`, `tr*`, `xs*`); `signatureHelp` reuses `engine.syscalls` data. `documentLink` makes `include "..."` paths clickable.

No XS mod files are touched; `human_assist.xs` overlay is unaffected.

## Scope

### In Scope
- `textDocument/signatureHelp` handler + capability advertisement
- `textDocument/documentLink` handler + capability advertisement
- Regression tests
- `ServerCapabilities` update

### Out of Scope
- Tier A: `codeAction` → `lsp-code-action-forward-declaration`; `completion` review → `lsp-completion-enhancements`
- Tier B: `documentHighlight` → `lsp-document-highlight`; `didSave` → `lsp-did-save-diagnostics`; `inlayHint` → `lsp-inlay-hints`; `didChangeConfiguration` → `lsp-did-change-configuration`
- formatting, pull diagnostics, executeCommand, *Refresh, skipped methods → future changes

## Capabilities

### New Capabilities
- `lsp-signature-help`: engine and workspace call signatures with active-param highlight
- `lsp-document-link`: clickable `include "..."` path tokens

### Modified Capabilities
- `lsp-server-capabilities`: adds `signatureHelpProvider` and `documentLinkProvider`

## Approach

- Wire `signature_help` and `document_link` on the `LanguageServer` trait.
- `signatureHelp`: triggers on `(` and `,`, looks up `engine.syscalls`, falls back to merged view / symbol table.
- `documentLink`: scans include directives and resolves targets via `workspace.resolve_include_for_file`.
- Advertise both providers.
- Add unit tests and extend `lsp_roundtrip_test`.

### Plugin version note

Current `tools/intellij-xs-plugin/gradle.properties:pluginVersion = 0.10.0`. New LSP features ⇒ apply phase should bump to **0.11.0** per `AGENTS.md`.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `lsp/src/server.rs` | Modified | Trait methods + capability flags |
| `lsp/src/engine_api.rs` | Reused | `find_syscall` lookup |
| `lsp/src/workspace.rs` | Reused | `resolve_include_for_file` |
| `lsp/src/signature_help.rs` | New | Signature + active-parameter computation |
| `lsp/src/document_link.rs` | New | Include scan + link generation |
| `lsp/tests/` | New | Regression tests |
| `lsp/src/bin/lsp_roundtrip_test.rs` | Modified | Sequence additions |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Trigger chars race with brace matcher | Low | Use `(` and `,` only |
| `documentLink` filesystem TOCTOU (R4-F-14) | Low | Reuse existing resolution; invalidate on watched-file changes |
| Large-file scan latency | Low | Linear scan of include directives only |

## Rollback Plan

1. Revert commit(s) adding `signature_help.rs`, `document_link.rs`, trait wiring, and capability flags.
2. No plugin or XS mod files change, so no rollback elsewhere.
3. The `0.11.0` bump can be skipped if unreleased.

## Dependencies

- `tower-lsp-server = "0.23.0"` and `ls-types = "0.0.6"` (already pinned)
- Engine API cache from `doxygen_retail.7z` (already loaded)


## Success Criteria

- [ ] Both handlers wired and providers advertised
- [ ] `cargo test` green
- [ ] New tests cover signature help and document links
- [ ] `lsp_roundtrip_test` exercises both requests
- [ ] No regressions in 369 existing tests (1 pre-existing flaky ignored)
