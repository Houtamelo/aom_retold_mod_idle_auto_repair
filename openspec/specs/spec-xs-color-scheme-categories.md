# Delta for XS Color Scheme Settings Page

> **Added/updated by change:** `expand-color-scheme-categories`

## Capability summary

The XS plugin SHALL expose an **"XS"** color-scheme page (renamed from `"xs"`) containing the existing six categories plus twelve inherited Rider General / Language Defaults categories.

## ADDED Requirements

### Requirement: uppercase XS color-scheme menu label

The system SHALL rename the page display name from `"xs"` to `"XS"`.

#### Scenario: menu label reads XS

- GIVEN plugin is installed
- WHEN the user opens **Settings → Editor → Color Scheme**
- THEN **"XS"** appears

### Requirement: inherited Rider General / Language Defaults categories

The XS color settings page SHALL expose these twelve inherited `TextAttributesKey` categories, grouped under **Code**, **Errors and Warnings**, and **Braces and Operators**: Identifier under caret, Matched brace, Unmatched brace, Unknown symbol, Braces, Brackets, Comma, Dot, Operation sign, Overloaded operator, Parentheses, Semi-colon.

#### Scenario: new categories visible

- GIVEN the user opens the XS color settings page
- WHEN the page renders
- THEN the categories appear under the correct groups

### Requirement: color customization persists across IDE restarts

User color choices SHALL persist across IDE restarts.

#### Scenario: customized color survives restart

- GIVEN the user changes any XS category color
- WHEN the IDE restarts
- THEN the customized color remains

### Requirement: existing syntax highlighting still works

Opening an `.xs` file SHALL keep existing `XsSyntaxHighlighter` behavior. Braces, brackets, parentheses, commas, semicolons, dots, and operator tokens SHOULD map to the new inherited keys; unmapped tokens SHALL fall back to their previous key.

#### Scenario: tokens highlight with new or fallback keys

- GIVEN an `.xs` file containing brace/operator punctuation
- WHEN it is highlighted
- THEN those tokens map to the new inherited keys

### Requirement: strict-TDD test for the expanded color page

A test SHALL assert the expanded page behavior.

#### Scenario: test asserts uppercase name and expanded descriptors

- GIVEN an updated or new `XsColorSettingsPageTest`
- WHEN it asserts `displayName == "XS"` and descriptors include the inherited keys
- THEN the test passes

### Requirement: plugin version bump for new settings UI

The committed `pluginVersion` SHALL be `0.3.0`.

#### Scenario: gradle.properties reflects minor bump

- GIVEN `tools/intellij-xs-plugin/gradle.properties`
- THEN `pluginVersion = 0.3.0`

## MODIFIED Requirements

### Requirement: discoverable color scheme page

The plugin MUST register an `XsColorSettingsPage` so that **"XS"** appears in **Settings → Editor → Color Scheme**.
(Previously: the page was registered with the display name `"xs"`.)

#### Scenario: happy path — XS appears

- GIVEN plugin is installed
- WHEN the user opens **Settings → Editor → Color Scheme**
- THEN **"XS"** appears

### Requirement: configurable text attributes

The `XsColorSettingsPage` SHALL expose `AttributesDescriptor` entries for Keyword, String, Comment, Number, Identifier, Default, and the inherited categories above. The preview snippet SHALL include representative examples of the new inherited categories.
(Previously: only Keyword, String, Comment, Number, Identifier, and a default attribute were exposed, and the preview showed only those categories.)

#### Scenario: attribute list exposes required categories

- GIVEN user opens XS page
- WHEN the page renders
- THEN it lists the six existing and twelve inherited categories

#### Scenario: preview includes inherited categories

- GIVEN user opens XS page
- WHEN preview renders
- THEN it shows braces, brackets, parens, commas, semicolons, dots, and operators

## REMOVED Requirements

None.

## Out of scope

- LSP `textDocument/semanticTokens` capability.
- Origin-path classification (engine / modded / unmodded).
- Engine/UnModded/Modded function, rule, constant, variable, class, local/static/extern variables.
- Built-in type as a dedicated semantic category.
- Issues 1, 2, and 4 from `docs/issues/2026-06-29-runtime-issues.md`.

## Cross-references

- **User issue:** `docs/issues/2026-06-29-runtime-issues.md` Issue 3.
- **Main spec:** `openspec/specs/spec-color-settings-page.md`.
- **Version policy:** `AGENTS.md`.

## Future work

A follow-up change will add LSP semantic tokens and a plugin-side converter for the semantic-token-driven categories from Issue #3.

## Verification approach

- `./gradlew :test` passes, including expanded `XsColorSettingsPageTest`.
- `./gradlew buildPlugin` produces a valid `.zip`.
- Manual: verify uppercase label, grouped categories, and preview.
