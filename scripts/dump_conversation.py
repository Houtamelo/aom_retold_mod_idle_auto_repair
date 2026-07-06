#!/usr/bin/env python3
"""Dump an OpenCode agent's full conversation to a single readable text file.

Usage:
    python3 dump_conversation.py <session_id> [output_file]
    python3 dump_conversation.py                                 # default session

Default session: only-olive-finch (the failed sub-agent from Pass 1).
"""

import sqlite3
import json
import sys
import os
from datetime import datetime, timezone

DEFAULT_SESSION = "ses_0ddd8d823ffeuxDYkxrt9Rrctf"
DEFAULT_OUTPUT = "conversation.txt"

# Mapping from part type to a readable role label
ROLE_LABELS = {
    "text": "ASSISTANT",
    "reasoning": "ASSISTANT (thinking)",
    "tool": "TOOL",
    "step-start": "step-start",
    "step-finish": "step-finish",
}

# Tools we want to show in full (otherwise the bash output is shown truncated)
TOOL_TRUNCATE_AT = 2000  # chars


def ms_to_iso(ms):
    """Convert Unix epoch milliseconds to ISO 8601 timestamp."""
    return datetime.fromtimestamp(ms / 1000, tz=timezone.utc).strftime(
        "%Y-%m-%dT%H:%M:%S.%fZ"
    )


def part_role_and_body(part_type, d):
    """Return (role_label, body_text) for a part's decoded data dict.

    The LLM's response stream is split by OpenCode into reasoning
    (think-block content) and text (everything else). When the LLM
    produces only a think block with no visible text after, the text
    part is just the close-tag fragment with no user-facing content.
    We detect that case and collapse it to a one-line marker.
    """
    if part_type == "text":
        text = d.get("text", "")
        # Detect "just the close tag of a think block" — common when
        # the LLM produces only thinking with no visible text after.
        if text.strip() == "</think>":
            return "(continuation of preceding reasoning — no visible text)", ""
        return ROLE_LABELS[part_type], text
    elif part_type == "reasoning":
        text = d.get("text", "")
        return ROLE_LABELS[part_type], text
    elif part_type == "tool":
        state = d.get("state", {})
        title = state.get("title", "(no title)")
        input_data = state.get("input", {})
        output = state.get("output", "")
        meta = state.get("metadata", {})

        body_lines = [f"Tool: {title}"]
        body_lines.append(f"Status: {state.get('status', '?')}")
        body_lines.append("")
        body_lines.append("INPUT:")
        body_lines.append(json.dumps(input_data, indent=2))
        body_lines.append("")
        body_lines.append("OUTPUT:")
        if len(output) > TOOL_TRUNCATE_AT:
            body_lines.append(
                f"[{len(output)} chars total — showing first {TOOL_TRUNCATE_AT}; "
                f"use dump_full_part.py for the rest]"
            )
            body_lines.append(output[:TOOL_TRUNCATE_AT])
            body_lines.append("... [truncated]")
        else:
            body_lines.append(output if output else "(no output)")
        return ROLE_LABELS[part_type], "\n".join(body_lines)
    elif part_type in ("step-start", "step-finish"):
        # Just a marker — show as a one-liner
        return ROLE_LABELS[part_type], json.dumps(d)
    else:
        return part_type.upper(), json.dumps(d, indent=2)


def main():
    session_id = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_SESSION
    output_path = sys.argv[2] if len(sys.argv) > 2 else DEFAULT_OUTPUT

    db_path = os.path.expanduser(
        "~/.local/share/opencode/opencode.db"
    )
    db = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    cur = db.cursor()

    cur.execute(
        """
        SELECT time_created,
               json_extract(data, '$.type') as t,
               data
        FROM part
        WHERE session_id = ?
        ORDER BY time_created
        """,
        (session_id,),
    )
    rows = cur.fetchall()
    if not rows:
        print(f"No parts found for session {session_id}")
        sys.exit(1)

    # Build the output
    out_lines = []
    out_lines.append(f"OpenCode session conversation dump")
    out_lines.append(f"Session ID:  {session_id}")
    out_lines.append(f"Total parts: {len(rows)}")
    out_lines.append(f"Started:     {ms_to_iso(rows[0][0])}")
    out_lines.append(f"Ended:       {ms_to_iso(rows[-1][0])}")
    out_lines.append("")
    out_lines.append("=" * 80)
    out_lines.append("")

    for i, (time_ms, part_type, data_str) in enumerate(rows):
        d = json.loads(data_str)
        role, body = part_role_and_body(part_type, d)
        ts = ms_to_iso(time_ms)

        # First text part is typically the user prompt
        if i == 0 and part_type == "text":
            role = "USER"

        out_lines.append(f"--- Part {i + 1}/{len(rows)} [{ts}] {role} ---")
        out_lines.append("")
        out_lines.append(body)
        out_lines.append("")
        out_lines.append("=" * 80)
        out_lines.append("")

    out_lines.append("")

    with open(output_path, "w") as f:
        f.write("\n".join(out_lines))

    print(f"Wrote {len(rows)} parts to {output_path}")
    print(f"File size: {os.path.getsize(output_path):,} bytes")


if __name__ == "__main__":
    main()
