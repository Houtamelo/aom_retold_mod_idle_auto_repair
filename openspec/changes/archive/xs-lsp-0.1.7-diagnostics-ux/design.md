# Design: xs-lsp-0.1.7-diagnostics-ux

Fix four LSP server bugs around diagnostic clearing, parse-error messaging, and engine-API symbol navigation without touching plugin code. All changes live in `tools/xs-language-server/`.

## Architecture Decisions

### AD-1: Empty-publish for clean files

**Context:** `diagnostics::collect_all` only inserts a URI entry when a diagnostic category produces items (`diagnostics.rs:56-106`). `server.rs::publish_diagnostics` iterates the resulting map (`server.rs:968`), so a clean file sends no `textDocument/publishDiagnostics` notification and stale markers remain. The doc comment at `server.rs:912-914` already claims the opposite.

**Decision:** Insert `diags.entry(uri.clone()).or_default();` at the end of `collect_all` whenever `current_uri` is present, regardless of whether any diagnostics accumulated. Keep `publish_diagnostics` unchanged so its key-set iteration naturally emits an empty array. Update the stale comment at `server.rs:912-914` and add a test proving the empty publish.

**Consequences:** Clients always receive a clearing publish after the last error is fixed. The downside is one extra empty notification on first open of a perfectly clean file, which LSP clients ignore cheaply.

**Alternative considered:** Inject an empty publish only in `publish_diagnostics` if `current_uri` is missing from the map. Simpler but makes the publisher aware of a single special URI rather than handling the map uniformly.

### AD-2: Parse-error message formatting

**Context:** `to_diagnostic` emits `"Parse error: ... (MISSING node, ~N column(s))"` and leaks `ERROR`/`MISSING` internals (`diagnostics.rs:135-157`). Users cannot act on these messages.

**Decision:** Rewrite `to_diagnostic` using `tree_sitter::LookaheadNamesIterator` over `node.parent()` (or the grammar's expected-token set) to produce:

- `MISSING` → `"Missing '<token>'"`, e.g. `"Missing ';'"`, `"Missing '}'"`.
- `ERROR` → `"Unexpected <kind> '<text>'"`, e.g. `"Unexpected identifier 'foo'"`.

Add a defensive fallback `"Parse error near line {row}, column {col}"` that omits grammar internals when expected tokens cannot be determined.

**Consequences:** Diagnostics become actionable and stop exposing parser internals. Mapping grammar symbol names to display tokens adds a small localization layer, but the fallback prevents malformed output if a future grammar change shifts node shapes.

**Alternative considered:** Hard-code messages for common parent kinds (e.g. parent `statement` ⇒ missing `;`). Easier to ship but breaks whenever the grammar changes or the error pattern is uncommon.

### AD-3: Engine-symbol references via workspace scan

**Context:** The references handler short-circuits engine-API symbols and returns `vec![]` (`server.rs:702-707`), so Find Usages on `aiEcho`/`kbUnitCount` produces no results.

**Decision:** When an identifier resolves *exclusively* to the engine API (`find_syscall`/`find_aiplan`) and neither the merged view nor the current symbol table contains a workspace definition, walk every `.xs` file visible in the workspace using the same pattern as workspace-symbol search (`project.visible_files(&ws_locked)`). For each file, parse it and call `references::find_identifier_uses`. Deduplicate with the existing `seen` hash and honor `include_declaration` (no declaration exists, so the flag is naturally satisfied). Enforce a per-file scan budget (~50 ms) and a total budget of ≤500 ms for typical mod workspaces (≤10 files); warn and truncate if a budget is exceeded.

**Consequences:** Engine-API references return real use-site locations. The scan is bounded and reuses existing helpers, keeping the change localized. The tradeoff is slightly higher latency on first references request for huge game folders.

**Alternative considered:** Build an inverted index of identifier use sites and invalidate it on `did_change_watched_files`. Faster but adds caching complexity for a patch release.

### AD-4: Engine-symbol definition returns null

**Context:** `goto_definition` returns a non-navigable virtual URI `xs-stub://engine/<name>` for engine symbols (`server.rs:596-609`). Clients cannot open these URIs, so the feature appears broken.

**Decision:** In `server.rs::goto_definition`, after checking the merged view and the current symbol table, return `Ok(None)` if the identifier matches only engine API (`find_syscall`/`find_aiplan`). Workspace-defined symbols continue to return real `Location`s exactly as before.

Update `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` to assert `null` for engine-symbol definition instead of the stub URI (`definition_resp` checks around line 417).

**Consequences:** The IDE displays a clean "no definition" rather than a dead link. This changes existing behavior asserted by the roundtrip test, so that test must be edited in the same change.

**Alternative considered:** Bundle engine-API source files as a virtual file system (e.g. generated documentation stubs). Rejected as out of scope for a 0.1.7 patch and deferred to a future change.

## Data Flow

```text
textDocument/didChange
        │
        ▼
server::did_change(uri, text, version)
        │
        ▼
server::publish_diagnostics(uri, text, version)
        │
        ▼
diagnostics::collect_all(...)  ──► DiagnosticsByUri
   │ current_uri always inserted    │ empty Vec when clean
   │ (AD-1 fix)                    │
   ▼                                │
for (diag_uri, diags) in map ◄─────┘
   │
   ▼
client.publish_diagnostics(uri, diags, Some(version))
```

The AD-1 boundary is the guaranteed empty map entry in `collect_all`; `publish_diagnostics` simply forwards whatever the map contains.

## File Changes

| File | Action | Description |
|------|--------|-------------|
| `tools/xs-language-server/src/diagnostics.rs` | Modify | Insert empty URI entry; rewrite `to_diagnostic` and add parse-message tests. |
| `tools/xs-language-server/src/server.rs` | Modify | Update doc comment; rework references engine-API branch; return `Ok(None)` for engine-API definitions. |
| `tools/xs-language-server/src/references.rs` | Reuse | `find_identifier_uses` and `filter_declaration` called for each workspace file. |
| `tools/xs-language-server/tests/game_folder_parse.rs` | Modify if needed | Ensure parse-message changes do not raise new unexpected-error counts; add test if desired. |
| `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` | Modify | Assert null engine-definition and empty clean-file publish. |
| `tools/intellij-xs-plugin/gradle.properties` | Modify | Bump `pluginVersion` 0.1.6 → 0.1.7. |

## Interfaces / Contracts

`DiagnosticsByUri` and diagnostic categories from `diagnostics.rs` are unchanged. No new public types are introduced. The expected behaviors are:

- `collect_all` returns a map containing `current_uri` mapped to a (possibly empty) `Vec<Diagnostic>`.
- `to_diagnostic` never produces a message containing the substrings `MISSING`, `ERROR`, `node`, or `column(s)`; it falls back to line/column only when lookahead data is unavailable.
- Engine-symbol `references` returns only workspace `Location`s; empty only if the identifier is unused.
- Engine-symbol `definition` returns `null`.

## Testing Strategy

| Layer | What to Test | Approach |
|-------|--------------|----------|
| Unit | Clean-file map entry | `diagnostics::collect_all` unit test asserts `diags.contains_key(uri)` with no items. |
| Unit | Parse message shapes | Unit tests for missing `;`, missing `}`, and unexpected identifier; assert blacklist words absent. |
| Integration | Empty publish on `didChange` | Extend `lsp_roundtrip_test.rs` to send a clean follow-up change and capture the empty publish. |
| Integration | Engine references | Fixture with `aiEcho` in two files; assert `textDocument/references` returns ≥2 locations. |
| Integration | Engine definition null | Update `lsp_roundtrip_test.rs` assertion for `aiEcho` definition from stub URI to `null`. |
| Regression | Game folder zero diagnostics | Run `game_folder_parse.rs` with `AOMR_GAME_PATH` to confirm new messages do not break thresholds. |

## Migration / Rollout

No migration required. Reverting the Rust commit(s) restores the previous `cargo test` baseline and prior plugin behavior.

## Risks & Open Questions

- **Known regression-test rewrite:** `lsp_roundtrip_test.rs` currently asserts the `xs-stub://engine/aiEcho` definition response. That assertion MUST be changed to expect `null` during apply.
- **Fallback path for AD-2:** The spec treats the fallback as an edge case. A unit test should force a node whose expected tokens cannot be resolved to verify the fallback message.
- **AD-3 performance on full game folder:** The per-file budget protects against stalls, but references on symbols used in hundreds of files will truncate. This is acceptable for a 0.1.7 patch; full game-folder references can be optimized later.
- **Capability comments:** The outdated references/definition capability comments in `server.rs:239-250` can be updated to reflect real behavior.

## Implementation Order

Strict TDD requires a failing test before each fix. Recommended order:

1. **AD-4 — Engine definition null:** Update the roundtrip test first, watch it fail on the stub URI, then change `goto_definition`. This is the simplest behavioral toggle and immediately unblocks the most visible "broken" feature.
2. **AD-1 — Empty clean-file publish:** Add a unit test or extend the roundtrip test to assert an empty publish, then insert the map entry. Doing this before parse-message changes keeps diagnostics tests stable.
3. **AD-2 — Actionable parse errors:** Add failing unit tests for `to_diagnostic`, then implement the lookahead mapping and fallback. Run after AD-1 so a clean file clearing also removes old parse messages correctly.
4. **AD-3 — Engine references:** Add a references fixture test last. It is the most invasive (workspace file scan) and benefits from the prior fixes being green.
5. **Version bump** and final full `cargo test` + `cargo run --bin lsp_roundtrip_test`.
