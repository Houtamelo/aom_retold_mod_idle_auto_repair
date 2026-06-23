# Design: IntelliJ Platform XS Plugin (`intellij-xs-plugin`)

## 1. Architecture overview

Three isolated sides cooperate through bundled JSON/TextMate resources.

```
IDE side (plugin/)                Build side (scripts/)           Engine side (read-only)
─────────────────                 ──────────────────              ────────────────────
XsFileTypeFactory                 regenerate-syscalls.groovy      docs/doxygen_retail/
XsSyntaxHighlighterFactory        (Jsoup HTML → syscalls.json)    extracted/xs.vsix/ source
XsCompletionContributor                                           bundled syscalls.json
XsDocumentationProvider                                           bundled aiplans.json
XsParameterInfoHandler                                            bundled syntaxes/xs.tmLanguage.json
XsEngineReferenceContributor
XsWorkspaceReferenceContributor
XsXmlConstantsService
XsSettingsConfigurable
```

Flow:
1. The plugin ships bundled `syscalls.json`, `aiplans.json`, and `syntaxes/xs.tmLanguage.json`.
2. At startup it loads the two JSON indexes into memory.
3. `XsCompletionContributor`, `XsDocumentationProvider`, and `XsParameterInfoHandler` answer engine queries textually.
4. A JFlex/Grammar-Kit parser (§4) builds PSI for workspace symbols; reference contributors resolve them.
5. `XsXmlConstantsService` reads user-configured extracted XML directories and merges `*_mods.xml` overlays.

## 2. Project layout (concrete tree)

```
tools/intellij-xs-plugin/
├── build.gradle.kts          ← gradle-intellij-plugin config
├── settings.gradle.kts
├── gradle.properties
├── plugin.xml                ← plugin manifest
├── src/
│   ├── main/
│   │   ├── kotlin/com/aomr/xs/
│   │   │   ├── XsLanguage.kt
│   │   │   ├── XsFileType.kt
│   │   │   ├── XsFileTypeFactory.kt
│   │   │   ├── XsSyntaxHighlighterFactory.kt
│   │   │   ├── completion/XsCompletionContributor.kt
│   │   │   ├── documentation/XsDocumentationProvider.kt
│   │   │   ├── parameterInfo/XsParameterInfoHandler.kt
│   │   │   ├── navigation/
│   │   │   │   ├── XsEngineReferenceContributor.kt
│   │   │   │   └── XsWorkspaceReferenceContributor.kt
│   │   │   ├── psi/         ← XS lexer + parser
│   │   │   │   ├── XsLexer.flex   (Grammar-Kit JFlex grammar)
│   │   │   │   ├── XsParser.bnf   (Grammar-Kit BNF grammar)
│   │   │   │   └── psi/   ← PSI element classes
│   │   │   ├── constants/
│   │   │   │   ├── XsEngineApi.kt     ← loads bundled syscalls.json
│   │   │   │   ├── XsAiPlans.kt
│   │   │   │   └── XsXmlConstantsService.kt  ← parses user XMLs
│   │   │   └── settings/
│   │   │       └── XsSettingsConfigurable.kt
│   │   └── resources/
│   │       ├── META-INF/plugin.xml
│   │       ├── syntaxes/xs.tmLanguage.json  ← vendored from VSIX
│   │       ├── syscalls.json                ← vendored from VSIX
│   │       └── aiplans.json                 ← vendored from VSIX
│   └── test/
│       ├── kotlin/com/aomr/xs/   ← IntelliJ testFramework tests
│       └── testData/              ← fixture .xs files
├── scripts/
│   └── regenerate-syscalls.groovy ← doxygen → syscalls.json
├── inputs/
│   └── doxygen/                   ← drop fresh docs/doxygen_retail/ here
└── README.md
```

## 3. Tech stack pin

| Component | Version / Choice |
|---|---|
| Kotlin | 2.0.21 |
| IntelliJ Platform Gradle Plugin | `org.jetbrains.intellij.platform` 2.2.1 |
| Target platform | `IC-252` (IntelliJ IDEA 2025.2) |
| Minimum platform | `IC-242` (2024.2) |
| JBR / JDK | 21 |
| JFlex | 1.9.2 |
| Grammar-Kit Gradle plugin | 2022.3.2 |
| jsoup | 1.18.3 |
| TextMate bundle plugin | `org.jetbrains.plugins.textmate` (bundled) |

## 4. Parser strategy ADR

- **Decision**: **A** — Grammar-Kit (JFlex + BNF → IntelliJ PSI).
- **Rationale**: XS is C-like with a manageable grammar; Grammar-Kit gives us auto-generated PSI, parser, and parser-debug tooling for free; the `.bnf` doubles as documentation. This is the canonical IntelliJ path and keeps reference/completion implementations aligned with platform expectations.
- **Trade-offs**: Less control over error recovery — we MUST define a recovery strategy in the BNF. JFlex generates files we will not commit.
- **Alternatives considered**: **B** (hand-rolled recursive-descent + manual PSI builder) was rejected because hand-rolled PSI builders are a known footgun for IntelliJ plugins; the grammar is small enough that Grammar-Kit's overhead is justified. If lambdas, `ref`, or arrays prove unwieldy, we may downgrade only the expression sub-grammar while keeping Grammar-Kit for declarations.

## 5. Vendor-vs-reference strategy

`syntaxes/xs.tmLanguage.json`, `syscalls.json`, and `aiplans.json` are **vendored** as plugin resources (Apache 2.0 from the VSIX) in `src/main/resources/`.

Regeneration flow:
1. Drop a fresh `docs/doxygen_retail/` into `tools/intellij-xs-plugin/inputs/doxygen/`.
2. Run `./gradlew regenerateSyscalls` (Groovy + jsoup; parses the relevant HTML files).
3. Diff the emitted `src/main/resources/syscalls.json` against the committed one.
4. Commit the regenerated JSON when the diff is acceptable.

`aiplans.json` follows the same flow when the relevant doxygen page changes.

## 6. Stub-generation ADR

- **Decision**: Use IntelliJ `LightVirtualFile` for engine syscall stubs.
- **Rationale**: No on-disk pollution; preserves the user's project tree; works identically in Rider and IDEA; `LightVirtualFile` is the standard pattern for generated content in IntelliJ.
- **Trade-offs**: Must be regenerated on each navigation request. Mitigate through an in-memory `name → stub text` cache keyed by syscall name. Stub text contains the signature, docstring, and parameter list; it is not executable XS.

## 7. State machines

### 7.1 Completion provider

```
Idle ──[ typing / Ctrl+Space ]──► Prefix lookup
Prefix lookup ──[ matches engine prefix ]──► Filter syscalls ──► Present LookupElements
Prefix lookup ──[ matches workspace prefix ]──► StubIndex query ──► Present variants
Prefix lookup ──[ identifier before '(' or ',' ]──► Parameter default lookup ──► Present default value
```

### 7.2 Workspace go-to-definition

```
Cursor on identifier ──► resolveReference()
  ├─ engine name?      ──► create LightVirtualFile stub ──► return stub PsiElement
  ├─ workspace symbol? ──► resolve to declaration PSI
  └─ neither           ──► no navigation
```

### 7.3 XML constants service

```
Settings changed / XML file changed ──► Scan directories
Scan directories ──► Parse *.xml + merge *_mods.xml overlays
Parse + merge ──► Build constants map ──► Cache by file path
Cache updated ──► Notify completion provider refresh
```

## 8. Performance & caching

- Engine API: ~1,805 syscalls + 193 AI-plan constants. Loaded once at plugin startup into immutable name-keyed maps.
- XML constants: indexed by resolved file path; invalidated on settings change or file-system change.
- PSI stubs: cached by `file URL + identifier` through `CachedValuesManager`.
- Workspace symbols: indexed via IntelliJ `StubIndex` for O(1) top-level lookups.
- Completion items: pre-built `LookupElementBuilder` lists at load time; platform prefix filtering avoids per-keypress allocation.

## 9. Risks & mitigations

| # | Risk | Mitigation |
|---|---|---|
| 1 | Grammar-Kit BNF → Java produces many generated files. | Add generated `gen/` directories to `.gitignore`; keep only `XsLexer.flex` and `XsParser.bnf` in source control. |
| 2 | Rider's plugin model differs slightly from IDEA's. | Add a Rider smoke-test gate before finishing P0 and P1. |
| 3 | TextMate scope mapping may not match VS Code. | Provide a scope-inspector editor action for P0 side-by-side verification. |
| 4 | Engine data drifts across game patches. | Deliver P5 regen pipeline and enforce the diff-then-commit workflow. |
| 5 | Doxygen HTML has non-uniform defaults. | Regen script logs warnings and skips malformed entries instead of failing hard. |

## 10. Cross-mod isolation

This plugin never reads or writes anything under `mod/`. All XS source files are loaded read-only through IntelliJ's standard virtual-file API. No mod overlay is touched — including shared files like `auto_repair.xs` and `auto_scout.xs` and any `human_assist.xs`. Deleting `tools/intellij-xs-plugin/` reverts the repository to its pre-plugin state with no deployable-mod impact.

## 11. Return contract

```json
{
  "status": "complete",
  "executive_summary": "Technical design for an IntelliJ Platform XS plugin under tools/intellij-xs-plugin/. The design pins a Grammar-Kit-first parser, bundled JSON/TextMate resources, in-memory LightVirtualFile syscall stubs, XML-derived constants with *_mods.xml merging, and a doxygen-to-JSON regeneration pipeline. It documents state machines, caching, cross-mod isolation, and the top design-level risks.",
  "artifacts": ["openspec/changes/intellij-xs-plugin/design.md"],
  "next_recommended": "sdd-tasks",
  "risks": [
    "Grammar-Kit error recovery for XS lambdas/ref/arrays may require a partial hand-rolled parser fallback.",
    "Rider-specific TextMate/completion behavior must be smoke-tested to avoid regressions.",
    "Doxygen HTML structure drift can break the regeneration script and must be tolerated with warnings."
  ],
  "skill_resolution": "paths-injected"
}
```
