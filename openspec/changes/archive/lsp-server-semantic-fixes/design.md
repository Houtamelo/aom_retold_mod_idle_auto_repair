# Design: lsp-server-semantic-fixes

This change eliminates the ~189 false-positive diagnostics the LSP server emits on the official AoM:R `game/**/*.xs` scripts. It updates `semantic.rs`, `typecheck.rs`, `diagnostics.rs`, and `symbols.rs` so that the LSP's view of a file matches the engine's link-unit view.

All six specs are satisfied:

- **A.0 / A.1** — duplicate-`extern` detection is scoped to the current file + its transitive `include` closure, and any remaining collision diagnostics are published on the declaration's own URI/range.
- **B** — callee resolution falls back to the already-loaded engine-API cache before emitting `Error 0310`.
- **C** — argument-count checks use a uniform default-arg rule: every non-`ref` parameter is optional at the call site because the XS compiler forces defaults at definition time, regardless of whether the source shows `= value`.
- **D** — `SymbolKind::Rule` symbols and dynamic rule registrations (`xsEnableRule`, `trRuleAdd`, `trRuleAddActive`, etc.) satisfy call expressions and bypass count/type checks.
- **E / F** — `tests/game_folder_parse.rs` is rewritten to run the full `diagnostics::collect_all` pipeline and assert zero diagnostics per category (and zero total).

## Architecture Decisions

### AD-1: Extern collision scope = current link unit

**Decision**: `check_extern_collisions` SHALL operate on the current file's merged-view closure (current file + every file reachable via `include` directives). Within that closure, duplicate `extern` declarations are allowed only when they lie on a single include chain (ancestor/descendant). Duplicate `extern` declarations in sibling included files, or an `extern` colliding with a non-`extern` definition, are flagged.

**Rejected alternatives**:
- Per-workspace scan (current behavior; produces false positives across unrelated files).
- Pure closure scan with no include-chain exception (would flag the valid main-file/header `extern` re-declaration pattern).

**Rationale**: matches the engine's textual-paste link-unit semantics and the spec scenarios.

### AD-2: Diagnostic URI/range source = declaration site

**Decision**: Duplicate-extern diagnostics SHALL be published on the declaration file's URI at the declaration symbol's range. The open file's `include` directive or comment lines SHALL NOT be used.

**Rejected alternatives**:
- Keep a flat `Vec<Diagnostic>` attached to the current file (the current bug).
- Use `related_information` on the current file as the primary location.

**Rationale**: LSP diagnostics are file-bounded. Correct attribution is required for go-to-definition and quick-fixes. Implementation returns a `DiagnosticsByUri` map; the server publishes one `publishDiagnostics` call per key.

### AD-3: Engine-API lookup pipeline

**Decision**: `semantic.rs` callee resolution SHALL consult, in order: (a) workspace symbols, (b) engine-API cache, (c) emit unresolved. Add `EngineApi::lookup(name) -> Option<&Syscall>` as the canonical semantic lookup.

**Rejected alternatives**:
- Pre-merge engine API into workspace symbol tables (would force cache invalidation on every settings change and inflate memory).
- Continue ignoring engine API in `semantic.rs`.

**Rationale**: Workspace symbols take precedence (they can shadow engine functions), engine data is already in memory, and `find_syscall` is the existing exact-match helper.

### AD-4: Rule symbol support + dynamic registrations

**Decision**: Preserve the existing `SymbolKind::Rule`. In addition, scan every file in the virtual project for rule-registration calls (`xsEnableRule`, `xsDisableRule`, `xsSetRuleMinInterval`, `xsSetRuleMaxInterval`, `xsRuleIgnoreIntervalOnce`, `trDelayedRuleActivation`, `trRuleAdd`, `trRuleAddActive`) and store the string-literal rule names in `VirtualProject::registered_rules: HashSet<String>`.

**Rejected alternatives**:
- Separate `RuleRegistry` struct (parallel data structure adds complexity).
- Rely only on `rule` definitions (misses rules enabled by string name).

**Rationale**: The engine treats rules as zero-parameter, void-return callables, and rule names are often registered by control calls rather than being defined in every consumer file.

### AD-5: Rule calls bypass parameter-count check

**Decision**: `typecheck.rs` SHALL treat any resolved `SymbolKind::Rule` callable as having no parameters and no return type: it emits no argument-count and no argument-type diagnostics. `semantic.rs` resolution SHALL accept both rule definitions and registered rules as callable.

**Rejected alternatives**:
- Try to infer rule parameter signatures from registration calls (rules have no declared parameters).
- Reject rule calls with arguments even though the engine tolerates them.

**Rationale**: matches spec D engine constraints.

### AD-6: Uniform default-arg rule (no source-based distinction)

- **Decision**: the call-site argument-count check SHALL be uniform across
  workspace and engine-API callees. Every non-`ref` parameter is optional
  (has a default value, compiler-enforced at definition time). Every
  `ref` parameter is required (and cannot have a default — the compiler
  rejects attempts). The LSP's `required_param_count` returns 0 for both
  source variants because it cannot yet reliably distinguish ref params
  end-to-end.
- **Rejected**: source-based distinction (workspace params without
  explicit `= value` are required). Wrong because the compiler forces
  defaults at definition time even when source omits them.
- **Rejected**: separate rule for engine API (treat all as optional) vs
  workspace (treat params without explicit defaults as required). Same
  reason.

### AD-7: Test harness expansion

**Decision**: Rewrite `tests/game_folder_parse.rs` to:
- Remove the engine-API-name filter.
- Call `diagnostics::collect_all` for each top-level file.
- Count diagnostics by stable `diagnostics.rs` categorization helpers.
- Assert `assert_eq!(0, count)` for duplicate-extern, unresolved-symbol, wrong-arg-count, rule-call-unresolved, wrong-range-uri, and total.

**Rejected alternative**: keep the filtered semantic-only test and add a separate test (the old test masks real bugs).

**Rationale**: The existing integration test reported "0 unresolved" only because it filtered engine names and skipped `typecheck.rs`.

### AD-8: Per-file diagnostic isolation

**Decision**: `diagnostics::collect_all` SHALL return diagnostics grouped by URI (`DiagnosticsByUri`). The server and the integration test publish or aggregate per URI. No diagnostic leaks into an unrelated file's publish set.

**Rejected alternative**: single flat `Vec<Diagnostic>` always published under the open file's URI.

**Rationale**: AD-2 requires diagnostics on declaration files; other checks are naturally scoped to the analyzed file.

### AD-9: Diagnostic categorization helpers

**Decision**: Add a `DiagnosticCategory` enum and `categorize(d: &Diagnostic) -> DiagnosticCategory` function in `diagnostics.rs`. `WrongRangeUri` is tracked by the integration test by cross-checking each diagnostic's range against its URI's source, not by emitting a new diagnostic.

**Rejected alternative**: string-matching diagnostic messages in the test.

**Rationale**: `tower_lsp::lsp_types::Diagnostic` is an external type; a wrapper struct would propagate through the whole server. Free functions keep the existing API unchanged and still give tests stable categories.

### AD-10: Commit + PR strategy

**Decision**: Deliver as a single PR with 5-6 work-unit commits, each leaving Rust tests green:

1. `test: expand game_folder_parse to full diagnostic pipeline` — add per-category counters (RED on current baseline).
2. `fix: resolve engine-API names in semantic callee checks` — fixes ~140 false positives.
3. `fix: scope duplicate-extern collisions to link unit and correct diagnostic URI` — fixes ~17 false positives.
4. `fix: default-aware argument count in typecheck` — fixes ~15 false positives.
5. `fix: allow rule symbols to satisfy call expressions` — fixes 2 false positives.
6. `chore: bump plugin version to 0.1.6`.

Each commit lands with the corresponding unit tests passing and the game-folder integration test still passing on the categories already fixed.

## Data structure changes

`SymbolKind` already contains the required variant; no change:

```rust
// tools/xs-language-server/src/symbols.rs
pub enum SymbolKind {
    Rule,
    Function,
    Variable,
    Constant,
}
```

New diagnostic grouping type:

```rust
// tools/xs-language-server/src/diagnostics.rs
pub type DiagnosticsByUri = HashMap<Url, Vec<Diagnostic>>;

pub enum DiagnosticCategory {
    ExternCollision,
    UnresolvedSymbol,
    WrongArgCount,
    WrongArgType,
    DefinitionError,
    WrongRangeUri,
    Other,
}

pub fn categorize(d: &Diagnostic) -> DiagnosticCategory;
pub fn is_duplicate_extern(d: &Diagnostic) -> bool;
pub fn is_unresolved_symbol(d: &Diagnostic) -> bool;
pub fn is_argument_mismatch(d: &Diagnostic) -> bool;
```

Engine-API lookup alias:

```rust
// tools/xs-language-server/src/engine_api.rs
pub type EngineSignature = Syscall;

impl EngineApi {
    pub fn lookup(&self, name: &str) -> Option<&EngineSignature> {
        self.find_syscall(name)
    }
}
```

Virtual project extended with registered rule names:

```rust
// tools/xs-language-server/src/semantic.rs
pub struct VirtualProject {
    pub files: HashMap<PathBuf, ParsedFile>,
    pub registered_rules: HashSet<String>,
}
```

Internal callee-source tag for typecheck:

```rust
// tools/xs-language-server/src/typecheck.rs
enum CalleeSource { Workspace, EngineApi }
```

The link-unit closure is taken from existing `MergedView`:

```rust
// tools/xs-language-server/src/merged_view.rs
impl MergedView {
    pub fn files(&self) -> impl Iterator<Item = &Path>; // own file + includes
}
```

## Function-level changes

### `semantic.rs::check_all`

Before:
```rust
pub fn check_all(project: &VirtualProject, current_file: &Path) -> Vec<Diagnostic>
```

After:
```rust
pub fn check_all(
    project: &VirtualProject,
    engine: &EngineApi,
    current_file: &Path,
) -> DiagnosticsByUri
```

It builds the `MergedView` first, then routes checks through per-URI aggregators.

### `semantic.rs::check_extern_collisions`

Before:
```rust
pub fn check_extern_collisions(project: &VirtualProject) -> Vec<Diagnostic>
```

After:
```rust
pub fn check_extern_collisions(
    project: &VirtualProject,
    current_file: &Path,
    merged: &MergedView,
) -> DiagnosticsByUri
```

It iterates only files in `merged.files()`, using each file's full `SymbolTable` so local definitions are visible. It emits one diagnostic per declaration under the declaration's URI.

### `semantic.rs::check_forward_declarations` and `check_forward_declarations_for_merged_view`

Both now accept `engine: &EngineApi`. After the workspace/merged forward-callable check fails, the functions return early if `engine.lookup(callee).is_some()`.

### `semantic.rs::forward_callable` / `forward_callable_merged`

They now accept rules and registered rules as callable:

```rust
matches!(s.kind, SymbolKind::Function | SymbolKind::Rule)
    || project.registered_rules.contains(name)
```

### `symbols.rs`

Add a new extractor for rule-registration calls:

```rust
pub fn extract_rule_registrations(tree: &Tree, source: &str) -> HashSet<String>
```

It walks top-level call expressions whose callee is one of the registration functions and reads the first string-literal argument.

### `typecheck.rs::check_one_call`

Before:
```rust
let params = target.params();
let expected_count = params.len();
if arg_count != expected_count { emit(...) }
```

After:
```rust
if resolved_symbol.kind == SymbolKind::Rule { return; }
let (params, source) = match target {
    Callee::Engine(s) => (&s.params, CalleeSource::EngineApi),
    Callee::Workspace(sym) => (&sym.params, CalleeSource::Workspace),
};
let required = required_count(params, source);
if arg_count > params.len() { emit("too many arguments"); }
else if arg_count < required { emit("missing required argument"); }
```

### `diagnostics.rs::collect_all`

Before:
```rust
pub fn collect_all(...) -> Vec<Diagnostic>
```

After:
```rust
pub fn collect_all(...) -> DiagnosticsByUri
```

It inserts parse/typecheck/missing-include diagnostics under the current file's URI and merges the per-URI map returned by semantic checks.

### `server.rs::publish_diagnostics`

After building `DiagnosticsByUri`, iterate entries and call `client.publish_diagnostics(uri, diags, Some(version))` for each.

## Pipeline diagram

```
File URI + content
  ↓
parser::parse                       → Tree
  ↓
server builds VirtualProject + MergedView  → link-unit closure
  ↓
symbols::extract_rule_registrations → registered_rules
  ↓
semantic::check_extern_collisions   → DiagnosticsByUri (declaration URIs)
semantic::check_forward_declarations → DiagnosticsByUri (current file URI)
  (workspace → engine.lookup(name)? → unresolved)
  ↓
typecheck::check_calls_with_merged  → current-file diagnostics
  (all params treated as optional at the call site)
  (rules: skip)
  (ref params: not yet tracked end-to-end, counting errs permissive)
  ↓
diagnostics::categorize             → DiagnosticCategory
  ↓
server publishes per-URI
```

## Test plan

| Spec | Test name | Assertion |
|------|-----------|-----------|
| A.0 | `test_extern_collision_across_unrelated_files` | no diagnostic for two files with same `extern` that are not in the same link unit |
| A.0 | `test_extern_collision_within_include_paste` | no diagnostic when current file + one include both declare same `extern` |
| A.0 | `test_extern_collision_sibling_includes` | diagnostic when two sibling includes declare same `extern` |
| A.1 | `test_diagnostic_uri_points_at_declaration` | diagnostic URI/range == declaration file and line |
| B | `test_engine_api_call_resolves` | no `Error 0310` for `xsSetContextPlayer(0)` |
| B | `test_workspace_shadows_engine_api` | workspace definition wins, no diagnostic |
| B | `test_unknown_call_unresolved` | `Error 0310` for `foobarBaz()` |
| C | `test_engine_api_call_omits_trailing_defaults` | no diagnostic for `aiPlanCreate(0, 0)` |
| C | `test_engine_api_call_too_many_args` | diagnostic for 7 args to 4-param syscall |
| C | `allows_omitting_workspace_arguments_without_explicit_default` | no diagnostic for `myFn(1)` even though source shows no explicit default |
| C | `test_workspace_call_omits_defaults` | no diagnostic for `void f(int x=-1,int y=-1) { }` called as `f(1)` |
| D | `test_rule_call_resolves` | no diagnostic for calling a defined rule |
| D | `test_rule_call_bypasses_arg_count` | no diagnostic for `r(1, 2)` on rule `r` |
| D | `test_registered_rule_call_resolves` | `xsEnableRule("foo")` makes `foo()` resolvable |
| F | `test_game_folder_zero_diagnostics_all_categories` | 7 category counts and total all `== 0` |

## Risks + mitigations

| Risk | Mitigation |
|------|------------|
| Engine-API extraction does not record `ref` parameters; treating all engine params as optional may miss real too-few-args errors for ref-only syscalls. | This is a deliberate conservative relaxation; false negatives are preferred over false positives. A future change can add `is_ref` to `engine_api::Param` once extraction supports it. |
| Per-URI diagnostic publishing touches server-side LSP plumbing. | Change is localized to `server.rs::publish_diagnostics`; existing behavior for non-extern diagnostics is preserved by placing them under the current file URI. |
| Rule-registration string scanning may false-positive on string arguments that are not rule names. | Only calls with callee names from the hard-coded registration-function list are scanned; this matches the spec's enumerated set. |
| Cross-spec ordering: A.0 and A.1 must land together to keep tests green. | They share one commit in AD-10. |
| Full `collect_all` over 149 top-level files may be slower than the old semantic-only scan. | Engine-API is loaded once; MergedView and project are built once per file. Profile after the first green run and cache if needed. |

## Out of scope

- Category E (syntax highlighting / TextMate bundle loading). This is IDE-side, not LSP-server.
- Re-extracting or correcting `doxygen_retail.7z` data. The cache already contains the needed symbols.
- Adding `is_ref` detection to the doxygen extractor (deferred until it is needed to remove the conservative engine-param relaxation).
- New completions, hovers, code actions, or workspace-wide references beyond diagnostic correctness.
- Mod scripts as the verification gate; the gate is the official `game/**/*.xs` tree.
