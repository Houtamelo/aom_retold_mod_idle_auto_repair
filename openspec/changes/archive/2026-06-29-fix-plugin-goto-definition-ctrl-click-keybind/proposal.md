# Proposal: fix-plugin-goto-definition-ctrl-click-keybind

## Status

**Draft**

## Background

Issue #1 in `docs/issues/2026-06-29-runtime-issues.md` reports that go-to-definition works only through the right-click context menu. Hover popups, right-click **Go to → Declarations or usages**, and the LSP `textDocument/definition` endpoint all return valid data, but **Ctrl+Click** and the **Ctrl+B / ⌘B** keybind do nothing in Rider.

The plugin is a thin IntelliJ Platform LSP client: it registers a `platform.lsp.serverSupportProvider`, starts the bundled Rust XS language server, and lets the platform auto-wire hover, completion, diagnostics, and Find Usages. The explore report (`openspec/changes/fix-plugin-goto-definition-ctrl-click-keybind/explore.md`) confirmed that the platform's Ctrl+Click / keybind path is not producing navigation targets for this lightweight XS PSI setup on 2024.2. There is **no** `GotoDeclarationHandler` or `PsiReferenceContributor` registered for XS, so the platform has no client-side fallback when its auto-wire gap is hit.

## Approach

**Chosen approach: (a) `GotoDeclarationHandler`.**

| Criterion | Approach (a) `GotoDeclarationHandler` | Approach (b) `PsiReferenceContributor` / `PsiReferenceProvider` |
|---|---|---|
| **Correctness for multi-target** | `getGotoDeclarationTargets()` returns `PsiElement[]`; the platform shows a chooser popup when there are multiple targets. | Multi-target requires `PsiPolyVariantReference.multiResolve()`. Possible, but adds reference-model complexity. |
| **Scope** | Minimal: plugs only the broken Ctrl+Click / keybind path. Does not touch hover, right-click, or Find Usages. | More invasive: changes how XS identifiers expose references, which can affect Find Usages and other reference-based features that already work. |
| **Future risk if platform auto-wire starts working** | Risk of duplicate navigation targets if a platform `GotoDeclarationHandler` also starts returning results. The handler can later be guarded or removed because it is isolated. | Risk of duplicate references in Find Usages (already functional today). Removing the contributor is harder once other code depends on it. |
| **Testability** | Single-method interface with clear inputs (`PsiElement`, offset, editor) and outputs (`PsiElement[]`). Easy to inject a seam / fake resolver. | Requires a reference contributor registration, `PsiReferenceProvider`, and fixture-level reference resolution. More boilerplate. |

Because the bug is specifically the missing Ctrl+Click / keybind wiring, and Find Usages + right-click already work, the `GotoDeclarationHandler` is the smallest, safest fix. It is the same extension point that LSP4IJ uses to implement `textDocument/definition` navigation.

### Implementation sketch

1. Create `com.aomr.xs.navigation.XsGotoDeclarationHandler` implementing `com.intellij.codeInsight.navigation.actions.GotoDeclarationHandler`.
2. Create a small seam `com.aomr.xs.navigation.XsDefinitionResolver` (production implementation: `XsLspDefinitionResolver`) so the handler can be unit-tested without a real LSP server.
3. Register `<gotoDeclarationHandler implementation="com.aomr.xs.navigation.XsGotoDeclarationHandler" />` in `META-INF/plugin.xml`.
4. Bump `pluginVersion` from `0.2.2` to `0.2.3` per `AGENTS.md` PATCH policy.

### LSP request API

The bundled IntelliJ Platform 2024.2 (build `IU-242.20224.300`) exposes the following methods on `com.intellij.platform.lsp.api.LspServer`:

```kotlin
// Suspendable request
suspend fun <Lsp4jResponse> sendRequest(
    request: (LanguageServer) -> CompletableFuture<Lsp4jResponse>
): Lsp4jResponse

// Synchronous request with timeout
fun <Lsp4jResponse> sendRequestSync(
    timeoutMs: Int,
    request: (LanguageServer) -> CompletableFuture<Lsp4jResponse>
): Lsp4jResponse

// Default timeout constant (10 seconds)
const val DEFAULT_REQUEST_TIMEOUT_MS = 10000
```

The handler will call the LSP `textDocument/definition` endpoint like this:

```kotlin
val params = DefinitionParams(server.getDocumentIdentifier(file), position)
val response = server.sendRequestSync(5000) { ls ->
    ls.textDocumentService.definition(params)
}
```

`response` is `Either<List<Location>, List<LocationLink>>` (or `null`). The handler converts every `Location` / `LocationLink` into a navigable `PsiElement` returned in the array.

## User-facing contract

- **Ctrl+Click** on a symbol in an `.xs` file navigates to its definition.
- **Go-to-definition keybind** (Ctrl+B / ⌘B) with the caret on a symbol navigates to its definition.
- **Multi-target**: if the LSP returns multiple `Location`s / `LocationLink`s, the platform's chooser popup appears; the user picks the target.
- **Timeout**: each LSP request is capped at **5 seconds**. On timeout the handler returns no targets and does nothing (no stale navigation, no error balloon).
- **No server / no result**: if the LSP server is not running or returns `null`/empty, the handler returns no targets.
- **Regressions**: hover and right-click **Declarations or usages** continue to work exactly as before.

## Scope

### In scope

- `XsGotoDeclarationHandler` implementation.
- `XsDefinitionResolver` seam + `XsLspDefinitionResolver` production implementation.
- `META-INF/plugin.xml` registration of the `gotoDeclarationHandler` extension.
- At least one IntelliJ Platform test exercising navigation through a fake resolver.
- `pluginVersion` PATCH bump (`0.2.2 → 0.2.3`).

### Out of scope

- Hover changes (already works).
- Right-click context menu "Go to → Declarations or usages" (already works).
- Find Usages (already works via platform `textDocument/references`).
- Issues 2, 3, and 4 from `docs/issues/2026-06-29-runtime-issues.md`.
- LSP server-side changes (`tools/xs-language-server/`). The server already implements `textDocument/definition` correctly.
- Include-statement navigation (Issue #4); the design should not block adding a second handler later.

### Audit overlap

No overlap with `docs/code-reviews/2026-06-29-lsp-and-plugin-review.md`. That audit was static-only; this is a runtime UX bug that static review did not catch.

## Impact

- **Files changed:**
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandler.kt` (new)
  - `tools/intellij-xs-plugin/src/main/kotlin/com/aomr/xs/navigation/XsLspDefinitionResolver.kt` (new)
  - `tools/intellij-xs-plugin/src/main/resources/META-INF/plugin.xml`
  - `tools/intellij-xs-plugin/src/test/kotlin/com/aomr/xs/navigation/XsGotoDeclarationHandlerTest.kt` (new)
  - `tools/intellij-xs-plugin/gradle.properties`
- **New test files:** `XsGotoDeclarationHandlerTest.kt` (and its fake resolver helpers, which may live in the same test file).
- **LOC delta:** +~130–170 lines of Kotlin, +1 line XML, +1 line properties; no deletions expected.
- **Risk:** medium. Depends on an internal/experimental LSP client API (`LspServer.sendRequestSync`), runs on the EDT with a timeout cap, and may produce duplicate targets if the platform's auto-wire later starts working.
- **Effort:** 6–10 hours (handler + resolver 3–4h, tests + fixtures 2–3h, Rider smoke test 1–2h).
- **Compatibility risk:** low for the existing right-click menu and hover, because they use different platform paths. The only regression vector is if the handler accidentally fires for non-XS files; code will guard against that.
- **pluginVersion bump:** `0.2.2 → 0.2.3` (per `AGENTS.md` PATCH policy: bug fix, no new public feature).

## Success criteria

- Ctrl+Click on a symbol navigates to its definition.
- The go-to-definition keybind navigates to its definition.
- Right-click menu **Go to → Declarations or usages** still works (no regression).
- Hover still works (no regression).
- `./gradlew buildPlugin` produces a valid plugin `.zip`.
- The new IntelliJ Platform test passes.

## Risks and unknowns

- **LSP request API is platform-internal.** `LspServer.sendRequestSync` is available in 2024.2 but is part of the non-public LSP client API. It may change in future platform versions; the plugin targets `2024.2–2025.2` today.
- **EDT blocking.** `GotoDeclarationHandler.getGotoDeclarationTargets` runs in a read action on the EDT. The 5-second `sendRequestSync` timeout prevents indefinite freezes, but the UI may still stutter briefly. The design phase should decide whether a progress indicator / `runBlockingCancellable { sendRequest(...) }` wrapper is required.
- **Duplicate targets if platform auto-wire improves.** If a later IntelliJ/Rider version fixes the LSP Ctrl+Click wiring, the user could see duplicate navigation targets. Mitigation: deduplicate by URI + offset, or remove the custom handler once platform wiring is verified.
- **Rider-specific verification required.** The bug was reported in Rider, where Ctrl+Click can interact with ReSharper. Headless platform tests may not reproduce Rider's action wiring; a manual smoke test in Rider is mandatory.
- **Location/LocationLink conversion.** Targets may point to files outside the project (e.g., vanilla `game/` files). The handler must convert any `file://` URI into a navigable `PsiElement`, falling back to `OpenFileDescriptor` if a `PsiFile` cannot be obtained.

## Open questions

1. Should the handler use `sendRequestSync(5000)` or `runBlockingCancellable { sendRequest { ... } }` to satisfy the EDT/non-blocking contract most cleanly?
2. Does the `com.intellij.gotoDeclarationHandler` extension point support an `order` attribute so our handler can act as a fallback? If so, should it be `order="last"`?
3. What is the exact chooser behavior when both the platform LSP handler and our handler return the same target? Is there a cheap way to detect platform-handled targets and skip ours?
4. Should the resolver seam return `List<PsiElement>` or `Array<PsiElement>` to align with the handler signature?

## Next step

The **spec phase** should formalize the `XsGotoDeclarationHandler` / `XsDefinitionResolver` contract, define the LSP-response-to-PsiElement conversion rules, and write the Given/When/Then test scenarios (single target, multi-target, timeout, non-XS file, engine-only symbol returning null).
