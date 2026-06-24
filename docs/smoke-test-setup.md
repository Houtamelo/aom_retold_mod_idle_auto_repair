# Manual Smoke Test — Setup Guide

**Date:** 2026-06-24
**For:** first real-world test of the redesigned XS Language Server
**Pre-requisite:** this guide assumes the smoke test is the next action after
the archive of the `xs-language-server` SDD change. The pending-task in
`docs/blockers/pending-task-include-paste.md` is the next planned work item.

---

## Artifacts

| File                                                    | Size   | SHA-256                                                          |
| ------------------------------------------------------- | ------ | ---------------------------------------------------------------- |
| `dist/xs-language-server`                               | 8.5 MB | `8de9272c13d2d98b5ac64241a46da44628291533c79406121d760b1d2e683eac` |
| `dist/intellij-xs-plugin-0.1.0.zip`                     | 1.0 MB | `f89435b1d13403cd9766bbf11d68df8898f744a98863c47e16e209befc78b3ac` |

Both were built from branch `xs-language-server/redesign-phase-5` (commit `d03ce69`).
Tests at build time: `cargo test` 75/0 green, `./gradlew test` 13/0 green, `lsp_roundtrip_test` PASS.

---

## Step 0 — Prerequisites

- **OS:** Linux (this build was produced on a Linux sandbox; the binary is
  Linux x86_64). For Windows, rebuild with `cargo build --release` on a
  Windows host.
- **IntelliJ IDEA or JetBrains Rider:** version 2024.2 or newer
  (matches the `ideaIC-2024.2` target the plugin was built against).
- **AoM:R installed:** at `~/.steam/steam/steamapps/common/Age of Mythology Retold/`
  (or the Proton equivalent on Linux). The LSP needs the game folder path.
- **A doxygen archive at the game root:** the LSP needs
  `<game_root>/doxygen_retail.7z`. Copy from this repo:
  ```bash
  cp docs/doxygen_retail.7z ~/.steam/steam/steamapps/common/Age\ of\ Mythology\ Retold/
  ```
  This 7z is the source for the engine API the LSP extracts and caches.
  If the file isn't there, the LSP exits with a descriptive error.

---

## Step 1 — Install the IntelliJ plugin

1. Open IntelliJ IDEA / Rider.
2. **Settings → Plugins → ⚙ → Install Plugin from Disk…**
3. Choose `dist/intellij-xs-plugin-0.1.0.zip`.
4. Restart the IDE when prompted.

You can verify the install:
- **Settings → Plugins → Installed** → search for "XS Language Server".
- The plugin should also expose a settings panel under
  **Settings → Languages & Frameworks → XS Language Server**.

---

## Step 2 — Place the LSP binary

The plugin auto-finds the LSP binary in this order
(see `XsLspConnection.kt::resolveBinaryPath`):

1. `-Dxs.lsp.path=...` system property (passed to the IDE's JVM)
2. `XS_LSP_PATH` environment variable
3. `<project_root>/tools/xs-language-server/target/release/xs-language-server`
   (the project-relative default; **does not apply if the project root
   doesn't have the LSP source checked out**)
4. `xs-language-server` on the `PATH`

Easiest: put the binary anywhere and set `XS_LSP_PATH`:

```bash
mkdir -p ~/bin
cp dist/xs-language-server ~/bin/
chmod +x ~/bin/xs-language-server
export XS_LSP_PATH=~/bin/xs-language-server
```

If you use this option, also add the export to your shell rc
(`~/.bashrc`, `~/.zshrc`, etc.) so it persists across IDE restarts.

Alternative: drop the binary into the project's `tools/` folder
so the plugin's default resolution works:
```bash
mkdir -p tools/xs-language-server/target/release
cp dist/xs-language-server tools/xs-language-server/target/release/
chmod +x tools/xs-language-server/target/release/xs-language-server
```

**Note:** the LSP runs over stdio. The plugin spawns it as a child
process; the IDE does not need any open port.

---

## Step 3 — Configure the game folder

1. In IntelliJ, open one of the mod projects in this repo
   (e.g., `mod/intelligent_auto_repair_and_scout/`).
2. **Settings → Languages & Frameworks → XS Language Server**
3. **Game folder:** `<AOMR_install_root>` (the directory that contains
   both `game/` and `doxygen_retail.7z`).
   Example: `~/.steam/steam/steamapps/common/Age of Mythology Retold`
4. **Mod paths:** click **Auto-detect**. The plugin will recursively
   scan the project for folders named exactly `game` (stopping at each
   `game/` boundary) and add each `game` folder's parent as a mod root.
5. If auto-detect finds nothing (the project is empty, or no
   `mod/<name>/game/...` tree exists), the IDE shows a notification.
   Add paths manually in that case.

---

## Step 4 — Open an XS file and verify diagnostics

Open any XS file in the mod, e.g.:
`mod/intelligent_auto_repair_and_scout/game/ai/human_assist/human_assist.xs`

You should see:

1. **Engine API completion**: type `aiE` → see `aiEcho`, `aiEchoCategory`,
   `aiEchoWarning`.
2. **Hover**: hover over `aiEcho` → signature + help text in a popup.
3. **Go-to-definition**: right-click on `aiEcho` → "Go to Declaration" →
   jumps to `xs-stub://engine/aiEcho` (a virtual declaration URI).
4. **Workspace symbol**: `Ctrl+Alt+Shift+S` (or via menu) → type a name
   → finds symbols from the current mod file plus `extern` symbols from
   the game folder.
5. **Semantic diagnostics**: introduce an error, e.g.,
   add a call to `nonExistentFunction()` — the IDE should underline it
   with an "unresolved symbol" message.
6. **Forward declaration**: call a function defined later in the same
   file without `mutable` and without a forward declaration — the IDE
   should flag the use.
7. **extern collision**: edit two files in the same mod to both declare
   `extern int gFoo = 0;` — the IDE should flag both files with a
   collision error.
8. **Unowned file**: open a `.xs` file that is NOT inside any registered
   mod root. The IDE should show a notification "file not part of any
   registered mod; engine API only", and the only diagnostics you'll
   get are engine-API checks (no cross-file resolution).

---

## Step 5 — Watcher invalidation

1. While the IDE is open, edit a file under `<game>/game/...`
   (e.g., add a comment to `<game>/game/ai/core/x.xs`).
2. The IDE should detect the change and send `didChangeWatchedFiles`
   to the LSP. Open a file in your mod and look at the diagnostics —
   they should reflect the updated game folder file.

This validates the cache invalidation path.

---

## Step 6 — Settings change handling

1. In the plugin settings, add a new mod path (or remove one).
2. Save. The plugin should send `didChangeWorkspaceFolders` to the LSP
   without restarting it (unless the game path also changed).
3. The next time you open a file in the new mod, diagnostics should
   reflect the new virtual project.

---

## Verifying the cache

After the first run, the LSP creates a cache at:

```
~/.local/state/aomr_lsp/v2/<sha256-of-7z>.json
```

The cache is keyed by the SHA-256 hash of `doxygen_retail.7z`. If the
file changes (engine update, new 7z generated), the cache is invalidated
and re-extracted automatically.

You can inspect the cache:
```bash
ls -la ~/.local/state/aomr_lsp/v2/
cat ~/.local/state/aomr_lsp/v2/<hash>.json | head -c 200
```

To force a cold re-extraction, delete the cache directory:
```bash
rm -rf ~/.local/state/aomr_lsp/
```

---

## What to record for the follow-up

When you're done, please note:

1. Which steps worked as expected.
2. Which steps failed (with the error message and IDE log if possible).
3. Any unexpected behaviour.
4. Whether the include-paste approximation (`docs/blockers/pending-task-include-paste.md`)
   caused any visible problems in the manual test.

This report feeds into the next SDD cycle (the include-paste fix).

---

## Stopping the LSP

The LSP shuts down when IntelliJ closes the project. To force a restart:

- **Settings → XS Language Server → change the game path → save**
  (the plugin will restart the LSP)
- Or close and reopen the project.

---

## Common issues

| Symptom                                          | Likely cause                                                                                 |
| ------------------------------------------------ | -------------------------------------------------------------------------------------------- |
| "Game folder not configured" notification        | Settings → XS Language Server → Game folder is empty                                         |
| "Failed to start XS Language Server"             | `XS_LSP_PATH` is unset and the binary isn't in the auto-discovered project location            |
| "doxygen_retail.7z not found in <game>" error    | Copy the 7z to `<game>/doxygen_retail.7z` (see Step 0)                                         |
| No diagnostics for engine API calls              | LSP didn't start; check the IDE log for errors                                                |
| "file not part of any registered mod" warning    | The open file is not under any mod root; auto-detect may have missed it; add manually         |
| IntelliJ doesn't show the XS settings panel      | The plugin didn't load; check **Settings → Plugins**                                          |
| Diagnostics look stale after editing a game file | Watcher not registered; check the IDE log for `workspace/didChangeWatchedFiles` registration |

If you see something not in this table, capture the IDE log
(**Help → Diagnostic Tools → Debug Log Settings** with
`#com.aomr.xs` and `#com.intellij.openapi.vfs.newvfs` set to `TRACE`),
then `tail -f ~/Library/Logs/JetBrains/.../idea.log` (macOS) or
`~/.cache/JetBrains/.../log/idea.log` (Linux).

---

## Next step after the smoke test

When the smoke test is complete, work on the pending task:
**`docs/blockers/pending-task-include-paste.md`** — replace the
include-paste approximation with a true merged view.
