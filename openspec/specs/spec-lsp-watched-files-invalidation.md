# LSP Watched-Files Per-Root Cache Invalidation Specification

<!-- delta-spec for archived change 2026-07-05-lsp-include-graph-aware-diagnostics -->

> **Added/updated by change:** `lsp-include-graph-aware-diagnostics`
> **Archived:** 2026-07-05
> **Change verdict:** PASS WITH DEVIATIONS

## Purpose

The LSP server SHALL eagerly invalidate cached per-root merged views when the client reports filesystem changes via `workspace/didChangeWatchedFiles`. This keeps diagnostics fresh when vanilla files, mod files, or sibling includes change outside the current editor buffer.

## Requirements

### R1 — Register watcher on initialization

The server SHALL register a `**/*.xs` watcher during LSP initialization if the client supports dynamic watcher registration.

- GIVEN a client that supports dynamic watcher registration, WHEN the server initializes, THEN it SHALL register a watcher scoped to the workspace folders for `**/*.xs`.
- GIVEN a client that does not support dynamic watcher registration, WHEN the server initializes, THEN it SHALL skip registration and degrade gracefully.

### R2 — Coalesce notification bursts

The server SHALL coalesce rapid sequences of watched-file notifications before invalidating caches.

- GIVEN many `.xs` files change within a short window (for example, an engine reload), WHEN the server receives the notifications, THEN it SHALL wait for a short coalescing window (≈50 ms) and process the distinct changed paths in one invalidation pass.

### R3 — Invalidate affected root caches

For each changed path, the server SHALL drop every per-root merged-view cache entry whose closure contains that path.

- GIVEN per-root cache entries exist for roots `R1` and `R2`, WHEN a file in `R1`'s closure changes, THEN the entry for `R1` SHALL be invalidated and the entry for `R2` SHALL remain if its closure does not contain the changed file.

### R4 — Mark reverse-include graph dirty

The server SHALL mark the reverse-include graph dirty when files are added or removed.

- GIVEN a watched file is created or deleted, WHEN the notification is processed, THEN the reverse-include graph SHALL be marked dirty so it rebuilds on the next `roots_that_include` query.

### R5 — Graceful degradation

If the client does not support dynamic watcher registration, the server SHALL still operate correctly with potentially stale caches until a file is reopened or the server restarts.

- GIVEN an unsupported client, WHEN a file outside the current buffer changes, THEN the server SHALL not crash and diagnostics for already-open files may lag until reopen.

## Scenarios

### S1 — Watcher registration on init

- GIVEN a client that supports dynamic capability registration
- WHEN `initialize` completes and `initialized` is received
- THEN the server SHALL register a watcher for `**/*.xs`

### S2 — Single file change invalidates root cache

- GIVEN `shared.xs` is in the closure of root `R.xs`
- AND `MergedView(R.xs)` is cached
- WHEN the client notifies that `shared.xs` changed
- THEN the cache entry for `MergedView(R.xs)` SHALL be invalidated

### S3 — File deletion marks graph dirty

- GIVEN a warm reverse-include graph
- WHEN the client notifies that an `.xs` file was deleted
- THEN the graph SHALL be marked dirty
- AND the next `roots_that_include` query SHALL rebuild it

### S4 — Notification burst coalescing

- GIVEN 50 `.xs` files change within 20 ms
- WHEN the server receives the notifications
- THEN it SHALL process the distinct paths in one coalesced invalidation pass
- AND SHALL not run 50 separate invalidation passes

### S5 — Unsupported client degrades gracefully

- GIVEN a client that does not support dynamic watcher registration
- WHEN the server initializes
- THEN it SHALL not register a watcher
- AND subsequent file changes outside the editor SHALL not crash the server

## Out of scope

- Re-diagnosing closed files eagerly; only open files are re-diagnosed on the next interaction.
- Server-initiated polling of the filesystem.
- Watching files outside registered workspace folders.
- Persistent invalidation log or audit trail.

## Verification approach

- Rust integration tests in `tools/xs-language-server/lsp/tests/per_root_cache_repro.rs` for invalidation-by-path.
- LSP handler tests that simulate `workspace/didChangeWatchedFiles` and assert cache invalidation.
- Manual test: edit a vanilla file in an external editor while a mod file is open in IntelliJ/Rider and confirm diagnostics refresh.

## Acceptance criteria

1. The server SHALL register a `**/*.xs` watcher during initialization when the client supports it.
2. The server SHALL coalesce rapid sequences of watched-file notifications.
3. The server SHALL invalidate every per-root cache entry whose closure contains a changed path.
4. The server SHALL mark the reverse-include graph dirty on file creation/deletion.
5. The server SHALL degrade gracefully when the client does not support dynamic watcher registration.
6. Invalidation SHALL NOT block other LSP handlers.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `lsp-include-graph-aware-diagnostics` | 2026-07-05 | PASS WITH DEVIATIONS | V1 uses eager invalidation via `did_change_watched_files` rather than purely lazy invalidation on the next `didOpen`. See archive `deviations.md` for V2 cleanup backlog. |
