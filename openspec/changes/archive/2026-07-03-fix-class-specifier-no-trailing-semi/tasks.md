# Tasks: `fix-class-specifier-no-trailing-semi`

## Review Workload Forecast

Total estimated LOC: ~30 lines (1 line grammar change + ~25 lines of regression tests + ~3 lines of `apply-progress.md` patch).

- T1 (grammar edit): 1 line in `xs.llw`
- T2 (regression tests): ~25 lines in `top_level.rs::tests`
- T3 (apply-progress.md patch): 3 lines
- T4 (end-to-end verify): no new code

Chained PRs recommended: **No** — single-file grammar edit + tests, well under the 400-line review budget.
400-line budget risk: **Low** — entire change is ≤ 30 LOC.
Decision needed before apply: **No** — the spec and proposal already lock the approach.

---

### T1 — Edit `class_specifier` to drop `';'`

**Phase:** 1
**Depends on:** (none)
**Specs referenced:** `specs/spec-grammar-fix.md`
**Files affected:** `tools/xs-language-server/lelwel-xs/src/xs.llw`

- [ ] Read `xs.llw` line 238 (current rule).
- [ ] Change `class_specifier: 'class' Identifier '{' class_member* '}' ';';` → `class_specifier: 'class' Identifier '{' class_member* '}';`.
- [ ] DO NOT touch any other line.
- [ ] Run `cargo build` to confirm the regenerated parser compiles.

### T2 — Add regression tests in `top_level.rs::tests`

**Phase:** 1
**Depends on:** T1
**Specs referenced:** `specs/spec-grammar-fix.md` (Scenarios 1-4)
**Files affected:** `tools/xs-language-server/lelwel-xs/src/ast/top_level.rs`

- [ ] Add `class_definition_parses_without_trailing_semi_no_diagnostics` — parse `class MyClass { int x = 5; }`, assert 0 diags and `ClassDefinition::from_cst` returns `Some(_)`.
- [ ] Add `class_definition_parses_with_trailing_semi_no_diagnostics` — parse `class MyClass { int x = 5; };`, assert 0 diags and `cd.semi` is a real span (not zero-width).
- [ ] Add `class_definition_no_trailing_semi_is_default` — parse `class MyClass { }` (empty body, no `;`), assert 0 diags.
- [ ] Add `field_declaration_inside_class_unchanged` — parse `class MyClass { int x = 5; void foo() {} }`, assert 2 class members, both with `;`-terminated declarations. (Sanity check that the inner rule `field_declaration` still requires `;`.)
- [ ] All 4 new tests must be green.

### T3 — Patch the T7 deviation log in the parent change

**Phase:** 1
**Depends on:** T1, T2
**Specs referenced:** (none — documentation update)
**Files affected:** `openspec/changes/2026-07-02-rich-typed-ast-layer/apply-progress.md`

- [ ] Find deviation #3 in the Phase 2 record (it currently says "the grammar requires it but real sources often omit it").
- [ ] Update the wording to: "the grammar previously required `;` but real sources never use it. **Resolved by `2026-07-03-fix-class-specifier-no-trailing-semi`** — the `;` was removed from `class_specifier` in `xs.llw:238`. The defensive zero-width-span fallback in `ClassDefinition::from_cst` remains in place as a safety net for partial parses (where the parser's error recovery may still surface a class_specifier with no `;`)."
- [ ] Add a single cross-reference line: "Grammar fix: see `openspec/changes/2026-07-03-fix-class-specifier-no-trailing-semi/verify-report.md`."

### T4 — End-to-end verify

**Phase:** 1
**Depends on:** T1, T2, T3
**Specs referenced:** `specs/spec-grammar-fix.md` (Scenario 3)
**Files affected:** (none — verification only)

- [ ] Run `cargo build` — zero errors.
- [ ] Run `cargo test` — all 98 + 4 = 102 tests green.
- [ ] Run `cargo run --example diag_cst` on `bo_system_internal.xs` and assert the "missing ';'" diagnostic count drops from 149 to 0.
- [ ] Confirm no other diagnostic categories increased (the 149-diagnostic drop should match the total drop exactly).
- [ ] Write `verify-report.md` with these numbers.

---

## Cross-phase invariants

- Each task MUST leave `cargo build` green before the next task starts.
- Total LOC of the change is ≤ 30 lines. Per-task estimates are tight; no task should exceed ~25 lines.

## Verification commands

```bash
export PATH="/home/houtamelo/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin:$PATH"
export RUSTUP_TOOLCHAIN=nightly
cd /home/houtamelo/Documents/projects/aom_retold_mod/tools/xs-language-server/lelwel-xs
cargo build                       # zero errors
cargo test                        # all 102 tests green
cargo run --example diag_cst      # no "missing ';'" on real retail class
```
