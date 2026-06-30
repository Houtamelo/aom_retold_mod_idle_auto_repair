# Design: `fix-goto-definition-on-include-statements`

## Goal

Extend the XS LSP server's `textDocument/definition` handler so that a cursor inside an `include "..."` path token resolves to the target file through the existing mod-overlay → vanilla `game/` pipeline. The IntelliJ plugin forwards the resulting `Location` unchanged (Issue #1's handler already does this), so all three triggers (Ctrl+Click, keybind, right-click → Go to → Definition) work without new plugin navigation code.

## Status

Design phase — no code changes yet.

## LSP architecture

### Call flow

The existing handler is `XsLanguageServer::goto_definition` in `tools/xs-language-server/src/server.rs:638-696`. The new include-aware branch runs **before** the existing `word::identifier_at_cursor` call at line 649.

```rust
async fn goto_definition(
    &self,
    params: GotoDefinitionParams,
) -> Result<Option<GotoDefinitionResponse>> {
    let uri = &params.text_document_position_params.text_document.uri;
    let pos = params.text_document_position_params.position;
    let text = {
        let docs = self.documents.lock().await;
        docs.get(uri).unwrap_or("").to_string()
    };

    // -----------------------------------------------------------------
    // Phase 0: include-directive path (new branch)
    // -----------------------------------------------------------------
    if let Some(current_file) = uri.to_file_path().ok() {
        if let Some(include_target) =
            parser::detect_include_path_at_position(&current_file, &text, pos.line, pos.character)
        {
            let (ws_clone, project) = {
                let ws = self.workspace.lock().await;
                let entry = ws.lookup_mod(uri);
                let project = match entry {
                    Some(e) => ws.build_virtual_project(e),
                    None => workspace::VirtualProject::default(),
                };
                (ws.clone(), project)
            };

            if let Some(resolved) = ws_clone
                .resolve_include_for_file(&project, &current_file, &include_target)
            {
                let def_uri = Url::from_file_path(&resolved)
                    .map_err(|_| tower_lsp::jsonrpc::Error::internal_error())?;
                let range = Range::new(Position::new(0, 0), Position::new(0, 0));
                return Ok(Some(GotoDefinitionResponse::Scalar(Location {
                    uri: def_uri,
                    range,
                })));
            }
            return Ok(None);
        }
    }

    // -----------------------------------------------------------------
    // Phase 1: existing identifier resolution (unchanged)
    // -----------------------------------------------------------------
    let Some(ident) = word::identifier_at_cursor(&text, pos.line, pos.character) else {
        debug!("definition: no identifier at {:?}", pos);
        return Ok(None);
    };

    // ... existing merged-view / symbol-table code remains exactly as is ...
}
```

### Where the branch is inserted

- `tools/xs-language-server/src/server.rs:649` — `word::identifier_at_cursor` must remain the **fallback**, not the first check.
- The include branch is placed **immediately after the `text` lookup** (`server.rs:644-647`) and **before** line 649.
- `current_file` is obtained from `uri.to_file_path()`; if the URI is not a `file://` URI, the branch is skipped and identifier resolution runs normally.

## The parser helper

Add `detect_include_path_at_position` to `tools/xs-language-server/src/parser.rs`. It parses the source with tree-sitter, finds an `include_directive` node whose range contains the cursor, verifies the cursor is inside its `path` (`string_literal`) child, and returns the unquoted path.

```rust
use std::path::Path;

/// If the cursor lies inside the path token of an `include_directive`,
/// return the unquoted include target. Otherwise return `None`.
///
/// `file` is accepted for diagnostic/logging symmetry but is not used by
/// the current implementation.
pub fn detect_include_path_at_position(
    _file: &Path,
    source: &str,
    line: u32,
    col: u32,
) -> Option<String> {
    let tree = parse(source)?;
    let root = tree.root_node();
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() != "include_directive" {
            continue;
        }
        if !range_contains(child, line, col) {
            continue;
        }
        let Some(path_node) = child.child_by_field_name("path") else {
            continue;
        };
        if path_node.kind() != "string_literal" {
            continue;
        }
        if !range_contains(path_node, line, col) {
            // Cursor is on the `include` keyword or the trailing `;` but
            // not inside the quoted path — fall through to identifier logic.
            return None;
        }
        let text = &source[path_node.byte_range()];
        let target = text
            .strip_prefix('"')
            .and_then(|s| s.strip_suffix('"'))
            .unwrap_or(text);
        return Some(target.to_string());
    }

    None
}

fn range_contains(node: tree_sitter::Node<'_>, line: u32, col: u32) -> bool {
    let start = node.start_position();
    let end = node.end_position();

    let after_start = line > start.row as u32
        || (line == start.row as u32 && col >= start.column as u32);
    let before_end = line < end.row as u32
        || (line == end.row as u32 && col < end.column as u32);

    after_start && before_end
}
```

`node_range` (already private in `parser.rs:43-50`) can be reused if the helper is later extended to return the path range; for containment logic the raw `start_position` / `end_position` checks above are sufficient.

### Behavior notes

- `include_directive` nodes are top-level children of the XS file in the tree-sitter grammar (`grammar.js:113-117`). The helper walks only the root children, so the search is O(number of top-level statements).
- Multi-line string literals are still a single `string_literal` node, so the row/column containment check works unchanged.
- If the cursor is on the `include` keyword or the semicolon, the helper returns `None`, which lets the handler fall through to identifier resolution.

## The Workspace helper

The existing `Workspace::resolve_include` (`tools/xs-language-server/src/workspace.rs:249-262`) is already `pub`, but its signature requires the caller to supply the includer's `game/`-relative path and a `VirtualProject`:

```rust
pub fn resolve_include(
    &self,
    project: &VirtualProject,
    from_rel: &str,
    include_target: &str,
) -> Option<PathBuf>
```

`server.rs` has the absolute `current_file` path, not the relative path, so add a convenience wrapper:

```rust
impl Workspace {
    /// Resolve `include_target` from an absolute source file path.
    ///
    /// Computes the file's relative path under the mod overlay (if it is an
    /// override) or the vanilla `game/` folder, then delegates to
    /// [`Self::resolve_include`]. Returns `None` if the file is outside both
    /// contexts or the target cannot be resolved.
    pub fn resolve_include_for_file(
        &self,
        project: &VirtualProject,
        from_file: &Path,
        include_target: &str,
    ) -> Option<PathBuf> {
        let from_rel = self.relative_path_for_file(project, from_file);
        self.resolve_include(project, &from_rel, include_target)
    }

    /// Reverse-map an absolute file path to the relative path used for
    /// include-root detection. Mirrors `merged_view::relative_path_for`.
    fn relative_path_for_file(&self, project: &VirtualProject, abs_path: &Path) -> String {
        project
            .file_overrides
            .iter()
            .find(|(_, p)| *p == abs_path)
            .map(|(rel, _)| rel.clone())
            .or_else(|| self.game_relative_path(abs_path))
            .unwrap_or_else(|| abs_path.to_string_lossy().into_owned())
    }
}
```

### Resolution order

`Workspace::resolve_include_for_file` keeps the existing precedence:

1. **Mod overlay** — `project.file_overrides.get(rel)` wins if the resolved relative path exists in the overlay.
2. **Vanilla `game/`** — falls back to `self.game_path.join("game").join(rel)`.
3. **Non-`.xs` rejection** — `resolve_file` returns `None` for non-readable/non-`.xs` paths, matching the merged-view safety rules.

If neither context yields a file, the handler returns `Ok(None)` (empty response), matching current behavior for unresolvable identifiers.

## Test fixtures

Create `tools/xs-language-server/tests/goto_include_repro.rs`. The tests use the same spawn-the-server + JSON-RPC pattern as `tests/r5_test_honesty_repro.rs`.

### Shared test helper shape

```rust
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::path::PathBuf;

fn frame(body: &str) -> Vec<u8> { /* same as r5_test_honesty_repro.rs */ }

fn read_response_with_id<R: Read>(reader: &mut R, expected_id: i64) -> serde_json::Value { /* same */ }

fn resolve_doxygen_archive() -> PathBuf { /* copy from r5_test_honesty_repro.rs */ }

fn spawn_with_fixture(vanilla_files: &[(&str, &str)], mod_files: &[(&str, &str)]) -> Fixture { ... }

fn definition_request(uri: &Url, line: u32, character: u32, request_id: i64) -> String { ... }
```

Each test:

1. Creates a temp `game/` tree (and optional mod overlay).
2. Copies `docs/doxygen_retail.7z` into the temp game root.
3. Spawns `CARGO_BIN_EXE_xs-language-server --game-path <temp_game>`.
4. Sends `initialize` with the mod folder as a workspace folder (when testing mod overlay).
5. Sends `textDocument/didOpen` for the includer.
6. Sends `textDocument/definition` with the cursor inside the include path.
7. Reads the response and asserts the resolved `uri`.

### Test cases

#### `test_direct_include_resolves_to_target`

- Fixture: `game/ai/core/main.xs` contains `include "debug.xs";`; `game/ai/core/debug.xs` exists.
- No mod registered.
- Cursor on the `d` of `debug.xs` (line 0, character around 9).
- Assert response `result.uri` ends with `game/ai/core/debug.xs`.

#### `test_mod_overlay_resolves_to_mod_file`

- Fixture:
  - Vanilla: `game/ai/core/debug.xs`
  - Mod: `mod_a/game/ai/core/debug.xs`
  - Includer: `mod_a/game/ai/core/main.xs` containing `include "debug.xs";`
- Register `mod_a` as a workspace folder.
- Cursor inside the include path.
- Assert response `result.uri` ends with `mod_a/game/ai/core/debug.xs`, not the vanilla copy.

#### `test_vanilla_fallback_when_mod_missing`

- Fixture:
  - Vanilla: `game/ai/core/debug.xs`
  - Mod: `mod_a/game/ai/core/main.xs` (no overlay for `debug.xs`)
- Includer contains `include "debug.xs";`.
- Register `mod_a`.
- Assert response points to the vanilla `game/ai/core/debug.xs`.

#### `test_not_found_returns_null`

- Fixture: `game/ai/core/main.xs` contains `include "does/not/exist.xs";`.
- No such file in mod or vanilla.
- Assert response `result` is `null` (or an empty array/array of zero length, depending on how `GotoDefinitionResponse` serializes).

#### `test_cursor_outside_path_token_uses_existing_logic`

- Fixture: `game/ai/core/main.xs` contains `include "debug.xs"; void foo() {}`.
- Cursor on the `include` keyword (line 0, character 2).
- Assert response `result` is `null` (no identifier there, so the existing logic also returns `null`).
- Optional stronger variant: place cursor on an identifier on the same line and assert identifier resolution still works.

## Plugin forwarding test

Add one regression test to `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt`.

```kotlin
fun testIncludeStringLiteralDelegatesToResolver() {
    // Source file contains an include directive. The caret is inside the
    // quoted path token, which is the element the platform passes to the
    // handler for Ctrl+Click / Go to Definition.
    val psiFile = myFixture.configureByText("a.xs", "include \"b.xs\";\n")
    val includeStringOffset = psiFile.text.indexOf('"') + 2
    val sourceElement = psiFile.findElementAt(includeStringOffset)!!

    // Simulate the LSP returning a target in an external file (b.xs).
    val externalFile = myFixture.configureByText("b.xs", "void helper() {}\n")
    val externalTarget = externalFile.findElementAt(externalFile.text.indexOf("helper"))!!

    val handler = XsGotoDeclarationHandler(TestResolver(listOf(externalTarget)))
    val result = handler.getGotoDeclarationTargets(
        sourceElement,
        sourceElement.textOffset,
        myFixture.editor
    )

    assertSize(1, result ?: emptyArray())
    assertEquals("b.xs", result!![0].containingFile.name)
}
```

This test proves that when the caret is inside the include path string literal, the handler:

1. Does **not** filter it out (the file extension check still passes because the containing file is `a.xs`).
2. Delegates to the resolver seam.
3. Returns the resolver's target, which for a real LSP response will be the resolved include file.

No new plugin navigation code is required; Issue #1's `XsGotoDeclarationHandler` and `XsLspDefinitionResolver` already convert `Location` URIs into `PsiElement`s via `VirtualFileManager`.

## Strict-TDD task list

1. **Task 1:** Write failing LSP test `test_direct_include_resolves_to_target` in `tools/xs-language-server/tests/goto_include_repro.rs`.
2. **Task 2:** Write failing LSP test `test_mod_overlay_resolves_to_mod_file`.
3. **Task 3:** Write failing LSP test `test_vanilla_fallback_when_mod_missing`.
4. **Task 4:** Write failing LSP test `test_not_found_returns_null`.
5. **Task 5:** Write failing LSP test `test_cursor_outside_path_token_uses_existing_logic`.
6. **Task 6:** Run `cargo test --test goto_include_repro`; confirm all five tests FAIL for the expected reason (include path returns `null`).
7. **Task 7:** Add `detect_include_path_at_position` (and `range_contains`) in `tools/xs-language-server/src/parser.rs`.
8. **Task 8:** Add `Workspace::resolve_include_for_file` and `Workspace::relative_path_for_file` in `tools/xs-language-server/src/workspace.rs`.
9. **Task 9:** Extend `XsLanguageServer::goto_definition` in `tools/xs-language-server/src/server.rs` with the Phase 0 include branch before line 649.
10. **Task 10:** Run `cargo test --test goto_include_repro`; confirm PASS.
11. **Task 11:** Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml`; confirm 215+ tests pass.
12. **Task 12:** Add `testIncludeStringLiteralDelegatesToResolver` in `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt`.
13. **Task 13:** Run `./gradlew :test`; confirm the new plugin test and all existing tests pass.
14. **Task 14:** Bump `pluginVersion` from `0.3.0` to `0.4.0` in `tools/intellij-xs-plugin/gradle.properties:12`.
15. **Task 15:** Run `./gradlew :buildPlugin`; confirm a `.zip` artifact is produced in `tools/intellij-xs-plugin/build/distributions/`.
16. **Task 16:** Update `docs/issues/2026-06-29-runtime-issues.md` Issue 4 status to **Resolved** and add a note referencing this change.
17. **Task 17:** Commit (handled by the orchestrator).

## ADRs

### ADR-1: Pure LSP fix vs split between LSP and plugin

- **Context:** Include resolution already lives in the LSP `Workspace` (mod overlay + vanilla fallback). Issue #1 added a plugin `GotoDeclarationHandler` that forwards LSP `Location`s unchanged.
- **Decision:** Implement detection and resolution entirely in the LSP server's `textDocument/definition` handler. The plugin receives the new `Location` through the existing forwarding path.
- **Consequences:** Single source of truth for include semantics; no Kotlin duplication of `Workspace::resolve_include`; all three editor triggers work simultaneously.
- **Alternatives:** Reimplement resolution in Kotlin (rejected: duplicate logic) or combined LSP+plugin fallback (rejected: overkill, no known gaps).

### ADR-2: Cursor-position comparison via tree-sitter node range

- **Context:** The handler must decide whether the cursor is on the include path token, the `include` keyword, or the semicolon.
- **Decision:** Use tree-sitter's `start_position` / `end_position` to test whether the LSP `(line, character)` point is inside the `string_literal` node of an `include_directive`.
- **Consequences:** Multi-line include paths work without extra byte-offset arithmetic; positions on the quote characters are accepted as navigation targets (harmless over-trigger).
- **Alternatives:** Manual byte-offset math on the source string (rejected: brittle for multi-byte characters and multi-line strings).

### ADR-3: Target range `(0,0)-(0,0)` for include results

- **Context:** The LSP `Location` returned for an include path should open the target file, but there is no meaningful single symbol to select inside an arbitrary included file.
- **Decision:** Return `Range::new(Position::new(0, 0), Position::new(0, 0))` for every include result.
- **Consequences:** The editor jumps to the top of the included file. Future work could improve this to the first top-level declaration.
- **Alternatives:** Return the full file range or no range (rejected: `(0,0)` is the conventional LSP "jump to file" signal).

### ADR-4: Workspace include resolution precedence

- **Context:** A mod may overlay a vanilla file at the same relative path under `game/`.
- **Decision:** `resolve_include_for_file` reuses the existing `resolve_file` precedence: mod overlay first, vanilla `game/` second.
- **Consequences:** Navigating from a mod file lands on the mod's version of the included target, matching the runtime behavior of the include graph.
- **Alternatives:** Always resolve to vanilla (rejected: would contradict the mod overlay model) or prefer the includer's directory (rejected: XS includes are rooted at `ai/`, `data/trigger/`, or `random_maps/`, not relative to the includer).

### ADR-5: Fall through to identifier resolution when include detection fails

- **Context:** If the cursor is on the `include` keyword, the semicolon, or a malformed directive, `detect_include_path_at_position` returns `None`.
- **Decision:** The handler falls through to the existing `word::identifier_at_cursor` branch.
- **Consequences:** Existing go-to-definition behavior is preserved for non-path positions; malformed include directives do not produce new error responses.
- **Alternatives:** Return `Ok(None)` immediately when inside any `include_directive` (rejected: would suppress identifier resolution on the `include` keyword, which some users might expect to query as a symbol, and would be a behavior change).

## File changes

| File | Action | Description |
|------|--------|-------------|
| `tools/xs-language-server/src/server.rs` | Modify | Add Phase 0 include branch in `goto_definition` before line 649. |
| `tools/xs-language-server/src/parser.rs` | Modify | Add `detect_include_path_at_position` and `range_contains` helpers. |
| `tools/xs-language-server/src/workspace.rs` | Modify | Add `resolve_include_for_file` and private `relative_path_for_file`. |
| `tools/xs-language-server/tests/goto_include_repro.rs` | Create | Strict-TDD LSP regression tests. |
| `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` | Modify | Add `testIncludeStringLiteralDelegatesToResolver`. |
| `tools/intellij-xs-plugin/gradle.properties` | Modify | Bump `pluginVersion` `0.3.0` → `0.4.0`. |
| `docs/issues/2026-06-29-runtime-issues.md` | Modify | Mark Issue 4 as resolved. |

## Testing strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | Include-path detection | `parser.rs` internal tests for `detect_include_path_at_position` (optional fast unit tests). |
| Unit | Workspace resolution | Existing `workspace.rs` tests for `resolve_include` cover overlay/fallback; new helper exercised by integration tests. |
| Integration | LSP include navigation | `tests/goto_include_repro.rs` spawns `xs-language-server` and asserts `textDocument/definition` responses. |
| Integration | Plugin forwarding | `XsGotoDeclarationHandlerTest.kt` asserts the handler delegates an include-path element to the resolver. |
| Build | Plugin artifact | `./gradlew :buildPlugin` produces a `.zip`; version is `0.4.0`. |

## Out-of-scope reminders

- **Issues 1, 2, 3** from `docs/issues/2026-06-29-runtime-issues.md` are explicitly not this change.
- **Plugin-side navigation logic:** No new `GotoDeclarationHandler`, reference contributor, or `OpenFileDescriptor` path is added. Issue #1's existing handler forwards the LSP result unchanged.
- **Identifier go-to-definition:** No changes beyond preserving the existing fallback.
- **Target-range precision:** `(0,0)-(0,0)` is deliberate; landing on the first declaration is future work.
- **Include completion, hover, or multi-include syntax:** Not touched.
- **XS mod scripts under `mod/`:** No changes to deployed mod source.

## Migration / rollout

No schema, cache, or mod migration is required. A clean revert of the four source files, the new test file, `gradle.properties`, and the issue-doc update restores the previous behavior.

## Risks and mitigation

| Risk | Mitigation |
|------|------------|
| Cursor-position math drifts for multi-line strings | Use tree-sitter node ranges (ADR-2) and include a multi-line test scenario. |
| `relative_path_for_file` diverges from `merged_view::relative_path_for` | Keep the same precedence (overlay key lookup, then `game_relative_path`, then absolute fallback) and add overlay/fallback tests. |
| Plugin does not underline the include string literal | The handler returns target `PsiElement`s; standard IntelliJ Platform behavior makes the source element clickable. Verify with a real plugin build (manual step). |
| Existing identifier definition regresses | `test_cursor_outside_path_token_uses_existing_logic` guards the fallback path; full `cargo test` guards general behavior. |

## Open questions

- None anticipated. The `doxygen_retail.7z` archive path used by integration tests is the same helper as `r5_test_honesty_repro.rs`, so test infrastructure is already proven.
