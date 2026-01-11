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
bd comments add <epic-id> "[CANCELLED] <reason>. Resume with: /ralph-beads --epic <epic-id>"
```

### Step 3: Cancel Ralph Loop

Use the Skill tool to invoke `ralph-loop:cancel-ralph`.

### Step 4: Report In-Progress Task

Check for any task currently in_progress under this epic:

```bash
bd list --parent=<epic-id> --status=in_progress --json
```

If a task is in progress, note it for the confirmation message. **Do NOT change its status** - the task remains in_progress so work is not lost when resuming.

### Step 5: Confirm Cancellation

Display:
- Epic ID and title
- Current progress (tasks complete/total)
- In-progress task (if any) - note that it was preserved
- How to resume: `/ralph-beads --epic <epic-id>`

Example output:
```
Cancelled Ralph-Beads loop.

Epic: rb-xxx - Feature implementation
Progress: 3/7 tasks complete (43%)
In-progress: rb-yyy - Implement validation (preserved)

Resume with: /ralph-beads --epic rb-xxx
```

$ARGUMENTS
