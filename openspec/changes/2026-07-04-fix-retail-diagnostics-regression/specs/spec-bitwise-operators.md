# Spec: Bitwise `&` and `|` operators

## What

Add single `&` and `|` tokens to the lexer and grammar, and define them as binary expression operators at precedence between equality and additive, matching typical C semantics.

## Why

`xs.llw:96` deliberately omits `Amp` and `Pipe` tokens; the comment at lines 96-100 says bitwise And/Or are not modeled to avoid unused-token warnings. Retail scripts nevertheless use bitwise AND/OR (e.g. `cVictoryTypesCurrent & cVictoryTypeRegicide`): approximately 170 single `&` and 83 single `|` uses. After comment stripping, 167 direct `invalid token` diagnostics point at these characters, plus downstream cascades. Estimated impact: **~200–1,000 diagnostics** (see `explore.md`, "Cause 4").

## Requirements

| ID | Description |
| --- | --- |
| R-BW-01 | `&` must be a valid token (`Token::Amp` in the lexer). |
| R-BW-02 | `|` must be a valid token (`Token::Pipe` in the lexer). |
| R-BW-03 | `&` must be a binary expression operator at precedence below equality and above additive (`a == b & c + d` parses as `a == (b & (c + d))`). |
| R-BW-04 | `|` must be a binary expression operator at the same precedence level as `&`, left-associative, and lower than `||`. |
| R-BW-05 | `&&` and `||` must continue to parse as logical AND/OR with unchanged precedence. |

## Scenarios

| ID | Given | When | Then |
| --- | --- | --- | --- |
| S-BW-01 | `int x = a & b;` | parsed with `xs_parser` | zero diagnostics; `&` is the bitwise-and operator. |
| S-BW-02 | `int x = a | b;` | parsed with `xs_parser` | zero diagnostics; `|` is the bitwise-or operator. |
| S-BW-03 | `int x = a && b;` | parsed with `xs_parser` | zero diagnostics; `&&` remains logical-and. |
| S-BW-04 | Retail snippet `cVictoryTypesCurrent & cVictoryTypeRegicide` | parsed with `xs_parser` | zero diagnostics. |
| S-BW-05 | `int x = a & b | c;` | parsed with `xs_parser` | parses as `(a & b) \| c` due to equal left-associative precedence. |

## Out of scope

Bitwise XOR (`^`), compound bitwise assignments (`&=`, `|=`), shift operators (`<<`, `>>`), and any use of `&` as an address-of/reference operator are out of scope.

## Files affected

- `tools/xs-language-server/xs-parser/src/lexer.rs` (near lines 188-195, add `Amp`/`Pipe` tokens)
- `tools/xs-language-server/xs-parser/src/xs.llw` (lines 96-103 for token declarations; lines 665-673 `binary_expr` for precedence)
