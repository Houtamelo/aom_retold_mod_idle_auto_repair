# XS Syntax Highlighting Specification

## Capability summary

The plugin SHALL syntax-highlight `.xs` files in IntelliJ-based IDEs (Rider, IDEA, etc.) by unifying the IntelliJ `Language` ID to lowercase `"xs"` so it matches the bundled TextMate bundle's `language` field. A native `XsSyntaxHighlighter` SHALL serve as a fallback when the TextMate bundle cannot be applied.

## Rationale

The current plugin declares `Language("XS")` while the bundled TextMate grammar declares `"language": "xs"`. IntelliJ's TextMate association is case-sensitive, so colorization is silently disabled. Lower-casing the language ID and adding a native fallback guarantees token coloring in every editor state.

## Requirements

### Requirement: unified language ID

The plugin MUST use the lowercase language ID `"xs"` for the IntelliJ `XsLanguage` and for every `language` attribute in `plugin.xml` that references the XS language.

#### Scenario: regression — no mixed-case language references remain

- GIVEN the existing `plugin.xml` contains `language="XS"` references
- WHEN the language ID is unified to `"xs"`
- THEN every `language` attribute in `plugin.xml` reads `language="xs"`
- AND no `language="XS"` reference remains

#### Scenario: editor language indicator shows lowercase xs

- GIVEN an `.xs` file is opened in the IDE with the plugin installed
- WHEN the editor status bar renders the file type
- THEN the language indicator displays `"xs"`

### Requirement: syntax highlighting via TextMate or native fallback

The plugin SHALL syntax-highlight `.xs` files using the bundled TextMate grammar. If the TextMate bundle fails to load, the native `XsSyntaxHighlighter` SHALL apply token coloring for keywords, comments, strings, numbers, and identifiers.

#### Scenario: happy path — TextMate highlighting applies

- GIVEN an `.xs` file is opened in an IntelliJ IDE with the plugin installed
- WHEN the IDE renders the editor view
- THEN the file is highlighted using TextMate scope names such as `keyword.xs`, `string.xs`, and `comment.xs`
- AND keywords, strings, comments, and numbers receive distinct colors

#### Scenario: fallback — native highlighter when TextMate is unavailable

- GIVEN the TextMate bundle cannot be loaded for any reason
- WHEN the IDE renders an `.xs` file
- THEN the native `XsSyntaxHighlighter` still colors keywords, comments, strings, numbers, and identifiers

#### Scenario: edge case — non-xs files are not highlighted as XS

- GIVEN a file whose extension is not `.xs`
- WHEN the file is opened in the IDE
- THEN the XS TextMate grammar and the native XS highlighter SHALL NOT be applied

## XS-engine constraints

- Syntax highlighting is lexical; the plugin SHALL NOT report engine-level semantic errors based on token colors.
- The `k_`, `g_`, and `s_` prefix scopes are naming conventions, not engine-enforced syntax.

## Out of scope

- Semantic highlighting based on symbol resolution.
- Color customization (covered by `spec-color-settings-page.md`).
- LSP-driven diagnostics or code inspections.

## Verification approach

- Automated: IntelliJ Platform tests using `BasePlatformTestCase` or `LightJavaCodeInsightFixtureTestCase` SHALL assert that `XsLanguage.ID == "xs"`, that a `SyntaxHighlighterFactory` is registered for the `"xs"` language, and that a fixture `.xs` file receives non-default `TextAttributesKey` tokens for keywords, strings, comments, and numbers where the test framework permits.
- Automated: a test SHALL scan `plugin.xml` and assert no `language="XS"` attribute remains.
- Manual: install the plugin in Rider/IDEA, open an `.xs` file, and verify that keywords, comments, and strings are colored.

## Acceptance criteria

1. `XsLanguage.INSTANCE.id` MUST equal `"xs"`.
2. Every `language` attribute in `plugin.xml` that references the XS language MUST be `"xs"`.
3. The bundled TextMate grammar's `language` field MUST remain `"xs"`.
4. The plugin MUST register a `SyntaxHighlighterFactory` for the `"xs"` language.
5. `.xs` files opened in the IDE SHALL receive syntax highlighting via the TextMate grammar or the native fallback.
6. The native fallback SHALL assign distinct `TextAttributesKey` instances to keywords, comments, strings, numbers, and identifiers.
7. Non-`.xs` files MUST NOT be highlighted as XS.
