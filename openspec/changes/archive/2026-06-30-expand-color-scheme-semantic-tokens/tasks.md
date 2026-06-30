# Tasks: `expand-color-scheme-semantic-tokens`

## Review Workload Forecast

| Field | Value |
|-------|-------|
| Estimated changed lines | ~900–1100 (+750 / -50 in tracked files; ~200 in new tests across LSP + plugin) |
| New files | 5 (`tools/xs-language-server/tests/semantic_tokens_repro.rs`, `tools/xs-language-server/src/semantic_tokens.rs`, `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterTest.kt`, `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt`, `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/XsTextMateIncludeKeywordTest.kt`) |
| Files changed | ~17 LSP + plugin source, grammar, properties, docs |
| 400-line budget risk | **High** |
| Chained PRs recommended | **Yes** |
| Suggested split | **3 chained PRs** (see below) |
| Delivery strategy | `auto-chain` |
| Decision needed before apply | **Yes** — orchestrator should pause and ask the user whether to split into chained PRs or accept a `size:exception` single PR. |

### Suggested Work Units

| Unit | Goal | Likely PR | Notes |
|------|------|-----------|-------|
| 1 | LSP semantic-token pipeline (`semantic_tokens.rs`, `server.rs` capability/handler, `symbols.rs` local/static extraction, `lib.rs`, 6 integration tests) | PR 1 | Serverside change; tested with `cargo test`; ~400 new/changed lines. |
| 2 | Plugin grammar/lexer/highlighter fixes (3a/3b/3c): TextMate `include`, native lexer `include`, new token types/flex rules, `XsSyntaxHighlighter` mappings, identifier-under-caret documentation, +10 plugin tests | PR 2 | Self-contained UX bug-fix chunk; tested with `./gradlew :test`; ~250 lines. |
| 3 | Plugin semantic-token coloring (Bucket C): 8 new `TextAttributesKey`s, `XsColorSettingsPage` descriptors, `XsSemanticTokensConverter` + `XsSemanticTokensSupport`, `XsLspServerDescriptor` wiring, +8 plugin tests, version bump | PR 3 | Depends on PR 1 LSP legend; tested with `./gradlew :test`; ~350 lines. |

---

## Phase 0: Setup

### Task 0.1: Confirm branch state
**TDD Cycle:** Setup
- [x] Verify the working tree is on a branch dedicated to `expand-color-scheme-semantic-tokens` and clean (`git status --short`).
- [ ] Confirm parent `HEAD` contains resolved Issues 1, 2, and 4 and the partial Issue 3 (`expand-color-scheme-categories`) so no unrelated work remains.
- **Verification:** `git status` shows only expected files; `git log --oneline -5` shows recent Issue #3 and Issue #4 SDD commits.

### Task 0.2: Confirm LSP build clean (220 tests baseline)
**TDD Cycle:** Setup
- [x] Build LSP crate with tests: `cargo build --manifest-path tools/xs-language-server/Cargo.toml --tests`.
- [ ] Record baseline test count.
- **Verification:**
  - Build exits `0`.
  - `cargo test --manifest-path tools/xs-language-server/Cargo.toml -- --list | grep -c ': test'` reports **220** existing tests.
  - Do not fix pre-existing warnings (e.g., unused `game_root`, `DID_CHANGE` const). Document the baseline.

### Task 0.3: Confirm plugin builds and tests pass (68 baseline)
**TDD Cycle:** Setup
- [x] Compile plugin and tests: `./gradlew :compileKotlin :compileTestKotlin`.
- [ ] Run plugin tests: `./gradlew :test`.
- **Verification:**
  - Compile exits `0`.
  - Existing 68 plugin tests pass (any environment-specific failure must be documented as pre-existing).

### Task 0.4: Note current `pluginVersion`
**TDD Cycle:** Setup
- [x] Read `tools/intellij-xs-plugin/gradle.properties`.
- **Verification:** `grep '^pluginVersion' tools/intellij-xs-plugin/gradle.properties` returns `0.4.0`.

---

## Phase 1: RED — Failing tests first

### Task 1.1: Write the 3a TextMate-grammar test
**TDD Cycle:** RED
- [x] Create `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/XsTextMateIncludeKeywordTest.kt`.
- [ ] Add `test_include_keyword_in_textmate_grammar`:
  - Load `syntaxes/xs.tmLanguage.json` from the test classpath.
  - Find the keyword pattern that matches `break|continue|else|for|if|return|while|do`.
  - Assert that the pattern also matches `include` (e.g. `assertTrue(pattern.contains("include"))`).
- [ ] Also add `test_native_lexer_recognizes_include_as_keyword`:
  - Instantiate `XsHighlightingLexer()`.
  - Start it on `"include"`.
  - Assert `tokenType == XsHighlightingTokenTypes.KEYWORD`.
- **Verification:** `./gradlew :compileTestKotlin` may compile (second assertion uses existing types). The first assertion must FAIL because `include` is absent from the grammar regex.

### Task 1.2: Run the 3a tests and confirm FAIL
**TDD Cycle:** RED confirmation
- [x] Run `./gradlew :test --tests "com.aomr.xs.textmate.XsTextMateIncludeKeywordTest"`.
- **Verification:** At least `test_include_keyword_in_textmate_grammar` FAILS (the native-lexer assertion may also fail if `XS_KEYWORDS` lacks `include`).

### Task 1.3: Write the 3b highlighter-mapping tests
**TDD Cycle:** RED
- [x] Create `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterTest.kt`.
- [ ] Add 8 tests, one per category. Each creates an `XsSyntaxHighlighter`, starts its lexer on a single token, reads `tokenType`, and asserts `getTokenHighlights(tokenType)` contains the expected key:
  - `test_braces_map_to_xs_braces` — `{` or `}` → `XsTextAttributes.BRACES`
  - `test_brackets_map_to_xs_brackets` — `[` or `]` → `XsTextAttributes.BRACKETS`
  - `test_comma_maps_to_xs_comma` — `,` → `XsTextAttributes.COMMA`
  - `test_dot_maps_to_xs_dot` — `.` → `XsTextAttributes.DOT`
  - `test_operation_sign_maps_to_xs_operation_sign` — `=` or `+` → `XsTextAttributes.OPERATION_SIGN`
  - `test_overloaded_operator_maps_to_xs_overloaded_operator` — same lexer token as `OPERATION_SIGN`, but assert `OVERLOADED_OPERATOR` key is returned alongside or instead as decided in the implementation (per design, `OVERLOADED_OPERATOR` shares the same lexer token and maps to `XS_OVERLOADED_OPERATOR`)
  - `test_parentheses_map_to_xs_parentheses` — `(` or `)` → `XsTextAttributes.PARENTHESES`
  - `test_semicolon_maps_to_xs_semicolon` — `;` → `XsTextAttributes.SEMI_COLON`
- **Note:** `XsTokenTypes.DOT`, `SEMI_COLON`, and `OPERATION_SIGN` do not exist yet. Three of the tests will **not compile** until the token types are added in Task 3.3.
- **Verification:** File created with the expected test skeleton.

### Task 1.4: Run the 3b tests and confirm FAIL
**TDD Cycle:** RED confirmation
- [x] Run `./gradlew :compileTestKotlin` and `./gradlew :test --tests "com.aomr.xs.highlight.XsSyntaxHighlighterTest"`.
- **Verification:**
  - Compile fails for `DOT`, `SEMI_COLON`, `OPERATION_SIGN` (expected RED).
  - For the five existing token types, tests FAIL at runtime because `XsSyntaxHighlighter` currently maps them to `XS_DEFAULT`.
- [ ] Record the RED evidence (failure names and counts) for the verify report.

### Task 1.5: Write the 3c identifier-under-caret test
**TDD Cycle:** RED
- [x] Create (or extend `XsColorSettingsPageTest.kt` with) a test that asserts the limitation is documented:
  - `test_identifier_under_caret_descriptor_documents_global_setting`
  - Load `XsColorSettingsPage.getAttributeDescriptors()`.
  - Find the descriptor whose display name contains `"Identifier under caret"`.
  - Assert its display name also contains `"uses global General"` (or similar rider limitation note).
- **Verification:** `./gradlew :compileTestKotlin` compiles; test FAILS because the current label is plain `"Code//Identifier under caret"`.

### Task 1.6: Run the 3c test and confirm FAIL
**TDD Cycle:** RED confirmation
- [x] Run `./gradlew :test --tests "com.aomr.xs.highlight.XsColorSettingsPageTest"` (or dedicated test class).
- **Verification:** The new assertion FAILS.

### Task 1.7: Write the Bucket C LSP semantic-token tests
**TDD Cycle:** RED
- [x] Create `tools/xs-language-server/tests/semantic_tokens_repro.rs`.
- [ ] Reuse the `xs_language_server::semantic_tokens` API (not yet created). Define tests against the public shape `semantic_tokens::compute_tokens(...)` and a `Token` struct with `line`, `char`, `len`, `token_type`, `modifiers`.
- [ ] Add the 6 tests:
  - `test_semantic_token_legend_advertised`
    - Import `xs_language_server::semantic_tokens::server_capabilities`.
    - Assert the returned `SemanticTokensServerCapabilities::SemanticTokensOptions` legend contains `function`, `variable`, `type` and the modifiers `engine`, `modded`, `unmodded`, `local`, `static`.
  - `test_engine_function_emits_engine_modifier`
    - Build a minimal `EngineApi` containing a syscall named `engineFn`.
    - Source: `void f() { engineFn(); }`.
    - Assert `compute_tokens` contains a token for `engineFn` with `token_type == SemanticTokenType::FUNCTION` and `"engine"` in modifiers.
  - `test_modded_function_emits_modded_modifier`
    - Build a `Workspace` with a mod overlay; point `current_file` at an overlay path.
    - Source: `void modFn() {} void caller() { modFn(); }`.
    - Assert caller’s `modFn` token has `function` + `"modded"`.
  - `test_unmodded_function_emits_unmodded_modifier`
    - Build a `Workspace` with only the vanilla game folder; point `current_file` at a vanilla path.
    - Source: `void vanillaFn() {} void caller() { vanillaFn(); }`.
    - Assert `function` + `"unmodded"`.
  - `test_local_variable_emits_local_modifier`
    - Source: `void foo() { int localVar = 1; localVar = 2; }`.
    - Assert a `variable` token for `localVar` with `"local"` modifier.
  - `test_builtin_type_emits_engine_modifier`
    - Source: `void foo() { int x = 1; }`.
    - Assert a `type` token for `int` with `"engine"` modifier.
- **Verification:** `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test semantic_tokens_repro --no-run` fails to compile because `semantic_tokens` module does not exist yet. Record this as RED.

### Task 1.8: Run the LSP tests and confirm FAIL
**TDD Cycle:** RED confirmation
- [x] Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test semantic_tokens_repro -- --nocapture`.
- **Verification:** Tests do not compile / all FAIL because `semantic_tokens` module, capability, and handler are missing.
- [ ] Save RED evidence for the verify report.

### Task 1.9: Write the Bucket C plugin converter tests
**TDD Cycle:** RED
- [x] Create `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt`.
- [ ] Add 8 tests, one per `(tokenType, modifiers)` combination expected from the LSP:
  - `function` + `["engine"]` → `XsTextAttributes.FUNCTION_ENGINE`
  - `function` + `["unmodded"]` → `XsTextAttributes.FUNCTION_UNMODDED`
  - `function` + `["modded"]` → `XsTextAttributes.FUNCTION_MODDED`
  - `variable` + `["local"]` → `XsTextAttributes.VARIABLE_LOCAL`
  - `variable` + `["static"]` → `XsTextAttributes.VARIABLE_STATIC`
  - `type` + `["engine"]` → `XsTextAttributes.TYPE_BUILTIN`
  - `type` + `["unmodded"]` → `XsTextAttributes.TYPE_UNMODDED_CLASS`
  - `type` + `["modded"]` → `XsTextAttributes.TYPE_MODDED_CLASS`
- **Verification:** File created; references `XsSemanticTokensConverter` and 8 new keys that do not exist yet.

### Task 1.10: Run the converter tests and confirm FAIL
**TDD Cycle:** RED confirmation
- [x] Run `./gradlew :compileTestKotlin` and `./gradlew :test --tests "com.aomr.xs.lsp.XsSemanticTokensConverterTest"`.
- **Verification:** Compile fails (no `XsSemanticTokensConverter`, no new `TextAttributesKey`s). Record as RED.

---

## Phase 2: GREEN — LSP minimal semantic-token pipeline

### Task 2.1: Add `semantic_tokens` module and capability
**TDD Cycle:** GREEN
- [x] Create `tools/xs-language-server/src/semantic_tokens.rs`.
- [ ] Add the legend and capability helper:
  ```rust
  use tower_lsp::lsp_types::{
      SemanticTokensFullOptions, SemanticTokensLegend, SemanticTokensOptions,
      SemanticTokensServerCapabilities, SemanticTokenType, SemanticTokenModifier,
  };

  const TOKEN_TYPES: &[SemanticTokenType] = &[
      SemanticTokenType::FUNCTION,
      SemanticTokenType::VARIABLE,
      SemanticTokenType::TYPE,
  ];

  const TOKEN_MODIFIERS: &[SemanticTokenModifier] = &[
      SemanticTokenModifier::new("engine"),
      SemanticTokenModifier::new("modded"),
      SemanticTokenModifier::new("unmodded"),
      SemanticTokenModifier::new("local"),
      SemanticTokenModifier::new("static"),
  ];

  pub fn server_capabilities() -> SemanticTokensServerCapabilities { ... }
  ```
- [x] Add `pub mod semantic_tokens;` to `tools/xs-language-server/src/lib.rs`.
- [x] Import `semantic_tokens` in `tools/xs-language-server/src/server.rs`.
- [x] In `initialize` capabilities, set `semantic_tokens_provider: Some(semantic_tokens::server_capabilities())`.
- **Verification:** `cargo build --manifest-path tools/xs-language-server/Cargo.toml` succeeds; `test_semantic_token_legend_advertised` from Task 1.7 now PASSes.

### Task 2.2: Define internal token type, `compute_tokens`, and `encode`
**TDD Cycle:** GREEN
- [x] In `semantic_tokens.rs`, define:
  ```rust
  #[derive(Debug, Clone, PartialEq)]
  pub struct Token {
      pub line: u32,
      pub char: u32,
      pub len: u32,
      pub token_type: SemanticTokenType,
      pub modifiers: Vec<SemanticTokenModifier>,
  }

  pub fn compute_tokens(
      source: &str,
      current_file: Option<&std::path::Path>,
      own_table: &symbols::SymbolTable,
      merged: Option<&merged_view::MergedView>,
      engine: &engine_api::SharedEngineApi,
      workspace: &workspace::Workspace,
      project: &workspace::VirtualProject,
  ) -> Vec<Token> { ... }

  pub fn encode(tokens: &[Token]) -> Vec<u32> { ... }
  ```
- [ ] `encode` must produce LSP delta encoding using the module’s `TOKEN_TYPES` and `TOKEN_MODIFIERS` indices and bitmasks.
- **Verification:** Module compiles (return `Vec::new()` as a stub if needed; tests will fail until extraction is implemented).

### Task 2.3: Add `extract_symbols` helper
**TDD Cycle:** GREEN
- [x] In `semantic_tokens.rs`, implement a private helper that walks the AST and yields `(name: String, kind: SymbolKind, range: Range, is_declaration: bool)` for every identifier node.
- [ ] Distinguish declarations (function name, parameter name, variable declarator) from uses.
- **Verification:** Add a `#[cfg(test)]` unit test in `semantic_tokens.rs` that calls the helper on a small source and verifies declarations are flagged.

### Task 2.4: Add `classify_origin`
**TDD Cycle:** GREEN
- [x] In `semantic_tokens.rs`, implement:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  enum Origin { Engine, Modded, Unmodded }

  fn classify_origin(
      file: &std::path::Path,
      workspace: &workspace::Workspace,
      project: &workspace::VirtualProject,
  ) -> Origin { ... }
  ```
- [ ] Rules per design:
  1. If `file` is a value in `project.file_overrides` → `Origin::Modded`.
  2. Else if `workspace.game_relative_path(file)` is `Some` → `Origin::Unmodded`.
  3. Else → `Origin::Engine`.
- **Verification:** Add unit tests covering overlay, vanilla game, and outside paths.

### Task 2.5: Add `classify_modifiers`
**TDD Cycle:** GREEN
- [x] In `semantic_tokens.rs`, implement:
  ```rust
  fn classify_modifiers(
      symbol: &symbols::Symbol,
      origin: Origin,
  ) -> Vec<SemanticTokenModifier> { ... }
  ```
- [ ] Output:
  - For functions: origin modifier (`engine`, `modded`, `unmodded`).
  - For variables: origin modifier plus `local` if `visibility == Visibility::Local` and not `static`, or `static` if the declaration carries `static`.
  - For top-level extern/static variables, the same rule applies.
- **Verification:** Unit-test the modifier vector directly.

### Task 2.6: Add the `semantic_tokens_full` handler
**TDD Cycle:** GREEN
- [x] In `server.rs`, add the handler inside `#[tower_lsp::async_trait] impl LanguageServer for XsLanguageServer`:
  ```rust
  async fn semantic_tokens_full(
      &self,
      params: SemanticTokensParams,
  ) -> Result<Option<SemanticTokens>> {
      let uri = params.text_document.uri;
      let text = { self.documents.lock().await.get(&uri).unwrap_or("").to_string() };
      let current_file = uri.to_file_path().ok();
      let merged = match current_file {
          Some(_) => self.get_or_build_merged_view(&uri, &text).await,
          None => None,
      };
      let own_table = {
          let tables = self.symbol_tables.lock().await;
          tables.get(&uri).cloned().unwrap_or_default()
      };
      let (project, ws) = {
          let ws = self.workspace.lock().await;
          let project = ws.lookup_mod(&uri)
              .map(|e| ws.build_virtual_project(e))
              .unwrap_or_default();
          (project, ws.clone())
      };
      let tokens = semantic_tokens::compute_tokens(
          &text,
          current_file.as_deref(),
          &own_table,
          merged.as_ref(),
          &self.engine,
          &ws,
          &project,
      );
      Ok(Some(SemanticTokens {
          result_id: None,
          data: semantic_tokens::encode(&tokens),
      }))
  }
  ```
- **Verification:** `cargo build` succeeds.

### Task 2.7: Add local-variable extraction to the symbol table
**TDD Cycle:** GREEN
- [x] In `tools/xs-language-server/src/symbols.rs`:
  - Add `pub is_static: bool` to `Symbol` and default it to `false`.
  - Set `is_static: modifiers.is_static` in `extract_function`, `extract_declaration`, `extract_forward_declaration`, `extract_error_*`.
  - Implement `pub fn build_full_symbol_table(tree: &Tree, source: &str) -> SymbolTable` that calls `build_symbol_table` then `extract_local_declarations(tree.root_node(), source, &mut table.symbols)`.
  - Implement `extract_local_declarations` to walk `compound_statement` bodies and capture `declaration` children whose `init_declarator` introduces an identifier; create `Symbol { kind: Variable, visibility: Local, is_static: modifiers.is_static, ... }`.
  - Update `server.rs` `rebuild_symbol_table` to call `build_full_symbol_table` instead of `build_symbol_table`.
- **Verification:** Unit-test in `symbols.rs` or `semantic_tokens_repro.rs` that a local variable inside a function is found.

### Task 2.8: Run the 6 LSP semantic-token tests and confirm PASS
**TDD Cycle:** GREEN confirmation
- [x] Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --test semantic_tokens_repro -- --nocapture`.
- **Verification:** All 6 tests PASS.

### Task 2.9: Run the full LSP test suite
**TDD Cycle:** GREEN confirmation
- [x] Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast`.
- **Verification:** Count is **226** (220 existing + 6 new), all green.

---

## Phase 3: GREEN — Plugin 3a (`include` keyword) and 3b (braces/operators)

### Task 3.1: Add `include` to the TextMate grammar and native keyword set
**TDD Cycle:** GREEN
- [ ] In `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json` line 16, change the keyword regex from:
  ```json
  "\\b(break|continue|else|for|if|return|while|do)\\b"
  ```
  to:
  ```json
  "\\b(include|break|continue|else|for|if|return|while|do)\\b"
  ```
- [ ] In `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt`, add `"include"` to `XS_KEYWORDS`.
- **Verification:** `./gradlew :test --tests "com.aomr.xs.textmate.XsTextMateIncludeKeywordTest"` PASSes.

### Task 3.2: Add missing lexer token types
**TDD Cycle:** GREEN
- [ ] In `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsTokenTypes.kt`, add:
  ```kotlin
  @JvmField val DOT = IElementType("XS_DOT", XsLanguage.INSTANCE)
  @JvmField val SEMI_COLON = IElementType("XS_SEMI_COLON", XsLanguage.INSTANCE)
  @JvmField val OPERATION_SIGN = IElementType("XS_OPERATION_SIGN", XsLanguage.INSTANCE)
  ```
- [ ] In `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsLexer.flex`, add rules:
  ```flex
  "."               { return XsTokenTypes.DOT; }
  ";"               { return XsTokenTypes.SEMI_COLON; }
  "=" | "+" | "-" | "*" | "/" | "%" | "<" | ">" | "!" | "~" | "&" | "|" | "^" { return XsTokenTypes.OPERATION_SIGN; }
  ```
- **Verification:** `./gradlew :compileKotlin` succeeds; `./gradlew :compileTestKotlin` succeeds (the 3b test file now compiles).

### Task 3.3: Map all 8 lexer token categories in `XsSyntaxHighlighter`
**TDD Cycle:** GREEN
- [ ] In `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt`, extend `getTokenHighlights`:
  ```kotlin
  XsTokenTypes.LBRACE, XsTokenTypes.RBRACE -> arrayOf(XsTextAttributes.BRACES)
  XsTokenTypes.LBRACKET, XsTokenTypes.RBRACKET -> arrayOf(XsTextAttributes.BRACKETS)
  XsTokenTypes.LPAREN, XsTokenTypes.RPAREN -> arrayOf(XsTextAttributes.PARENTHESES)
  XsTokenTypes.COMMA -> arrayOf(XsTextAttributes.COMMA)
  XsTokenTypes.DOT -> arrayOf(XsTextAttributes.DOT)
  XsTokenTypes.SEMI_COLON -> arrayOf(XsTextAttributes.SEMI_COLON)
  XsTokenTypes.OPERATION_SIGN -> arrayOf(XsTextAttributes.OPERATION_SIGN, XsTextAttributes.OVERLOADED_OPERATOR)
  ```
- **Decision:** `OVERLOADED_OPERATOR` shares the `OPERATION_SIGN` lexer token in this minimal implementation, so both keys are returned. Document this choice in a task note.
- **Verification:** `./gradlew :test --tests "com.aomr.xs.highlight.XsSyntaxHighlighterTest"` all 8 PASS.

### Task 3.4: Run existing plugin highlight tests and all plugin tests
**TDD Cycle:** GREEN confirmation
- [ ] Run `./gradlew :test`.
- **Verification:** Existing `XsSyntaxHighlighterFactoryTest` and `XsColorSettingsPageTest` still pass; no regressions.

---

## Phase 4: GREEN — Plugin 3c (identifier under caret)

### Task 4.1: Investigate per-language identifier-under-caret behavior
**TDD Cycle:** GREEN
- [ ] Inspect the bundled 2024.2 platform sources for `IdentifierHighlighterPass`.
- [ ] Confirm that `IdentifierHighlighterPass` uses the global `EditorColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES` or `CodeInsightColors.IDENTIFIER_UNDER_CARET_ATTRIBUTES` directly and ignores the per-language key.
- [ ] Run a quick headless test or manual experiment in Rider if possible.
- **Verification:** Document the finding (platform limitation) in the task notes.

### Task 4.2: Document the limitation in the color-scheme page
**TDD Cycle:** GREEN
- [ ] In `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt`, update the KDoc for `IDENTIFIER_UNDER_CARET` to note the Rider/IntelliJ platform limitation.
- [ ] In `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt`, change the descriptor label to:
  ```kotlin
  AttributesDescriptor(
      "Code//Identifier under caret (uses global General → Identifier under caret)",
      XsTextAttributes.IDENTIFIER_UNDER_CARET
  )
  ```
- **Verification:** `./gradlew :test --tests "com.aomr.xs.highlight.XsColorSettingsPageTest"` PASSes (the 3c assertion now matches).

### Task 4.3: Run the 3c test and confirm PASS
**TDD Cycle:** GREEN confirmation
- [ ] Run `./gradlew :test --tests "com.aomr.xs.textmate.XsTextMateIncludeKeywordTest"` and the 3c test.
- **Verification:** PASS.

---

## Phase 5: GREEN — Plugin Bucket C (semantic-token color mapping)

### Task 5.1: Add 8 semantic `TextAttributesKey`s
**TDD Cycle:** GREEN
- [ ] In `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt`, add:
  ```kotlin
  val FUNCTION_ENGINE = createTextAttributesKey("XS_FUNCTION_ENGINE", DefaultLanguageHighlighterColors.FUNCTION_CALL)
  val FUNCTION_UNMODDED = createTextAttributesKey("XS_FUNCTION_UNMODDED", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)
  val FUNCTION_MODDED = createTextAttributesKey("XS_FUNCTION_MODDED", DefaultLanguageHighlighterColors.FUNCTION_DECLARATION)
  val VARIABLE_LOCAL = createTextAttributesKey("XS_VARIABLE_LOCAL", DefaultLanguageHighlighterColors.LOCAL_VARIABLE)
  val VARIABLE_STATIC = createTextAttributesKey("XS_VARIABLE_STATIC", DefaultLanguageHighlighterColors.STATIC_FIELD)
  val TYPE_BUILTIN = createTextAttributesKey("XS_TYPE_BUILTIN", DefaultLanguageHighlighterColors.KEYWORD)
  val TYPE_UNMODDED_CLASS = createTextAttributesKey("XS_TYPE_UNMODDED_CLASS", DefaultLanguageHighlighterColors.CLASS_NAME)
  val TYPE_MODDED_CLASS = createTextAttributesKey("XS_TYPE_MODDED_CLASS", DefaultLanguageHighlighterColors.CLASS_NAME)
  ```
- **Verification:** `./gradlew :compileKotlin` succeeds.

### Task 5.2: Register the 8 descriptors in `XsColorSettingsPage`
**TDD Cycle:** GREEN
- [ ] In `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt`, add after the Braces and Operators block:
  ```kotlin
  AttributesDescriptor("Identifier//Function//Engine function", XsTextAttributes.FUNCTION_ENGINE),
  AttributesDescriptor("Identifier//Function//UnModded function", XsTextAttributes.FUNCTION_UNMODDED),
  AttributesDescriptor("Identifier//Function//Modded function", XsTextAttributes.FUNCTION_MODDED),
  AttributesDescriptor("Identifier//Variable//Local variable", XsTextAttributes.VARIABLE_LOCAL),
  AttributesDescriptor("Identifier//Variable//Static variable", XsTextAttributes.VARIABLE_STATIC),
  AttributesDescriptor("Identifier//Type//Built-in type", XsTextAttributes.TYPE_BUILTIN),
  AttributesDescriptor("Identifier//Type//UnModded class", XsTextAttributes.TYPE_UNMODDED_CLASS),
  AttributesDescriptor("Identifier//Type//Modded class", XsTextAttributes.TYPE_MODDED_CLASS),
  ```
- [ ] Optionally extend `DEMO_TEXT` and `TAG_MAP` to preview one engine function, one local variable, and one built-in type.
- **Verification:** Extend `XsColorSettingsPageTest.test_attribute_descriptors_contain_*` or add a new test; it PASSes.

### Task 5.3: Inspect the platform `LspSemanticTokensSupport` API
**TDD Cycle:** GREEN
- [ ] Search the bundled IntelliJ Platform 2024.2 sources for `LspSemanticTokensSupport`.
- [ ] Determine whether the customization point is:
  - an abstract `com.intellij.platform.lsp.api.customization.LspSemanticTokensSupport` class, or
  - an extension point such as `com.intellij.platform.lsp.semanticTokensCustomizer`.
- [ ] Record the exact override signature (e.g., `fun getTextAttributesKey(tokenType: String, modifiers: List<String>): TextAttributesKey?`).

### Task 5.4: Create `XsSemanticTokensConverter` and `XsSemanticTokensSupport`
**TDD Cycle:** GREEN
- [ ] Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverter.kt`:
  ```kotlin
  object XsSemanticTokensConverter {
      fun convert(tokenType: String, modifiers: List<String>): TextAttributesKey = when {
          tokenType == "function" && "modded" in modifiers -> XsTextAttributes.FUNCTION_MODDED
          tokenType == "function" && "unmodded" in modifiers -> XsTextAttributes.FUNCTION_UNMODDED
          tokenType == "function" && "engine" in modifiers -> XsTextAttributes.FUNCTION_ENGINE
          tokenType == "variable" && "static" in modifiers -> XsTextAttributes.VARIABLE_STATIC
          tokenType == "variable" && "local" in modifiers -> XsTextAttributes.VARIABLE_LOCAL
          tokenType == "type" && "engine" in modifiers -> XsTextAttributes.TYPE_BUILTIN
          tokenType == "type" && "modded" in modifiers -> XsTextAttributes.TYPE_MODDED_CLASS
          tokenType == "type" && "unmodded" in modifiers -> XsTextAttributes.TYPE_UNMODDED_CLASS
          else -> XsTextAttributesKeys.XS_DEFAULT
      }
  }
  ```
- [ ] Create `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt` as a thin wrapper/override that delegates to `XsSemanticTokensConverter.convert`.
- [ ] If the platform requires a `plugin.xml` extension, register it in `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`.

### Task 5.5: Wire the converter through `XsLspServerDescriptor`
**TDD Cycle:** GREEN
- [ ] In `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt`, override `lspCustomization`:
  ```kotlin
  override val lspCustomization = object : LspCustomization() {
      override val semanticTokensCustomizer = XsSemanticTokensSupport()
  }
  ```
- [ ] Import the platform `LspCustomization` class and adjust the exact property/method name based on Task 5.3 findings.
- **Verification:** `./gradlew :compileKotlin` succeeds.

### Task 5.6: Run the 8 converter tests and confirm PASS
**TDD Cycle:** GREEN confirmation
- [ ] Run `./gradlew :test --tests "com.aomr.xs.lsp.XsSemanticTokensConverterTest"`.
- **Verification:** All 8 PASS.

### Task 5.7: Run all existing plugin tests
**TDD Cycle:** GREEN confirmation
- [ ] Run `./gradlew :test`.
- **Verification:** All plugin tests pass (expected count: 86 after new tests are added; any environment failure is documented).

---

## Phase 6: REFACTOR — Documentation

### Task 6.1: Add KDoc to the 8 new keys in `XsTextAttributes.kt`
**TDD Cycle:** REFACTOR
- [ ] For each new key, add a one-line KDoc explaining its semantic meaning and default platform fallback.
- **Verification:** `./gradlew :compileKotlin` succeeds; no new warnings.

### Task 6.2: Add KDoc to `XsSemanticTokensConverter.kt`
**TDD Cycle:** REFACTOR
- [ ] Add a file-level KDoc explaining the LSP legend contract and the precedence of modifiers.
- **Verification:** `./gradlew :compileKotlin` succeeds.

### Task 6.3: Add KDoc to the LSP `semantic_tokens_full` handler
**TDD Cycle:** REFACTOR
- [ ] Add a rustdoc comment or inline comment in `server.rs` above `semantic_tokens_full` explaining the data flow and the role of `current_file`, `merged`, and `own_table`.
- **Verification:** `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml --all-targets` shows no new warnings attributable to this handler.

---

## Phase 7: VERIFY

### Task 7.1: Run `cargo test --no-fail-fast`
**TDD Cycle:** VERIFY
- [ ] Run `cargo test --manifest-path tools/xs-language-server/Cargo.toml --no-fail-fast`.
- **Verification:** All 226+ tests pass.

### Task 7.2: Run `cargo clippy --all-targets`
**TDD Cycle:** VERIFY
- [ ] Run `cargo clippy --manifest-path tools/xs-language-server/Cargo.toml --all-targets`.
- **Verification:** Exits `0`; new warnings are not allowed. Pre-existing warnings must match the baseline from Task 0.2.

### Task 7.3: Run `./gradlew :test`
**TDD Cycle:** VERIFY
- [ ] Run `./gradlew :test`.
- **Verification:** Green except for any pre-existing environment failure documented in Phase 0.

### Task 7.4: Bump `pluginVersion` 0.4.0 → 0.5.0
**TDD Cycle:** VERIFY
- [ ] Edit `tools/intellij-xs-plugin/gradle.properties`:
  ```properties
  pluginVersion = 0.5.0
  ```
- **Rationale:** MINOR bump per `AGENTS.md` (new LSP feature + new settings UI + new grammar scope).
- **Verification:** `grep '^pluginVersion' tools/intellij-xs-plugin/gradle.properties` returns `0.5.0`.

### Task 7.5: Run `./gradlew :buildPlugin` and confirm `.zip` artifact
**TDD Cycle:** VERIFY
- [ ] Run `./gradlew :buildPlugin`.
- **Verification:** Succeeds and produces `tools/intellij-xs-plugin/build/distributions/intellij-xs-plugin-0.5.0.zip`.

### Task 7.6: Write the verify report
**TDD Cycle:** VERIFY documentation
- [ ] Create `openspec/changes/expand-color-scheme-semantic-tokens/verify-report.md`.
- [ ] Include:
  - Summary of changes (3a/3b/3c + Bucket C).
  - Test results: `cargo test` count and pass/fail, `./gradlew :test` count and pass/fail.
  - Clippy warning status.
  - `pluginVersion` after bump.
  - Deviations from design (expected: none).
  - Manual smoke-test notes (Rider verification of `include`, braces/operators, semantic function colors).
- **Verification:** Report file exists and data matches actual verification outputs.

---

## Phase 8: DOCUMENT

### Task 8.1: Update `docs/issues/2026-06-29-runtime-issues.md` Issue 3
**TDD Cycle:** DOCUMENT
- [ ] Open `docs/issues/2026-06-29-runtime-issues.md`.
- [ ] Near the Issue 3 status line, append:
  ```
  **Resolved 2026-06-30 (sub-bugs 3a/3b/3c + Bucket C partial)** by `openspec/changes/expand-color-scheme-semantic-tokens/`.
  Includes keyword `include` highlighting, braces/operators mapping, documented identifier-under-caret limitation, and engine/modded/unmodded semantic-token categories for functions, variables, and types.
  ```
- **Verification:** `grep -A4 'Issue 3' docs/issues/2026-06-29-runtime-issues.md` shows the updated resolved status.

### Task 8.2: Update `AGENTS.md` test count line
**TDD Cycle:** DOCUMENT
- [ ] Open `AGENTS.md`.
- [ ] Update the plugin/LSP test count paragraph (around line 121) to:
  ```
  cargo test for unit tests (226 tests as of 2026-06-30...). Plugin tests: 86 (...
  ```
- **Verification:** `grep -E '226 tests|86 \(.*\) plugin tests' AGENTS.md` matches.

---

## Phase 9: COMMIT

### Task 9.1: Stage only intended files
**TDD Cycle:** COMMIT
- [ ] Add the exact set of files expected:
  - `tools/xs-language-server/src/server.rs`
  - `tools/xs-language-server/src/lib.rs`
  - `tools/xs-language-server/src/symbols.rs`
  - `tools/xs-language-server/src/semantic_tokens.rs` (new)
  - `tools/xs-language-server/tests/semantic_tokens_repro.rs` (new)
  - `tools/intellij-xs-plugin/src/main/resources/syntaxes/xs.tmLanguage.json`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsTokenTypes.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/psi/XsLexer.flex`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsHighlightingLexer.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighter.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsTextAttributes.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/highlight/XsColorSettingsPage.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsLspServerDescriptor.kt`
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensSupport.kt` (new)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverter.kt` (new)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/textmate/XsTextMateIncludeKeywordTest.kt` (new)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/highlight/XsSyntaxHighlighterTest.kt` (new)
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/lsp/XsSemanticTokensConverterTest.kt` (new)
  - `tools/intellij-xs-plugin/gradle.properties`
  - `AGENTS.md`
  - `docs/issues/2026-06-29-runtime-issues.md`
  - SDD artifacts: `openspec/changes/expand-color-scheme-semantic-tokens/verify-report.md`, this `tasks.md` (if project convention commits SDD artifacts).
- [ ] Ensure `scripts/deploy-mods.sh` is **not** staged.
- [ ] Discard any `cargo fmt` or `ktfmt` churn outside the touched lines.
- **Verification:** `git diff --cached --name-only` lists exactly the intended files.

### Task 9.2: Prepare commit message
**TDD Cycle:** COMMIT
- [ ] Write Conventional Commit message:
  ```
  feat(xs-lsp, xs-plugin): semantic tokens for engine/modded/unmodded functions + local/static variables + types (Issue #3 proper)

  - LSP: advertise textDocument/semanticTokens/full with function/variable/type legend and engine/modded/unmodded/local/static modifiers.
  - LSP: implement semantic_tokens.rs with AST extraction, origin classification, local/static extraction, and delta encoding.
  - Plugin: fix 3a (`include`), 3b (braces/operators), and 3c (documented identifier-under-caret limitation).
  - Plugin: add 8 new semantic color categories and wire XsSemanticTokensConverter through LspSemanticTokensSupport.
  - Bump pluginVersion 0.4.0 → 0.5.0 (MINOR per AGENTS.md).
  ```
- **Verification:** Message ready; actual commit performed by the orchestrator after `sdd-verify`.

---

## Cross-reference to design/spec

| Requirement | Covered by | Notes |
|-------------|------------|-------|
| R1 `include` keyword | Tasks 1.1–1.2, 3.1 | TextMate + native lexer. |
| R2 braces/operators | Tasks 1.3–1.4, 3.2–3.3 | New token types/flex rules + 8 highlighter mappings. |
| R3 identifier under caret | Tasks 1.5–1.6, 4.1–4.3 | Document platform limitation; does not fix engine-level pass. |
| R4 engine/modded/unmodded functions | Tasks 1.7–1.8, 2.1–2.9 | LSP emitter + legend + plugin converter. |
| R5 local/static variables | Tasks 1.7–1.8, 2.5, 2.7 | `build_full_symbol_table` + modifier classification. |
| R6 built-in/unmodded/modded types | Tasks 1.7–1.8, 2.4, 2.6 | Primitive type nodes + class origin lookup. |
| R7 LSP capability/legend | Tasks 2.1, 2.2 | Server capabilities advertising. |
| R8 plugin color mapping | Tasks 1.9–1.10, 5.1–5.6 | 8 keys, descriptors, converter, wiring. |
| R9 pluginVersion 0.5.0 | Task 7.4 | MINOR bump. |

## Skill resolution

- `writing-plans/SKILL.md` — used to structure exact file paths, TDD cycles, and verification steps.
- `test-driven-development/SKILL.md` — used for RED/GREEN/REFACTOR ordering and "watch it fail" verification.
- `work-unit-commits/SKILL.md` — used to size the change, identify the 400-line budget risk, and recommend chained PR slices before implementation.
- `_shared/SKILL.md` — used as the shared SDD skill context reference.