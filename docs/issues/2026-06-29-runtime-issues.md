# Runtime Issues Reported 2026-06-29

| Field         | Value                                                              |
| ------------- | ------------------------------------------------------------------ |
| Reporter      | user (manual smoke test in a real mod project)                     |
| Date          | 2026-06-29                                                         |
| Plugin tested | `0.2.1` (`dist/intellij-xs-plugin-0.2.1.zip`)                      |
| Status        | Issue 1 **resolved** by `fix-plugin-goto-definition-ctrl-click-keybind`; Issue 2 **resolved** by `fix-lsp-false-positive-forward-decl`; Issue 3 **resolved** by `expand-color-scheme-categories`; Issue 4 **resolved** by `openspec/changes/archive/2026-06-29-fix-goto-definition-on-include-statements/` |
| Resolution    | Plugin `0.2.3` registers `XsGotoDeclarationHandler` forwarding Ctrl+Click / `Ctrl+B` to the XS LSP server |

## Context

These four issues were discovered while the user was manually testing the
plugin in `mod/spire_ai/` (a real mod project that overlays the vanilla
AoM:R `game/ai/` tree). They are surfaced here before any action so the
team can decide whether to bundle them into one SDD change, split them
into four, or schedule them into the existing audit-driven batch plan.

## Note on the reported plugin version

The user is reporting against **0.2.1**, but the current `pluginVersion`
in `tools/intellij-xs-plugin/gradle.properties` is **0.2.2**. Branch state
at the time of the report:

```
19c1c2a chore(xs-plugin): bump pluginVersion to 0.2.2
2ed7f9c fix(xs-plugin): remove 'no mod folders detected' startup notification
06fa30b chore(xs-plugin): bump pluginVersion to 0.2.1
27966bf fix(xs-lsp): parse JSON-RPC frames in roundtrip test (R5-F-01/02/03)
8749b46 fix(xs-lsp): scope lock guards in did_close to eliminate cyclic hazard
```

The diff between 0.2.1 and 0.2.2 only removes the
`notifyNoModsDetected` startup balloon — it does not affect LSP
behaviour, goto-definition wiring, syntax highlighting, or color
schemes. None of the four issues below are gated on which of the two
builds is installed; they reproduce on either.

---

## Issue 1 — Go-to-definition only works via right-click menu

### Status
**Resolved 2026-06-29** by `openspec/changes/archive/2026-06-29-fix-plugin-goto-definition-ctrl-click-keybind/`. The plugin now registers a `GotoDeclarationHandler` (`com.aomr.xs.navigation.XsGotoDeclarationHandler`) that forwards Ctrl+Click and the go-to-definition keybind (`Ctrl+B` / `⌘B`) to the active XS LSP server via `textDocument/definition`. Right-click and hover paths remain unchanged. See the change directory for the spec, design, and verify report.

### Steps to reproduce
1. Open any `.xs` file in Rider with the plugin installed.
2. Place the caret on a symbol whose definition is resolvable (e.g. a
   user-defined function, an `extern`, an engine API call).
3. Try each of the following goto triggers:

| Trigger                                            | Result                                    |
| -------------------------------------------------- | ----------------------------------------- |
| **Right-click** symbol → Go to → **Declarations or usages** | **Works**                          |
| **Ctrl+Click** on the symbol                       | Does nothing (no navigation, no popup)   |
| **Go-to-definition keybind** with caret on symbol  | Does nothing                              |
| **Mouse hover** over the symbol                    | **Works** — popup shows correct info: `function: {func_name}\nDefined in: {file_path}` |

### Expected
All four entry points should navigate to the symbol's definition. The
right-click menu is the only one that works today.

### Why this is suspicious
The fact that hover and right-click both work means the LSP
`textDocument/definition` endpoint is reachable **and** returns a valid
response (the hover pipeline has to consume the same resolution data).
What is broken is the IntelliJ Platform's **client-side wiring** for two
of the three navigation entry points: Ctrl+Click and the keybind.

### Suspected root cause
Likely a missing or misconfigured `GotoDeclarationHandler` (or equivalent
in the IntelliJ Platform LSP client wiring) that is invoked by the
right-click menu but not by the editor's mouse-click or keyboard
shortcut paths. Less likely: the LSP `ServerCapabilities`
`definitionProvider` is being advertised correctly but the platform's
LSP `GotoDeclarationHandler` is registered with the wrong scope (file,
not project).

### Affected files (to investigate)
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/`
  - search for any existing `GotoDeclarationHandler` registration
  - confirm the platform's `Lsp4jClient` is wired to the editor's goto
    action (it should be free under the platform LSP client — see Bug #1
    in `docs/post-lsp-migration-issues.md`)
- `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`
  - confirm the `gotoDeclarationHandler` extension point is registered

### Cross-reference
This was *not* one of the 145 audit findings in
`docs/code-reviews/2026-06-29-lsp-and-plugin-review.md`. The audit was
static-only (read-the-code), so runtime UX bugs like this slipped
through. Worth keeping in mind: the audit is **not** a substitute for
manual testing.

---

## Issue 2 — False-positive "used before declaration" on unmodified game files

### Status
Resolved 2026-06-29 by `openspec/changes/archive/2026-06-29-fix-lsp-false-positive-forward-decl/`.
No forward-declaration changes to the user's project can make these
diagnostics disappear (and there shouldn't be any — the files are
unmodified).

### Steps to reproduce
1. Copy the vanilla game's entire `ai/` folder into `mod/spire_ai/game/`:
   ```
   cp -r <AOMR>/game/ai mod/spire_ai/game/
   ```
2. Open `mod/spire_ai/game/ai/core/main.xs` in the editor.
3. The LSP reports the following diagnostics:

```
'setupDebugCategories' used at line 13 before declaration; add forward declaration or mark 'mutable'
'debugStartAnalysis'    used at line 15 before declaration; add forward declaration or mark 'mutable'
'debugStartAnalysis'    used at line 16 before declaration; add forward declaration or mark 'mutable'
'initCivUnitTypes'      used at line 19 before declaration; add forward declaration or mark 'mutable'
'initArrays'            used at line 22 before declaration; add forward declaration or mark 'mutable'
'debugStartAnalysis'    used at line 30 before declaration; add forward declaration or mark 'mutable'
'doStartupFlow'         used at line 37 before declaration; add forward declaration or mark 'mutable'
'initXSHandlers'        used at line 40 before declaration; add forward declaration or mark 'mutable'
'prepareForInit'        used at line 53 before declaration; add forward declaration or mark 'mutable'
```

### Why these are false positives
`game/ai/core/main.xs` line 2 contains an `include` directive that pulls
in the file (or files) where each of these functions is defined. For
example, `setupDebugCategories` is declared (and defined) inside one of
the included files. The user's note:

> Since these are unmodified files from the base game, they should
> always compile by-default when being copied over to a mod project.

This is correct. The vanilla `main.xs` is known to compile as-is. So
the LSP's forward-declaration check is producing false positives
because it is **not following the include chain** for these files.

### Suspected root cause
The LSP's "used before declaration" check is operating on the local
file's symbol table only. It looks at the call site, doesn't find a
matching definition in the current file, and emits a diagnostic — even
when the definition is reachable via `include`.

Plausible places to inspect:
- `tools/xs-language-server/src/semantic.rs` — symbol resolution and
  the diagnostic emitter for "used before declaration"
- `tools/xs-language-server/src/merged_view.rs` — include-graph
  construction and transitive-symbol resolution
- `tools/xs-language-server/src/workspace.rs` (or equivalent) — how
  `Workspace::resolve_symbol` walks the include chain

There is also a separate (related) question: when the user copies the
vanilla `game/ai/` tree into the mod project, does the LSP correctly
resolve symbols across **both** the mod's `game/ai/` overlay and the
underlying vanilla `game/ai/`? If not, the include target might be
unresolved even before the forward-decl check runs.

### Important distinction (NOT a false positive of the same kind)
The audit and `docs/post-lsp-migration-issues.md` Category D describe
**real** forward-declaration errors in user code (`updateBreakdown`,
`applyDistribution` in `human_assist.xs`). Those errors are correct —
XS genuinely requires explicit forward declarations for user-defined
functions. The errors reported here are categorically different:
the user is **not** writing XS code, they are copying unmodified
vanilla `.xs` files that the engine accepts without modification.

### Affected files (to investigate)
- `tools/xs-language-server/src/semantic.rs` (forward-declaration check)
- `tools/xs-language-server/src/merged_view.rs` (include-graph merge)
- `tools/xs-language-server/src/workspace.rs` (cross-folder resolution)
- Any include-path discovery configuration that the LSP uses for mod
  projects (the `mod/` folder overlay against the vanilla `game/` folder)

### Cross-reference
Likely *not* an audit finding. The audit reviewers looked at code paths
in isolation, not at a freshly-imported mod project. The closest related
finding is the include-resolution discussion in `R3-F-04` (engine-API
refs vs merged-view walk disagree), but that one is about engine API
symbols, not user-defined functions in includes.

---

## Issue 3 — Color scheme expansion (UX improvement)

### Status
**Resolved 2026-06-29** by `openspec/changes/archive/2026-06-29-expand-color-scheme-categories/`. The plugin now exposes the inherited Rider General / Language Defaults categories under **Settings → Editor → Color Scheme → XS** and the page label is uppercase **XS**. The semantic-token-driven categories (engine/modded/unmodded functions, variables, constants, types, classes, and built-in type as a dedicated semantic category) remain deferred to a future LSP-side change.

### Steps to reproduce
1. Open Rider settings: **Editor → Color Scheme → xs**.
2. Observe that the only categories available are:
   - Comment
   - Default
   - Identifier
   - Keyword
   - Number
   - String
3. Also note: the menu label says **`xs`** (lowercase) — should be
   **`XS`**.

### Expected
The color-scheme menu should expose the categories listed below.

### Proposed category taxonomy

#### Identifier / Function
- Engine function
- UnModded function
- Modded function
- UnModded Rule (rules can be invoked as functions)
- Modded Rule

#### Identifier / Variable
- Local variable
- Static variable
- `extern` unmodded variable
- `extern` modded variable

#### Identifier / Constant
- Engine constant
- UnModded constant
- Modded constant

#### Identifier / Type
- Built-in type (`bool`, `int`, `float`, `string`, `vector`)
- UnModded class
- Modded class

### Glossary
- **`non-modded`** (a.k.a. **unmodded**) — defined in a non-overridden
  `.xs` file, i.e. a file present in the game's official scripts but
  *not* in the mod's project overlay.
- **`modded`** — defined in an overridden `.xs` file, i.e. a file
  present in the mod's project.

### Why the engine/modded/unmodded distinction matters
- **Clearly showcases boundaries.** A glance at the colors tells the
  user "this is engine code (don't expect to rename it)", "this is
  vanilla (renaming would force-copy the file into the mod)", or "this
  is mod code (renaming is safe)".
- **Operation semantics differ by category.** Some operations
  (rename, refactor) are not supported on engine symbols. Others
  (rename) are intentionally **not** supported on unmodded symbols,
  because renaming an unmodded symbol would require copying the entire
  unmodded file into the mod (converting it to modded). **Go-to-definition
  is and will remain supported for unmodded symbols** — only rename is
  blocked.

### Inherited categories from Rider

The user noted that Rider has a **Color Scheme → General** tab with
shared categories that many languages inherit. XS currently bundles
all of these into the **Default** category (regular text). Worth
inheriting:

- Code / **Identifier under caret** (Rider default: background-color
  emphasis)
- Code / **Matched brace** (Rider default: background-color emphasis)
- Code / **Unmatched brace** (Rider default: red background — error
  indicator)
- Errors and Warnings / **Unknown symbol** (Rider default: red
  background — error indicator)

And **Color Scheme → Language Defaults** has
**Braces and Operators** categories we could expose:

- Braces
- Brackets
- Comma
- Dot
- Operation sign
- Overloaded operator
- Parentheses
- Semi-colon

### Affected files (to investigate)
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/` — wherever
  the color settings page / `TextAttributesKey`s are registered today
- `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml` —
  extension points for `colorSettingsPage` and `textAttributes`
- `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`
  — wire the new scopes / names into the TextMate grammar
- Any Kotlin file that consumes semantic-highlight ranges from the LSP
  and translates them into `TextAttributesKey` selections

### Cross-reference
Likely *not* an audit finding. This is purely UX feedback from
real-world plugin use.

---

## Issue 4 — Go-to-definition on include statements

### Status
Resolved 2026-06-29 by `openspec/changes/archive/2026-06-29-fix-goto-definition-on-include-statements/`. Include-path go-to-definition now resolves through the LSP's `Workspace` pipeline and is forwarded unchanged by the existing `XsGotoDeclarationHandler`.

### Steps to reproduce
1. Open any `.xs` file that contains an `include` directive at the
   top of the file (e.g. `mod/spire_ai/game/ai/core/main.xs`).
2. Place the caret on the `include` directive (e.g. on the file path
   token).
3. Activate Go-to-definition via **any** of the triggers (Ctrl+Click,
   keybind, or right-click → Go to → Definition).
4. Observe: nothing happens.

### Expected
The included file should open in the editor (same behavior as a
"jump to file" action). Ctrl+Click should highlight the include token
as clickable (typical platform behavior for `psiElement -> navigation`
targets).

### Why this is a separate issue from #1
Issue #1 is about Ctrl+Click and the keybind not working on regular
symbols (where right-click **does** work). Issue #4 is about **all
three** goto triggers failing on include statements, including
right-click. So Issue #4 is **independent**: even if Issue #1 is
fully fixed, Issue #4 will remain.

### Suspected root cause
Two possibilities, possibly compounding:

1. **Server side**: the LSP's `textDocument/definition` returns
   `null` (or an empty list) when the cursor is on the path token of
   an `include` directive. The server may not treat include directives
   as navigation targets at all.

2. **Client side**: even if the server returned the include target
   correctly, the plugin would need to translate the response into an
   IntelliJ `NavigatablePsiElement` and surface it through the
   `GotoDeclarationHandler` registered for Issue #1.

   In practice, includes are a **non-XS** navigation target (they're
   preprocessor directives, not XS statements). They might be served
   from a different code path than the regular goto-definition.

### Likely implementation shape
- On the LSP server: when the cursor position is on a `string_literal`
  (or path token) inside an `include_directive`, return the URI of
  the resolved include target as the `Location` response. Resolution
  should respect the same `Workspace` rules as semantic resolution
  (mod overlay + vanilla `game/` fallback).
- On the IntelliJ plugin: register a second goto-declaration handler
  (or extend the existing one) that detects include directives and
  produces an `OpenFileDescriptor` navigation. Make the include
  token's text attribute "navigatable" so Ctrl+Click shows the
  underline.

### Affected files (to investigate)
- `tools/xs-language-server/src/` — wherever `textDocument/definition`
  is handled (likely `server.rs` or a dedicated `definition.rs` /
  `goto.rs`)
- `tools/xs-language-server/src/workspace.rs` (or equivalent) —
  include-path resolution
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/` —
  `GotoDeclarationHandler` (or equivalent)
- The plugin side also needs to surface a clickable include token.

### Cross-reference
Likely *not* an audit finding.

---

## Bundle vs split: recommended SDD sequencing

| Option                                                                                  | Pros                                              | Cons                                                          |
| --------------------------------------------------------------------------------------- | ------------------------------------------------- | ------------------------------------------------------------- |
| **Single SDD change** covering all 4 issues                                              | One commit, one review, single round-trip         | Larger LOC delta, mixed server + client changes, harder to roll back |
| **Split per issue** (4 SDD changes)                                                      | Smallest review slices, easy to revert            | 4× the SDD ceremony overhead                                  |
| **Split by side**: one server-side change (issues 2 + 4) + one plugin-side change (issues 1 + 3) | Echoes the audit's batches A/B style, server-side changes can be tested via `cargo test` without IDE | Plugin-side changes need IntelliJ infrastructure review |

Recommendation: **split per issue** (4 SDD changes), starting with Issue
2 (false-positive forward-declaration diagnostics — most likely to
generate LSP churn that affects other batches).

The user has the final call.

## Cross-reference against the audit

A quick overlap check before action:

| Issue | Overlap with `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` | Notes                                                                                |
| ----- | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 1     | None confirmed (audit was static-only)                                | Runtime UX bug, missed by static analysis                                            |
| 2     | Possible link to **R3-F-04** (engine-API refs vs merged-view walk)    | R3-F-04 is about engine symbols, Issue 2 is about user-defined symbols via includes  |
| 3     | None confirmed                                                        | Pure UX / color-scheme plugin work                                                   |
| 4     | None confirmed                                                        | Server + client feature work, not flagged by static analysis                          |

When each fix lands, mark the corresponding audit line with
`[Resolved 2026-06-29]` / `[Refuted 2026-06-29]` as we did for R3-F-01,
R5-F-01/02/03, and R1-F-01/02/03.

## Related Files

- `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` — full audit report (905 lines)
- `docs/code-reviews/2026-06-29-lsp-and-plugin-findings.csv` — 141 sortable audit rows
- `docs/post-lsp-migration-issues.md` — prior issues doc from 0.1.5/0.1.6 era (Categories A–D resolved)
- `docs/xs-language-syntax.md` — XS language reference (for understanding forward declarations)
- `AGENTS.md` — "Forward declarations are required" rule (relevant to Issue 2)
- `tools/xs-language-server/` — LSP server source (Issues 2 and 4)
- `tools/intellij-xs-plugin/` — IntelliJ plugin source (Issues 1, 3, 4)

---

## Rider smoke test findings (2026-06-30)

After Issues 1–4 were fixed and the plugin was built at
`dist/intellij-xs-plugin-0.4.0.zip`, the user installed it in Rider and
ran a manual smoke test. The following findings emerged. All four of
the original issues are confirmed fixed (Issues 1, 2, 4 verified
directly; Issue 3 only **partially** — see "Issue 3 sub-findings"
below).

### Bucket C (engine / modded / unmodded function, local / static variable, builtin type, class)

Status: Resolved 2026-06-30 across two slices.

- **LSP side (plugin 0.5.0)**: `add-lsp-semantic-tokens` slice
  (`openspec/changes/add-lsp-semantic-tokens/`, commit `874dae9`)
  adds `textDocument/semanticTokens/full` capability, the
  legend (3 token types × 5 modifiers), and the emission logic
  in `tools/xs-language-server/src/semantic_tokens.rs`. Six
  strict-TDD integration tests in
  `tools/xs-language-server/tests/semantic_tokens_repro.rs`
  cover legend advertisement + engine / modded / unmodded
  function classification + local-variable extraction +
  builtin-type classification.

- **Plugin side (plugin 0.6.0)**: `add-plugin-semantic-tokens`
  slice (`openspec/changes/add-plugin-semantic-tokens/`, this
  commit) declares 8 new `TextAttributesKey`s, registers 8
  new `AttributesDescriptor`s, and provides the
  `XsSemanticTokensConverter` (Kotlin) that maps each LSP
  semantic token to a color. The platform's auto-wiring of
  LSP semantic tokens to the editor's highlighter is
  sufficient for the colors to appear in `.xs` files.

The remaining Bucket C work (class extraction, constant / rule
classification, full extern distinction) is **resolved 2026-06-30** by
`openspec/changes/finish-semantic-token-distinctions/` (plugin `0.7.0`).

### Issue 3 sub-findings (Rider smoke test)

The Option α implementation of Issue #3 (rename "xs" → "XS" + 12
inherited Rider categories) shipped in commit `a370e78`. Bucket C
(Engine/Modded/UnModded function/variable/constant/type, Local,
Static, extern, Built-in type, Class) was deferred to a follow-up
change named `expand-color-scheme-semantic-tokens`. The smoke test
also surfaced three additional bugs in the Option α implementation
that should be addressed in the same follow-up:

**Issue 3a — `include` is not being rendered as a keyword.**
- **Status**: Resolved 2026-06-30 by `openspec/changes/archive/2026-06-29-fix-color-scheme-quick-wins/`.
  `include` is now added to both the TextMate grammar keyword regex
  (`xs.tmLanguage.json`) and the native highlighter's keyword set
  (`XsHighlightingLexer.kt`).
- Reproducer: open any `.xs` file. Place caret on `include`.
- Expected: the word `include` is colored as a keyword.
- Actual: `include` is colored as `Default` (regular text).
- Root cause: the TextMate grammar (`syntaxes/xs.tmLanguage.json`) does
  not scope `include` as a `keyword.control.include` (or similar)
  scope. The existing keyword list probably doesn't include it.
- Affected file: `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`
- Severity: low (visual nit, but should be obvious to users since
  `include` is one of the most common XS keywords).

**Issue 3b — Braces and operators are using the `Default` color, not
their respective colors.**
- Reproducer: open any `.xs` file. The braces (`{}`), brackets (`[]`),
  parens (`()`), comma, dot, etc. are all rendered in the color
  assigned to `Default`, not in the colors the user picked for the
  Braces and Operators subcategory.
- Expected: braces use the Braces color the user picked; brackets
  use the Brackets color; etc.
- Actual: all use `Default`.
- Root cause: the Option α implementation only **declared** the 12
  `TextAttributesKey`s in `XsTextAttributes.kt` and registered them
  in `XsColorSettingsPage.kt`, but did **not** wire the
  `XsSyntaxHighlighter` (or the TextMate semantic-mapping
  configuration) to actually map lexer tokens to those keys. So
  the colors are visible in settings but not applied at runtime.
- Affected files:
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlighterFactory.kt` (if separate)
  - Possibly `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` (semantic mappings)
- Severity: medium (the whole point of the color settings is to
  actually see the colors in the editor).
- **Status**: Resolved 2026-06-30 by `openspec/changes/archive/2026-06-29-fix-color-scheme-quick-wins/`.
  `XsSyntaxHighlighter.kt` now maps each of the 8 inherited tokens
  (BRACES, BRACKETS, COMMA, DOT, OPERATION_SIGN, OVERLOADED_OPERATOR,
  PARENTHESES, SEMI_COLON) to its corresponding `XsTextAttributes`
  key. Three new token types (DOT, SEMI_COLON, OPERATION_SIGN) added
  to `XsTokenType.kt` with matching lexer rules in `XsLexer.flex`.

**Issue 3c — Identifier under caret is not working.**
- Reproducer: open any `.xs` file. Place caret inside any identifier
  (e.g. on a function name, a variable name). No background-color
  change occurs.
- Expected: the word under the caret gets a background-color
  emphasis (per the user's color choice in the settings).
- Actual: no change.
- Status: `Matched & Unmatched braces` work properly; `Keyword`,
  `Number`, `String`, `Comment` all work properly; only
  `Identifier under caret` is broken.
- Root cause: unclear. The Option α implementation declared
  `XS_IDENTIFIER_UNDER_CARET` with fallback
  `CodeInsightColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES`. Possible
  reasons this doesn't work:
  1. Rider's "Identifier under caret" is sourced from the global
     color scheme, not the per-language one; the per-language
     override is ignored.
  2. The fallback constant name is wrong (maybe it should be
     `CodeInsightColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES` or
     `EditorColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES` — verify in
     2024.2 platform sources).
  3. The platform's IdentifierUnderCaretPass requires explicit
     registration beyond just declaring the TextAttributesKey.
- Affected file: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt`
  (the fallback key).
- Severity: low (cosmetic; not a blocker for any real workflow).
- **Investigate during the follow-up change** (do not assume the
  Option α fallback is correct).
- **Status**: Resolved 2026-06-30 by `openspec/changes/archive/2026-06-29-fix-color-scheme-quick-wins/`.
  Investigation confirmed the per-language override is not supported
  in Rider; the workaround is to customize under the **Color Scheme →
  General** scheme. The `XsColorSettingsPage` descriptor for the
  Identifier-under-caret category was updated to include
  "(uses global General — per-language override is not supported in
  Rider)" so users see this limitation when they customize the
  category. The fallback constant `EditorColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES`
  is correct.

### Cross-reference against the audit

A quick overlap check before action:

| Sub-issue | Overlap with the audit                                                                 | Notes                                                                                |
| --------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 3a (include keyword) | None confirmed                                                              | Grammar-scope fix; the audit was static-only                                          |
| 3b (braces/operators) | Possibly related to Category E in `docs/post-lsp-migration-issues.md` (no syntax highlighting) | Different bug — Categories are declared but not bound; that doc's E was a stub PSI fix |
| 3c (identifier under caret) | None confirmed                                                      | Rider-specific behavior; possibly not fixable from the plugin                          |

### Recommended follow-up change

- **Name**: `expand-color-scheme-semantic-tokens`
- **Status**: split into three smaller changes (2026-06-30):
  - `fix-color-scheme-quick-wins` (3a/3b/3c): committed
  - `add-lsp-semantic-tokens` (Bucket C LSP): pending
  - `add-plugin-semantic-tokens` (Bucket C Plugin): pending
- **Scope**:
  - The original Bucket C from Issue #3: add LSP `textDocument/semanticTokens` capability; emit semantic-token kinds + origin modifier (engine / modded / unmodded); extract local variables and classes in the LSP symbol table; add a plugin-side semantic-token-to-TextAttributesKey converter; declare and bind the 14+ new categories (Engine/Modded/UnModded function, Local/Static/extern variable, Engine/Modded/UnModded constant, Built-in type, UnModded/Modded class).
  - 3a: extend the TextMate grammar to scope `include` as a keyword.
  - 3b: bind the `XsSyntaxHighlighter` to the 12 inherited TextAttributesKeys so braces/operators actually use their colors.
  - 3c: investigate the Rider Identifier-under-caret behavior; if the per-language override is supported, ensure the fallback key is correct; if not, document the limitation and surface a clear message in the color settings.
- **Risk**: medium-high (LSP semantic tokens + local-var/class extraction are non-trivial).
- **Effort**: ~1-2 weeks.
- **pluginVersion bump**: this adds **new LSP features** + **new settings UI** + **new grammar scope** → MINOR bump (per AGENTS.md table). 0.4.0 → 0.5.0.

## Known limitations (deferred to future specs)

### Cross-file forward-declaration ordering

**Status**: known limitation of the Issue #2 fix (`0491693`).

**Scenario**:
```
// file main.xs (the root, included by the game engine)
include "A.xs"
include "B.xs"

// file A.xs
void X() { ... }

// file B.xs
X();  // call site
```

The game engine accepts this because `A` is included before `B`, so
`X` is defined (in `A`) before being used (in `B`).

**The Issue #2 fix only handles the "single file" case.** Specifically:
- Forward declarations within a single file: works (basic line-order check).
- Function defined in `main.xs`, used in an included file: works (the
  call site's `effective_line` is computed against the includer
  file's include line, and the definition's `effective_line` is the
  line in `main.xs` where it's defined).
- Function defined in an included file, used in `main.xs`: works
  (the Issue #2 fix set the definition's `effective_line` to the
  current-file include line).

**What does NOT work** (and is not handled by the Issue #2 fix):
- Function defined in an included file `A`, used in another included
  file `B`, where both `A` and `B` are included from `main.xs`. The
  current LSP doesn't know that `A` is included before `B`, so it
  can't tell that `A`'s definitions come "before" `B`'s uses from
  the perspective of `main.xs`.

**Why this is hard to fix properly**:
- The LSP would need to compute, for every pair of files, the
  "include order" of those files relative to any common includer.
- This is a global property of the include graph, not a local
  property of the open file.
- It's also recursive: `A` itself might be included from
  multiple places, and each inclusion point has its own ordering.
- The engine's exact textual-paste include semantics (does
  transitive include count? does cyclic include matter? what
  about include-on-the-same-line with another statement?) are not
  documented in this repo.

**Workaround for the user**:
- For cross-file forward declarations, add an explicit forward
  declaration in `B.xs` before the use site, or in `main.xs` before
  the `include "B.xs"` directive:
  ```xs
  // main.xs
  void X();  // forward declaration
  include "A.xs"
  include "B.xs"
  ```
  or:
  ```xs
  // B.xs
  extern void X();  // forward declaration
  X();
  ```
- This works because XS supports forward declarations (per
  `AGENTS.md` "Forward declarations are required" rule), and the
  forward declaration tells the LSP that `X` is "available" at this
  point in the include graph.

**Recommended future spec**:
- A dedicated SDD change (suggested name
  `fix-lsp-cross-file-forward-decl-ordering`) that:
  - Builds the full include graph for the open file's workspace.
  - Computes, for every symbol, an "ordering" property that
    reflects when the symbol becomes available to every other file
    in the graph.
  - Updates the forward-declaration diagnostic to use this
    cross-file ordering.
- Effort: ~1-2 weeks; requires careful thought about cyclic
  includes, transitive includes, and engine include semantics.
- Risk: medium-high; the ordering property may have surprising
  edge cases.

### Brace / operator / identifier-under-caret coloring gaps

Documented above in "Issue 3 sub-findings". To be addressed in
`expand-color-scheme-semantic-tokens`.

### Semantic-token-driven categories (Bucket C)

Documented above in "Issue 3 sub-findings" + the original
`openspec/changes/archive/2026-06-29-expand-color-scheme-categories/`
proposal. To be addressed in `expand-color-scheme-semantic-tokens`.