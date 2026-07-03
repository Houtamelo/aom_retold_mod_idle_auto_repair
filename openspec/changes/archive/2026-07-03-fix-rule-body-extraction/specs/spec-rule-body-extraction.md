# Spec: `fix-rule-body-extraction`

## Capability summary

The `block_item^` rule in `tools/xs-language-server/lelwel-xs/src/xs.llw` SHALL dispatch declarations via a parser-side `?is_type` predicate instead of the over-permissive `?t` predicate. This allows `rule Name { body }` bodies that contain statements or local declarations to parse with zero ERROR nodes, so `RuleDefinition::from_cst` succeeds for non-empty rule bodies.

## Rationale

`RuleDefinition::from_cst` currently returns `None` for any non-empty rule body because the grammar emits ERROR nodes for real XS inside `compound_statement`. The typed AST layer already defines the `RuleDefinition` shape; this change fulfills its expectation without adding a fallback in AST extraction. A small `TypeTable` context gates whether an identifier should parse as a type, preventing `x;` from being mis-parsed as a declaration while still accepting `MyClass x;`.

## Grammar source of truth

`tools/xs-language-server/lelwel-xs/src/xs.llw`. The `block_item^` alternatives for declaration and function-definition SHALL use `?is_type` on the first token; statement alternatives SHALL remain unchanged.

## Scenarios

### Scenario: S-RBE-01 Empty rule body baseline
- GIVEN the source `rule r { }`
- WHEN `RuleDefinition::from_cst` is called
- THEN it returns `Some(_)` with zero body items

### Scenario: S-RBE-02 Single statement body
- GIVEN the source `rule r { x; }`
- WHEN `RuleDefinition::from_cst` is called
- THEN it returns `Some(_)` with one statement body item

### Scenario: S-RBE-03 Single declaration body
- GIVEN the source `rule r { int x = 1; }`
- WHEN `RuleDefinition::from_cst` is called
- THEN it returns `Some(_)` with one declaration body item

### Scenario: S-RBE-04 Class-typed declaration body
- GIVEN the source `rule r { MyClass x; }` and a `TypeTable` containing `MyClass`
- WHEN `RuleDefinition::from_cst` is called
- THEN it returns `Some(_)` with one declaration body item

### Scenario: S-RBE-05 Mixed declarations and statements
- GIVEN the source `rule r { int x = 1; x; }`
- WHEN `RuleDefinition::from_cst` is called
- THEN it returns `Some(_)` with one declaration and one statement body item

### Scenario: S-RBE-06 Sampled retail rule body
- GIVEN a body excerpt from `game/ai/core/economic_units.xs` rule `deleteExcessGatherers`
- WHEN the parser processes the sampled rule body
- THEN zero ERROR nodes appear at the rule level

### Scenario: S-RBE-07 TypeTable must be populated for primitives
- GIVEN an empty `TypeTable`
- WHEN parsing `rule r { int x; }`
- THEN the body does NOT extract as a declaration
- AND extraction falls through to a statement body item

### Scenario: S-RBE-08 Unknown identifier falls through
- GIVEN a `TypeTable` containing primitives but not `MyClass`
- WHEN parsing `rule r { MyClass x; }`
- THEN the body falls through to statement extraction

### Scenario: S-RBE-09 Existing tests remain green
- GIVEN the existing 159 `lelwel-xs` tests
- WHEN `cargo test` runs in `tools/xs-language-server/lelwel-xs/`
- THEN all tests pass

## API additions

| Item | Description |
|------|-------------|
| `pub struct TypeTable` | Parser context for the `?is_type` predicate |
| `pub primitives: HashSet<&'static str>` | Built-in type keywords (`int`, `float`, `bool`, `string`, `vector`) |
| `pub classes: HashSet<String>` | Caller-supplied class identifiers; populated by Phase 4 |
| `impl Default for TypeTable` | Returns a table pre-seeded with primitive keywords |
| `ParserCallbacks::Context = TypeTable` | Replaces `()` so generated predicates can consult context |
| `pub fn is_type(&self, name: &str) -> bool` | True when `name` is in `primitives` or `classes` |
| `pub use` in `ast/mod.rs` | Re-exports `TypeTable` for Phase 4 integration |

## Out of scope

- LSP wiring (Phase 4).
- Full Limitation 6 cleanup: L1B/L2B placeholders, cast expressions, and `typedef` support.
- Populating `TypeTable` from engine API or workspace symbols; only the API is added now.

## Non-functional requirements

- **Performance**: parsing a 5,000-line XS file MUST remain under 50ms; the predicate lookup is O(1).
- **Compilation**: `cargo build` in `lelwel-xs` MUST remain green after the `xs.llw` change regenerates `parser.rs`.
- **Tests**: the existing 159 `lelwel-xs` tests MUST pass.
- **Verification method**: `cargo test --manifest-path tools/xs-language-server/Cargo.toml` per `openspec/config.yaml` strict TDD override for `tools/xs-language-server/**`.
