# SDD Proposal — `2026-07-02-rich-typed-ast-layer`

**Status:** Ready for tasks
**Project:** `aom_retold_mod_idle_auto_repair`
**Change name:** `2026-07-02-rich-typed-ast-layer`
**Branch:** `xs-lsp` (work happens in the scratch subdir `lelwel-xs/` which is gitignored — will be moved to the LSP source tree in Phase 4 below)

---

## 1. Title and summary

Build a typed AST layer for the new lelwel-generated XS parser, replacing the ad-hoc tree-sitter `Cursor` walks the LSP currently relies on. The typed AST will give every LSP feature (symbol extraction, completion, hover, definition, rename, formatting) a stable, typed data model that survives future grammar or lexer changes. The design chosen is the "rich-types" pattern (eager extraction + wrapper tokens) — the user explicitly requested this over the C.llw lazy pattern so the layer can power a future formatter from day one.

## 2. Why

- **User-facing problem:** The XS LSP currently navigates XS files via `tree_sitter::Cursor` walks. This forces every feature to re-derive structure from raw syntax nodes — symbol extraction duplicates declaration parsing, hover duplicates identifier resolution, and tooling changes break the codebase in 5+ unrelated places simultaneously. There is no shared typed-AST abstraction.

- **Why now:** The lelwel migration (see `2026-06-30-xs-language-server` archive + `openspec/changes/archive/xs-language-server/`) is producing a new `Cst<'a>` parser. The old tree-sitter `Cursor` is going away. We must replace it with a typed AST before any LSP feature can be ported, or the port ends up doing the same fragile raw-Cst walks the user explicitly wants to leave behind.

- **Why rich-types over lazy:** The user stated: *"I do actually want to support formatting."* Format preservation needs token positions in the AST — this is naturally satisfied by wrapper-token types (`Paren`, `Brace`, `Comma`) carrying spans alongside the inner data. The lazy C.llw pattern would have required a separate format-pass over the raw CST, doubling the format-preservation work.

## 3. What changes

| Area | Change |
|------|--------|
| `tools/xs-language-server/lelwel-xs/src/ast/` | New module hierarchy: `cst_helpers.rs`, `tokens.rs`, `spanned.rs`, `type_system.rs`, `declaration.rs`, `top_level.rs`, `preproc.rs`, `statement.rs`, `expr.rs`, `argument.rs`, `parameter.rs`, `format.rs` |
| `tools/xs-language-server/lelwel-xs/src/ast/mod.rs` | Replace placeholder (~73 lines) with module declarations + core re-exports |
| `tools/xs-language-server/lelwel-xs/src/lib.rs` | No change beyond `pub mod ast;` (already there) |
| `tools/xs-language-server/lsp/src/symbols.rs:557` | After Pass 3, delete the ERROR-recovery helper `extract_error_function_definition` — typed AST distinguishes declarations from statements |
| `tools/xs-language-server/lsp/src/symbols.rs:485` | After Pass 3, remove the silent `None => return` that drops no-init declarations — typed AST exposes initializer-presence per declarator |
| `examples/test_parse.rs` | After Pass 3, add a formatter round-trip test that demonstrates the AST preserves source byte-for-byte on the existing test source |

The grammar (`src/xs.llw`) already has the migration changes from `2026-06-30-xs-language-server`. The AST layer operates on the existing `Cst<'a>` and does not require grammar changes.

## 4. What does NOT change

- `tools/xs-language-server/tree-sitter-xs/` — old tree-sitter parser; removed in `2026-06-30-xs-language-server` follow-up, not here.
- `tools/xs-language-server/lsp/` Rust LSP handlers — out of scope for this change; will consume the typed AST once Pass 3 lands. See Phase 4 in the tasks list.
- `tools/intellij-xs-plugin/` — Kotlin client; no awareness of the AST layer. Reaches the LSP via standard LSP messages.
- `mod/*` XS sources and `scripts/deploy-mods.sh` — product code; untouched.
- The grammar (`xs.llw`) — operated on read-only by this change. The 15 pre-existing spurious diagnostics on `examples/test_parse.rs` (caused by `?t` quirks in `block_item^` function-body parsing) are unrelated and remain as a follow-up.

## 5. Scope

### 5.1 In scope

- The typed AST layer under `tools/xs-language-server/lelwel-xs/src/ast/`.
- Wrappers for token streams: `Parenthesized<T>`, `Braced<T>`, `Bracketed<T>`, `CommaSeparatedList<T>`, `SemiColonSeparatedList<T>`, `Spanned<T>`, `TokenSpan`.
- CST navigation helpers: `child_by_rule`, `children_by_rule`, `child_by_token`, `only_child`.
- All ~85 rule variants from `xs.llw` get a typed Rust struct with `from_cst(cst, NodeRef) -> Option<Self>` and (where meaningful) accessor methods.
- A proof-of-concept formatter in Pass 3 that round-trips a single XS file byte-for-byte, demonstrating the wrapper-token scheme works.
- Unit tests for every helper and AST type.
- Documentation: this change document + the design doc at `docs/plans/2026-07-02-typed-ast-design.md` (the latter is the formal design artifact).

### 5.2 Out of scope

- Wiring the typed AST into LSP handlers (`tools/xs-language-server/lsp/src/`). This is a follow-up change once all three passes of the AST layer are complete and green.
- A production formatter. The Pass 3 formatter proves the design; a real formatter must decide on indentation, comment preservation, line-break policy — these are deferred to a future change.
- Resolving the 15 pre-existing `?t` grammar quirks (Limitation 6 in `2026-06-30-xs-language-server`). The AST layer works around them by extracting structure from whatever subset of the tree the parser actually produces.
- Lifetime-based string interning. The first three passes use owned `String`s; lifetime introduction happens in a follow-up if profiling reveals allocation pressure.
- Removing the `tools/xs-language-server/tree-sitter-xs/` directory. That removal is part of the `2026-06-30-xs-language-server` change's follow-up, not here.

## 6. Constraints and risks

| Risk | Mitigation |
|------|-----------|
| Grammar shapes do not match the design doc's expectations | Specs are written against `xs.llw` as ground truth; deviations go through spec updates, not silent workarounds |
| Cross-module type dependencies cause cascading compile errors | Build the crate end-to-end after each module (`cargo build`); Pass 1 ships first because Pass 2/3 depend on it |
| AST extraction is slow on large files | Eager extraction allocates once per file; benchmark before locking in the design. Currently expected < 50ms for typical XS files (< 5k lines) |
| The PoC grammar produces 15 spurious diagnostics on real XS files | AST extraction skips ERROR subtrees; diagnostic count is unaffected |
| Token/rule names clash between wrapper types (e.g., `Bracket`) and the lelwel `Token::Bracket` enum | Wrapper types live in `tokens.rs` and re-export through `mod.rs`; no clash because lelwel exports as `Token::LBrak` / `RBrak` (no `Bracket` in generated) |

## 7. Rollback plan

The typed AST is a new module in `lelwel-xs/`. Until the LSP wires it in (Phase 4 follow-up), nothing depends on it. Reverting means:
1. `rm -rf tools/xs-language-server/lelwel-xs/src/ast/` — delete the new module.
2. Restore `lelwel-xs/src/ast/mod.rs` to the previous ~73-line placeholder.
3. `cargo build` — the PoC's `examples/test_parse.rs` continues to work against the raw `Cst<'a>` parser, as before.

No on-disk product code (`mod/*.xs`), no deployment artifacts, no LSP wiring change. The cost of rollback is < 5 minutes.

## 8. Success criteria

- `cargo build` succeeds with zero errors in `tools/xs-language-server/lelwel-xs/`.
- `cargo test` passes for every AST module (≥ 30 tests across 11+ files).
- The proof-of-concept formatter round-trips 5 hand-selected retail XS files byte-for-byte.
- Every AST type has at least one end-to-end test asserting `from_cst()` extracts the expected shape.
- The `from_cst()` extraction is robust to error subtrees (does not panic on malformed input).

## 9. Open questions for spec phase

1. **Identifier storage:** `Spanned<String>` (owned) for Pass 1-3 — confirmed in user direction.
2. **Trailing-comma preservation:** `CommaSeparatedList` stores `(item, Option<Span>)` per item — confirmed in design doc.
3. **Span on every node vs. computed on demand:** every node carries `NodeRef`; spans via `cst.span(self.syntax)` — confirmed in design doc.
4. **Multi-element declarators with mixed initializers:** `InitDeclaratorList { items: Vec<InitDeclarator> }` — each declarator independently tracks its initializer.

These were resolved during design. No further questions expected at spec phase.
