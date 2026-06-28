# `xs-language-server` Spec Index

This directory contains the delta specs for the redesigned Rust XS Language Server. Each spec defines one independently testable capability.

## Spec files

| Spec | Capability |
|---|---|
| [`spec-engine-data-pipeline.md`](spec-engine-data-pipeline.md) | Extract engine API from `doxygen_retail.7z`, cache by SHA-256, resolve game path. |
| [`spec-virtual-project-overlay.md`](spec-virtual-project-overlay.md) | Per-mod virtual project: game folder + mod `game/` overlay, include resolution, include-root inference. |
| [`spec-file-watching-cache-invalidation.md`](spec-file-watching-cache-invalidation.md) | Client-side file watching, per-file parse cache, cache invalidation. |
| [`spec-semantic-diagnostics.md`](spec-semantic-diagnostics.md) | Engine-enforced semantic diagnostics: `extern`, `mutable`, use-before-definition, cross-file visibility, int/float compatibility. |
| [`spec-intellij-client-integration.md`](spec-intellij-client-integration.md) | IntelliJ plugin as thin LSP client: settings, auto-detect, LSP lifecycle, watchers. |
| [`spec-vscode-removal.md`](spec-vscode-removal.md) | Remove VS Code client scope and `extracted/xs.vsix` from the repository. |

## Dependency notes

- `spec-engine-data-pipeline`, `spec-virtual-project-overlay`, and `spec-file-watching-cache-invalidation` define the new workspace/data architecture.
- `spec-semantic-diagnostics` depends on all three; it needs engine data, virtual-project overlays, and cached cross-file symbol tables.
- `spec-intellij-client-integration` depends on the server capabilities in the first four specs.
- `spec-vscode-removal` is independent repository cleanup.

## Resolved open questions

1. **Include-root context** — inferred from relative path under `game/ai/`, `game/data/trigger/`, or `game/random_maps/`.
2. **`mutable` redefinition equality** — same signature requires identical name, parameter types, and default values.
3. **Combined mod shared sources** — out of scope; packaging concern handled by `scripts/deploy-mods.sh`.
4. **Cache GC policy** — keep current `v1/<hash>.json`; old schema-version directories are left in place and ignored.
5. **Int/float type checking** — implicit `int` ↔ `float` compatibility in arithmetic, comparisons, and engine-call arguments.
