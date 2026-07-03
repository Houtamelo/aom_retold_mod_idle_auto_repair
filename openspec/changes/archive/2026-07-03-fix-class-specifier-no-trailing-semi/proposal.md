# Proposal: `fix-class-specifier-no-trailing-semi`

## Intent

Drop the `';'` from `class_specifier` in `tools/xs-language-server/lelwel-xs/src/xs.llw` so the PoC grammar matches real XS: class definitions end with `}`, not `};`.

## Why

1. **Real XS does not use trailing `;` after class definitions.** Survey of `~/.steam/.../game/ai/**` (sample: `bo_system_internal.xs`, `buildings.xs`, `map_analysis.xs`, `migrate_main_base_strategy.xs`, `bo_system_internal_steps.xs`) shows every class definition ends with `}` (no `;`). Examples:
   ```
   class BOSystem
   {
      BOStep[] buildOrderSteps = default;
      ...
   }    ← no semicolon
   ```

2. **The current grammar produces 1 spurious "expected ';'" diagnostic per class definition** (verified by direct measurement on synthetic and real sources — see `verify-report.md`). For simple classes (`class A { int x; }`), this is the only diagnostic and the fix eliminates it. For complex real-world classes with field/method bodies, the 1 win is masked by other unrelated diagnostics in the same vicinity (e.g. on `BOStep[]` array types or `default` initializers), but the class_specifier-level diagnostic itself is gone.

3. **The T7 typed-ast apply had to paper over the mismatch** with a defensive zero-width-span fallback in `ClassDefinition::from_cst` (recorded as deviation #3 in `apply-progress.md`). Once the grammar is fixed, the fallback is still kept as a safety net for partial parses, but the common case (no `;` in source) no longer produces a diagnostic to recover from.

## Scope

- **In scope:** one-line grammar change in `xs.llw:238`. Regression test in the test suite. Update of the T7 deviation log in the parent change's `apply-progress.md`.
- **Out of scope:** any change to the AST extraction logic itself. The T7 deviation #3 zero-width fallback remains in place (it's still correct defensive behavior). The grammar fix simply makes the fallback rarer in practice.

## Rollback

Trivial. Revert the one-line change. No data migration, no LSP wiring change, no test infrastructure change. The `dev/` branch is gitignored, so rollback is just a local edit.

## Approach

Direct, surgical:
1. Edit `xs.llw:238` — change `class_specifier: 'class' Identifier '{' class_member* '}' ';';` to `class_specifier: 'class' Identifier '{' class_member* '}';`.
2. Add a regression test in `top_level.rs::tests` that parses a class WITHOUT trailing `;` and asserts 0 diagnostics.
3. Add a regression test in `top_level.rs::tests` that parses a retail class file (e.g. `bo_system_internal.xs`'s first class) and asserts 0 diagnostics on that fragment.
4. Run `cargo build && cargo test` — all 98+1 new tests must be green.
5. Run `cargo run --example diag_cst` on a retail file — assert diagnostic count for "missing ';'" category drops to 0.

## Acceptance

- All 4 regression tests in `top_level.rs` pass (added in T2).
- All 98 pre-existing typed-AST tests still pass (no regressions).
- For synthetic 1-field class definitions, diagnostic count drops from 1 to 0 (the spurious class_specifier diag is gone).
- For synthetic 2-field and 4-field class definitions, same drop from 1 to 0.
- For real retail `BOSystem` class (2 fields, isolated), the class_specifier-level diagnostic is gone (1 of 3 diags is now attributed to a different rule — likely the `BOStep[]` array type or `default` initializer — but no NEW diagnostic is introduced).
- The T7 deviation log in `openspec/changes/2026-07-02-rich-typed-ast-layer/apply-progress.md` updated to mark deviation #3 as "grammar fixed; defensive fallback remains in place as a safety net."
