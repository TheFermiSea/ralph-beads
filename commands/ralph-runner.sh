#!/usr/bin/env bash
# Safety wrapper to run /ralph-beads with guaranteed cleanup

set -euo pipefail

REPO_ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
TRACKER_FILE="$REPO_ROOT/.ralph-worktree-tracker"

cleanup() {
  if [ -f "$TRACKER_FILE" ]; then
    WORKTREE_PATH=$(cat "$TRACKER_FILE")
    if [ -d "$WORKTREE_PATH" ]; then
      echo "Cleaning up worktree: $WORKTREE_PATH"
      (cd "$REPO_ROOT" && git worktree remove --force "$WORKTREE_PATH") || true
    fi
    rm -f "$TRACKER_FILE"
  fi
}
trap cleanup EXIT INT TERM

# Ensure clean start
rm -f "$TRACKER_FILE"

CLAUDE_BIN=${CLAUDE_BIN:-claude}
if ! command -v "$CLAUDE_BIN" >/dev/null 2>&1; then
  echo "ERROR: Claude CLI not found (set CLAUDE_BIN to override)" >&2
  exit 1
fi

"$CLAUDE_BIN" /ralph-beads "$@"
