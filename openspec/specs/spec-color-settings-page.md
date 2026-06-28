# XS Color Scheme Settings Page Specification

## Capability summary

The plugin SHALL register a Color Scheme settings page so that XS appears under **Settings → Editor → Color Scheme → xs**. The page SHALL expose configurable `TextAttributesKey` descriptors for Keyword, String, Comment, Number, Identifier, and a default attribute, and SHALL render a live preview of an XS code snippet.

## Rationale

Users expect JetBrains language plugins to expose color customization through the standard Color Scheme settings. Without a registered `ColorSettingsPage`, XS is absent from that list and users cannot tune token colors.

## Requirements

### Requirement: discoverable color scheme page

The plugin MUST register an `XsColorSettingsPage` so that "xs" appears in the **Settings → Editor → Color Scheme** language list and selecting it opens the XS settings page.

#### Scenario: happy path — xs appears in color scheme list

- GIVEN the plugin is installed in an IntelliJ IDE
- WHEN the user opens **Settings → Editor → Color Scheme**
- THEN "xs" appears in the language list
- AND selecting "xs" displays the `XsColorSettingsPage`

### Requirement: configurable text attributes

The `XsColorSettingsPage` SHALL expose `ColorDescriptor` entries for at least Keyword, String, Comment, Number, Identifier, and a default XS attribute so the user can customize their colors.

#### Scenario: attribute list exposes required token categories

- GIVEN the user opens the XS color settings page
- WHEN the page renders the attribute list
- THEN it contains configurable entries for Keyword, String, Comment, Number, Identifier, and a default attribute

### Requirement: live preview panel

The `XsColorSettingsPage` SHALL render a preview panel containing an XS code snippet whose tokens are colored with the currently selected attributes.

#### Scenario: preview reflects current color choices

- GIVEN the user is on the XS color settings page
- WHEN the page renders
- THEN the preview panel displays an XS code snippet
- AND the snippet's keywords, strings, comments, numbers, and identifiers are colored according to the current settings

#### Scenario: preview updates when colors change

- GIVEN the user changes a color attribute on the XS color settings page
- WHEN the change is applied in the page
- THEN the preview panel updates to reflect the new color

## XS-engine constraints

- Color descriptors are presentation-only; changing a color MUST NOT change how the XS language is parsed or interpreted.

## Out of scope

- Semantic highlighting colors (e.g., distinct colors for engine API vs. user functions).
- Export/import of color schemes.
- Per-mod color overrides.

## Verification approach

- Automated: an IntelliJ Platform test SHALL assert that the `ColorSettingsPage` extension for the `"xs"` language resolves to the `XsColorSettingsPage` implementation and that it returns a non-empty array of `AttributesDescriptor` entries covering Keyword, String, Comment, Number, Identifier, and a default attribute.
- Automated: the test SHALL assert that `getDemoText()` returns a non-empty XS code snippet.
- Manual: open **Settings → Editor → Color Scheme → xs**, verify the attribute list and preview panel render, and verify that changing a color updates the preview.

## Acceptance criteria

1. A `ColorSettingsPage` MUST be registered for the `"xs"` language.
2. The page's display name MUST be `"xs"` or `"XS"` as shown in the color scheme list.
3. The page SHALL expose `AttributesDescriptor` entries for Keyword, String, Comment, Number, Identifier, and a default attribute.
4. The page SHALL return a non-empty `demoText` string containing XS code.
5. The preview panel SHALL apply the current color settings to the demo text.
6. Changing a color attribute in the page SHALL update the preview immediately.
