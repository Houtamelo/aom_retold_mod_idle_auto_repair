# Per-Agent Prompt Template for Playtest Log Analysis

This is the prompt to send to each parallel `delegate` agent (type: `"general"`). Replace `{CHUNK_FILE}`, `{LINE_START}`, `{LINE_END}`, `{CHUNK_NUM}`, `{TOTAL_CHUNKS}`, and `{RULES}` with actual values.

---

You are analyzing a chunk of an Age of Mythology: Retold AI log file. The game was played with the "Extra Ai + AoModAi" mod loaded.

Read the file at `{CHUNK_FILE}` (lines {LINE_START}-{LINE_END} of the full log). This is chunk {CHUNK_NUM} of {TOTAL_CHUNKS} from a large log file{LAST_CHUNK_NOTE}.

Produce a structured analysis with these sections:

## 1. Chunk Timeline (bullet points)

Key events in chronological order: game start, age-ups, major build decisions, army movements, attacks, base events. One bullet per notable event with the timestamp prefix from the log (e.g., `00:05:30`).

## 2. Rule Activity

{RULES}

Any mentions of:
- The rules listed above running
- Debug messages from those rules
- Plan creation/destruction by those rules
- Any messages about wall rings, attack-cancellation, rusher delay, lifetime expiry

Quote the exact log lines with timestamps.

## 3. Errors / Warnings

Any XS errors, rule failures, parse errors, or unusual warnings. Quote exact lines.

## 4. Suspicious / Interesting Behaviors

Anything that looks odd: AI stuck in a loop, plans created then immediately destroyed, units idle, AI not responding to attacks, weird timing between events. Include timestamps.

## 5. Chunk Summary (2-3 sentences)

A concise summary of what this chunk covers (game time range, overall activity level){END_NOTE}.

Be thorough but concise. Quote exact log lines for anything important. Use timestamps from the log for all timeline entries.

---

## Usage examples

### For the wall-port playtest (looking for `secondRingWallPlanMonitor`)

Replace `{RULES}` with:

```
Search for these specific rules and patterns:
- `secondRingWallPlanMonitor` rule running
- `debugSecondRing` messages (from the mod's wall feature)
- "2nd ring", "second ring", "wall plan" creation/destruction
- Attack-cancellation messages ("sustained attack", "Destroyed 2nd ring wall plan")
- Rusher delay messages
- 12-minute lifetime expiry
- Wall ring creation attempts
```

### For a generic AI analysis (no specific rules)

Replace `{RULES}` with:

```
Search for any notable rule activity:
- Rule starts/stops (lines matching `--- Running Rule`)
- god power casts (`God power was cast`)
- age-ups (`Strategy:.*ending`, `Activated new strategy`)
- attack launches (`LAUNCHING ATTACK`)
- base events (TC built/destroyed, settlements claimed/lost)
- resign monitor (`resignMonitor`, `healthScore`)
```

## Chunk calculations

For a 22,144-line file split into 8 chunks:
- Chunk size: `(22144 + 7) / 8 = 2768` lines per chunk
- Chunk 0: lines 1-2768
- Chunk 1: lines 2769-5536
- Chunk 2: lines 5537-8304
- ...
- Chunk 7: lines 19377-22144 (FINAL chunk — add `{LAST_CHUNK_NOTE} = " This is the FINAL chunk — it should cover the late game / end of match."` and `{END_NOTE} = ". Note if this chunk covers the end of the match"`)

For other file sizes, compute: `CHUNK_SIZE = ceil(TOTAL_LINES / 8)`.
