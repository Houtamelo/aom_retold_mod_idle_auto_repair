#!/usr/bin/env bash
#
# deploy-mods.sh -- Copy mod source files into the AoMR local-mods folder.
#
# Synchronises all five local-deploy targets in one go:
#   - Idle Auto-Repair                       (auto_repair.xs + human_assist.xs)
#   - Intelligent Auto-Scout                 (auto_scout.xs  + human_assist.xs)
#   - Intelligent Auto-Repair and Scout      (auto_repair.xs + auto_scout.xs + unified human_assist.xs)
#   - Human Assist Improvements              (auto_repair.xs + auto_scout.xs + unified human_assist.xs)
#   - Extra Ai + AoModAi                     (full game/ai/ tree, 22 files)
#
# Caller is responsible for ensuring AoMR is closed -- overwriting loaded
# mod scripts mid-session can corrupt the running game.
#
# Environment overrides:
#   AOMR_LOCAL_MODS   path to the mods/local/ folder in the Proton prefix.
#                     Defaults to the typical Steam Deck-style path on Linux.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC_ROOT="$REPO_ROOT/mod"

DEPLOY_ROOT="${AOMR_LOCAL_MODS:-$HOME/.steam/steam/steamapps/compatdata/1934680/pfx/drive_c/users/steamuser/Games/Age of Mythology Retold/76561198001426736/mods/local}"

# --- preflight ---------------------------------------------------------------

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

# Recursively copy a directory tree, printing one md5 line per file.
# Used for mods whose overlay is a whole subtree (e.g. Extra Ai + AoModAi's
# 22 files under game/ai/core/...) rather than a single human_assist.xs.
deploy_tree() {
   local src="$1" dest="$2"
   if [ ! -d "$src" ]; then
      echo "  SKIP (missing source): $src" >&2
      return 1
   fi
   mkdir -p "$dest"
   local count=0
   while IFS= read -r -d '' file; do
      rel="${file#"$src"/}"
      dest_file="$dest/$rel"
      mkdir -p "$(dirname "$dest_file")"
      cp "$file" "$dest_file"
      printf "  %s  %s\n" "$(md5sum "$dest_file" | awk '{print $1}')" "$dest_file"
      count=$((count+1))
   done < <(find "$src" -type f -print0)
   echo "  Total: $count files"
}

REPAIR_SRC="$SRC_ROOT/idle_auto_repair/game/ai/human_assist"
SCOUT_SRC="$SRC_ROOT/intelligent_auto_scout/game/ai/human_assist"
COMBINED_SRC="$SRC_ROOT/intelligent_auto_repair_and_scout/game/ai/human_assist"
HUMAN_SRC="$SRC_ROOT/human_assist_improvements/game/ai/human_assist"

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

echo "=== Human Assist Improvements ==="
deploy "$REPAIR_SRC/auto_repair.xs"        "$DEPLOY_ROOT/Human Assist Improvements/game/ai/human_assist/auto_repair.xs"
deploy "$SCOUT_SRC/auto_scout.xs"          "$DEPLOY_ROOT/Human Assist Improvements/game/ai/human_assist/auto_scout.xs"
deploy "$HUMAN_SRC/human_assist.xs"        "$DEPLOY_ROOT/Human Assist Improvements/game/ai/human_assist/human_assist.xs"
deploy "$HUMAN_SRC/auto_relic_delivery.xs" "$DEPLOY_ROOT/Human Assist Improvements/game/ai/human_assist/auto_relic_delivery.xs"

echo "=== Extra Ai + AoModAi ==="
EXTRA_AI_SRC="$SRC_ROOT/Extra Ai + AoModAi/game/ai"
EXTRA_AI_DEST="$DEPLOY_ROOT/Extra Ai + AoModAi/game/ai"
deploy_tree "$EXTRA_AI_SRC" "$EXTRA_AI_DEST"

echo "Done."
