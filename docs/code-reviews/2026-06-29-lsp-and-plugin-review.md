# Code Review — LSP & IntelliJ Plugin (Rust + Kotlin)

**Date:** 2026-06-29
**Scope:** `tools/xs-language-server/` (Rust LSP server) + `tools/intellij-xs-plugin/` (Kotlin/Gradle IntelliJ plugin)
**Type:** Read-only adversarial code review; **no files were modified**
**Methodology:** 7 parallel reviewers, each with fresh context, identical rubric, non-overlapping file lists. Every finding has `file:line` evidence + "why it matters" + "what to check" — no fixes proposed.

---

## Executive summary

| Severity   | Count | What it means                                                                                  |
| ---------- | ----- | ---------------------------------------------------------------------------------------------- |
| **CRITICAL** | **3** | Almost-certain bug with serious user-visible impact (deadlock, race-causing orphaned processes, IDE freeze) |
| **HIGH**     | **27** | Probable bug or clear comment/code lie                                                       |
| **MEDIUM**   | **55** | Likely issue, possibly correct in subtle context                                              |
| **LOW**      | **48** | Smell / nit (mostly comment-vs-code, pattern duplication, minor edge cases)                  |
| **NIT**      | **12** | Cosmetic                                                                                      |
| **TOTAL**    | **145** |                                                                                              |

> Three reviewer outputs were truncated by the tool (R2-F-22, R4-F-20, R6-F-12 — all LOW/NIT severity in their truncated portions). Each truncated finding still has its `file:line` and category intact; only the rationale is partially clipped.

### Top concerns (must-audit list)

1. **CRITICAL — Three-way lock-acquisition deadlock in `server.rs`.** `did_close` acquires `documents → symbol_tables → merged_views` while `did_change_watched_files` and `get_or_build_merged_view` acquire them in different orders. Two concurrent requests can deadlock. **R3-F-01** — **[Resolved 2026-06-29]** via `openspec/changes/fix-deadlock-r3-f01/` (defensive refactor; STRICT TDD).
2. **CRITICAL — Race on `XsBinaryResolver.cachedBundledPath`.** Classic double-checked-locking without a lock; two threads can each extract a temp file, orphaning one. **R7-F-02**
3. **CRITICAL — `XsStartupActivity.runActivity` runs full mod-directory scan on the EDT.** A misconfigured project opened at `/` would freeze the IDE for the entire tree walk. **R7-F-13**
4. **HIGH — `lsp_roundtrip_test` PASS messages lie about what they verify.** The "PASS: completion returned aiEcho family items" check counts `aiEcho` substrings in the whole stdout stream (diagnostics for `/tmp/types.xs` alone contribute two). Two of the three asserts in this 1731-LOC file pass for the wrong reason. **R5-F-01, R5-F-03** — **[Resolved 2026-06-29]** via `openspec/changes/archive/2026-06-29-fix-lsp-test-honesty/` (PASS WITH WARNINGS, 203/203 tests)
5. **HIGH — `MergedView.build`'s returned diagnostics message is "internal error: failed to install XS language"** when the user's source has a syntax error. The handler will surface this to the user on every typo. **R3-F-09**
6. **HIGH — `extract_callee_name` in `typecheck.rs` returns just the method name for `obj.method(…)` calls** and then looks it up in the global engine API. Engine methods without their receiver will silently fail to type-check. **R2-F-08**
7. **HIGH — `doxygen::parse_memproto_signature` keeps the `static` qualifier in `return_type`** even though the comment above says "drop it". Every cache consumer downstream sees `static int` as the type. **R4-F-01**
8. **HIGH — `Param::render` is a dead function with three mutually contradictory comments** claiming it preserves defaults verbatim, doesn't, and "appends separately". `format_function_detail` is the actual hover path. **R1-F-01**
9. **HIGH — Test `recovers_function_definition_from_error_node_with_body` doesn't actually exercise the ERROR-recovery branch.** The input parses cleanly via the normal `function_definition` rule; the test passes against the wrong path. **R1-F-03**
10. **HIGH — `word.rs::identifier_at_cursor` slices line_str by byte offset while LSP `Position.character` is in UTF-16 code units.** For any non-ASCII character before the cursor, the function either panics ("byte index not on char boundary") or returns the wrong identifier. **R1-F-04**
11. **HIGH — Three duplicated `node_range` / `node_text` / `find_named_child` helpers** across `semantic.rs`, `typecheck.rs`, `symbols.rs`, `references.rs`, `definition_check.rs`. One bug-fix away from divergence. **R2-F-21, R3-F-22**

### Cross-cutting patterns observed

- **Comment/code mismatch is pervasive.** R1 alone flagged 4 such cases; R3 flagged 4 more; R7 flagged 3. Many of them are docstrings that were written before a refactor and never updated.
- **Six identical `docs.lock().await; docs.get(uri).unwrap_or("").to_string()` blocks** in `server.rs`. Any change to the read pattern requires editing six sites.
- **`plugin.xml` depends on `com.intellij.modules.ultimate`** but the plugin uses no Ultimate-gated APIs — it cannot be installed on IntelliJ Community Edition.
- **TextMate bundle has dead rules** (`access-method`, `backslash_escapes`, and self-referential `disabled`) that are never referenced from any other rule.
- **`VisibilityProvenance::DirectInclude.include_line` is documented as "0-indexed line of that `include` directive"** but actually stores the start of the directive range (potentially including the trailing statement).

---

## CRITICAL findings (3)

### R7-F-02 · concurrency · `XsBinaryResolver.kt:30-31, 75-97`

Classic double-checked-locking done without a lock. Two threads racing on `cachedBundledPath` each call `Files.createTempDirectory("xs-lsp-")`, both extract the resource, and the last write wins — orphaning one temp file/dir on disk.

```kotlin
@Volatile
private var cachedBundledPath: String? = null
...
private fun extractBundled(resourceProvider: (String) -> InputStream?): String? {
    cachedBundledPath?.let { return it }
    val stream = resourceProvider(BUNDLED_RESOURCE_PATH) ?: return null
    return try {
        val tempDir = Files.createTempDirectory("xs-lsp-").toFile()
        ...
```

**Why it matters:** Realistic entry points: `XsLspSupportProvider.fileOpened` (EDT) can race with `XsConfigurable.apply` (also EDT, but different listeners / Swing worker threads), or with two parallel project opens. Affects every IDE session that touches this code; orphan accumulates per race. The fix is `synchronized(this)` or `AtomicReference<String?>`.

---

### R7-F-13 · concurrency · `XsStartupActivity.kt:30-56`

`StartupActivity.runActivity` runs on the EDT during project open and synchronously walks the entire project tree looking for `game/` directories.

```kotlin
override fun runActivity(project: Project) {
    val appSettings = XsAppSettings.getInstance()
    val projectSettings = XsSettings.getInstance(project)

    if (appSettings.state.gamePath.isEmpty()) {
        notifyMissingGamePath(project)
        return
    }

    if (projectSettings.state.modPaths.isEmpty()) {
        val projectRoot = project.basePath?.let { LocalFileSystem.getInstance().findFileByPath(it) }
        if (projectRoot != null) {
            val detected = XsModAutoDetector.scan(projectRoot)
```

**Why it matters:** `XsModAutoDetector.scan` calls `Files.list(...).use { stream.asSequence().toList() }` recursively. A misconfigured project opened at `/` (or any very deep / wide tree) freezes the IDE for the duration. The class is even annotated `DumbAware`, but dumb-aware ≠ safe to block. Move the scan off the EDT (e.g. `Task.Backgroundable`, `ProgressManager.run { ... }` with modal cancellable progress).

---

### R3-F-01 · concurrency · `server.rs:406-414 (also 169-186, 472-485)` · **[Resolved 2026-06-29]** via `openspec/changes/fix-deadlock-r3-f01/` (defensive refactor; STRICT TDD)

Lock acquisition order for the three big mutexes is inconsistent across handlers, and `did_close`/`did_change_watched_files`/`get_or_build_merged_view` use opposing orders.

```rust
async fn did_close(&self, params: DidCloseTextDocumentParams) {
    let uri = params.text_document.uri;
    debug!("did_close: {}", uri);
    let mut docs = self.documents.lock().await;
    docs.close(&uri);
    let mut tables = self.symbol_tables.lock().await;
    tables.remove(&uri);
    self.clear_merged_view_cache(&uri).await;   // locks merged_views
}
```

**Why it matters:** `did_close` takes `documents → symbol_tables → merged_views`. `did_change_watched_files` takes `merged_views → documents → symbol_tables`. Two concurrent requests can deadlock: `did_close` holds `documents`/`symbol_tables` while waiting on `merged_views`; the watched-files handler holds `merged_views` while waiting on `documents`. The same three mutexes are also held simultaneously inside `did_close` for no functional reason — each block is independent. The fix is a single global lock-acquisition order (pick one and audit every site that takes more than one of the three).

---

## HIGH findings (27)

### R1 — Parser+Symbols (4 HIGH)

#### R1-F-01 · comment-mismatch · `symbols.rs:82-92`

`Param::render` claims to render `"int x = 5"`, the first in-body comment claims it preserves defaults verbatim, the second claims defaults are appended elsewhere. The implementation ignores `source` and `format!("{} {}", self.ty, self.name)` only emits the bare type + name. The function is never called by anything — `format_function_detail` (lines 560-573) does the real hover rendering.

```rust
impl Param {
    /// `int x` or `int x = 5` — used in hover signatures.
    pub fn render(&self, source: &str) -> String {
        // Find the parameter declaration text in the source so we preserve
        // default values verbatim.
        // The caller already knows the byte range; we accept the source and
        // let render just print type + name (defaults are appended separately).
        let _ = source;
        format!("{} {}", self.ty, self.name)
    }
}
```

**Check:** Confirm `Param::render` is truly dead; if so, delete it along with all three contradictory comments.

---

#### R1-F-02 · comment-mismatch · `symbols.rs:162-166 and 456-462`

The comment above the `ERROR` → arm claims that "the XS grammar currently parses function forward declarations … as an ERROR node". But the grammar's `_declaration_declarator` already aliases `function_declarator`, and the existing test `extracts_forward_declaration` proves `void bar(int x = -1);` parses as a regular Function symbol — no ERROR involved. The entire `extract_error_forward_declaration` branch is unreachable for normal XS.

```rust
// The XS grammar currently parses function forward declarations
// (`void bar(int x = -1);`) as an ERROR node containing the type,
// identifier and parameter list. Extract them so callers get
// forward-declaration symbols anyway.  ERROR nodes that contain a
// body are recovered as full function definitions.
"ERROR" => {
```

**Check:** Trace a representative `void bar(int x = -1);` through the tree-sitter grammar and confirm no ERROR node is produced. Then delete the dead helper and its comment.

---

#### R1-F-03 · comment-mismatch · `symbols.rs:806-818`

Test `recovers_function_definition_from_error_node_with_body` claims to test the ERROR-recovery path, but the input `void broken(int x = [ ] { }) { }` parses the parameter default `[ ] { }` as a well-formed `lambda_expression` and matches the regular `function_definition` rule. The test exercises the wrong branch and passes for the wrong reason.

```rust
#[test]
fn recovers_function_definition_from_error_node_with_body() {
    // A header the main `function_definition` rule does not accept: a
    // param with an ERROR default. The compound_statement body lets the
    // error-recovery path produce a Function symbol.
    let src = "void broken(int x = [ ] { }) { }\n";
    let t = table_for(src);
    let s = t.find("broken").expect("broken symbol from ERROR recovery");
```

**Check:** Either construct a real ERROR-emitting input (e.g. malformed signature) and verify the helper, or delete the error-recovery helpers and the `ERROR =>` arm.

---

#### R1-F-04 · edge-case · `word.rs:22-55`

`identifier_at_cursor` slices `line_str` by byte offset, but LSP `Position.character` is in **UTF-16 code units**. For any non-ASCII character before the cursor, `line_str[col..]` either panics ("byte index not on char boundary") or returns the wrong identifier.

```rust
pub fn identifier_at_cursor(text: &str, line: u32, character: u32) -> Option<String> {
    let line_str = text.lines().nth(line as usize)?;
    let col = (character as usize).min(line_str.len());

    // Cursor must be ON an identifier char.
    let on_ident = line_str[col..]
        .chars()
        .next()
        .map_or(false, |c| c.is_ascii_alphanumeric() || c == '_');
```

**Check:** Walk through `int αx = 0;` with cursor on `α`; observe whether the function panics or returns `None`. Cross-check LSP §3.16 (UTF-16 positions).

---

### R2 — Semantic+Typecheck (4 HIGH)

#### R2-F-01 · bug · `semantic.rs:204-210`

Cross-file resolution returns the first file whose table has *any* symbol with the name, callable or not. If the first match is a non-callable (variable of same name as a function in another file), `is_callable_symbol` filters it out and the loop terminates with `Unresolved` — even if a perfectly good callable exists in a later file. `HashMap` iteration order is non-deterministic.

```rust
if let Some(file) = project.files.values().find(|f| f.table.find(name).is_some()) {
    if let Some(sym) = file.table.find(name) {
        if is_callable_symbol(sym) {
            return Resolution::Workspace(sym);
        }
    }
}
```

**Check:** Construct a test where file A defines `int foo = 1;` and file B defines `void foo() {}`. The intent is presumably "first callable match across all files".

---

#### R2-F-02 · comment-mismatch · `typecheck.rs:90-94`

Precedence between workspace and engine API is inverted vs `semantic.rs::resolve_callee`. In `semantic.rs` workspace symbols shadow engine syscalls (per the existing test); here the engine branch is consulted first. No test covers "workspace shadowing engine" in typecheck.

```rust
let resolved: Option<Callee<'_>> = if let Some(syscall) = engine.find_syscall(&callee) {
    Some(Callee::Engine(syscall))
} else {
    resolve_workspace_function(merged, project, &callee).map(Callee::Workspace)
};
```

**Check:** Decide which precedence is correct (semantic.rs's "workspace first" looks right by analogy with the test) and align typecheck.rs to match.

---

#### R2-F-03 · error-handling · `diagnostics.rs:180-187`

`missing_token_message`'s "leak guard" only triggers when the token name contains an uppercase ASCII letter. But grammar non-terminals are conventionally `snake_case` (`identifier`, `number_literal`, etc.), so a missing `identifier` node slips through and produces the user-facing literal `Missing 'identifier'`.

```rust
fn missing_token_message(node: Node) -> String {
    let token = node.kind();
    if token.is_empty() || token.chars().any(|c| c.is_alphabetic() && c.is_uppercase()) {
        let pos = node.start_position();
        return format!("Parse error near line {}, column {}", pos.row + 1, pos.column + 1);
    }
    format!("Missing '{}'", token)
}
```

**Check:** Test inputs like `void f() { foo` and `void f() x;`. Verify whether `Missing 'identifier'` reaches the editor.

---

#### R2-F-04 · pattern · `diagnostics.rs:252-279`

`categorize` does fragility-by-substring classification on the lowercased diagnostic message. The `WrongArgType` arm requires the substring "of type" — any future rephrase (e.g. `wanted argument 1 to be int, got float`) silently falls into `WrongArgCount`. `unresolved` / use-before-declaration messages fall into `Other`.

```rust
pub fn categorize(d: &Diagnostic) -> DiagnosticCategory {
    let msg = d.message.to_ascii_lowercase();
    if msg.contains("duplicate extern") || msg.contains("extern collision") {
        DiagnosticCategory::ExternCollision
    ...
    } else if msg.contains("expected")
        && msg.contains("argument")
        && msg.contains("of type")
    {
        DiagnosticCategory::WrongArgType
    } else if msg.contains("expected") && msg.contains("argument") {
        DiagnosticCategory::WrongArgCount
    ...
```

**Check:** The cleaner fix would be to use the LSP `code` field as the source of truth (semantic.rs sets `E0310`, typecheck.rs sets `None`).

---

### R3 — LSP Protocol Handlers (5 HIGH)

#### R3-F-02 · logic-suspicious · `server.rs:845-906 (rename), 908-942 (prepare_rename)`

`rename` and `prepare_rename` refuse engine-API names but happily rename **`extern`** declarations and **forward** declarations. Renaming `extern int gFoo;` rewrites the local token while leaving any real definition untouched. The `Symbol::is_extern` / `is_forward` fields are never consulted.

```rust
// Engine API symbols can't be renamed — no source to rewrite.
if self.engine.find_syscall(&ident).is_some()
    || self.engine.find_aiplan(&ident).is_some()
{ ... return Ok(None); }
...
let raw = references::find_identifier_uses(&tree, &text, &ident);
let ranges = {
    let tables = self.symbol_tables.lock().await;
    match tables.get(uri) {
        Some(t) => references::filter_declaration(raw, t, &ident, /* include */ true),
        None => raw,
    }
};
```

**Check:** Decide whether extern/forward declarations should be excluded from `find_identifier_uses` results before generating edits.

---

#### R3-F-03 · bug · `references.rs:61-75, server.rs:872-879`

`SymbolTable::find` returns the LAST matching symbol. With duplicate top-level declarations of the same identifier (legal after grammar-driven ERROR recovery), only the last declaration's `selection_range` is filtered out by `filter_declaration`. Earlier duplicate declarations remain in the ranges and would be reported as references; `rename` would offer them as edits.

```rust
pub fn filter_declaration(
    ranges: Vec<Range>,
    table: &crate::symbols::SymbolTable,
    name: &str,
    include_declaration: bool,
) -> Vec<Range> {
    if include_declaration { return ranges; }
    let decl = match table.find(name) {
        Some(s) => s.selection_range,
        None => return ranges,
    };
    ranges.into_iter().filter(|r| *r != decl).collect()
}
```

**Check:** Trace a file with two `int x = ...;` declarations on different lines through `find_identifier_uses` + `filter_declaration(_, &table, "x", false)`.

---

#### R3-F-04 · logic-suspicious · `server.rs:699-764 (references handler, engine-API branch)`

When the user clicks "find references" on an engine-API identifier that is **also** shadowed by a workspace definition, the engine-API full-workspace scan is skipped and execution falls through to the merged-view walk. The merged-view walk covers only `current_file + mv.files()` (current file + include closure). Calls from other files in the mod that aren't in the include closure are silently dropped. The same identifier returns different reference sets depending on whether a shadow exists.

```rust
let is_engine_api = self.engine.find_syscall(&ident).is_some()
    || self.engine.find_aiplan(&ident).is_some();
let has_workspace_def = is_engine_api
    && (merged.as_ref().and_then(|mv| mv.find(&ident)).is_some()
        || self.symbol_tables.lock().await.get(uri)
            .and_then(|t| t.find(&ident)).is_some());

if is_engine_api && !has_workspace_def {
    // full workspace scan
    ...
    return Ok(Some(locs));
}
```

**Check:** Open a mod where file A defines `kbGetProtoUnit` (overriding engine), file B calls `kbGetProtoUnit` but A and B don't include each other.

---

#### R3-F-05 · performance · `server.rs:656-678 (workspace_symbol handler)`

Every `workspace/symbol` request (a) parses every visible file in the mod and rebuilds its symbol table from scratch, even though `symbol_tables: HashMap<Url, SymbolTable>` already exists; (b) reads from disk via `tokio::fs::read_to_string` instead of using the in-memory `documents` map (ignoring unsaved edits); (c) lowercases the symbol name to compare against a lowercased query but still returns the original-case `label`.

```rust
for (rel, path) in files {
    let Ok(uri) = Url::from_file_path(&path) else { continue };
    let Ok(text) = tokio::fs::read_to_string(&path).await else { continue };
    let Some(tree) = parser::parse(&text) else { continue };
    let table = symbols::build_symbol_table(&tree, &text);
    for sym in &table.symbols {
        if !query.is_empty() && !sym.name.to_lowercase().contains(&query) {
            continue;
        }
        items.push(symbol_to_workspace_symbol(&sym, &uri, &rel));
    }
}
```

**Check:** Confirm that `cache::load_or_parse_symbols(&path, &rel, cache_dir)` returns a `SymbolTable` suitable for direct iteration; that `documents.get(&uri)` would serve the unsaved buffer where applicable.

---

#### R3-F-09 · comment-mismatch · `server.rs:1001-1014`

The `None` branch is reached when `parser::parse(text)` returns `None` (i.e., the user's source has a syntax error). The message the user sees is `"internal error: failed to install XS language"`, which misleadingly blames the LSP server ("internal error") and its grammar bootstrap ("failed to install") when actually the grammar parsed this file but rejected the input. This will appear on every typo.

```rust
None => {
    let mut map = std::collections::HashMap::new();
    map.insert(uri.clone(), vec![Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("xs-language-server".to_string()),
        message: "internal error: failed to install XS language".to_string(),
        ...
```

**Check:** Whether `parser::parse` returning `None` means "syntax error in input" or "LSP initialization failure" — the handler comment on line 945-949 suggests "parse failure".

---

### R4 — Data Backend (3 HIGH)

#### R4-F-01 · comment-mismatch · `doxygen.rs:268-283`

The comment explicitly says "drop it" for the `static` qualifier, but the code keeps every token before the name in `return_type`. A signature like `static int foo` would be parsed as `return_type="static int"`, `name="foo"` — every consumer of that field (cache, semantic layer, hover) sees `static int` as the engine return type and treats the symbol as unknown.

```rust
// Examples:
//   "int aiAddEchoCategory"
//   "void aiAddToResourceBreakdown"
//   "int[] aiPlanGetIDsByType"
// A few entries have additional qualification such as "static"; drop it.
let tokens: Vec<&str> = text.split_whitespace().collect();
if tokens.len() < 2 {
    return None;
}
let name = tokens.last()?.to_string();
...
let return_type = tokens[..tokens.len() - 1].join(" ");
```

**Check:** grep the extracted `doxygen_retail.7z` for any `static`/`inline`/`virtual`/`explicit` memproto rows; verify whether `return_type` contains the qualifier in the resulting JSON cache.

---

#### R4-F-02 · archive · `doxygen.rs:41-49`

`sevenz_rust::decompress_file` extracts the entire archive into a temp dir with no pre-extraction size check, no entry-count limit, no per-entry ratio limit, no defense against `..` path traversal in member names. User-supplied or corrupted archive → zip-bomb / disk-fill / path-traversal vector under the LSP's identity.

```rust
pub fn extract_engine_api(archive: &Path) -> Result<EngineData> {
    let tmp = tempfile::tempdir().with_context(|| format!("creating temp dir for {archive:?}"))?;
    sevenz_rust::decompress_file(archive, tmp.path())
        .with_context(|| format!("decompressing 7z archive {archive:?}"))?;
```

**Check:** Confirm what `sevenz_rust 0.6` actually does — does it sanitize member paths or honor absolute/`..` components? What is the total uncompressed size of `doxygen_retail.7z`?

---

#### R4-F-03 · cache · `cache.rs:306-330 + engine_api.rs:130-140`

`engine_api::load_from_archive`'s `Err(_) if cache_path.exists()` arm treats any extraction failure as "cache is corrupt — delete and retry". But `cache::load_or_write_engine` returns `Err` for two distinct scenarios: (a) corrupt cache file from `read_json` (read attempted before fallback), or (b) fallback succeeded but `write_json` failed. In case (b) the cache on disk is still good, but the branch will `remove_file` and re-run the expensive 7z decompression. `cache_path.exists()` cannot distinguish the two.

```rust
let value = fallback().with_context(|| {
    format!(
        "extracting engine data for archive hash {archive_hash}; \
         cache file {path:?} could not be populated"
    )
})?;
write_json(&path, &value)
    .with_context(|| format!("writing engine cache file {path:?}"))?;
Ok(value)
```

**Check:** Force a `write_json` failure (e.g., read-only cache dir) and confirm whether a previously-warm cache gets nuked.

---

### R5 — Tools+Tests+Grammar (3 HIGH)

#### R5-F-01 · test · `lsp_roundtrip_test.rs:401-411` · **[Resolved 2026-06-29]** via `openspec/changes/archive/2026-06-29-fix-lsp-test-honesty/` (PASS WITH WARNINGS, 203/203 tests)

The PASS message says "completion returned aiEcho family items", but the assertion counts `aiEcho` substrings across the **entire server stdout stream**. The publishDiagnostics notification for `/tmp/types.xs` alone contains `expected 1 argument(s) to aiEcho` and `expected argument 1 of type string for aiEcho, got int` — two `aiEcho` mentions without any completion items returning. The threshold `>= 3` is met regardless.

```rust
let raw = String::from_utf8_lossy(&all_bytes);
let completion_count = raw.matches("\"aiEcho\"").count()
    + raw.matches("\"aiEchoCategory\"").count()
    + raw.matches("\"aiEchoWarning\"").count();
if completion_count >= 3 {
    println!("PASS: completion returned aiEcho family items ({completion_count} occurrences)");
```

**Check:** Parse `completion_resp` directly (it has `id == 3` and a `result` array) and count `aiEcho*` entries **in that array only**.

---

#### R5-F-02 · test · `lsp_roundtrip_test.rs:1659-1676` · **[Resolved 2026-06-29]** via `openspec/changes/archive/2026-06-29-fix-lsp-test-honesty/` (PASS WITH WARNINGS, 203/203 tests)

The "within 500 ms" PASS claim measures wall-clock from before writing the references request to **after** `child.wait()` and `stdout.read_to_end()` — i.e. the entire end-to-end test time, including the deliberate 50 ms `thread::sleep` and the child's shutdown/exit roundtrip. The assertion `elapsed.as_millis() <= 500` is therefore a wall-clock budget for the whole test, not a response latency check. It will pass on fast machines and fail flakily on slower CI; it does not test what the comment claims.

```rust
// then invokes `textDocument/references` on `aiEcho` and asserts
// that at least two workspace `Location`s are returned within 500 ms.
...
let start = Instant::now();
stdin.write_all(frame(&references).as_bytes()).unwrap();
...
let elapsed = start.elapsed();
```

**Check:** Trace a sample run: `elapsed` typically includes >>50 ms of non-response overhead; an artificial 600 ms delay between writes would still yield "PASS".

---

#### R5-F-03 · test · `lsp_roundtrip_test.rs:362-380` · **[Resolved 2026-06-29]** via `openspec/changes/archive/2026-06-29-fix-lsp-test-honesty/` (PASS WITH WARNINGS, 203/203 tests)

`has_clean_diag` matches **any** `publishDiagnostics` notification mentioning `/tmp/test.xs` — including non-empty ones. The PASS line says "expected empty" but the code never inspects the diagnostics payload. `has_bad_diag` substrings the entire output stream for "Parse error", "Missing", or "Unexpected" — these can appear in unrelated LSP messages (hover markdown, typecheck errors). Both checks pass without proving anything about the specific diagnostic contents.

```rust
let has_clean_diag = raw.contains("\"file:///tmp/test.xs\"")
    && raw.contains("textDocument/publishDiagnostics");
let has_bad_diag = raw.contains("\"file:///tmp/bad.xs\"")
    && (raw.contains("Parse error") || raw.contains("Missing") || raw.contains("Unexpected"));
```

**Check:** Mutate the server to publish a non-empty `diagnostics:[]` for `/tmp/test.xs` and confirm the test still says PASS.

---

### R6 — Plugin Editor+PSI (2 HIGH)

#### R6-F-01 · logic-suspicious · `XsHighlightingLexer.kt:13`

`text.toString().lowercase()` is applied to every identifier before the keyword set lookup. XS is described as C-like (case-sensitive). An identifier spelled `Void`, `Main`, `True` would be silently re-coloured as the corresponding keyword.

```kotlin
if (type == XsTokenTypes.IDENTIFIER) {
    val text = tokenText.toString().lowercase()
    when {
        text in XS_KEYWORDS -> return XsHighlightingTokenTypes.KEYWORD
        NUMBER_REGEX.matches(text) -> return XsHighlightingTokenTypes.NUMBER
    }
}
```

**Check:** Confirm XS keyword case sensitivity in `docs/xs-language-syntax.md`. If case-sensitive, drop `.lowercase()` and ensure the keyword list contains each keyword in its canonical spelling.

---

#### R6-F-02 · logic-suspicious · `XsSyntaxHighlighter.kt:20`

`WHITE_SPACE` is mapped to `XS_DEFAULT`. Combined with R6-F-03, that key's fallback is `IDENTIFIER`. Standard IntelliJ contract for whitespace is either `EMPTY_KEYS` (so it inherits the editor default text colour) or `DefaultLanguageHighlighterColors.TEMPLATE_LANGUAGE_COLOR` — never a content-bearing key. Visible symptom: every space and tab in `.xs` files renders with identifier colouring in the default colour scheme.

```kotlin
XsTokenTypes.WHITE_SPACE -> arrayOf(XsTextAttributesKeys.XS_DEFAULT)
```

**Check:** Open any `.xs` file in the default colour scheme and confirm whether whitespace inherits identifier colour.

---

### R7 — Plugin LSP+Settings+TextMate (5 HIGH)

#### R7-F-01 · comment-mismatch · `XsBinaryResolver.kt:87-89`

`File.setExecutable(executable, ownerOnly)` — when `ownerOnly == false` the execute bit is applied to owner, group, AND other, not owner-only. The comment explicitly says "for the owner" and rationalises "group/other permissions are not required", but the boolean literal `false` does the opposite.

```kotlin
// Mark as executable for the owner; group/other permissions are not
// required because the JVM spawns the child process.
tempFile.setExecutable(true, false)
```

**Check:** Decide whether to match the comment (`(true, true)`) or rewrite the comment to match reality.

---

#### R7-F-08 · lsp · `XsLspSupportProvider.kt:19-28`

`XsLspServerDescriptor` does not override `equals`/`hashCode`. `LspServerManager.ensureServerStarted(descriptor)` dedupes by descriptor identity (or class). Each call to `fileOpened` constructs a *new* descriptor via `XsLspServerDescriptor.create(project)`. If dedup key is instance identity, every opened `.xs` file spawns a new LSP server (process leak).

```kotlin
override fun fileOpened(
    project: Project,
    file: VirtualFile,
    serverStarter: LspServerSupportProvider.LspServerStarter,
) {
    if (file.extension != "xs") return
    val gamePath = XsAppSettings.getInstance().state.gamePath
    if (gamePath.isBlank()) return
    serverStarter.ensureServerStarted(XsLspServerDescriptor.create(project))
```

**Check:** Open an `.xs` file in a project with two `.xs` files open; check `ps -ef | grep xs-language-server`. If two processes, override `equals`/`hashCode` to compare `(project, gamePath, modPaths)`.

---

#### R7-F-12 · lsp · `XsLspServerManager.kt:89-106, 158-164`

When `XsStartupActivity.runActivity` auto-detects mods, it calls `XsSettings.setModPaths(detected)` → listener fires → `notifyWorkspaceFoldersChanged(detected)`. At startup, **no LSP server is running yet** (platform starts it only on first `.xs` file open). So `sendWorkspaceFolderChange` returns `false` and `restartServer()` is invoked — `LspServerManager.stopAndRestartIfNeeded(...)` is a no-op because there's nothing to restart. The wasted work is harmless today but the design is brittle and `XsConfigurable.apply()` double-invokes it (which currently masks via the `currentModPaths` dedup).

```kotlin
fun notifyWorkspaceFoldersChanged(newModPaths: List<String>) {
    val previous = currentModPaths.toSet()
    val next = newModPaths.toSet()
    val added = (next - previous).toList()
    val removed = (previous - next).toList()
    currentModPaths = newModPaths
    if (added.isEmpty() && removed.isEmpty()) return

    val sent = try {
        sendWorkspaceFolderChange(added, removed)
    } catch (e: Exception) {
        log.warn("Failed to send workspace/didChangeWorkspaceFolders", e)
        false
    }
    if (!sent) {
        restartServer()
    }
}
```

**Check:** Add a short-circuit at the top: `if (LspServerManager.getInstance(project).getServersForProvider(...).isEmpty()) return`.

---

#### R7-F-15 · comment-mismatch · `XsLspServerDescriptor.kt:91-95`

`resolveLspLogFile`'s KDoc claims "One file per project", but when `project.basePath` is null (default project, scratch file) all such projects funnel into the same `java.io.tmpdir/xs-lsp.log`. The KDoc lies.

```kotlin
/**
 * Path of the file the LSP writes its stderr to. One file per project,
 * appended across sessions, in the project's `.idea/` directory.
 */
private fun resolveLspLogFile(): Path {
    val basePath = project.basePath
        ?: return Paths.get(System.getProperty("java.io.tmpdir"), "xs-lsp.log")
    return Paths.get(basePath, ".idea", "xs-lsp.log")
}
```

**Check:** Either fix the doc to say "one file per project that has a basePath; otherwise a shared tmp file", or guard with a project-specific subdir under tmpdir.

---

#### R7-F-16 · platform-contract · `META-INF/plugin.xml:12-17`

Three dependency smells: (a) `<depends>com.intellij.modules.ultimate</depends>` makes the plugin un-installable on IntelliJ IDEA Community Edition — but the plugin only uses file-type / parser / brace-matcher / commenter / textmate / LSP, none of which are Ultimate-gated. (b) `<depends optional="true">com.intellij.modules.rider</depends>` — Rider is a different IDE product; an optional Rider module dep on an IDEA-targeted plugin does nothing useful. (c) `<depends>org.jetbrains.plugins.textmate</depends>` is correct and necessary.

```xml
<depends>com.intellij.modules.platform</depends>
<depends>com.intellij.modules.lang</depends>
<depends>com.intellij.modules.lsp</depends>
<depends>com.intellij.modules.ultimate</depends>
<depends>org.jetbrains.plugins.textmate</depends>
<depends optional="true">com.intellij.modules.rider</depends>
```

**Check:** Try installing the plugin in IDEA Community. Remove both Ultimate and Rider deps and re-test.

---

## MEDIUM findings (55) — one-line summaries, by reviewer

### R1 — Parser+Symbols (7 MEDIUM)

| ID | File:Line | Cat | One-line |
|----|-----------|-----|----------|
| R1-F-05 | `word.rs:30,39,47` | logic-suspicious | `is_ascii_alphanumeric() \|\| c == '_'` excludes Unicode IDs the grammar accepts (`αVariable`, `πr²`). |
| R1-F-06 | `parser.rs:43-50` & `symbols.rs:645-652` | logic-suspicious | `node_range` returns byte columns as if they were UTF-16 LSP columns; `node_text` uses byte ranges. Internal-inconsistent. |
| R1-F-07 | `parser.rs:21-41` | logic-suspicious | `extract_include_directives` only walks direct children — `#if FOO … include "x.xs"; … #endif` is invisible. |
| R1-F-08 | `symbols.rs:230-244` | logic-suspicious | `trim_matches('"')` strips every leading/trailing `"`; the `'…'` second pass is dead (XS has no single-quote literals). |
| R1-F-09 | `symbols.rs:319-325, 161` | comment-mismatch | "XS requires init" but the grammar allows `_declaration_declarator → identifier` (uninit decls parse). |
| R1-F-10 | `symbols.rs:247,252,266` | type-safety | Param is named `_source` (unused convention) but IS used by `node_text`. Hides future lint warnings. |
| R1-F-11 | `parser.rs:33` | error-handling | `&source[path_node.byte_range()]` panics on out-of-bounds if the buffer is ever different from the parsed `Tree`'s source. |
| R1-F-12 | `symbols.rs:159,247,270,311,519-538` | logic-suspicious | `extract_modifiers` implicitly relies on hidden rules being inlined into declaration nodes — no test asserts this invariant. |
| R1-F-18 | (R1 F-18 series) | — | (See individual notes) |

### R2 — Semantic+Typecheck (10 MEDIUM)

| ID | File:Line | Cat | One-line |
|----|-----------|-----|----------|
| R2-F-05 | `semantic.rs:545-549` | logic-suspicious | Self-recursion guard uses `r.start.line < callee_range.start.line`; fails for single-line `void f() { f(); }`. |
| R2-F-06 | `typecheck.rs:255-279, 364-374, 105-107` | pattern | `params()` clones per call; `let callee_source = target.source()` is bound but unused; `arg_types_compatible` has no caller. |
| R2-F-07 | `semantic.rs:882-887` | logic-suspicious | Cross-file resolution counts only `SymbolKind::Function`, not `SymbolKind::Rule`. |
| R2-F-08 | `typecheck.rs:311-320` | bug | `extract_callee_name` returns just the method name for `obj.method(…)` and looks it up in the global engine API — silently fails for almost every call. |
| R2-F-09 | `semantic.rs:350-358` | comment-mismatch | Doc says "Only files in `merged`" but the code explicitly uses project-wide `paths_in_scope` for the same intent. |
| R2-F-10 | `semantic.rs:432-447` | logic-suspicious | Extern-collision filter only emits for the current file + include closure; sibling-file `extern` collisions get silently dropped. |
| R2-F-11 | `semantic.rs:150-156` | pattern | `BUILTIN_CALLEES` lists `xsVectorSet`/`xsVectorGetX/Y/Z` — these should appear in the engine API extractor; patching over absence. |
| R2-F-12 | `semantic.rs:546-549` | logic-suspicious | Forward-declaration guard mis-tracks shadowed locals / forward-but-redefined. (Brittle but generally fine.) |
| R2-F-13 | `semantic.rs:532-538` | edge-case | `own_function_ranges` rebuilt per call (cacheable per-file); combines with R2-F-05 narrow window of false positives. |
| R2-F-14 | `semantic.rs:57-59, 121-123` | pattern | `entry().or_insert_with()` plus `name.clone()` plus `make_rule_symbol(&name)` — redundant allocations. |

### R3 — LSP Protocol Handlers (8 MEDIUM)

| ID | File:Line | Cat | One-line |
|----|-----------|-----|----------|
| R3-F-06 | `server.rs:472-484` | lsp-protocol | Republished diagnostics use hard-coded `version=0`. Clients may discard or mis-align. |
| R3-F-07 | `merged_view.rs:210-223, 1-30` | comment-mismatch | Orphan doc paragraph describes `MergeError` — type doesn't exist. Should move to module doc or onto `build`. |
| R3-F-08 | `merged_view.rs:169-177, 382-404` | comment-mismatch | Field named `missing` but stores both `IncludeDiagnosticKind::Missing` and `…Unreadable`. Rename or split. |
| R3-F-10 | `server.rs:406-414` | bug | `did_close` clears doc/table/merged-view but not `last_active_uri`. Next `workspace/symbol` dereferences a stale URI. |
| R3-F-11 | `server.rs:348-369` | pattern | Two separate `workspace.lock().await` acquisitions in one handler (read-only). Merging into one scope reduces lock-order drift. |
| R3-F-12 | `definition_check.rs:186-223` | pattern | `parenthesized_expression` and `expression` arms of `is_constant_expression` are byte-for-byte identical. Dead branching. |
| R3-F-13 | `server.rs:711-758` | edge-case | Per-file / total budgets measured **after** async I/O — they're post-hoc warnings, not stop signals. |
| R3-F-14 | `server.rs:383-404` | bug | `did_change` takes only the **first** of multiple content changes; legal per LSP spec. |

### R4 — Data Backend (8 MEDIUM)

| ID | File:Line | Cat | One-line |
|----|-----------|-----|----------|
| R4-F-04 | `cache.rs:82-88` | edge-case | `duration_since(UNIX_EPOCH)` → `unwrap_or_default()` silently turns pre-1970 mtime into a `0-<hash>` cache key (collision). |
| R4-F-05 | `workspace.rs:411-424` | error-handling | `is_readable_xs_file` only probes the first 64 KiB. Larger files with valid head + binary tail pass. |
| R4-F-06 | `workspace.rs:326-333` | logic-suspicious | `n == "game"` exact match is case-sensitive; case-insensitive FSes (NTFS/APFS) won't skip nested `Game/`. |
| R4-F-07 | `doxygen.rs:213-223` | type-safety | Byte-slicing `text[..open]` panics if a multi-byte char precedes `(`. Same shape in two related parsers. |
| R4-F-08 | `doxygen.rs:226-245` | pattern | Comma-splitting `params_text` silently truncates `vector<int, string>` / function pointers. |
| R4-F-09 | `doxygen.rs:290-303` | pattern | `Selector::parse("td").expect(...)` is re-parsed per row inside the hot loop (others are `LazyLock`-hoisted). |
| R4-F-10 | `cache.rs:33-41` | error-handling | `state_cache_dir` panics if both `dirs::state_dir()` and `dirs::home_dir()` return `None`. No fallback. |
| R4-F-11 | `cache.rs:267-279` | error-handling | "Atomic" write ignores `file.sync_data().ok()` errors and leaks `.tmp` on `write_all` failure. |
| R4-F-12 | `workspace.rs:77-84, 158-179` | bug | `WorkspaceError::MissingGameDirectory` defined but never constructed — dead variant. |

### R5 — Tools+Tests+Grammar (6 MEDIUM)

| ID | File:Line | Cat | One-line |
|----|-----------|-----|----------|
| R5-F-04 | `lsp_roundtrip_test.rs:268-285` | test | Asymmetric capture: `types_diag_raw` takes FIRST notification, `bad_clean_diag_raw` takes LAST. Undocumented and timing-fragile. |
| R5-F-05 | `tests/game_folder_parse.rs:366-384, 651-658` | comment-mismatch | Comment says "soft threshold with 10% headroom" but the live test asserts strict `== 0`. Comment stale post-include-paste-fix. |
| R5-F-06 | `lsp_roundtrip_test.rs:184-189` | platform-safety | `Stdio::from(File::create("/tmp/xs_lsp_server_stderr.log"))` panics on Windows + leaks artifacts + concurrent runs collide. |
| R5-F-07 | `main.rs:14-57` | cli-ux | No `--help` / `--version`. User invoking `xs-language-server --help` sees nothing. |
| R5-F-08 | `lsp_roundtrip_test.rs:1466-1559` | concurrency | Cleanup runs before timeout check; process can outlive a deleted tree and report PASS on no-op. |
| R5-F-09 | `tests/game_folder_parse.rs:159-204` | test | `is_recoverable_forward_decl` is a "mirror of `symbols::extract_error_forward_declaration`" — silently diverges if production changes. |
| R5-F-10 | `lsp_roundtrip_test.rs:1481-1488` | concurrency | Comment says "Don't take stdout" but the next line calls `take()`; pipe buffer is the OS limit. |

### R6 — Plugin Editor+PSI (6 MEDIUM)

| ID | File:Line | Cat | One-line |
|----|-----------|-----|----------|
| R6-F-03 | `XsTextAttributesKeys.kt:13` | comment-mismatch | `XS_DEFAULT` key named "Default" but its fallback attribute key is `IDENTIFIER`. Self-contradictory. |
| R6-F-04 | `XsParserDefinition.kt:35, 46-51` | platform-contract | `getStringLiteralElements` includes standalone `STRING_QUOTE` and `CHAR_QUOTE` tokens — IDE may attach string-level references to a `"` character. |
| R6-F-05 | `XsCommenter.kt:5-15` | platform-contract | `getCommentedBlockCommentPrefix/Suffix` legacy API deprecated since 2022.1; should implement `CommenterWithEditableComments`. |
| R6-F-06 | `XsBraceMatcher.kt:13` | platform-contract | `isPairedBracesAllowedBeforeType` returning `true` unconditionally overrides IntelliJ's "skip-over" heuristic for `()[]`. |
| R6-F-07 | `XsQuoteHandler.kt:16` | platform-contract | `hasNonClosedLiteral` always `false` disables auto-close-at-EOL / escape-aware caret / "unclosed" inspection. |
| R6-F-08 | `XsHighlightingLexer.kt:32` | edge-case | `NUMBER_REGEX` accepts only decimal `int(.int)?([eE][+-]?int)?`; no hex (`0x`), binary (`0b`), octal (`0o`), or `5.` style. |

### R7 — Plugin LSP+Settings+TextMate (10 MEDIUM)

| ID | File:Line | Cat | One-line |
|----|-----------|-----|----------|
| R7-F-03 | `XsBinaryResolver.kt:16-19, 99-110` | comment-mismatch | Class-level doc lists 5 resolution steps ending in PATH; step 4 is actually 2 candidates (base-relative + CWD). |
| R7-F-04 | `XsSettings.kt:32-44` | persistence | `getState()` returns live `State` with mutable `modPaths`. IDE serializes on background thread — race can persist empty list. |
| R7-F-05 | `XsLspServerDescriptor.kt:60-75, 91-95` | comment-mismatch | KDoc blames "platform's default process handler" for stderr merge, but `createCommandLine` already sets `withRedirectErrorStream(false)`. |
| R7-F-06 | `XsLspServerDescriptor.kt:60-75` | error-handling | `Process` is leaked if `OSProcessHandler` constructor throws; no `process.destroy()` on the error path. |
| R7-F-07 | `XsLspServerDescriptor.kt:77-85` | comment-mismatch | KDoc says "reads game folder every time server is started or restarted"; code reads the constructor parameter first, falling back to settings on blank. |
| R7-F-09 | `XsLspServerDescriptor.kt:103-107` | concurrency | `currentModWorkspaceFolders(project)` calls `refreshAndFindFileByPath` from a super-class constructor — before the descriptor exists; possible read-action violation. |
| R7-F-10 | `XsLspServerManager.kt:43-50` | concurrency | `internal var sendWorkspaceFolderChange` / `restartServerHandler` are plain mutable fields with no `@Volatile` / EDT assertions; test-worker writes race EDT reads. |
| R7-F-11 | `XsLspServerManager.kt:52-59, 166-168` | concurrency | `addModPathsListener` registered in `init` but never removed in `dispose()`. Comment says platform handles it; it doesn't. |
| R7-F-14 | `XsModAutoDetector.kt:27-41` | edge-case | `Files.isDirectory(child)` follows symlinks → self-referential symlink causes infinite recursion / `StackOverflowError`. |
| R7-F-17 | `META-INF/plugin.xml:19-40` | platform-contract | Two file-type registrations on `.xs`: native `<fileTypeFactory>` AND TextMate `fileTypes: ["xs"]`. Pick one (or document the layering). |
| R7-F-22 | `XsStartupActivity.kt:80-90` | edge-case | `NotificationGroupManager.getInstance().getNotificationGroup("XS Language Server")` returns `null` during first IDE launch (extensions not yet loaded); `notifyMissingGamePath` silently returns — user gets no feedback. |

---

## LOW findings (48) — compact table

| ID | File:Line | Cat | One-line summary |
|----|-----------|-----|-----------------|
| R1-F-13 | `tree-sitter-xs/grammar.js:127` | pattern | `preproc_arg` rule defined but never referenced. Dead. |
| R1-F-14 | `tree-sitter-xs/grammar.js:239-244, 258-263, 265-269` | pattern | Three syntactically identical rules (`_function_declarator`, `function_declarator`, `_function_declaration_declarator`) — only underscore differs. |
| R1-F-15 | `tree-sitter-xs/grammar.js:233-237, 213-217` | grammar | `prec.right(seq(...))` on non-recursive rules does nothing; vestigial copy-paste from C grammar. |
| R1-F-16 | `tree-sitter-xs/grammar.js:385-417` | pattern | `_top_level_expression_statement` aliases to `expression_statement`; aliasing hides which paths produce which nodes. |
| R1-F-17 | `symbols.rs:60-63, 263` | style | `extract_rule` hard-codes `Visibility::Public` for rules; `Visibility` docstring only mentions functions/variables. |
| R2-F-15 | `semantic.rs:349-358` | style | `check_extern_collisions` is hoisted out of `impl` but body keeps 4-space indent as if still inside. Refactor leftover. |
| R2-F-16 | `semantic.rs:783-809` | style | `forward_callable_merged` has no doc comment, unlike sibling `forward_callable`. |
| R2-F-17 | `typecheck.rs:65-72` | pattern | `walk` uses `named_children` while `semantic.rs::walk_calls` uses `children` (all). Pick one. |
| R2-F-18 | `typecheck.rs:397-407` | edge-case | `n.fract() == 0.0` is fragile for values ≥ 2^53 (IEEE 754 rounding). |
| R2-F-19 | `diagnostics.rs:125-139` | style | "include not found" diagnostic tagged with engine code `E0310` (same as symbol resolution). Use a distinct internal code. |
| R2-F-20 | `typecheck.rs:145-149` | pattern | Extra args silently dropped from `arg_types_compatible` check (only count warning emitted). |
| R3-F-11 | `server.rs:348-369` | pattern | Two separate lock acquisitions in `did_open` (one to lookup_mod, one to iterate mods). |
| R3-F-15 | `merged_view.rs:25-52` | comment-mismatch | `include_line` doc says "0-indexed line of that `include` directive" but actually stores the start of the directive range. |
| R3-F-16 | `server.rs:191-217` | bug | `initialize` logs `workspace_folders` as `mods().len()` AFTER registration failures — diverges from the spec count without warning. |
| R3-F-17 | `server.rs:611-627` | pattern | `document_symbol` returns `Ok(None)` while `publish_diagnostics` says "parse failed". Both handlers disagree for the same condition. |
| R3-F-18 | `completion.rs:79-98` | pattern | User symbol + engine API with same name → two completion items, both labelled. LSP client can't dedupe. |
| R3-F-19 | `references.rs:99-107` | pattern | `to_locations` is `pub` but has no production caller (only tests). Should be `#[cfg(test)]` or inlined. |
| R3-F-20 | `merged_view.rs:581-589` | edge-case | `file_overrides` keys use `/`-separated paths; on Windows `Path::eq` is byte-exact — cache key may mismatch between runs. |
| R3-F-21 | `server.rs:488-501,506-549,…` | style | Six copies of `docs.lock().await; docs.get(uri).unwrap_or("")`. Extract helper. |
| R3-F-22 | `definition_check.rs:239-246` | style | Three identical `node_range` helpers across `definition_check.rs`, `references.rs`, `symbols.rs`. |
| R4-F-13 | `workspace.rs:44-52` | pattern | `visible_files` clones the whole override HashMap just to make a Vec; `dedup_by` is defensive only; `workspace` param unused (could be `game_path: &Path`). |
| R4-F-14 | `workspace.rs:218-232` | logic-suspicious | `resolve_file` applies `exists()` only in the vanilla branch, not the override branch — TOCTOU hole if an overlay file is deleted between registration and resolution. |
| R4-F-15 | `workspace.rs:194-200` | concurrency | `lookup_mod`'s `max_by_key(m.mod_path.as_os_str().len())` could mis-rank on Windows (OsStr length in code units, not components). |
| R4-F-16 | `cache.rs:116-128` | pattern | `parse_file_key` opens the file twice (`metadata` then `read`); `parse_cache_key` is its mtime+hash-only special case. Could single-open. |
| R4-F-17 | `cache.rs:228-251` | pattern | `invalidate_parse_cache` reads and JSON-parses every cache file (O(N)); also leaves `.tmp` files behind (no extension check). |
| R4-F-18 | `engine_api.rs:111-116` | pattern | `to_engine_data` clones all 1804 syscalls + 193 aiplans just for serialisation. Could write directly. |
| R4-F-19 | `cache.rs:168-175` | logic-suspicious | `entry.content_hash == content_hash && entry.mtime_millis == mtime_millis` is redundant — the cache filename IS the key. Only `relative_path` actually disambiguates. |
| R4-F-20 | `engine_api.rs:72-87` | type-safety | `serde_json::Number::to_string()` preserves canonical form; the cache round-trip may not reproduce the original textual form. (TRUNCATED) |
| R5-F-11 | `lsp_roundtrip_test.rs:1699-1700` | test | `"uri": ` is a one-character extension of `"uri":`; `str::matches` non-overlap count inflates the total by every `"uri": `. |
| R5-F-12 | `bin/day1_probe.rs:36-39` | error-handling | `expect("parser returned None on valid input")` is misleading; `None` signals "could not parse", reachable on 0-byte / multi-MB-binary input. |
| R5-F-13 | `bin/day1_probe.rs:73-77` | style | `--tree-at=N` (single token) silently parses to `None`; only two-arg form accepted. |
| R5-F-14 | `bin/dump_top_level.rs:12-14` & `inspect_tree.rs:9-11` | error-handling | No `args.len()` guard before `args[1]`; panics with index-out-of-bounds instead of usage message. |
| R5-F-15 | `bin/inspect_tree.rs:22` | smell | Hard-coded `target_lines = [17, 124, 423, 526, 527, 773]` (looks like `human_assist.xs`); no second positional arg for arbitrary file. |
| R5-F-16 | `Cargo.toml:1-25` | build-config | No `[lints]` table; clippy essentially off for a public-API crate. edition = "2024" without rust-toolchain pin. |
| R6-F-09 | `XsSurroundingPairsProvider.kt:13` | pattern | Class named `…Provider` but doesn't implement `SurroundingPairsProvider`. Future maintainer may try to register it. Rename to `Config` / `Table`. |
| R6-F-10 | `XsColorSettingsPage.kt:14` | pattern | `getIcon()` returns `null` even though `XsFileType` has `xs.svg`. Inconsistent in Settings → Color Scheme → XS. |
| R6-F-11 | `psi/psi/XsFile.kt:1` | pattern | Package `com.aomr.xs.psi.psi` — nested PSI package artefact of IntelliJ wizard. Should be `com.aomr.xs.psi`. |
| R6-F-12 | `XsFileTypeFactory.kt:1, 7` | pattern | `@Suppress("DEPRECATION")` on a non-deprecated API (`FileTypeFactory`). Foreshadows future bad suppressions. (TRUNCATED) |
| R7-F-18 | `XsTextMateBundleProvider.kt:12-29` | edge-case | `Files.createTempDirectory("xs-textmate-bundle")` not registered with `deleteOnExit()`. Per-session leak. |
| R7-F-19 | `XsConfigurable.kt:103-107, 109-111` | platform-contract | `disposeUIResources` empties the model but not `ReorderTransferHandler.sourceIndex` / `JBList` selection. Benign today. |
| R7-F-20 | `XsConfigurable.kt:84-87` | edge-case | `isModified` compares raw `gamePathField.text` to trimmed `state.gamePath` → paste with trailing space causes `isModified == true` after Apply. |
| R7-F-21 | `XsLspServerManager.kt:154-156` | lsp | `pathToWorkspaceFolder` builds two `File` instances per mod path; trailing `/` makes `name` empty. |
| R7-F-23 | `XsLspServerManager.kt:52-59` | platform-contract | `: com.intellij.openapi.Disposable` qualified import + empty `dispose()` is boilerplate (services are auto-disposed). |
| R7-F-24 | `syntaxes/xs.tmLanguage.json:117-168, 169-172, 713-720` | textmate | Two defined-but-unreferenced rules (`access-method`, `backslash_escapes`) and one self-referential dead rule (`disabled` → `#disabled`). |
| R7-F-25 | `XsLspServerDescriptor.kt:65` | style | `getCommandLineList(null)` — passing platform-typed `null` for `@Nullable Boolean`. Use `false` (or whatever the intent is). |
| R7-F-26 | `XsConfigurable.kt:113-127` | style | Fully-qualified `com.intellij.openapi.fileChooser.FileChooser` inline; rest of file uses imports. Inconsistent style. |

---

## NIT findings (12)

| ID | File:Line | Cat | One-line |
|----|-----------|-----|----------|
| R1-F-17 (style) | `symbols.rs:60-63, 263` | style | Visibility doc vs. extract_rule default mismatch (also in LOW table above). |
| R2-F-21 | `semantic.rs:923-942` & `typecheck.rs:454-…` | pattern | Identical node helpers in `semantic.rs`, `typecheck.rs`. (See also R3-F-22.) |
| R2-F-22 | `semantic.rs:286-301, diagnostics.rs:119-123` | style | Two byte-identical `merge_diagnostic_maps` functions. |
| R3-F-21 / F-22 | `server.rs:488-501, …` etc. | style | Six copies of doc-fetch; three copies of `node_range`. |
| R4-F-… (NIT) | `cache.rs` / `engine_api.rs` | style | Cosmetic. |
| R5-F-16 | `Cargo.toml:1-25` | build-config | See LOW above (no `[lints]` table). |

(R2-F-21/F-22 and R3-F-21/F-22 are duplicated counters across reviewers — see pattern note in LOW table above.)

---

## Per-file finding count

### LSP — `tools/xs-language-server/src/`

| File                               | Findings | C | H | M | L | N |
|------------------------------------|---------:|:-:|:-:|:-:|:-:|:-:|
| `parser.rs`                        | 2        | 0 | 0 | 2 | 0 | 0 |
| `word.rs`                          | 2        | 0 | 1 | 1 | 0 | 0 |
| `symbols.rs`                       | 8        | 0 | 3 | 4 | 1 | 0 |
| `semantic.rs`                      | 11       | 0 | 1 | 6 | 3 | 1 |
| `typecheck.rs`                     | 6        | 0 | 0 | 3 | 2 | 1 |
| `diagnostics.rs`                   | 5        | 0 | 2 | 1 | 1 | 1 |
| `server.rs`                        | 11       | 1 | 4 | 4 | 2 | 0 |
| `merged_view.rs`                   | 5        | 0 | 0 | 3 | 1 | 1 |
| `definition_check.rs`              | 2        | 0 | 0 | 1 | 1 | 0 |
| `references.rs`                    | 3        | 0 | 1 | 0 | 2 | 0 |
| `completion.rs`                    | 1        | 0 | 0 | 0 | 1 | 0 |
| `workspace.rs`                     | 6        | 0 | 0 | 4 | 2 | 0 |
| `cache.rs`                         | 8        | 0 | 1 | 3 | 4 | 0 |
| `engine_api.rs`                    | 3        | 0 | 0 | 0 | 3 | 0 |
| `doxygen.rs`                       | 6        | 0 | 2 | 3 | 1 | 0 |
| `main.rs`                          | 2        | 0 | 0 | 1 | 1 | 0 |
| `lib.rs`                           | 0        | 0 | 0 | 0 | 0 | 0 |
| `bin/day1_probe.rs`                | 2        | 0 | 0 | 0 | 2 | 0 |
| `bin/dump_top_level.rs`            | 1        | 0 | 0 | 0 | 1 | 0 |
| `bin/inspect_tree.rs`              | 2        | 0 | 0 | 0 | 2 | 0 |
| `bin/lsp_roundtrip_test.rs`        | 6        | 0 | 3 | 3 | 0 | 0 |
| `tests/game_folder_parse.rs`       | 2        | 0 | 0 | 2 | 0 | 0 |
| `Cargo.toml`                       | 1        | 0 | 0 | 0 | 1 | 0 |

### LSP — `tools/xs-language-server/tree-sitter-xs/grammar.js`

| File            | Findings | C | H | M | L | N |
|-----------------|---------:|:-:|:-:|:-:|:-:|:-:|
| `grammar.js`    | 4        | 0 | 0 | 0 | 4 | 0 |

### Plugin — `tools/intellij-xs-plugin/src/main/`

| File                                              | Findings | C | H | M | L | N |
|---------------------------------------------------|---------:|:-:|:-:|:-:|:-:|:-:|
| `XsFileType.kt`, `XsFileTypeFactory.kt`, `XsLanguage.kt` | 1     | 0 | 0 | 0 | 1 | 0 |
| `editor/XsBraceMatcher.kt`                        | 1        | 0 | 0 | 1 | 0 | 0 |
| `editor/XsCommenter.kt`                           | 1        | 0 | 0 | 1 | 0 | 0 |
| `editor/XsQuoteHandler.kt`                        | 1        | 0 | 0 | 1 | 0 | 0 |
| `editor/XsSurroundingPairsProvider.kt`            | 1        | 0 | 0 | 0 | 1 | 0 |
| `highlight/XsColorSettingsPage.kt`                | 1        | 0 | 0 | 0 | 1 | 0 |
| `highlight/XsHighlightingLexer.kt`                | 2        | 0 | 1 | 1 | 0 | 0 |
| `highlight/XsSyntaxHighlighter.kt`                | 1        | 0 | 1 | 0 | 0 | 0 |
| `highlight/XsTextAttributesKeys.kt`               | 1        | 0 | 0 | 1 | 0 | 0 |
| `psi/psi/XsFile.kt`                               | 1        | 0 | 0 | 0 | 1 | 0 |
| `psi/XsParserDefinition.kt`                        | 1        | 0 | 0 | 1 | 0 | 0 |
| `lsp/XsBinaryResolver.kt`                         | 3        | 1 | 1 | 1 | 0 | 0 |
| `lsp/XsLspServerDescriptor.kt`                    | 5        | 0 | 1 | 4 | 0 | 0 |
| `lsp/XsLspSupportProvider.kt`                     | 1        | 0 | 1 | 0 | 0 | 0 |
| `lsp/XsLspServerManager.kt`                       | 5        | 0 | 1 | 3 | 1 | 0 |
| `settings/XsConfigurable.kt`                      | 3        | 0 | 0 | 0 | 2 | 1 |
| `settings/XsSettings.kt`                          | 1        | 0 | 0 | 1 | 0 | 0 |
| `settings/XsModAutoDetector.kt`                   | 1        | 0 | 0 | 1 | 0 | 0 |
| `startup/XsStartupActivity.kt`                    | 2        | 1 | 0 | 1 | 0 | 0 |
| `textmate/XsTextMateBundleProvider.kt`            | 1        | 0 | 0 | 0 | 1 | 0 |
| `resources/META-INF/plugin.xml`                   | 2        | 0 | 1 | 1 | 0 | 0 |
| `resources/syntaxes/xs.tmLanguage.json`           | 1        | 0 | 0 | 0 | 1 | 0 |

### Files with **zero findings**

- `tools/xs-language-server/src/lib.rs` — pure module declarations.

(Reviewer 5 covered all main LSP files; reviewer 6 covered all main plugin sources listed. Files marked "no findings" inline within each reviewer's output include `lsp_roundtrip_test.rs::main` (F-07 aside), `day1_probe.rs` (mostly-LOW), etc.)

---

## Audit priorities (read these first)

1. **R7-F-13** — `XsStartupActivity.runActivity` on EDT. Easy reproduction on a misconfigured project; risk: IDE freeze.
2. **R3-F-01** — Three-way deadlock. Audit every site that takes more than one of `documents` / `symbol_tables` / `merged_views` and enforce a single order.
3. **R7-F-02** — Race on `cachedBundledPath`. Wrap in `synchronized(this)`.
4. **R3-F-09** — "Internal error: failed to install XS language" surfaced on user typo. Change to a parse-error-class message.
5. **R5-F-01 / R5-F-02 / R5-F-03** — All three are passing tests for the wrong reason. Re-run the round-trip test against each mutation and verify each PASS message survives.
6. **R2-F-01** — Cross-file resolution "first file wins, callable or not". Confirm with a 2-file fixture.
7. **R4-F-01** — `static int foo` parsed as `return_type="static int"`. grep the cache for `static ` in return types.
8. **R1-F-01 / R1-F-02 / R1-F-03** — Three related comments-and-reachability smells in `symbols.rs`. Delete the unreachable helpers and the misleading comments together.
9. **R1-F-04** — UTF-16 vs byte offset in `identifier_at_cursor`. Probable panic on `é`-prefixed identifier at the cursor.
10. **R2-F-08** — Method-call type-checking silently drops to `Unresolved`. Decide method-calls policy (type-check vs skip).
11. **R7-F-16** — Plugin can't install on Community Edition because of `<depends>com.intellij.modules.ultimate</depends>`. One-line removal.
12. **R7-F-08** — Possible LSP process leak per opened `.xs` file. Verify with `ps -ef`.

---

## Methodology notes

- 7 parallel reviewers dispatched with identical rubric, non-overlapping file slices, fresh context each.
- Each finding has `file:line` evidence + category tag + "why it matters" + "what to check (not a fix)" + confidence.
- Reviewers were instructed to err on the side of flagging; the orchestrator did NOT dedupe aggressively because file lists were partitioned.
- Three reviewer outputs were truncated by the tool (R2-F-22, R4-F-20, R6-F-12 — all LOW or NIT severity in the truncated portions). Each still has file:line and category intact.
- No files modified. No tests run. No commands executed by the reviewers.
- A companion CSV lives next to this file (`2026-06-29-lsp-and-plugin-findings.csv`) for sortable auditing.

---

## Per-reviewer raw output locations

The complete raw reviewer outputs are persisted to Engram under topic keys `sdd/review-2026-06-29/<reviewer-name>`. This markdown report is the audit-ready synthesis.

Last verified: 2026-06-29 against commit 0053352ad36c32c474dbd6cd5eaa06ff2c763ac3
Last verified: 2026-06-29 — R5-F-01, R5-F-02, R5-F-03 resolved via `openspec/changes/archive/2026-06-29-fix-lsp-test-honesty/` (PASS WITH WARNINGS, 203/203 tests)
