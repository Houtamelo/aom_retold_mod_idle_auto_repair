# Verify Report: fix-goto-definition-on-include-statements

## Verdict

PASS WITH WARNINGS

The functional requirements (R1–R8) are met: the LSP resolves include paths to the correct URI, mod overlays win over vanilla files, missing targets return `null`, identifier fallback is preserved, the plugin handler forwards include-path elements, and `pluginVersion` is bumped to `0.4.0`. Builds and all Rust tests are green. The warning is due to working-tree cleanliness: the apply phase left many unrelated `rustfmt` and `scripts/deploy-mods.sh` modifications unstaged, and the intended files are not staged either, contrary to `tasks.md` Task 7.1.

## Test results

- Total LSP tests: **220** (182 unit + 38 integration)
- New LSP tests pass: **5** of **5** in `tools/xs-language-server/tests/include_goto_definition_repro.rs`
- Plugin navigation test passes: **yes** (`testIncludeStringLiteralDelegatesToResolver`)
- Pre-existing plugin test failure: **1** (`XsStartupActivityTest.autoDetectedModsAreSavedAndNotified`) — environmental, caused by untracked `mod/spire_ai/` and `mod/test_targeting/` directories under the project root; unrelated to this change.
- Build: **clean**
  - `cargo build --manifest-path tools/xs-language-server/Cargo.toml --tests`: success
  - `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml --all-targets`: success, no new warnings attributable to touched files or the new test
  - `./gradlew :buildPlugin`: success
- Plugin `.zip`: **built** at `dist/intellij-xs-plugin-0.4.0.zip`
- `.zip` version: **0.4.0**

## Spec coverage

| Requirement | Test | Status |
| ----------- | ---- | ------ |
| R1 — Direct include navigation | `include_goto_definition_repro::test_direct_include_resolves_to_target` | PASS |
| R2 — Mod overlay precedence | `include_goto_definition_repro::test_mod_overlay_resolves_to_mod_file` | PASS |
| R3 — Vanilla fallback | `include_goto_definition_repro::test_vanilla_fallback_when_mod_missing` | PASS |
| R4 — Not-found returns null | `include_goto_definition_repro::test_not_found_returns_null` | PASS |
| R5 — Cursor outside path token falls back | `include_goto_definition_repro::test_cursor_outside_path_token_uses_existing_logic` | PASS |
| R6 — Plugin forwards include-path Location | `XsGotoDeclarationHandlerTest.testIncludeStringLiteralDelegatesToResolver` | PASS (shallow — stubs resolver) |
| R7 — LSP + plugin regression tests added and pass | `include_goto_definition_repro.rs` (5 tests) + `XsGotoDeclarationHandlerTest` (1 new test) | PASS |
| R8 — `pluginVersion = 0.4.0` | `tools/intellij-xs-plugin/gradle.properties:12` | PASS |

## Adversarial findings

1. **Working tree is not in the expected commit-ready state.** `git diff --cached --stat` is empty (nothing staged), while `git diff --stat` shows changes to 23 files. The intended files for this change are modified but not staged, and a large amount of unrelated rustfmt churn is present across `tools/xs-language-server/src/*.rs` and `tools/xs-language-server/tests/game_folder_parse.rs`, plus an unrelated change to `scripts/deploy-mods.sh`. This contradicts `tasks.md` Task 7.1, which says intended files should be staged and unrelated churn discarded.
2. **No automated test covers R1b (multi-line include paths).** `detect_include_path_at_position` uses tree-sitter node ranges, so multi-line paths are expected to work, but the spec scenario R1b is not exercised by any test.
3. **Plugin forwarding test is shallow.** `testIncludeStringLiteralDelegatesToResolver` uses a stub `TestResolver`; it does not exercise the real `XsLspDefinitionResolver` path that converts an LSP `Location` URI to a `PsiElement` via `VirtualFileManager`. It therefore proves delegation but not end-to-end LSP→editor navigation.
4. **Escaped quotes inside include paths are not unescaped.** A path such as `include "with\\\"quote.xs"` (source text with escaped quotes) would have its surrounding literal quotes stripped but the `\"` sequence left in the returned string, causing filesystem resolution to fail. This is an edge case not expected in real AoM:R includes.
5. **Potential LSP position/byte mismatch for non-ASCII paths.** `range_contains` compares tree-sitter byte columns against LSP UTF-16 character positions. Include paths are ASCII in practice, so this is low-risk.

## Working tree state

- Diff vs HEAD: 23 files modified, including:
  - Intended: `tools/xs-language-server/src/server.rs`, `tools/xs-language-server/src/parser.rs`, `tools/xs-language-server/src/workspace.rs`, `tools/xs-language-server/tests/include_goto_definition_repro.rs`, `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt`, `tools/intellij-xs-plugin/gradle.properties`, `docs/issues/2026-06-29-runtime-issues.md`
  - Unrelated rustfmt/formatting churn: `tools/xs-language-server/src/bin/day1_probe.rs`, `dump_top_level.rs`, `inspect_tree.rs`, `cache.rs`, `completion.rs`, `definition_check.rs`, `diagnostics.rs`, `doxygen.rs`, `engine_api.rs`, `lib.rs`, `main.rs`, `references.rs`, `typecheck.rs`, `word.rs`, `tests/game_folder_parse.rs`
  - Unrelated functional change: `scripts/deploy-mods.sh` (adds `test_targeting` deploy block)
  - Unrelated archive edit: `openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/tasks.md` (marks old tasks complete)
  - Untracked files/directories: `mod/spire_ai/`, `mod/test_targeting/`, `scripts/encode_tactic_xmb.py`, `scripts/find_units_with_tag.py`, `docs/code-reviews/2026-06-29-lsp-and-plugin-findings.csv`, `docs/research/map_awareness_research.md`, `openspec/changes/fix-goto-definition-on-include-statements/*`
- Staged: **clean** (no files staged)
- `pluginVersion`: `0.3.0 → 0.4.0` (MINOR bump, per AGENTS.md new-LSP-feature policy)

## Deviations from design

- **Include target path in tests:** Design examples used `include "debug.xs";` from `game/ai/core/main.xs`, but `Workspace::resolve_include` treats the target relative to the include root (`ai/`), so `"debug.xs"` would resolve to `game/ai/debug.xs`. Tests use `include "core/debug.xs";` to hit `game/ai/core/debug.xs`, matching real XS include-root semantics while preserving the scenario intent. This was already noted by apply.
- **Test file name:** Design.md specified `goto_include_repro.rs`; tasks.md and the actual file are `include_goto_definition_repro.rs`. The tasks.md name is used.
- **Staging:** tasks.md Task 7.1 says intended files should be staged, but `git diff --cached` is empty. This is a process deviation, not a functional one.

## Risks

- The unrelated rustfmt and `deploy-mods.sh` churn could be accidentally committed alongside the intended change if the orchestrator stages with `git add .` instead of selective staging.
- Untracked `mod/spire_ai/` and `mod/test_targeting/` directories will continue to cause `XsStartupActivityTest.autoDetectedModsAreSavedAndNotified` to fail in this environment until they are moved out of the project root or the test is made deterministic.
- R1b (multi-line include paths) is unverified at runtime; while the code path is identical to single-line paths, a regression could slip in unnoticed.

## Recommendation

**Commit only after cleanup.** Selectively stage the intended source/test/doc/gradle files, discard the unrelated rustfmt churn and `scripts/deploy-mods.sh` changes, revert the unrelated archive `tasks.md` edit, and then commit. Do not commit the unstaged noise.
