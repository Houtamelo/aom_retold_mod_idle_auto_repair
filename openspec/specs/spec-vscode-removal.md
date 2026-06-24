# VS Code Client Removal Specification

> **Added/updated by change:** `xs-language-server` (workspace & engine-data redesign)  
> **Archived:** 2026-06-24  
> **Change verdict:** PASS WITH DEVIATIONS

## Capability summary
This change explicitly removes the VS Code client scope from the repository: `extracted/xs.vsix` is deleted and not tracked, the `.gitignore` exception that allowed it is removed, week 7 commit `1628aa6` is stranded or reverted, and no replacement VS Code extension is built.

## Rationale
The `xs.vsix` artifact came from the official game and was converted into an LSP client in commit `1628aa6`. That work is outside this project's responsibility; maintaining a forked VS Code extension creates legal and maintenance risk and duplicates the official game's client.

## Scenarios

### Scenario: happy path — repository clean of VS Code client
- GIVEN `extracted/xs.vsix` was removed
- AND `.gitignore` contains `extracted/` with no exception for `xs.vsix`
- WHEN a developer lists tracked files
- THEN no VS Code extension binary is present

### Scenario: edge case — history remains
- GIVEN commit `1628aa6` introduced the `.vsix` and `.gitignore` exception
- WHEN the repository history is inspected
- THEN the commit is still present (stranded) or has been reverted in subsequent history
- AND the working tree contains no `.vsix` files

### Scenario: negative case — accidental re-add
- GIVEN a build script attempts to recreate `extracted/xs.vsix`
- WHEN CI or a contributor tries to commit it
- THEN `.gitignore` prevents tracking

## XS-engine constraints
- None. This is a repository-scope cleanup, not an XS semantic change.

## Out of scope
- Removing unrelated files under `extracted/` if the folder is used for other CryBar-extracted reference data in the future.
- Interacting with the official game's VS Code extension; this project simply does not ship one.

## Verification approach
- Automated: CI check asserting no `.vsix` files are tracked and `.gitignore` does not contain `!extracted/xs.vsix`.
- Manual: inspect the working tree and `git ls-files | grep '\.vsix$'`.

## Acceptance criteria
1. `extracted/xs.vsix` SHALL be removed from the working tree with `git rm` if present.
2. `.gitignore` SHALL ignore the `extracted/` directory and SHALL NOT contain an exception for `extracted/xs.vsix`.
3. Commit `1628aa6` SHALL be reverted OR left stranded in history; in either case, the resulting working tree SHALL NOT contain `extracted/xs.vsix`.
4. The project SHALL NOT build, ship, or commit a replacement VS Code extension.
5. The IntelliJ plugin SHALL remain the only IDE client developed in this repository.

---

## Change history

| Change | Date | Verdict | Notes |
|---|---|---|---|
| `xs-language-server` | 2026-06-24 | PASS WITH DEVIATIONS | Initial spec; commit `1628aa6` was left stranded in history; working tree contains no `.vsix` files. |
