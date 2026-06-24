# XS Language Server — Rust LSP Spike

**Status:** HISTORICAL — superseded by `openspec/changes/xs-language-server/` (Phase 1–5 complete). Last updated 2026-06-23.
**Owner:** houtamelo
**Goal of the spike:** de-risk the assumption that a Rust LSP can give us "error checks without booting the game" for XS in ~1 week, before committing to the architectural pivot.

---

## Why this exists

The original plan was to build an IntelliJ Platform plugin (`tools/intellij-xs-plugin/`) that ports the VS Code XS extension's feature set to JetBrains IDEs. After completing P0 + P0.5 + P1 (engine API surface, 1,716 Kotlin LOC, 36 tests green), we realized the architecture is the wrong shape for the actual goal.

**The goal** (user statement, 2026-06-23): "I think the end goal would be to have an LSP so that we can run error checks without booting the game."

**The problem with the current plan:**
- The IntelliJ plugin does data lookup (string-match engine API names). It does NOT parse user code.
- It has no parser, no AST, no type checker, no diagnostics.
- P3 and P4 add IDE features (class member completion, XML constants) but DO NOT move toward diagnostics.
- To get diagnostics via the Kotlin-native path: 2-3 more months of P2 + a new "diagnostics phase" + type-inference work — and even then IntelliJ's type system is JVM-centric, ill-suited to a scripting language.

**The pivot:** build a Rust LSP server (the "real" language implementation), and have the IntelliJ plugin (and the existing VS Code extension) become thin LSP clients.

---

## The spike plan (one week, low-risk)

If any single day fails, the spike fails — abort and resume the Kotlin path.

### Day 1-2: tree-sitter XS grammar

1. Add `tree-sitter` and `tree-sitter-c` (or fork the C grammar) as a starting point.
2. Modify the grammar for XS specifics:
   - **Note:** the first draft of this spike planned prefix-based heuristics (`k[A-Z]\w*`, `g[A-Z]\w*`, `s[A-Z]\w*`). The actual redesigned LSP uses no prefix heuristics for semantics; see `openspec/changes/xs-language-server/explore.md` §3.4 for the locked semantic rules (`const`, `extern`, `mutable`) and Doxygen-sourced engine API.
   - XS-specific keywords: `rule`, `void`, `bool`, `vector`
   - Strip C-specific stuff XS doesn't have: preprocessor, headers, struct/union, bit fields
3. Test by parsing `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs` and asserting it parses with no errors.
4. Test a deliberately broken `.xs` file and confirm tree-sitter reports the error location.

**De-risk check:** if tree-sitter's C grammar can't easily be adapted to XS, or the grammar is unbearably complex, the spike fails here.

### Day 3: tower-lsp server skeleton

1. `cargo new --bin xs-language-server` (or library + binary).
2. Add `tower-lsp = "0.20"` and `tokio = { version = "1", features = ["full"] }`.
3. Implement `LanguageServer` trait: `initialize`, `initialized`, `shutdown`, `exit`. ~100 LOC of boilerplate.
4. Server runs over stdio. Test by connecting manually with `nc` or a small Rust client.
5. Echo back the `textDocument/didOpen` notification to confirm round-trip works.

**De-risk check:** if tower-lsp's API is awkward or the LSP message handling has surprises, the spike fails here.

### Day 4-5: publish parse-error diagnostics

1. Wire tree-sitter into the LSP: on `textDocument/didOpen` and `textDocument/didChange`, re-parse the document.
2. Collect parse errors (tree-sitter gives byte ranges and error messages).
3. Convert to LSP `Diagnostic` objects and call `client.publish_diagnostics()`.
4. Test by sending a malformed `.xs` file via the LSP client and verifying the diagnostics appear.

**De-risk check:** this is the most important milestone. If this works, the pivot is justified.

### Day 6-7: engine API completion via LSP

1. Add `serde_json` dependency. Load `tools/intellij-xs-plugin/src/main/resources/syscalls.json` and `aiplans.json` at server start (or symlink them; can move to LSP's own `resources/` later).
2. Implement `textDocument/completion`: prefix-match the identifier at the cursor against the engine API.
3. Return `CompletionItem` list with `label`, `kind`, `detail` (signature), `documentation` (help text).
4. Test by typing `aiE` and receiving `aiEcho`, `aiEchoCategory`, `aiEchoWarning` in the response.

**Spike complete if Day 7 ends with:** a Rust binary that responds to LSP `initialize`/`didOpen`/`completion`, publishes parse-error diagnostics, and returns engine API completion items. Total expected: ~400-600 LOC of Rust.

---

## After the spike (if it succeeds)

| Week | Work                                                                                                          |
| ---- | ------------------------------------------------------------------------------------------------------------- |
| 2    | Wire up `textDocument/hover` (return `MarkupContent` with signature + help from `syscalls.json`).             |
| 2    | Wire up `textDocument/definition` for engine syscalls (return a `Location` pointing at a virtual URI like `xs-stub://aiEcho`). |
| 3    | Symbol table + name resolution: parse top-level declarations, build a flat symbol table, resolve workspace functions/rules/variables. Add `textDocument/definition` for workspace symbols. |
| 4    | Add `textDocument/references`. Add `textDocument/rename`. |
| 5    | Type checking: XS has a type system (int, float, bool, string, vector, class instances). Add "wrong arg count" and "wrong arg type" diagnostics. |
| 6    | Convert the IntelliJ plugin to an LSP client (~200 LOC). Remove `XsEngineApi`, `XsCompletionContributor`, `XsDocumentationProvider`, `XsParameterInfoHandler`, `XsEngineReferenceContributor`, `XsEngineStubGenerator`, `XsCallContextDetector`. Keep file type, TextMate, brace matcher. |
| 7    | Convert the existing VS Code extension (`extracted/xs.vsix`) to an LSP client too. Same 1.16 MB `syscalls.json` and 57 KB of `out/extension.js` get replaced by ~200 LOC of vscode-languageclient glue. |

---

## Project layout (as built)

```
tools/
├── intellij-xs-plugin/        # thin LSP client (~1,000 LOC Kotlin)
│   ├── build.gradle.kts
│   ├── src/main/kotlin/com/aomr/xs/
│   │   ├── XsLanguage.kt
│   │   ├── XsFileType.kt
│   │   ├── settings/XsSettings.kt            # PersistentStateComponent
│   │   ├── settings/XsConfigurable.kt        # Settings UI
│   │   ├── settings/XsModAutoDetector.kt     # Recursive game/ scan
│   │   ├── lsp/XsLspServerManager.kt         # LSP lifecycle
│   │   ├── startup/XsStartupActivity.kt      # First-run UX
│   │   ├── textmate/XsTextMateBundleProvider.kt
│   │   └── editor/XsBraceMatcher.kt
│   └── src/main/resources/
│       ├── syntaxes/xs.tmLanguage.json    # vendored from extracted/xs.vsix
│       └── META-INF/plugin.xml
└── xs-language-server/        # Rust crate — real language implementation
    ├── Cargo.toml
    ├── src/
    │   ├── main.rs           # binary entry, CLI/env game-path parsing
    │   ├── server.rs         # LanguageServer impl
    │   ├── parser.rs         # tree-sitter wrapper
    │   ├── diagnostics.rs    # parse-error + semantic diagnostics
    │   ├── engine_api.rs     # Doxygen-extracted engine API loader
    │   ├── cache.rs          # SHA-256 cache for engine + per-file parse results
    │   ├── doxygen.rs        # 7z extraction + HTML scraping
    │   ├── workspace.rs      # virtual project + mod overlay
    │   ├── symbols.rs        # symbol table extraction
    │   ├── semantic.rs       # extern/mutable/forward-decl diagnostics
    │   ├── typecheck.rs      # engine-call and cross-file type checks
    │   ├── completion.rs     # textDocument/completion handler
    │   └── references.rs / word.rs / rename.rs
    └── tree-sitter-xs/       # forked tree-sitter XS grammar
        ├── grammar.js
        └── src/
```

Engine data is no longer loaded from static `syscalls.json`/`aiplans.json`; it is extracted at runtime from `doxygen_retail.7z` and cached under `~/.local/state/aomr_lsp/v1/`.

---

## What to keep from the current Kotlin work

| Keep                                                                                  | Why                                                                |
| ------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `XsLanguage`, `XsFileType`, `XsFileTypeFactory`                                       | File-type registration is IntelliJ-specific, has to live in Kotlin |
| `XsTextMateBundleProvider` + `xs.tmLanguage.json`                                       | Syntax highlighting is client-side; LSP doesn't do colors         |
| Brace matcher, commenter, quote handler, surrounding pairs (no `SurroundingPairsProvider` extension — it's a source-of-truth class only) | IDE-side behavior, not language-level                              |
| `build.gradle.kts`, `plugin.xml`, CI workflow                                          | Packaging is correct                                               |
| `docs/doxygen_retail/`                                                                 | Upstream source for the engine API; the Rust LSP regen pipeline will read this in P5 |

## What gets replaced (don't preserve)

| Replace                                                                                                                                                                                                                                              | Replacement                                                                 |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `XsEngineApi`, `XsAiPlans` (~120 LOC)                                                                                                                                                                                                              | Rust-side `engine_api.rs` with `syscalls.json` + `aiplans.json`             |
| `XsCompletionContributor`, `XsCallContextDetector` (~280 LOC)                                                                                                                                                                                      | LSP `textDocument/completion` returning matching engine syscalls             |
| `XsDocumentationProvider` (~85 LOC)                                                                                                                                                                                                                | LSP `textDocument/hover` returning signature + help                          |
| `XsParameterInfoHandler` (~120 LOC)                                                                                                                                                                                                                | LSP `textDocument/signatureHelp` (standard LSP message)                     |
| `XsEngineReferenceContributor`, `XsEngineStubGenerator` (~120 LOC)                                                                                                                                                                                 | LSP `textDocument/definition` with a `xs-stub://` virtual URI                |
| `XsLexer.flex`, `XsParserDefinition`, `XsIdentifier`, `XsASTFactory`, `XsTokenType` (~150 LOC)                                                                                                                                                       | tree-sitter on the Rust side; IntelliJ sees LSP diagnostics, no PSI needed |
| All indexing tests in `XsEngineApiTest`, `XsAiPlansTest` (~75 LOC)                                                                                                                                                                                 | Rust-side `#[test]` for the loader                                          |

Net: IntelliJ plugin shrinks from ~1,716 LOC to ~300 LOC. The "real" language implementation lives in Rust.

---

## Decisions to confirm at the start of next session

These were assumed (and may need adjustment):

1. **Crate location**: `tools/xs-language-server/` (sibling to `tools/intellij-xs-plugin/`). Alternative: sub-crate inside `tools/intellij-xs-plugin/lsp/`. User leaned toward sibling; no strong opinion either way.
2. **tree-sitter grammar source**: fork tree-sitter-c, modify for XS. Alternative: write the XS grammar from scratch. Forking is faster and more proven.
3. **LSP transport**: stdio (tower-lsp default). No need for socket-based unless we want to attach a debugger.
4. **JSON library**: `serde_json` (standard).
5. **Async runtime**: `tokio` (required by `tower-lsp`).
6. **Engine data**: copy `syscalls.json` and `aiplans.json` from the IntelliJ plugin's resources into the Rust crate's resources. Long-term, the Rust crate becomes the source of truth and the IntelliJ plugin imports from it.
7. **Test runner for Rust**: `cargo test` standard, plus `tower-lsp`'s test client if available.

---

## Pre-existing context the next session will need

- The current `intellij-xs-plugin/` Kotlin work: see `openspec/changes/intellij-xs-plugin/` for proposal, design, tasks, specs, verify-report, and apply-progress.
- The 6 open PRs (NOT YET MERGED):
  - `intellij-xs-plugin/p0-scaffold`
  - `intellij-xs-plugin/p1.1-engine-data`
  - `intellij-xs-plugin/p1.2-engine-completion`
  - `intellij-xs-plugin/p1.4-hover-docs`
  - `intellij-xs-plugin/p1.5-parameter-info-and-stubs`
  - `intellij-xs-plugin/verify-fixes-p1`
- The original proposal/design said "stacked-to-main" PRs. **Decision: don't merge the P1.5+1.6 PR or anything past P0/P0.5** until the spike validates the pivot. P0 and P0.5 are safe to merge regardless of the pivot (they're file-type + TextMate + IDE behaviors, all useful in the LSP-client world).
- The XS language reference docs: `docs/xs-language-syntax.md`, `docs/MythRMConstants.txt`, `docs/MythTRConstants.txt`, `docs/doxygen_retail/`.
- The existing VS Code extension `extracted/xs.vsix` (we extracted it to `/tmp/opencode/xs_ext/` for inspection; vendored grammar at `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`).

---

## How to actually start the spike next session

```bash
# 1. Verify state
cd /home/houtamelo/Documents/projects/aom_retold_mod
git status
ls tools/xs-language-server/ 2>/dev/null && echo "already exists" || echo "needs creation"

# 2. Check that Rust toolchain is available
cargo --version
rustc --version

# 3. If not: install
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# 4. Create the crate
cd tools
cargo new --bin xs-language-server
cd xs-language-server

# 5. Add dependencies
cargo add tower-lsp --features="runtime-agnostic"
cargo add tokio --features="full"
cargo add tree-sitter
cargo add serde_json

# 6. Initialize the tree-sitter grammar (fork of tree-sitter-c, or write from scratch)
# See Day 1-2 of the spike plan above

# 7. Day 3+: follow the spike plan
```

If any of step 1-3 reveals a problem (no cargo, broken state, etc.), pause and ask the user.

---

## Outcome

The spike succeeded and the pivot was justified. The redesigned XS Language Server was implemented across five phases:

1. Doxygen 7z extraction + SHA-256 cache + `--game-path` CLI/env parsing.
2. Virtual project workspace with vanilla `game/` folder + per-mod overlay + `include` resolution.
3. Cross-file semantic diagnostics: `extern` collision detection, forward-declaration validation, `mutable` redefinition checks, and cross-file type checking.
4. IntelliJ plugin converted to a thin LSP client with settings UI, mod auto-detect, and LSP lifecycle management.
5. VS Code `.vsix` removed from scope; `extracted/xs.vsix` is no longer tracked.

The full design, specs, implementation record, and verification report live under `openspec/changes/xs-language-server/`. See `apply-progress.md` for the implementation record and `verify-report.md` for the final verification results.

## What this doc is NOT

- It's not a spec for the LSP server. That's a separate artifact (will live at `openspec/changes/xs-language-server/specs/` if/when we formalize via SDD).
- It's not a commitment. The spike can fail. If it does, we resume the Kotlin path.
- It's not a port plan for the VS Code extension. That's a separate task for week 7 of the post-spike plan.
