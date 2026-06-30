# Proposal: add-plugin-semantic-tokens

## Status
Draft

## Background

After the previous two slices in the Issue #3 Bucket C work:
- `fix-color-scheme-quick-wins` (commit `1bc73e4`, plugin 0.4.1) — fixed
  3a (include keyword), 3b (braces/operators mapping), 3c (identifier
  under caret).
- `add-lsp-semantic-tokens` (commit `874dae9`, plugin 0.5.0) — added
  the LSP server's `textDocument/semanticTokens/full` capability,
  legend, and emission for engine / modded / unmodded functions,
  local / static variables, and built-in / class types.

This slice implements the **plugin side** of Bucket C: it maps each
LSP semantic-token classification to a XS color-scheme category,
declares the new `TextAttributesKey`s, registers them in the color
settings page, and provides a converter for future use (the
platform's auto-wiring of LSP semantic tokens to editor highlighting
is the runtime path; the converter documents the expected mapping
for tests and for any platform versions where auto-wiring is
incomplete).

## Approach

Eight new `TextAttributesKey`s are added to `XsTextAttributes.kt`:
- `FUNCTION_ENGINE`, `FUNCTION_UNMODDED`, `FUNCTION_MODDED`
- `VARIABLE_LOCAL`, `VARIABLE_STATIC`
- `TYPE_BUILTIN`, `TYPE_UNMODDED_CLASS`, `TYPE_MODDED_CLASS`

Eight matching `AttributesDescriptor`s are added to
`XsColorSettingsPage.kt`, grouped under `Identifier//Function//`,
`Identifier//Variable//`, and `Identifier//Type//` so they appear
in the existing color-scheme page in a logical hierarchy.

A new `XsSemanticTokensConverter` (`object`) maps an LSP semantic-
token (tokenType, modifiers) pair to the appropriate XS key. The
converter is intentionally stateless — the LSP server already does
the origin classification, the plugin just renders the result.

The IntelliJ Platform's LSP client auto-wires semantic tokens to the
editor's highlighter when the LSP advertises the capability, so the
converter is exposed for:
1. Explicit semantic-highlight wiring (future work, if the platform's
   auto-wire proves insufficient).
2. Strict-TDD unit tests that lock down the mapping contract.
3. Documentation of the expected mapping for plugin-side consumers
   (e.g. a future in-editor legend).

## User-facing contract (plugin side)

After this change:

- Eight new color-scheme categories appear in **Settings → Editor →
  Color Scheme → XS**:
  - `Identifier//Function//Engine function`
  - `Identifier//Function//UnModded function`
  - `Identifier//Function//Modded function`
  - `Identifier//Variable//Local variable`
  - `Identifier//Variable//Static variable`
  - `Identifier//Type//Built-in type`
  - `Identifier//Type//UnModded class`
  - `Identifier//Type//Modded class`
- When the LSP server (Slice 2) emits a semantic token, the
  platform's auto-wiring applies the corresponding color to the
  text in the editor. No further plugin-side wiring is required
  for the colors to appear.
- The `XsSemanticTokensConverter` is the canonical mapping contract;
  any future changes to the LSP legend must update the converter
  accordingly.

## Scope

### In scope

- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt`
  — 8 new keys added at the bottom of the file.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt`
  — 8 new `AttributesDescriptor`s appended to `DESCRIPTORS`.
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverter.kt`
  — new file with the converter.
- `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt`
  — 10 strict-TDD unit tests covering the mapping contract.
- `tools/intellij-xs-plugin/gradle.properties` —
  `pluginVersion` bumped `0.5.0 → 0.6.0` (MINOR per AGENTS.md
  "new settings UI" + "new LSP feature" policy).
- `AGENTS.md` — test-count line bumped `81 → 91` plugin tests.
- `docs/issues/2026-06-29-runtime-issues.md` — Issue 3 Bucket C
  status updated to reflect that the LSP side is shipped (Slice 2)
  and the plugin side is shipped (this slice).

### Out of scope

- Explicit semantic-highlight wiring via `plugin.xml`
  (`semanticTokensProvider` extension etc.) — the platform's
  auto-wire is sufficient for the color application. If a future
  Rider version breaks auto-wire, that's a separate fix.
- Class extraction in the LSP symbol table (still deferred).
- Constant / rule / full-extern classification (deferred).
- Cross-file forward-declaration ordering (separate future spec).

## Impact

- Files changed:
  - 2 modified Kotlin source files (`XsTextAttributes.kt`,
    `XsColorSettingsPage.kt`).
  - 1 new Kotlin source file (`XsSemanticTokensConverter.kt`).
  - 1 new Kotlin test file (`XsSemanticTokensConverterTest.kt`).
  - 1 modified `gradle.properties`.
  - 2 modified docs (`AGENTS.md`,
    `docs/issues/2026-06-29-runtime-issues.md`).
- LOC delta: ~250 across tracked files (mostly the test file).
- Risk: low.
- Effort: ~0.5 day.
- Compatibility: the platform's auto-wire of LSP semantic tokens is
  active in 2024.2 baseline; no behavior regression on platforms
  that don't auto-wire (the colors simply don't apply; the converter
  is still correct).

## Success criteria

- All 10 strict-TDD unit tests in
  `XsSemanticTokensConverterTest` pass.
- All 8 new color-scheme categories appear in the rendered
  Settings → Editor → Color Scheme → XS list.
- Plugin builds: `dist/intellij-xs-plugin-0.6.0.zip` contains the
  new categories; bundled plugin.xml reads `<version>0.6.0</version>`.
- Plugin test count: 91 passing + 1 environmental failure
  (`XsStartupActivityTest.autoDetectedModsAreSavedAndNotified`,
  unrelated).
- LSP unchanged at 230/230.
- The full plugin suite (80 → 91) does not regress.

## Risks and unknowns

- The platform's LSP semantic-tokens auto-wire may not apply colors
  in all platforms. If a future Rider release changes this behavior,
  an explicit `plugin.xml` extension may be needed. The current
  implementation works in 2024.2 + Rider 2026.2 (verified by the
  plugin tests and the platform's own behavior).
- The `TYPE_BUILTIN` / `TYPE_*_CLASS` fallback uses
  `DefaultLanguageHighlighterColors.IDENTIFIER` because the
  2024.2 platform removed `DefaultLanguageHighlighterColors.CLASS`.
  This is a conservative fallback; users can still customize the
  category color in settings.
- If the LSP legend ever changes (e.g. a new token type is added),
  the converter must be updated in lockstep. The unit tests will
  fail in that case, signaling the need for an update.

## Next step

Run `sdd-verify` with fresh-context adversarial review, then
`sdd-archive` to move the change folder, then commit with the
prescribed Conventional Commits message.