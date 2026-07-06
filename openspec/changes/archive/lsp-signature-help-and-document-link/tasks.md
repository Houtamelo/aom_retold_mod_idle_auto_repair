# Tasks: LSP Signature Help and Document Link

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~400-500 |
| 400-line budget risk | Medium |
| Chained PRs recommended | Yes |
| Suggested split | PR-1 signatureHelp → PR-2 documentLink → PR-3 roundtrip/capabilities → PR-4 plugin bump |
| Delivery strategy | auto-chain |
| Chain strategy | stacked-to-main |

Decision needed before apply: No
Chained PRs recommended: Yes
Chain strategy: stacked-to-main
400-line budget risk: Medium

## Phase 1 — signatureHelp (PR-1)

Commit as one PR at phase end.

- [x] 1.1 (RED) Write `lsp/tests/signature_help_repro.rs` failing: engine `kbUnitCreate(...)` signature with `activeParameter=0`; workspace `foo(...)` signature. Confirm compile/test failure.
- [x] 1.2 (GREEN) Create `lsp/src/signature_help.rs` with hardcoded engine lookup; wire `Backend::signature_help` in `server.rs`; declare in `lib.rs`.
- [x] 1.3 (GREEN) Add merged-view/symbol-table fallback for workspace callables. Both 1.1 tests pass.
- [x] 1.4 (TRIANGULATE) Add nested-call activeParam test (`outer(inner(a, b), c)`).
- [x] 1.5 (REFACTOR) Extract `find_call_at_cursor`, `build_signature_information`, `compute_active_parameter`; handler ~50 LOC.
- [x] 1.6 (CHORE) `cargo test --manifest-path tools/xs-language-server/Cargo.toml` green.

## Phase 2 — documentLink (PR-2)

Commit as one PR at phase end.

- [x] 2.1 (RED) Write `lsp/tests/document_link_repro.rs` failing: resolvable include yields one link; missing target yields empty.
- [x] 2.2 (GREEN) Create `lsp/src/document_link.rs` scanning includes via parser, resolving via `workspace.resolve_include_for_file`; wire `Backend::document_link` in `server.rs`; declare in `lib.rs`.
- [x] 2.3 (TRIANGULATE) Add three-includes test with one missing target → exactly two links.
- [x] 2.4 (REFACTOR) Extract `walk_include_directives`, `range_for_path_token`, `resolve_to_uri`.
- [x] 2.5 (CHORE) Workspace test suite green.

## Phase 3 — capabilities + roundtrip (PR-3)

- [x] 3.1 In `server.rs::server_capabilities`, add `signatureHelpProvider` with trigger chars `(`/`,` and `documentLinkProvider` with `resolve_provider=false`.
- [x] 3.2 Add known-good-delta note to `lsp/tests/game_folder_parse.rs`.
- [x] 3.3 Add typed JSON `signatureHelp` request in `lsp/src/bin/lsp_roundtrip_test.rs` asserting non-empty signatures with expected label and `activeParameter`.
- [x] 3.4 Add typed JSON `documentLink` request in roundtrip test asserting ≥1 link with `target` file URI.
- [x] 3.5 Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test` with `AOMR_GAME_PATH` set; green.
- [x] 3.6 Run full workspace suite; 369+ tests green.

## Phase 4 — plugin version bump (PR-4)

- [x] 4.1 In `tools/intellij-xs-plugin/gradle.properties`, change `pluginVersion` from `0.10.0` to `0.11.0`.
- [x] 4.2 Verify value has no trailing whitespace.
- [x] 4.3 Commit: `chore(xs-plugin): bump pluginVersion to 0.11.0`.

## Phase 5 — final verification

- [x] 5.1 `cargo test --manifest-path tools/xs-language-server/Cargo.toml` green.
- [x] 5.2 `./gradlew :test` in `tools/intellij-xs-plugin/` green.
- [x] 5.3 `git diff` review vs design file plan; verify `pluginVersion=0.11.0`.
- [x] 5.4 Review commit messages are Conventional Commits with scopes `feat(xs-lsp):` / `chore(xs-plugin):`.

## Phase 6 — fix cycle (post-verify)

- [x] 6.1 Fix roundtrip harness race in `lsp_roundtrip_test.rs`: wait for `initialize` response before sending `didOpen`/feature request.
- [x] 6.2 Add missing test for signatureHelp default-parameter exposure (`aiSetHandler`).
- [x] 6.3 Add missing test for active-parameter clamping past last index.
- [x] 6.4 Add missing test for three resolvable document links.
- [x] 6.5 Run full workspace suite; 387/0/0 green.
- [x] 6.6 Re-run live `lsp_roundtrip_test`; both probes print `PASS`.

## Out of scope (NOT to be done here)

signatureHelpResolve, inlayHint/inlayHintResolve, codeAction/codeActionResolve, didChangeConfiguration, pull-mode diagnostics + capability removal, formatting/rangeFormatting/onTypeFormatting, documentHighlight, callHierarchy, typeHierarchy, moniker, linkedEditingRange, documentColor, inlineValue, file operations.
