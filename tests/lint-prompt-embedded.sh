#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
PROMPT_FILE="$ROOT/commands/ralph-beads.md"
TEMP_DIR=$(mktemp -d)

echo "== Extracting Bash blocks from $PROMPT_FILE =="

awk '
  /^```bash/ { file = sprintf("%s/block_%03d.sh", dir, ++n); print "#!/bin/bash" > file; next }
  /^```/ { file = "" }
  file { print > file }
' dir="$TEMP_DIR" "$PROMPT_FILE"

FAILURES=0
if ls "$TEMP_DIR"/*.sh >/dev/null 2>&1; then
  for f in "$TEMP_DIR"/*.sh; do
    if grep -q "<" "$f"; then
      echo "SKIP: placeholder block $(basename "$f")"
      continue
    fi
    if command -v shellcheck >/dev/null 2>&1; then
      if ! shellcheck -e SC2154,SC2034,SC1072,SC1073,SC1009,SC2260,SC2086,SC2105,SC2261,SC2164 "$f"; then
        echo "FAIL: Syntax error in bash block $(basename "$f")"
        cat -n "$f"
        FAILURES=1
      fi
    else
      echo "SKIP: shellcheck not installed; skipping $(basename "$f")"
    fi
  done
fi

rm -rf "$TEMP_DIR"

if [ "$FAILURES" -eq 0 ]; then
  echo "PASS: All embedded scripts valid."
else
  exit 1
fi
