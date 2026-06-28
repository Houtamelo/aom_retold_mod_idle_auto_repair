# Proposal: xs-lsp-0.1.7-diagnostics-ux

## Intent
Fix four user-reported LSP server bugs so diagnostics clear reliably after edits, parse errors are actionable, and engine-API symbols (`kbUnitCount`, `aiEcho`, etc.) behave correctly in Find References and Go to Definition. Only the Rust server under `tools/xs-language-server/` is touched; IDE-side work stays in the separate `xs-plugin-0.2.0-ui-polish` change.

## Scope

### In Scope
- Clear stale diagnostics by publishing an empty array when a file becomes clean.
- Rewrite parse-error messages for `MISSING`/`ERROR` tree-sitter nodes into actionable text.
- Return real use-site locations for references on engine-API symbols.
- Stop returning non-navigable `xs-stub://engine/<name>` URIs for engine-API definitions.
- Bump `pluginVersion` 0.1.6 → 0.1.7 and add tests under strict TDD.

### Out of Scope
- TextMate language-ID alignment / syntax highlighting.
- ColorScheme / `<colorSettingsPage>` registration.
- Any Kotlin or plugin XML changes.

## Capabilities

### New Capabilities
- `diagnostic-clearance`: publish empty diagnostics when a previously-dirty file becomes clean.
- `parse-error-ux`: generate human-readable parse error messages.

### Modified Capabilities
- `engine-api-resolution`: extend resolution to support references and honest definition responses for engine symbols.

## Approach
1. **Stale diagnostics**: ensure `collect_all` inserts an empty entry for `current_uri` when no diagnostics exist, and verify `publish_diagnostics` sends it. Update the stale doc comment at `server.rs:912-914`.
2. **Parse messages**: in `to_diagnostic`, map `"MISSING"` to `"Missing '<expected-token>'"` using `LookaheadNamesIterator`; map `"ERROR"` to `"Unexpected token '<actual>'"` from node text.
3. **Engine references**: when the symbol is engine API, walk workspace `.xs` files and use `references::find_identifier_uses` to return use-site locations.
4. **Engine definition**: return `Ok(None)` for engine symbols with no source location, removing stub URIs. Update the roundtrip test assertions accordingly.

## Affected Areas

| Area | Impact | Description |
|------|--------|-------------|
| `tools/xs-language-server/src/diagnostics.rs` | Modified | Clean-file map entry; parse message formatting. |
| `tools/xs-language-server/src/server.rs` | Modified | Diagnostic publish, references, definition, doc comment. |
| `tools/xs-language-server/src/references.rs` | Reused | Identifies engine-symbol use sites across workspace files. |
| `tools/xs-language-server/src/bin/lsp_roundtrip_test.rs` | Modified | Expect empty diagnostics and null engine-symbol definition. |
| `tools/intellij-xs-plugin/gradle.properties` | Modified | `pluginVersion` 0.1.6 → 0.1.7. |

## Risks

| Risk | Likelihood | Mitigation |
|------|------------|------------|
| Existing roundtrip test asserts stub-URI definition behavior | High | Update test assertions in the same change. |
| `LookaheadNamesIterator` may return raw grammar symbols | Med | Map common expected tokens; iterate via tests. |
| Workspace-wide scan for engine references could regress performance | Low | Reuse existing workspace index; profile if needed. |
| Doc-comment mismatch (`server.rs:912-914`) may confuse reviewers | Med | Update comment and add a test proving empty publish. |

## Rollback Plan
Revert the Rust-only commit(s). No XS mod files or plugin code change, so rollback restores the previous `cargo test` baseline and prior plugin behavior.

## Dependencies
None beyond the existing `tree-sitter` 0.26.9 dependency and the workspace engine-API cache.

## Success Criteria
- [ ] `cargo test` passes and `cargo run --bin lsp_roundtrip_test` passes.
- [ ] A second `didChange` that produces zero diagnostics publishes an empty diagnostic array.
- [ ] Parse error messages no longer expose raw `"MISSING"`/`"ERROR"` node kinds.
- [ ] `aiEcho` references return at least one real workspace location; `aiEcho` definition returns `null`.
- [ ] `pluginVersion` in `gradle.properties` is `0.1.7`.
