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

If `--epic <id>` provided, use that ID.
Otherwise, look for `.claude/ralph-loop.local.md` to find active epic.

### Step 2: Update Epic State

```bash
bd set-state <epic-id> mode=paused --reason "<reason or 'Cancelled by user'>"
bd comments add <epic-id> --body "[CANCELLED] <reason>. Resume with: /ralph-beads --epic <epic-id>"
```

### Step 3: Cancel Ralph Loop

Use the Skill tool to invoke `ralph-loop:cancel-ralph`.

### Step 4: Confirm Cancellation

Display:
- Epic ID
- Current progress (tasks complete/total)
- How to resume

$ARGUMENTS
