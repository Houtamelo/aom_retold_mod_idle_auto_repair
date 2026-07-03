# Tasks: Fix rule-body extraction — 2026-07-03-fix-rule-body-extraction

| Field | Value |
|---|---|
| PR strategy | single-PR, 1 reviewer |
| Est. changed lines | ~250-300 |
| 400-line budget risk | Low |

Decision needed before apply: No
Chained PRs recommended: No
Chain strategy: size-exception
400-line budget risk: Low

## Implementation tasks

| ID | Title | Acceptance criteria | Files touched | LoC | Depends on |
|---|---|---|---|---|---|
| T-RBE-01 | Create `TypeTable` | Default seeds primitives; `is_type` works | `tools/xs-language-server/lelwel-xs/src/ast/type_table.rs` | ~50 | — |
| [x] T-RBE-02 | Wire `TypeTable` into parser | `ParserCallbacks::Context = TypeTable`; predicate compiles | `tools/xs-language-server/lelwel-xs/src/parser.rs` | ~20 | T-RBE-01 |
| [x] T-RBE-03 | Implement `predicate_block_item_1` | Token lookup via `is_type`; unknowns fall through | `tools/xs-language-server/lelwel-xs/src/parser.rs` | ~25 | T-RBE-02 |
| [x] T-RBE-04 | Update `block_item^` grammar | Uses `?1` predicate + `block_declaration_or_definition` helper | `tools/xs-language-server/lelwel-xs/src/xs.llw` | ~10 | T-RBE-03 |
| [x] T-RBE-05 | Re-export `TypeTable` | Reachable from `xs_parser::ast` | `tools/xs-language-server/lelwel-xs/src/ast/mod.rs` | ~5 | T-RBE-01 |
| [x] T-RBE-06 | Add `parse_with_types` helper | Builds CST with supplied `TypeTable` | `tools/xs-language-server/lelwel-xs/src/ast/top_level.rs` | ~10 | T-RBE-02, T-RBE-04 |
| [x] T-RBE-07 | Add rule-body unit tests | S-RBE-01..S-RBE-08 pass | `tools/xs-language-server/lelwel-xs/src/ast/top_level.rs` | ~120 | T-RBE-05, T-RBE-06 |
| [x] T-RBE-08 | Keep existing tests green | `cargo test` runner green (159 + 8 tests) | — | 0 | T-RBE-07 |
| [x] T-RBE-09 | Verify build and retail sample | `cargo build` green; sample has zero rule ERRORs | — | 0 | T-RBE-08 |

## Dependency graph

```text
T-RBE-01 ─┬─ T-RBE-02 ─ T-RBE-03 ─ T-RBE-04 ─┐
          └─ T-RBE-05 ────────────────────────┤
                                              ▼
                          T-RBE-06 ─ T-RBE-07 ─ T-RBE-08 ─ T-RBE-09
```

## Test plan

| Scenario | Input | Expected result |
|---|---|---|
| S-RBE-01 | `rule r { }` | 0 body items |
| S-RBE-02 | `rule r { x; }` | 1 statement |
| S-RBE-03 | `rule r { int x=1; }` | 1 declaration |
| S-RBE-04 | `rule r { MyClass x; }` with `MyClass` | 1 declaration |
| S-RBE-05 | `rule r { int x=1; x; }` | decl + statement |
| S-RBE-06 | `<100` LoC of `economic_units.xs::deleteExcessGatherers` | zero rule-level ERRORs |
| S-RBE-07 | `rule r { int x; }` with empty table | statement |
| S-RBE-08 | `rule r { MyClass x; }` with default table | statement |
| S-RBE-09 | existing 159 `lelwel-xs` tests | all pass |

## Delivery strategy

Single PR. If apply diff exceeds 400 lines, split commits by file only.

## Risk register

- Missing class name (Medium): seed primitives, allow external class sets, sample retail.
- Top-level bare-id shift (Low): gate only `block_item^`; keep `top_level_item^` unchanged.
- Predicate callback mismatch (Low): match lelwel 0.10 `predicate_block_item_1` shape.
- Qualifier mis-classification (Low): qualifiers are declaration starts in XS.
- `for (int i...)` unsupported (Low): sample stops before loop; Limitation 2 separate.

## Work-unit commit boundaries

1. `ast/type_table.rs` + `ast/mod.rs` re-export.
2. `parser.rs` context/predicate + `xs.llw` grammar + regenerated parser.
3. `ast/top_level.rs` helper + S-RBE-01..S-RBE-08 tests.
4. `cargo test` / `cargo build` / retail sample.
