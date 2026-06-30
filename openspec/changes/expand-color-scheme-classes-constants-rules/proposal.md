# Proposal: Expand class member extraction for semantic coloring

> **Note on naming**: The folder name `expand-color-scheme-classes-constants-rules` predates this narrow follow-up. The work below is the Bucket C remainder: extracting class fields and methods. A rename to `expand-class-member-extraction` is recommended for future follow-ups but is deferred here to keep existing links stable.

## Intent

The prior slice (`finish-semantic-token-distinctions`, `0.7.0`) added `SymbolKind::Class` and colored class *names*, but explicitly skipped class body members. This change extracts fields and methods from `class_specifier` bodies so member references such as `obj.field` and `Class.method()` can resolve, receive distinct semantic tokens, and appear in the IDE outline.

## Scope

### In Scope
- Extract class fields and methods from `field_declaration_list` as `ClassField`/`ClassMethod` symbols with `class_owner` set.
- Keep class members `Visibility::Local` so they are not merged across files.
- Add `member` to the LSP semantic-token modifier legend and emit it on member declarations and `field_expression` references.
- Resolve member-reference origin with an ambiguous-name heuristic (prefer `modded` if any modded match exists, then `unmodded`, then `engine`).
- Surface class members in `DocumentSymbol` and `workspace/symbol` results.
- Add member-specific color keys/descriptors and extend the plugin semantic-token converter.
- Bump `pluginVersion` to `0.8.0` and add Rust/Kotlin tests.

### Out of Scope
- Type-inference-based member resolution (we match by name across all classes).
- Constructors, `new` expressions, class inheritance, virtual methods, access modifiers, and static class members.
- Array built-in calls (`.size()`, `.add()`, `.clear()`) remain uncolored unless already handled.
- Cross-file member lookup through `include` paste.

## Capabilities

### New Capabilities
- `lsp-class-member-extraction`: Walk `class_specifier` bodies and emit field/method symbols.
- `lsp-class-member-semantic-tokens`: Add `member` modifier, color `field_expression` references, and wire plugin member color categories.

### Modified Capabilities
- `lsp-class-extraction`: Remove the member-extraction exclusion; add `class_owner` to symbols; include members in outline/workspace results.

## Spec Deltas

- `specs/spec-lsp-class-member-extraction.md` (new)
- `specs/spec-lsp-class-member-semantic-tokens.md` (new)
- `specs/spec-lsp-class-extraction.md` (modified)

## Approach

Server side: extend `SymbolKind` with `ClassField` and `ClassMethod`, add `class_owner` to `Symbol`, and walk each `class_specifier` body. Members get `Visibility::Local`. Bump the per-file parse cache from `game_parse/v2/` to `v3/` so stale caches rebuild with member symbols. In `semantic_tokens.rs`, add the `member` modifier and classify `field_expression` nodes by matching the member name against any class in the workspace.

Plugin side: add six member text-attribute keys (member function/variable cross engine/modded/unmodded), register color descriptors, update `XsSemanticTokensConverter` to honor the `member` modifier, and ensure the converter remains unit-testable.

## Affected Areas

| Area | Impact |
|------|--------|
| `tools/xs-language-server/src/symbols.rs` | New kinds, `class_owner`, class-body walker |
| `tools/xs-language-server/src/semantic_tokens.rs` | `member` modifier + `field_expression` classifier |
| `tools/xs-language-server/src/cache.rs` | `game_parse/v2/` → `v3/` schema bump |
| `tools/intellij-xs-plugin/.../highlight/` | New member keys/descriptors/converter branches |
| `tools/intellij-xs-plugin/gradle.properties` | `pluginVersion = 0.8.0` |

## Risks

| Risk | Mitigation |
|------|------------|
| Member-name ambiguity mis-colors references when two classes share a name | Document heuristic; future type-inference slice can refine |
| `field_expression` may color array builtins if names collide | Avoid emitting tokens when no class member matches |
| Cache schema bump forces re-parse of ~300 vanilla files | Verify one-time startup cost in integration test |
| Malformed class bodies may produce `ERROR` nodes | Guard extraction with node-kind checks |
| Plugin color-page display-name drift | Keep strings synchronized with a dedicated test |

## pluginVersion Impact

**MINOR: `0.7.0 → 0.8.0`.** A new LSP feature (member extraction), a new semantic-token modifier, and new plugin color categories are user-visible additions.

## Slice Plan

**One slice / single PR.** The LSP symbol extraction, semantic-token emission, and plugin color wiring are tightly coupled; shipping them separately would leave each partial. The user's preflight has no line cap, and the estimated 600–750 changed lines is within that budget.

## Rollback Plan

Revert the PR. If cached symbols cause issues, delete `~/.local/state/aomr_lsp/game_parse/v3/` (or simply `~/.local/state/aomr_lsp/`) and reinstall the previous `0.7.0` zip if needed.

## Dependencies

- Bundled IntelliJ Platform must continue to expose `LspSemanticTokensSupport`/`lspSemanticTokensSupport` (already required at `2024.2.2`).

## Open Questions

1. Exact display names for member descriptors in the plugin Color Scheme page.
2. Whether `getTokenTypes()` must be updated to advertise any new modifier-aware categories (platform modifiers are typically not advertised as types; verify during apply).

## Acceptance Criteria

- [ ] `cargo test` passes new LSP tests for class field/method extraction and member semantic tokens.
- [ ] `./gradlew test` passes plugin converter tests for member origin combos.
- [ ] The XS Color Scheme page shows member function and member variable groups, and the plugin version is `0.8.0`.
- [ ] A document outline shows class members inside their parent class.
- [ ] A workspace symbol search can find a class member by name.
