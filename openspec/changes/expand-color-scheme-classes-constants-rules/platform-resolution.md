# Platform API Resolution

## MemberIndex placement decision

After reading the existing symbol table (`tools/xs-language-server/src/symbols.rs`) and semantic-token pipeline (`tools/xs-language-server/src/semantic_tokens.rs`), the `MemberIndex` is placed inside `tools/xs-language-server/src/semantic_tokens.rs` rather than a new `member_index.rs` or inside `symbols.rs`.

### Rationale

- `symbols.rs` is foundational. Pulling in workspace/cache/semantic-modifier logic there would create an upward dependency from `symbols` → `workspace`/`cache`/`semantic_tokens`.
- `semantic_tokens.rs` is already the consumer that needs to resolve member-name origins on every keystroke. It already imports `workspace`, `engine_api`, and `symbols`.
- Keeping the index adjacent to the token walker avoids adding a new module and keeps the public surface minimal: `MemberIndex::new` builds the map, and `MemberIndex::origin(name)` answers the heuristic.

### Data structure

```rust
struct MemberIndex {
    origins: HashMap<String, Origin>,
}
```

- Key: member identifier (`field` or `method` name).
- Value: strongest `Origin` seen for that name (`Modded > Unmodded > Engine`).

### Build strategy

`MemberIndex::build` scans:

1. `own_table.symbols` for `ClassField`/`ClassMethod` entries (covers members declared in the file being colored).
2. Every `(rel, path)` returned by `project.visible_files(workspace)` for `ClassField`/`ClassMethod` entries, loaded via `cache::load_or_parse_symbols`.

The current file is handled via `own_table` so tests do not need to write it to disk just to index it. For each member symbol found, its origin is computed with the existing `classify_origin(path, workspace, project)` and promoted using the strength ordering above.

### Deviations from `design.md`

None for the MemberIndex design. The design assumed the struct could live in symbols or a new file; the resolution is to keep it in `semantic_tokens.rs` and document the choice here.
