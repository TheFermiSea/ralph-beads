#!/usr/bin/env bash

set -euo pipefail

ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
FAILURES=0

section() {
  echo "== $1 =="
}

maybe_shellcheck() {
  local target=$1
  if command -v shellcheck >/dev/null 2>&1; then
    shellcheck -x "$target" || { echo "FAIL: shellcheck $target"; FAILURES=1; }
  else
    echo "SKIP: shellcheck not installed; skipping $target"
  fi
}

syntax_check() {
  local target=$1
  if ! bash -n "$target"; then
    echo "FAIL: bash -n $target"
    FAILURES=1
  fi
}

smoke_runner() {
  section "Smoke test: ralph-runner"
  local tmp
  tmp=$(mktemp -d)
  local mock="$tmp/claude"
  local log="$tmp/log"
  cat >"$mock" <<'MOCK'
#!/usr/bin/env bash
echo "$@" >>"$CLAUDE_SMOKE_LOG"
exit 0
MOCK
  chmod +x "$mock"
  CLAUDE_SMOKE_LOG="$log" CLAUDE_BIN="$mock" "$ROOT/commands/ralph-runner.sh" --dry-run "smoke-test"
  if ! grep -q "/ralph-beads" "$log"; then
    echo "FAIL: runner did not invoke claude with /ralph-beads"
    FAILURES=1
  fi
  rm -rf "$tmp"
}

snapshot_prompt() {
  section "Snapshot: ralph-beads prompt invariants"
  local file="$ROOT/commands/ralph-beads.md"
  local patterns=(
    "bd prime || echo"
    "--resume <id>"
    "bd --no-daemon ready --mol <mol-id> --limit 1 --json"
    "bd graph <epic-id>"
    "git add <path1> <path2>"
  )
  for pat in "${patterns[@]}"; do
    if ! grep -Fq -- "$pat" "$file"; then
      echo "FAIL: snapshot missing pattern: $pat"
      FAILURES=1
    fi
  done
}

section "Syntax checks"
syntax_check "$ROOT/scripts/check-deps.sh"
syntax_check "$ROOT/commands/ralph-runner.sh"

section "Shellcheck (if available)"
maybe_shellcheck "$ROOT/scripts/check-deps.sh"
maybe_shellcheck "$ROOT/commands/ralph-runner.sh"

section "Prompt lint"
"$ROOT/tests/lint-prompt-embedded.sh" || FAILURES=1

section "Logic verification"
"$ROOT/tests/verify-complexity.sh" || FAILURES=1

section "Safety verification"
"$ROOT/tests/verify-safety.sh" || FAILURES=1

smoke_runner || FAILURES=1
snapshot_prompt || FAILURES=1

section "OpenCode E2E structure"
"$ROOT/tests/e2e-opencode.sh" || FAILURES=1

if [ "$FAILURES" -ne 0 ]; then
  echo "Tests completed with failures."
  exit 1
fi

echo "All tests passed."
