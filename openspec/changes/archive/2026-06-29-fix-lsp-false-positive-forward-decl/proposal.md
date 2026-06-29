# Proposal: fix-lsp-false-positive-forward-decl

## Status
Draft

## Background
Issue #2 reports false-positive "used before declaration" diagnostics when opening vanilla `game/ai/core/main.xs` copied into `mod/spire_ai/game/ai/`. The LSP resolves included symbols via a merged view, and `setupDebugCategories` and friends are found correctly from transitive includes. However, the line-order check in `semantic.rs` compares the call-site line against `VisibilityProvenance::include_line()`, which for transitive includes stores the line of the *intermediate* file's `include` directive, not the line in `main.xs` where the include chain starts. A call after the direct include but before that intermediate-file line is therefore flagged incorrectly.

## Approach
Adopt **approach (a): precompute an effective-current-file line** inside `VisibilityProvenance` at merged-view build time.

Rationale:
- **Performance:** the merged view is cached; adding the field avoids rewalking the `IncludeGraph` for every call site on every keystroke.
- **Correctness:** the effective line is fixed when the view is built, matching the textual-paste semantics the rest of the merged model assumes. Cycle handling inherits the existing visited-file guard.
- **Testability:** tests can assert the precomputed `effective_line` directly, and regression fixtures become deterministic.
- **Maintenance:** `VisibilityProvenance` then carries a single invariant — where the symbol came from and where it becomes visible in the analysed file — rather than scattering graph walks across `semantic.rs`.
- **Alignment:** only `merged_view.rs` build logic and three `semantic.rs` call sites need to change; no parser/grammar work.

## User-facing contract
- Opening a mod `.xs` file produces **zero** "used before declaration" diagnostics for any function reached via a direct or transitive include, as long as the call appears after the effective include line in that file.
- Copying the vanilla `game/ai/core/main.xs` into a mod overlay produces **zero** such diagnostics.
- A call that genuinely appears **before** its effective include line is still reported as an error.
- Existing real forward-declaration errors in the same file (e.g. `updateBreakdown` in `human_assist.xs`) remain detected.
- No plugin-side changes; behavior is LSP-server-only.

## Scope

### In scope
- Add an `effective_line` field (or accessor) to `VisibilityProvenance::DirectInclude` / `TransitiveInclude`, populated from the outermost include line in the file being analysed.
- Update `merged_view.rs` `MergedView::build` / `walk_includes` to carry and record the effective line.
- Update `semantic.rs` line-order checks: `resolve_callee`, `effective_line`, and `forward_callable_merged`.
- Update `MergedView::visibility_line` to return the effective line.
- Add strict-TDD regression tests: Issue #2 fixture and mutable-redefinition ordering across a transitive include.
- Reinterpret existing tests that assert raw `include_line` for transitive includes.

### Out of scope
- Issue 1, Issue 3, Issue 4 from `docs/issues/2026-06-29-runtime-issues.md`.
- Duplicate-`extern` diagnostics observed during reproduction.
- Grammar/parser changes.
- Plugin-side (Kotlin) changes.
- R3-F-04 engine-API references scope.
- Changes to any XS mod script under `mod/`.

### Audit overlap
R3-F-04 "engine-API refs vs merged-view walk disagree" touches the merged-view / include area but is about reference-set scope, not line ordering. **Split:** do not co-fix; line-order changes to `VisibilityProvenance` may create reusable helpers, but R3-F-04 stays its own change.

## Impact
- **Files changed:** `tools/xs-language-server/src/merged_view.rs`, `tools/xs-language-server/src/semantic.rs`; possibly small test updates in the same files.
- **New test files:** `tools/xs-language-server/tests/issue2_repro.rs` (or equivalent inline test).
- **LOC delta:** +~80 / -~20.
- **Risk:** Medium — line ordering is load-bearing for forward-declaration diagnostics and mutable-redefinition ordering.
- **Effort:** 4–8 hours, including test reinterpretation and careful review.
- **Compatibility risk:** Low for users; existing tests may need expectations updated because transitive `include_line` assertions now describe the raw intermediate-file line while behavior uses `effective_line`. No public Rust API break outside the crate.
- **Rollback plan:** Revert `merged_view.rs` and `semantic.rs`; the current raw `include_line` logic is a near-subset except for the line-order comparison sites, so a clean revert restores today's diagnostics.
- **Patch-maintenance impact:** None — this is LSP tooling only; no mod overlay script changes.

## Success criteria
- The Issue #2 reproducer emits zero false-positive "used before declaration" diagnostics.
- `cargo test --manifest-path tools/xs-language-server/Cargo.toml` passes (209 baseline).
- New `tests/issue2_repro.rs` regression test passes.
- Existing `tests/game_folder_parse.rs` still passes.
- A real same-file forward-declaration error (e.g. `updateBreakdown` fixture) still produces the expected diagnostic.

## Risks and unknowns
- Engine's exact textual-paste include semantics are undocumented; the chosen model is "symbols visible at the earliest current-file include line that reaches them."
- Mutable-redefinition ordering now depends on the same effective-line invariant; must verify with a dedicated test.
- Cyclic include chains are terminated by the existing visited guard, but their effective line must be taken from the first reaching edge, not the cycle-closing edge.
- Multiple root includes may reach the same file; the first DFS path determines the stored effective line.
- Some existing tests may conflate raw intermediate include line with current-file effective line.

## Open questions
- Should `VisibilityProvenance::include_line()` keep its current name and semantics, with a new `effective_line()` accessor, or should `include_line()` be redefined to the current-file line? Backward compatibility of existing tests suggests keeping both.
- How should the `IncludeGraph` edge ordering interact with effective-line selection if the same file is included through multiple root paths at different lines? The likely answer is "first encountered during build."

## Next step
Spec phase should formalize the line-order rule for direct vs transitive includes and write the Issue #2 regression fixture as a strict-TDD test.
