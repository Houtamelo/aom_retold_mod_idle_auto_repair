---
name: playtest-log-analysis
description: "Trigger: analyze AI playtest log, parse AI output, summarize large log file, playtest results, AoM:Retold AI log. Split large AI log files into chunks and dispatch parallel agents to produce timelines and flag suspicious behaviors."
license: Apache-2.0
metadata:
  author: aom-retold-mod
  version: "1.0"
---

# When to Use

- AI playtest produced a log file too large to read in one pass (>3000 lines).
- User wants a timeline + suspicious-behavior list from an AI debug log.
- Log may be UTF-16LE (AoM:Retold engine default on Windows).

# Hard Rules

1. **Detect encoding first.** If `head -3 file.txt` shows every ASCII char followed by what looks like spaces or null bytes, the file is UTF-16LE. Convert before processing.
2. **Use 3000 lines/chunk.** Smaller (500) is too granular — agents only see 30-60s of game time, can't distinguish routine from anomalous. Larger (5000+) risks context bloat per agent.
3. **Dispatch via `delegate` (async, parallel).** All agents run simultaneously. Use `agent: "general"`.
4. **Quote exact log lines.** Every timeline entry and suspicious-behavior note must include the log's timestamp prefix (e.g. `00:05:30 (312182)`).
5. **Synthesize by issue, not by chunk.** Aggregate findings across all agents into one global timeline + one issue list. Each issue will naturally gather evidence from multiple chunks.
6. **Don't pre-trust proactive API verification.** If you grep to verify function names exist, use `grep -rE "\b${func}\("` (with the opening paren) — a bare `grep "funcName"` matches prefixes and gave false positives in the wall-port playtest.

# Decision Tree

```
Log > 3000 lines?        → Yes: chunk and dispatch
                         → No:  read directly, skip this skill

File UTF-16LE?           → Yes: iconv -f UTF-16LE -t UTF-8 first
                         → No:  use as-is

How many chunks?         → lines / 3000, round up (min 2, max ~12)
                         → 22k lines = 8 chunks of ~2768

Agent type?              → "general" via delegate (async, parallel)

What to synthesize?      → 1 global timeline (by timestamp)
                         → 1 issue list (by issue, with cross-chunk evidence)
```

# Execution Steps

## Step 1: Detect encoding and convert

```bash
# Detect: if head shows null bytes between chars, it's UTF-16LE
head -3 "$LOG_FILE"

# Convert if needed
iconv -f UTF-16LE -t UTF-8 "$LOG_FILE" > "$LOG_FILE.utf8"

# Verify line count preserved
wc -l "$LOG_FILE" "$LOG_FILE.utf8"
```

## Step 2: Split into chunks

```bash
TOTAL=$(wc -l < "$LOG_FILE.utf8")
CHUNK_SIZE=$(( (TOTAL + 7) / 8 ))  # round up for 8 chunks
split -l $CHUNK_SIZE -d -a 2 "$LOG_FILE.utf8" "$TMP_DIR/chunk_"
# Produces: chunk_00, chunk_01, ..., chunk_07
```

For non-22k-line files, adjust chunk count: `CHUNK_SIZE=$(( TOTAL / DESIRED_AGENTS ))` where `DESIRED_AGENTS` targets ~3000 lines per agent.

## Step 3: Dispatch parallel agents

Launch N `delegate` calls in a single message (all parallel). Each agent gets the prompt template from `assets/agent-prompt-template.md` with the chunk file path and line range substituted.

Key prompt structure per agent (5 sections):
1. **Chunk Timeline** — bullet points with log timestamps
2. **Specific Rule Activity** — grep for the feature(s) being tested (e.g. `secondRingWallPlanMonitor`, `debugSecondRing`)
3. **Errors / Warnings** — quote exact lines
4. **Suspicious / Interesting Behaviors** — with timestamps
5. **Chunk Summary** — 2-3 sentences (game time range, activity level)

## Step 4: Synthesize

When all agents return:

1. Read all N delegation results via `delegation_read`.
2. Merge timelines by timestamp (each agent's bullets already have `MM:SS` prefix from the log).
3. Aggregate issues: each issue will naturally appear in multiple chunks — combine evidence.
4. Cross-reference against any spec scenarios (e.g. B1-B12 from the SDD change) to validate feature behavior.
5. Report: one global timeline + one issue list, ranked by severity.

# Resources

- Agent prompt template: [assets/agent-prompt-template.md](assets/agent-prompt-template.md)
- Example log file (this playtest): `.tmp/MythRetoldAIOutputPlayer2.utf8.txt` (22,144 lines)
- Example issues catalog (this playtest): `docs/playtest_2026-06-19_ai_issues.md`
- Engram pattern: `pattern/playtest-analysis-parallel-chunk-dispatch-workflow`
