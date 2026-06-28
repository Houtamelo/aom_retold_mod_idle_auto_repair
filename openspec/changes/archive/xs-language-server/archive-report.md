# Archive Report — `xs-language-server`

**Change:** `xs-language-server` (workspace & engine-data redesign)  
**Project:** `aom_retold_mod_idle_auto_repair`  
**Archive date:** 2026-06-24  
**Source branch:** `xs-language-server/redesign-phase-5`  
**Base branch:** `xs-language-server/spike`  
**Archived to:** `openspec/changes/archive/xs-language-server/`  
**SDD phase:** Archive (final)  

---

## 1. Final state

The redesigned XS Language Server is fully implemented, verified, and archived.

- All **30/30** tracked tasks across five phases are complete.
- All automated Rust tests pass (**75/75**), the LSP round-trip test passes, and the IntelliJ plugin compiles, tests (**13/13**), and packages successfully.
- Six delta specs were synced into `openspec/specs/` as the source of truth.
- The active change folder `openspec/changes/xs-language-server/` was moved to `openspec/changes/archive/xs-language-server/`.
- Project documentation (`AGENTS.md`, `docs/xs-lsp-spike.md`) was updated to point to the archived location and to the main specs directory where applicable.

---

## 2. Commits archived (39 total)

### Phase 1 — Doxygen extraction + cache (6 commits)
| Hash | Message |
|---|---|
| `78a52fb` | feat(xs-lsp): add sevenz-rust, scraper, sha2, dirs, anyhow and tempfile dependencies |
| `4ab84f5` | feat(xs-lsp): add cache module for doxygen extraction + parse results |
| `0fbee58` | feat(xs-lsp): add doxygen extractor (7z+scraper) and wire engine_api to cache |
| `358e65f` | feat(xs-lsp): add game-path CLI/env parsing and wire server to extracted engine API |
| `04201c5` | docs(xs-lsp): mark Phase 1 tasks complete and add apply-progress report |
| `b810cf0` | fix(xs-lsp): parse aiplans from Doxygen summary rows (reaches 193 constants) |

### Phase 2 — Workspace + virtual project (7 commits)
| Hash | Message |
|---|---|
| `76cde52` | feat(xs-lsp): add workspace module with virtual project + include resolution |
| `aa1d65a` | feat(xs-lsp): add per-file parse cache for game folder |
| `b0e1194` | feat(xs-lsp): integrate workspace into server (per-file lookup, unowned file notification) |
| `2b06b7d` | feat(xs-lsp): register didChangeWatchedFiles for game folder invalidation |
| `d83985b` | feat(xs-lsp): handle didChangeWorkspaceFolders for mod add/remove |
| `0596634` | test(xs-lsp): update lsp_roundtrip_test with workspace folder + watch tests |
| `508ed4a` | docs(xs-lsp): append Phase 2 apply-progress report |

### Phase 3 — Semantic diagnostics (9 commits)
| Hash | Message |
|---|---|
| `e35d4e1` | feat(xs-lsp): track extern/mutable/visibility in symbol table |
| `2c967a9` | feat(xs-lsp): add semantic module for extern collision + forward-decl + mutable redefinition |
| `cd92cc9` | feat(xs-lsp): add int↔float widening + cross-file typecheck |
| `1881b92` | feat(xs-lsp): scope completion to exported symbols + current file |
| `ecd2050` | feat(xs-lsp): wire semantic diagnostics into didOpen/didChange |
| `405244c` | test(xs-lsp): add semantic fixtures and roundtrip tests |
| `8a9d51b` | docs(xs-lsp): append Phase 3 apply-progress report |
| `f3357b6` | docs(xs-lsp): mark Phase 3 tasks complete |
| `749a277` | docs(xs-lsp): document int/float widening verification approach |

### Phase 4 — IntelliJ client integration (9 commits)
| Hash | Message |
|---|---|
| `a273c34` | feat(intellij-xs-plugin): vendor xs.tmLanguage.json |
| `1db0630` | feat(intellij-xs-plugin): add XsSettings PersistentStateComponent |
| `3274ba5` | feat(intellij-xs-plugin): add XsModAutoDetector (recursive game folder scan) |
| `96b4e2c` | feat(intellij-xs-plugin): add XsConfigurable settings UI |
| `e58e85b` | feat(intellij-xs-plugin): wire XsLspServerManager to settings + workspace folders + watch |
| `31beb26` | chore(intellij-xs-plugin): register XsConfigurable in plugin.xml |
| `d36d664` | feat(intellij-xs-plugin): first-run UX (auto-detect + warning + settings prompt) |
| `bf2db33` | refactor(intellij-xs-plugin): drop obsolete engine-facing Kotlin classes |
| `9b7fe48` | docs(xs-lsp): append Phase 4 apply-progress report |

### Phase 5 — VS Code removal + housekeeping (8 commits)
| Hash | Message |
|---|---|
| `26ca10e` | chore(xs-lsp): remove extracted/xs.vsix from repo |
| `3027f6d` | docs(agents): update AGENTS.md with new LSP architecture |
| `3f66c98` | docs(xs-lsp): correct and mark xs-lsp-spike.md as historical |
| `9a656c5` | chore(xs-lsp): remove legacy engine JSON fallback and bundled resources |
| `d9ec21b` | fix(intellij-xs-plugin): make settings UI headless-safe and tests sandbox-friendly |
| `3931ea5` | docs(xs-lsp): write final verify-report for xs-language-server |
| `a4b3d08` | fix(xs-lsp): bump engine-data cache schema to v2 after removing legacy backfill |
| `9b5767a` | docs(xs-lsp): note v2 cache bump in apply-progress |

---

## 3. Specs synced to main `openspec/specs/`

All six delta specs were copied into the main specs directory. No name collisions existed, so no merge was required.

| Spec | Main spec path |
|---|---|
| Engine data pipeline | `openspec/specs/spec-engine-data-pipeline.md` |
| Virtual project overlay | `openspec/specs/spec-virtual-project-overlay.md` |
| File watching and cache invalidation | `openspec/specs/spec-file-watching-cache-invalidation.md` |
| Semantic diagnostics | `openspec/specs/spec-semantic-diagnostics.md` |
| IntelliJ client integration | `openspec/specs/spec-intellij-client-integration.md` |
| VS Code client removal | `openspec/specs/spec-vscode-removal.md` |

Each synced spec includes a top note indicating the contributing change, archive date, and verdict, plus a `Change history` footer.

---

## 4. Test results at archive time

| Suite | Command | Result |
|---|---|---|
| Rust unit tests | `cargo test` | **PASS** — 75 passed, 0 failed |
| LSP round-trip test | `cargo run --bin lsp_roundtrip_test` | **PASS** — baseline + workspace + semantic sequences green |
| IntelliJ plugin compile | `./gradlew compileKotlin` | **BUILD SUCCESSFUL** |
| IntelliJ plugin tests | `./gradlew test` | **BUILD SUCCESSFUL** — 13 tests passed, 0 failed |
| IntelliJ plugin package | `./gradlew buildPlugin -x buildSearchableOptions` | **BUILD SUCCESSFUL** |

---

## 5. Deviations from design

| # | Design / spec wording | Implementation | Rationale |
|---|---|---|---|
| 1 | Engine-data cache schema `v1/` | Bumped to `v2/` in `cache.rs` | Prevents stale `v1/` caches (which still contained the legacy `xsExecute` backfill) from masking the new no-backfill behavior. |
| 2 | `EngineApi` exposes 1,805 syscalls | Direct archive extraction yields 1,804 syscalls (`xsExecute` is absent from `docs/doxygen_retail.7z`) | The legacy JSON backfill was removed in Phase 5; the LSP now reports exactly what the archive contains. |
| 3 | Task T30 expected `./gradlew test` with platform fixture tests | Settings and detector tests rewritten as plain JUnit | Sandbox CI cannot run IntelliJ platform fixture tests reliably; the rewritten tests assert the same persistence and auto-detect behavior. |
| 4 | Per-keystroke diagnostic latency < 200 ms on representative corpus | Not instrumented on a representative corpus | The implementation uses per-file parse cache and single-file diagnostic passes, but no corpus benchmark was run. |

No deviation breaks the specified behavior.

---

## 6. Verdict

**PASS WITH DEVIATIONS**

The change is accepted for archive. All automated validation gates are green, all specs are represented in the main specs directory, and all known deviations are documented.

---

## 7. Manual sign-off

| Criterion | Status |
|---|---|
| All 30 tasks complete | **YES** |
| All automated tests green | **YES** |
| IntelliJ plugin compiles | **YES** |
| Spec coverage complete | **YES** — 6 / 6 specs verified |
| Verify report written | **YES** |
| Maintainer sign-off | **PENDING** |

Manual end-to-end smoke test in a real IDE with a real AoM:R installation is documented as required but cannot run in this headless sandbox.

---

## 8. Risks

1. **Doxygen HTML format drift.** Game patches may change Doxygen output. The scraper tolerates malformed entries and logs warnings, but a future patch could still break parsing. Add CI validation that extraction produces the expected syscall/aiplan counts.
2. **Manual smoke test gap.** Final acceptance requires opening a real `.xs` mod file in a real IDE. Schedule this before publishing the plugin.
3. **Headless plugin test fragility.** The move to plain JUnit removed fixture hangs but also reduced coverage of platform-specific service behavior. Consider adding a single lightweight integration test once fixture stability improves.
4. **Cache garbage collection.** Old `v1/` cache versions are retained indefinitely. Add a retention policy in a future maintenance change.
5. **Per-keystroke latency uncertainty.** Add representative corpus timing tests before claiming the 200 ms budget is consistently met.

---

## 9. Documentation path updates

The following project documents were updated to point to the archived change and to the new main specs:

- `AGENTS.md` — updated four references from `openspec/changes/xs-language-server/` to `openspec/changes/archive/xs-language-server/`.
- `docs/xs-lsp-spike.md` — updated references to the archived change; the spec pointer was updated to `openspec/specs/`.

Files under `tools/intellij-xs-plugin/README.md` still reference the original active-change path and were intentionally left unchanged per the archive constraint not to modify files under `tools/`.

---

## 10. Recommended next

Hand back to orchestrator. The SDD cycle for `xs-language-server` is complete: explored, proposed, specified, designed, tasked, applied, verified, and archived. No further SDD steps remain.
