# Design: `true-include-paste`

## 1. Architecture overview

The redesigned Rust XS Language Server replaces the line-regex approximation of `include "..."` with a true textual-paste model. For every analyzed file it builds a `MergedView` that contains the current file plus the resolved, visibility-filtered symbols from every direct and transitive include. That merged scope feeds completion, hover, go-to-definition, references, semantic diagnostics, and type checking instead of the per-file symbol table or the whole-project fallback.

The new pipeline:

```text
[didChange/didOpen] → parser::parse → symbols::build_symbol_table (per-file)
                                              ↓
                                  parser::extract_include_directives
                                              ↓
                              workspace::resolve_include (per target)
                                              ↓
                              merged_view::MergedView::build
                                              ↓
                          MergedView (current + direct + transitive)
                                              ↓
              ┌─────────────────────────────┬─────────────┐
       completion/hover/        semantic::            typecheck::
       definition/references    check_forward        resolve_workspace
                                _declarations         _function
                                              ↓
                              diagnostics::collect_all
                                              ↓
                                    client.publish_diagnostics
```

Two parallel scopes exist in the diagnostic path:

1. **Project-wide scope** (`semantic::VirtualProject`) is still built once per `publish_diagnostics` for the single remaining project-wide rule: `extern` collision detection.
2. **Per-file include-closure scope** (`MergedView`) is built/cached per open file and is used for forward-declaration checks, mutable-redefinition checks, user-function type checking, and every non-mutating LSP feature.

## 2. Module layout

Create a new module **`src/merged_view.rs`** and add `pub mod merged_view;` in `src/lib.rs` after `pub mod workspace;`.

**Justification**

- The include-graph builder, cycle detector, visibility filter, and provenance bookkeeping are a distinct concern from overlay resolution in `workspace.rs` and from protocol handling in `server.rs`.
- `workspace::VirtualProject` is already responsible for mod-overlay path mapping; adding merge arithmetic there would turn it into a 700-line file.
- `server.rs` should only cache and invalidate merged views, not compute them.

This confirms the proposal's §6.3 recommendation: `merged_view.rs` is the right home.

## 3. New public API surface

All types live in the new `crate::merged_view` module.

```rust
/// Where a symbol in a merged view came from and how it entered the scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisibilityProvenance {
    /// Defined in the file being analyzed.
    OwnFile,
    /// Pasted from an included file.
    Included {
        /// Absolute path of the file that defines the symbol.
        origin: PathBuf,
        /// 1 = direct include, 2 = included by a direct include, etc.
        depth: usize,
        /// Absolute path of the file whose `include` directive introduced
        /// this symbol into the current scope.
        introduced_at: PathBuf,
        /// 0-indexed line of that `include` directive in `introduced_at`.
        include_line: u32,
    },
}

/// A symbol visible in the merged scope, with its provenance metadata.
#[derive(Debug, Clone)]
pub struct MergedSymbol {
    pub symbol: crate::symbols::Symbol,
    pub provenance: VisibilityProvenance,
}

/// A single resolved include relationship.
#[derive(Debug, Clone)]
pub struct IncludeEdge {
    pub from: PathBuf,
    pub to: PathBuf,
    pub root: crate::workspace::IncludeRoot,
    /// Line of the `include` directive in `from`.
    pub include_line: u32,
}

/// Directed graph of resolved includes for one file.
#[derive(Debug, Default, Clone)]
pub struct IncludeGraph {
    pub edges: Vec<IncludeEdge>,
}

impl IncludeGraph {
    /// Return every file that has an edge pointing at `target`.
    pub fn dependents(&self, target: &Path) -> Vec<&Path>;
    /// True if a cycle was encountered during graph construction.
    pub fn is_cyclic(&self) -> bool;
}

/// Diagnostic produced when an include target cannot be resolved.
#[derive(Debug, Clone)]
pub struct IncludeDiagnostic {
    pub target: String,
    pub from: PathBuf,
    pub root: crate::workspace::IncludeRoot,
    pub range: tower_lsp::lsp_types::Range,
}

/// The resolved, filtered scope of one file.
#[derive(Debug, Clone)]
pub struct MergedView {
    current_file: PathBuf,
    own_table: crate::symbols::SymbolTable,
    symbols: Vec<MergedSymbol>,
    graph: IncludeGraph,
    missing: Vec<IncludeDiagnostic>,
    /// Source text for every file in the closure (current file + includes).
    /// Needed for cross-file `textDocument/references`.
    sources: std::collections::HashMap<PathBuf, String>,
    /// Per-file symbol tables for the same closure.
    tables: std::collections::HashMap<PathBuf, crate::symbols::SymbolTable>,
}

impl MergedView {
    /// Build the merged view for `file` from its (already-parsed) source text.
    ///
    /// * Resolves every `include` through `workspace::resolve_include`.
    /// * Loads included-file symbol tables through `cache::load_or_parse_symbols`.
    /// * Cycles are terminated cleanly; symbols seen up to the re-entry remain.
    /// * Missing targets become `IncludeDiagnostic`s, not fatal errors.
    pub fn build(
        file: &Path,
        source: &str,
        own_table: &crate::symbols::SymbolTable,
        workspace: &crate::workspace::Workspace,
        project: &crate::workspace::VirtualProject,
        cache_dir: &Path,
    ) -> Result<MergedView, MergeError>;

    pub fn find(&self, name: &str) -> Option<&MergedSymbol>;
    pub fn matching(&self, prefix: &str) -> impl Iterator<Item = &MergedSymbol>;
    pub fn graph(&self) -> &IncludeGraph;
    pub fn missing_includes(&self) -> &[IncludeDiagnostic];
    pub fn own_table(&self) -> &crate::symbols::SymbolTable;
    pub fn source(&self, path: &Path) -> Option<&str>;
    pub fn files(&self) -> impl Iterator<Item = &Path>;

    /// Earliest line at which `name` is visible from the current file.
    /// Returns `0` for own-file symbols and the relevant `include_line` for
    /// included symbols.
    pub fn visibility_line(&self, name: &str) -> Option<u32>;
}

#[derive(Debug, thiserror::Error)]
pub enum MergeError {
    #[error("failed to read include {path}: {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("include cycle detected: {}", .0.iter().map(|p| p.display().to_string()).join(" -> "))]
    Cycle(Vec<PathBuf>),
}
```

**Visibility during merge** (proposal §6.6)

The existing `symbols::Visibility` enum is **not** extended. Instead, each merged symbol carries a `VisibilityProvenance` wrapper. The mergefilter is applied once when symbols are copied into `MergedView`:

- Current-file symbols — all are kept (`Local`, `Const`, `Extern`, `Public`).
- Included symbols:
  - `Variable` / `Constant` — kept only if `visibility == Visibility::Extern`.
  - `Function` / `Rule` — kept unless `visibility == Visibility::Local` (i.e. unless marked `static`).

This keeps `symbols.rs` untouched except for grammar-symbol extraction and avoids inventing a new `Visibility::ExportedViaInclude` variant.

## 4. Data flow per LSP request

### 4.1 `textDocument/completion`

```rust
let uri = params.text_document_position.text_document.uri;
let text = documents.get(&uri)?;
let current_file = uri.to_file_path()?;
let merged = get_or_build_merged_view(&uri, &text).await;
let prefix = completion::prefix_at_cursor(text, pos.line, pos.character);
let mut items = engine.matching_syscalls(&prefix).map(...);
items.extend(engine.matching_aiplans(&prefix).map(...));
items.extend(completion::complete_merged(&engine, merged.as_ref(), &prefix));
Ok(CompletionResponse::Array(items))
```

The regex helper `included_files` and the `path.ends_with(target)` check are removed entirely.

### 4.2 `textDocument/hover`

```rust
let uri = ...;
let text = documents.get(&uri)?;
let pos = ...;
let Some(ident) = word::identifier_at_cursor(text, pos.line, pos.character) else { return Ok(None); };
if let Some(syscall) = engine.find_syscall(&ident) { return format_hover_syscall(syscall); }
if let Some(aiplan) = engine.find_aiplan(&ident) { return format_hover_aiplan(aiplan); }
let merged = get_or_build_merged_view(&uri, text).await;
let Some(ms) = merged.find(&ident) else { return Ok(None); };
Ok(format_hover_merged_symbol(ms, &uri_for(&ms.provenance)))
```

Hover for an included symbol shows its signature and a `file://` URI pointing to the defining file.

### 4.3 `textDocument/definition`

```rust
let uri = ...;
let text = documents.get(&uri)?;
let pos = ...;
let Some(ident) = word::identifier_at_cursor(text, pos.line, pos.character) else { return Ok(None); };
if engine symbol { return Ok(stub engine Location); }
let merged = get_or_build_merged_view(&uri, text).await;
let Some(ms) = merged.find(&ident) else { return Ok(None); };
let def_uri = Url::from_file_path(&ms.origin_path()).unwrap();
Ok(GotoDefinitionResponse::Scalar(Location { uri: def_uri, range: ms.symbol.selection_range }))
```

### 4.4 `textDocument/references`

```rust
let uri = ...;
let pos = ...;
let text = documents.get(&uri)?;
let Some(ident) = word::identifier_at_cursor(text, pos.line, pos.character) else { return Ok(None); };
if engine symbol { return Ok(Some(vec![])); }
let merged = get_or_build_merged_view(&uri, text).await;
let mut locs = Vec::new();
for path in merged.files() {
    let source = merged.source(path).unwrap_or_default();
    let tree = parser::parse(source)?;
    let table = merged.tables().get(path);
    let mut ranges = references::find_identifier_uses(&tree, source, &ident);
    if !params.context.include_declaration {
        if let Some(t) = table { ranges = references::filter_declaration(ranges, t, &ident, false); }
    }
    locs.extend(references::to_locations(&Url::from_file_path(path)?, ranges));
}
Ok(Some(locs))
```

`textDocument/rename` remains file-local: it walks only `text` for the current URI.

### 4.5 `textDocument/publishDiagnostics`

```rust
let current_file = uri.to_file_path()?;
let key = compute_merged_view_cache_key(uri, text).await;
let (merged, project) = if let Some((cached_key, mv)) = merged_views.get(uri) && cached_key == &key {
    (mv, build_semantic_project(uri).await) // project can be cached separately later
} else {
    let own_table = symbol_tables.get(uri).cloned().unwrap_or_default();
    let project = build_semantic_project(uri).await;
    let mv = MergedView::build(current_file, text, &own_table, &ws, &project?, &cache_dir)?;
    merged_views.insert(uri.clone(), (key.clone(), mv.clone()));
    (mv, project)
};
let tree = parser::parse(text)?;
let diags = diagnostics::collect_all(
    &tree, text, &engine, &mv.own_table(), project.as_ref(), Some(&mv),
);
client.publish_diagnostics(uri.clone(), diags, Some(version)).await;
```

`extern` collision still receives the full project, while forward-decl/mutable/type checks receive the merged view.

## 5. Grammar extension

Target file: `tools/xs-language-server/tree-sitter-xs/grammar.js`.

### 5.1 Add `function_pointer_type`

After `primitive_type` and before `field_declaration_list`, add:

```js
function_pointer_type: $ => seq(
  field('return', $.type_specifier),
  field('parameters', $.parameter_list),
),
```

### 5.2 Add `lambda_expression`

In the expression section, after `parenthesized_expression`, add:

```js
lambda_expression: $ => seq(
  '[',
  ']',
  optional(field('parameters', $.parameter_list)),
  optional(seq('->', field('return', $.type_specifier))),
  field('body', $.compound_statement),
),
```

Add `$.lambda_expression` to `_expression_not_binary`:

```js
_expression_not_binary: $ => choice(
  $.conditional_expression,
  $.assignment_expression,
  $.unary_expression,
  $.update_expression,
  $.cast_expression,
  $.new_expression,
  $.subscript_expression,
  $.call_expression,
  $.field_expression,
  $.identifier,
  $.number_literal,
  $.string_literal,
  $.true,
  $.false,
  $.null,
  $.parenthesized_expression,
  $.lambda_expression,        // <-- new
),
```

### 5.3 Allow function-pointer types in parameter declarations

Replace the `parameter_declaration` rule (current lines 351-356) with:

```js
parameter_declaration: $ => seq(
  repeat($._declaration_modifiers),
  field('type', choice($.type_specifier, $.function_pointer_type)),
  field('declarator', optional($.identifier)),
  optional(seq('=', field('default', $.expression))),
),
```

### 5.4 Add a conflict

Append to the `conflicts` array:

```js
[$.function_pointer_type, $.type_specifier],
```

### 5.5 Regeneration

```bash
cd tools/xs-language-server/tree-sitter-xs
tree-sitter generate
```

This rewrites `src/parser.c`, `src/grammar.json`, and `src/node-types.json`. Because `tools/xs-language-server/Cargo.toml` uses a path dependency on `tree-sitter-xs`, no crate-version bump is required; rebuild the parent crate with:

```bash
cd tools/xs-language-server
cargo build
```

### 5.6 Verification fixture

The spec grammar test is:

```xs
void boVillager(int a = -1, void(int) afterQueue = [](int id = -1) {}) { }
```

After the change the top-level node must be `function_definition` with name `boVillager` and exactly two parameters. The second parameter's `type` child must be a `function_pointer_type` whose default is a `lambda_expression`.

## 6. Symbol extraction update

Target file: `tools/xs-language-server/src/symbols.rs`.

### 6.1 Lambda default values

`extract_param_default` already captures any child after `=`. Once `lambda_expression` is a single node, it will return `[](int id = -1) {}` verbatim. No logic change is required unless the grammar produces a `lambda_expression` whose children are exposed as separate named fields, in which case `extract_param_default` should fall back to the whole-node text. The recommended implementation is unchanged:

```rust
fn extract_param_default(node: tree_sitter::Node<'_>, source: &str) -> Option<String> {
    let mut saw_eq = false;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "=" { saw_eq = true; }
        else if saw_eq { return Some(node_text(child, source).trim().to_string()); }
    }
    None
}
```

### 6.2 Function-pointer parameter types

Current `extract_params` (lines 445-463) reads the parameter type from `primitive_type` or `array_type`. Update it to recognize `function_pointer_type` and render it as `return_type(params)`:

```rust
fn extract_params(node: tree_sitter::Node<'_>, source: &str) -> Vec<Param> {
    let mut params = Vec::new();
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() != "parameter_declaration" { continue; }
        let ty = if let Some(fp) = find_named_child(child, "function_pointer_type") {
            format_function_pointer_type(fp, source)
        } else {
            find_named_child(child, "primitive_type")
                .or_else(|| find_named_child(child, "array_type"))
                .map(|n| node_text(n, source).to_string())
                .unwrap_or_default()
        };
        let name = find_named_child(child, "identifier")
            .map(|n| node_text(n, source).to_string())
            .unwrap_or_default();
        let default = extract_param_default(child, source);
        params.push(Param { ty, name, default });
    }
    params
}

fn format_function_pointer_type(node: tree_sitter::Node<'_>, source: &str) -> String {
    let ret = find_named_child(node, "return")
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();
    let params = find_named_child(node, "parameters")
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_else(|| "()".to_string());
    format!("{}{}", ret, params)
}
```

### 6.3 ERROR-node recovery for function definitions

Currently `extract_error_forward_declaration` (lines 333-378) returns early if a `compound_statement` child exists. Add a companion that recovers a `Function` symbol from an `ERROR` containing a primitive type, identifier, parameter list, and a body:

```rust
fn extract_error_function_definition(
    node: tree_sitter::Node<'_>,
    source: &str,
    out: &mut Vec<Symbol>,
) {
    if node.kind() != "ERROR" { return; }
    if find_named_child(node, "compound_statement").is_none() { return; }
    let ty = find_named_child(node, "primitive_type")
        .or_else(|| find_named_child(node, "array_type"))
        .map(|n| node_text(n, source).to_string())
        .unwrap_or_default();
    let name_node = match find_named_child(node, "identifier") {
        Some(n) => n,
        None => return,
    };
    let params = match find_named_child(node, "parameter_list") {
        Some(n) => extract_params(n, source),
        None => return,
    };
    let name = node_text(name_node, source).to_string();
    let modifiers = extract_modifiers(node);
    let full_range = node_range(node);
    let selection_range = node_range(name_node);
    let detail = format_function_detail(&ty, &name, &params);
    out.push(Symbol {
        name,
        kind: SymbolKind::Function,
        ty,
        params,
        is_extern: modifiers.is_extern,
        is_mutable: modifiers.is_mutable,
        is_forward: false,
        visibility: function_visibility(&modifiers),
        full_range,
        selection_range,
        detail,
    });
}
```

Call it from `build_symbol_table` alongside the existing ERROR branch:

```rust
"ERROR" => {
    extract_error_forward_declaration(child, source, &mut table.symbols);
    extract_error_function_definition(child, source, &mut table.symbols);
}
```

If the grammar fix is correct, the `bo_*` functions will be extracted through the normal `function_definition` path; the ERROR recovery path is a safety net for remaining lambda variants.

## 7. Cache invalidation strategy

### 7.1 Cache key

The merged view for a file depends only on:

1. the current file content, and
2. the content of every resolved include file.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MergedViewCacheKey {
    pub current_hash: String,
    pub includes: Vec<(PathBuf, String)>, // sorted by path
}

impl MergedViewCacheKey {
    pub fn new(current_source: &str, view: &MergedView) -> Self {
        let current_hash = crate::cache::sha256_bytes(current_source.as_bytes());
        let mut includes: Vec<_> = view
            .files()
            .filter(|p| *p != view.current_file())
            .map(|p| (p.to_path_buf(), crate::cache::sha256_bytes(view.source(p).unwrap_or("").as_bytes())))
            .collect();
        includes.sort_by(|a, b| a.0.cmp(&b.0));
        Self { current_hash, includes }
    }
}
```

The hash reuses `cache::sha256_bytes` so no additional disk state is needed.

### 7.2 Storage

Server state grows one field in `XsLanguageServer` (`src/server.rs` line 42-55):

```rust
pub merged_views: Arc<Mutex<HashMap<Url, (MergedViewCacheKey, crate::merged_view::MergedView)>>>,
```

The map is populated in `publish_diagnostics` and read by non-mutating handlers.

### 7.3 Invalidation

- On `did_close` — remove the entry for the URI.
- On `didChange` — the current-source hash changes, so the next `publish_diagnostics` rebuilds automatically.
- On `workspace/didChangeWatchedFiles` — for each changed file, walk every cached merged view's `IncludeGraph` and remove entries whose closure contains the changed path or any of its dependents. Then re-diagnose those open files.

This addresses proposal §6.4. A later optimization could keep a reverse-dependency index, but a linear scan over open files is sufficient because the number of open files per LSP session is small.

### 7.4 Lifetime

Per open file only. When the file is closed the cached merged view is dropped immediately.

## 8. Open-question answers

For each deferred question from proposal §6:

### 8.1 Should grammar/symbol work be in this change?

**Yes.** Without it the dominant `bo_*` callees (`boVillager`, `boBuild`, `boUnit`, `boTransaction`, etc.) in `game/ai/core/bo_system/bo_system.xs` surface as `ERROR` nodes rather than `function_definition` symbols. Merging symbol tables cannot expose symbols that were never extracted, so the unresolved-symbol baseline will not move meaningfully. The grammar change is therefore in scope.

### 8.2 Should `extern` collision and `mutable` redefinition operate on the full project or on the include closure?

- **`extern` collision stays project-wide.** `extern` enforces single-definition linking across all files the engine loads together, not only files pasted via `include`.
- **`mutable` redefinition operates on the merged include closure.** Included files are pasted, so the effective translation unit for redefinition is the current file plus its resolved includes.

Implementation: `semantic::check_extern_collisions` continues to take the full `semantic::VirtualProject`; a new `semantic::check_mutable_redefinitions_for_merged_view` takes `&MergedView`.

### 8.3 Where does the merged view live?

In a new `src/merged_view.rs` module next to `workspace.rs`. It isolates the include graph, cycle detection, merge arithmetic, and visibility filtering from `server.rs` and avoids bloating `workspace.rs`, which already owns overlay resolution.

### 8.4 What is the cache invalidation key and lifetime?

Key: `(sha256(current_file_text), sorted [(include_absolute_path, sha256(include_text))])`. Lifetime: per open file, dropped on close. Invalidation on watched-file changes uses the `IncludeGraph` stored in each cached `MergedView`.

### 8.5 Should `references`/`rename` cross include boundaries?

- `references` **yes**. Because included files are pasted text, uses inside them are semantically uses in the current translation unit.
- `rename` **no**. Renaming a symbol in a shared include target would rewrite a file used by many other includers; that needs a separate design and is out of scope.

### 8.6 How should `static`/`extern` visibility be represented during a merge?

Keep the existing `Visibility` enum unchanged. Wrap each merged symbol with `VisibilityProvenance` metadata and apply the filter once, during merge construction. The rule is:

- Included `Variable`/`Constant` kept only if `visibility == Extern`.
- Included `Function`/`Rule` kept unless `visibility == Local`.

This avoids inventing `Visibility::ExportedViaInclude` and keeps the visibility rule explicit and testable in one place.

### 8.7 What is the engine behavior for `mutable extern` functions?

Treat `mutable` and `extern` as orthogonal:

- `mutable` makes a function forward-callable and redefinable.
- `extern` makes a symbol visible across files without `include`.

If both modifiers appear on the same function, the symbol has `is_mutable = true` and `visibility = Extern`; both sets of rules apply normally. No XS sample in the shipped game demonstrates this combination, so a manual engine-verification fixture should be added before final acceptance, but the rule does not require special handling.

## 9. File-by-file change summary

| File | LOC delta | New public items | Notes |
|------|-----------|------------------|-------|
| `src/lib.rs` | +1 | `merged_view` module | New module registration. |
| `src/parser.rs` | +25 | `extract_include_directives` | Returns `(target, range)` pairs from a parsed tree; used by `merged_view.rs`. |
| `src/workspace.rs` | +45 | `game_relative_path`, `relativize` made `pub(crate)` | Computes `game/` relative path for both overlay and vanilla files so `MergedView` can use `cache::load_or_parse_symbols`. |
| `src/symbols.rs` | +70 / ~30 | `format_function_pointer_type`, `extract_error_function_definition` | Recognize function-pointer parameter types; recover function symbols from ERROR nodes that have a body. |
| `src/merged_view.rs` (NEW) | ~260 | `MergedView`, `IncludeGraph`, `IncludeEdge`, `VisibilityProvenance`, `MergedSymbol`, `MergeError`, `IncludeDiagnostic`, `MergedView::build` | Core include-paste scope builder. |
| `src/completion.rs` | -30 / +20 | `complete_merged` | Remove `included_files` regex, `path.ends_with`, and project-symbol loop; consume `MergedView::matching`. |
| `src/hover.rs` | — | (server.rs helpers) | No new file; format helper extended to accept a `MergedSymbol`. |
| `src/semantic.rs` | +90 / ~80 | `check_forward_declarations_for_merged_view`, `check_mutable_redefinitions_for_merged_view`, `forward_callable_merged` | Forward-decl and mutable checks use the merged scope and respect include directive line numbers. `extern` collision remains project-wide. |
| `src/typecheck.rs` | +20 / ~10 | `resolve_workspace_function` refactored | Looks up user functions in `MergedView` instead of the full project. |
| `src/diagnostics.rs` | +15 / ~10 | `collect_all` signature extended | Accepts an optional `MergedView`; routes it to semantic and typecheck. |
| `src/references.rs` | +25 | `find_identifier_uses_in_source`, `to_locations` reused | References handler in `server.rs` loops over all files in the merged closure. |
| `src/server.rs` | +100 / ~70 | `merged_views`, `get_or_build_merged_view`, `invalidate_merged_views_for` | Cache merged views, invalidate on watched-file changes, use them in all non-mutating handlers. |
| `src/cache.rs` | +15 | `sha256_bytes` already public; no new API | Optionally expose `parse_file_key_with_hash` if needed for the cache key. |
| `tree-sitter-xs/grammar.js` | +~35 / generated | `function_pointer_type`, `lambda_expression` | Regenerate `src/parser.c`, `src/grammar.json`, `src/node-types.json`. |
| `tests/game_folder_parse.rs` | +~40 / ~10 | threshold + per-callee guard | Tighten `UNRESOLVED_SYMBOL_THRESHOLD` to < 1,000; guard top-10 `bo_*` counts. |
| `src/bin/lsp_roundtrip_test.rs` | +~90 / ~20 | include scenarios | Completion/hover/definition/references across includes; missing include diagnostic. |

## 10. Test plan

| Layer | What | Approach |
|-------|------|----------|
| Unit — grammar | `void boVillager(int a = -1, void(int) afterQueue = [](int id = -1) {}) { }` parses as `function_definition` named `boVillager`. | `#[test]` in `tree-sitter-xs` or `symbols.rs` using `parser::parse`. |
| Unit — symbol extraction | `boConditionalWait` `bool() condition = []() -> bool { ... }` extracts as `Function` with two parameters. | `#[test]` in `symbols.rs`. |
| Unit — merged view | Direct include, transitive include, cycle, missing target, mod overlay, AI/TR/RM include roots, static hidden, extern visible. | `#[test]` in `src/merged_view.rs` (or `workspace.rs` if tests live there). |
| Unit — semantic | Call before include diagnostic; call after include clean; included static variable unresolved. | `#[test]` in `src/semantic.rs` fixtures. |
| Integration — game folder | `semantic_pipeline_unresolved_count_within_threshold` reports < 1,000; top-10 `bo_*` counts drop ≥80%. | `tests/game_folder_parse.rs` with `AOMR_GAME_PATH`. |
| E2E — LSP roundtrip | Completion/hover/definition/references across include boundaries; missing include diagnostic. | `src/bin/lsp_roundtrip_test.rs`. |
| Manual | IntelliJ/Rider smoke test on `mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`. | Check cross-include completion, hover, go-to-definition. |

All 75 existing LSP unit tests must continue to pass with no regression.

## 11. Risk register

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| 1 | A lambda variant (e.g. `[]() -> bool { ... }` on `bo_system.xs:164`) still parses as `ERROR` after the grammar change. | Medium | High | Add ERROR-node recovery for function definitions with bodies; run the full `bo_system.xs` parse test before final acceptance. |
| 2 | Transitive-include cache invalidation misses a dependent open file when a deep include changes. | Medium | High | Store `IncludeGraph` in every cached `MergedView` and explicitly invalidate entries whose closure contains the changed path; add unit tests spanning overlay + vanilla cycles. |
| 3 | `textDocument/references` across included utility files produces duplicate results when the same file is included transitively through multiple paths. | Medium | Low | The graph is keyed by absolute path, so each file is visited at most once; add a roundtrip test with a shared utility file. |
| 4 | Per-keystroke latency exceeds 200 ms when merging 20+ files. | Medium | Medium | Cache keys on content hash; profile with `human_assist.xs`; warm per-file parse caches are reused. |
| 5 | Function-pointer type `void(int)` conflicts with `cast_expression` `(type) value` or `parenthesized_expression`. | Low | High | Add the conflict `[function_pointer_type, type_specifier]` and run the full game-folder parse test to confirm no new parse errors. |

## 12. Implementation order

Work-unit commit order. Each step is independently committable and verifiable with `cargo test`.

1. **Grammar** — add `function_pointer_type`, `lambda_expression`, regenerate parser, run existing tests.
2. **Symbols extraction** — handle `function_pointer_type` in `extract_params`; add `extract_error_function_definition`.
3. **Parser helper** — add `parser::extract_include_directives`.
4. **Workspace helper** — expose `workspace::game_relative_path`.
5. **New `merged_view.rs`** — `IncludeGraph`, `IncludeEdge`, `MergedView::build`, cycle detection, visibility filter, unit tests.
6. **Completion** — delete `included_files` regex and `path.ends_with`; route through `MergedView`.
7. **Semantic** — `check_forward_declarations_for_merged_view`, `check_mutable_redefinitions_for_merged_view`; leave `extern` collision project-wide.
8. **Typecheck** — `resolve_workspace_function` via `MergedView`.
9. **Diagnostics wiring** — pass `MergedView` through `collect_all`.
10. **Server cache** — add `merged_views`, `get_or_build_merged_view`, invalidate on `did_change_watched_files`.
11. **References** — expand to files in the merged closure; keep rename file-local.
12. **Roundtrip + integration tests** — add fixtures, game-folder threshold, top-10 regression guard.

### Suggested PR split

| PR | Steps | Rationale |
|---|---|---|
| 1 | 1–2 | Grammar + symbol extraction; self-contained, large generated diff. |
| 2 | 3–5 | Core merged-view infrastructure and unit tests. |
| 3 | 6–11 | Plumb merged view into LSP handlers and server state. |
| 4 | 12 | Tests, threshold changes, and verification notes. |

This keeps reviewable units under ~400 lines of human-written code each while still allowing the whole change to land on `xs-language-server/true-include-paste`.

---

## Inputs read for this design

- `openspec/changes/true-include-paste/proposal.md`
- `openspec/changes/true-include-paste/exploration.md`
- `openspec/changes/true-include-paste/specs/spec-true-include-paste.md`
- `openspec/changes/true-include-paste/specs/spec-semantic-diagnostics.md`
- `openspec/changes/true-include-paste/specs/spec-virtual-project-overlay.md`
- `openspec/changes/archive/xs-language-server/design.md`
- `tools/xs-language-server/src/workspace.rs`
- `tools/xs-language-server/src/symbols.rs`
- `tools/xs-language-server/src/parser.rs`
- `tools/xs-language-server/src/semantic.rs`
- `tools/xs-language-server/src/typecheck.rs`
- `tools/xs-language-server/src/completion.rs`
- `tools/xs-language-server/src/diagnostics.rs`
- `tools/xs-language-server/src/server.rs`
- `tools/xs-language-server/src/cache.rs`
- `tools/xs-language-server/src/references.rs`
- `tools/xs-language-server/tree-sitter-xs/grammar.js`
- `tools/xs-language-server/Cargo.toml`
- `tools/xs-language-server/src/lib.rs`
