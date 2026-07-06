#!/usr/bin/env python3
"""
Build the test_targeting mod.

Reads source XMLs from source/ and writes deployed artifacts to game/:
  - source/proto_mods.xml -> game/data/gameplay/proto_mods.xml (verbatim)
  - source/tactics/*.xml -> game/data/gameplay/tactics/*.XMB (encoded)

Run from the mod/test_targeting directory:
    python3 build.py
"""
import shutil
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
ENCODER = REPO_ROOT / "scripts" / "encode_tactic_xmb.py"
SOURCE_DIR = Path(__file__).resolve().parent / "source"
DEPLOY_DIR = Path(__file__).resolve().parent / "game"


def main() -> int:
    if not ENCODER.is_file():
        print(f"error: encoder not found at {ENCODER}", file=sys.stderr)
        return 1

    # 1. Copy proto_mods.xml verbatim (the merge syntax is XML; the engine
    #    reads .xml directly without .XMB conversion).
    src_proto = SOURCE_DIR / "proto_mods.xml"
    dst_proto = DEPLOY_DIR / "data" / "gameplay" / "proto_mods.xml"
    dst_proto.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src_proto, dst_proto)
    print(f"copied  {src_proto.relative_to(SOURCE_DIR.parent)} -> {dst_proto.relative_to(DEPLOY_DIR.parent)}")

    # 2. Encode each source/tactics/*.xml to game/data/gameplay/tactics/*.XMB
    src_tactics_dir = SOURCE_DIR / "tactics"
    dst_tactics_dir = DEPLOY_DIR / "data" / "gameplay" / "tactics"
    dst_tactics_dir.mkdir(parents=True, exist_ok=True)
    import subprocess

    for src_xml in sorted(src_tactics_dir.glob("*.xml")):
        # source/tactics/cyclops_test.tactics.xml -> game/data/gameplay/tactics/cyclops_test.tactics.XMB
        xmb_name = src_xml.name.replace(".xml", ".XMB")
        dst_xmb = dst_tactics_dir / xmb_name
        result = subprocess.run(
            ["python3", str(ENCODER), str(src_xml), str(dst_xmb)],
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            print(f"FAILED  {src_xml.name}: {result.stderr}", file=sys.stderr)
            return 1
        print(f"encoded {src_xml.name} -> {xmb_name}")

    print()
    print(f"deploy directory: {DEPLOY_DIR}")
    print("ready for deployment via scripts/deploy-mods.sh")
    return 0


if __name__ == "__main__":
    sys.exit(main())