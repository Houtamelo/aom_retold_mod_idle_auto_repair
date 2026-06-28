# Archive Report: lsp-server-semantic-fixes

## 1. Verdict

**PASS WITH ISSUES**

## 2. Summary

This change eliminated the ~189 false-positive diagnostics the Rust XS LSP server emitted when linting the official AoM:R `game/**/*.xs` scripts. It scoped duplicate-`extern` detection to the current link unit, corrected diagnostic URI/range attribution, made semantic callee resolution fall back to the engine-API cache, changed argument-count checks to respect default parameters, and recognized `rule` symbols (plus dynamic registrations) as valid callees. The IntelliJ plugin was bumped to **0.1.6** to ship the updated LSP binary.

## 3. What changed

### Specs synced to `openspec/specs/`

- `spec-extern-collision-scope.md`
- `spec-diagnostic-range-uri.md`
- `spec-engine-api-resolution.md`
- `spec-default-arg-aware-typecheck.md`
- `spec-callable-rules.md`
- `spec-game-folder-test-coverage.md`

### LSP server / plugin files modified

- `tools/xs-language-server/src/semantic.rs` — link-unit extern collisions, engine-API fallback, rule resolution
- `tools/xs-language-server/src/diagnostics.rs` — per-URI grouping, `DiagnosticCategory`, stable helpers
- `tools/xs-language-server/src/typecheck.rs` — default-aware arg counts, rule bypass
- `tools/xs-language-server/src/symbols.rs` — rule registration extraction, `SymbolKind::Rule`
- `tools/xs-language-server/src/engine_api.rs` — `EngineApi::lookup`
- `tools/xs-language-server/tests/game_folder_parse.rs` — full-diagnostic zero-assertion gate
- `tools/intellij-xs-plugin/gradle.properties` — `pluginVersion` 0.1.5 → 0.1.6
- `tools/intellij-xs-plugin/build/distributions/intellij-xs-plugin-0.1.6.zip` produced

### Delivery

Six planned work-unit commits on `xs-language-server/true-include-paste-tests`, plus one additional task-completion marker commit.

## 4. Verification evidence

From `verify-report.md`:

- All 6 tracked diagnostic categories report **0** on the full official game folder:
  - `duplicate_extern=0`
  - `wrong_uri=0`
  - `unresolved_symbol=0`
  - `wrong_arg_count=0`
  - `rule_call_unresolved=0`
  - `total=0`
- **153** library unit tests passed, 0 failures.
- **9** game-folder integration tests passed, 0 failures.
- `cargo build --release` succeeded.
- `./gradlew buildPlugin` succeeded; `dist/intellij-xs-plugin-0.1.6.zip` (4.07 MB) produced.
- Conventional commits used throughout.

## 5. Deviations from design

No semantic or architectural deviations from `design.md` were required; the implementation followed all 10 architecture decisions (AD-1 through AD-10) and satisfies every spec scenario.

Process/hygiene deviations recorded in the verification report:

| # | Deviation | Status | Details |
|---|---|---|---|
| 1 | Extra implementation commit | Accepted | The branch landed 7 implementation commits instead of the planned 6. Commit `87b13fc` combined T17 (plugin version bump) and T18 (docs update); commit `80e9e22` added a task-completion marker. Both are conventional-commits compliant and do not change the code state. |

## 6. Open issues / future work

- **Category E (syntax highlighting)** remains out of scope for the LSP server; it is IDE-side (stub PSI / TextMate bundle loading) and not addressed by this change.
- Pre-existing `cargo` warnings in unrelated files (`unused_variables` in `server.rs`, deprecated LSP fields, dead code in `AiplansFile`) were not introduced by this change and remain for future cleanup.
- Unrelated working-tree strays from the concurrent `true-include-paste` change are present in the same branch. They must be excluded from the commit for this change.

## 7. Commit history

Main implementation commits (newest first):

| SHA | Commit |
|---|---|
| `87b13fc` | `chore(xs-lsp): bump plugin to 0.1.6 and document semantic-fix resolution` |
| `e194747` | `fix(xs-lsp): make rules and function-pointer callbacks callable` |
| `9444723` | `fix(xs-lsp): make typecheck argument-count check default-aware` |
| `ba44ec4` | `fix(xs-lsp): scope extern collision checks and emit diagnostics at declaration site (fixes ~34 false positives)` |
| `1c20ab2` | `fix(xs-lsp): resolve callees against engine API cache (fixes ~140 false positives)` |
| `5cc9b60` | `test(xs-lsp): expand game_folder_parse to assert zero diagnostics across all categories` |

Additional marker commit:

- `80e9e22` `docs: mark SDD tasks T17-T18 complete`

## 8. Related artifacts

- `openspec/changes/archive/xs-language-server/` — prior foundational LSP work.
- `openspec/changes/archive/true-include-paste/` — concurrent change on the same branch whose working-tree strays must not be included here.
- `openspec/changes/archive/intellij-xs-plugin-platform-lsp-migration/` — the migration that enabled diagnostics to surface in the IDE and motivated this follow-up.
- `docs/post-lsp-migration-issues.md` — updated with resolution status for categories A-D.

## 9. Engram

Saved as `sdd/lsp-server-semantic-fixes/archive`:

- **title**: "SDD archive: lsp-server-semantic-fixes"
- **topic_key**: `sdd/lsp-server-semantic-fixes/archive`
- **type**: `architecture`

Updated `next-change/lsp-server-semantic-fixes` (obs #1357):

- **title**: "Resolved: lsp-server-semantic-fixes (PASS WITH ISSUES)"
- **content**: link to `openspec/changes/archive/lsp-server-semantic-fixes/archive-report.md`

## 10. Notes

- Source code was not modified during archive; only spec copies, folder move, and report creation were performed.
- The user should commit the new `openspec/specs/` files and the archived change folder separately from unrelated working-tree strays.
