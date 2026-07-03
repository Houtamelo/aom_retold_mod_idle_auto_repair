# Typed AST Layer Specification

## Capability summary

The XS LSP parser crate (`xs-parser` at `tools/xs-language-server/lelwel-xs/`) SHALL expose a typed AST layer under `src/ast/` that converts the lelwel-generated `Cst<'a>` into per-rule Rust structs, with wrapper-token types preserving formatter-required spans. The AST MUST support extraction by `from_cst(cst, NodeRef) -> Option<Self>` per type, MUST be robust to ERROR subtrees, and SHALL cover every rule in the current `src/xs.llw` grammar.

## Rationale

The LSP currently walks `tree_sitter::Cursor` and re-derives structure on every feature (symbol extraction, hover, completion, etc.). A typed AST gives every LSP feature a single shared data model: a feature depends on `Declaration::declarators` rather than re-walking raw syntax nodes. Wrapper-token types (`Paren`, `Brace`, `Comma`) preserve the punctuation positions needed for format-preserving output, which the user explicitly requested.

## Grammar source of truth

All AST struct definitions MUST match `tools/xs-language-server/lelwel-xs/src/xs.llw`. If the grammar changes, the AST layer's specs MUST be re-checked against the new rule names and shapes.

## Storage strategy

The first three passes use owned data:
- `Identifier = Spanned<String>`
- `StringLiteral.value = String` (raw lexeme with quotes)
- `IntLiteral.value = String` (raw lexeme, parse on demand)
- `FloatLiteral.value = String` (raw lexeme, parse on demand)

Lifetime introduction (`&'src str`) is deferred to a follow-up change once the LSP signals what it needs to hold across requests.

## Scenarios

### Scenario: AST extraction from a clean source
- GIVEN an XS source file with no syntax errors
- WHEN the LSP calls `TranslationUnit::from_cst(&cst, NodeRef::ROOT)`
- THEN extraction returns `Some(_)` with the full hierarchy populated
- AND every node carries a `Span` representing the first-to-last token range in source order

### Scenario: AST extraction with errors
- GIVEN an XS source file with the 15 pre-existing `?t`-induced diagnostics
- WHEN the LSP calls `from_cst()` on any rule whose subtree contains an ERROR
- THEN the method MUST return `None` rather than `panic!` or return a partial struct
- AND the caller can choose to fall back on the raw `Cst` for that node

### Scenario: Wrapper-token span preserved
- GIVEN a declaration `int x = 5;` (tokens at offsets `[0..3]`, `[4..5]`, `[7..8]`, `[11..12]`)
- WHEN the AST is extracted
- THEN `Declaration.semi` equals the span of `;` (`[11..12]`)
- AND `Declaration.span` equals `[0..12]` (covering `int x = 5;` inclusive)

### Scenario: Format-preserving round-trip
- GIVEN an XS source file (e.g. `examples/test_parse.rs` contents or one retail file)
- WHEN the AST is extracted and the proof-of-concept formatter emits it
- THEN the formatter output is byte-for-byte identical to the input source
- (This is the success criterion for Pass 3. Demonstrates the wrapper-token scheme is sound.)

### Scenario: Span on every node
- GIVEN any AST struct that wraps a tree node
- WHEN the LSP needs the source range for a diagnostic
- THEN `node.span(&cst)` returns a valid `Span` referencing the node's first-to-last token
- AND the LSP does not need to store spans separately

### Scenario: Multi-element declarators with mixed initializers
- GIVEN `int x, y = 2, z = 3;`
- WHEN extracted to AST
- THEN `InitDeclaratorList.items` has 3 elements
- AND `items[0].initializer == None` (x has no initializer)
- AND `items[1].initializer == Some((Span, Expr))` (y has `= 2`)
- AND `items[2].initializer == Some((Span, Expr))` (z has `= 3`)

### Scenario: Comma-separated list separator preservation
- GIVEN a parameter list `(int x, int y, int z)` (no trailing comma)
- WHEN extracted to AST
- THEN `CommaSeparatedList.items` has 3 elements
- AND `items[0].1 == Some(span_of_comma_1)`
- AND `items[1].1 == Some(span_of_comma_2)`
- AND `items[2].1 == None` (no trailing comma)

## XS-engine constraints

- The AST layer is read-only from the LSP perspective. It does not write to `xs.llw`, the lexer, the parser, or any source file.
- The AST layer is pure: every `from_cst` is deterministic and side-effect-free.
- The AST layer must not depend on the engine API or any game-side knowledge.

## Non-functional requirements

- **Performance:** `from_cst` extraction over a 5,000-line XS file MUST complete in < 50ms on a modern developer laptop.
- **Allocation:** owned `String`s are acceptable in the first three passes. Lifetime interning is deferred.
- **Compilation:** `cargo build` must remain green at every task boundary in the tasks list. No task ships without a green build.

## Open questions deferred

1. Lifetime introduction (`&'src str` for identifiers and string literals).
2. Trait-based dispatch (`AstNode` trait) — start without; add only if generic walks become valuable.
3. Production formatter — separate change once this layer proves sound.
