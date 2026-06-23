# SDD Tasks — `intellij-xs-plugin`

**Change:** `intellij-xs-plugin`  
**Artifact store:** `openspec`  
**Execution mode:** auto  
**Review budget:** 800 changed lines; per-PR soft limit 400 changed lines  
**Generated:** 2026-06-22  

---

## 1. Phase map

### P0 — Scaffold

- **Spec files this phase implements:** `spec-packaging.md`, `spec-syntax-highlighting.md`
- **Design sections this phase exercises:** §1 Architecture overview, §2 Project layout, §3 Tech stack pin, §5 Vendor-vs-reference strategy, §9 Risks & mitigations (TextMate/Rider parity)
- **Deliverable:** A buildable IntelliJ Platform plugin under `tools/intellij-xs-plugin/` that recognizes `*.xs` files and highlights them using the bundled VS Code TextMate grammar.
- **Manual smoke test:** Run `./gradlew runIde`, open `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`, and confirm keywords, strings, comments, rule names, and `k`/`g`/`s` prefixes are colored. Repeat in Rider if available.
- **Lines-of-code estimate:** P0: ~420 plugin/build lines plus vendored `xs.tmLanguage.json` (≈1,626 lines, reviewed but not authored; excluded from review budget).
- **Risk:** TextMate scope rendering may differ between IntelliJ IDEA and VS Code/Rider. Mitigation: side-by-side parity test on `human_assist.xs` before marking done.

### P1 — Engine API surface

- **Spec files this phase implements:** `spec-engine-completion.md`, `spec-hover-docs.md`, `spec-parameter-info.md`, plus the engine-stub portion of `spec-workspace-navigation.md`
- **Design sections this phase exercises:** §1 (engine side), §5 (bundled JSON resources), §6 (LightVirtualFile stubs), §7.1 (completion provider), §8 (Engine API caching)
- **Deliverable:** All 1,805 engine syscalls auto-complete, insert default parameter values on `(`/`,`, show signature + help on hover, highlight the active parameter, and open an in-memory documentation stub on `Ctrl+B`.
- **Manual smoke test:** In `human_assist.xs`, type `aiE` and complete `aiEcho`; type `aiPlanCreate(` and confirm the first default parameter appears; hover over `kbUnitCount`; place the caret inside `aiPlanSetVariableBool(...)` and confirm parameter tracking; press `Ctrl+B` on `aiPlanCreate`.
- **Lines-of-code estimate:** P1: ~1,050 lines across data layer, completion, parameter info, hover, and stubs.
- **Risk:** Returning/updating 1,805+ lookup items must avoid per-keystroke allocation. Mitigation: pre-build immutable `LookupElementBuilder` lists at plugin load.

### P2 — Minimal PSI and workspace code intelligence

- **Spec files this phase implements:** `spec-class-resolution.md` (lexer, parser, top-level declarations, recovery), `spec-workspace-navigation.md` (workspace functions/rules/variables/classes)
- **Design sections this phase exercises:** §4 Parser strategy ADR (Grammar-Kit), §7.2 Workspace go-to-definition, §8 (PSI/caching), §9 risk 1 (generated files)
- **Deliverable:** A real PSI built by a JFlex lexer and Grammar-Kit BNF grammar supports top-level functions, rules, variables, and classes; `Ctrl+B` on a workspace function/rule/variable/class name jumps to its declaration; top-level workspace symbols appear in completion.
- **Manual smoke test:** Open `human_assist.xs`, press `Ctrl+B` on `disableAutoScouting` and confirm it jumps to the function body; type the first few characters of `checkReservePlan` and accept a workspace completion.
- **Lines-of-code estimate:** P2: ~1,200 lines across lexer, BNF grammar, parser definition, PSI element classes, references, and tests.
- **Risk:** Grammar-Kit may struggle with XS recovery on partial declarations. Mitigation: add explicit recovery rules in the BNF and keep parser fixture tests for every broken input.

### P3 — Class and lambda fidelity

- **Spec files this phase implements:** `spec-class-resolution.md` (classes, methods, members, lambdas, `ref`, arrays), `spec-workspace-navigation.md` (member resolution), `spec-hover-docs.md` (workspace hover extension)
- **Design sections this phase exercises:** §4 Parser strategy ADR (partial fallback), §7.1 (member lookups), §7.2 (workspace symbols), §8 (StubIndex/caching)
- **Deliverable:** Classes expose methods and members; member access `obj.member` / `obj.method(...)` resolves and completes after `.`; lambdas, `ref`, and array types parse and expose named elements; hover works on workspace symbols and members.
- **Manual smoke test:** Open a fixture containing a class and lambda, type a class instance followed by `.`, and confirm member/method completion; press `Ctrl+B` on a member reference.
- **Lines-of-code estimate:** P3: ~1,160 lines across grammar extensions, member resolution, completion, lambda/ref/array PSI, and hover.
- **Risk:** Member resolution without full type inference can mis-resolve when variable names shadow classes. Mitigation: scope resolution is limited to declared types; document this limitation as parity with the VS Code extension.

### P4 — XML-derived constants + AI plan constants

- **Spec files this phase implements:** `spec-xml-constants.md`, `spec-aiplan-constants.md`
- **Design sections this phase exercises:** §7.3 XML constants service, §3 (jsoup not used here; XML parsing uses JDK/DOM plus case-insensitive normalization)
- **Deliverable:** A settings page captures five XML paths; the plugin recursively scans `*.xml` files, merges `*_mods.xml` overlays, and completes `cUnitType*`, `cTech*`, `cCiv*`, `cCulture*`, and `cProtoPower*` constants. Bundled `aiplans.json` drives `cPlan*` completion.
- **Manual smoke test:** Configure paths to extracted game XMLs (proto, techtree, civs, cultures, powers), type `cUnitTypeVill` and complete `cUnitTypeVillagerGreek`; type `cTechAge2`, `cCivEgyptians`, `cCultureAtlantean`, `cProtoPowerRestoration`, and `cPlanAddResourcePriority`.
- **Lines-of-code estimate:** P4: ~800 lines across settings UI, XML service, constants completion, AI plan completion, and file watching.
- **Risk:** XML files from patches may change tag shapes; case-insensitive normalization must remain tolerant. Mitigation: unit tests use synthetic minimal XMLs plus one real extracted file if available.

### P5 — Regeneration pipeline

- **Spec files this phase implements:** `spec-regen-pipeline.md`
- **Design sections this phase exercises:** §5 Vendor-vs-reference strategy, §3 Tech stack pin (`jsoup` 1.18.3)
- **Deliverable:** `./gradlew regenerateSyscalls` parses the relevant `docs/doxygen_retail/*.html` files, emits a fresh `src/main/resources/syscalls.json`, and fails cleanly when inputs are missing.
- **Manual smoke test:** Run `./gradlew regenerateSyscalls`, inspect the git diff against committed `syscalls.json`, and confirm the counts per source file match the distribution in `explore.md`. Verify P1 completion/hover still work with the regenerated JSON.
- **Lines-of-code estimate:** P5: ~380 lines across Gradle task, jsoup parser, validation tests, and procedure doc.
- **Risk:** Doxygen HTML may drift between game builds (missing default tables, overloads). Mitigation: script logs warnings, skips malformed entries, deduplicates by name, and never overwrites on empty output.

---

## 2. Task list

### 0.1 Gradle project skeleton
- [x] Completed
- **Files touched:**
  - `tools/intellij-xs-plugin/build.gradle.kts`
  - `tools/intellij-xs-plugin/settings.gradle.kts`
  - `tools/intellij-xs-plugin/gradle.properties`
  - `tools/intellij-xs-plugin/.gitignore`
  - `tools/intellij-xs-plugin/README.md`
- **Spec reference:** `spec-packaging.md`
- **Acceptance:**
  - `./gradlew buildPlugin` succeeds on a clean checkout.
  - `./gradlew runIde` launches an IDE with the (still empty) plugin.
  - JVM target is 21 and Kotlin version matches the design pin.
  - Signing is wired through environment variables without failing local builds.
- **Verification:** CI `buildPlugin` step + manual `runIde` launch.
- **Estimated changed lines:** ~160
- **Can stand alone?** yes — every later task depends on this.

### 0.2 Plugin manifest + language/file type registration
- [x] Completed
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/XsLanguage.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/XsFileType.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/XsFileTypeFactory.kt`
  - `tools/intellij-xs-plugin/src/main/resources/icons/xs.svg` (language icon)
- **Spec reference:** `spec-packaging.md`
- **Acceptance:**
  - `*.xs` files are associated with the `XS` language.
  - `plugin.xml` declares dependencies on `com.intellij.modules.platform` and `org.jetbrains.plugins.textmate`.
  - Optional Rider dependency loads without error in both IDEA and Rider.
  - Plugin `id`, `name`, `version`, `since-build`, and `until-build` are pinned.
- **Verification:** `buildPlugin` manifest validation + manual open of `human_assist.xs`.
- **Estimated changed lines:** ~120
- **Can stand alone?** yes — depends on 0.1.

### 0.3 TextMate bundle vendoring and provider
- [x] Completed
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` (vendored from `extracted/xs.vsix/extension/syntaxes/xs.json`)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/textmate/XsTextMateBundleProvider.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/XsTextMateHighlightingTest.kt`
  - `tools/intellij-xs-plugin/src/test/testData/human_assist.xs`
- **Spec reference:** `spec-syntax-highlighting.md`
- **Acceptance:**
  - The bundled grammar is loaded as a TextMate bundle for `*.xs`.
  - Keywords (`rule`, `if`, `void`, `int`) and string literals receive non-default scopes.
  - Non-`.xs` files do not get colored as XS.
  - Visual parity with VS Code on `human_assist.xs` is confirmed manually.
- **Verification:** automated `XsTextMateHighlightingTest` + side-by-side manual parity check in IDEA and Rider.
- **Estimated changed lines:** ~80 plugin/test lines plus the vendored grammar (excluded from review-budget).
- **Can stand alone?** yes — depends on 0.2.

### 0.4 Build validation and bundled-resource guard
- [x] Completed
- **Files touched:**
  - `tools/intellij-xs-plugin/build.gradle.kts` (resource validation task)
  - `.github/workflows/intellij-xs-plugin.yml` (new CI workflow)
- **Spec reference:** `spec-packaging.md`
- **Acceptance:**
  - `./gradlew buildPlugin` fails with a clear message if `syscalls.json`, `aiplans.json`, or `xs.tmLanguage.json` are missing.
  - CI runs `./gradlew buildPlugin` and `./gradlew test`.
  - `gen/` and IDEA-generated files are ignored.
- **Verification:** automated CI run + deliberate missing-resource failure test.
- **Estimated changed lines:** ~60
- **Can stand alone?** yes — depends on 0.1 and runs best after 0.3.

### 1.1 Vendored engine resources + API indexes
- [x] Completed
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/resources/syscalls.json` (vendored from `extracted/xs.vsix/extension/syscalls/syscalls.json`)
  - `tools/intellij-xs-plugin/src/main/resources/aiplans.json` (vendored from `extracted/xs.vsix/extension/constants/aiplans.json`)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/constants/XsEngineApi.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/constants/XsAiPlans.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/constants/XsEngineApiTest.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/constants/XsAiPlansTest.kt`
- **Spec reference:** `spec-engine-completion.md`, `spec-hover-docs.md`, `spec-parameter-info.md`, `spec-aiplan-constants.md`
- **Acceptance:**
  - `XsEngineApi.lookup("aiEcho")` returns the full syscall object (name, help, return_type, params, filename).
  - All 1,805 syscalls and 193 AI-plan constants are loaded at plugin init.
  - Lookup is O(1) by exact name and supports prefix iteration.
- **Verification:** automated `XsEngineApiTest` and `XsAiPlansTest` (assert counts + lookup).
- **Estimated changed lines:** ~120 plugin/test lines plus two vendored JSON files (excluded from budget).
- **Can stand alone?** yes — prerequisite for P1 tasks and P4 AI-plan completion.

### 1.2 Engine syscall name completion
- [x] Completed
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/completion/XsCompletionContributor.kt` (syscall-name branch)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/completion/XsSyscallCompletionTest.kt`
  - `tools/intellij-xs-plugin/src/test/testData/minimal.xs`
- **Spec reference:** `spec-engine-completion.md`
- **Acceptance:**
  - Typing `aiE` proposes `aiEcho`, `aiEchoCategory`, `aiEchoWarning`.
  - Selecting a syscall inserts a complete call with parentheses.
  - Completions are filtered by the current identifier prefix.
  - No completions are offered inside comments or string literals.
- **Verification:** automated `XsSyscallCompletionTest` + manual in `human_assist.xs`.
- **Estimated changed lines:** ~220
- **Can stand alone?** yes — depends on 1.1.

### 1.3 Default parameter-value completion on `(` / `,`
- [x] Completed
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/completion/XsCompletionContributor.kt` (parameter-value branch)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/completion/XsSyscallCompletionTest.kt` (additional assertions)
- **Spec reference:** `spec-engine-completion.md`
- **Acceptance:**
  - Typing `aiPlanCreate(` inserts/proposes the first parameter's default value when one exists.
  - Typing `,` inside a known syscall call proposes the next parameter's default value.
  - Zero-parameter syscalls (`xsDisableSelf(`) insert nothing extra.
- **Verification:** automated test additions + manual in `human_assist.xs`.
- **Estimated changed lines:** ~150
- **Can stand alone?** yes — extends 1.2; can ship together as one PR.

### 1.4 Hover documentation provider
- [x] Completed
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/documentation/XsDocumentationProvider.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/documentation/XsHoverTest.kt`
- **Spec reference:** `spec-hover-docs.md`
- **Acceptance:**
  - Hovering over `kbUnitCount` renders `int kbUnitCount(...)` and help paragraph.
  - Empty help shows a deterministic placeholder instead of a blank popup.
  - Hover resolution runs on a background thread and does not block the UI.
- **Verification:** automated `XsHoverTest` + manual hover on `aiPlanCreate`.
- **Estimated changed lines:** ~180
- **Can stand alone?** yes — depends on 1.1.

### 1.5 Parameter info handler
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/parameterInfo/XsParameterInfoHandler.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/parameterInfo/XsParameterInfoTest.kt`
- **Spec reference:** `spec-parameter-info.md`
- **Acceptance:**
  - Caret inside `aiPlanSetVariableBool(planID, |)` bolds the second parameter.
  - Signature with types and names is shown for known syscalls.
  - Missing closing parenthesis is handled gracefully.
  - No popup for identifiers absent from `syscalls.json`.
- **Verification:** automated `XsParameterInfoTest` + manual inside `aiPlanAddUnitType(...)`.
- **Estimated changed lines:** ~220
- **Can stand alone?** yes — depends on 1.1.

### 1.6 Engine syscall go-to-definition stubs
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsEngineReferenceContributor.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsEngineStubGenerator.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsEngineNavigationTest.kt`
- **Spec reference:** `spec-workspace-navigation.md` (engine-stub portion)
- **Acceptance:**
  - `Ctrl+B` on any syscall name opens a generated in-memory `LightVirtualFile`.
  - Stub shows the syscall signature, parameters, and help text.
  - File tab title clearly indicates it is a generated documentation view.
- **Verification:** automated `XsEngineNavigationTest` + manual `Ctrl+B` on `aiPlanCreate`.
- **Estimated changed lines:** ~160
- **Can stand alone?** yes — depends on 1.1.

### 2.1 JFlex lexer
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsLexer.flex`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsLexerAdapter.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsTokenType.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsElementType.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/psi/XsLexerTest.kt`
- **Spec reference:** `spec-class-resolution.md` (foundation)
- **Acceptance:**
  - Lexer emits distinct token types for keywords, identifiers, numeric/string literals, operators, comments, and braces.
  - Generated lexer Java files live under `gen/` and are ignored by git.
- **Verification:** automated `XsLexerTest` (tokenize `minimal.xs`, assert counts/types).
- **Estimated changed lines:** ~180
- **Can stand alone?** yes — prerequisite for parser/PSI.

### 2.2 GrammarKit BNF grammar for top-level declarations
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsParser.bnf`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/psi/XsParsingTest.kt`
  - `tools/intellij-xs-plugin/src/test/testData/parser/TopLevel.xs`
  - `tools/intellij-xs-plugin/src/test/testData/parser/TopLevel.txt` (expected PSI tree)
- **Spec reference:** `spec-class-resolution.md`
- **Acceptance:**
  - Grammar parses function definitions, rule definitions, variable declarations, and class declarations.
  - Broken top-level declaration does not hide subsequent declarations (error recovery).
  - BNF produces a parser that compiles and passes fixture tests.
- **Verification:** automated `XsParsingTest`.
- **Estimated changed lines:** ~320
- **Can stand alone?** yes — depends on 2.1.

### 2.3 Parser definition + PSI element classes
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsParserDefinition.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsFile.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsNamedElement.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsFunctionDeclaration.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsRuleDeclaration.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsVariableDeclaration.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsClassDeclaration.kt`
- **Spec reference:** `spec-class-resolution.md`
- **Acceptance:**
  - `XsParserDefinition` is registered in `plugin.xml`.
  - Functions, rules, variables, and classes expose named `PsiElement`s.
  - `XsFile` exposes top-level declaration lists.
- **Verification:** fixture tests asserting named elements + `getName()` behavior.
- **Estimated changed lines:** ~280
- **Can stand alone?** yes — depends on 2.2.

### 2.4 Workspace reference contributor for top-level symbols
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsWorkspaceReferenceContributor.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsTopLevelReferenceProvider.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsWorkspaceGoToDefinitionTest.kt`
- **Spec reference:** `spec-workspace-navigation.md`
- **Acceptance:**
  - `Ctrl+B` on a workspace function name jumps to its declaration.
  - Same for rules, variables, and class type names.
  - Forward references (use before definition) resolve correctly.
  - Unknown identifiers do not open navigation and do not error.
- **Verification:** automated `XsWorkspaceGoToDefinitionTest` + manual `Ctrl+B` on `disableAutoScouting`.
- **Estimated changed lines:** ~240
- **Can stand alone?** yes — depends on 2.3.

### 2.5 Top-level workspace completion
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/completion/XsWorkspaceCompletionContributor.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/completion/XsWorkspaceCompletionTest.kt`
- **Spec reference:** `spec-class-resolution.md`, `spec-workspace-navigation.md`
- **Acceptance:**
  - Typing the prefix of a function or rule name proposes workspace variants.
  - Workspace variants do not shadow engine syscall variants (both are shown).
  - Items display a relevant icon/insert handler for functions vs rules.
- **Verification:** automated `XsWorkspaceCompletionTest` + manual in `human_assist.xs`.
- **Estimated changed lines:** ~180
- **Can stand alone?** yes — depends on 2.3.

### 3.1 Extend grammar/PSI for class body, methods, members
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsParser.bnf`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsClassBody.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsMethodDeclaration.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsMemberVariableDeclaration.kt`
  - Parser fixture tests
- **Spec reference:** `spec-class-resolution.md`
- **Acceptance:**
  - Class declarations expose methods and member variables as named PSI children.
  - Visibility/type modifiers are represented (without semantic validation).
  - Error recovery keeps sibling declarations visible after a malformed class.
- **Verification:** parser fixture tests + manual class fixture.
- **Estimated changed lines:** ~260
- **Can stand alone?** yes — depends on 2.3.

### 3.2 Member access resolution and references
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsMemberReferenceProvider.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsWorkspaceReferenceContributor.kt` (add member ranges)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsMemberNavigationTest.kt`
- **Spec reference:** `spec-class-resolution.md`, `spec-workspace-navigation.md`
- **Acceptance:**
  - `Ctrl+B` on `obj.member` resolves to the member declaration in the declared class of `obj`.
  - `Ctrl+B` on `obj.method(...)` resolves to the method declaration.
  - Resolution works across files for top-level class declarations.
- **Verification:** automated `XsMemberNavigationTest` + manual class fixture.
- **Estimated changed lines:** ~280
- **Can stand alone?** yes — depends on 3.1.

### 3.3 Member and method completion after `.`
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/completion/XsMemberCompletionContributor.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/completion/XsMemberCompletionTest.kt`
- **Spec reference:** `spec-class-resolution.md`
- **Acceptance:**
  - Typing `group.` after a variable of class type proposes members and methods of that class.
  - Inherited members are not required for v1.
  - No member completions appear for engine-syscall identifiers.
- **Verification:** automated `XsMemberCompletionTest` + manual class fixture.
- **Estimated changed lines:** ~200
- **Can stand alone?** yes — depends on 3.2.

### 3.4 Lambda, `ref`, and array PSI support
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsParser.bnf` (lambda, ref, array rules)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsLambdaType.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsRefType.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/psi/XsArrayType.kt`
  - Parser fixture tests
- **Spec reference:** `spec-class-resolution.md`
- **Acceptance:**
  - Lambda type syntax is parsed and represented as a callable PSI element.
  - `ref` and array modifiers (`int[]`, `ref float`) are represented on parameters and variables.
  - Identifiers inside these declarations remain named PSI elements.
- **Verification:** parser fixture tests + manual lambda fixture.
- **Estimated changed lines:** ~260
- **Can stand alone?** yes — depends on 2.3; partially independent of class work.

### 3.5 Hover for workspace symbols and members
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/documentation/XsDocumentationProvider.kt` (extended)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/documentation/XsWorkspaceHoverTest.kt`
- **Spec reference:** `spec-hover-docs.md` (extended), `spec-class-resolution.md`
- **Acceptance:**
  - Hovering over a workspace function shows its declaration signature.
  - Hovering over a class member or method shows declared type/signature.
  - Engine syscalls continue to hover correctly.
- **Verification:** automated `XsWorkspaceHoverTest` + manual class/lambda fixture.
- **Estimated changed lines:** ~160
- **Can stand alone?** yes — depends on 3.1 and 3.2.

### 4.1 Settings page for XML paths
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/XsSettingsState.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/settings/XsSettingsConfigurable.kt`
  - `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` (settings extension)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/settings/XsSettingsTest.kt`
- **Spec reference:** `spec-xml-constants.md`
- **Acceptance:**
  - Settings UI exposes five labeled file-chooser fields: proto, techtree, civs, cultures, powers.
  - Paths are persisted per project and restored on reopen.
  - Validation marks missing directories and does not crash.
- **Verification:** automated `XsSettingsTest` + manual open of Settings → Languages & Frameworks → XS Language.
- **Estimated changed lines:** ~180
- **Can stand alone?** yes — depends on P0.

### 4.2 XML scanner/parser with `*_mods.xml` overlay merging
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/constants/XsXmlConstantsService.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/constants/XsXmlConstants.kt` (model)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/constants/XsXmlConstantsServiceTest.kt`
  - `tools/intellij-xs-plugin/src/test/testData/xml/proto.xml`
  - `tools/intellij-xs-plugin/src/test/testData/xml/proto_mods.xml`
  - `tools/intellij-xs-plugin/src/test/testData/xml/techtree.xml`
  - `tools/intellij-xs-plugin/src/test/testData/xml/civs.xml`
  - `tools/intellij-xs-plugin/src/test/testData/xml/cultures.xml`
  - `tools/intellij-xs-plugin/src/test/testData/xml/powers.xml`
- **Spec reference:** `spec-xml-constants.md`
- **Acceptance:**
  - Recursive scan of configured directories finds `*.xml` files.
  - `*_mods.xml` files are merged additively with their base file.
  - Element/attribute names are normalized case-insensitively (`_name`, `Name`).
  - `<UnitType>` children of `<unit>` are emitted as additional `cUnitType*` constants.
  - Five prefix maps (`cUnitType`, `cTech`, `cCiv`, `cCulture`, `cProtoPower`) are built.
- **Verification:** automated `XsXmlConstantsServiceTest` + manual with real extracted XMLs.
- **Estimated changed lines:** ~320
- **Can stand alone?** yes — depends on 4.1.

### 4.3 AI plan constants completion contributor
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/completion/XsAiPlanCompletionContributor.kt`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/completion/XsAiPlanCompletionTest.kt`
  - `tools/intellij-xs-plugin/src/test/testData/aiplans/minimal.xs`
- **Spec reference:** `spec-aiplan-constants.md`
- **Acceptance:**
  - Typing `cPlan` proposes constants from `aiplans.json` filtered by prefix.
  - Lookup items display `variable_type` and `variable_value` as detail text.
  - `cPlan*` items do not appear for unrelated prefixes.
- **Verification:** automated `XsAiPlanCompletionTest` + manual in plan code.
- **Estimated changed lines:** ~120
- **Can stand alone?** yes — depends on 1.1.

### 4.4 Constant completion contributor + file-system refresh
- **Files touched:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/completion/XsConstantsCompletionContributor.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/constants/XsXmlConstantsService.kt` (watcher/invalidation)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/completion/XsXmlConstantsCompletionTest.kt`
- **Spec reference:** `spec-xml-constants.md`
- **Acceptance:**
  - Typing `cUnitTypeVill` proposes `cUnitTypeVillagerGreek` and matching XML-derived units.
  - Same for `cTech*`, `cCiv*`, `cCulture*`, `cProtoPower*`.
  - Changing a configured XML file refreshes the constant index without an IDE restart.
  - When no XML path is configured, XML-derived constants are not offered.
- **Verification:** automated `XsXmlConstantsCompletionTest` + manual real XML configuration.
- **Estimated changed lines:** ~180
- **Can stand alone?** yes — depends on 4.2 and 4.3.

### 5.1 Gradle task `regenerateSyscalls` (jsoup HTML parser)
- **Files touched:**
  - `tools/intellij-xs-plugin/build.gradle.kts` (task registration and jsoup dependency)
  - `tools/intellij-xs-plugin/scripts/regenerate-syscalls.gradle`
- **Spec reference:** `spec-regen-pipeline.md`
- **Acceptance:**
  - `./gradlew regenerateSyscalls` reads the relevant `docs/doxygen_retail/*.html` files.
  - Emits `src/main/resources/syscalls.json` matching the schema (`name`, `help`, `return_type`, `params`, `filename`).
  - Missing default-value tables produce empty default strings; malformed entries are skipped with a warning.
  - Deduplicates by syscall name and preserves array brackets/`void`.
  - Fails cleanly when `docs/doxygen_retail/` is missing.
- **Verification:** manual run + diff inspection against committed `syscalls.json`.
- **Estimated changed lines:** ~260
- **Can stand alone?** yes — depends on P0 Gradle skeleton.

### 5.2 Regeneration validation tests and procedure doc
- **Files touched:**
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/regen/RegenerateSyscallsTest.kt`
  - `tools/intellij-xs-plugin/README.md` (regeneration procedure)
- **Spec reference:** `spec-regen-pipeline.md`
- **Acceptance:**
  - Automated test runs the generator against committed `docs/doxygen_retail/` and asserts per-source-file counts match the distribution in `explore.md`.
  - README documents: run task → inspect diff → commit updated JSON.
  - P1 completion/hover tests still pass after regeneration.
- **Verification:** automated `RegenerateSyscallsTest` + `./gradlew test`.
- **Estimated changed lines:** ~120
- **Can stand alone?** yes — depends on 5.1.

---

## 3. PR forecast

### P0 — Scaffold

- **PR count:** 1
- **PR boundaries:** `P0-scaffold` contains project skeleton, manifest, file type, language icon, TextMate bundle provider, vendored grammar, CI guard.
- **Per-PR changed lines:** ~400 plugin/build lines; vendored `xs.tmLanguage.json` is reviewed but excluded from the 400-line mental budget.

### P1 — Engine API surface

- **PR count:** 4
- **PR boundaries:**
  - `P1a-engine-data` — vendored JSONs + `XsEngineApi`/`XsAiPlans` + index tests.
  - `P1b-engine-completion` — syscall-name completion + default-parameter completion + `XsSyscallCompletionTest`.
  - `P1c-engine-hover` — `XsDocumentationProvider` + `XsHoverTest`.
  - `P1d-engine-parameter-info` + `P1e-engine-stubs` — `XsParameterInfoHandler` and in-memory syscall stubs. *(If either slice nears 400 lines, split into two PRs.)*
- **Per-PR changed lines:** 120, 370, 180, 380 (hover excluded; stubs + parameter info combined if safe).

### P2 — Minimal PSI and workspace code intelligence

- **PR count:** 5
- **PR boundaries:**
  - `P2a-lexer` — JFlex lexer, token types, adapter, lexer tests.
  - `P2b-grammar` — Grammar-Kit BNF for top-level declarations + parsing fixture tests.
  - `P2c-parser-definition` — `XsParserDefinition`, `XsFile`, named PSI element classes.
  - `P2d-workspace-references` — reference contributor/provider for top-level symbols + go-to-definition tests.
  - `P2e-workspace-completion` — top-level workspace completion + tests.
- **Per-PR changed lines:** ~180, ~320, ~280, ~240, ~180.

### P3 — Class and lambda fidelity

- **PR count:** 5
- **PR boundaries:**
  - `P3a-class-grammar` — extend BNF/PSI for classes, methods, members.
  - `P3b-member-references` — member access resolution and navigation.
  - `P3c-member-completion` — dot-completion for members and methods.
  - `P3d-lambda-ref-array` — lambda/ref/array PSI support.
  - `P3e-workspace-hover` — hover for workspace symbols and members.
- **Per-PR changed lines:** ~260, ~280, ~200, ~260, ~160.

### P4 — XML-derived constants + AI plan constants

- **PR count:** 4
- **PR boundaries:**
  - `P4a-settings` — XML-path settings UI + persistence + validation.
  - `P4b-xml-parser` — recursive XML scanner/merger + test fixtures.
  - `P4c-aiplan-completion` — `cPlan*` completion from bundled `aiplans.json`.
  - `P4d-constants-completion` — XML-derived constant completion + file-system watcher.
- **Per-PR changed lines:** ~180, ~320, ~120, ~180.

### P5 — Regeneration pipeline

- **PR count:** 1 (or 2 if the Gradle implementation and validation docs diverge)
- **PR boundaries:** `P5-regen` contains the Gradle task, jsoup script, validation test, and README procedure.
- **Per-PR changed lines:** ~380.

### Total

P0 ~400 + P1 ~1,050 + P2 ~1,200 + P3 ~1,160 + P4 ~800 + P5 ~380 = **~4,990 changed plugin/build/test lines** (vendored JSON/TextMate files excluded from budget).  
Largest single PR: **~370 changed lines** (`P1b-engine-completion` or `P4b-xml-parser`).  
Phases with total >800 changed lines: **P1, P2, P3, P4**.

### Review Workload Forecast

- **Chained PRs recommended:** **yes** — every phase after P0 exceeds 800 total changed lines and several individual PRs sit near the 400-line soft limit.
- **400-line budget risk:** **medium** — the forecast keeps each PR ≤400, but parser-heavy PRs (`P2b-grammar`, `P4b-xml-parser`) are dense and could grow once edge-case tests are added.
- **Decision needed before apply:** **yes** — confirm the chained-PR strategy (feature-branch chain with a tracker vs. stacked PRs to `main`).
- **Recommendation:** **auto-chain per phase**, splitting any task that crosses 400 changed lines during implementation. P2 and P3 are the highest-risk phases; keep parser, PSI, references, and completion in separate PRs.

---

## 4. Apply batch boundaries

Default assumption: one apply batch per phase, but PR-aware splitting means some phases need multiple sequential batches.

```text
P0  [single batch]
    └── 0.1 → 0.2 → 0.3 → 0.4

P1  [split into 4 apply batches]
    ├── 1.1 (engine data layer)
    ├── 1.2 + 1.3 (engine completion + parameter defaults)
    ├── 1.4 (hover)
    └── 1.5 + 1.6 (parameter info + engine stubs)

P2  [split into 5 apply batches]
    ├── 2.1 (JFlex lexer)
    ├── 2.2 (GrammarKit BNF top-level grammar)
    ├── 2.3 (parser definition + PSI element classes)
    ├── 2.4 (workspace references)
    └── 2.5 (workspace completion)

P3  [split into 5 apply batches]
    ├── 3.1 (class PSI grammar)
    ├── 3.2 (member references)
    ├── 3.3 (member completion)
    ├── 3.4 (lambda/ref/array PSI)
    └── 3.5 (workspace hover)

P4  [split into 4 apply batches]
    ├── 4.1 (settings page)
    ├── 4.2 (XML scanner/merger)
    ├── 4.3 (AI plan constants completion)
    └── 4.4 (XML constants completion + file watcher)

P5  [single batch, or split into 5.1 then 5.2]
    └── 5.1 → 5.2
```

**Phases that should NOT be delivered as a single apply batch:** P1, P2, P3, P4. P0 and P5 may remain single batches if line counts hold, but P0 still ships as one batch because the scaffold pieces are tightly coupled (build, manifest, bundle, CI guard).

---

## 5. Roll-forward + rollback notes

### What "done" looks like per phase

- **P0 done:** `./gradlew runIde` opens an IDE; opening `human_assist.xs` shows XS syntax colors and the file is recognized as an XS file. Visual parity with VS Code is acceptable.
- **P1 done:** In `human_assist.xs`, engine syscalls complete, parameter defaults insert on `(`/`,`, hover shows signature + help, parameter info tracks active argument, and `Ctrl+B` on a syscall opens a generated stub.
- **P2 done:** `Ctrl+B` on `disableAutoScouting` jumps to its declaration; typing a workspace function/rule prefix offers completions. Fixture `minimal.xs` parses without crashing.
- **P3 done:** A fixture containing classes and lambdas supports member completion after `.`, member/method go-to-definition, and hover on local symbols.
- **P4 done:** Configured game XMLs drive `cUnitType*`, `cTech*`, `cCiv*`, `cCulture*`, `cProtoPower*` completion, and bundled `aiplans.json` drives `cPlan*` completion. File changes refresh results.
- **P5 done:** `./gradlew regenerateSyscalls` produces a diffable `syscalls.json`; counts match `explore.md` distribution; all P1 tests still pass.

### Rollback

- The plugin is isolated under `tools/intellij-xs-plugin/`. It does not touch `mod/` or any deployed mod file.
- No global IDE configuration, PATH, or environment variables are changed by the plugin code itself.
- **If the whole change must be reverted:** `git rm -rf tools/intellij-xs-plugin/` restores the repo to its pre-plugin state.
- **Vendored files** (`syscalls.json`, `aiplans.json`, `xs.tmLanguage.json`) are copies from `extracted/xs.vsix/`; they can be restored from the VSIX if needed.
- **Generated files** (Grammar-Kit/JFlex outputs under `gen/`) are gitignored; no manual cleanup is required beyond removing `gen/` if it was created locally.
- **If a single phase goes wrong:** revert the commits for that phase's batches. Later phases depend on earlier PSI/parser foundation, so reverting P2 requires reverting P3 as well. P1 and P4 are largely independent and can be rolled back separately if P0 remains.

---

## 6. Return contract

```json
{
  "status": "complete",
  "executive_summary": "Broke the IntelliJ XS plugin into six phases (P0–P5) and 23 implementation tasks, each mapped to a concrete spec, design section, file list, and acceptance criteria. The forecast recommends auto-chaining PRs because the total implementation is roughly 5,000 plugin/build/test lines and several phases exceed 800 changed lines; parser and XML work are the densest slices. P0 and P5 can remain single batches, while P1–P4 should be split into multiple PR-sized apply batches to respect the 400-line review budget.",
  "artifacts": ["openspec/changes/intellij-xs-plugin/tasks.md"],
  "review_workload_forecast": {
    "chained_prs_recommended": true,
    "four_hundred_line_budget_risk": "medium",
    "estimated_total_changed_lines": 4990,
    "largest_single_pr_estimate": 370,
    "decision_needed_before_apply": true
  },
  "delivery_strategy_recommendation": "auto-chain",
  "apply_batch_suggestion": "P0 → P1[1.1→1.2+1.3→1.4→1.5+1.6] → P2[2.1→2.2→2.3→2.4→2.5] → P3[3.1→3.2→3.3→3.4→3.5] → P4[4.1→4.2→4.3→4.4] → P5[5.1→5.2]",
  "risks": [
    "Grammar-Kit may struggle with XS lambdas, ref, arrays, and error recovery, requiring partial hand-rolled parser fallback.",
    "TextMate scope parity between IntelliJ IDEA/Rider and VS Code may need tuning after manual smoke tests.",
    "Doxygen HTML structure can drift across game patches, so the P5 regeneration script must be fault-tolerant."
  ],
  "skill_resolution": "paths-injected"
}
```
