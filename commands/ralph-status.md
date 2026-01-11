---
description: Check status of a Ralph-Beads epic
argument-hint: "<epic-id>"
---

# Ralph Status

Display comprehensive status of a Ralph-Beads epic including:
- Epic completion percentage
- Task breakdown (complete/in-progress/blocked/pending)
- Recent activity
- Dependency graph

## Usage

```bash
/ralph-status <epic-id>
```

## Implementation

Parse `<epic-id>` from $ARGUMENTS.

Run the following commands and display results:

```bash
# Epic overview
bd show <epic-id>

# Completion status
bd epic status <epic-id>

# Task breakdown
echo "=== Tasks ==="
bd list --epic=<epic-id> --status=open
bd list --epic=<epic-id> --status=in_progress
bd list --epic=<epic-id> --status=closed

# Ready work
echo "=== Ready to Work ==="
bd ready --epic=<epic-id>

# Recent activity
echo "=== Recent Activity ==="
bd comments list <epic-id> --limit=10

# Dependencies
echo "=== Dependency Graph ==="
bd graph <epic-id>
```

Format output as a clear status report.

$ARGUMENTS
