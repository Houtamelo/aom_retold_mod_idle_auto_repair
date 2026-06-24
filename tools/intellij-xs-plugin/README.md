# IntelliJ Platform XS Plugin

IntelliJ Platform / Rider plugin for Age of Mythology: Retold `.xs` script files. The plugin is intentionally a thin LSP client: it registers the XS file type, bundles a TextMate grammar for syntax highlighting, provides editor helpers, and manages the connection to the Rust `xs-language-server`. All language semantics (diagnostics, completion, hover, go-to-definition) are handled by the LSP.

## What works today

- Buildable IntelliJ Platform plugin with `./gradlew buildPlugin`.
- Registers the `XS` language and associates `*.xs` files with it.
- Loads the bundled VS Code TextMate grammar as a TextMate bundle.
- Validates that the required bundled resource (`syntaxes/xs.tmLanguage.json`) is present before packaging.
- Settings page at **Settings → Languages & Frameworks → XS** for the AoM:R game folder and mod list.
- Auto-detect mods by recursively scanning the project for directories named exactly `game`.
- Spawns the Rust LSP server with `--game-path` and sends workspace folders / file watchers.

## Manual smoke test

```bash
./gradlew runIde
```

Open **Settings → Languages & Frameworks → XS**, set the game folder to the AoM:R install root, and click **Auto-detect mods**. Then open `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` and confirm syntax highlighting works and LSP diagnostics appear.

## Project docs

- Design and plan: `openspec/changes/intellij-xs-plugin/`
- Specs: `openspec/changes/intellij-xs-plugin/specs/`
- Current LSP redesign: `openspec/changes/xs-language-server/`
