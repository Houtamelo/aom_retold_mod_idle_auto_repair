# Spec: `fix-class-specifier-no-trailing-semi`

## Capability summary

The `class_specifier` rule in `tools/xs-language-server/lelwel-xs/src/xs.llw` SHALL match real XS: a class definition is `class Name { members }` with no trailing `;`. The grammar's current `';'` suffix on `class_specifier` is wrong and produces ~149 spurious "invalid syntax, expected: ';'" diagnostics per 1000-line retail XS file with class definitions.

## Rationale

The T7 typed-ast work applied with the grammar's `';'` intact and worked around the mismatch with a defensive zero-width-span fallback in `ClassDefinition::from_cst`. Survey of real retail XS files (sample: 5 files in `~/.steam/.../game/ai/`) shows zero instances of class definitions ending with `;`. The grammar should match the source-of-truth.

## Grammar source of truth

Real XS code. Five sampled files (`buildings.xs`, `map_analysis.xs`, `migrate_main_base_strategy.xs`, `bo_system_internal.xs`, `bo_system_internal_steps.xs`) all show class definitions ending with `}` (no `;`). The grammar must accept this form without diagnostic.

## Storage strategy

N/A — this is a grammar-only fix, not an AST shape change.

## Scenarios

### Scenario: Class definition without trailing `;` parses cleanly
- GIVEN the source `class MyClass { int x = 5; }` (no `;` after `}`)
- WHEN the parser processes it
- THEN 0 diagnostics are emitted
- AND the resulting `Rule::ClassSpecifier` node spans the entire source
- AND `ClassDefinition::from_cst` returns `Some(cd)` with `cd.members` populated

### Scenario: Class definition with trailing `;` still parses cleanly
- GIVEN the source `class MyClass { int x = 5; };` (with `;` after `}`)
- WHEN the parser processes it
- THEN 0 diagnostics are emitted
- AND the resulting `Rule::ClassSpecifier` node spans the entire source
- AND `ClassDefinition::from_cst` returns `Some(cd)` with `cd.semi` populated as the span of the `;` token (this preserves the "rare-form" support; deviation #3's zero-width fallback only fires for the no-`;` case)

### Scenario: Regression — class with multiple fields (real-world shape)
- GIVEN the source `class A { int x; int y; }` (two fields, no `;` after `}`)
- WHEN the parser processes it
- THEN 0 diagnostics are emitted (the class_specifier-level diagnostic is gone even with multiple members)
- AND the resulting `Rule::ClassSpecifier` node spans the entire source
- AND `ClassDefinition::from_cst` returns `Some(cd)` with 2 members

### Scenario: No regression on a real retail class
- GIVEN the first 5 lines of the real `BOSystem` class from `bo_system_internal.xs` (a 2-field class with `BOStep[]` array types and `default` initializers)
- WHEN the parser processes it
- THEN the class_specifier-level diagnostic is gone
- AND no NEW diagnostic is introduced by the grammar change (the 2 other diagnostics on the snippet are about `BOStep[]` array type or `default` initializer, not class_specifier)

### Scenario: Other `;`-requiring rules unaffected
- GIVEN the source `void f() { return; }` (function body with `return;`)
- WHEN the parser processes it
- THEN 0 diagnostics are emitted (the `';'` after `return` is still required — only `class_specifier` was changed)
- AND the AST extraction returns `Some` for the function definition

## XS-engine constraints

- The grammar change is local to `xs.llw`. No lexer changes, no parser changes, no AST shape changes.
- The `ClassDefinition` AST struct's `semi: Span` field is preserved (now genuinely `None`-equivalent or zero-width for real XS, and a real span for the rare `};` form).
- The T7 deviation #3 zero-width fallback in `ClassDefinition::from_cst` is preserved as a defensive measure for any other edge case (e.g., partial class definitions from incomplete parses).

## Non-functional requirements

- **Performance:** the grammar change is a single-token removal; no perf impact.
- **Compilation:** `cargo build` must remain green. The build script regenerates `parser.rs` from `xs.llw`; the regenerated parser must compile cleanly.
- **Tests:** all 98 existing typed-AST tests must remain green. The new regression tests must pass.
- **Diagnostic count:** no new diagnostic categories introduced.

## Out of scope

- Removing the `';'` from other grammar rules. Spot-check confirmed the only `';'`-suffixed rule that doesn't match real XS is `class_specifier`. The other rules (`include_directive`, `field_declaration`, `declaration`, `forward_declaration_rule`, `expression_statement`, `return`/`break`/`continue` statements) all genuinely require `;` per C-style convention.
- Tightening `ClassDefinition::from_cst` to return `None` for missing `;`. The defensive fallback is preserved; the grammar fix makes it rarely trigger in practice.
- Changes to the `dev/` (Xcode) or any other tooling.
