# Proposal: Fix Retail-XS-File Diagnostic Regression

## Why

The typed-AST XS parser emits **181,473 ERROR-severity diagnostics across 300 of 302 parseable retail `.xs` files** in the shipped AoM:R game folder—105× over the asserted threshold of 1,730 in `tools/xs-language-server/lsp/tests/game_folder_parse.rs` (explore.md §Summary). The test only appeared to pass historically because it was run with an incorrect `AOMR_GAME_PATH` pointing to a non-existent directory, causing it to skip silently (`explore.md` §Files referenced). When the real path `/home/houtamelo/.steam/steam/steamapps/common/Age of Mythology Retold/` is supplied, the test panics at the 181K count.

The user-visible impact is that any mod developer opening a retail `.xs` file in an LSP-enabled IDE sees a wall of red squigglies, masking genuine mod-level diagnostics. Eighty-eight percent of the noise is the generic `"invalid syntax"` message, driven by a ~9.5× cascade multiplier: each independent parse failure triggers error recovery that consumes many tokens (`explore.md` §Top diagnostic message categories, §Cascade analysis).

## Root causes (from explore.md)

| # | Cause | Files affected | Diagnostic reduction estimate | Effort |
|---|-------|---------------:|------------------------------:|--------|
| 1 | Missing comment lexer regexes (`//` and `/* ... */`) | ~300 (`explore.md` §Cause 1) | ~154,000 (181,473 → 27,370 after comment stripping) | S |
| 2 | Dangling `else` not implemented in `if_statement` | ~299 (`explore.md` §Cause 2) | ~15,000–20,000 | M |
| 3 | `for (int i = 0; ...)` declaration form not supported | ~60 (direct), cascades wider (`explore.md` §Cause 3) | ~6,000–9,000 | M |
| 4 | Bitwise `&` and `|` not tokenized | ~190 (`explore.md` §Cause 4) | ~200–1,000 | S |
| 5 | `TypeTable` not seeded with workspace class names in LSP diagnostics | ~235 declarations (`explore.md` §Cause 6) | ~200–500 | M |
| 6 | Function-pointer-typed variable declarations (Limitation 1B) | 12 occurrences (`explore.md` §Cause 5) | <100 | M |
| 7 | Honest remaining limitations emitted as `ERROR` | residual after above | all that remain | S |

## Fix strategy (ordered)

### Phase 1: Comment lexer regexes (S effort, ~154K reduction)

- Single change to `tools/xs-language-server/xs-parser/src/lexer.rs`.
- Add Logos `#[regex(...)]` to `LineComment` and `BlockComment`.
- Keep them in the `skip` set.
- Preserve newlines so line numbers stay correct ("skip is enough for diagnostics" but newline/offset fidelity matters for LSP ranges; `explore.md` §Cause 1, §Open questions).

### Phase 2: `else` branch in `if_statement` (M effort, ~15-20K reduction)

- `tools/xs-language-server/xs-parser/src/xs.llw:525` — extend to `'if' parenthesized_expression statement ['else' statement]`.
- The grammar comment at `xs.llw:509-523` flags this as a known limitation (`explore.md` §Cause 2).
- May require matched/unmatched statement split to satisfy lelwel's LL(1) check (`explore.md` §Open questions).

### Phase 3: `for (int i = 0; ...)` (M effort, ~6-9K reduction)

- Extend `for_init` at `xs.llw:550` to allow `declaration_specifiers init_declarator_list` as alternative to `[expression]`.
- The grammar comment at `xs.llw:544-548` documents this as Limitation 2A (`explore.md` §Cause 3).
- Requires `ParserCallbacks` / `TypeTable` predicate for LL(1) disambiguation.

### Phase 4: Bitwise `&`/`|` (S effort, ~200-1K reduction)

- Add `Amp='&'` and `Pipe='|'` tokens to `xs.llw` and `lexer.rs`.
- Add them to `binary_expr` at appropriate precedence (between equality and additive, matching C semantics; `explore.md` §Cause 4).

### Phase 5 (deferred): Wire workspace class names into LSP diagnostics

- LSP change in `tools/xs-language-server/lsp/src/diagnostics.rs:29`.
- Pass class names to `TypeTable::with_primitives()` so local declarations like `BOSystem myBO = ...;` parse (`explore.md` §Cause 6).
- ~200-500 diagnostic reduction.

### Phase 6 (deferred): Function-pointer-typed variables (Limitation 1B)

- Pure grammar addition for `void(int) foo = ...;` form.
- Only 12 retail occurrences; <100 diagnostic reduction (`explore.md` §Cause 5).
- Improves mod support more than retail.

### Phase 7 (insurance): Downgrade remaining honest limitations to WARNING

- After phases 1-4, surface any remaining unsupported construct as `Warning` not `Error` in the LSP.
- This guarantees the 1,730 threshold cannot be breached by design gaps (`explore.md` §Proposed fix strategy #7).

## Out of scope

- Full Limitation 6 (`typedefs`, casts, complete `ParserCallbacks` wiring) — separate future change.
- Top-level compound statements (`{ ... }` as file-level construct).
- Lambdas.
- Performance optimization (typed AST vs tree-sitter timing).

## Success criteria

1. `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test game_folder_parse` (with `AOMR_GAME_PATH=/home/houtamelo/.steam/steam/steamapps/common/Age\ of\ Mythology\ Retold`) passes.
2. The actual ERROR diagnostic count is ≤ 1,730 (`explore.md` §Summary).
3. The per-file cap of 100 is not exceeded by any retail file.
4. `cargo test --lib --tests --manifest-path tools/xs-language-server/Cargo.toml` reports 252+ tests passing, no new failures.
5. The `retail_chairon_symbols` and other retail-sample integration tests remain green.

## Risks and rollback

### Risk 1: Comment regex breaks LSP ranges
- If comments are stripped at the wrong time, line numbers downstream might shift. Mitigation: ensure comments are stripped **at lex time** before tokens are produced; preserve newlines via regex (`explore.md` §Cause 1).

### Risk 2: `else` branch triggers LL(1) conflict in lelwel
- lelwel 0.10.4 may reject the simple `['else' statement]` alternative if it creates an ambiguity with subsequent `if` (matched/unmatched). Mitigation: implement the standard matched/unmatched split (`explore.md` §Open questions).

### Risk 3: `for_init` predicate requires TypeTable classes
- Disambiguating `for (int i ...)` from `for (foo ...)` requires the type/expression lookahead, which depends on `TypeTable::is_type(...)`. Mitigation: wire workspace class names into the LSP path before changing the grammar, or seed a fallback `TypeTable` in unit tests.

### Risk 4: Cascade multiplier masks incomplete fixes
- A 9.5× cascade means the threshold test might still pass at 1,730 even if only the comment regex is fixed, hiding remaining grammar gaps. Mitigation: after each phase, re-run `parse_every_game_folder_file_completes_without_unexpected_errors` and check the **actual count**, not just the threshold (`explore.md` §Cascade analysis).

### Rollback
- Each phase is a self-contained commit (small grammar + lexer change). Revert via `git revert <commit-sha>` if regressions appear. The existing 252-test unit suite is the safety net.

## Files to be modified

- `tools/xs-language-server/xs-parser/src/lexer.rs` — comment regexes (phase 1), bitwise tokens (phase 4)
- `tools/xs-language-server/xs-parser/src/xs.llw` — else (phase 2), for_init (phase 3), bitwise (phase 4)
- `tools/xs-language-server/xs-parser/src/parser.rs` — for_init predicate wiring (phase 3)
- `tools/xs-language-server/lsp/src/diagnostics.rs` — TypeTable seeding (phase 5)
- `tools/xs-language-server/lsp/tests/game_folder_parse.rs` — possibly tighten threshold after phases land
- `openspec/specs/spec-lexer-comments.md` — new (phase 1)
- `openspec/specs/spec-if-else.md` — new (phase 2)
- `openspec/specs/spec-for-init-decl.md` — new (phase 3)
- `openspec/specs/spec-bitwise-operators.md` — new (phase 4)

## Phased PR strategy

Each phase maps to a single commit. Recommend:

- Commit 1 (PR-A): Phase 1 — comment regexes. Expected: ~154K diagnostic reduction.
- Commit 2 (PR-B): Phase 2 — else branch. Expected: ~15-20K reduction.
- Commit 3 (PR-C): Phase 4 — bitwise tokens. Expected: ~200-1K reduction (do this before for_init since for_init is harder).
- Commit 4 (PR-D): Phase 3 — for_init decl. Expected: ~6-9K reduction.
- Commit 5 (PR-E): Phase 5 — workspace class names. Expected: ~200-500 reduction.
- Commit 6 (PR-F): Phase 7 — downgrade insurance. Final cleanup, no diagnostic reduction expected but guarantees safety net.
