---
description: Check status of a Ralph-Beads epic
argument-hint: "[--verbose] [<epic-id>]"
---

# Ralph Status

Display comprehensive status of a Ralph-Beads epic including:
- Epic title, status, and mode
- Task completion: N/M complete (X%)
- Current task (if in_progress)
- Blocked tasks count
- Recent iteration logs (with --verbose)

## Usage

```bash
/ralph-status [--verbose] [<epic-id>]
```

**Arguments:**
- `<epic-id>` - Optional. Epic to query. If omitted, finds most recent ralph-labeled epic.
- `--verbose` - Optional. Include recent iteration history from comments.

## Implementation

### Step 1: Parse Arguments

Parse `$ARGUMENTS` to extract:
- `--verbose` flag (boolean)
- `<epic-id>` (optional string)

### Step 2: Find Epic

If no `<epic-id>` provided, find the most recent ralph-labeled epic:

```bash
# Find most recent ralph epic (handle empty list and missing updated field)
EPIC_JSON=$(bd list --type=epic --label=ralph --json 2>/dev/null || echo "[]")
EPIC_ID=$(echo "$EPIC_JSON" | jq -r '
  if length == 0 then empty
  else sort_by(.updated // .created // "") | reverse | .[0].id // empty
  end
')

if [ -z "$EPIC_ID" ]; then
  echo "No ralph epics found. Start one with: /ralph-beads --mode plan \"Your task\""
  exit 0
fi
```

### Step 3: Display Status

Run the following commands and format as a status report:

```bash
# Epic title and status (validate epic exists first)
EPIC_DATA=$(bd show <epic-id> --json 2>/dev/null)
if [ -z "$EPIC_DATA" ]; then
  echo "ERROR: Epic <epic-id> not found"
  exit 1
fi
echo "$EPIC_DATA" | jq -r '"Epic: \(.id // "?") - \(.title // "Untitled")\nStatus: \(.status // "unknown") | Mode: \(.state.mode // "not set")"'

# Task completion stats (handle empty lists)
TOTAL_JSON=$(bd list --parent=<epic-id> --json 2>/dev/null || echo "[]")
COMPLETE_JSON=$(bd list --parent=<epic-id> --status=closed --json 2>/dev/null || echo "[]")
TOTAL=$(echo "$TOTAL_JSON" | jq 'length // 0')
COMPLETE=$(echo "$COMPLETE_JSON" | jq 'length // 0')

# Safe division - handle shell arithmetic
if [ "$TOTAL" -gt 0 ] 2>/dev/null; then
  PERCENT=$((COMPLETE * 100 / TOTAL))
else
  PERCENT=0
  echo "Progress: No tasks created yet (still in planning?)"
fi
[ "$TOTAL" -gt 0 ] && echo "Progress: $COMPLETE/$TOTAL tasks complete ($PERCENT%)"

# Current task (in_progress) - safe array access
CURRENT_JSON=$(bd list --parent=<epic-id> --status=in_progress --json 2>/dev/null || echo "[]")
CURRENT=$(echo "$CURRENT_JSON" | jq -r 'if length > 0 then .[0] | "\(.id) - \(.title)" else "" end')
[ -n "$CURRENT" ] && echo "Current: $CURRENT"

# Blocked tasks count
BLOCKED_JSON=$(bd list --parent=<epic-id> --status=blocked --json 2>/dev/null || echo "[]")
BLOCKED=$(echo "$BLOCKED_JSON" | jq 'length // 0')
echo "Blocked: $BLOCKED tasks"

# Ready to work
echo ""
echo "=== Ready to Work ==="
bd ready --parent=<epic-id> --limit=5 || echo "None"
```

### Step 4: Verbose Output (if --verbose)

If `--verbose` flag is set, include iteration history:

```bash
echo ""
echo "=== Recent Iterations ==="
bd comments <epic-id>
```

### Step 5: Dependency Graph

Always show the dependency graph for context:

```bash
echo ""
echo "=== Dependency Graph ==="
bd graph <epic-id>
```

## Example Output

```
Epic: bd-abc123 - Ralph: Add user authentication
Status: in_progress | Mode: building
Progress: 3/7 tasks complete (43%)
Current: bd-xyz789 - Implement login validation
Blocked: 2 tasks

=== Ready to Work ===
bd-xyz790 - Add password hashing

=== Dependency Graph ===
[graph output]
```

With `--verbose`:
```
...
=== Recent Iterations ===
  [iter:5] [task:bd-xyz789] [tests:12/0/0] Started login validation
  [iter:4] [task:bd-xyz788] [tests:10/0/0] Completed session management
```

$ARGUMENTS
