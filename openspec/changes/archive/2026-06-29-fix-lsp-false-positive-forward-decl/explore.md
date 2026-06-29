# fix-lsp-false-positive-forward-decl — Explore Report

## Bug status

CONFIRMED.

A standalone reproduction that mirrors the user's setup (copy the vanilla `game/ai/` tree into `mod/spire_ai/game/` and analyze `game/ai/core/main.xs`) emits the exact diagnostics reported in Issue #2. The symbols are resolved correctly by the merged view; the diagnostic is emitted because the forward-declaration line-order check uses the wrong line number for transitively-included symbols.

## Root cause

The LSP's forward-declaration check computes the effective definition line of an included symbol with `ms.provenance.include_line()`. For **direct** includes that value is the line of the `include` directive in the file being diagnosed, which is correct. For **transitive** includes it is the line of the `include` directive in the *intermediate* file (the file that directly includes the symbol's defining file), which is incorrect.

In `tools/xs-language-server/src/semantic.rs:820-846`, `forward_callable_merged` compares that line against the call site line. When the transitive-include line is larger than the call line, the function returns `false` and `check_forward_declarations_for_merged_view` emits `"'X' used at line N before declaration"`. This is what happens for every call in `main.xs` to functions defined in files included by `core/core.xs` (e.g. `setupDebugCategories` defined in `core/utilities/debug.xs`).

The same wrong value is also used by `effective_line()` (`semantic.rs:367`) for ordering `mutable` redefinitions across the merged scope, and by `resolve_callee()` (`semantic.rs:207`) for the workspace-first resolution rule.

## Code-path map

- User opens `mod/spire_ai/game/ai/core/main.xs`.
- `server.rs:1057` `publish_diagnostics` builds a semantic project and a merged view.
- `server.rs:1060` `build_semantic_project` loads the mod overlay + vanilla fallback via `semantic::VirtualProject::load_from_workspace` (`semantic.rs:81`).
- `server.rs:1064` `get_or_build_merged_view` calls `merged_view::MergedView::build` (`merged_view.rs:235`) with the registered mod's `VirtualProject`.
- `merged_view.rs:274` `resolve_include_edge` resolves `include "core/core.xs"` and recursively walks its includes; symbols from `debug.xs`, `setup.xs`, etc. are added with `VisibilityProvenance::TransitiveInclude`.
- `server.rs:1076` `diagnostics::collect_all` runs.
- `diagnostics.rs:84-85` with `merged` present, calls `semantic::check_forward_declarations_for_merged_view`.
- `semantic.rs:725` calls `forward_callable_merged`.
- `semantic.rs:834-837` computes `def_line` from `ms.provenance.include_line()`; for transitive includes this is the line inside `core/core.xs`, not the line inside `main.xs`.
- `semantic.rs:838` `def_line < call_line` fails → returns `false`.
- `semantic.rs:738-750` emits `"'X' used at line N before declaration"`.

## Mod-overlay behaviour

The workspace/mod-overlay plumbing is **not** the immediate cause. `Workspace::lookup_mod` (`workspace.rs:199`), `Workspace::build_virtual_project` (`workspace.rs:208`), and `Workspace::resolve_include_edge` (`workspace.rs:266`) correctly prefer files in `mod/spire_ai/game/` over the vanilla `game/` tree. The reproduction confirms that `merged.find("setupDebugCategories")` returns a `TransitiveInclude` whose `origin` is the overlay path `mod/spire_ai/game/ai/core/utilities/debug.xs`.

What is missing is the translation from "line in the intermediate file" to "effective line in the file being diagnosed". That translation is needed for any transitive include, whether the file comes from a mod overlay or directly from the vanilla game folder.

## Test coverage

`tools/xs-language-server/tests/game_folder_parse.rs` does **not** catch this bug because it deliberately skips files that are targets of an `include` directive. `main.xs` is included by many AI personality files via `include "core\main.xs"`, so it is classified as an include-target and excluded from top-level analysis (`analyze_top_level_diagnostics`:460-469). The issue only manifests when a user opens an include-target file directly, which the test suite currently avoids.

Existing unit tests in `semantic.rs` cover direct includes (`call_after_include_is_clean`, `direct_include_resolves_symbol`), transitive include resolution (`transitive_include_resolves_symbol`), and call-before-include errors (`call_before_include_is_error`), but they do not cover the case that triggers Issue #2: a call in the current file that appears *after* the direct include but *before* the transitive include line in the intermediate file.

Total LSP test count is **209** per `cargo test` (182 lib + 9 merged_view + 7 workspace + 5 r5_test_honesty_repro + 6 symbols_cleanup_repro).

## Audit cross-reference

`docs/code-reviews/2026-06-29-lsp-and-plugin-review.md` flags **R3-F-04** "engine-API refs vs merged-view walk disagree" (`server.rs:699-764`). R3-F-04 is about the references handler dropping workspace-wide engine-API references when a workspace shadow exists; it concerns the *scope* of the merged-view walk, not the *line ordering* of included symbols. The root cause here is distinct.

There is a surface overlap: both issues live in the merged-view / include-resolution area and could touch `MergedView`, `VisibilityProvenance`, or the `IncludeGraph`. Fixing the line-order bug cleanly may require changes to `VisibilityProvenance` or `IncludeGraph` helpers that R3-F-04 would also benefit from, but Issue #2 can be resolved independently.

No other audit finding directly describes the transitive-include line-order bug.

## Scope recommendation for proposal phase

- **Files to change:**
  - `tools/xs-language-server/src/merged_view.rs` — add an effective visibility line in the current file to `VisibilityProvenance::DirectInclude` / `TransitiveInclude`, or add a helper that walks the `IncludeGraph` to compute it. Rough estimate: 30–80 LOC depending on approach.
  - `tools/xs-language-server/src/semantic.rs` — update `forward_callable_merged` (`semantic.rs:820`), `effective_line` (`semantic.rs:367`), and `resolve_callee` (`semantic.rs:207`) to use the effective current-file line instead of the raw `include_line()`. Rough estimate: 20–40 LOC.
  - Existing `merged_view.rs` and `semantic.rs` tests may need small updates where they assert `include_line` values.

- **New tests needed:**
  - A regression test for Issue #2: fixture `main.xs` includes `core/core.xs`, `core/core.xs` includes `helper.xs` at a late line, `main.xs` calls the helper shortly after its own `include "core/core.xs"` — must produce zero "before declaration" diagnostics.
  - A mutable-redefinition ordering test across a transitive include to guard the `effective_line` path.

- **New test files:**
  - Likely none; new fixtures can live as inline strings in `semantic.rs` / `merged_view.rs` tests, consistent with existing fixtures.

- **Data-structure changes:**
  - **Yes.** Either `VisibilityProvenance` grows an effective-current-file-line field, or a new helper on `MergedView` / `IncludeGraph` computes it on demand. The former is cleaner because it centralizes the computation at merge-build time; the latter is less invasive to the provenance type but pushes graph traversal into every consumer.

- **Risk:**
  - Medium. The line-order rule is load-bearing for both forward-declaration diagnostics and `mutable` redefinition ordering. A fix must preserve the existing "call before include is an error" behavior for direct includes and must not over-accept calls that truly precede the include chain.

- **Effort:**
  - 4–8 hours including tests and a careful review of existing `include_line` consumers.

- **Audit overlap:**
  - R3-F-04 (related area, no direct overlap in root cause). R3-F-04 should be tracked separately.

## Risks / unknowns

- The exact textual-paste semantics the engine uses for includes are not documented in this repo. The safest LSP model is "symbols from the entire include closure become visible at the earliest include line in the current file that reaches them," but stricter engine behavior is possible.
- There may be interactions with `#include` / preprocessor directives not modeled by the current parser/extractor.
- Existing tests assert `include_line` values for transitive includes; those assertions will need to be reinterpreted or split into "raw include line in intermediate file" vs. "effective line in diagnosed file."
- The reproduction also surfaced duplicate-`extern` diagnostics when the entire `ai/` tree is loaded as a project. Those are unrelated to Issue #2 and should not be bundled into this change unless explicitly scoped.

## Next step

The proposal phase should decide between (a) enriching `VisibilityProvenance` with an effective-current-file-line field computed at merge-build time, or (b) computing the effective line on demand from the `IncludeGraph`, then spec the line-order rules for direct vs. transitive includes and add the Issue #2 regression fixture.