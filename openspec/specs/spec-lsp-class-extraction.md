# spec-lsp-class-extraction

> **Status**: Active — promoted from `openspec/changes/finish-semantic-token-distinctions/` (commit `dfa829d`, 2026-06-30).

## Purpose

This specification extends the XS LSP symbol table so that `class` declarations contribute named symbols. With class names in the table, type-identifier references can resolve to the file in which the class is actually defined and receive the correct origin modifier (`modded` or `unmodded`) instead of always falling back to `engine`.

## Background

The tree-sitter XS grammar already parses `class_specifier` nodes, but the symbol-table builder ignores them. `SymbolKind` currently contains `Rule`, `Function`, `Variable`, and `Constant`, but no `Class`. As a result, the semantic-token classifier cannot resolve user-defined class references through the merged workspace view or the current file's own table; it falls back to `Origin::Engine`, which the plugin renders as a built-in primitive type (`TYPE_BUILTIN`). This mislabels vanilla classes as built-ins and hides the engine/modded/unmodded distinction for class references.

The grammar exposes a `class_specifier` with a `name` field (`_type_identifier`) and a `body` field (`field_declaration_list`). For origin classification, only the class name needs to be captured; fields and methods inside the body are not required.

## Requirements

### R1 — `SymbolKind::Class` variant

The `SymbolKind` enum SHALL gain a new `Class` variant.

- GIVEN a valid XS source containing `class Foo {}`, WHEN the symbol table is built, THEN it SHALL contain a `Symbol` whose `kind` is `SymbolKind::Class`.

### R2 — Walk `class_specifier` nodes

The symbol-table builder SHALL walk top-level `class_specifier` AST nodes and produce `Symbol` entries with `kind: SymbolKind::Class`. Each entry SHALL capture the class name, the full range of the `class_specifier`, and the selection range of the class-name identifier. The provenance used for later semantic-token classification SHALL be the file in which the `class_specifier` was parsed.

- GIVEN `class MyClass { ... }` in `mod/spire_ai/game/foo.xs`, WHEN `build_symbol_table` runs, THEN it SHALL emit a `Class` symbol named `MyClass` with valid ranges and the same provenance as any top-level declaration in that file.

### R3 — Class member extraction

Class members (fields, methods) SHALL also be extracted from inside `class_specifier` bodies and represented as first-class symbols. See `spec-lsp-class-member-extraction.md` for the full member-extraction requirements.

- GIVEN a class `MyClass` with a field `value` and a method `doWork`, WHEN the symbol table is built, THEN it SHALL contain a `ClassField` named `value` and a `ClassMethod` named `doWork`, both with `class_owner: Some("MyClass")`.

### R4 — Additive `SymbolKind::Class`

The new `SymbolKind::Class` variant SHALL be additive. It SHALL NOT change the serialization format, hash implementation, or cached engine/symbol data in a way that breaks warm caches stored at `~/.local/state/aomr_lsp/v2/`. If warm caches from a previous version cannot be loaded gracefully, the design phase SHALL propose a cache-migration strategy (for example, bump the cache schema version or rebuild from source).

- GIVEN a warm `v2` cache created before this change, WHEN the LSP starts, THEN it SHALL either load the cache without error or transparently rebuild the affected symbol data from source.

### R5 — Class reference origin in semantic tokens

The `semantic_tokens::classify_type_identifier` function SHALL look up `_type_identifier` references in the merged view or the current file's own table. When a class symbol is found, it SHALL emit `SemanticTokenType::TYPE` with the origin modifier of the defining file (`engine`, `modded`, or `unmodded`). The existing fallback to `Origin::Engine` for unresolved type identifiers SHALL remain unchanged.

- GIVEN class `MapInfo` declared in `<AOMR>/game/foo.xs` and referenced as `MapInfo gMapInfo;` in `mod/spire_ai/game/bar.xs`, WHEN semantic tokens are computed, THEN the `MapInfo` reference SHALL emit `SemanticTokenType::TYPE` with modifier `unmodded`.
- GIVEN class `MyAIClass` declared in `mod/spire_ai/game/foo.xs` and referenced in `mod/spire_ai/game/bar.xs`, WHEN semantic tokens are computed, THEN the reference SHALL emit `SemanticTokenType::TYPE` with modifier `modded`.

## Scenarios

### S1 — Vanilla class reference in a mod file

- GIVEN a class `MyClass` defined in `<AOMR>/game/foo.xs` (vanilla, NOT in any mod overlay),
- WHEN a reference `MyClass x;` is parsed in `mod/spire_ai/game/bar.xs`,
- THEN the LSP SHALL emit `SemanticTokenType::TYPE` with modifier `unmodded` for the reference.

### S2 — Modded class reference

- GIVEN a class `MyModdedClass` defined in `mod/spire_ai/game/foo.xs`,
- WHEN a reference `MyModdedClass x;` is parsed in `mod/spire_ai/game/bar.xs`,
- THEN the LSP SHALL emit `SemanticTokenType::TYPE` with modifier `modded`.

### S3 — Unresolved type falls back to built-in behavior

- GIVEN the user has no `class` defined for `Foo`,
- WHEN a reference `Foo x;` is parsed,
- THEN the LSP SHALL fall back to `Origin::Engine`, preserving existing built-in-type behavior.

### S4 — Performance on a large vanilla file

- GIVEN a large vanilla game file (for example, 5000+ lines with 50+ class declarations),
- WHEN the symbol table is built,
- THEN it SHALL complete in less than 100 ms on the reference development machine.

### S5 — Cache compatibility

- GIVEN the LSP cache from a previous version (before this change),
- WHEN the LSP starts,
- THEN it SHALL either load the cache gracefully (ignoring class entries) or rebuild from source, with no fatal error.

### S6 — Class members are extracted alongside class names

- GIVEN a class `MyClass` with field `value` and method `doWork`,
- WHEN the symbol table is built,
- THEN it SHALL emit one `Class` symbol for `MyClass`, one `ClassField` for `value`, and one `ClassMethod` for `doWork`.

### S7 — Class members appear in outline and workspace symbols

- GIVEN class `Unit` with method `takeDamage`,
- WHEN a `DocumentSymbol` or `workspace/symbol` request is received,
- THEN `takeDamage` SHALL appear as a child of `Unit` and SHALL be searchable by name.

## Out of scope

- Constructors and dedicated `new` expression analysis.
- Semantic-token emission for class members (covered by `spec-lsp-class-member-semantic-tokens.md`).
- Cross-file forward-declaration ordering.
- Recoloring the existing `TYPE_UNMODDED_CLASS` / `TYPE_MODDED_CLASS` fallback keys.
- Engine-side `extern` classification for classes.

## Verification approach

- Strict-TDD Rust tests in `tools/xs-language-server/tests/class_extraction_repro.rs` covering S1–S3 and S5.
- A `symbols.rs` unit test covering R1 and the `class_specifier` walker (R2).
- A performance sanity test or benchmark covering S4.
- An integration test that starts the server with a warm `~/.local/state/aomr_lsp/v2/` cache to confirm cache compatibility (S5).
- Manual Rider smoke test confirming that class references in vanilla and modded files render with distinct colors.
