# SDD Proposal — `intellij-xs-plugin`

**Status:** Ready for spec  
**Project:** `aom_retold_mod_idle_auto_repair`  
**Change name:** `intellij-xs-plugin`  

---

## 1. Intent (Why)

We are building an IntelliJ Platform plugin that brings first-class IDE support for Age of Mythology: Retold XS scripts to JetBrains products (IntelliJ IDEA and Rider). Today, modders in this repo edit XS in plain text or VS Code; the existing VS Code extension already captures the engine API surface, a TextMate grammar, and workspace code intelligence. Porting that support to IntelliJ makes Rider a viable environment for AoM:R AI scripting and gives contributors who prefer JetBrains the same completion, hover, go-to-definition, parameter info, syntax highlighting, and XML-derived constants that VS Code users already have. The plugin never touches the deployed mods; it is purely a development-time tooling addition under `tools/intellij-xs-plugin/`.

---

## 2. Scope (What)

### 2.1 In scope

The plugin targets full feature parity with the VS Code XS extension's six capability families:

1. **Engine syscall auto-completion and parameter completion** — complete the 1,805 engine syscalls from `syscalls.json`; insert default parameter values when the user types `(` or `,`.
2. **Workspace symbol completion** — complete user-defined top-level functions, rules, variables, classes, methods, and members; class member completion after `.` once PSI exists.
3. **Hover documentation** — show syscall signature + docstring on hover; show user-symbol declarations once PSI exists.
4. **Go-to definition / references** — `Ctrl+B` on a syscall jumps to a generated in-memory stub; `Ctrl+B` on workspace symbols resolves to declarations; find-usages once PSI exists.
5. **Parameter highlight / parameter info** — bold the current parameter inside any engine call; extend to workspace calls once PSI exists.
6. **Syntax highlighting** — reuse the existing TextMate grammar for keywords, types, comments, strings, numbers, rule names, and `k`/`g`/`s` prefixes.

Plus:

7. **XML-derived constants + AI plan constants** — complete `cUnitType*`, `cTech*`, `cCiv*`, `cCulture*`, `cProtoPower*` from user-supplied extracted XML files, plus `cPlan*` constants from `aiplans.json`.
8. **Regeneration pipeline** — a committed Gradle task/CLI that rebuilds `syscalls.json` from `docs/doxygen_retail/` when the engine data changes.

### 2.2 Out of scope (v1)

- **Debugging integration with the AoM:R game engine** — no breakpoints, no attach, no aiEcho log viewer.
- **Deployment tooling** — no deploy-to-`mods/local/` button; the existing `scripts/deploy-mods.sh` remains the deploy path.
- **Formatting / code style enforcement** — no automatic reindentation or style inspections.
- **Refactorings** — no rename symbol across files, no safe delete, no extract method.
- **Live Data.bar extraction** — the plugin reads extracted XML files only; it will not crack `data/Data.bar`.
- **Marketplace publishing** — distribution is deferred until the plugin is functional end-to-end.

### 2.3 Success criteria

The plugin is "done" when a user can:

- Open `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` in the IDE and see correct syntax coloring.
- Type `aiE`, press `Ctrl+Space`, and see `aiEcho`, `aiEchoCategory`, and `aiEchoWarning` with engine-API docstrings.
- Type `aiPlanCreate(` and have the first default parameter value inserted or proposed.
- Hover over `kbUnitCount` and see `int kbUnitCount(...)` plus its help paragraph.
- Press `Ctrl+B` on `aiPlanCreate` and land in a generated stub or documentation view.
- Press `Ctrl+B` on a workspace function such as `disableAutoScouting` and jump to its declaration.
- Inside `aiPlanSetVariableBool(...)`, see the current parameter bolded in the parameter-info popup.
- Configure five XML paths and get completion for `cUnitTypeVillagerGreek`, `cTechAge2`, etc.
- Run `./gradlew regenerateSyscalls` and produce a `syscalls.json` diff consistent with `docs/doxygen_retail/`.

---

## 3. Approach (How)

### 3.1 Architecture sketch

- **IDE-side plugin** — Gradle/Kotlin project under `tools/intellij-xs-plugin/`. Built with IntelliJ Platform Gradle Plugin 2.x, Kotlin 1.9.25/2.x, JVM 21, targeting `IC-252` with a minimum runtime of `IC-242`.
- **Data bundle** — `syscalls.json`, `aiplans.json`, and `syntaxes/xs.json` are copied into `src/main/resources/` as read-only bundled resources.
- **Build-time regeneration script** — `tools/intellij-xs-plugin/scripts/` (or a Gradle task) parses `docs/doxygen_retail/*.html` and emits a fresh `syscalls.json` for inspection and commit when the engine changes.
- **User XML inputs** — one settings component stores five directory/file paths; a recursive file scanner finds `*.xml` plus additive `*_mods.xml` overlays and emits prefixed constants.

### 3.2 Reuse strategy

| Asset | How it is reused | Where it lives in the plugin |
|-------|------------------|------------------------------|
| `syntaxes/xs.json` | Bundled as a TextMate bundle via `TextMateBundleProvider` | `src/main/resources/xs-textmate-bundle/syntaxes/xs.json` |
| `syscalls.json` | Loaded once at startup, indexed by name | `src/main/resources/data/syscalls.json` |
| `aiplans.json` | Loaded once at startup, indexed by prefix | `src/main/resources/data/aiplans.json` |
| `docs/doxygen_retail/` | Regeneration-only input; not bundled | referenced from `tools/intellij-xs-plugin/scripts/` |

### 3.3 Phasing

Phasing follows the explore doc's cheap-first / expensive-last recommendation. Each phase produces a manually testable plugin.

#### P0 — Scaffold

- Create `tools/intellij-xs-plugin/` Gradle project.
- Register `XsFileType`, `XsLanguage`, and a language icon for `*.xs`.
- Bundle the TextMate grammar through `TextMateBundleProvider` and declare a dependency on the bundled `org.jetbrains.plugins.textmate` plugin.
- Add a `runIde` run configuration and verify the plugin loads.

**Deliverable:** `.xs` files are syntax-highlighted.

#### P1 — Engine API surface

- Load `syscalls.json` and `aiplans.json` as bundled resources and build in-memory name indexes.
- Implement `XsCompletionContributor` for syscall names and parameter-value completion on `(`/`,`.
- Implement `XsDocumentationProvider` for syscall hover.
- Implement `XsParameterInfoHandler` for syscall parameter highlight.

**Deliverable:** Engine syscalls auto-complete with docs and parameter info.

#### P2 — Workspace code intelligence (minimal)

- Build a JFlex lexer and a minimal parser (GrammarKit first; hand-rolled fallback if lambdas/`ref`/arrays prove unwieldy) that recognizes functions, rules, variables, classes, and top-level identifiers.
- Implement `PsiReferenceContributor` + `PsiReferenceProvider` for workspace go-to-definition.
- Extend completion to propose top-level workspace functions and rules.

**Deliverable:** Clicking a workspace function name jumps to its definition.

#### P3 — Class and lambda fidelity

- Extend the PSI for classes, methods, member variables, `ref`, arrays, and lambda types.
- Implement member access resolution (`obj.member`, `obj.method(...)`).
- Add member completion after `.` and hover for class members/methods.

**Deliverable:** Parity with VS Code extension v1.0.5–1.0.8 class/lambda features.

#### P4 — XML-derived constants + AI plan constants

- Add a settings page for the five XML paths (`proto.xml`, `techtree.xml`, `civs.xml`, `cultures.xml`, `powers.xml`).
- Recursively scan the configured directories for `*.xml` and overlay `*_mods.xml` files.
- Parse entries and emit prefixed constants (`cUnitType*`, `cTech*`, `cCiv*`, `cCulture*`, `cProtoPower*`).
- Load `aiplans.json` and emit `cPlan*` constants.
- Watch configured XML files for changes and refresh completion.

**Deliverable:** Unit/tech/civ/culture/power constants and AI plan constants auto-complete.

#### P5 — Regeneration pipeline

- Implement a Gradle task or small Kotlin CLI (`./gradlew regenerateSyscalls`) that parses the relevant doxygen HTML files with jsoup and emits `syscalls.json`.
- Run it against the committed `docs/doxygen_retail/` and compare with the bundled JSON.
- Document the update procedure: run task → inspect diff → commit updated JSON.

**Deliverable:** Regenerating against a new game build produces a compatible `syscalls.json`.

---

## 4. Risks & Mitigations

| # | Risk | Impact | Mitigation |
|---|------|--------|------------|
| 1 | **TextMate rendering parity gaps** — IntelliJ's TextMate plugin may interpret the 1,626-line C++-derived grammar differently than VS Code on edge-case captures or regex flags. | Low–Medium | Make P0's definition of done a side-by-side highlight comparison of `human_assist.xs` in Rider and VS Code; fix scopes before moving to P1. |
| 2 | **PSI parser is the long pole** — XS lambdas, `ref`, arrays, and expression precedence may exceed GrammarKit's comfort zone, forcing a hand-rolled parser. | Medium–High | Start with GrammarKit (canonical IntelliJ path), but budget for downgrade to a hand-rolled recursive-descent parser. Defer P3 until P2 is solid. |
| 3 | **Engine data drift** — `syscalls.json` and `aiplans.json` become stale when the game patches. | Medium | Deliver P5 regeneration pipeline before calling v1 complete; document the "run `./gradlew regenerateSyscalls` → inspect diff → commit" procedure. |
| 4 | **XML discovery friction** — users must extract XMLs from `Data.bar` manually and configure paths; misconfiguration silently disables constant completion. | Low | Settings UI uses labeled file choosers and validation; if paths are missing, show a non-blocking banner with a link to extraction instructions. |
| 5 | **Rider-specific behavior** — TextMate bundle registration and completion behavior may differ from IntelliJ IDEA. | Low | Add a Rider smoke test to P0's acceptance criteria before marking scaffold complete. |

---

## 5. Rollback plan

- The plugin is isolated under `tools/intellij-xs-plugin/`. It does not modify any file outside that folder and it does not touch the `mod/` deployable packages.
- No environment variables, system PATH entries, or global IDE settings are changed by the code itself.
- If the change needs to be reverted, delete `tools/intellij-xs-plugin/`. The repo returns to its pre-plugin state; no source under `mod/` is affected.
- The regeneration pipeline is also inside the plugin folder (`tools/intellij-xs-plugin/scripts/` or a Gradle task); removing the folder removes the pipeline.
- No user-facing artifact under `mod/*/game/` references the plugin, so deployed mods remain unchanged.

---

## 6. Patch-maintenance impact on `human_assist.xs`

**Explicit statement:** this change does NOT modify `mod/*/game/ai/human_assist/human_assist.xs` or any other `.xs` source file in the mod packages. The combined mod's `human_assist.xs` overlay, and the `auto_repair.xs` / `auto_scout.xs` include files, remain untouched. Patch-maintenance notes already documented in `mod/<name>/README.md` files continue to apply exactly as before; the plugin is an external IDE tool and introduces no new patch-maintenance burden on XS overlays.

---

## 7. Open questions for user confirmation

The following decisions were baked during exploration. Please confirm or push back:

1. **XML inputs** — The plugin reads from extracted XML files only (no `Data.bar` cracking). The user points to a directory and the plugin recursively scans for `*.xml`. Is this acceptable?
2. **Mods overlay** — The plugin automatically discovers and merges `*_mods.xml` additive override files when present. Keep this behavior?
3. **Minimum IntelliJ Platform** — Minimum supported platform is **2024.2 / IC-242**; the plugin is built/targeted against **2025.2 / IC-252**. Confirm?
4. **Syscall go-to-definition** — For engine syscalls, the plugin uses IntelliJ's `LightVirtualFile` API to generate an in-memory stub on demand (no disk pollution). Confirm documentation-stub behavior is preferred?
5. **Regeneration pipeline dependencies** — The doxygen-to-JSON pipeline may use jsoup or a similar HTML parser as a Maven/Gradle dependency. Acceptable?

---

## 8. Test & verification strategy

### 8.1 Automated tests (IntelliJ testFramework)

- Add `testFramework(TestFrameworkType.Platform)` in Gradle.
- Add fixture files under `src/test/testData/`:
  - `human_assist.xs` — copy of `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`.
  - `minimal.xs` — hand-written XS with a function, a rule, a variable, a class, a lambda, and a class method.
- Write one test class per feature family:
  - `XsSyscallCompletionTest` — assert `aiPlanCreate` appears and `aiPlanCreate(` inserts the first default parameter.
  - `XsHoverTest` — assert the generated documentation contains the syscall signature.
  - `XsParameterInfoTest` — assert the parameter popup highlights the correct index.
  - `XsWorkspaceGoToDefinitionTest` — assert `Ctrl+B` on a local function name resolves to its declaration.
  - `XsTextMateHighlightingTest` — assert token types are assigned for keywords and strings.
  - `XsXmlConstantsCompletionTest` — assert `cUnitTypeVillagerGreek` appears when a synthetic proto XML is configured.

### 8.2 Manual smoke test

- Run `./gradlew runIde`.
- Open the `aom_retold_mod` project.
- Open `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`.
- Verify syntax colors, engine-syscall completion, hover on `aiPlanCreate`, parameter info inside `aiPlanAddUnitType(...)`, go-to-definition on `disableAutoScouting`, XML-derived constants (once configured), and Rider load.

### 8.3 Note on XS execution

No automated XS execution is required; correctness is judged by IDE behavior, not by the game engine. The existing repo verification method (`scripts/deploy-mods.sh` + in-game log inspection) remains unchanged for `.xs` source changes.

---

## 9. Deliverable contract

Return to orchestrator:

```json
{
  "status": "complete",
  "executive_summary": "Propose an IntelliJ Platform plugin under tools/intellij-xs-plugin/ that ports the VS Code XS extension's six feature families to JetBrains IDEs. The plugin is isolated, has no patch-maintenance impact on existing mod overlays, and follows a P0–P5 phased delivery. Top risks are TextMate parity, parser complexity, and engine-data drift; mitigations include GrammarKit-first parser strategy, in-memory syscall stubs, and a committed doxygen-to-JSON regeneration pipeline.",
  "artifacts": [
    "openspec/changes/intellij-xs-plugin/proposal.md",
    "engram:sdd/intellij-xs-plugin/proposal"
  ],
  "next_recommended": "sdd-spec",
  "risks": [
    "TextMate rendering parity between IntelliJ and VS Code may require scope tuning.",
    "PSI parser complexity (lambdas, ref, arrays) could force a hand-rolled parser.",
    "Engine data drift requires the P5 regeneration pipeline to stay accurate across patches."
  ],
  "open_questions_for_user": [
    "Accept extracted-XML-only input (no Data.bar cracking)?",
    "Keep automatic *_mods.xml overlay merging?",
    "Confirm minimum platform IC-242 and build target IC-252?",
    "Confirm LightVirtualFile in-memory stubs for syscall go-to-definition?",
    "Accept jsoup or similar HTML parser for the regen pipeline?"
  ],
  "skill_resolution": "paths-injected"
}
```

---

## 10. References

- Explore artifact: `openspec/changes/intellij-xs-plugin/explore.md`
- VS Code source: `extracted/xs.vsix`
- Engine API docs: `docs/doxygen_retail/`
- Existing mod source to smoke-test: `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`
