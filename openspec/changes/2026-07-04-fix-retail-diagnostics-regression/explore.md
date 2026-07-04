# Explore: Retail-file Diagnostic Regression

## Summary

- **Total ERROR diagnostics**: 181,473 across the parseable retail `.xs` files
- **Files affected**: 300 of 302 parseable text files (the remaining ~20 `.xs` entries under `random_maps/` are binary/serialized data and are skipped by the LSP test)
- **Threshold overrun**: 105× the asserted cap of 1,730
- **Top diagnostic message**: `"invalid syntax"` — 160,074 instances (88.2%)
- **Cascade multiplier estimate**: ~9.5× overall (181,473 total / ~19,048 independent decision-point diagnostics). After stripping comments in a controlled replay, the multiplier falls to ~5.4× (27,370 / ~5,106).

Raw data was dumped to `/tmp/diag_dump.txt` and `/tmp/diag_dump_stripped.txt`. The "stripped" replay replaced `// ...` and `/* ... */` with whitespace so the parser no longer sees comment markers as tokens.

## Top diagnostic message categories

| # | Message | Count | Pct | Files affected | Root cause |
|---|---|---|---:|---:|---|
| 1 | `invalid syntax` | 160,074 | 88.2% | ~300 | Cascade token-consume during error recovery |
| 2 | `invalid syntax, expected one of: 'bool', 'break', 'class', 'const', ...` (top-level first set) | 18,692 | 10.3% | ~299 | Parser resync at top-level boundary because the previous construct could not be parsed |
| 3 | `invalid token` | 2,351 | 1.3% | ~190 | Lexer emits `Token::Error` for characters it does not recognize |
| 4 | `invalid syntax, expected one of: 'default', '!', 'false', ... 'vector'` (expression first set) | 130 | 0.07% | ~60 | `for (int i = 0; ...)` condition/init slot misparsed (Limitation 2A) |
| 5 | `invalid syntax, expected one of: '=', ',', '{', '(', ')', ';'` | 111 | 0.06% | ~40 | Recovery inside class/declarator contexts |
| 6 | `invalid syntax, expected one of: 'const', 'extern', <identifier>, '(', 'mutable', 'ref', 'static'` | 51 | 0.03% | ~30 | Declaration-specifier recovery |
| 7 | `invalid syntax, expected one of: 'bool', 'const', 'extern', 'float', <identifier>, 'int', 'mutable', '}', 'ref', 'static', 'string', 'vector', 'void'` | 40 | 0.02% | ~20 | Class member recovery |
| 8 | `invalid syntax, expected one of: '=', ';'` | 16 | 0.01% | ~10 | Field/declaration without following `;` or direct `for`-init mismatch |
| 9 | `invalid syntax, expected one of: <identifier>, '('` | 2 | <0.01% | 2 | Function declarator edge cases |
| 10 | `invalid syntax, expected: <identifier>` | 2 | <0.01% | 2 | Preprocessor `#define`/class edge cases |
| 11 | `invalid syntax, expected one of: '=', ',', ')'` | 2 | <0.01% | 2 | Parameter-list recovery |
| 12 | `invalid syntax, expected one of: 'const', 'extern', <identifier>, '{', '[', '(', 'mutable', 'ref', 'static'` | 1 | <0.01% | 1 | Declaration specifier edge case |
| 13 | `invalid syntax, expected: '{'` | 1 | <0.01% | 1 | Class/rule body edge case |

## Cascade analysis

- **Independent-error count estimate**: ~19,048 (the `expected ...` diagnostics plus `invalid token` diagnostics emitted at the decision points where the parser gives up).
- **Cascade-error count estimate**: ~162,425 (the plain `"invalid syntax"` tokens consumed while the parser resynchronizes).
- **Overall cascade multiplier**: ~181,473 / ~19,048 ≈ **9.5×**.
- **After comment stripping**:
  - Total ERRORs: 27,370
  - Decision-point / expected diagnostics: ~4,852 long top-level resyncs + 167 `invalid token` + smaller expected variants = ~5,106
  - Plain cascade `"invalid syntax"`: 22,264
  - Cascade multiplier: ~5.4×

The cascade is driven by how the generated lelwel parser recovers: when a construct such as an `if/else`, a `for (int ...)` loop, or a class body fails, the parser calls `advance_with_error` repeatedly until it finds a token from the parent FOLLOW set (usually a top-level keyword like `void`, `int`, `rule`, etc.). Each consumed token becomes one `"invalid syntax"` diagnostic. Fixing the **root failure** therefore removes many downstream ERRORs, not just one.

## Root cause mapping

### Cause 1: Missing comment lexer regexes (NEW — not in listed Limitations)

- **What**: `tools/xs-language-server/xs-parser/src/lexer.rs` declares `LineComment` and `BlockComment` tokens but never attaches `#[regex(...)]` patterns to them. As a result the lexer does not skip `//` or `/* ... */`; comment markers become `/`, `*`, `=` and other unrecognised tokens, and the parser immediately loses synchronisation.
- **Evidence**: A minimal `"// comment\nvoid f() {}\n"` input produces an ERROR diagnostic. After stripping every comment in every retail file to whitespace and re-parsing, total ERRORs drop from **181,473 to 27,370** (a **154,103-error reduction**, 85%).
- **Limitation mapping**: NEW
- **Affected diagnostics**: ~154,103 (85%)
- **Fix strategy**: Add Logos regexes for `//` line comments and `/* ... */` block comments in `lexer.rs` and keep them in the `skip` set. Preserve newlines so line numbers for downstream diagnostics do not drift.
- **Fix effort**: S
- **Estimated diagnostic reduction**: ~154,000

### Cause 2: Dangling `else` not implemented (NEW grammar limitation)

- **What**: `xs.llw:525` defines `if_statement` as `'if' parenthesized_expression statement` with no `else` branch. The file comment at `xs.llw:509-523` explicitly calls this out as a "KNOWN LIMITATION". Retail XS makes heavy use of `if ... else` and `if ... else if ...` chains, so every `else` is parsed as an unexpected top-level token.
- **Evidence**: After stripping comments, the most common first expected-error lines are `SRC> }`, `SRC> else`, and `SRC> else if (...)`. The retail tree contains **2,269 `else` occurrences**.
- **Limitation mapping**: NEW (not listed as 1A/1B/2A/2B/4/6)
- **Affected diagnostics**: Hard to isolate exactly, but dominates the remaining ~27k after comments are fixed. Micro reproduction: `void f() { if (a) {} else {} }` produces 6 ERRORs; with larger function bodies each `else` cascades through many tokens.
- **Fix strategy**: Extend `if_statement` to `'if' parenthesized_expression statement ['else' statement]` in `xs.llw`, or implement the matched/unmatched statement split if lelwel's LL(1) check requires it. This is a pure grammar change.
- **Fix effort**: M
- **Estimated diagnostic reduction**: ~15,000–20,000

### Cause 3: `for (int i = 0; ...)` not supported (Limitation 2A)

- **What**: `xs.llw:550` defines `for_init: [expression]`, so a declaration in the `for` initializer is rejected. The grammar comment at `xs.llw:544-548` flags this as Limitation 2A.
- **Evidence**: Retail files contain **1,118 occurrences** of `for (int|bool|float|string|vector ...`. A minimal reproduction produces 8 ERRORs. After comment stripping, the direct expected message tied to the for condition slot appears 49 times, but the generic cascade tokens from each failed loop are much larger.
- **Limitation mapping**: 2A
- **Affected diagnostics**: ~5,000–10,000 (estimated from 1,118 occurrences × 4–9 cascade tokens each)
- **Fix strategy**: Allow `declaration_specifiers init_declarator_list` as a `for_init` alternative. This overlaps with the type-vs-expression ambiguity, so it will need the `ParserCallbacks` / `TypeTable` predicate (Limitation 6) for clean LL(1) disambiguation.
- **Fix effort**: M
- **Estimated diagnostic reduction**: ~6,000–9,000

### Cause 4: Bitwise `&` and `|` not tokenized (NEW)

- **What**: `xs.llw:96` deliberately omits single `&` and `|` tokens to avoid unused-token warnings. However retail scripts use bitwise AND/OR (`cVictoryTypesCurrent & cVictoryTypeRegicide`, etc.).
- **Evidence**: After stripping comments there are **167 `invalid token` diagnostics**; inspection shows they point at `&` and `|` characters. Retail counts: ~170 single `&` uses, ~83 single `|` uses.
- **Limitation mapping**: NEW
- **Affected diagnostics**: 167 direct invalid-token errors plus downstream cascade within the expression; estimate <1,000.
- **Fix strategy**: Add `Amp='&'` and `Pipe='|'` tokens to `xs.llw` and `lexer.rs`, then add them to `binary_expr` at an appropriate precedence level (between equality and additive, matching typical C semantics).
- **Fix effort**: S
- **Estimated diagnostic reduction**: ~200–1,000

### Cause 5: Function-pointer-typed variable declarations (Limitation 1B)

- **What**: Forms like `void(int) callback = ...;` are not parsed by `declaration`. The grammar comment at `xs.llw:354-359` and `xs.llw:475-482` documents this as Limitation 1.
- **Evidence**: Only **12 occurrences** in the retail tree match the pattern `type '(' ... ')' name '='`. The analyzer's expected-error sample contained 3 function-pointer-variable examples.
- **Limitation mapping**: 1B
- **Affected diagnostics**: <100
- **Fix strategy**: Add a function-pointer type alternative to `declaration_specifiers` / `type_specifier` or extend `declarator` to capture `void(int) foo`. This may need a ParserCallbacks predicate to resolve the ambiguity with function forward declarations.
- **Fix effort**: M
- **Estimated diagnostic reduction**: <100

### Cause 6: User-defined type local declarations without TypeTable (Limitation 6)

- **What**: Inside function/rule bodies, `block_item` uses `predicate_block_item_1` to decide whether an identifier starts a declaration. The predicate only knows primitive types unless the caller seeds `TypeTable` with class names. The diagnostics path in `tools/xs-language-server/lsp/src/diagnostics.rs:29` calls `TypeTable::with_primitives()` with no class names, so local declarations like `BOSystem myBO = ...;` are rejected.
- **Evidence**: ~235 lines match a `CapitalizedType name = ...` pattern.
- **Limitation mapping**: 6
- **Affected diagnostics**: Few hundred at most in the current parser; the LSP could eliminate most by passing workspace class names into `collect_diagnostics`.
- **Fix strategy**: Wire the workspace / symbol extraction into `collect_diagnostics` so `TypeTable` is seeded with known class names. This is an LSP change, not a grammar change.
- **Fix effort**: M
- **Estimated diagnostic reduction**: ~200–500

### Cause 7: Preprocessor directives (handled, not a top contributor)

- **What**: `xs.llw:180-184` models `#if`, `#elif`, `#else`, `#endif`, and `#define`. Retail counts are 235 `#if`, 27 `#define`, and **0 `#ifdef`/`#ifndef`**. The native `include "..."` directive parses without errors in isolation.
- **Evidence**: After comment stripping, almost no expected errors land on `#if`/`#define` lines; the analyzer's `preprocessor` bucket was empty in the first-error sample.
- **Limitation mapping**: Not a regression driver
- **Affected diagnostics**: <100
- **Fix strategy**: None required for retail; keep current coverage.
- **Fix effort**: —
- **Estimated diagnostic reduction**: —

## Proposed fix strategy (ordered by impact / effort)

1. **(Highest impact, lowest effort) Add comment regexes** — expected to remove ~154,000 diagnostics. Single file change in `lexer.rs`; must keep newline/offset fidelity for LSP ranges.
2. **(High impact, medium effort) Implement `else` in `if_statement`** — expected to remove ~15,000–20,000 of the remaining ~27,000. Pure grammar change in `xs.llw`; revisit if lelwel requires the matched/unmatched split.
3. **(Medium impact, medium effort) Support `for (int i = 0; ...)`** — expected to remove ~6,000–9,000. Needs grammar change plus `ParserCallbacks`/`TypeTable` predicate (hooks Limitation 6).
4. **(Low impact, low effort) Add `&` and `|` tokens/operators** — expected to remove ~200–1,000 invalid-token cascades.
5. **(Low impact, medium effort) Wire TypeTable class names into LSP diagnostics** — removes remaining user-type local-declaration errors.
6. **(Low impact, medium effort) Support function-pointer-typed variables** — mainly for parity / mod files; retail count is tiny.
7. **(Insurance) Downgrade honest limitations**: If any known unsupported construct remains after the grammar fixes, surface it as `WARNING` rather than `ERROR` in the LSP so the 1,730 threshold is not breached by design gaps.

## Open questions for propose phase

- Comment regexes: should the LSP preserve comment tokens for documentation/semantic purposes, or is pure skip sufficient? Skip is enough for diagnostics but may matter for formatter round-trip tests.
- `else` branch: does lelwel accept an optional `'else' statement` directly on `if_statement`, or do we need the full matched/unmatched statement split to avoid LL(1) conflicts?
- `for (int ...)` declaration form: do we extend `for_init` to a full `declaration_specifiers init_declarator_list`, or a restricted single-declarator form? The predicate must distinguish `for (int i = ...)` from `for (foo = ...)`.
- Should the LSP pass workspace class names into `collect_diagnostics` now, or wait until the grammar supports local class-type declarations cleanly?
- Downgrade policy: which remaining unsupported constructs (if any) are acceptable to emit as `WARNING` instead of `ERROR` after the fixes are applied?
- Do any retail files actually use `#include` or `#ifdef`? Our grep found zero, but a second pass with case-insensitive / preprocessor-aware matching would confirm.

## Files referenced

- `tools/xs-language-server/xs-parser/src/lexer.rs` — missing `LineComment`/`BlockComment` regexes
- `tools/xs-language-server/xs-parser/src/xs.llw` — grammar rules for `if_statement` (line 525, limitation 509-523), `for_init` (line 550, limitation 544-548), `type_specifier`/declarator limitations (lines 354-359, 475-482), and operator tokens (line 101)
- `tools/xs-language-server/xs-parser/src/parser.rs` — `ParserCallbacks` implementation and `predicate_block_item_1`
- `tools/xs-language-server/xs-parser/src/ast/type_table.rs` — `TypeTable` used by the predicate
- `tools/xs-language-server/lsp/src/diagnostics.rs` — maps parser `Severity::Error` to LSP `DiagnosticSeverity::ERROR` and currently passes only primitive types to the parser
- `tools/xs-language-server/lsp/tests/game_folder_parse.rs` — threshold test that fails with the real game path

## Method notes

The raw diagnostic dump was produced by a temporary Rust binary that calls `xs_parser::parser::Parser::new_with_context(source, &mut diags, TypeTable::with_primitives()).parse(&mut diags)` for every `.xs` file under `~/.steam/steam/steamapps/common/Age of Mythology Retold/game/`. Messages were grouped with `sort | uniq -c`. The comment-stripped replay used a naive string-literal-aware scrubber that replaced comment bodies with spaces, preserving line structure.
