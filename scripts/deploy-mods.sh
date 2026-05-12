#!/usr/bin/env bash
#
# deploy-mods.sh -- Copy mod source files into the AoMR local-mods folder.
#
# Synchronises all three local-deploy targets in one go:
#   - Idle Auto-Repair                       (auto_repair.xs + human_assist.xs)
#   - Intelligent Auto-Scout                 (auto_scout.xs  + human_assist.xs)
#   - Intelligent Auto-Repair and Scout      (auto_repair.xs + auto_scout.xs + unified human_assist.xs)
#
# Refuses to run while AoMR is open (overwriting loaded mod scripts mid-session
# can corrupt the running game per the no-game-folder-writes house rule).
#
# Run from the host shell -- NOT from inside the claude-sandbox container.
# The sandbox doesn't bind-mount ~/.steam, so it cannot reach the deploy root.
#
# Environment overrides:
#   AOMR_LOCAL_MODS   path to the mods/local/ folder in the Proton prefix.
#                     Defaults to the typical Steam Deck-style path on Linux.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC_ROOT="$REPO_ROOT/mod"

DEPLOY_ROOT="${AOMR_LOCAL_MODS:-$HOME/.steam/debian-installation/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local}"

# --- preflight ---------------------------------------------------------------

if pgrep -x AoMRT_s.exe > /dev/null; then
   echo "ERROR: AoMRT_s.exe is running. Close AoMR before deploying." >&2
   exit 1
fi

if [ ! -d "$DEPLOY_ROOT" ]; then
   echo "ERROR: deploy root not found: $DEPLOY_ROOT" >&2
   echo "Set AOMR_LOCAL_MODS env var to override." >&2
   exit 1
fi

# --- deploy ------------------------------------------------------------------

deploy() {
   local src="$1" dest="$2"
   if [ ! -f "$src" ]; then
      echo "  SKIP (missing source): $src" >&2
      return 1
   fi
   mkdir -p "$(dirname "$dest")"
   cp "$src" "$dest"
   printf "  %s  %s\n" "$(md5sum "$dest" | awk '{print $1}')" "$dest"
}

REPAIR_SRC="$SRC_ROOT/idle_auto_repair/game/ai/human_assist"
SCOUT_SRC="$SRC_ROOT/intelligent_auto_scout/game/ai/human_assist"
COMBINED_SRC="$SRC_ROOT/intelligent_auto_repair_and_scout/game/ai/human_assist"

echo "=== Idle Auto-Repair ==="
deploy "$REPAIR_SRC/auto_repair.xs"   "$DEPLOY_ROOT/Idle Auto-Repair/game/ai/human_assist/auto_repair.xs"
deploy "$REPAIR_SRC/human_assist.xs"  "$DEPLOY_ROOT/Idle Auto-Repair/game/ai/human_assist/human_assist.xs"

echo "=== Intelligent Auto-Scout ==="
deploy "$SCOUT_SRC/auto_scout.xs"     "$DEPLOY_ROOT/Intelligent Auto-Scout/game/ai/human_assist/auto_scout.xs"
deploy "$SCOUT_SRC/human_assist.xs"   "$DEPLOY_ROOT/Intelligent Auto-Scout/game/ai/human_assist/human_assist.xs"

echo "=== Intelligent Auto-Repair and Scout ==="
deploy "$REPAIR_SRC/auto_repair.xs"        "$DEPLOY_ROOT/Intelligent Auto-Repair and Scout/game/ai/human_assist/auto_repair.xs"
deploy "$SCOUT_SRC/auto_scout.xs"          "$DEPLOY_ROOT/Intelligent Auto-Repair and Scout/game/ai/human_assist/auto_scout.xs"
deploy "$COMBINED_SRC/human_assist.xs"     "$DEPLOY_ROOT/Intelligent Auto-Repair and Scout/game/ai/human_assist/human_assist.xs"

echo "Done."
