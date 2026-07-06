# Format Preservation Specification

## Capability summary

The typed AST layer SHALL preserve, for every derived `String`-or-list wrapper (`Parenthesized<T>`, `Braced<T>`, `Bracketed<T>`, `CommaSeparatedList<T>`, `SemiColonSeparatedList<T>`, `TokenSpan`, `Paren`, `LBrace`, etc.), the `Span`s of the wrapping punctuation tokens. The proof-of-concept formatter in Pass 3 SHALL demonstrate that these preserved spans are sufficient to round-trip a representative XS file byte-for-byte, including trailing-comma placement and intra-line whitespace.

## Rationale

The user explicitly stated: *"I do actually want to support formatting."* Format preservation is part of the AST layer's value from day one — without wrapper-token spans, a separate format-pass over the raw `Cst` would be needed, doubling the format code. By preserving the spans inside the AST, the formatter becomes a pure traversal that emits source as-is from the stored spans.

## Design

Each wrap type stores:
- `open: Span` (the `(`, `{`, or `[` token)
- `inner: T`
- `close: Span` (the `)`, `}`, or `]` token)

Each list type stores items paired with the trailing separator span:
- `items: Vec<(T, Option<Span>)>`

The trailing comma is `None` for the last item.

The formatter's strategy: iterate the AST depth-first, emitting `&source[span.start..span.end]` slices as it walks. No whitespace normalization, no comment rewriting, no reordering — pure passthrough.

## Scenarios

### Scenario: parenthesized wrap round-trips
- GIVEN `int(int) callback = ...;`
- WHEN the formatter emits `FunctionPointerParam.fn_type.1.open` and `.close`
- THEN the emitted bytes are `(` and `)` at the same source offsets as the original

### Scenario: comma-separated list round-trips
- GIVEN `int x, int y, int z` (no trailing comma)
- WHEN the formatter iterates `ParameterList.items`
- THEN the emitted bytes exactly match the input: `x`, `, `, `y`, `, `, `z`

### Scenario: trailing comma preserved
- GIVEN `(int x, int y,)` (trailing comma — engine tolerates this for argument lists in some cases)
- WHEN the formatter iterates `ArgumentList.items`
- THEN the trailing comma span is included in the output

### Scenario: round-trip on real XS source
- GIVEN the 17-line test source in `examples/test_parse.rs`
- WHEN the proof-of-concept formatter runs `format_source(source) -> String`
- THEN `format_source(source) == source` byte-for-byte

### Scenario: round-trip on retail XS file
- GIVEN 5 hand-selected retail XS files totaling < 5,000 lines
- WHEN the formatter runs each file
- THEN each file's `format_output == file_source` byte-for-byte
- (Validation step at end of Pass 3.)

## Limitations

- The proof-of-concept formatter does not handle leading/trailing newlines, indentation policy, or comment attachment. Those are deferred to a production-formatter change.
- The PoC expects single-byte UTF-8 source for the round-trip tests (no CRLF or BOM normalization). This matches the AoM:R engine's input expectations.

## Open questions

- Comment preservation: the design does not store comment spans. If a future formatter must preserve comments, Pass 3 will need a follow-up to attach `LineComment`/`BlockComment` tokens to nearby AST nodes. Not in scope for this change.
