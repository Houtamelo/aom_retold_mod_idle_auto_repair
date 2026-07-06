# LSP UX improvements — feature gap audit (exploration)

**Scope:** Identify LSP contracts exposed by `tower-lsp-server 0.23.0` / `ls-types 0.0.6` that our XS language server does NOT currently implement, and that would provide a meaningful UX improvement for users editing `.xs` files in IntelliJ.

**Status:** exploration only. No code changes proposed. This artifact exists to record findings so we can plan a future SDD change without re-doing the research.

**Predecessor:** the "fix retail diagnostics regression" SDD change (PR-K through PR-O) stabilized the LSP at 0 ERROR / 335 passing tests. The diagnostics work is complete; this exploration covers the *next* layer of UX improvements.

---

## Section 1 — versions & architecture

### Versions

- `tower-lsp-server = "0.23.0"` (`tools/xs-language-server/lsp/Cargo.toml:16`)
- `ls-types = "0.0.6"` (transitive dep, recorded in `Cargo.lock`)

### Architecture in one paragraph

`tower-lsp-server 0.23.0` is a thin router over `ls-types 0.0.6`. Three pieces:

1. **The `LanguageServer` trait** in `tower_lsp_server` declares one async method per LSP request/notification — 67 methods total. Every LSP spec method corresponds to a trait method. The trait bodies are either (a) empty `;` (required implementation) or (b) a default block that returns `Error::method_not_found()` for requests / silently drops for notifications.

2. **A declarative macro** `rpc!(pub trait LanguageServer: ...)` generates both the trait and a sibling module `tower_lsp_server::generated` that exports `register_lsp_methods()`. That function registers every trait method (plus the two fixed handlers `exit` and `$/cancelRequest`) onto a JSON-RPC router. Dispatch is by method-name string; **no capability gating** — if the client sends the request, the trait method is called.

3. **The `Client` handle** provides server-to-client primitives: `publish_diagnostics`, `register_capability`, `apply_edit`, `code_lens_refresh`, `semantic_tokens_refresh`, `inlay_hint_refresh`, `workspace_diagnostic_refresh`, `create_work_done_progress`, `progress()` builder, `configuration()`, `workspace_folders()`, etc. (all from `tower-lsp-0.23.0/src/service/client.rs`).

### Required vs. default

`initialize` and `shutdown` are the only two methods with no default body. Every other method has a default no-op. Implementors can override any subset.

### Capability flags

`ServerCapabilities` returned from `initialize` is **client-facing only** per the LSP spec — it tells the client "you may send these requests." The framework dispatches any registered method if the client sends it regardless of capability advertisement. Examples:

- `signature_help_provider` → unlocks `textDocument/signatureHelp`
- `code_action_provider` → unlocks `textDocument/codeAction`
- `inlay_hint_provider` → unlocks `textDocument/inlayHint`
- `document_formatting_provider` → unlocks `textDocument/formatting`

---

## Section 2 — implementation status (67 methods)

Status key: **[IMPL]** = we override it; **[DEFAULT]** = no-op default; **[N/A]** = framework-internal.

| #  | Trait method                       | LSP method                              | Status                                                       |
|----|------------------------------------|-----------------------------------------|--------------------------------------------------------------|
| 1  | `initialize`                        | `initialize`                              | **[IMPL]**                                                   |
| 2  | `initialized`                       | `initialized`                             | **[IMPL]**                                                   |
| 3  | `shutdown`                          | `shutdown`                                | **[IMPL]**                                                   |
| —  | (auto)                            | `exit`                                    | **[N/A]** registered by framework                            |
| 4  | `did_open`                          | `textDocument/didOpen`                    | **[IMPL]**                                                   |
| 5  | `did_change`                        | `textDocument/didChange`                  | **[IMPL]**                                                   |
| 6  | `did_save`                          | `textDocument/didSave`                    | **[DEFAULT]** — current no-op                                |
| 7  | `did_close`                         | `textDocument/didClose`                   | **[IMPL]**                                                   |
| 8  | `will_save`                         | `textDocument/willSave`                   | DEFAULT                                                      |
| 9  | `will_save_wait_until`              | `textDocument/willSaveWaitUntil`          | DEFAULT                                                      |
| 10 | `notebook_did_open`                 | `notebookDocument/didOpen`                | DEFAULT (irrelevant)                                          |
| 11 | `notebook_did_change`               | `notebookDocument/didChange`              | DEFAULT (irrelevant)                                          |
| 12 | `notebook_did_save`                 | `notebookDocument/didSave`                | DEFAULT (irrelevant)                                          |
| 13 | `notebook_did_close`                | `notebookDocument/didClose`               | DEFAULT (irrelevant)                                          |
| 14 | `goto_definition`                   | `textDocument/definition`                 | **[IMPL]**                                                   |
| 15 | `goto_declaration`                  | `textDocument/declaration`                | DEFAULT                                                       |
| 16 | `goto_type_definition`              | `textDocument/typeDefinition`             | DEFAULT                                                       |
| 17 | `goto_implementation`               | `textDocument/implementation`             | DEFAULT                                                       |
| 18 | `references`                        | `textDocument/references`                 | **[IMPL]**                                                   |
| 19 | `prepare_call_hierarchy`            | `textDocument/prepareCallHierarchy`       | DEFAULT                                                       |
| 20 | `incoming_calls`                    | `callHierarchy/incomingCalls`             | DEFAULT                                                       |
| 21 | `outgoing_calls`                    | `callHierarchy/outgoingCalls`             | DEFAULT                                                       |
| 22 | `prepare_type_hierarchy`            | `textDocument/prepareTypeHierarchy`       | DEFAULT                                                       |
| 23 | `supertypes`                        | `typeHierarchy/supertypes`                | DEFAULT                                                       |
| 24 | `subtypes`                          | `typeHierarchy/subtypes`                  | DEFAULT                                                       |
| 25 | `document_highlight`                | `textDocument/documentHighlight`          | DEFAULT                                                       |
| 26 | `document_link`                     | `textDocument/documentLink`               | DEFAULT                                                       |
| 27 | `document_link_resolve`             | `documentLink/resolve`                    | DEFAULT                                                       |
| 28 | `hover`                             | `textDocument/hover`                      | **[IMPL]**                                                   |
| 29 | `code_lens`                         | `textDocument/codeLens`                   | DEFAULT                                                       |
| 30 | `code_lens_resolve`                 | `codeLens/resolve`                        | DEFAULT                                                       |
| 31 | `folding_range`                     | `textDocument/foldingRange`               | DEFAULT                                                       |
| 32 | `selection_range`                   | `textDocument/selectionRange`             | DEFAULT                                                       |
| 33 | `document_symbol`                   | `textDocument/documentSymbol`             | **[IMPL]**                                                   |
| 34 | `semantic_tokens_full`              | `textDocument/semanticTokens/full`        | **[IMPL]**                                                   |
| 35 | `semantic_tokens_full_delta`        | `textDocument/semanticTokens/full/delta`  | DEFAULT                                                       |
| 36 | `semantic_tokens_range`             | `textDocument/semanticTokens/range`       | DEFAULT                                                       |
| 37 | `inline_value`                      | `textDocument/inlineValue`                | DEFAULT (irrelevant — debug-API)                               |
| 38 | `inlay_hint`                        | `textDocument/inlayHint`                  | DEFAULT                                                       |
| 39 | `inlay_hint_resolve`                | `inlayHint/resolve`                       | DEFAULT                                                       |
| 40 | `moniker`                           | `textDocument/moniker`                    | DEFAULT                                                       |
| 41 | `completion`                        | `textDocument/completion`                 | **[IMPL]** (review needed)                                    |
| 42 | `completion_resolve`                | `completionItem/resolve`                  | DEFAULT                                                       |
| 43 | `signature_help`                    | `textDocument/signatureHelp`              | DEFAULT                                                       |
| 44 | `code_action`                       | `textDocument/codeAction`                 | DEFAULT                                                       |
| 45 | `code_action_resolve`               | `codeAction/resolve`                      | DEFAULT                                                       |
| 46 | `document_color`                    | `textDocument/documentColor`              | DEFAULT (irrelevant — no colors)                               |
| 47 | `color_presentation`                | `textDocument/colorPresentation`          | DEFAULT (irrelevant)                                          |
| 48 | `formatting`                        | `textDocument/formatting`                 | DEFAULT                                                       |
| 49 | `range_formatting`                  | `textDocument/rangeFormatting`            | DEFAULT                                                       |
| 50 | `on_type_formatting`                | `textDocument/onTypeFormatting`           | DEFAULT                                                       |
| 51 | `rename`                            | `textDocument/rename`                     | **[IMPL]**                                                   |
| 52 | `prepare_rename`                    | `textDocument/prepareRename`              | **[IMPL]**                                                   |
| 53 | `linked_editing_range`              | `textDocument/linkedEditingRange`         | DEFAULT (irrelevant)                                          |
| 54 | `diagnostic`                        | `textDocument/diagnostic`                 | **DEFAULT** but **advertised** in `ServerCapabilities`       |
| 55 | `workspace_diagnostic`              | `workspace/diagnostic`                    | **DEFAULT** but **advertised** in `ServerCapabilities`       |
| 56 | `symbol`                            | `workspace/symbol`                        | **[IMPL]**                                                   |
| 57 | `symbol_resolve`                    | `workspaceSymbol/resolve`                 | DEFAULT                                                       |
| 58 | `did_change_configuration`          | `workspace/didChangeConfiguration`        | DEFAULT                                                       |
| 59 | `did_change_workspace_folders`      | `workspace/didChangeWorkspaceFolders`     | **[IMPL]**                                                   |
| 60 | `did_change_watched_files`          | `workspace/didChangeWatchedFiles`         | **[IMPL]**                                                   |
| 61 | `execute_command`                   | `workspace/executeCommand`                | DEFAULT                                                       |
| 62 | `will_create_files`                 | `workspace/willCreateFiles`               | DEFAULT                                                       |
| 63 | `did_create_files`                  | `workspace/didCreateFiles`                | DEFAULT                                                       |
| 64 | `will_rename_files`                 | `workspace/willRenameFiles`               | DEFAULT                                                       |
| 65 | `did_rename_files`                  | `workspace/didRenameFiles`                | DEFAULT                                                       |
| 66 | `will_delete_files`                 | `workspace/willDeleteFiles`               | DEFAULT                                                       |
| 67 | `did_delete_files`                  | `workspace/didDeleteFiles`                | DEFAULT                                                       |

**Tally:** 14 implemented, 53 default, 1 N/A.

> **Correction from prior assumption:** `did_save` and `semantic_tokens_full_delta` are currently default no-ops. They are NOT implemented (a small misconception in earlier notes).

> **One correctness footgun:** `server.rs:319-324` returns `DiagnosticServerCapabilities::Options(DiagnosticOptions { workspace_diagnostics: false, ... })` but the trait methods `diagnostic` and `workspace_diagnostic` are not overridden. Per LSP 3.17+, any pull-mode-aware client (newer IntelliJ LSP4IJ) will issue `textDocument/diagnostic` requests that always return `method_not_found`. Two cheap options: implement the handlers, or drop the capability advertisement.

---

## Section 3 — Tier A: clear wins, small/medium cost

| Method                                          | Cost       | Why                                                                                                                                                                                                                                                                                                                                                                                                                                              |
|-------------------------------------------------|------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **`textDocument/signatureHelp`**                 | ~150 lines | XS is dense with engine calls (`kbUnitCreate`, `trPlayerScore`, `aiPlanSet*`). Show the active overload + highlighted parameter as the user types `(`. Trivial wire-up against `engine.syscalls` (already used by hover).                                                                                                                                                                                                                              |
| **`textDocument/codeAction` — "Add forward declaration"** | medium     | AGENTS.md calls this out as the most common regression across SDD apply cycles (8 forward-decl bugs in one PR). A code action that reads the parsed AST and inserts a matching signature block at the top of the file would eliminate the entire failure mode. Ship this first; rename-symbol and extract-rule become optional follow-ups.                                                                                                                |
| **`textDocument/completion` (review + completion_resolve)** | small      | The current `complete` impl is thin. We have all the data: 172-syscall engine API + AI plan constants + merged-scope rule names + local symbol table. Completion alone is 1000+ items and probably the single biggest UX gap users see today.                                                                                                                                                                                                          |

## Section 4 — Tier B: implement when concrete use case lands

| Method                                          | Verdict     | Why                                                                                                                                                                                                                                                                                                                                                                                                  |
|-------------------------------------------------|-------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **`textDocument/documentLink`**                 | implement   | Make `include "..."` paths clickable. Reuses the existing `workspace.resolve_include_for_file` (used by goto-definition). Ctrl-click an include jumps to the included `.xs`. ~80 lines.                                                                                                                                                                                                                    |
| **`textDocument/documentHighlight`**            | implement   | "Highlight all occurrences of this identifier." Reuses `references::find_identifier_uses`. ~50 lines.                                                                                                                                                                                                                                                                                                       |
| **`textDocument/didSave`**                      | implement   | Currently a default no-op. The on-disk file now matches the buffer, so re-publish diagnostics with the bumped version. ~20 lines.                                                                                                                                                                                                                                                                            |
| **`textDocument/inlayHint`**                    | future      | Inline parameter-name hints at call sites (`kbUnitCreate(: int player, : int unitType, ...)`). The `InlayHint` / `InlayHintParams` / `InlayHintKind` types are all in `ls-types-0.0.6/src/inlay_hint.rs`. Ship when someone asks "what's the type of `v` here?"                                                                                                                                                  |
| **`workspace/didChangeConfiguration`**          | implement   | Currently config is read once from `AOMR_GAME_PATH` at startup (`server.rs:129`). A real settings UI (the IntelliJ plugin's XS settings page) would push new game-folder paths via this. Without it, users must restart the IDE to pick up new config. Easy wire-up: implement the handler, refresh workspace, invalidate merged-view caches.                                                                              |

## Section 5 — Tier C: large effort, ship later

| Method                                          | Verdict     | Why                                                                                                                                                                                                                                                                                                                                                                                                  |
|-------------------------------------------------|-------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **`textDocument/formatting` / `rangeFormatting`** | future      | Auto-format XS. The typed AST already supports it; we just need a pretty-printer. Risk: `onTypeFormatting` at `;` and `{` keys would race with the user's brace-matcher. Launch with full-document `formatting` only; skip on-type.                                                                                                                                                                              |
| **`textDocument/diagnostic` (pull mode)**       | future      | LSP 3.17+ pull diagnostics. Our publish-push is fine in practice, but pull mode fixes the version-staleness contract that push mode struggles with. Cost is small (data is in `publish_diagnostics` already) but requires deciding the `Unchanged` vs `Full` reporting contract.                                                                                                                              |
| **`workspace/executeCommand`**                  | future      | "Restart LSP" / "Regenerate engine cache" / "Force redeploy mod" menu items in the IntelliJ plugin. The client-side wrappers exist; we just need an impl. Ship when the plugin grows beyond LSP glue.                                                                                                                                                                                                       |
| **`textDocument/inlayHint/refresh` / `codeLens/refresh` / `diagnostic/refresh` / `semanticTokens/refresh`** | future      | Server-to-client requests; require the corresponding inbound methods to exist. Wire them up when implementing Tier B inlay hints. `ls-types-0.0.6/src/request.rs:166-195` declares all four `*Refresh` request types; `tower-lsp-0.23.0/src/service/client.rs:269-389` provides the `Client` wrappers for free.                                                                                                |
| **`textDocument/semanticTokens/full/delta`**    | future      | Full-scan recompute on every keystroke is wasteful for large files. Delta mode halves token traffic. Only meaningful when files exceed ~5k tokens. Defer until performance complaints arrive.                                                                                                                                                                                                          |

## Section 6 — Tier D: skip / not applicable

| Method                                          | Verdict           | Why                                                                                                                                                                                                                                                                                    |
|-------------------------------------------------|-------------------|------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `goto_declaration` / `goto_type_definition` / `goto_implementation` | skip              | XS has no class/method declaration-vs-definition distinction (no Java-style typed class hierarchy), no `interface`/`implements`, no typed generics. `goto_declaration` is rarely understood even by editors.                                                                                                                                          |
| `prepare_call_hierarchy` / `incoming_calls` / `outgoing_calls` | skip              | Call hierarchy is for OO languages with method-resolution. XS rules can call each other but the graph is shallow and users can read it directly.                                                                                                                                            |
| `prepare_type_hierarchy` / `supertypes` / `subtypes` | skip              | XS classes are data records, not a type hierarchy. No inheritance.                                                                                                                                                                                                                       |
| `moniker`                                       | skip              | Symbol monikers are for LSIF cross-repo linking. We don't produce LSIF.                                                                                                                                                                                                                  |
| `linked_editing_range`                          | skip              | HTML/JSX tag-pair editor. XS has no tagged-pair constructs.                                                                                                                                                                                                                              |
| `folding_range` / `selection_range`             | skip / optional   | IntelliJ does its own folding. Marginal value.                                                                                                                                                                                                                                            |
| `document_color` / `color_presentation`         | skip              | No color literals.                                                                                                                                                                                                                                                                        |
| `inline_value`                                  | skip              | Debug-API feature for JS-like debuggers.                                                                                                                                                                                                                                                  |
| All `*Notebook*` methods, all `*_files` workspace methods | skip              | Wrong model (XS isn't a notebook; file creation/deletion isn't a workflow).                                                                                                                                                                                                              |

---

## Section 7 — underused `Client` primitives (free, available now)

`Client` provides these server-to-client primitives we don't currently use but could:

- `Client::create_work_done_progress(token)` + `Client::progress(token, title).with_message(...).with_pct(...).begin()` — streaming `$/progress` updates. Use before cache rebuild / symbol-table work that takes >500ms.
- `Client::inlay_hint_refresh()` — invalidate cached inlay hints after a non-buffer event (e.g. include re-paste).
- `Client::semantic_tokens_refresh()` — invalidate cached tokens after `did_change_watched_files` modifies a file on disk.
- `Client::apply_edit(WorkspaceEdit)` — push multi-file edits (for code actions that span files).
- `Client::configuration(items)` — pull from the IntelliJ settings page on demand.

---

## Section 8 — top 3 recommendations

In order of ROI:

1. **`textDocument/signatureHelp`** — single-feature-ROI. Trivial implementation against `engine.syscalls` (we already use it for hover). Every IntelliJ user expects it. Cost: ~150 lines including trigger-character wiring.

2. **`textDocument/codeAction` — "Add forward declaration"** — the only code action worth shipping first because it eliminates an entire bug class (AGENTS.md's #1 SDD-apply regression category). Cost: medium. After that, rename-symbol, extract-rule, include-relative-path become optional.

3. **`workspace/didChangeConfiguration` + remove-or-implement the advertised `diagnostic_provider` capability** — both are correctness gaps, not feature gaps. `didChangeConfiguration` lets the plugin's settings page push a new `AOMR_GAME_PATH` without an IDE restart. The advertised-but-unimplemented `diagnostic_provider` (server.rs:319) is a footgun: pull-mode-aware clients issue `textDocument/diagnostic` that always `method_not_found`.

---

## Section 9 — open decisions

1. **Do we ship a release for each tier or one big "LSP UX" PR?**
   - Signature help is self-contained and low-risk; cleanest first PR.
   - Code action (forward decl) touches the diagnostic pipeline; benefits from being its own PR.
   - The `diagnostic_provider` fix is a 1-line capability removal OR a few-line handler — whichever the team prefers.

2. **Do we add `inlay_hint` at the same time as `signature_help`?**
   - Both reuse `engine.syscalls`. Bundling them saves a round-trip.
   - Counter-argument: inlay hints can be visually noisy; ship signature_help first, get feedback, then inlay_hint.

3. **Pull-mode diagnostics: implement or remove the capability?**
   - Removing is safer (no risk of stale `Unchanged` reports) but limits future pull-mode work.
   - Implementing is the more correct path but adds version-tracking complexity.

4. **Formatting strategy.**
   - If we ship `formatting`, the question becomes: do we own the entire format (indent, brace style) or just do whitespace normalization?

---

## Next step for the orchestrator

When we pick this up, the recommended sequencing is:

- **Change 1** — `signatureHelp` + `documentLink` (parallel features, no shared code paths)
- **Change 2** — `codeAction` with one action: "Add forward declaration"
- **Change 3** — correctness fixes: `didChangeConfiguration` + `diagnostic_provider` advertisement
- **Change 4** — `inlayHint` (if the team wants it)
- **Change 5** — `formatting` (if the team wants it)

Each is a clean, self-contained PR. None of them conflict with the diagnostics work that just landed.
