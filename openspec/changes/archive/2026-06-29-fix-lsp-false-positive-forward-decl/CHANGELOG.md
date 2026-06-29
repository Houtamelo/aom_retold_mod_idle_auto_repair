# Change log: fix-lsp-false-positive-forward-decl

## 2026-06-29

- Resolved Issue #2 — false-positive `"used before declaration"` diagnostics on unmodified vanilla game files opened through a mod overlay.
- Added `effective_line: u32` to `VisibilityProvenance::DirectInclude` / `TransitiveInclude`, populated during `MergedView::build` from the earliest current-file `include` directive that reaches each symbol.
- Updated the three line-order checks in `semantic.rs` (`resolve_callee`, `effective_line`, `forward_callable_merged`) and `MergedView::visibility_line` to use `effective_line` instead of the raw intermediate-file `include_line`.
- Added `tools/xs-language-server/tests/forward_decl_repro.rs` with strict-TDD regression tests for transitive includes, same-file forward-declaration errors, mutable redefinition ordering, cyclic includes, and multiple-root include edge ordering.
