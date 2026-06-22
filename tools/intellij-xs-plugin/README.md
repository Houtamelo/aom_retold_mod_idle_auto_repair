# IntelliJ Platform XS Plugin

P0 scaffold for an IntelliJ Platform / Rider plugin that adds basic support for Age of Mythology: Retold `.xs` script files.

## What works in P0

- Buildable IntelliJ Platform plugin with `./gradlew buildPlugin`.
- Registers the `XS` language and associates `*.xs` files with it.
- Loads the bundled VS Code TextMate grammar as a TextMate bundle.
- Validates that required bundled resources (`syscalls.json`, `aiplans.json`, `xs.tmLanguage.json`) are present before packaging.
- CI workflow runs `./gradlew buildPlugin` and `./gradlew test` on pushes and PRs to `main`.

## Manual smoke test

```bash
./gradlew runIde
```

Open `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` and confirm keywords, strings, comments, rule names, and `k`/`g`/`s` prefixes are colored.

## Project docs

- Design and plan: `openspec/changes/intellij-xs-plugin/`
- Specs: `openspec/changes/intellij-xs-plugin/specs/`
