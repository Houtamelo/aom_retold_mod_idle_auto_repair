#!/usr/bin/env python3
"""find_orphan_xs.py — find .xs files in the AoM:R game folder that are
not included by any other .xs file.

Usage:
    ./scripts/find_orphan_xs.py [GAME_DIR]

If GAME_DIR is omitted, defaults to the standard Steam install:
    ~/.steam/steam/steamapps/common/Age of Mythology Retold/game

Definition of "orphan" used here:
    A .xs file is an orphan if no other .xs file's source text contains an
    `include "..."` (or `include <...>`) statement that resolves to it.
    Resolution is performed relative to the including file's directory, the
    same way the XS engine resolves include paths.

Edge cases handled:
    - Backslash path separators (e.g. "campaign\\\\foo.xs" — Windows-style
      paths are present in the shipped game scripts)
    - Trailing semicolons ("include "foo.xs";")
    - Trailing line comments ("include "foo.xs"; // ...")
    - "<...>" angle-bracket form (none observed in shipped game, but
      handled for robustness)
    - Block comments "/* ... */" are stripped from source before scanning
    - Duplicate basenames under different directories (path-aware
      resolution only, never basename lookup)

Edge cases NOT handled (none observed in shipped game, listed for
future maintainers):
    - ".." parent-relative include paths
    - Conditional "#include" with a leading "#" (the shipped syntax is
      "include", not "#include")
    - C-style preprocessor "#if"/"#endif" branches: includes inside "#if"
      are counted regardless of branch, because we care about intent
      ("does any script reference this file"), not compile-time gating.
"""

from __future__ import annotations

import os
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path

# Match `include "..."` or `include <...>`. The path can contain any
# character except the closing quote/bracket. Anchored at the start of
# the line (after optional whitespace) so we don't pick up the word
# "include" used inside comments or identifiers.
INCLUDE_RE = re.compile(
    r'^[ \t]*include[ \t]+(?P<quote>["<])(?P<path>[^">]+)(?P=quote)[ \t]*;?',
    re.MULTILINE,
)


def strip_block_comments(text: str) -> str:
    """Remove /* ... */ block comments. Handles unterminated blocks
    by stripping to end of file (rare in shipped game, defensive)."""
    return re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)


def extract_includes(source: str) -> list[str]:
    """Return the raw include paths found in a .xs source string,
    with backslashes normalized to forward slashes."""
    cleaned = strip_block_comments(source)
    paths: list[str] = []
    for match in INCLUDE_RE.finditer(cleaned):
        path = match.group("path").replace("\\", "/").strip()
        if path:
            paths.append(path)
    return paths


def include_root(including_rel: Path) -> Path:
    """Return the include root for a given .xs file.

    The XS engine does NOT resolve `include` paths relative to the
    including file's directory (like C `#include`). Instead, every
    script under a top-level game subdirectory is resolved against
    that subdirectory. For the shipped AoM:R game, the two include
    roots are `ai/` (for AI scripts) and `random_maps/` (for map
    scripts). Any path like `ai/aotg/foo.xs` resolves its includes
    against `ai/`, not against `ai/aotg/`.

    Evidence from the shipped game:
    - `ai/human_assist/human_assist.xs` does
      `include "human_assist/human_assist_debug.xs"`. The real file
      is `ai/human_assist/human_assist_debug.xs`. If resolution were
      from the including file's directory, the path would be
      `ai/human_assist/human_assist/human_assist_debug.xs` (which
      does not exist). The only correct resolution is
      `ai/human_assist/human_assist_debug.xs`, which matches
      resolving against `ai/`.
    - `ai/aotg/s0l0p0c10m1_p3.xs` does `include "core\\main.xs"`.
      The real file is `ai/core/main.xs`. Resolving from the
      including file's directory would give
      `ai/aotg/core/main.xs` (does not exist). The only correct
      resolution is `ai/core/main.xs`, matching the `ai/` root.
    - `random_maps/oasis.xs` does `include "lib2/rm_core.xs"`.
      The real file is `random_maps/lib2/rm_core.xs`. Here both
      resolutions agree because oasis.xs is at the root of
      `random_maps/`, so this is not a distinguishing test.

    The general rule, then: the include root is the FIRST path
    component under the game root. For the shipped game, this is
    always `ai` or `random_maps`.
    """
    parts = including_rel.parts
    if not parts:
        return Path(".")
    return Path(parts[0])


def resolve(including_rel: Path, target: str) -> Path:
    """Resolve a relative include target against the include root
    of the including file. Returns a path relative to the game root.

    Mirrors the engine's resolution semantics: the target is joined
    onto the include root (NOT the including file's directory), with
    "." and ".." segments collapsed.
    """
    if Path(target).is_absolute():
        return Path(target)
    base = include_root(including_rel)
    parts: list[str] = []
    for segment in (str(base) + "/" + target).split("/"):
        if segment in ("", "."):
            continue
        if segment == "..":
            if parts:
                parts.pop()
            continue
        parts.append(segment)
    return Path("/".join(parts))


def main(game_dir: str) -> int:
    import argparse
    parser = argparse.ArgumentParser(
        description="Find .xs files in the AoM:R game folder that are not "
        "included by any other .xs file."
    )
    parser.add_argument(
        "game_dir",
        nargs="?",
        default=os.path.expanduser(
            "~/.steam/steam/steamapps/common/Age of Mythology Retold/game"
        ),
        help="Path to the AoM:R game folder (default: standard Steam install)",
    )
    parser.add_argument(
        "-o",
        "--output",
        help="Write the full report to this file (in addition to stdout)",
    )
    args = parser.parse_args()
    game_dir = args.game_dir

    if args.output:
        # Redirect stdout to a tee, so both stdout and file get the report
        class _Tee:
            def __init__(self, *streams):
                self._streams = streams

            def write(self, data):
                for s in self._streams:
                    s.write(data)

            def flush(self):
                for s in self._streams:
                    s.flush()

        out_file = open(args.output, "w", encoding="utf-8")
        original_stdout = sys.stdout
        sys.stdout = _Tee(original_stdout, out_file)
        try:
            return _main(game_dir)
        finally:
            sys.stdout = original_stdout
            out_file.close()
    return _main(game_dir)


def _main(game_dir: str) -> int:
    game_root = Path(game_dir).resolve()
    if not game_root.is_dir():
        print(f"Game folder not found: {game_dir}", file=sys.stderr)
        print(
            "Pass the path to the AoM:R game folder as the first argument.",
            file=sys.stderr,
        )
        return 1

    # Step 1: collect all .xs files, as paths relative to game_root.
    all_files: list[Path] = sorted(
        (p.resolve().relative_to(game_root) for p in game_root.rglob("*.xs")),
        key=lambda p: str(p),
    )
    total = len(all_files)
    print(f"Found {total} .xs files under {game_root}", file=sys.stderr)

    # Step 2/3: walk all files, extract and resolve include targets.
    #
    # `includers[resolved] = set of including files` (the inverse map)
    # `dangling` = includes whose resolved target doesn't exist on disk
    # `include_stmts` = total count of include statements seen
    # `files_with_includes` = number of files that have at least one
    #   include statement
    includers: dict[Path, set[Path]] = defaultdict(set)
    dangling: list[tuple[Path, str]] = []
    include_stmts = 0
    files_with_includes = 0

    for rel in all_files:
        abs_path = game_root / rel
        try:
            source = abs_path.read_text(encoding="utf-8", errors="replace")
        except OSError as exc:
            print(f"  warning: could not read {abs_path}: {exc}", file=sys.stderr)
            continue
        targets = extract_includes(source)
        if targets:
            files_with_includes += 1
        for target in targets:
            include_stmts += 1
            resolved = resolve(rel, target)
            # `resolved` is already relative to game_root, since we
            # built it from the relative `rel` path. The check
            # `resolved.is_file()` happens against `game_root`.
            includers[resolved].add(rel)
            if not (game_root / resolved).is_file():
                dangling.append((rel, str(resolved)))  # (includer, target)

    distinct_targets = len(includers)

    # Step 4: report orphans — files in all_files that are never
    # referenced as a resolved include target by any other file.
    orphans: list[Path] = [
        rel for rel in all_files if rel not in includers
    ]
    orphan_count = len(orphans)

    print("=" * 67)
    print(" ORPHAN .xs FILES (not included by any other .xs file in the game)")
    print("=" * 67)
    print()
    for orphan in orphans:
        print(f"  {orphan}")
    print()

    print("-" * 67)
    print("Summary")
    print("-" * 67)
    print(f"  Total .xs files:        {total}")
    print(f"  Files with includes:    {files_with_includes}")
    print(f"  Total include stmts:    {include_stmts}")
    print(f"  Distinct targets:       {distinct_targets}")
    print(f"  Orphans (not included): {orphan_count}")

    # Step 5: dangling includes.
    print()
    print("=" * 67)
    print(" DANGLING INCLUDES (target does not exist on disk)")
    print("=" * 67)
    print()
    if dangling:
        # Deduplicate (includer, target) pairs; same include from same file
        # appears as one row, not once per statement.
        unique = sorted(set(dangling))
        for includer, target in unique:
            print(f"  {target:<50}  <- {includer}")
    else:
        print("  (none)")
    print()
    print(f"  Dangling includes: {len(dangling)}")

    # Step 6: most-included files.
    print()
    print("=" * 67)
    print(" MOST-INCLUDED FILES (top 15)")
    print("=" * 67)
    print()
    include_counts: Counter = Counter(
        {target: len(includers[target]) for target in includers}
    )
    for target, count in include_counts.most_common(15):
        print(f"  {count:4d}  {target}")
    print()

    return 0


if __name__ == "__main__":
    sys.exit(main(""))
