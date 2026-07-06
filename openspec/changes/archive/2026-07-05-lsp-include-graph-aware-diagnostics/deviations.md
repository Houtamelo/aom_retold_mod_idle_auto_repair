# Deviations from design.md captured during PR-5/6 implementation

User-confirmed V1-only deviations (locked 2026-07-05 during SDD apply session):

- Currently-open → mod-overlay → alphabetical aggregation ordering (locked mid-session, design.md §11.1)
- did_change_watched_files eager invalidation (locked mid-session, design.md §11.2)

V2 cleanup backlog (still TODO):

- Drop project-fallback in `forward_callable_merged` once PR-5 covers all paths
- Drop `project.files` scan in `resolve_workspace_function`
- Full per-root iteration in `server::publish_diagnostics` (currently picks FIRST root only)
