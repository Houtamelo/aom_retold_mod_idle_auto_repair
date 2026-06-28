# Default-Argument-Aware Type-Check Specification

> **Added/updated by change:** `lsp-server-semantic-fixes`

## Capability summary

The LSP server SHALL count only parameters that are **required** when validating call-site argument counts. In XS every non-`ref` parameter has a default value, so trailing arguments may be omitted. The server SHALL flag calls that provide too many arguments or too few required arguments, but SHALL allow omission of parameters that are optional.

## Rationale

The current `typecheck::check_one_call` uses `arg_count != params.len()`, which reports an error whenever the call passes fewer arguments than the function declares. Real AoM:R source routinely calls engine syscalls with only the leading arguments (e.g., `aiPlanCreate("...", cPlanExplore)`), because the engine supplies defaults for the trailing parameters. The type check must match engine semantics.

## Scenarios

### Scenario: happy path — omitting trailing default args on an engine syscall

- GIVEN the engine API declares `aiPlanCreate` with four parameters, all with defaults
- WHEN the LSP lints a call site `aiPlanCreate(0, 0)` (2 arguments)
- THEN no argument-count diagnostic SHALL be emitted

### Scenario: error — too many arguments

- GIVEN the engine API declares `aiPlanCreate` with four parameters
- WHEN the LSP lints a call site `aiPlanCreate(0, 0, 0, 0, 0, 0, 0)` (7 arguments)
- THEN a "too many arguments" diagnostic SHALL be emitted

### Revised requirement: no source-based "missing required argument" diagnostic

> The LSP SHALL treat every parameter as optional at the call site. The XS compiler
> enforces a default value on every non-`ref` parameter at definition time; this
> default may be implicit (not written in source). The LSP therefore does not flag
> "missing argument" errors based on source-visible defaults. `ref` parameters
> remain required by the compiler; the LSP will enforce this once `Param::is_ref`
> is propagated into the call-site check.

### Scenario: happy path — engine API parameter without extracted default is still optional

- GIVEN an engine syscall whose parameters have no documented default values in the cache
- WHEN the LSP lints a call site that omits trailing arguments
- THEN no argument-count diagnostic SHALL be emitted, because the engine treats all non-`ref` parameters as defaultable

## XS-engine constraints

- XS requires every non-`ref` parameter to have a default value; trailing positional arguments can be omitted.
- XS rejects any default value on a `ref` parameter; `ref` parameters are therefore always required.
- XS does not support skipping non-trailing positional arguments (e.g., `f(1,,3)` is invalid).
- The LSP's call-site argument-count check SHALL be uniform across workspace and engine-API callees, because the same compiler rule applies to both.

## Out of scope

- Type checking of `ref` parameters separately from non-`ref` parameters.
- Constant folding or evaluation of default-value expressions.
- Inferring missing defaults from call-site evidence; defaults must come from the declared signature or the engine-API cache.

## Verification approach

- Automated unit test: `void f(int x = -1, int y = -1) {}` called as `f(1)` produces no diagnostics.
- Automated unit test: `aiPlanCreate(...)` with 2 and 7 arguments exercise both allowed omission and too-many-args.
- Integration test `game_folder_parse.rs`: argument-mismatch count across the official game folder is zero.

## Acceptance criteria

1. A call site with `arg_count <= params.len()` is LEGAL; the XS compiler forces a default on every non-`ref` parameter at definition time.
2. A call site with `arg_count > params.len()` SHALL be flagged as an error (too many arguments).
3. The LSP SHALL NOT flag a call site as "missing required argument" solely because a workspace parameter lacks a source-visible default value.
4. Once `Param::is_ref` is propagated into the call-site check, `ref` parameters SHALL be treated as required.
5. Engine-API syscalls and workspace-defined functions SHALL follow the same uniform rule.
