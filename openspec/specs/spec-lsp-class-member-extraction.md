# spec-lsp-class-member-extraction

> **Status**: Active — promoted from `openspec/changes/expand-color-scheme-classes-constants-rules/` (commits `9cc7344` + `e512f10`, 2026-06-30).

## Purpose

This specification extends the XS LSP symbol table so that class bodies contribute named member symbols. Class fields and methods become first-class `Symbol` entries carrying their enclosing class name, enabling member-reference resolution, document-outline nesting, and workspace-wide symbol search.

## Background

The promoted spec `spec-lsp-class-extraction.md` added `SymbolKind::Class` and captured class names. That specification explicitly excluded walking the `field_declaration_list` body. As a result, references such as `obj.field` and `Class.method()` still fall back to local or global symbol lookup, or remain unresolved. With class members in the workspace symbol table, the LSP can resolve these references by member name and propagate origin metadata (`engine`, `unmodded`, `modded`) to downstream features.

## Requirements

### R1 — `SymbolKind::ClassField` and `SymbolKind::ClassMethod` variants

The `SymbolKind` enum SHALL gain the new variants `ClassField` and `ClassMethod`.

- GIVEN a class declares a field `int health = 100;`, WHEN the symbol table is built, THEN the table SHALL contain a `Symbol` whose `kind` is `SymbolKind::ClassField`.
- GIVEN a class declares a method `void takeDamage(int amount)`, WHEN the symbol table is built, THEN the table SHALL contain a `Symbol` whose `kind` is `SymbolKind::ClassMethod`.

### R2 — `Symbol::class_owner` field

The `Symbol` struct SHALL gain a `class_owner: Option<String>` field. For `ClassField` and `ClassMethod` symbols it SHALL be `Some(<enclosing class name>)`; for all top-level symbols it SHALL be `None`.

- GIVEN class `Unit` with field `health`, WHEN the symbol table is built, THEN the field's `class_owner` SHALL equal `Some("Unit")`.
- GIVEN a top-level variable `int gFoo;`, WHEN the symbol table is built, THEN its `class_owner` SHALL equal `None`.

### R3 — Class member visibility remains `Visibility::Local`

Class member symbols SHALL use `Visibility::Local`. They SHALL NOT be merged across files via `include` paste or the merged workspace view.

- GIVEN class `Foo` with field `value` declared in both a mod file and a vanilla file, WHEN the merged workspace view is queried, THEN each member SHALL remain scoped to its own file's class and SHALL NOT surface as a duplicated global symbol.

### R4 — Extract class fields

The symbol-table builder SHALL walk each `field_declaration` child of a `class_specifier` body and emit a `Symbol` with `kind: SymbolKind::ClassField`, `name` equal to the field identifier, and `class_owner: Some(<class name>)`.

- GIVEN `class Foo { int value; }`, WHEN `build_full_symbol_table` runs, THEN it SHALL emit a `ClassField` named `value` with `class_owner: Some("Foo")`.

### R5 — Extract class methods

The symbol-table builder SHALL walk each function declaration inside a `class_specifier` body and emit a `Symbol` with `kind: SymbolKind::ClassMethod`, `name` equal to the method identifier, and `class_owner: Some(<class name>)`.

- GIVEN `class Foo { void bar() {} }`, WHEN `build_full_symbol_table` runs, THEN it SHALL emit a `ClassMethod` named `bar` with `class_owner: Some("Foo")`.

### R6 — Preserve existing `SymbolKind::Class` extraction

The existing `class_specifier` name extraction that produces `SymbolKind::Class` SHALL remain unchanged.

- GIVEN `class MyClass { ... }`, WHEN the symbol table is built, THEN it SHALL still emit exactly one `SymbolKind::Class` named `MyClass` for the class name.

### R7 — Cache schema bump

The per-file parse-cache schema SHALL be bumped from `game_parse/v2/` to `game_parse/v3/`. The server SHALL rebuild stale `v2` caches transparently and without fatal error.

- GIVEN a warm `game_parse/v2/` parse cache from a previous version, WHEN the LSP starts with the new code, THEN it SHALL either discard the `v2` cache and rebuild as `v3`, or load it gracefully as a fallback, with no fatal error.

### R8 — Class members in document outline and workspace symbol results

Class member symbols SHALL appear inside their owning class in `DocumentSymbol` responses and SHALL be searchable by name in `workspace/symbol`.

- GIVEN class `Unit` with method `takeDamage`, WHEN a `DocumentSymbol` request is received for the file, THEN the response SHALL contain `Unit` as a parent with `takeDamage` as a child.
- GIVEN class `Unit` with method `takeDamage`, WHEN a `workspace/symbol` request is received with query `takeDamage`, THEN the response SHALL include the member.

## Scenarios

### S1 — Field and method extraction from a vanilla class

- GIVEN a class `MyClass` with field `health` and method `takeDamage` declared in `<AOMR>/game/foo.xs`,
- WHEN the symbol table is built,
- THEN the table SHALL contain `ClassField { name: health, class_owner: Some("MyClass") }` AND `ClassMethod { name: takeDamage, class_owner: Some("MyClass") }`.

### S2 — Member reference resolves through workspace symbol table

- GIVEN a class `MyClass` with field `health` declared in `<AOMR>/game/foo.xs`,
- WHEN a reference `obj.health` is parsed in `mod/spire_ai/game/bar.xs`,
- THEN the LSP SHALL resolve `health` to the field defined in `MyClass` through the workspace's class member table.

### S3 — Ambiguous member name origin heuristic

- GIVEN class `MyClass` (vanilla) and class `MyModdedClass` (mod) both define a field `value`,
- WHEN a reference `obj.value` is parsed,
- THEN the LSP SHALL emit the `modded` modifier if any modded class has the member, else `unmodded` if any vanilla class has it, else `engine`.

### S4 — Cache compatibility on upgrade

- GIVEN a previous-version `game_parse/v2/` parse cache exists,
- WHEN the server starts with the new code,
- THEN it SHALL rebuild the cache under `game_parse/v3/` or load the old cache without fatal error.

### S5 — Performance on a large class file

- GIVEN a vanilla file with 5000+ lines and 50+ class declarations (including members),
- WHEN the symbol table (with member extraction) is built,
- THEN it SHALL complete in less than 200 ms on the reference development machine.

### S6 — Mixed class body

- GIVEN a class body with fields, methods, and nested declarations,
- WHEN the symbol table is built,
- THEN all valid members SHALL be extracted and no class member SHALL be skipped or misclassified.

### S7 — Malformed class body resilience

- GIVEN a class body with missing semicolons or invalid syntax producing `ERROR` nodes,
- WHEN the symbol table is built,
- THEN the walker SHALL skip invalid children and continue without panic.

## Out of scope

- Type inference for the object expression in `obj.field` or `obj.method()`.
- Constructors and `new` expressions.
- Class inheritance, virtual methods, and method overriding.
- Access modifiers (`public`/`private`).
- Static versus instance member distinction.
- Array built-in calls (`.size()`, `.add()`, `.clear()`) remain uncolored unless already handled elsewhere.
- Cross-file member lookup via `include` paste.

## Verification approach

- Strict-TDD Rust tests in `tools/xs-language-server/tests/class_member_extraction_repro.rs` covering S1–S3, S5, S6, and S7.
- A `symbols.rs` unit test asserting `SymbolKind::ClassField` and `SymbolKind::ClassMethod` variants exist (R1).
- A workspace-symbol integration test asserting `workspace/symbol` returns a class member by name (R8).
- A manual Rider smoke test confirming the document outline nests class members under their parent class (R8, S6).
