import sqlite3, json

db = sqlite3.connect('file:/home/houtamelo/.local/share/opencode/opencode.db?mode=ro', uri=True)
cur = db.cursor()

# Query all parts for the failed agent session
cur.execute("""
    SELECT time_created,
           json_extract(data, '$.type') as t,
           length(data) as size,
           data
    FROM part
    WHERE session_id = 'ses_0ddd8d823ffeuxDYkxrt9Rrctf'
    ORDER BY time_created
""")

# Fetch all rows once (cursors are forward-only and get exhausted after one pass)
rows = cur.fetchall()

# Count types
type_counts = {}
for time_created, part_type, size, data_str in rows:
    type_counts[part_type] = type_counts.get(part_type, 0) + 1

print("=== Part type counts ===")
for t, n in sorted(type_counts.items()):
    print(f"  {t}: {n}")
print()

# Print a summary of each part (small enough for terminal)
print("=== Part summary (chronological) ===")
for time_created, part_type, size, data_str in rows:
    d = json.loads(data_str)
    title = ""
    if part_type == "tool":
        # The tool name lives at data->state->title
        title = d.get("state", {}).get("title", "")
    elif part_type == "text":
        # Show a snippet of the text
        text = d.get("text", "")
        snippet = text[:120].replace("\n", " ")
        if len(text) > 120:
            snippet += "..."
        title = f'"{snippet}"'
    elif part_type == "reasoning":
        text = d.get("text", "")
        snippet = text[:120].replace("\n", " ")
        if len(text) > 120:
            snippet += "..."
        title = f'[reason] "{snippet}"'
    print(f"  t={time_created}  type={part_type:14}  size={size:6}  {title}")
