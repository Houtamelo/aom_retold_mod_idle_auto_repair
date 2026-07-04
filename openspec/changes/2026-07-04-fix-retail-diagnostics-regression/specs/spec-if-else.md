# Spec: `else` branch in `if` statements

## What

Extend the `if_statement` rule in `xs.llw` so that `if (cond) statement else statement` parses, including `else if` chains and empty-branch forms.

## Why

`xs.llw:525` currently defines `if_statement` as `'if' parenthesized_expression statement`, and lines 509-523 explicitly document the missing `else` as a known limitation. Every retail `else` (2,269 occurrences) therefore parses as an unexpected top-level token and triggers large recovery cascades. Impact estimate after comments are fixed: **~15,000–20,000 diagnostics** (see `explore.md`, "Cause 2").

## Requirements

| ID | Description |
| --- | --- |
| R-ELSE-01 | `if (cond) statement` must continue to parse as before. |
| R-ELSE-02 | `if (cond) statement else statement` must parse successfully. |
| R-ELSE-03 | `if (cond) statement else if (cond) statement` chains must parse. |
| R-ELSE-04 | `if (cond) {} else {}` empty-branch form must parse. |
| R-ELSE-05 | The typed AST must represent the optional `else` branch in the `If` statement variant. |
| R-ELSE-06 | If lelwel's LL(1) check rejects a direct optional `'else' statement`, implement the matched/unmatched statement split to resolve dangling-else ambiguity. |

## Scenarios

| ID | Given | When | Then |
| --- | --- | --- | --- |
| S-ELSE-01 | `void f() { if (a) {} else {} }` | parsed with `xs_parser` | zero diagnostics; AST has both `then` and `else` branches. |
| S-ELSE-02 | `void f() { if (a) {} else if (b) {} else {} }` | parsed with `xs_parser` | zero diagnostics; the chain is a single nested `if` in the final `else`. |
| S-ELSE-03 | `void f() { if (a) b(); else c(); }` | parsed with `xs_parser` | `b()` is in the `then` branch and `c()` is in the `else` branch. |
| S-ELSE-04 | `void f() { if (a) { b(); } else { c(); } }` | parsed with `xs_parser` | spans of `then` and `else` blocks match the source ranges. |

## Out of scope

Semantic analysis of dangling-else binding (only syntactic parsing and AST shape is required), and changes to the ternary conditional `?:` operator.

## Files affected

- `tools/xs-language-server/xs-parser/src/xs.llw` (lines 509-525, `if_statement` rule)
- `tools/xs-language-server/xs-parser/src/ast/statement.rs` (`If` variant / typed AST shape)
