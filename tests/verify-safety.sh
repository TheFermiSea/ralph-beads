#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
RUNNER="$ROOT/commands/ralph-runner.sh"
TEST_DIR=$(mktemp -d)
MOCK_CLAUDE="$TEST_DIR/mock_claude"

cd "$TEST_DIR"
git init -q --initial-branch=main
git config user.email "test@example.com"
git config user.name "Test"
touch README.md && git add README.md && git commit -q -m "init"

cat > "$MOCK_CLAUDE" <<'EOF'
#!/bin/bash
git worktree add ../worktree-test -b feature/test >/dev/null 2>&1
echo "$(pwd)/../worktree-test" > .ralph-worktree-tracker
sleep 10
EOF
chmod +x "$MOCK_CLAUDE"

echo "== Testing Signal Handling & Cleanup =="
CLAUDE_BIN="$MOCK_CLAUDE" "$RUNNER" --dry-run "safety-test" &
PID=$!

sleep 2
if [ ! -d "../worktree-test" ]; then
  echo "FAIL: Mock did not create worktree"
  kill -9 $PID || true
  rm -rf "$TEST_DIR" "../worktree-test" 2>/dev/null || true
  exit 1
fi

kill -INT $PID
wait $PID 2>/dev/null || true

if [ -d "../worktree-test" ]; then
  echo "FAIL: Worktree was NOT cleaned up"
  rm -rf "$TEST_DIR" "../worktree-test" 2>/dev/null || true
  exit 1
fi

echo "PASS: Worktree cleaned up successfully"
rm -rf "$TEST_DIR" "../worktree-test" 2>/dev/null || true
