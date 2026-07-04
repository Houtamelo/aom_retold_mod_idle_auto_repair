# Spec: Declaration in `for` initializer

## What

Allow `for (declaration_specifiers init_declarator; cond; step) body` as an alternative to `for (expression; cond; step) body` in `xs.llw`, so loops like `for (int i = 0; i < 10; i++) {}` parse.

## Why

`xs.llw:550` defines `for_init: [expression]`, and lines 544-548 flag the declaration form as Limitation 2A. Retail scripts contain **1,118 occurrences** of `for (int|bool|float|string|vector ...`, each generating a direct parse failure and a cascade of downstream `invalid syntax` diagnostics. Estimated impact after comments and dangling-else fixes: **~6,000–9,000 diagnostics** (see `explore.md`, "Cause 3").

## Requirements

| ID | Description |
| --- | --- |
| R-FOR-01 | `for (init; cond; step) body` must continue to parse when `init` is an expression. |
| R-FOR-02 | `for (declaration_specifiers init_declarator = expr; cond; step) body` must parse. |
| R-FOR-03 | `for (;;) body` must continue to parse as an infinite loop. |
| R-FOR-04 | Disambiguation between `for (int i = ...)` (declaration) and `for (foo = ...)` (expression) must use the existing `ParserCallbacks` / `TypeTable` predicate instead of a fragile ordered-choice heuristic. |

## Scenarios

| ID | Given | When | Then |
| --- | --- | --- | --- |
| S-FOR-01 | `void f() { for (int i = 0; i < 10; i++) {} }` | parsed with `xs_parser` | zero diagnostics; `i` is scoped to the loop. |
| S-FOR-02 | `void f() { for (;;) {} }` | parsed with `xs_parser` | zero diagnostics. |
| S-FOR-03 | `void f() { for (auto var x : list) {} }` (range-for) | parsed with `xs_parser` | may fail; range-for is explicitly out of scope. |
| S-FOR-04 | `void f() { for (int i = 0; i < 10; i++) aiTask(...); }` | parsed with `xs_parser` | zero diagnostics; `i` is referenced correctly in the condition and step. |

## Out of scope

C++ range-for syntax, multi-declarator `for` initializers (`for (int i=0, j=0; ...)`), and declarations using user-defined class types without `TypeTable` seeding are out of scope for this grammar fix.

## Files affected

- `tools/xs-language-server/xs-parser/src/xs.llw` (lines 544-550, `for_init` rule)
- `tools/xs-language-server/xs-parser/src/parser.rs` (`ParserCallbacks` / `TypeTable` predicate)
- `tools/xs-language-server/xs-parser/src/ast/statement.rs` (`For` variant shape)
