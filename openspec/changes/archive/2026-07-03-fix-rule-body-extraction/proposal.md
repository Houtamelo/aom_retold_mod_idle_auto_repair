# Proposal: `2026-07-03-fix-rule-body-extraction`

## Intent

Phase 4 will wire the LSP to the typed AST in `tools/xs-language-server/lelwel-xs/`. Today `RuleDefinition::from_cst` returns `None` for real (non-empty) rule bodies because the PoC grammar emits `ERROR` nodes for them. That would make rule symbols disappear from outline, hover, and semantic tokens. This change fixes the grammar so `RuleDefinition` nodes are produced for real XS rules.

## Scope

- In scope
  - Grammar change that lets `rule Name { body }` parse cleanly when the body contains statements or local declarations.
  - Parser-side `TypeTable` + predicate support needed by that grammar change.
  - Tests proving `RuleDefinition::from_cst` succeeds for non-empty rule bodies.
- Out of scope
  - LSP wiring (Phase 4).
  - Full Limitation 6 cleanup (L1B/L2B/casts) — only the `block_item^` predicate path required for rule bodies.
  - Populating `TypeTable` from engine API/workspace symbols — the API is provided; Phase 4 will call it.

## Capabilities

### New Capabilities
- `rule-body-extraction`: parser support for non-empty rule bodies so typed AST extraction succeeds.

### Modified Capabilities
- None (the AST shape in `spec-typed-ast.md` already defines `RuleDefinition`; this change fulfills its existing expectation).

## Approach

Replace the over-permissive `?t` predicates on the declaration/function-definition branches of `block_item^` with `?is_type`. The predicate consults a small `TypeTable` context containing primitive keywords and any caller-supplied class names. When the first token is an unknown identifier like `x`, the parser falls through to `expression_statement` instead of mis-parsing it as a type name. Rule bodies use `compound_statement`, which uses `block_item^`, so a clean rule node is produced and `RuleDefinition::from_cst` succeeds unchanged.

## Affected code paths

| File | Line | Change |
|------|------|--------|
| `tools/xs-language-server/lelwel-xs/src/xs.llw` | ~573–577 | Replace `?t` on declaration/function-definition alternatives in `block_item^` with `?is_type` |
| `tools/xs-language-server/lelwel-xs/src/parser.rs` | 27–39 | Change `ParserCallbacks::Context` from `()` to `TypeTable`; implement the generated predicate method(s) |
| `tools/xs-language-server/lelwel-xs/src/ast/top_level.rs` | ~386–408 | No code change; extraction succeeds once the grammar produces clean rule nodes |
| `tools/xs-language-server/lelwel-xs/src/ast/mod.rs` | re-export list | Re-export `TypeTable` so Phase 4 can build a parser context |

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `TypeTable` misses a class name used in retail rule bodies | Medium | Seed primitive keywords and accept external known-type sets; verify against sampled retail files |
| Predicate changes top-level bare-identifier parsing | Low | Only gate `block_item^`; leave `top_level_item^` ordering untouched |
| lelwel predicate callback signature mismatch | Low | Match generated `predicate_block_item_is_type` shape from lelwel 0.10 codegen |

## Rollback plan

1. Revert `xs.llw` to the `?t` form.
2. Revert `parser.rs` `Context` back to `()`.
3. `cargo test` in `lelwel-xs` returns to the current green baseline (rule bodies still fail to produce `RuleDefinition` nodes, but nothing downstream consumes them yet).

## Dependencies

- `lelwel` 0.10 predicate/codegen behavior (read-only dependency).
- Phase 4 will supply the populated `TypeTable` from engine API + workspace class extraction. For this change, tests construct `TypeTable` manually.

## Success criteria

- [ ] `cargo test` in `lelwel-xs` stays green, including the existing 159 tests.
- [ ] New test: `rule r { x; }` yields a `RuleDefinition` with one body item.
- [ ] New test: `rule r { int x = 1; }` yields a `RuleDefinition` with a declaration body item.
- [ ] Sampled retail rule body (e.g. `economic_units.xs` `deleteExcessGatherers`) no longer emits rule-level error nodes.
- [ ] Workspace `cargo test --manifest-path tools/xs-language-server/Cargo.toml` has no new failures.

## Alternate approaches rejected

- **AST-only fallback in `RuleDefinition::from_cst`:** Rejected per user direction (F1); the typed AST should not paper over grammar bugs.
- **Reorder `block_item^` to put `statement` first:** Would misparse class-typed local declarations (`BOSystem bo;`) as statements.
- **Implement full TypeTable + LSP wiring now:** Out of scope for the grammar-first change; Phase 4 will consume the `TypeTable` API.

## References

- Phase 3 typed-AST record: `openspec/changes/archive/2026-07-02-rich-typed-ast-layer/` (proposal/design/apply-progress note the rule-body grammar bug).
- Phase 4 explore: `openspec/explore/2026-07-03-phase4-lsp-wiring/explore.md` Section 6 identifies the `RuleDefinition::from_cst` blocker.
- Migration plan: `docs/plans/2026-07-01-migrate-tree-sitter-to-lelwel.md` Limitation 6 defines the `TypeTable` + ParserCallbacks design.

## Sign-off

Verification method per `openspec/config.yaml`: the global method is `manual_deploy_and_ingame_log`, but this tooling-only change falls under the `strict_tdd_overrides` for `tools/xs-language-server/**`, so the effective verification method is `cargo test --manifest-path tools/xs-language-server/Cargo.toml`.
