# Verify Report: lsp-signature-help-and-document-link

## Re-verification (post-fix-cycle)

**Date**: 2026-07-06
**Triggered by**: previous verify returned `status: blocked` with Gate 4 (roundtrip race) and Gate 2 (3 untested scenarios) failures.
**Fix-cycle commits**: `3e24758` (tests + race fix), `ca5cf24` (apply-progress update).
**Overall verdict this round**: PASS-WITH-DEVIATIONS

## Gate Results (this round)

| Gate | Description | Previous | This Round |
|------|-------------|----------|------------|
| 1 | Workspace test suite | PASS (381/0/0) | PASS — 387 passed / 0 failed / 0 ignored (+6 new tests, 0 regressions) |
| 2 | Spec compliance matrix | PARTIAL (15/18 COMPLIANT, 3 UNTESTED) | PASS — 18 / 18 scenarios COMPLIANT (0 FAILING, 0 UNTESTED) |
| 3 | Capability advertisement static check | PASS | PASS — `signatureHelpProvider` and `documentLinkProvider` present with correct options; all pre-existing flags preserved |
| 4 | Roundtrip harness (live, with AOMR_GAME_PATH) | FAIL (Server not initialized) | PASS — both probes print `PASS`; all baseline/include/semantic/engine probes still pass; exit code 0 |
| 5 | Plugin build smoke | PASS | PASS — `./gradlew :test` BUILD SUCCESSFUL |
| 6 | Deviation audit | PASS-WITH-DEVIATIONS | PASS-WITH-DEVIATIONS — existing deviations remain acceptable; one new self-referential artifact-hash deviation added (see below) |
| 7 | No XS mod / Kotlin main touched | PASS | PASS — `git diff --stat master~7..master -- 'mod/' 'tools/intellij-xs-plugin/src/main/kotlin/'` returned empty |

## Compliance Matrix

### `lsp-signature-help` (5 reqs / 9 scenarios)

| Spec | Requirement | Scenario | Test name | Verdict |
|------|-------------|----------|-----------|---------|
| lsp-signature-help | REQ-SIG-01 | Engine syscall with multiple parameters | `engine_syscall_signature_at_open_paren` | COMPLIANT |
| lsp-signature-help | REQ-SIG-01 | Engine syscall with defaults | `engine_syscall_signature_exposes_default_value`, `engine_syscall_signature_exposes_bool_default` | COMPLIANT |
| lsp-signature-help | REQ-SIG-02 | Call to undefined function | `unknown_function_returns_none` | COMPLIANT |
| lsp-signature-help | REQ-SIG-03 | User-defined function | `workspace_function_signature_in_current_file`, `included_workspace_function_signature_via_merged_view` | COMPLIANT |
| lsp-signature-help | REQ-SIG-03 | Registered rule | `rule_signature_has_no_parameters` | COMPLIANT |
| lsp-signature-help | REQ-SIG-04 | Open parenthesis | `engine_syscall_signature_at_open_paren` | COMPLIANT |
| lsp-signature-help | REQ-SIG-04 | Comma inside argument list | `engine_syscall_active_parameter_advances_after_comma` | COMPLIANT |
| lsp-signature-help | REQ-SIG-05 | Second argument | `engine_syscall_active_parameter_advances_after_comma`, `nested_call_reports_inner_signature` | COMPLIANT |
| lsp-signature-help | REQ-SIG-05 | Cursor beyond parameter count | `active_parameter_clamped_to_last_index`, `active_parameter_clamped_for_single_param_function` | COMPLIANT |

### `lsp-document-link` (5 reqs / 6 scenarios)

| Spec | Requirement | Scenario | Test name | Verdict |
|------|-------------|----------|-----------|---------|
| lsp-document-link | REQ-DLK-01 | Single include | `single_resolvable_include_yields_one_link` | COMPLIANT |
| lsp-document-link | REQ-DLK-02 | Missing target | `unresolved_include_returns_empty` | COMPLIANT |
| lsp-document-link | REQ-DLK-03 | Quoted path range | `single_resolvable_include_yields_one_link` | COMPLIANT |
| lsp-document-link | REQ-DLK-04 | Three includes | `three_resolvable_includes_yield_three_links` | COMPLIANT |
| lsp-document-link | REQ-DLK-04 | File with no includes | `file_with_no_includes_returns_empty` | COMPLIANT |
| lsp-document-link | REQ-DLK-05 | Mod overlay copy | `mod_overlay_is_preferred_over_vanilla` | COMPLIANT |

### `lsp-server-capabilities` (3 reqs / 3 scenarios)

| Spec | Requirement | Scenario | Test name | Verdict |
|------|-------------|----------|-----------|---------|
| lsp-server-capabilities | REQ-CAP-01 | Initialize response | static check + roundtrip capabilities JSON | COMPLIANT (static) |
| lsp-server-capabilities | REQ-CAP-02 | Initialize response | static check + roundtrip capabilities JSON | COMPLIANT (static) |
| lsp-server-capabilities | REQ-CAP-03 | Capability preservation | `git diff master~7..master -- tools/xs-language-server/lsp/src/server.rs` | COMPLIANT (static) |

**Totals:** 13 reqs / 18 scenarios / 18 COMPLIANT / 0 FAILING / 0 UNTESTED.

## Deviations Carried From apply-progress.md

| Deviation | Verdict | Rationale |
|-----------|---------|-----------|
| Capability flags advertised in PR-1 and PR-2 instead of the planned PR-3 slot | COMPLIANT-WITH-DEVIATION | Net `ServerCapabilities` matches design §3; PR boundaries only are fuzzier. |
| `tests/game_folder_parse.rs` per-change "known-good-delta" note treated as N/A | COMPLIANT-WITH-DEVIATION | No per-change convention exists in that file; existing threshold assertions already cover the change because new handlers do not emit diagnostics. Workspace count held at 387/0/0. |
| `./gradlew :test` deliberately skipped during apply | moot | Re-run in Gate 5 passed. |
| First apply delegation returned empty; orchestrator completed inline | historical | Not a compliance issue; documented for traceability. |
| `self_referential_artifact_hash`: `apply-progress.md` left a placeholder hash (`<this artifact commit>`) instead of a real commit hash | COMPLIANT-WITH-DEVIATION | The actual artifact commit is `ca5cf24`. The placeholder was replaced in the working file but the literal text inside the "Commits added by this fix cycle" list still contains the placeholder string. It is a meta-recording artifact only; source/test truth is unaffected. |

## Live Roundtrip Output (this round)

Command:

```bash
AOMR_GAME_PATH="/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold" \
  cargo build --manifest-path tools/xs-language-server/Cargo.toml --bin lsp_roundtrip_test && \
AOMR_GAME_PATH="/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold" \
  /home/houtamelo/Documents/projects/aom_retold_mod/tools/xs-language-server/target/debug/lsp_roundtrip_test 2>&1 | tail -60
```

Relevant output:

```text
PASS (include completion_across_include): response matched expected content
PASS (include hover_across_include): response matched expected content
PASS (include definition_across_include): response matched expected content
PASS (cycle_does_not_hang): server exited cleanly despite include cycle
[test] received msg: {"id":1301,"jsonrpc":"2.0","result":{"capabilities":{"completionProvider":{"resolveProvider":false,"triggerCharacters":[".","_"]},"definitionProvider":true,"diagnosticProvider":{"identifier":"xs-language-server","interFileDependencies":true,"workspaceDiagnostics":false},"documentLinkProvider":{"resolveProvider":false},"documentSymbolProvider":true,"hoverProvider":true,"referencesProvider":true,"renameProvider":{"prepareProvider":true},"semanticTokensProvider":{"full":true,"legend":{"tokenModifiers":["engine","modded","unmodded","local","static","extern","member"],"tokenTypes":["function","variable","type","constant","rule"]},"range":false},"signatureHelpProvider":{"triggerCharacters":["(",","]},"textDocumentSync":1,"workspace":{"workspaceFolders":{"changeNotifications":true,"supported":true}},"workspaceSymbolProvider":true}}}
[test] received msg: {"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"diagnostics":[],"uri":"file:///tmp/aomr_inc_mod_signature_help_probe_68791/game/ai/main.xs","version":1}}
[test] received msg: {"id":1300,"jsonrpc":"2.0","result":{"activeParameter":0,"activeSignature":0,"signatures":[{"documentation":{"kind":"markdown","value":"Adds this text to the AI Debug Output window in the \"All\" category."},"label":"void aiEcho(string text = \"Warning: Provide message.\")","parameters":[{"label":"string text = \"Warning: Provide message.\""}]}]}}
PASS (signature_help_probe): signatures=1 label="void aiEcho(string text = "Warning: Provide message.")" activeParameter=0 | label_ok=true active_ok=true sigs_ok=true
[test] received msg: {"id":1311,"jsonrpc":"2.0","result":{"capabilities":{...}}}
[test] received msg: {"jsonrpc":"2.0","method":"textDocument/publishDiagnostics","params":{"diagnostics":[],"uri":"file:///tmp/aomr_inc_mod_document_link_probe_68791/game/ai/main.xs","version":1}}
[test] received msg: {"id":1310,"jsonrpc":"2.0","result":[{"range":{"end":{"character":16,"line":0},"start":{"character":9,"line":0}},"target":"file:///tmp/aomr_inc_mod_document_link_probe_68791/game/ai/util.xs"}]}
PASS (document_link_probe): links=1 first_target="file:///tmp/aomr_inc_mod_document_link_probe_68791/game/ai/util.xs" target_matches_util=true
PASS (engine references): responded 2 aiEcho location(s) in 0 ms (request to response)
```

Process exited with code 0.

## Final Verdict

**OVERALL: PASS-WITH-DEVIATIONS**

All seven verification gates pass. The implementation is functionally correct, fully covered by passing tests, correctly advertises both new capabilities, and the live `lsp_roundtrip_test` exercises `textDocument/signatureHelp` and `textDocument/documentLink` end-to-end. The only remaining issues are documentation/meta deviations listed in the deviation audit; none affect runtime behavior or spec compliance.

## Evidence Log (this round)

- `cargo test --manifest-path tools/xs-language-server/Cargo.toml` → 387 passed / 0 failed / 0 ignored.
- `git log --oneline master~7..master` → 7 commits on master (32322e5, f236dfc, 87830fb, c0d5e2b, 1586749, 3e24758, ca5cf24).
- `git diff --stat master~7..master -- 'mod/' 'tools/intellij-xs-plugin/src/main/kotlin/'` → empty.
- `git diff master~7..master -- tools/xs-language-server/lsp/src/server.rs` → adds `signature_help_provider`, `document_link_provider`, handler trait methods; no existing capability removed or changed.
- Live `lsp_roundtrip_test` → all probes PASS, exit code 0.
- `./gradlew :test` in `tools/intellij-xs-plugin/` → BUILD SUCCESSFUL.
