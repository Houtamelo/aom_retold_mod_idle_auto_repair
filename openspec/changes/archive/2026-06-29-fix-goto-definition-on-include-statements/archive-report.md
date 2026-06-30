# Archive Report: fix-goto-definition-on-include-statements

## Status
Archived

## Date
2026-06-29

## Artifacts
- Spec (NEW): `openspec/specs/spec-lsp-include-goto-definition.md`
- Spec (synced from prior): N/A
- Design: `openspec/changes/archive/2026-06-29-fix-goto-definition-on-include-statements/design.md`
- Tasks: `openspec/changes/archive/2026-06-29-fix-goto-definition-on-include-statements/tasks.md`
- Verify report: `openspec/changes/archive/2026-06-29-fix-goto-definition-on-include-statements/verify-report.md`
- Proposal: `openspec/changes/archive/2026-06-29-fix-goto-definition-on-include-statements/proposal.md`
- Exploration: `openspec/changes/archive/2026-06-29-fix-goto-definition-on-include-statements/explore.md`

## Outcome
This change resolves Issue #4 by extending the XS LSP server's `textDocument/definition` handler so that a cursor inside an `include "..."` path token resolves to the included file. Resolution reuses the existing `Workspace::resolve_include` pipeline (mod overlay first, vanilla `game/` fallback), so navigating from a mod file lands on the mod's copy of the target when one exists. The plugin's existing `XsGotoDeclarationHandler` (added by Issue #1) forwards the resulting `Location` unchanged, so all three editor triggers — Ctrl+Click, the go-to-definition keybind, and right-click → Go to → Definition — work without new plugin navigation code.

Verification verdict is **PASS WITH WARNINGS**. All functional requirements (R1–R8) are met:
- `textDocument/definition` resolves single-line include paths (`R1`).
- Multi-line path containment is handled by tree-sitter node ranges (`R1b`), though no automated test exercises it.
- Mod overlays take precedence over vanilla files (`R2`).
- Unmodded paths fall back to the vanilla `game/` tree (`R3`).
- Unresolvable targets return an empty response (`R4`).
- Positions outside the path token fall through to identifier resolution (`R5`).
- `XsGotoDeclarationHandler` forwards include-path elements to the resolver (`R6`).
- Five new LSP regression tests and one new plugin regression test pass (`R7`).
- `pluginVersion` is bumped to `0.4.0` (`R8`).

The warnings recorded in the verify report are process or edge-case items, not functional blockers:
1. The apply phase left a large amount of unrelated `rustfmt` churn and an unrelated edit to `scripts/deploy-mods.sh` unstaged; intended files are also unstaged.
2. No automated test covers R1b (multi-line include paths).
3. The new plugin forwarding test is shallow — it uses a stub `TestResolver`, not the real `XsLspDefinitionResolver`/`VirtualFileManager` path.
4. Escaped quotes inside include paths are not unescaped.
5. `range_contains` compares tree-sitter byte columns against LSP UTF-16 character positions, which is low-risk for ASCII include paths.

Builds are clean: `cargo build`/`cargo clippy` for the LSP crate pass, `./gradlew :buildPlugin` produces `dist/intellij-xs-plugin-0.4.0.zip`, and the full Rust test suite passes.

## Test count trajectory
- LSP: **215 → 220** (+5 from `tools/xs-language-server/tests/include_goto_definition_repro.rs`)
- Plugin: **67 → 68** (+1 `testIncludeStringLiteralDelegatesToResolver` in `XsGotoDeclarationHandlerTest.kt`)

## Plugin version
Bumped `0.3.0 → 0.4.0` (MINOR, per `AGENTS.md` new-LSP-feature policy).

## Cross-references
- User issue: `docs/issues/2026-06-29-runtime-issues.md` Issue 4 (now marked Resolved).
- Related archived change: `openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/` — its `XsGotoDeclarationHandler` forwards the LSP `Location` produced by this change.
- Related spec: `openspec/specs/spec-lsp-forward-decl-honesty.md` (Issue #2) for the `Workspace::resolve_include` pipeline reused here.
- Plugin handler spec: `openspec/specs/spec-xs-goto-declaration-handler.md`.
- Version policy: `AGENTS.md` plugin version bump policy.
- Strict-TDD overrides: `openspec/config.yaml` mandates `cargo test` and `./gradlew :test` for changes under `tools/xs-language-server/` and `tools/intellij-xs-plugin/`.

## Commit guidance
The orchestrator should commit only the intended files. Recommended Conventional Commits message:

```
feat(xs-lsp): navigate from include statement path token via textDocument/definition (Issue #4)

- Add include-aware branch in XsLanguageServer::goto_definition before
  identifier resolution.
- Add parser::detect_include_path_at_position to detect the cursor inside
  an include_directive path token.
- Add Workspace::resolve_include_for_file wrapper that maps an absolute
  includer path to the existing mod-overlay/vanilla resolve_include pipeline.
- Add LSP integration tests in tools/xs-language-server/tests/include_goto_definition_repro.rs
  covering direct navigation, mod overlay, vanilla fallback, not-found, and
  identifier fallback.
- Add plugin regression test testIncludeStringLiteralDelegatesToResolver in
  XsGotoDeclarationHandlerTest.kt to verify Issue #1's handler forwards
  include-path location elements unchanged.
- Mark Issue #4 resolved in docs/issues/2026-06-29-runtime-issues.md.

Bumps pluginVersion 0.3.0 → 0.4.0.
Closes Issue #4 from docs/issues/2026-06-29-runtime-issues.md.
```

Selectively stage:
- `tools/xs-language-server/src/server.rs`
- `tools/xs-language-server/src/parser.rs`
- `tools/xs-language-server/src/workspace.rs`
- `tools/xs-language-server/tests/include_goto_definition_repro.rs`
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt`
- `tools/intellij-xs-plugin/gradle.properties`
- `docs/issues/2026-06-29-runtime-issues.md`
- `openspec/specs/spec-lsp-include-goto-definition.md`
- `openspec/changes/archive/2026-06-29-fix-goto-definition-on-include-statements/`

Do **not** stage the unrelated `rustfmt` churn across `tools/xs-language-server/src/*.rs`, the unrelated `scripts/deploy-mods.sh` edit, or the unrelated archive `tasks.md` update from the earlier change.
