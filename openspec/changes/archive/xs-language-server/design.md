# Design: `xs-language-server` (workspace & engine-data redesign)

## 1. Overview

The redesigned Rust XS Language Server models each Age of Mythology: Retold mod as an independent virtual project built from three layers: engine API extracted from `doxygen_retail.7z`, the vanilla `game/` folder, and the mod's `game/` overlay. It keeps the proven tree-sitter parser and LSP handlers from the spike and adds Doxygen extraction with SHA-256 caching, per-file parse caching, cross-file `const`/`extern`/`mutable` semantics, and client-side file watching. This implements `spec-engine-data-pipeline.md`, `spec-virtual-project-overlay.md`, `spec-file-watching-cache-invalidation.md`, `spec-semantic-diagnostics.md`, `spec-intellij-client-integration.md`, and `spec-vscode-removal.md`.

## 2. Module layout

Crate: `tools/xs-language-server/`

| File | Action | Responsibility |
|------|--------|----------------|
| `src/main.rs` | Create | CLI parsing (`--game-path`, positional, `AOMR_GAME_PATH`), game-path validation, cache + engine bootstrap, `LspService` startup. |
| `src/doxygen.rs` | Create | 7z extraction (`sevenz-rust`), Doxygen HTML parsing (`scraper`), emitting engine-api JSON that matches existing `Syscall` / `AiplanConstant` shapes. |
| `src/cache.rs` | Create | `~/.local/state/aomr_lsp/v1/<hash>.json` for engine data; `game_parse/v1/<mtime>-<sha256>.json` per-file parse cache; `dirs` + SHA-256 key derivation. |
| `src/workspace.rs` | Create | Virtual-project construction; mod registration; overlay resolution; `include` lookup with include-root inference; per-file parse-cache lookup. |
| `src/semantic.rs` | Create | `extern` collision detection, forward-declaration validation, `mutable` redefinition equality, cross-file symbol resolution. |
| `src/server.rs` | Extend | Workspace-folder tracking, virtual-project selection, file watcher registration, `window/showMessage` for unowned files, scoped workspace symbols, existing handlers preserved. |
| `src/engine_api.rs` | Extend | Keep `Syscall`, `Param`, `AiplanConstant`; remove sibling-JSON `load_default`; load from `cache.rs` result. |
| `src/symbols.rs` | Extend | Record `extern`/`mutable`/forward declarations; distinguish file-local vs exported symbols. |
| `src/typecheck.rs` | Extend | Integrate with virtual-project symbol index; allow int↔float widening; resolve cross-file function types. |
| `src/completion.rs` | Extend | Combine engine API + exported project symbols; hide file-local vars outside their file. |
| `src/diagnostics.rs` | Extend | Wire semantic diagnostics (extern collision, forward decl, undefined symbol) into publish path. |
| `src/parser.rs` | Keep | tree-sitter wrapper. |
| `src/word.rs` | Keep | Identifier extraction. |
| `src/references.rs` | Extend later | Per-file walks kept; workspace-wide expansion after virtual project is stable. |
| `src/rename.rs` | Create later | LSP rename using workspace index. |
| `tree-sitter-xs/grammar.js` | Keep | Existing XS grammar. |

Snippet: `src/main.rs` bootstrap flow

```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let game_path = resolve_game_path()?; // --game-path > positional > AOMR_GAME_PATH
    let archive = game_path.join("doxygen_retail.7z");
    ensure!(archive.exists(), "doxygen_retail.7z not found at {game_path:?}");
    let engine = cache::load_or_extract_engine(&archive).await?;
    let server = XsLanguageServer::new(client, engine, game_path);
    ...
}
```

## 3. Data flow

### 3.1 LSP startup

```
main()
  |
  v
resolve_game_path() -- CLI arg > positional > env AOMR_GAME_PATH
  |
  +-- missing? -> exit with error
  |
  v
validate doxygen_retail.7z exists
  |
  v
sha2(7z file) -> cache key
  |
  +-- ~/.local/state/aomr_lsp/v1/<hash>.json exists & valid? -> load engine
  |
  +-- miss or corrupt -> sevenz-rust extract -> scraper parse HTML
          |
          v
    serialize Syscalls + Aiplans JSON -> write v1/<hash>.json -> load engine
          |
          v
    Server ready
```

### 3.2 File open

```
Client -> textDocument/didOpen(uri, text)
              |
              v
        server::find_owning_mod(uri)
              |
              +-- unowned -> window/showMessage("engine API only")
              |             publish parse diagnostics only
              |
              +-- owned -> workspace::VirtualProject for that mod
                            |
                            v
                      parse + cache lookup (workspace.rs)
                            |
                            v
                      symbols::build_symbol_table with extern/mutable flags
                            |
                            v
                      semantic::diagnose(project)
                            |
                            v
                      typecheck::check_calls with cross-file lookup
                            |
                            v
                      client.publish_diagnostics(uri, diags, version)
```

### 3.3 Game file change

```
Client -> workspace/didChangeWatchedFiles
              |
              v
        cache::invalidate_parse(uri) if changed/deleted
              |
              v
        workspace::invalidate_dependents(uri)  // files with include graph edge
              |
              v
        Re-diagnose open mod files affected by the change
              |
              v
        publish_diagnostics for affected URIs
```

### 3.4 Mod add

```
Client -> workspace/didChangeWorkspaceFolders({added: [mod_uri]})
              |
              v
        workspace::register_mod(mod_uri)
              |
              v
        Build VirtualProject overlay map for mod_uri/game/
              |
              v
        Ready for didOpen; existing open files remap if now owned
```

## 4. LSP protocol surface

Capabilities advertised in `initialize` (initial result from existing `server.rs` extended):

```rust
ServerCapabilities {
    text_document_sync: Some(TextDocumentSyncCapability::Options(
        TextDocumentSyncOptions {
            open_close: Some(true),
            change: Some(TextDocumentSyncKind::FULL),
            will_save: Some(true),
            will_save_wait_until: Some(true),
            save: Some(SaveOptions::default().into()),
        },
    )),
    completion_provider: Some(CompletionOptions {
        resolve_provider: Some(false),
        trigger_characters: Some(vec![".".into(), "_".into()]),
        ..Default::default()
    }),
    hover_provider: Some(HoverProviderCapability::Simple(true)),
    definition_provider: Some(OneOf::Left(true)),
    references_provider: Some(OneOf::Left(true)),
    rename_provider: Some(OneOf::Right(RenameOptions {
        prepare_provider: Some(true),
        ..Default::default()
    })),
    document_symbol_provider: Some(OneOf::Left(true)),
    workspace_symbol_provider: Some(OneOf::Left(true)),
    workspace: Some(WorkspaceServerCapabilities {
        workspace_folders: Some(WorkspaceFoldersServerCapabilities {
            supported: Some(true),
            change_notifications: Some(OneOf::Left(true)),
        }),
        file_operations: None,
    }),
    diagnostic_provider: Some(DiagnosticServerCapabilities::Options(
        DiagnosticOptions {
            identifier: Some("xs-language-server".to_string()),
            inter_file_dependencies: true,
            workspace_diagnostics: false,
            work_done_progress_options: Default::default(),
        },
    )),
    ..Default::default()
}
```

The server also dynamically registers a watcher for `<game-path>/game/**/*.xs` if the client supports `workspace/didChangeWatchedFiles`.

## 5. Cache layout

Base directory: `dirs::state_dir().map(|d| d.join("aomr_lsp")).unwrap_or_else(|| home_dir().join(".aomr_lsp"))`

| Path | Content | Key |
|------|---------|-----|
| `v1/<sha256-of-7z>.json` | Extracted syscalls + aiplans JSON | SHA-256 of `doxygen_retail.7z` |
| `game_parse/v1/<mtime>-<sha256>.json` | Per-file parse tree + symbol table | `mtime` and SHA-256 of file content |
| `v0/` / old version dirs | Left in place, ignored | N/A |

The `v1/` schema prefix allows future incompatible bumps without deleting or overwriting old cache files.

## 6. Workspace model

Each registered workspace folder maps to one `VirtualProject`:

```
mod/<name>/game/<rel>        ← overlay (REPLACES game folder file at same rel)
<game-path>/game/<rel>       ← game folder (only if no overlay)
<doxygen extracted API>      ← engine API (always present)
```

- **Overlay**: a mod file at relative path `R` hides `<game-path>/game/R` from all analysis consumers.
- **Ownership**: A file is owned by the longest registered workspace-folder URI prefix that contains it.
- **Unowned files**: `window/showMessage("file not part of any registered mod; engine API only")`; only engine-based features are offered.
- **Include-root inference**: determined from the file's relative path under `game/`:
  - `game/ai/...` -> root `game/ai/`
  - `game/data/trigger/...` -> root `game/data/trigger/`
  - `game/random_maps/...` -> root `game/random_maps/`
- **Include resolution** for `include "foo.xs"` in file `game/<root>/<dir>/file.xs`:
  1. `<mod>/game/<root>/<dir>/foo.xs`
  2. `<game-path>/game/<root>/<dir>/foo.xs`
- **Cross-file visibility**:
  - Variables: only `extern` symbols visible across files without `include`.
  - Functions: visible across files if defined before the call site or if `mutable`.
  - `include` files behave like textual paste.
- **Workspace symbols**: scope limited to the owning virtual project.

## 7. IntelliJ plugin wiring

Package: `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/`

| File | Action | Responsibility |
|------|--------|----------------|
| `settings/XsSettings.kt` | Create | `PersistentStateComponent` storing `gamePath: String`, `modPaths: List<String>`. |
| `settings/XsConfigurable.kt` | Create | Settings UI under Settings → Languages & Frameworks → XS; game-folder text field; mod list with +/−; Auto-detect button. |
| `settings/XsModAutoDetector.kt` | Create | Recursive scan for directories named exactly `game`; stop recursion at each `game/` boundary; return parent of each `game/` as mod root. |
| `lsp/XsLspServerManager.kt` | Create/extend | Spawn Rust binary with `--game-path`; send `workspace/didChangeWorkspaceFolders`; register `didChangeWatchedFiles` watcher; restart on game-path change; resend on mod list change. |
| `META-INF/plugin.xml` | Modify | Add `<projectConfigurable>` for `XsConfigurable`; remove engine-facing extension points (`completion.contributor`, `documentationProvider`, `parameterInfoHandler`, `psi.referenceContributor` for engine). |
| `completion/`, `documentation/`, `parameterInfo/`, `navigation/XsEngineReferenceContributor.kt`, `constants/XsEngineApi.kt`, `constants/XsAiPlans.kt` | Remove | Replaced by LSP. |
| `extracted/xs.vsix` | Remove (`git rm`) | See `spec-vscode-removal.md`. |
| `.gitignore` exception | Remove | Delete `!extracted/xs.vsix` if present. |
| `syntaxes/xs.tmLanguage.json` | Vendor | Ensure committed at `src/main/resources/syntaxes/xs.tmLanguage.json`. |

`build.gradle.kts` currently validates `syscalls.json`, `aiplans.json`, and `syntaxes/xs.tmLanguage.json` as bundled resources. After the engine data moves to the LSP, the validation task for `syscalls.json` and `aiplans.json` should be removed; only `syntaxes/xs.tmLanguage.json` remains required.

## 8. First-run UX

1. IDE startup → `StartupActivity.DumbAware` runs.
2. Read `XsSettings.gamePath`; if empty, open Settings UI prompting for it.
3. Read `XsSettings.modPaths`; if empty, run `XsModAutoDetector.scan()` and populate the list.
4. If detection returns nothing, show a non-blocking `NotificationGroup.WARNING`:
   > "Couldn't find any mod folders. Add paths in Settings → XS Language Server → Mods."
5. Spawn Rust LSP via `XsLspServerManager` with `--game-path <gamePath>`.
6. LSP validates `<gamePath>/doxygen_retail.7z`; exits immediately with a descriptive error if missing.
7. Plugin sends `workspace/didChangeWorkspaceFolders` with detected mod URIs.
8. Plugin registers `didChangeWatchedFiles` watcher for `<gamePath>/game/**/*.xs`.

## 9. Performance budgets

| Scenario | Budget | How met |
|----------|--------|---------|
| Cold start | < 10 s | 7z extraction + HTML scraping + JSON serialization; one-time per archive. |
| Warm start | < 1 s | SHA-256 of archive, file existence check, JSON deserialize. |
| Per keystroke | < 200 ms | Incremental parse of one file + symbol lookup in cached project tables. |
| Initial game-folder parse | amortized | Per-file `game_parse/v1/` cache keyed by mtime+sha256. |

## 10. Testing strategy

| Layer | What | Approach |
|-------|------|----------|
| Unit | 7z extraction output count/schema vs committed JSON; cache key derivation; overlay resolution; include-root inference; extern collision; mutable redefinition equality; int↔float compatibility. | `#[test]` in `doxygen.rs`, `cache.rs`, `workspace.rs`, `semantic.rs`, `typecheck.rs`. |
| Integration | Full LSP round-trip per `lsp_roundtrip_test.rs` pattern extended with `didChangeWorkspaceFolders`, `didChangeWatchedFiles`, semantic diagnostics. | `src/bin/lsp_roundtrip_test.rs` or `#[test]` using tower-lsp test client. |
| Fixtures | Synthetic mini game folders + mini mods covering forward-decl, extern, mutable, include contexts. | `tests/fixtures/` checked in as plaintext `.xs` files. |
| Manual | Compare LSP diagnostics with AoM:R engine load errors. | Documented separately in verification notes. |

## 11. Crate dependencies

Additions to `Cargo.toml`:

```toml
[dependencies]
sevenz-rust = "0.6"
scraper = "0.21"
sha2 = "0.10"
dirs = "5"
anyhow = "1"
```

Existing and retained:
- `tower-lsp`, `tokio`, `serde`, `serde_json`, `tree-sitter`, `tree-sitter-xs`, `lsp-types`, `tracing`, `tracing-subscriber`.

## 12. Migration plan

| Phase | Focus | Files |
|-------|-------|-------|
| 1 | 7z extraction + cache + CLI game-path parsing | New `main.rs`, `doxygen.rs`, `cache.rs`; modify `engine_api.rs` loader. |
| 2 | Virtual project + mod registration + overlay + include | New `workspace.rs`. |
| 3 | Semantic diagnostics | New `semantic.rs`; extend `symbols.rs`, `diagnostics.rs`. |
| 4 | IntelliJ settings UI + auto-detect + LSP lifecycle | New Kotlin settings classes; extend `plugin.xml`; create `XsLspServerManager`. |
| 5 | VS Code removal + TextMate grammar vendoring | Remove `extracted/xs.vsix`, update `.gitignore`, ensure `xs.tmLanguage.json` present. |

Rollback: design work occurs on a new branch (`xs-language-server/redesign`); `xs-language-server/spike` remains untouched; IntelliJ plugin can revert to file-type-only state if client changes break.

## 13. Risks and mitigations

Restated from `explore.md` with concrete mitigations:

| Risk | Mitigation |
|------|------------|
| `sevenz-rust` / `scraper` cannot meet cold-start budget or parse Doxygen HTML reliably. | Day-0 spike task to extract `docs/doxygen_retail.7z`, compare counts against committed JSON, measure time; scraper skips malformed entries and logs warnings; never silently emits empty engine API. |
| Doxygen HTML format drift on game patches. | Tolerant scraping; validation that critical HTML files produce expected syscall counts; warnings, not hard failures. |
| Overlay / include path bookkeeping errors. | Dedicated `workspace.rs` with unit tests for path mapping, replacement, include lookup. |
| Forward-decl / `extern` false positives. | Extensive fixtures; capture real engine error messages and compare manually. |
| Per-keystroke latency on large projects. | Per-file parse cache; only re-parse changed files; incremental symbol-table update. |
| IntelliJ client capability gaps (dynamic watcher registration). | Feature-detect client capabilities; degrade to stale diagnostics manual refresh when unsupported. |
| Cache directory missing / unwritable. | Fallback from `dirs::state_dir()` to `~/.aomr_lsp`. |

## 14. References

Inputs read for this design:

- `openspec/changes/xs-language-server/explore.md`
- `openspec/changes/xs-language-server/proposal.md`
- `openspec/changes/xs-language-server/specs/README.md`
- `openspec/changes/xs-language-server/specs/spec-engine-data-pipeline.md`
- `openspec/changes/xs-language-server/specs/spec-virtual-project-overlay.md`
- `openspec/changes/xs-language-server/specs/spec-file-watching-cache-invalidation.md`
- `openspec/changes/xs-language-server/specs/spec-semantic-diagnostics.md`
- `openspec/changes/xs-language-server/specs/spec-intellij-client-integration.md`
- `openspec/changes/xs-language-server/specs/spec-vscode-removal.md`
- `openspec/changes/intellij-xs-plugin/design.md`
- `tools/xs-language-server/src/` (current Rust LSP implementation)
- `tools/xs-language-server/Cargo.toml`
- `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/` (current plugin code)
- `tools/intellij-xs-plugin/build.gradle.kts`
- `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`
- `docs/xs-lsp-spike.md`
- `AGENTS.md`
- `openspec/config.yaml`

Relevant prior artifacts:
- `docs/doxygen_retail.7z` and `docs/doxygen_retail/`
- `mod/*/game/ai/human_assist/human_assist.xs`
- `scripts/deploy-mods.sh`
