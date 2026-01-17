#!/usr/bin/env bash
set -euo pipefail

EXTRACTED_LOGIC=$(mktemp)
cat > "$EXTRACTED_LOGIC" << 'EOF'
#!/bin/bash
TASK="$1"
COMPLEXITY_ARG="$2"

COMPLEXITY="${COMPLEXITY_ARG:-standard}"
if [ -z "$COMPLEXITY_ARG" ]; then
  if echo "$TASK" | grep -qiE 'fix typo|update comment|rename|spelling|whitespace'; then
    COMPLEXITY="trivial"
  elif echo "$TASK" | grep -qiE 'add (button|toggle|flag)|toggle|remove unused|update (version|dep)'; then
    COMPLEXITY="simple"
  elif echo "$TASK" | grep -qiE 'auth|security|payment|migration|credential|token|encrypt|password'; then
    COMPLEXITY="critical"
  fi
fi
echo "$COMPLEXITY"
EOF
chmod +x "$EXTRACTED_LOGIC"

FAILURES=0
check() {
  local task="$1"
  local arg="$2"
  local expected="$3"
  local result
  result=$("$EXTRACTED_LOGIC" "$task" "$arg")
  if [ "$result" != "$expected" ]; then
    echo "FAIL: '$task' (Arg: $arg) -> Expected $expected, got $result"
    FAILURES=1
  else
    echo "PASS: '$task' -> $expected"
  fi
}

echo "== Testing Complexity Heuristics =="
check "Fix typo in readme" "" "trivial"
check "Update comments" "" "trivial"
check "Add feature toggle" "" "simple"
check "Refactor user auth" "" "critical"
check "Standard refactor" "" "standard"
check "Fix typo" "critical" "critical"

rm "$EXTRACTED_LOGIC"
exit "$FAILURES"
