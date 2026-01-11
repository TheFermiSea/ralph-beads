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
# Find most recent ralph epic
EPIC_ID=$(bd list --type=epic --label=ralph --json 2>/dev/null | jq -r 'sort_by(.updated) | reverse | .[0].id // empty')
if [ -z "$EPIC_ID" ]; then
  echo "No ralph epics found. Start one with: /ralph-beads --mode plan \"Your task\""
  exit 0
fi
```

### Step 3: Display Status

Run the following commands and format as a status report:

```bash
# Epic title and status
bd show <epic-id> --json | jq -r '"Epic: \(.id) - \(.title)\nStatus: \(.status) | Mode: \(.state.mode // "unknown")"'

# Task completion stats
TOTAL=$(bd list --parent=<epic-id> --json | jq 'length')
COMPLETE=$(bd list --parent=<epic-id> --status=closed --json | jq 'length')
PERCENT=$((COMPLETE * 100 / (TOTAL > 0 ? TOTAL : 1)))
echo "Progress: $COMPLETE/$TOTAL tasks complete ($PERCENT%)"

# Current task (in_progress)
CURRENT=$(bd list --parent=<epic-id> --status=in_progress --json | jq -r '.[0] | "\(.id) - \(.title)"' 2>/dev/null)
if [ -n "$CURRENT" ] && [ "$CURRENT" != "null - null" ]; then
  echo "Current: $CURRENT"
fi

# Blocked tasks count
BLOCKED=$(bd list --parent=<epic-id> --status=blocked --json 2>/dev/null | jq 'length')
echo "Blocked: $BLOCKED tasks"

# Ready to work
echo ""
echo "=== Ready to Work ==="
bd ready --parent=<epic-id> --limit=5
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
