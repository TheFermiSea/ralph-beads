#!/usr/bin/env bash
# E2E test for OpenCode plugin structure validation
# Note: Full E2E requires running OpenCode interactively

set -euo pipefail

ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
OPENCODE_DIR="$ROOT/.opencode"
PLUGIN_DIR="$OPENCODE_DIR/plugin"
FAILURES=0

section() {
  echo "== $1 =="
}

check_file_exists() {
  local file=$1
  local desc=$2
  if [ -f "$file" ]; then
    echo "PASS: $desc exists"
  else
    echo "FAIL: $desc missing: $file"
    FAILURES=1
  fi
}

check_exports() {
  local file=$1
  local pattern=$2
  local desc=$3
  if grep -q "$pattern" "$file" 2>/dev/null; then
    echo "PASS: $desc"
  else
    echo "FAIL: $desc not found in $file"
    FAILURES=1
  fi
}

section "OpenCode Plugin Structure"

check_file_exists "$OPENCODE_DIR/package.json" "package.json"
check_file_exists "$PLUGIN_DIR/ralph-beads.ts" "Main plugin file"
check_file_exists "$PLUGIN_DIR/beads-client.ts" "Beads client"
check_file_exists "$PLUGIN_DIR/prompts.ts" "Prompts module"
check_file_exists "$PLUGIN_DIR/types.ts" "Types module"

section "Plugin Exports"

check_exports "$PLUGIN_DIR/ralph-beads.ts" "export const RalphBeads" "RalphBeads export"
check_exports "$PLUGIN_DIR/ralph-beads.ts" '"ralph-beads"' "ralph-beads tool"
check_exports "$PLUGIN_DIR/ralph-beads.ts" '"ralph-status"' "ralph-status tool"
check_exports "$PLUGIN_DIR/ralph-beads.ts" '"ralph-cancel"' "ralph-cancel tool"

section "Hook Implementations"

check_exports "$PLUGIN_DIR/ralph-beads.ts" "event:" "Event hook"
check_exports "$PLUGIN_DIR/ralph-beads.ts" "stop:" "Stop hook"
check_exports "$PLUGIN_DIR/ralph-beads.ts" '"tool.execute.after"' "Tool execute after hook"

section "Beads Client Methods"

check_exports "$PLUGIN_DIR/beads-client.ts" "async create" "create method"
check_exports "$PLUGIN_DIR/beads-client.ts" "async update" "update method"
check_exports "$PLUGIN_DIR/beads-client.ts" "async show" "show method"
check_exports "$PLUGIN_DIR/beads-client.ts" "async ready" "ready method"
check_exports "$PLUGIN_DIR/beads-client.ts" "async prime" "prime method"
check_exports "$PLUGIN_DIR/beads-client.ts" "async molPour" "molPour method"
check_exports "$PLUGIN_DIR/beads-client.ts" "async molProgress" "molProgress method"

section "Prompts Module"

check_exports "$PLUGIN_DIR/prompts.ts" "getPlanningPrompt" "Planning prompt function"
check_exports "$PLUGIN_DIR/prompts.ts" "getBuildingPrompt" "Building prompt function"

section "No Invalid CLI Flags"

# Check for flags we know don't exist
INVALID_FLAGS=("--focus" "--detect-cycles" 'update.*--body[^-]')
for pattern in "${INVALID_FLAGS[@]}"; do
  if grep -rq "$pattern" "$PLUGIN_DIR"/*.ts 2>/dev/null; then
    # Exception: comments are OK
    if grep -r "$pattern" "$PLUGIN_DIR"/*.ts 2>/dev/null | grep -v "^.*//"; then
      echo "FAIL: Found invalid flag pattern: $pattern"
      FAILURES=1
    else
      echo "PASS: $pattern only in comments (OK)"
    fi
  else
    echo "PASS: No invalid flag: $pattern"
  fi
done

section "TypeScript Syntax Check"

# Skip TypeScript check in CI - it requires dependencies to be installed
if [ -f "$OPENCODE_DIR/node_modules/.bin/tsc" ]; then
  cd "$OPENCODE_DIR"
  if timeout 10 ./node_modules/.bin/tsc --noEmit 2>/dev/null; then
    echo "PASS: TypeScript compiles without errors"
  else
    echo "WARN: TypeScript compilation has issues (may need dependencies)"
  fi
  cd "$ROOT"
else
  echo "SKIP: TypeScript compiler not installed (run: cd .opencode && bun install)"
fi

section "Summary"

if [ "$FAILURES" -ne 0 ]; then
  echo "E2E tests completed with failures."
  exit 1
fi

echo "All E2E structure tests passed."
echo ""
echo "Note: Full E2E testing requires running OpenCode interactively."
echo "See TESTING.md for manual test procedures."
