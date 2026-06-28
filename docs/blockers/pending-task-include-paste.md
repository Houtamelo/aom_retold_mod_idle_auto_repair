# Pending Task — XS `include "..."` textual paste

**Status:** pending
**Owner:** houtamelo
**Created:** 2026-06-24
**Target:** immediately after the first manual smoke test of the redesigned LSP
**Related:** `openspec/changes/archive/xs-language-server/verify-report.md` (Known Issues §7, item 3)
**Branches:** `xs-language-server/redesign-phase-5` (current); will land on a new `xs-language-server/true-include-paste` branch

---

## Problem (summary from verify-report)

`completion.rs:107` (and by extension the semantic / typecheck pass on the current
file) handles `include "foo.xs"` as an **approximation** rather than a true textual
paste. The current code:

1. **Extracts include targets via a per-line regex** (`included_files` in
   `completion.rs:131-149`). Misses multi-line includes, comments containing
   `include`, and string literals.
2. **Matches included files by `path.ends_with(target)`** — a string-suffix match,
   not the proper `workspace.rs::resolve_include` (which understands include-root
   context: `AI` / `TRIGGER` / `RANDOM_MAP`).
3. **Does not follow transitive includes.** If `core.xs` itself `include`s
   `utils.xs`, `utils.xs`'s symbols are not added to the completion scope.
4. **Treats all included-file symbols as visible**, regardless of `extern` /
   `static` modifiers. Static (file-local) variables in the included file
   should NOT be visible in the includer.
5. **Does not feed the merged view into the semantic analyzer.** Each file is
   analyzed independently, so a function defined in `core.xs` and called from
   `human_assist.xs` may be flagged as "use before definition" inside `core.xs`
   even though in the engine's view the function is callable at the call site.

The verify report classified this as a known issue, not a blocker, because the
common case (one level of direct include, no mod overlay, no file-local/exported
distinction in the included file) works.

## Why now

The user wants the redesigned LSP smoke-tested manually in a real
IntelliJ + AoM:R install before any further work. As soon as that smoke test
confirms the common case, this task should be picked up immediately.

## Target behavior

Build a "merged view" of each file at analysis time:

1. **Walk the AST** for `include "..."` nodes. Use tree-sitter (cleaner than the
   line-based regex). Handles edge cases (comments, string literals, multi-line).
2. **Resolve each include** through `workspace.rs::resolve_include` (which already
   understands include-root context and mod overlay). This replaces the
   `path.ends_with(target)` heuristic.
3. **Recurse** into the included files (transitive includes). Stop at cycles
   (use a visited set).
4. **Merge the symbol tables** in scope order:
   - First, the current file's own symbols (file-local)
   - Then, the included files' exported symbols (`extern`-marked only, not `static`)
   - Then, the included files' other exported functions (`const` constants from
     Doxygen, etc.)
5. **Build a single "merged" virtual view** for the current file. Use it for:
   - `textDocument/completion` (no change to API; just feed the merged set)
   - `textDocument/hover` (resolve identifiers against the merged scope)
   - `textDocument/definition` (jump to the right file)
   - `textDocument/references` (search the merged view, not just the current file)
   - `semantic::check_forward_declarations` (no false positives in the includer)
   - `typecheck.rs` (resolve function call types against the merged view)

## Files to change

| File                                                   | Change                                                                                                                                                                                                                                  |
| ------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `tools/xs-language-server/src/workspace.rs`            | New API: `merge_includes_for_file(file: &Path, project: &VirtualProject) -> MergedView`. Reuses `resolve_include` + the existing per-file `SymbolTable` cache.                                                                          |
| `tools/xs-language-server/src/symbols.rs`              | Add `Visibility::ExportedViaInclude` to distinguish symbols that are file-local in the included file but visible in the includer (only after the merge pass). Existing `Extern` / `Const` / `Local` stay.                                |
| `tools/xs-language-server/src/completion.rs`            | Replace `included_files` regex + `path.ends_with` with a call into `workspace::merge_includes_for_file`. Remove the per-line text scan.                                                                                                  |
| `tools/xs-language-server/src/semantic.rs`             | `check_forward_declarations` should walk the merged view (no flag in the includer if the definition is in an included file; flag in the includer if the include target doesn't exist or doesn't define the symbol).                    |
| `tools/xs-language-server/src/typecheck.rs`            | Cross-file function type lookup: route through the merged view, not the raw virtual project.                                                                                                                                            |
| `tools/xs-language-server/src/word.rs`                 | `identifier_at_cursor` stays the same (file-local). The merged view is only for cross-file resolution.                                                                                                                                    |
| `tools/xs-language-server/src/references.rs`           | Consider searching the merged view (not just the current file) so a `gMyGlobal` reference from a mod file can find the definition in an included file. Document this as a behaviour change in `spec-semantic-diagnostics.md`.        |
| `tools/xs-language-server/src/diagnostics.rs`          | Wire the merged view into the diagnostic pipeline. On `didOpen` / `didChange`, build the merged view for the current file and pass it to `semantic` and `typecheck`.                                                                     |
| `tools/xs-language-server/src/cache.rs`                | Per-file parse cache: still keyed by `<mtime>-<sha256>`. No new cache needed for merged views (they're cheap to build from already-cached symbol tables).                                                                                  |
| `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` | New test cases: include that resolves; include that doesn't exist; transitive include chain; mod overlay on the included file; include cycle.                                                                                          |
| `tools/xs-language-server/src/semantic_fixtures/`      | Add fixture: `include_forward_decl.xs` (calls a function defined in an included file, no forward decl in the includer, included file has it) — should produce **no** forward-decl error in the includer.                                |
| `openspec/specs/spec-virtual-project-overlay.md`       | Add a scenario: "true textual paste" (the new behavior). Update the verification approach.                                                                                                                                                |
| `openspec/specs/spec-semantic-diagnostics.md`          | Update the forward-decl rule to clarify: a symbol is considered "defined" if it is in the merged view of the current file, not just the current file's own declarations.                                                                |
| `openspec/changes/archive/xs-language-server/verify-report.md` | Remove "include textual paste is approximated" from Known Issues once the fix is verified. Add the fix commits to the archive-report timeline.                                                                                  |

## Acceptance criteria

1. The line-based `included_files` regex in `completion.rs:131-149` is removed
   and replaced with an AST walk.
2. `path.ends_with(target)` matching is gone; included files are resolved
   through `workspace.rs::resolve_include` (with include-root context + mod
   overlay).
3. Transitive includes work: a function defined in `a.xs` → included by
   `b.xs` → included by `c.xs` is callable from `c.xs` in the completion,
   hover, definition, and semantic checks.
4. Static (file-local) variables in the included file are NOT visible in the
   includer (matches engine semantics). `extern` variables in the included
   file ARE visible.
5. The `forward_decl_ok.xs` and `forward_decl_missing.xs` semantic fixtures
   continue to pass (no regression in the current behavior).
6. A new fixture (`include_forward_decl.xs`) passes: a function defined in
   the included file is callable from the includer without a forward
   declaration, no diagnostic in the includer.
7. Mod overlay on an included file: if the mod's `core.xs` shadows the
   game's `core.xs`, the LSP resolves the include to the mod's version, and
   the merged view reflects the mod's symbols, not the game's.
8. Include cycle (file A includes B, B includes A): handled without infinite
   loop. Visited-set prevents re-entry. Symbols from the cycle are visible
   up to the first re-entry.
9. `cargo test` green; `cargo run --bin lsp_roundtrip_test` green.
10. Spec scenarios added; verify-report's "include textual paste is
    approximated" entry removed; archive-report updated.

## Estimated size

- New code: ~300-400 LOC (mostly in `workspace.rs::merge_includes_for_file`
  and the diagnostic pipeline wiring).
- Modified code: ~150 LOC across the affected files.
- New tests: ~150 LOC (fixtures + round-trip scenarios).
- Total delta: ~600-700 LOC.

## Out of scope

- Macro-like preprocessor directives (`#define`, `#if`/`#endif`) — already
  noted as a known limitation in the existing parser; not introduced by
  this task.
- `include` resolving to a file outside the virtual project (e.g.,
  absolute paths). XS only allows relative includes; absolute paths are
  an error.
- Cross-mod includes (file in mod A including a file in mod B's overlay
  that's not in mod A's tree). Currently not supported by the design;
  defer.

## Dependencies

- `workspace.rs::resolve_include` (already exists from Phase 2) — used as
  the resolution primitive.
- Per-file parse cache (already exists from Phase 2) — provides the
  symbol tables to merge.
- Tree-sitter XS grammar (no changes needed; `include` is already a
  node in the grammar).

## Verification strategy

1. **Unit tests** for `merge_includes_for_file`:
   - Single direct include (the common case)
   - Transitive chain (2+ levels)
   - Mod overlay on the included file
   - Include cycle
   - Missing include target (file not found)
   - Include root context (AI vs TRIGGER vs RANDOM_MAP)
2. **Semantic fixtures**:
   - `include_forward_decl.xs` (positive)
   - `include_missing.xs` (negative — should produce an error for the
     missing include)
3. **LSP round-trip test**:
   - Open file with include; verify completion includes the included
     file's symbols
   - Open file with include; verify forward-decl diagnostics do not fire
     in the includer for symbols defined in the included file
4. **Manual smoke test** (in the real IntelliJ + AoM:R install):
   - Open `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`
   - Verify completion offers symbols from the included files
   - Verify hover on a symbol defined in an included file shows the right
     definition
   - Verify go-to-definition from the includer lands in the included file
5. **Spec update**:
   - Add a "true textual paste" scenario to
     `spec-virtual-project-overlay.md`
   - Update `spec-semantic-diagnostics.md` forward-decl rule

## Rollback

If the fix introduces regressions, revert via `git revert <fix-commits>` on
the `xs-language-server/true-include-paste` branch. The current behaviour
(approximation) is the known-issue baseline; the fix is additive.

## Linked artifacts

- `openspec/changes/archive/xs-language-server/verify-report.md` — original
  known issue
- `openspec/specs/spec-virtual-project-overlay.md` — workspace + include
  resolution spec
- `openspec/specs/spec-semantic-diagnostics.md` — semantic rules spec
- `openspec/changes/archive/xs-language-server/design.md` — Phase 2 design
  for the original (approximated) include handling
- `tools/xs-language-server/src/completion.rs` — current approximation
- `tools/xs-language-server/src/workspace.rs` — `resolve_include` primitive
  to be reused
