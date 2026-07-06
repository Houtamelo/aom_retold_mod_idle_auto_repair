import sqlite3, json
import sys

db = sqlite3.connect('file:/home/houtamelo/.local/share/opencode/opencode.db?mode=ro', uri=True)
cur = db.cursor()

# Usage: python3 dump_full_part.py <part_id>
# e.g. python3 dump_full_part.py prt_f22283a80001adu6zSw0AAFRWp
if len(sys.argv) < 2:
    print("Usage: python3 dump_full_part.py <part_id>")
    sys.exit(1)

part_id = sys.argv[1]
cur.execute("SELECT json_extract(data, '$.type') as t, data FROM part WHERE id = ?", (part_id,))
row = cur.fetchone()
if not row:
    print(f"No part found with id {part_id}")
    sys.exit(1)

part_type, data_str = row
d = json.loads(data_str)
print(f"=== Part {part_id} (type={part_type}) ===\n")

if part_type == "tool":
    # Tool parts have: state.input (command), state.output (result), state.title
    state = d.get("state", {})
    print(f"TITLE: {state.get('title', '(none)')}")
    print(f"\nINPUT:")
    print(json.dumps(state.get("input", {}), indent=2))
    print(f"\nOUTPUT:")
    output = state.get("output", "")
    if len(output) > 5000:
        print(f"({len(output)} chars total — showing first 5000)")
        print(output[:5000])
        print("... [truncated]")
    else:
        print(output)
elif part_type in ("text", "reasoning"):
    text = d.get("text", "")
    print(text)
else:
    # step-start, step-finish, etc.
    print(json.dumps(d, indent=2))
