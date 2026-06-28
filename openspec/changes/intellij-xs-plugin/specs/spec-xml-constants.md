# XS XML-Derived Constants Completion Specification

## Capability summary
The plugin SHALL complete game-data constants (`cUnitType*`, `cTech*`, `cCiv*`, `cCulture*`, `cProtoPower*`) emitted from user-supplied extracted XML files configured through settings.

## Rationale
XS scripts reference hundreds of unit types, technologies, civilizations, cultures, and god powers by prefixed constants. Generating these names from the game's XML files keeps the IDE aligned with the data the engine actually loads.

## Scenarios

### Scenario: happy path — proto units complete

- GIVEN the user has configured a path to a `proto.xml` directory
- WHEN the user types `cUnitTypeVill` and invokes completion
- THEN the lookup contains `cUnitTypeVillagerGreek` and other matching prefixed names

### Scenario: edge case — mixed-case XML is normalized

- GIVEN an XML entry uses `Name` in one file and `name` in another
- WHEN the scan finishes
- THEN both entries produce a correctly prefixed constant
- AND a `*_mods.xml` overlay file is merged additively with the base file

### Scenario: negative case — missing XML path gives no constants

- GIVEN the user has not configured the proto XML path
- WHEN the user types `cUnitType`
- THEN no XML-derived unit constants are offered
- AND a non-blocking notification MAY prompt the user to configure paths

## XS-engine constraints

- Constant names are textual prefixes applied to values that the AoM:R engine reads from `data/Data.bar`. The plugin does not know the numeric runtime IDs and does not validate them.
- The XML inputs are extracted files provided by the user; reading directly from `Data.bar` is outside this spec.

## Out of scope
This spec does NOT cover extraction from `data/Data.bar`, runtime XML reloading outside of file-system events, or AI plan constants.

## Verification approach

- Automated: `XsXmlConstantsCompletionTest` creates synthetic XML fixtures under `src/test/testData/`, configures paths, and asserts that `cUnitTypeVillagerGreek`, `cTechAge2`, etc. are contributed.
- Manual: configure paths to extracted game XMLs and confirm completion in `human_assist.xs`.

## Acceptance criteria

- The plugin MUST expose settings for the five XML sources: `xs.protoXml`, `xs.techtreeXml`, `xs.civsXml`, `xs.culturesXml`, and `xs.powersXml`.
- The plugin MUST recursively scan the configured directories for `*.xml` files.
- The plugin MUST merge additive `*_mods.xml` overlay files with the corresponding base file.
- The plugin MUST emit constants with the prefixes `cUnitType`, `cTech`, `cCiv`, `cCulture`, and `cProtoPower`.
- The plugin MUST normalize XML element and attribute names case-insensitively, including `_name` and `Name`.
- The plugin MUST include `<UnitType>` children of `<unit>` entries as additional `cUnitType` constants.
- The plugin MUST refresh the constant index when the configured XML files change on disk.
- The plugin MUST NOT require access to `Data.bar` to provide these completions.
