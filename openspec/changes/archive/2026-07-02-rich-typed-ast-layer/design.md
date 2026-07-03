# Design — `2026-07-02-rich-typed-ast-layer`

This document is a digest of the design decisions for the typed AST layer. The comprehensive design doc with full type catalogue, helper API, and per-pass rationale lives at `docs/plans/2026-07-02-typed-ast-design.md` and is the canonical reference.

---

## 1. Architectural overview

The lelwel PoC parser crate (`tools/xs-language-server/lelwel-xs/`) currently exposes:
- A `Cst<'a>` produced by `Parser::new(source, &mut diags).parse(&mut diags)`.
- A flat `Vec<Node>` indexed by `NodeRef`.
- A `Rule` enum, a `Token` enum, and a `Span = core::ops::Range<usize>`.

The new typed AST layer sits ON TOP of `Cst<'a>` and converts each `NodeRef` into a typed Rust struct via the `from_cst` extraction pattern. The CST remains the source of truth; the typed AST is a read-only view.

```
                       ┌─────────────────────────────┐
   .xs source ─► lexer ─► Cst<'a> ─► AST nodes ─► LSP features
                       └─────────────────────────────┘
                                          (Pass 1+2+3 in this change)
                                          (consumed by LSP in Phase 4 follow-up)
```

The AST layer never owns the source — it stores `Spans` that borrow from `Cst<'a>` via `&source[span.start..span.end]`.

## 2. Why rich-types, not lazy

Two patterns are common for typed ASTs on a flat CST:
1. **Lazy (C.llw):** each AST type holds a `NodeRef` and a `&Cst`. Accessor methods walk children on demand. No allocation per node.
2. **Eager (rich types):** each AST type eagerly extracts all fields at construction time. Allocations happen once per file.

We chose rich types because:
- **Format preservation** is the user-stated motivation. Format-preserving output needs spans of every wrapper token at every AST level. With lazy extraction, the formatter walks the raw CST anyway — defeating the purpose of a typed AST.
- **Direct field access** (`param.t_ref`, `param.ty`, `param.ident`) is faster on the hot path than repeated `cst.children()` walks.
- **One-pass extraction** is a single source of truth: if extraction succeeds, every accessor is valid; if it fails, the caller knows the entire subtree is malformed.

The cost — one allocation per AST node — is acceptable for XS files (typically < 30k nodes).

## 3. Core types

| Type | Role |
|------|------|
| `Spanned<T>` | Value + Span — used for identifiers, literals, and one-off spans |
| `Parenthesized<T>` | `open: Span, inner: T, close: Span` — for `(expr)`, `(params)` |
| `Braced<T>` | `{ open: Span, inner: T, close: Span }` — for `{ body }` |
| `Bracketed<T>` | `[ open: Span, inner: T, close: Span }` — for `[index]`, `[]` |
| `CommaSeparatedList<T>` | `items: Vec<(T, Option<Span>)>` — trailing comma span tracked per item |
| `SemiColonSeparatedList<T>` | same shape as comma list, but with `;` separator (parity only) |
| `TokenSpan` | `token: Token, span: Span` — diagnostic positions on rules that need a specific token |

Each wrapper is `Copy + Clone + Debug + PartialEq + Eq + Hash` if T is, zero-sized if T isn't stored (e.g., `Paren`).

## 4. CST navigation helpers

Section 4 of `docs/plans/2026-07-02-typed-ast-design.md` lists four helpers in `cst_helpers.rs`:
- `child_by_rule(cst, parent, rule) -> Option<NodeRef>`
- `children_by_rule(cst, parent, rule) -> Vec<NodeRef>`
- `child_by_token(cst, parent, token) -> Option<Span>`
- `only_child(cst, parent) -> Option<NodeRef>`

These are reused across every `from_cst` body.

## 5. Span attachment strategy

Section 5 of the design doc commits to:
- Every AST struct stores `syntax: NodeRef`.
- `node.span(&cst)` computes the span on demand via `cst.span(self.syntax)`.
- No `Span` storage duplication.

The "always have a `&Cst` handy" pattern is intentional: it costs one parameter per accessor but eliminates the storage overhead and the bug class where `Spans` get out of sync.

## 6. Per-file module breakdown

| Module | ~LoC | Depends on |
|--------|------|-----------|
| `mod.rs` | 30 | (entry point) |
| `cst_helpers.rs` | 150 | parser |
| `tokens.rs` | 200 | parser |
| `spanned.rs` | 200 | tokens |
| `type_system.rs` | 300 | spanned, type_system-internal |
| `declaration.rs` | 600 | spanned, type_system, expr-placeholder |
| `top_level.rs` | 400 | declaration, preproc, statement |
| `preproc.rs` | 350 | spanned |
| `statement.rs` | 400 | declaration, expr-placeholder |
| `expr.rs` | 600 | spanned, expr-relative |
| `argument.rs` | 100 | expr |
| `parameter.rs` | 200 | declaration, expr-placeholder |
| `format.rs` | 250 | all AST modules |

Total: ~3,800 LOC across 13 modules (matches the design doc's estimate of "Pass 1 ~700 lines + Pass 2 ~800 + Pass 3 ~800 + integration tests").

## 7. Expr placeholder strategy

Three AST types reference `Expr` as a field:
- `InitDeclarator.initializer: Option<(Span, Expr)>` — `int x = 5;`
- `RegularParam.default: Option<(Span, Expr)>` — `int x = -1`
- `FunctionPointerParam.default: Option<(Span, Expr)>`
- `Argument.expr: Expr`
- `LambdaExpr`, `ConditionalExpr`, etc. (Pass 3)

To break the Pass 1 → Pass 3 dependency, **Pass 1 stores `Option<NodeRef>` (the raw `from_cst` result for an Expr context).** A helper method `initializer(&self, cst: &Cst) -> Option<(Span, Expr)>` is added in Pass 3 that performs the Expr extraction on demand. This keeps the API stable while Pass 1 ships without Pass 3 in place.

Alternative considered: define `enum Expr {}` as a placeholder and use `unimplemented!()` on the extraction. Rejected — the placeholder approach is stable across Pass 1-2-3, with no `unimplemented!` markers in production code.

## 8. Test strategy

Each AST type gets 2-5 unit tests in `#[cfg(test)] mod tests`. Tests:
- Parse a synthetic source.
- Extract via `from_cst`.
- Assert structural shape (which enum variant, which fields populated).
- Assert spans are correct.
- For wrapper types, assert round-trip preservation.

The proof-of-concept formatter test in `format.rs`:
- Uses the test source from `examples/test_parse.rs` as one fixture.
- Adds 5 hand-selected retail XS files from `<game_path>/game/ai/`.
- Asserts `format_output == source` byte-for-byte.

## 9. Out of scope

Section 5 of `proposal.md` lists the out-of-scope items: LSP wiring, lifetime interning, production formatter, and the `?t` grammar quirks. This section is just a reminder that completion of this change does not require those follow-ups.

## 10. References

- Design doc (canonical): `docs/plans/2026-07-02-typed-ast-design.md`
- Migration plan (parser-side): `docs/plans/2026-07-01-migrate-tree-sitter-to-lelwel.md` and `openspec/changes/archive/xs-language-server/`
- Grammar source of truth: `tools/xs-language-server/lelwel-xs/src/xs.llw`
- Generated API (read-only reference): `target/debug/build/xs-parser-*/out/generated.rs`
- PoC placeholder (to be replaced): `tools/xs-language-server/lelwel-xs/src/ast/mod.rs`
