# `intellij-xs-plugin` Spec Index

This directory contains one independently testable spec per plugin capability. The work is organized so that each phase (P0–P5) has a clear contract that can be implemented and verified without waiting for later capabilities.

## Spec files

| Spec | Capability | Primary phase |
|------|------------|---------------|
| [`spec-packaging.md`](spec-packaging.md) | Gradle build, `plugin.xml`, bundled resources, version pinning, signing readiness | P0 foundation |
| [`spec-syntax-highlighting.md`](spec-syntax-highlighting.md) | TextMate grammar registration and dark-theme parity | P0 |
| [`spec-engine-completion.md`](spec-engine-completion.md) | Engine syscall name and default-parameter completion | P1 |
| [`spec-hover-docs.md`](spec-hover-docs.md) | Hover documentation for engine syscalls | P1 |
| [`spec-parameter-info.md`](spec-parameter-info.md) | Parameter hints inside engine-syscall calls | P1 |
| [`spec-class-resolution.md`](spec-class-resolution.md) | PSI fidelity for classes, methods, members, lambdas, `ref`, arrays | P2/P3 |
| [`spec-workspace-navigation.md`](spec-workspace-navigation.md) | Go-to-definition for engine stubs and workspace symbols | P2/P3 |
| [`spec-xml-constants.md`](spec-xml-constants.md) | XML-derived constant completion from user-supplied game XMLs | P4 |
| [`spec-aiplan-constants.md`](spec-aiplan-constants.md) | Bundled AI plan constant completion from `aiplans.json` | P4 |
| [`spec-regen-pipeline.md`](spec-regen-pipeline.md) | Doxygen HTML → `syscalls.json` regeneration task | P5 |

## Dependency graph

```text
spec-packaging
       │
       ├────── spec-syntax-highlighting
       │
       ├────── spec-engine-completion
       │            │
       │            ├── spec-hover-docs
       │            ├── spec-parameter-info
       │            └── spec-aiplan-constants
       │
       ├── spec-class-resolution
       │            │
       │            └── spec-workspace-navigation
       │
       ├── spec-xml-constants
       │
       └── spec-regen-pipeline
```

### Sequencing rules

1. `spec-packaging` and `spec-syntax-highlighting` SHALL land first. They provide the language, file type, and resource-loading foundation.
2. `spec-engine-completion`, `spec-hover-docs`, and `spec-parameter-info` can build in parallel once `spec-packaging` is in place. They share the bundled `syscalls.json` data but do not require a full XS parser.
3. `spec-aiplan-constants` can also ship once `spec-packaging` is ready; it reuses the same completion infrastructure as engine syscalls.
4. `spec-class-resolution` MUST land before `spec-workspace-navigation`. Workspace go-to-definition for local functions, classes, and members requires the PSI elements this spec defines.
5. `spec-xml-constants` depends on `spec-packaging` for the settings page extension point and file watchers, but not on the XS parser.
6. `spec-regen-pipeline` SHOULD land last. It refreshes the same `syscalls.json` consumed by P1 completion, hover, and parameter info; its output can be reviewed and committed after engine-facing behavior is stable.

## Cross-references by concern

| Concern | Relevant specs |
|---------|---------------|
| `syscalls.json` data contract | `spec-engine-completion`, `spec-hover-docs`, `spec-parameter-info`, `spec-workspace-navigation`, `spec-regen-pipeline` |
| Completion contributor behavior | `spec-engine-completion`, `spec-aiplan-constants`, `spec-xml-constants`, `spec-class-resolution` |
| PSI / parser | `spec-class-resolution`, `spec-workspace-navigation` |
| Settings / user configuration | `spec-packaging`, `spec-xml-constants` |
| In-memory generated files | `spec-workspace-navigation` |
| TextMate bundle | `spec-packaging`, `spec-syntax-highlighting` |

## Note on scope boundaries

- Engine-facing specs intentionally avoid requiring a real XS parser; they operate on the bundled JSON and on simple textual call-site recognition.
- Parser-backed specs (`spec-class-resolution`, `spec-workspace-navigation`) explicitly stay out of type inference, semantic checks, and refactoring.
- `spec-packaging` only prepares the plugin for Marketplace distribution; actual Marketplace publishing is deferred.
