# Spec: Lexer skips line and block comments

## What

Bind `#[regex(...)]` patterns to the existing `LineComment` and `BlockComment` token variants in `lexer.rs` so that `//` and `/* ... */` comments are skipped during tokenization and do not cascade into parser diagnostics.

## Why

`tools/xs-language-server/xs-parser/src/lexer.rs:200-201` declares `LineComment` and `BlockComment`, and `xs.llw:109-111` lists them as skip tokens, but neither variant has a `#[regex(...)]` attribute. Comment markers therefore lex as individual punctuation tokens (`/`, `*`, etc.) and immediately desynchronize the parser. Explore data shows that stripping comments from every retail file drops total ERROR diagnostics from 181,473 to 27,370 — a reduction of **~154,103 errors (85%)** (see `explore.md`, "Cause 1").

## Requirements

| ID | Description |
| --- | --- |
| R-COM-01 | The lexer must skip `//` line comments without producing tokens. |
| R-COM-02 | The lexer must skip `/* ... */` block comments without producing tokens. |
| R-COM-03 | Newlines inside line comments must be preserved so downstream LSP line numbers stay correct. |
| R-COM-04 | A block comment opened near EOF without a closing `*/` must recover gracefully (emit one `invalid token` diagnostic but not crash). |
| R-COM-05 | Comments must not appear in the typed AST as `Token::LineComment`/`Token::BlockComment` nodes; they are pure skip tokens. |

## Scenarios

| ID | Given | When | Then |
| --- | --- | --- | --- |
| S-COM-01 | `"// hello\nvoid f() {}"` | parsed with `xs_parser` | one function definition, zero diagnostics. |
| S-COM-02 | `"/* hello */\nvoid f() {}"` | parsed with `xs_parser` | one function definition, zero diagnostics. |
| S-COM-03 | `"/* multi\n   line */\nvoid f() {}"` | parsed with `xs_parser` | line 2 is `void f() {}` and line offsets are preserved. |
| S-COM-04 | A multi-line retail XS file such as `game/ai/campaign/fott/fott02_p2.xs` | parsed before and after the fix | diagnostics are reduced by approximately 85%. |
| S-COM-05 | `"/* unclosed block comment at EOF"` | tokenized | produces a single `invalid token` diagnostic and terminates normally. |

## Out of scope

Preserving comment text for documentation, formatter round-trips, or semantic highlighting is not required; nested block comments and C `//` line continuations are also out of scope.

## Files affected

- `tools/xs-language-server/xs-parser/src/lexer.rs` (lines 200-201, add regexes)
- `tools/xs-language-server/xs-parser/src/xs.llw` (lines 109-111, already declares skip tokens)
