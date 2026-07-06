# Design: LSP Signature Help and Document Link

## Technical Approach

Add two orthogonal LSP request handlers to the XS language server:

- `textDocument/signatureHelp` — triggered on `(` and `,`; returns the call
  signature and highlighted active parameter for engine syscalls, workspace
  callables (functions, rules), or `null` if the callee cannot be resolved.
- `textDocument/documentLink` — returns clickable links for every
  `include "..."` directive whose target resolves through the workspace.

Both handlers reuse existing data sources (`engine_api::SharedEngineApi`,
`workspace::Workspace`, `merged_view::MergedView`, `parser` helpers) and follow
existing handler patterns in `server.rs`. No XS mod source files are changed.

```text
  cursor in call site
        │
        ▼
  server::signature_help()
        │
        ├─ document store ── text buffer
        ├─ engine_api.find_syscall() ── engine signature
        ├─ merged_view.find() ── workspace callable
        └─ symbol_tables ── current-file fallback
        │
        ▼
  pure helper: (callee, activeParameter) → SignatureHelp

  textDocument/documentLink
        │
        ▼
  server::document_link()
        │
        ├─ document store ── text buffer
        ├─ parser::extract_include_directives()
        ├─ workspace.resolve_include_for_file()
        └─ Url::from_file_path(target)
        │
        ▼
  Vec<DocumentLink>
```

## Architecture Decisions

### Decision: dedicated modules for the two handlers

**Choice**: add `lsp/src/signature_help.rs` and `lsp/src/document_link.rs`,
declared in `lsp/src/lib.rs`, and called from `server.rs` trait methods.

**Alternatives**: inline logic directly in `server.rs` trait methods.

**Rationale**: matches the project precedent (`semantic_tokens.rs` is its own
module, `workspace.rs` is its own module) and keeps the `LanguageServer` impl
readable. Pure helper functions inside the modules are easy to unit-test.

**Trade-offs**: two extra files vs. a larger `server.rs`; the split pays off
for independent review and TDD.

### Decision: engine API takes precedence for signature help

**Choice**: try `engine.find_syscall(name)` first; if missing, fall back to
`merged_view.find(name)`; if still missing, fall back to the current-file
`symbol_tables` entry.

**Alternatives**: workspace-first (mod overrides can shadow engine names).

**Rationale**: this mirrors `hover` and `semantic_tokens`: engine signatures
are richer and stable, while a workspace symbol that re-uses an engine name
would only shadow through explicit redefinition. The existing precedence rule
(engine → merged → own table) already handles this consistently.

**Trade-offs**: a mod that intentionally redefines an engine API name would
show the engine signature. We accept this because XS engine names are reserved
in practice.

### Decision: trigger characters are `(` and `,` only

**Choice**: advertise `trigger_characters: ["(", ","]` and no
`retrigger_characters`.

**Alternatives**: also trigger on `;`, whitespace, or identifier chars.

**Rationale**: `explore.md` §3 notes that extra trigger characters risk racing
with the editor brace matcher. `(` and `,` are the only characters that change
the active parameter in a meaningful way.

**Trade-offs**: signature help only appears when the user types these chars;
clients that auto-trigger on identifier typing will not show help until `(`.

### Decision: resolve provider is `false` for both capabilities

**Choice**: `SignatureHelpOptions { resolve_provider: None, ... }` and
`DocumentLinkOptions { resolve_provider: false, ... }`.

**Alternatives**: implement `signatureHelpResolve` or `documentLinkResolve`.

**Rationale**: engine and workspace data is static enough to resolve up front;
no extra round-trip is needed. This mirrors how `diagnostic_provider` uses
static options and matches tower-lsp defaults.

**Trade-offs**: if a future feature wants lazy parameter documentation,
`resolve_provider` must be added then.

### Decision: lock acquisition follows R3-F-01 global order

**Choice**: each handler holds only the locks it needs, never more than one
mutex guard at a time.

- `document_link` holds `documents` only.
- `signature_help` holds `documents`, then may call
  `get_or_build_merged_view`, which already scopes `workspace`,
  `symbol_tables`, and `merged_views` one at a time.

**Rationale**: R3-F-01 established this order after a deadlock in
`textDocument/didClose`. New handlers must not reintroduce multi-lock races.

**Trade-offs**: merged-view build may be redone concurrently; the cache keeps
it cheap and race-safe.

### Decision: document link target is a file URI

**Choice**: convert the resolved absolute path to a `tower_lsp_server::ls_types::Uri`
via `Uri::from_file_path`, consistent with how other handlers return locations.

**Alternatives**: return a raw filesystem path string.

**Rationale**: the LSP spec requires `target` to be a URI, and existing code
already uses `Uri::from_file_path` for `goto_definition` results.

**Trade-offs**: Windows percent-encoding differences are handled by the
`tower-lsp-server` `Uri` type.

### Decision: accept include-resolution TOCTOU (R4-F-14)

**Choice**: `workspace.resolve_include_for_file` checks file existence at
request time; if the file disappears before the user clicks the link, the link
may 404. Mitigation is `did_change_watched_files`, which invalidates parse and
merged-view caches and will cause the client to re-request links.

**Alternatives**: subscribe to FS watchers inside the handler or continuously
cache link state.

**Rationale**: cheap and consistent with the existing `goto_definition` and
hover behavior.

**Trade-offs**: a deleted/renamed target can appear clickable for a short
window. The impact is a single failed client navigation, not stale diagnostics.

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `tools/xs-language-server/lsp/src/lib.rs` | Modify | Add `pub mod signature_help;` and `pub mod document_link;` |
| `tools/xs-language-server/lsp/src/server.rs` | Modify | Override `signature_help` and `document_link`; update `ServerCapabilities` |
| `tools/xs-language-server/lsp/src/signature_help.rs` | Create | Call-site analysis + signature/activeParameter construction |
| `tools/xs-language-server/lsp/src/document_link.rs` | Create | Include scan + link range/target generation |
| `tools/xs-language-server/lsp/tests/signature_help_repro.rs` | Create | Integration tests for engine and workspace signatures |
| `tools/xs-language-server/lsp/tests/document_link_repro.rs` | Create | Links for resolved includes, omission for missing targets |
| `tools/xs-language-server/lsp/tests/game_folder_parse.rs` | Modify | Note known-good delta: new handlers do not affect parse/symbol thresholds |
| `tools/xs-language-server/lsp/src/bin/lsp_roundtrip_test.rs` | Modify | Add `textDocument/signatureHelp` and `textDocument/documentLink` sequence assertions |
| `tools/intellij-xs-plugin/gradle.properties` | Modify | Bump `pluginVersion` `0.10.0` → `0.11.0` (separate commit) |

## Interfaces / Contracts

```rust
// signature_help.rs
pub fn signature_help(
    source: &str,
    pos: Position,
    engine: &engine_api::SharedEngineApi,
    merged: Option<&MergedView>,
    own_table: Option<&symbols::SymbolTable>,
) -> Option<SignatureHelp>;

pub fn find_call_at_cursor(source: &str, line: u32, character: u32)
    -> Option<(String, u32)>; // (callee, active_parameter)

// document_link.rs
pub fn document_links(
    source: &str,
    current_file: &std::path::Path,
    workspace: &workspace::Workspace,
    project: &workspace::VirtualProject,
) -> Vec<DocumentLink>;
```

- `find_call_at_cursor` is a pure function: walk backward from the cursor,
  skip balanced pairs, and stop at the nearest unmatched `(`. The identifier
  immediately before it is the callee. `active_parameter` counts commas at
  nesting depth 0 between that `(` and the cursor, capped to
  `param_count - 1`.
- `document_links` calls `parser::extract_include_directives` to get the
  quoted-path range, shrinks it by one character on each side, resolves the
  target, and returns a link only when resolution succeeds.

## Type Wiring (`ls-types` 0.0.6)

Mirror `semantic_tokens.rs` imports:

```rust
use tower_lsp_server::ls_types::{
    SignatureHelp, SignatureHelpParams, SignatureInformation, ParameterInformation,
    Documentation, MarkupContent, MarkupKind, SignatureHelpOptions,
    DocumentLink, DocumentLinkParams, DocumentLinkOptions, Range, Position,
};
```

- `ParameterInformation.label` is the string `"type name"` (with the documented
  default value appended when available).
- `SignatureInformation.documentation` is
  `Documentation::MarkupContent(MarkupContent { kind: MarkupKind::Markdown, value: ... })`.
- `ServerCapabilities` fields:
  - `signature_help_provider: Some(SignatureHelpOptions { trigger_characters: Some(vec!["(".into(), ",".into()]), ..Default::default() })`
  - `document_link_provider: Some(DocumentLinkOptions { resolve_provider: false, ..Default::default() })`

## Test Strategy

Strict TDD per `strict-tdd` skill:

| Handler | RED | GREEN | TRIANGULATE | REFACTOR |
|--------|-----|-------|------------|----------|
| `signature_help` | `tests/signature_help_repro.rs` fails to compile/import `signature_help` module; adding an empty `signature_help` module makes the test compile but fail assertions | Return `null` for everything; first assertion (`kbUnitCreate` has a signature) passes | Add unknown-fn `null` case and workspace-function case | Extract `find_call_at_cursor` and `build_signature_information` helpers |
| `document_link` | `tests/document_link_repro.rs` fails to import `document_link` module | Return `[]` for everything; first assertion (`include "core.xs"` resolves to one link) passes | Add unresolved-include omission case and mod-overlay precedence case | Extract include-range shrink helper |

### Integration fixtures

- **signature help**: open `/tmp/test.xs`:
  ```xs
  void test() {
     kbUnitCreate(0, -1, 1);
     unknownFn(1, 2);
  }
  ```
  Assert `signatureHelp` at `(` for `kbUnitCreate` returns a signature whose
  label contains `kbUnitCreate` and `activeParameter == 0`; assert at the
  first comma returns `activeParameter == 1`; assert `unknownFn` returns
  `null`.
- **document link**: open `/tmp/links.xs`:
  ```xs
  include "core.xs";
  include "does/not/exist.xs";
  ```
  (resolve `"core.xs"` in the fixture workspace). Assert exactly one link,
  range excludes quotes, target URI ends with `core.xs`.

### `lsp_roundtrip_test.rs` additions

- Add `textDocument/signatureHelp` request for `kbUnitCreate` in the baseline
  fixture and assert `result.signatures` is non-empty and contains the
  function name.
- Add `textDocument/documentLink` request for a file with one resolvable
  include and assert `result` has one entry whose `target` is a `file://`
  URI (R5-F-01: assert JSON structure, not raw substrings).

## Risks + Mitigations

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Trigger chars race with brace matcher | Low | Only `(` and `,` advertised |
| `documentLink` filesystem TOCTOU (R4-F-14) | Low | Resolve at request time; rely on `did_change_watched_files` invalidation |
| Active-parameter heuristic miscounts nested calls | Low | Triangulate with nested-call fixture; cap to `param_count - 1` |
| Roundtrip test asserts raw substrings (R5-F-01/02/03) | Low | Parse JSON responses and assert typed fields |
| Lock-order regression | Low | Hold only one mutex guard at a time; scope `get_or_build_merged_view` lock acquisitions |

## Out of Scope

Per the proposal:

- `signatureHelpResolve`
- `textDocument/codeAction`
- `textDocument/completion` enhancements
- `textDocument/documentHighlight`
- `textDocument/didSave`
- `textDocument/inlayHint`
- `workspace/didChangeConfiguration`
- formatting, pull diagnostics, `executeCommand`, `*Refresh`
- Removing the advertised-but-unimplemented `diagnostic_provider` capability

## Definition of Done

- [ ] `signature_help` and `document_link` trait methods wired.
- [ ] `ServerCapabilities` advertises `signatureHelpProvider` and
      `documentLinkProvider` with the correct options.
- [ ] `cargo test --manifest-path tools/xs-language-server/Cargo.toml` green
      (369 existing tests unaffected; 1 pre-existing flaky remains).
- [ ] New integration tests for both handlers committed and passing.
- [ ] `lsp_roundtrip_test` asserts both requests.
- [ ] `pluginVersion` bumped `0.10.0` → `0.11.0` in a separate commit.

## Plugin Version Bump Detail

Current: `tools/intellij-xs-plugin/gradle.properties:pluginVersion = 0.10.0`
Target: `0.11.0` (MINOR bump per `AGENTS.md` because this adds new LSP
features). The bump must be an isolated commit; it changes only
`gradle.properties`.

## Sequencing for the Apply Phase

Both features are independent (no shared files or state). Either PR can land
first.

- **PR-1**: `signature_help.rs`, capability flag, `signature_help_repro.rs`.
  RED → GREEN → TRIANGULATE → REFACTOR.
- **PR-2**: `document_link.rs`, capability flag, `document_link_repro.rs`.
  RED → GREEN → TRIANGULATE → REFACTOR.
- **PR-3**: Extend `lsp_roundtrip_test.rs` and add known-good-delta note to
  `tests/game_folder_parse.rs`. Run full LSP roundtrip.
- **PR-4** (optional/isolated): `pluginVersion` `0.10.0` → `0.11.0`.
