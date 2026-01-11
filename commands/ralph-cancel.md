---
description: Cancel an active Ralph-Beads loop and preserve state
argument-hint: "[--epic <id>] [--reason <text>]"
---

# Ralph Cancel

Gracefully cancel an active Ralph-Beads loop while preserving all state in beads.

## Usage

```bash
/ralph-cancel                           # Cancel current loop
/ralph-cancel --epic <id>               # Cancel specific epic
/ralph-cancel --reason "Need to pivot"  # Cancel with reason
```

## Implementation

### Step 1: Identify Active Epic

**Priority order for finding epic:**

1. If `--epic <id>` provided, use that ID
2. Otherwise, check `.claude/ralph-loop.local.md` for active loop context
3. Fallback: Find most recent in_progress ralph epic

```bash
# Option 1: Explicit ID
if [ -n "$EPIC_ARG" ]; then
  EPIC_ID="$EPIC_ARG"

# Option 2: Active loop context file
elif [ -f ".claude/ralph-loop.local.md" ]; then
  # Extract epic ID from local context (format varies)
  EPIC_ID=$(grep -oP 'Epic:\s*\K[a-z0-9-]+' .claude/ralph-loop.local.md 2>/dev/null | head -1)

# Option 3: Fallback to most recent in_progress ralph epic
else
  EPIC_ID=$(bd list --type=epic --label=ralph --status=in_progress --json 2>/dev/null | \
    jq -r 'sort_by(.updated // .created // "") | reverse | .[0].id // empty')
fi

# Validate we found something
if [ -z "$EPIC_ID" ]; then
  echo "ERROR: No active Ralph-Beads epic found."
  echo "Provide an epic ID explicitly: /ralph-cancel --epic <id>"
  echo "Or check: bd list --type=epic --label=ralph"
  exit 1
fi

# Verify epic exists
bd show $EPIC_ID >/dev/null 2>&1 || { echo "ERROR: Epic $EPIC_ID not found"; exit 1; }
```

### Step 2: Confirmation (Safety Check)

Before cancelling, show what will be affected:

```bash
# Get epic details
TITLE=$(bd show $EPIC_ID --json | jq -r '.title // "Untitled"')
TOTAL=$(bd list --parent=$EPIC_ID --json 2>/dev/null | jq 'length // 0')
COMPLETE=$(bd list --parent=$EPIC_ID --status=closed --json 2>/dev/null | jq 'length // 0')
IN_PROG=$(bd list --parent=$EPIC_ID --status=in_progress --json 2>/dev/null | jq -r '.[0].title // empty')

echo "About to cancel Ralph-Beads loop:"
echo "  Epic: $EPIC_ID - $TITLE"
echo "  Progress: $COMPLETE/$TOTAL tasks complete"
[ -n "$IN_PROG" ] && echo "  In-progress task: $IN_PROG (will be preserved)"
echo ""
echo "This will pause the loop but preserve all state."
```

Use AskUserQuestion to confirm: "Proceed with cancellation?"

### Step 3: Update Epic State

```bash
bd set-state $EPIC_ID mode=paused
bd comments add $EPIC_ID "[CANCELLED] ${REASON:-Cancelled by user}. Resume with: /ralph-beads --epic $EPIC_ID"
```

### Step 4: Cancel Ralph Loop

Use the Skill tool to invoke `ralph-loop:cancel-ralph`.

### Step 5: Report Cancellation

Display confirmation with all details:

```bash
echo "Cancelled Ralph-Beads loop."
echo ""
echo "Epic: $EPIC_ID - $TITLE"
echo "Progress: $COMPLETE/$TOTAL tasks complete"
[ -n "$IN_PROG" ] && echo "In-progress: $IN_PROG (preserved)"
echo ""
echo "Resume with: /ralph-beads --epic $EPIC_ID"
```

**Note:** In-progress tasks are preserved - their status remains in_progress so work is not lost when resuming.

$ARGUMENTS
