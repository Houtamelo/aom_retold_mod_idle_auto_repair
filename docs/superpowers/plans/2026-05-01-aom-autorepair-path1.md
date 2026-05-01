# AoM:R Auto-Repair — Path 1: `<type>AutoRepair</type>` Empirical Test

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Determine empirically whether `<type>AutoRepair</type>` is a working ProtoAction type in Age of Mythology: Retold by adding it to `VillagerGreek` via a `proto_mods.xml` mod, then observing whether an idle Greek villager auto-tasks to repair a damaged friendly building within range.

**Architecture:** Pure-data mod under `mods/local/aom_autorepair_test/` mirroring the `game/` directory tree. Single `game/data/gameplay/proto_mods.xml` adds one `<protoaction>` block plus three flag entries to `VillagerGreek`. Auto-cast machinery wired through documented engine flags: `CastAbilitySelf` (unit) + `AutoCastBySelf` (protoaction) + `AutoCommandStartDisabled` (off-by-default toggle). The *only* change to vanilla data is to one unit; nothing else is touched, so any observed behavior change is attributable to the new protoaction.

**Tech Stack:** XML data files, CryBar.Cli (run via Wine for `.bar`/`.XMB` operations), AoM:R in-game mod manager, the existing `extracted/gameplay/` reference dump from the prior research session.

**Decision criteria for this experiment:**
- **PASS:** the mod loads, a toggle button appears on the Greek villager command card, and an idle villager with auto-repair enabled walks to a nearby damaged friendly building and repairs it.
- **PARTIAL:** mod loads but only some of {button appears, auto-task fires, repair succeeds} works. Document which subset.
- **FAIL:** mod fails to load, or mod loads but no behavior change is observed.

A PASS unlocks a much larger plan (extend to all civs, Norse infantry, polish UI/strings). A FAIL/PARTIAL kicks the project to path 2 (Heal-as-repair) or path 3 (AI-script hook).

## Execution phases

houtamelo plays AoM:R in parallel with development sessions. To avoid touching a running game's files, this plan splits into two phases:

- **Phase A — local-only, safe to run while the game is running.** Tasks 0 (steps 1-3, 5), 1, 2 (step 1 only), 3. All writes confined to `/home/houtamelo/Documents/projects/aom_retold_mod/`. The symlink-into-game-folder step in Task 0 step 4 and Task 2 step 2 are explicitly skipped.
- **Phase B — requires AoM:R to be closed.** Task 0 step 4 (symlink), Task 2 steps 2-3 (deploy + verify in Mod Manager), Task 4 (load check), Task 5 (in-game test), Task 6 (record results). houtamelo runs these manually after closing the game.

Subagents executing this plan must ONLY do Phase A work and stop after Task 3. Do not write to `/home/houtamelo/.steam/` under any circumstances.

---

### Task 0: Initialize project as a git repo and link the mod folder

**Files:**
- Create: `/home/houtamelo/Documents/projects/aom_retold_mod/.gitignore`
- Create: `/home/houtamelo/Documents/projects/aom_retold_mod/README.md`
- Create symlink: `/home/houtamelo/Documents/projects/aom_retold_mod/mod_link` →  the AoM:R local mods folder under Proton

**Why:** Plans assume git for commits; AoM:R only loads mods from a fixed Proton-prefix path so we keep the mod source in this repo and symlink the deployed copy into the game's expected location.

- [ ] **Step 1: Initialize git**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
git init
git config user.email "antoniopedrogf@hotmail.com"
git config user.name "houtamelo"
```

Expected: `Initialized empty Git repository in /home/houtamelo/Documents/projects/aom_retold_mod/.git/`

- [ ] **Step 2: Write `.gitignore` to exclude bulk extracted data**

```bash
cat > /home/houtamelo/Documents/projects/aom_retold_mod/.gitignore <<'EOF'
# Extracted vanilla game data — reference only, not part of the mod.
# Re-extract from Data.bar with CryBar.Cli when needed.
extracted/

# CryBar tools (download, not source).
tools/

# OS / editor noise
.DS_Store
*.swp
EOF
```

- [ ] **Step 3: Write `README.md` stub**

```bash
cat > /home/houtamelo/Documents/projects/aom_retold_mod/README.md <<'EOF'
# AoM: Retold Auto-Repair Mod (WIP)

QoL mod adding a per-unit auto-repair toggle to villagers and Norse infantry.
When enabled and the unit is idle, the unit auto-tasks to repair nearby damaged
friendly buildings. Off by default.

## Layout
- `mod/` — the mod source files (mirrors the in-game mod folder layout).
- `docs/superpowers/plans/` — implementation plans.
- `extracted/` (gitignored) — vanilla game data extracted with CryBar.Cli for
  reference. Re-extract on demand; not committed.
- `tools/` (gitignored) — CryBar.Cli download for working with .bar/.XMB files.
- `mod_link` (symlink, gitignored) — points to the AoM:R local mods folder so
  the game loads `mod/` directly.

## Status
Currently in path 1: testing whether `<type>AutoRepair</type>` is a working
ProtoAction type via a minimal proto_mods.xml. See
`docs/superpowers/plans/2026-05-01-aom-autorepair-path1.md`.
EOF
```

- [ ] **Step 4: Create the `mod/` source directory and the symlink to AoM:R's local mods folder**

```bash
mkdir -p /home/houtamelo/Documents/projects/aom_retold_mod/mod
ln -s "/home/houtamelo/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local" /home/houtamelo/Documents/projects/aom_retold_mod/mod_link
ls -l /home/houtamelo/Documents/projects/aom_retold_mod/mod_link
```

Expected: `lrwxrwxrwx ... mod_link -> /home/houtamelo/.steam/.../mods/local`

- [ ] **Step 5: Commit the scaffolding**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
git add .gitignore README.md
git commit -m "chore: project scaffolding for AoM:R auto-repair mod"
```

---

### Task 1: Get a confirmed `proto_mods.xml` syntax reference from a real mod

**Why:** The official `BANG_Documentation/Data documentation/AoMRT Comprehensive Core Data Guide.pdf` documents `proto.xml` schema but does NOT document the `_mods.xml` additive variant syntax (only mentioned in community guides). Before writing our own, we copy a known-working example so we don't burn an iteration on syntax mistakes. Strategy: search Nexus Mods for a small AoMR mod that edits `proto.xml`, download it, inspect.

**Files:**
- Create: `tools/reference_mods/` (gitignored) — downloaded reference mods, kept for reading only

- [ ] **Step 1: Open Nexus Mods AoMR catalog and find a small mod that ships a `proto_mods.xml`**

Browse: https://www.nexusmods.com/games/ageofmythologyretold/mods (sort by recent or downloads). Pick a mod whose description mentions tweaking unit stats (e.g. an "AOMR Overhaul"-style mod, or any "buff villagers" mod). Download the archive (anonymous downloads are allowed for small mods, or use a free Nexus account).

Expected outcome: a `.zip` or `.rar` containing a folder structure with `game/data/gameplay/proto_mods.xml`.

- [ ] **Step 2: Extract the reference mod into `tools/reference_mods/`**

```bash
mkdir -p /home/houtamelo/Documents/projects/aom_retold_mod/tools/reference_mods
cd /home/houtamelo/Documents/projects/aom_retold_mod/tools/reference_mods
# Replace <archive> with the downloaded file path
unzip ~/Downloads/<archive>.zip -d ./<modname>/
find ./<modname>/ -name 'proto_mods.xml' -o -name 'proto_mods.xml.XMB'
```

Expected: at least one path printed.

- [ ] **Step 3: If the file is `proto_mods.xml.XMB` (compiled), convert to readable XML**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
WINEDEBUG=-all wine ./tools/CryBar.Cli/crybar.exe convert xmb-to-xml ./tools/reference_mods/<modname>/.../proto_mods.xml.XMB
```

Expected: a `proto_mods.xml` appears next to the XMB.

- [ ] **Step 4: Read the reference and capture the schema**

Read the file with `Read` tool. Note:
- Root element name (e.g. `<protomods>` vs `<proto>` vs something else)
- Per-unit syntax: are units identified by `<unit name="X">` or `<unit name="X" mode="merge">` or `<unit id="X">`?
- How to add a new `<protoaction>` to an existing unit (append vs replace)
- How to add a new `<flag>` to an existing unit (does it append or replace the whole flag list?)

Write findings into a short note that Task 3 will reference:

```bash
cat > /home/houtamelo/Documents/projects/aom_retold_mod/docs/proto_mods_syntax.md <<'EOF'
# proto_mods.xml syntax (verified from <reference mod>)

Root element: <ROOT>
Per-unit modification:

<EXAMPLE FROM REFERENCE>

Verified merge behavior:
- Adding a new <protoaction>: <YES/NO with example>
- Adding a new <flag>: <YES/NO with example>
EOF
```

- [ ] **Step 5: Commit the syntax note**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
mkdir -p docs
git add docs/proto_mods_syntax.md
git commit -m "docs: capture proto_mods.xml syntax from reference mod"
```

---

### Task 2: Create the mod skeleton and verify the game detects it

**Files:**
- Create: `mod/aom_autorepair_test/preview.png` (any 256x256 PNG, optional but conventional)
- Create: deployment via the existing `mod_link` symlink

**Why:** Before adding any data changes, confirm the game sees an empty local mod folder in the in-game Mod Manager. This isolates "does the game see the mod?" from "does my XML parse?" so we can debug them separately.

- [ ] **Step 1: Create the mod folder and a `game/data/gameplay/` placeholder**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod/mod
mkdir -p aom_autorepair_test/game/data/gameplay
ls -R aom_autorepair_test
```

Expected:

```
aom_autorepair_test:
game

aom_autorepair_test/game:
data

aom_autorepair_test/game/data:
gameplay

aom_autorepair_test/game/data/gameplay:
```

- [ ] **Step 2: Deploy the mod folder via symlink so the game finds it**

```bash
ln -s /home/houtamelo/Documents/projects/aom_retold_mod/mod/aom_autorepair_test "/home/houtamelo/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local/aom_autorepair_test"
ls -l "/home/houtamelo/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local/"
```

Expected: a symlink entry `aom_autorepair_test -> /home/houtamelo/Documents/projects/aom_retold_mod/mod/aom_autorepair_test`.

- [ ] **Step 3: Launch AoM:R, open the in-game Mod Manager, confirm the empty mod is listed**

Launch AoM:R via Steam. From the title screen click `Mods` → `Mod Manager`. Expected: a row labeled `aom_autorepair_test` appears in the local mods list. It does not need to be enabled yet. If it does not appear, check (a) symlink path matches the steamID folder used by the game (`76561198001426736` is the only one currently — confirmed by listing `mods/local`), (b) the folder name is non-empty.

- [ ] **Step 4: Commit the empty mod skeleton**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
# Keep the empty gameplay folder tracked so future tasks have a place to write
touch mod/aom_autorepair_test/game/data/gameplay/.gitkeep
git add mod/
git commit -m "feat: empty aom_autorepair_test mod skeleton"
```

---

### Task 3: Write the minimal `proto_mods.xml` that adds the AutoRepair protoaction to VillagerGreek

**Files:**
- Create: `mod/aom_autorepair_test/game/data/gameplay/proto_mods.xml`

**Why:** This is the heart of the experiment. Add exactly the minimum needed to test whether `<type>AutoRepair</type>` is recognized: one new protoaction on `VillagerGreek` with `AutoCastBySelf` set, plus the unit-level enabling flags `CastAbilitySelf` and `AutoCommandStartDisabled`. No new icons, strings, or commands — relying on the existing `AutoRepairUnit` protoUnitCommand entry from `proto_unit_commands.xml` for any UI surfacing the engine may auto-wire.

**Reference for what fields the protoaction needs:** the BANG Core Data Guide (page 22-37) describes `<type>` (action type), `<rate type="X">value</rate>` (target type and rate), `<maxrange>` (engagement range), `<autocastdistance>` (auto-search radius for AutoCastBySelf actions), `<flag>...</flag>` for protoaction flags. The unit's existing `<flag>NoIdleActions</flag>` may suppress idle behavior; we leave it for now and observe — if the test fails we have a known knob to flip in a follow-up iteration.

- [ ] **Step 1: Verify the exact unit entry we're modifying**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
grep -nE '<unit name="VillagerGreek">|<flag>|<protoaction>' extracted/gameplay/proto.xml | awk -F: 'NR==1{start=$1} $0~/<unit name=/ && NR>1 {if (NR>1) exit; print} NR<=200{print}' | head -40
```

Expected: confirms `<unit name="VillagerGreek">` appears at line 1571 in vanilla and that the unit currently has flags `CollidesWithProjectiles, CorpseDecays, ShowGarrisonButton, DontRotateObstruction, ObscuredByUnits, CommonCommands, AutoTrainable, HasDefaultAttack, NoIdleActions, CreateUnitGroupAutomatically, NonAutoFormedUnit, UseSharedBuildLimit`. None of them are `CastAbilitySelf` or `AutoCommandStartDisabled` — i.e. our additions don't conflict with anything pre-existing.

- [ ] **Step 2: Write `proto_mods.xml` using the syntax confirmed in Task 1**

The example below uses the syntax `<unit name="X" mode="merge">` which is the AoE3:DE convention and the most likely-correct guess; **substitute with whatever syntax Task 1 confirmed if it differs**.

```bash
cat > /home/houtamelo/Documents/projects/aom_retold_mod/mod/aom_autorepair_test/game/data/gameplay/proto_mods.xml <<'EOF'
<protomods>
    <unit name="VillagerGreek" mode="merge">
        <flag>CastAbilitySelf</flag>
        <flag>AutoCommandStartDisabled</flag>

        <protoaction>
            <name>AutoRepair</name>
            <type>AutoRepair</type>
            <maxrange>0.2</maxrange>
            <autocastdistance>20.0</autocastdistance>
            <rate type="Building">0.5</rate>
            <flag>AutoCastBySelf</flag>
            <active>0</active>
        </protoaction>
    </unit>
</protomods>
EOF
```

Field-by-field rationale (so a reader can change one variable per iteration):
- `<name>AutoRepair</name>` — internal action name. Distinct from the existing `Repair` action so they do not collide.
- `<type>AutoRepair</type>` — **the central hypothesis under test**. If the engine has no such type, expect either a parse error (mod fails to load) or silent ignore (mod loads, no button, no behavior).
- `<maxrange>0.2</maxrange>` — engagement distance for the actual repair work, mirrors the vanilla `Repair` action in `villager.tactics`.
- `<autocastdistance>20.0</autocastdistance>` — search radius for auto-cast targets. Matches the Priest's `<autoattackrange>20</autoattackrange>` so it's a comparable scale.
- `<rate type="Building">0.5</rate>` — vanilla repair rate is 0.5 HP/sec. Setting the same here is balance-neutral if the engine actually applies it; if the engine ignores the rate for type=AutoRepair, that's diagnostic info.
- `<flag>AutoCastBySelf</flag>` — protoaction-level flag the docs describe as enabling auto-activation when `CastAbilitySelf` is set on the unit.
- `<active>0</active>` — start with this action's auto-cast toggle off (matches the user requirement of "off by default").

- [ ] **Step 3: Sanity-check the XML is well-formed**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
xmllint --noout mod/aom_autorepair_test/game/data/gameplay/proto_mods.xml && echo OK
```

Expected: `OK`. If `xmllint` is not installed: `sudo apt install libxml2-utils`.

- [ ] **Step 4: Commit**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
git add mod/aom_autorepair_test/game/data/gameplay/proto_mods.xml
git commit -m "feat: minimal AutoRepair protoaction on VillagerGreek"
```

---

### Task 4: Verify the mod loads without errors

**Files:** none — pure observation.

**Why:** Independent of the in-game test of behavior, the mod must first parse and load. A parse error would block all later observation and is fast to detect.

- [ ] **Step 1: Enable the mod in the in-game Mod Manager**

Launch AoM:R via Steam. Title screen → `Mods` → `Mod Manager`. Locate `aom_autorepair_test` and click the toggle to enable it (button turns blue/green). Disable any other mods that modify `proto.xml` to avoid confounds; the two existing subscribed mods (Advanced Tooltips, Mythic UI) only affect strings and UI files so they are safe to leave enabled. Restart the game if prompted.

- [ ] **Step 2: Watch for error dialogs and log files**

After enabling, watch for any error dialog. Then check the log directory:

```bash
ls -la "/home/houtamelo/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/" | grep -iE 'log|crash'
```

Expected: identify the most recently modified log file. Tail it and look for any error mentioning `proto_mods`, `AutoRepair`, or `VillagerGreek`:

```bash
LOGDIR="/home/houtamelo/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736"
find "$LOGDIR" -name '*.log' -newer "$LOGDIR/myth-mod-status.json" -print -exec grep -iE 'autorepair|proto_mods|villagergreek|error' {} +
```

Expected output: either no matches (clean load — best case) OR error lines that point at our file. If errors appear, they are diagnostic data — record them in Task 6 and decide whether to fix-and-retry or abandon path 1.

- [ ] **Step 3: Decide based on observation**

If the mod fails to load with an XML/schema error specifically about our changes: jump to Task 6 (record outcome). The `<type>AutoRepair</type>` not being a recognized type might surface here OR might silently load — either is informative.

If the mod loads cleanly: proceed to Task 5.

---

### Task 5: In-game behavioral test

**Files:** none — observation only. Optionally screenshot to `docs/screenshots/`.

**Why:** This is the actual hypothesis test. We need a controlled environment where: (1) one Greek villager is idle, (2) one friendly building near it is damaged, (3) the auto-repair toggle is on. Then we observe whether the villager auto-tasks.

- [ ] **Step 1: Start a skirmish vs an Easy AI on a small map**

Title screen → `Single Player` → `Skirmish`. Set:
- Map: any small 1v1 map (e.g. `Mediterranean`)
- Civilization: Greek (any major god — Zeus is fine), to spawn a `VillagerGreek`
- AI: 1 opponent, Easy difficulty (we don't want them harassing during the test)
- Game speed: Slow (gives time to observe)
- Start the game.

- [ ] **Step 2: Locate one villager and inspect its command card**

Click any starting villager. In the bottom-right command card, look for a button corresponding to the AutoRepair action. The engine surfaces command-card buttons for protoactions automatically when they have an associated protoUnitCommand; whether one appears for our new protoaction is an important observation.

Possible outcomes to record:
- (a) **A new button appears** (likely using the `AutoRepairUnit` UI command and the "Repair this building." string). This is strong PASS evidence for path 1.
- (b) **No new button appears, but the existing `Repair` button now toggles** (right-click toggles auto-cast). Possible if the engine interprets `AutoRepair` as a configuration of `Repair`.
- (c) **No UI change at all.** Suggests the engine ignored the protoaction. PARTIAL or FAIL.

Take a screenshot regardless (default key is usually `F12` via Steam; otherwise use the system screenshot tool).

- [ ] **Step 3: If a toggle exists, enable it; otherwise skip to Step 5**

If outcome (a) or (b) from Step 2: enable auto-cast (left-click the button if (a); right-click if (b)). The button should change visual state to indicate "on" if the engine supports it.

- [ ] **Step 4: Damage a friendly building near the villager**

While the villager is idle and standing near (within 20 tiles of) a friendly building (e.g. the starting Town Center, House, or Storehouse): use a chat-cheat to damage that building. AoM:R has the cheat `WUV WOO` (does damage to selected own unit; selecting a building first applies it to the building). Select the building, type `WUV WOO` and press Enter to apply HP loss. Repeat several times until the building visibly takes damage and shows the damage effect.

If chat cheats are disabled in skirmish: alternative is to wait for the AI to attack one of your buildings, or to start a custom scenario in the editor with pre-damaged buildings.

- [ ] **Step 5: Observe the villager for ~15 seconds of in-game time**

Selected the idle villager again. Watch:
- Does the villager move toward the damaged building on its own?
- Does the villager play the build/repair animation upon arrival?
- Does the building's HP increase over time?
- Does the player's wood/gold stockpile decrease (vanilla repair costs resources)?

Record outcome: which of {move, animate, HP up, cost resources} occurred.

- [ ] **Step 6: Negative control — turn the toggle off and damage another building**

If a toggle was found in Step 2: turn it off, damage another nearby building, and confirm the villager does NOT auto-task. This rules out a coincidence where some other engine behavior (e.g. AI Villager Priority) drove the action.

---

### Task 6: Record outcome and decide next step

**Files:**
- Create: `docs/path1_results.md`

**Why:** Plan-as-experiment requires the result captured in writing so the next planning session has the data without having to re-observe.

- [ ] **Step 1: Write the results document**

```bash
cat > /home/houtamelo/Documents/projects/aom_retold_mod/docs/path1_results.md <<'EOF'
# Path 1 results — AutoRepair empirical test

Date: <YYYY-MM-DD>
Game build: <see Mod Manager → game version, e.g. 19.10938>

## Mod-load outcome (Task 4)
- Loaded cleanly: <yes / no>
- Errors observed: <none / quote any error lines>

## Command-card outcome (Task 5 step 2)
- New button appeared: <yes / no>
- Button label / icon: <description, screenshot path>
- Right-click toggled existing Repair: <yes / no / not tested>

## Behavioral outcome (Task 5 steps 4-5)
- Villager moved to damaged building: <yes / no>
- Repair animation played: <yes / no>
- Building HP recovered: <yes / no>
- Resources spent: <yes / no — and amount per HP if measurable>

## Negative control (Task 5 step 6)
- Toggle-off prevented auto-task: <yes / no / not tested>

## Verdict
- <PASS / PARTIAL / FAIL>

## If PASS — next step
Open follow-up plan to: extend protoaction to VillagerEgyptian, VillagerNorse,
VillagerAtlantean, VillagerChinese, the Egyptian Laborer, and the Norse
Hersir/Ulfsark/etc. Localize the `STR_PUC_AUTO_REPAIR` strings if they're
generic enough or add new ones. Decide whether NoIdleActions should be removed
from villagers.

## If PARTIAL — diagnostics
What worked: <list>
What didn't: <list>
Most-likely-fix: <hypothesis, e.g. "remove NoIdleActions flag and retry">

## If FAIL — fall-through to path 2 or 3
Decision: <path 2 (Heal-as-repair) / path 3 (AI-script hook)>
Rationale: <one sentence>
EOF
```

Fill in every angle-bracket placeholder based on the actual observations from Task 5.

- [ ] **Step 2: Commit the results**

```bash
cd /home/houtamelo/Documents/projects/aom_retold_mod
git add docs/path1_results.md
git commit -m "docs: record path 1 (AutoRepair) experiment results"
```

- [ ] **Step 3: Update the project memory with the verdict**

Per the auto-memory system, update `/home/houtamelo/.claude/projects/-home-houtamelo-Documents-projects-aom-retold-mod/memory/project_aom_retold_mod.md` with a one-line verdict under a new "Path 1 verdict" section so future Claude sessions don't re-investigate. Just edit that file directly with the Edit tool — append:

```
## Path 1 verdict (2026-05-XX)
<PASS / PARTIAL / FAIL>. <One-sentence summary>. Next: <path 2 / path 3 / extend mod>.
```

---

## Self-review

**Spec coverage:**
- "Test `<type>AutoRepair</type>` empirically" → Task 3 writes the action; Task 5 observes.
- "30 lines of XML" → Task 3's proto_mods.xml is exactly that.
- "Pure data, no XS, no per-map injection" → no XS files anywhere in the plan; the only data file is `proto_mods.xml`.
- "Decision: PASS / PARTIAL / FAIL" → Task 6 explicitly records this.
- Off-by-default toggle → `<active>0</active>` + `AutoCommandStartDisabled` flag (Task 3 step 2).
- Per-unit-instance toggle → relies on the engine's right-click toggle UI on auto-cast buttons; observation is part of Task 5.

**Placeholder scan:** no "TODO" / "fill in details" / "write tests for the above" entries. The angle-bracket placeholders in Task 6 step 1 are intentional — that file is a results template that gets filled in *during* Task 6, not a plan placeholder.

**Type/identifier consistency:** "VillagerGreek" used identically in Tasks 3 and 5. "AutoRepair" is the protoaction `<name>` AND `<type>`; that's fine because they live in different namespaces in proto.xml. Mod folder name `aom_autorepair_test` consistent in Tasks 0, 2, 3, 4. Symlink path consistent throughout.

**Unknown that could derail the plan:** the actual `proto_mods.xml` root-element name and merge-mode syntax (Task 1 sets up to verify; Task 3 step 2 explicitly flags the assumed syntax). If Task 1 surfaces a different syntax than `<protomods>` + `mode="merge"`, Task 3 step 2 must be re-edited before continuing.
