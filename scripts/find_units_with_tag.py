#!/usr/bin/env python3
"""
List all <unit> entries in AoM:R's proto.xml that carry a given <unittype> tag.

Usage:
  ./find_units_with_tag.py <tag> [<tag> ...]
  ./find_units_with_tag.py MythUnitSiege
  ./find_units_with_tag.py MythUnitSiege AbstractSiegeWeapon LogicalTypeAutoAttackFocusBuildings
  ./find_units_with_tag.py MythUnitMelee --invert           # units WITHOUT MythUnitMelee
  ./find_units_with_tag.py MythUnitSiege --context          # show each unit's tag list

Default input: ./extracted/gameplay/proto.xml (relative to repo root).
Override with --file PATH.
"""
import argparse
import re
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


def build_unit_line_map(proto_path: Path) -> dict[str, int]:
    """Map each unit name to its line number in the file.

    Done via regex because ElementTree doesn't expose sourceline on Elements
    without a custom parser. Each <unit name="X"> opens a block; we record the
    line of the opening tag.
    """
    name_re = re.compile(r'<unit\s+name="([^"]+)"')
    mapping: dict[str, int] = {}
    with proto_path.open("r", encoding="utf-8") as f:
        for line_no, line in enumerate(f, start=1):
            match = name_re.search(line)
            if match is None:
                continue
            name = match.group(1)
            # If the same name appears twice (e.g., <unit> redefinition later
            # in the file), the last occurrence wins.
            mapping[name] = line_no
    return mapping


def find_units_with_tags(
    proto_path: Path,
    query_tags: list[str],
    line_map: dict[str, int],
    show_context: bool,
    invert: bool,
) -> list[tuple[str, int, list[str]]]:
    """Return list of (unit_name, line_no, all_tags) for matching units.

    Match semantics: a unit matches if it carries AT LEAST ONE of the query tags
    (unless invert=True, in which case it matches if it carries NONE of them).
    """
    query_set = set(query_tags)
    matches: list[tuple[str, int, list[str]]] = []

    for _, elem in ET.iterparse(proto_path, events=("end",)):
        if elem.tag != "unit":
            continue
        name = elem.get("name")
        if name is None:
            continue
        unit_tags = [
            child.text.strip()
            for child in elem.findall("unittype")
            if child.text
        ]
        has_any = any(t in query_set for t in unit_tags)
        if (not has_any) if invert else has_any:
            matches.append((name, line_map.get(name, -1), unit_tags))

    return matches


def main() -> int:
    repo_root = Path(__file__).resolve().parents[1]  # scripts/ -> repo root
    default_proto = repo_root / "extracted" / "gameplay" / "proto.xml"

    parser = argparse.ArgumentParser(
        description="List AoM:R units that have (or lack) specific <unittype> tags."
    )
    parser.add_argument(
        "tags",
        nargs="+",
        help="One or more <unittype> tag names to search for.",
    )
    parser.add_argument(
        "--file",
        type=Path,
        default=default_proto,
        help=f"Path to proto.xml (default: {default_proto})",
    )
    parser.add_argument(
        "--context",
        action="store_true",
        help="Show the unit's full <unittype> list for each match.",
    )
    parser.add_argument(
        "--invert",
        action="store_true",
        help="Show units that have NONE of the given tags.",
    )
    args = parser.parse_args()

    if not args.file.is_file():
        print(f"error: {args.file} does not exist", file=sys.stderr)
        return 1

    line_map = build_unit_line_map(args.file)
    matches = find_units_with_tags(
        args.file, args.tags, line_map, args.context, args.invert
    )

    verb = "lacking" if args.invert else "with"
    if len(args.tags) == 1:
        header = f"{len(matches)} unit(s) {verb} <unittype>{args.tags[0]}</unittype>"
    else:
        header = f"{len(matches)} unit(s) {verb} any of {args.tags}"
    print(header)
    print()

    query_set = set(args.tags)
    for name, line, tags in sorted(matches, key=lambda m: m[0].lower()):
        print(f"  {name}   (line {line})")
        if args.context:
            for t in tags:
                marker = "*" if t in query_set else " "
                print(f"      {marker} {t}")
            print()

    return 0


if __name__ == "__main__":
    sys.exit(main())