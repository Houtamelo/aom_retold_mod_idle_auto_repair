# Design: `fix-lsp-symbols-cleanup` — Remove dead `Param::render` and misleading test in `symbols.rs`

## Overview

This change is a single-file cleanup in `tools/xs-language-server/src/symbols.rs`. It deletes the unused `Param::render` method (lines 84-94), the now-empty `impl Param` block it lives in, and the `is_false` helper (lines 80-82), replacing the helper with an inline `serde(skip_serializing_if = ...)` closure. It also renames `recovers_function_definition_from_error_node_with_body` (lines 807-819) to describe the regular `function_definition` extraction path it actually exercises. The expected delta is approximately `-25 / +5` LOC. There is no production behavior change for the LSP server: `Param::render` had no callers, and the renamed test still covers the same code path.

## Architecture Decisions

### ADR-1: Inline `is_false` via `std::ops::Not::not`

`is_false` exists solely to support `#[serde(skip_serializing_if = "is_false")]` on `Param::is_ref`. We will replace the annotation with `#[serde(default, skip_serializing_if = "std::ops::Not::not")]`, which compiles under Rust 1.85.0 and preserves the exact same JSON shape: `is_ref` is omitted when `false` and emitted when `true`. The free function can then be deleted.

### ADR-2: Rename the misleading test instead of deleting it

`recovers_function_definition_from_error_node_with_body` claims to exercise ERROR recovery, but the test input `void broken(int x = [ ] { }) { }` matches the regular `function_definition` rule because the outer `{ }` is a real body; only the default-value subtree is an ERROR. The test still verifies useful behavior — that `extract_function` can tolerate an inner ERROR in the default value and still produce a usable `Function` symbol — so we keep the assertion body and rename it to `function_definition_with_inner_error_in_default_value_still_extracts`. The regular `function_definition` path is covered elsewhere by existing `test_symbol_*` tests, but the inner-ERROR-in-default-value scenario is unique to this test.

### ADR-3: Invert R1-F-01 audit tests to strict-TDD RED→GREEN contracts

The six audit-grade tests in `tools/xs-language-server/tests/symbols_cleanup_repro.rs` currently pass on the buggy state by asserting the lies are real. We will restructure them as follows:

1. `r1_f01_param_render_has_no_callers` — delete. Once `Param::render` is removed, the test is meaningless.
2. `r1_f01_param_render_body_has_unused_source_param` — replace with `param_render_method_no_longer_exists_on_param`, a compile-time contract (e.g., `let _ = Param::render;` must fail to compile post-fix, or a marker trait / type-level assertion).
3. `r1_f02_forward_declaration_parses_as_error_node` — keep as-is. It documents the R1-F-02 refutation and does not conflict with this change.
4. `r1_f02_extract_error_helpers_have_no_real_callers` — keep as-is.
5. `r1_f03_test_input_parses_via_function_definition_rule` — keep as-is.
6. `r1_f03_symbol_extracted_via_extract_function_path` — keep as-is.
7. Add `misleading_test_renamed_to_function_definition_with_inner_error_in_default_value`, a lightweight contract asserting the test function exists under its new name.

### ADR-4: No public API change visible to consumers

`Param::render` is `pub`, but with zero internal callers and no external consumers (the LSP server is the only crate in the workspace). Removing it is a non-breaking internal cleanup. The `is_ref` serialization output is unchanged; only the predicate form used by serde is inlined.

### ADR-5: No spec delta sync required outside this change folder

The proposal states "Modified Capabilities: None". The new capability spec `spec-lsp-symbol-honesty.md` is authored inside the change folder (`openspec/changes/fix-lsp-symbols-cleanup/specs/`) and will be promoted to `openspec/specs/` during the archive phase. No other spec file is touched.

## Code Structure Sketch

```rust
/// A function parameter — `int x` or `string s = "default"`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Param {
    pub ty: String,
    pub name: String,
    /// Raw default-value expression text, e.g. `"-1"` or `"\"hi\""`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
    /// `ref` modifier — parameter passed by reference.
    /// The XS compiler rejects defaults on ref params.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub is_ref: bool,
}
```

The `impl Param` block and the `is_false` helper are gone. The misleading test becomes:

```rust
#[test]
fn function_definition_with_inner_error_in_default_value_still_extracts() {
    let src = "void broken(int x = [ ] { }) { }\n";
    let t = table_for(src);
    let s = t.find("broken").expect("broken symbol from function_definition");
    assert_eq!(s.kind, SymbolKind::Function);
    assert!(!s.is_forward);
    assert_eq!(s.params.len(), 1);
    assert_eq!(s.params[0].name, "x");
}
```

## Data Flow

No data-flow change. Symbol extraction continues to parse `function_definition` nodes and emit `Symbol` values with `Vec<Param>`. The only differences are:

- `Param` is a plain struct with no methods.
- Serializing a `Symbol` to JSON skips `is_ref` when `false` exactly as before.
- The regression test suite asserts the post-cleanup state instead of the pre-cleanup lies.

## File Changes

| File | Action | Description |
|---|---|---|
| `tools/xs-language-server/src/symbols.rs` | Modify | Delete `is_false` (lines 80-82). Delete the `impl Param` block containing `render` (lines 84-94). Update `Param::is_ref` annotation to `skip_serializing_if = "std::ops::Not::not"`. Rename test at lines 807-819. Approx `-25 / +5` LOC. |
| `tools/xs-language-server/tests/symbols_cleanup_repro.rs` | Modify | Replace the two R1-F-01 audit tests with strict-TDD post-fix contracts. Add a new test asserting the misleading test was renamed. Keep R1-F-02 and R1-F-03 tests as-is. |

## Interfaces / Contracts

No new public interfaces, types, or protocols. `Param` remains a `#[derive(...)]` public struct with the same four fields. The JSON shape of serialized symbols is unchanged.

## Testing Strategy

Strict TDD per the tooling-domain override:

1. **RED stage**: run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test symbols_cleanup_repro`. The new post-fix contracts for `Param::render` absence and the renamed test are expected to fail against the current code.
2. **GREEN stage**: apply the deletions and test renames, then rerun. The full `symbols_cleanup_repro.rs` test suite passes.
3. **REFACTOR stage**: confirm no clippy regressions and full suite green.

Final verification:

| Layer | What to test | Approach |
|---|---|---|
| Unit / regression | `tools/xs-language-server/tests/symbols_cleanup_repro.rs` | `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test symbols_cleanup_repro` |
| Compile-time contract | `Param::render` no longer exists | New strict-TDD test fails to compile if the method is present |
| Full suite | All LSP unit and integration tests | `cargo test --manifest-path tools/xs-language-server/Cargo.toml` |
| Lint | No new warnings | `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml` |

## Migration / Rollout

No migration required. `git revert <change-sha>` is fully safe. This change deletes dead code, inlines a serde predicate, and renames one test. There is no protocol change, schema change, cache-version bump, or deployed-mod behavior change.

## Open Questions

None.

## Risks

| Risk | Likelihood | Mitigation |
|---|---|---|
| `Param::render` has a hidden caller we missed | Very low | `cargo test --manifest-path tools/xs-language-server/Cargo.toml` will surface any breakage immediately. |
| `Not::not` is not in scope for the serde predicate | Very low | Fallback to `#[serde(default, skip_serializing_if = "\|b: &bool\| !*b")]` if the compiler rejects the method path. |
| The test rename breaks a downstream test that names the old test | None | Rust test functions are not name-referenced outside their own test binary. |
