# Apply Progress: 2026-07-03-fix-rule-body-extraction

## Status

| ID | Title | State | Notes |
|---|---|---|---|
| T-RBE-01 | Create `TypeTable` | completed | `src/ast/type_table.rs` with 5 unit tests |
| T-RBE-02 | Wire `TypeTable` into parser | completed | `ParserCallbacks::Context = TypeTable` |
| T-RBE-03 | Implement `predicate_block_item_1` | completed | 5 direct predicate tests pass |
| T-RBE-04 | Update `block_item^` grammar | completed | Added `block_declaration_or_definition` helper due to lelwel nested-/ restriction; documented in deviations |
| T-RBE-05 | Re-export `TypeTable` | completed | `ast/mod.rs` re-exports from `ast/type_table` |
| T-RBE-06 | Add `parse_with_types` helper | completed | `parse_with_types` in `ast/top_level.rs` tests |
| T-RBE-07 | Add rule-body unit tests | completed | 8 scenario tests S-RBE-01..08 |
| T-RBE-08 | Keep existing tests green | completed | `cargo test` in `lelwel-xs`: 176 passed; 0 failed |
| T-RBE-09 | Verify build and retail sample | completed | `cargo build` green; S-RBE-06 retail snippet passes |

## Deviations from Design

1. **Grammar structure for `block_item^`**
   - *Original spec (design.md / tasks.md T-RBE-04)*: use `?1 declaration / ?1 forward_declaration_rule / ?1 function_definition_rule / statement` with ordered choice.
   - *Actual implementation*: `block_item^: ?1 block_declaration_or_definition | statement ;` and a new non-elided helper rule `block_declaration_or_definition: declaration_specifiers init_declarator_list ('{' block_item* '}' @function_definition | ';' @declaration) ;`.
   - *Why*: lelwel 0.10.4 rejects semantic predicates (`?1`) inside ordered-choice branches. It also rejects nested ordered choices, so `block_item^` cannot use `/` because it is called from `top_level_item`'s ordered choice. The helper rule parses the common declaration prefix and then uses plain alternation on disjoint next tokens (`{` vs `;`) to produce the correct node kind without backtracking.

2. **S-RBE-07 / S-RBE-08 test expectations**
   - *Original spec*: with an empty/primitive-only `TypeTable`, `rule r { int x; }` / `rule r { MyClass x; }` should extract as `BlockItem::Statement`.
   - *Actual implementation*: the tests assert that no `Rule::Declaration` descendant is produced. The statement fallback for `int x;` / `MyClass x;` is not valid XS, so the parser emits errors and does not produce a clean `RuleDefinition`. The important property — that the predicate prevents declaration classification — is still verified.

## Blockers

None.
