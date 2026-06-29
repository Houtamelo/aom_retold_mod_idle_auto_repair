# RED evidence: fix-lsp-false-positive-forward-decl

Recorded during Phase 1 of the strict-TDD apply phase.

## Test file

`tools/xs-language-server/tests/forward_decl_repro.rs`

## Baseline

- `cargo build --tests` succeeded with pre-existing warnings only.
- Baseline test count: 209.

## Status after writing Phase 1 tests

| Test | Result on unfixed code |
|------|------------------------|
| `test_vanilla_main_xs_in_mod_overlay_produces_zero_false_positives` | FAIL — produces 1 false-positive `"before declaration"` diagnostic |
| `test_real_same_file_forward_decl_error_still_detected` | PASS |
| `test_mutable_function_called_before_redefinition_does_not_emit` | PASS |
| `test_cyclic_includes_do_not_infinite_loop` | PASS |
| `test_multiple_root_includes_takes_minimum_effective_line` | UNABLE TO RUN — references non-existent `MergedSymbol::effective_line()` |
| `test_definition_in_current_file_uses_definition_line` | UNABLE TO RUN — references non-existent `MergedSymbol::effective_line()` |

## Issue #2 false-positive diagnostic

Fixture layout:
- `ai/core/main.xs` includes `"core/core.xs"` on line 0 and calls `setupDebugCategories()` on line 2.
- `ai/core/core.xs` includes `"core/utilities/debug.xs"` on a late line.
- `ai/core/utilities/debug.xs` defines `setupDebugCategories()`.

Observed diagnostic:

```text
Diagnostic {
  range: Range { start: Position { line: 2, character: 15 }, end: Position { line: 2, character: 37 } },
  severity: Some(Error),
  code: Some(String("E0310")),
  source: Some("xs-language-server"),
  message: "'setupDebugCategories' used at line 3 before declaration; add forward declaration or mark 'mutable'",
  ...
}
```

This confirms the root cause: the line-order check compares the call-site line against the intermediate file's `include` line (`core/core.xs`), not the current-file (`main.xs`) include line.

## Compile-time RED evidence for `effective_line`

When the two `effective_line` tests are enabled, the test binary fails to compile:

```text
error[E0599]: no method named `effective_line` found for reference `&MergedSymbol` in the current scope
  --> tests/forward_decl_repro.rs:219:12
```

This is expected RED state: the accessor must be added to `VisibilityProvenance` and `MergedSymbol` during Phase 2.
