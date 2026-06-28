# XS Plugin Packaging and Build Specification

## Capability summary
The plugin SHALL be buildable as an IntelliJ Platform plugin using a pinned Gradle setup, shall register all required language services via `plugin.xml`, and shall bundle the data files, TextMate grammar, and signing configuration required for distribution.

## Rationale
Without a reproducible build and correct packaging, P0–P5 capabilities cannot be loaded, tested, or shipped. This capability underpins every other plugin capability.

## Scenarios

### Scenario: happy path — plugin builds and runs locally

- GIVEN a clean checkout of `tools/intellij-xs-plugin/`
- WHEN the user runs `./gradlew buildPlugin`
- THEN the build succeeds and produces an installable plugin distribution
- AND `./gradlew runIde` launches an IDE with the plugin loaded

### Scenario: edge case — optional Rider dependency

- GIVEN `plugin.xml` declares an optional dependency on Rider
- WHEN the plugin is loaded into IntelliJ IDEA Community
- THEN the plugin activates normally
- AND when loaded into Rider it also activates without a missing-dependency error

### Scenario: negative case — missing bundled resource fails the build

- GIVEN a required bundled resource such as `syscalls.json` is absent from `src/main/resources/`
- WHEN `./gradlew buildPlugin` runs
- THEN the build fails with a clear validation error before packaging

## XS-engine constraints

- The plugin contains only offline, static data derived from engine documentation and user-supplied XMLs. It does not link to or execute the AoM:R engine.
- Minimum and target platform versions SHALL be pinned so that APIs relied on by the other specs are available.

## Out of scope
This spec does NOT cover publishing to the JetBrains Marketplace, CI release pipelines, or deploying AoM:R mods from the IDE.

## Verification approach

- Automated: a CI step runs `./gradlew buildPlugin` and `./gradlew test`.
- Manual: run `./gradlew runIde`, open the `aom_retold_mod` project, and confirm `.xs` files are recognized.

## Acceptance criteria

- The plugin MUST build with IntelliJ Platform Gradle Plugin 2.x.
- The plugin MUST target platform `IC-252` and support `IC-242` as the minimum runtime.
- The plugin MUST use Kotlin and JVM target 21.
- The plugin MUST declare the `XS` language and `*.xs` file type.
- The plugin MUST declare a dependency on `org.jetbrains.plugins.textmate`.
- The plugin MUST bundle `syscalls.json`, `aiplans.json`, and `syntaxes/xs.json` in `src/main/resources/`.
- The plugin MUST register extension points for completion, documentation, parameter info, and workspace references as required by the other specs.
- The build MUST validate the presence of bundled data files before packaging.
- The build MUST support plugin signing through environment variables, even if no key is present in a local build.
- The plugin version, `since-build`, and `until-build` MUST be explicitly pinned.
