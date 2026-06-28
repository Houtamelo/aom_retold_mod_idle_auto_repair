# Game-Folder Diagnostic Test Coverage Specification

> **Added/updated by change:** `lsp-server-semantic-fixes`

## Capability summary

The integration test `tests/game_folder_parse.rs` SHALL exercise the complete diagnostic pipeline — parse errors, type checks, and semantic checks — against the full official AoM:R `game/**/*.xs` tree. It SHALL stop filtering out engine-API names and SHALL assert zero diagnostics for every tracked category plus zero total diagnostics. The test SHALL skip cleanly when `AOMR_GAME_PATH` is not set.

## Rationale

The baseline integration test only ran `SemanticChecker::check_all`, manually filtered engine-API names, and never invoked `typecheck::check_calls_with_merged`. That allowed ~189 false-positive diagnostics to pass undetected while still reporting "0 unresolved". A strict, unfiltered, end-to-end gate is required to prove the LSP matches the engine on real shipping source.

## Scenarios

### Scenario: happy path — full game folder has zero diagnostics

- GIVEN the game folder is installed and parseable
- WHEN the test runs
- THEN all six tracked counts SHALL be zero:
  - `duplicate_extern_count`
  - `wrong_diagnostic_uri_count`
  - `unresolved_symbol_count`
  - `wrong_arg_count_count`
  - `rule_call_unresolved_count`
  - `total_diagnostic_count`

### Scenario: skip when game folder is absent

- GIVEN the `AOMR_GAME_PATH` environment variable is not set
- WHEN the test runs
- THEN the test SHALL skip cleanly with a clear message

### Scenario: examples printed on failure

- GIVEN the game folder is installed
- AND at least one tracked diagnostic category is non-zero
- WHEN the test runs
- THEN the test SHALL print representative file paths and messages for the failing category
- AND the assertion SHALL fail

## XS-engine constraints

- The official game scripts are the source of truth for valid XS syntax and semantics.
- Binary `.xs` files under `random_maps/` are not valid UTF-8 and remain excluded from the text-file set.
- Included helper files are not analyzed as top-level files, matching the existing test design.

## Out of scope

- Adding performance benchmarks or start-up timing assertions.
- Testing mod overlays; the gate is the vanilla game folder.
- Network or auto-update tests for the game installation.

## Verification approach

- The test itself is the verification artifact; it runs with `cargo test --test game_folder_parse -- --nocapture`.
- Local development runs with `AOMR_GAME_PATH` set; CI runs without it and expects the skip path.

## Acceptance criteria

1. The test SHALL invoke the full diagnostics pipeline (`diagnostics::collect_all`), including `semantic.rs` and `typecheck.rs`, without filtering out engine-API names.
2. The test SHALL use stable categorization helpers in `diagnostics.rs` to count diagnostics in each category; string matching SHALL NOT be the primary classifier.
3. The test SHALL assert:
   - `duplicate_extern_count == 0`
   - `wrong_diagnostic_uri_count == 0`
   - `unresolved_symbol_count == 0`
   - `wrong_arg_count_count == 0`
   - `rule_call_unresolved_count == 0`
   - `total_diagnostic_count == 0`
4. The test SHALL run against every top-level parseable `.xs` file under `~/.steam/steam/steamapps/common/Age of Mythology Retold/game/**/*.xs`.
5. The test SHALL skip cleanly if `AOMR_GAME_PATH` is not set, preserving CI compatibility.
