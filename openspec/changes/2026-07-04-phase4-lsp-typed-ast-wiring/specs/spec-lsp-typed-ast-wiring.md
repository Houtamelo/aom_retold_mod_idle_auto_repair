# LSP Typed-AST Wiring Specification

## Purpose

Wire `tools/xs-language-server/lsp/` to the typed AST, replacing `tree_sitter` walks, as three stacked PR slices.

## Non-goals

- No new LSP features or capability flags.
- No grammar fixes beyond already-archived Phase 3 + class_specifier + rule-body extraction.
- Full Limitation 6 cleanup is out of scope (only predicate dispatch is fixed).
- No `lsp-types` version bump.

## Requirements

### Requirement: PR-A — symbols.rs rewrite

The system SHALL build `SymbolTable` from `TranslationUnit::from_cst` and SHALL remove the `extract_error_*` workarounds.

#### Scenarios

| ID | Given | When | Then |
|---|---|---|---|
| S-LSP-A-01 | XS source with rule, class, and function definitions | `TranslationUnit::from_cst` extracts it | unit contains matching `ClassDefinition`, `RuleDefinition`, and `FunctionDefinition` |
| S-LSP-A-02 | A `TranslationUnit` with top-level items | `build_symbol_table(tu)` runs | returns one `SymbolEntry` per item with correct `SymbolKind` and byte-span `Range` |
| S-LSP-A-03 | old `symbols.rs` helper surface | `grep` for `pub extract_error_function_definition` | no result and remaining calls fail to compile |
| S-LSP-A-04 | old helper surface | `grep` for `pub extract_error_forward_declaration` | no result and remaining calls fail to compile |
| S-LSP-A-05 | retail excerpt e.g. `game/ai/core/chairon.xs` | parsed via typed AST and `build_symbol_table` | symbol count matches old count minus deleted workarounds |

### Requirement: PR-B — handler rewires

The system SHALL re-implement LSP handlers as typed-AST walks and SHALL translate spans via `span_to_range`.

#### Scenarios

| ID | Given | When | Then |
|---|---|---|---|
| S-LSP-B-01 | source file and its `TranslationUnit` | `semantic_tokens_full(text)` runs | typed-AST token list produced via `span_to_range`, `tree_sitter` grep returns 0 |
| S-LSP-B-02 | identifier in a retail sample | `find_references(identifier, tu)` walks typed AST | returns same references as old tree-sitter path |
| S-LSP-B-03 | identifier resolved to a symbol | `definition_position(identifier, tu)` runs | returns typed-AST-defined `definition_position()` |
| S-LSP-B-04 | parser output with `lelwel::Diagnostic`s | `diagnostics.rs` processes them | counts diagnostics with severity translation, no `is_error`/`is_missing` checks |
| S-LSP-B-05 | parsed `TranslationUnit` | `definition_check.rs` validates declarations/params | checks use typed AST walks of `FunctionDefinition.declarator` and `DeclarationSpecifiers` |
| S-LSP-B-06 | game-folder integration test | typed-AST diagnostic counts compared to baseline | severity thresholds hold with no regression |
| S-LSP-B-07 | existing `tests/symbols_cleanup_repro.rs` | file is deleted and replaced | new tests assert typed-AST extraction on same fixtures |

### Requirement: PR-C — drop tree-sitter and promote scratch crate

The system SHALL remove tree-sitter from the LSP crate, promote `lelwel-xs/` to tracked `xs-parser/`, and preserve the test/build baseline.

#### Scenarios

| ID | Given | When | Then |
|---|---|---|---|
| S-LSP-C-01 | LSP source after PR-C | `grep -r tree_sitter lsp/src/` runs | returns 0 hits |
| S-LSP-C-02 | `lsp/Cargo.toml` | `grep tree-sitter` runs | returns 0 hits |
| S-LSP-C-03 | `lsp/src/parser.rs` | `parse(source, types)` called | wraps `xs_parser::parser::Parser::new`, returns `(Cst, Vec<Diagnostic>)`, no tree-sitter state |
| S-LSP-C-04 | debug binaries `day1_probe`, `dump_top_level`, `inspect_tree` | PR-C completes | each is deleted or rewritten against typed AST |
| S-LSP-C-05 | workspace member `lelwel-xs/` | pre-PR-A mechanical rename runs | becomes `xs-parser/`, `.gitignore` entry removed, `Cargo.toml` updated |
| S-LSP-C-06 | existing 252 LSP unit tests | `cargo test` runs after rename + dep drop | all pass or count improves |
| S-LSP-C-07 | release build | `xs-language-server` starts over stdio | responds correctly to `initialize` and `shutdown` |

## API additions

| Function | Module | Description |
|---|---|---|
| `build_symbol_table(tu: &TranslationUnit) -> SymbolTable` | `lsp/src/symbols.rs` | Builds symbol table from typed AST. |
| `span_to_range(source: &str, byte_offset: usize) -> lsp_types::Range` | `lsp/src/range.rs` (new) | Maps byte offset to LSP `Range`. |
| `parse(source: &str, types: &TypeTable) -> (Cst, Vec<Diagnostic>)` | `lsp/src/parser.rs` | Parses source via typed-AST parser. |

## Verification

All scenarios SHALL be verified with `cargo test --manifest-path tools/xs-language-server/Cargo.toml`.
